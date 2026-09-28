//! The sim thread (R5): one dedicated OS thread driving an [`rl_semaphore_env::Episode`],
//! paced by [`crate::pace::Ticker`], publishing an encoded [`Tick`] to every connection.

use std::sync::RwLock;
use std::sync::mpsc::{Receiver as StdReceiver, RecvTimeoutError, TryRecvError};
use std::time::Instant;

use axum::extract::ws::Utf8Bytes;
use rl_semaphore_env::{Episode, FixedTime};
use rl_semaphore_protocol::{Metrics, ServerMessage, Snapshot};
use rl_semaphore_sim::{FixedTimePlan, Scenario};
use tokio::sync::broadcast;
use tokio::sync::oneshot::Sender as OneshotSender;

use crate::error::ServerError;
use crate::pace::{Decision, Ticker};

/// One published step: the episode and step it belongs to, and the encoded
/// [`ServerMessage::Snapshot`] and, when due, [`ServerMessage::Metrics`] of that step (R5.4,
/// R5.5). Encoded once by the sim thread; connections forward the text as is.
#[derive(Debug, Clone)]
pub(crate) struct Tick {
    /// The episode this tick belongs to.
    pub episode: u64,
    /// The step within that episode.
    pub step: u64,
    /// The encoded `Snapshot` message.
    pub snapshot: Utf8Bytes,
    /// The encoded `Metrics` message, when this step is due for one (R5.4).
    pub metrics: Option<Utf8Bytes>,
}

impl Tick {
    /// This tick's key, `(episode, step)`, compared lexicographically (R6.2).
    pub(crate) const fn key(&self) -> (u64, u64) {
        (self.episode, self.step)
    }
}

/// The latest published state (R5.6, R6.2, R7.1): the most recent tick, and the most recent
/// `Metrics` text of its episode (which may be from an earlier step, when the latest tick has
/// none).
#[derive(Debug, Clone)]
pub(crate) struct Latest {
    /// The most recently published tick.
    pub tick: Tick,
    /// The most recent `Metrics` text of `tick.episode`.
    pub metrics: Utf8Bytes,
}

/// Encodes `msg` to a wire-ready [`Utf8Bytes`].
fn encode(msg: &ServerMessage) -> Result<Utf8Bytes, ServerError> {
    msg.to_json()
        .map(Utf8Bytes::from)
        .map_err(ServerError::Encode)
}

/// Builds the tick for `episode` (at index `episode_index`) at its current step, including a
/// `Metrics` message if the step is 0, a multiple of `metrics_every`, or `steps_per_episode`
/// (R5.4).
pub(crate) fn build_tick(
    episode: &Episode,
    episode_index: u64,
    steps_per_episode: u64,
    metrics_every: u64,
) -> Result<Tick, ServerError> {
    let step = episode.sim().step_count();
    let seed = episode.seed();

    let mut snapshot = Snapshot::from_sim(episode.sim());
    snapshot.episode = episode_index;
    snapshot.seed = seed;

    let due = step == 0 || step.is_multiple_of(metrics_every) || step == steps_per_episode;
    let metrics = if due {
        let mut m = Metrics::from_sim(&episode.summary(), episode.sim());
        m.episode = episode_index;
        m.seed = seed;
        Some(encode(&ServerMessage::Metrics(m))?)
    } else {
        None
    };

    Ok(Tick {
        episode: episode_index,
        step,
        snapshot: encode(&ServerMessage::Snapshot(snapshot))?,
        metrics,
    })
}

/// Stores `tick` as the latest state, keeping the latest `Metrics` of the episode when `tick`
/// has none (R5.6), then broadcasts it. A send with no receivers is not an error.
pub(crate) fn publish(latest: &RwLock<Latest>, tx: &broadcast::Sender<Tick>, tick: Tick) {
    // INVARIANT: every episode's step 0 carries `Metrics` (R5.4), so by the time a tick of an
    // episode has none, `latest` already holds one from that same episode.
    let metrics = tick.metrics.clone().unwrap_or_else(|| {
        latest
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .metrics
            .clone()
    });
    {
        let mut guard = latest
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Latest {
            tick: tick.clone(),
            metrics,
        };
    }
    // A send with no receivers returns `Err`, which is not an error for us (R5.6).
    let _ = tx.send(tick);
}

/// Why the sim thread stopped.
pub(crate) enum StopReason {
    /// The stop channel fired or disconnected (R4.6).
    Stopped,
    /// The episode failed, or a message could not be encoded (R4.7). Already reported on the
    /// error channel.
    Failed,
}

/// Drives one episode after another, publishing a tick every period, until `stop` fires or a
/// step fails.
///
/// `episode_index`, `episode` and `ticker` are the state of the episode already in progress:
/// its step 0 was already built and published by the caller before spawning this loop, so R4.5
/// holds. This function continues from step 1 of that same episode.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    scenario: &Scenario,
    plan: &FixedTimePlan,
    base_seed: u64,
    steps_per_episode: u64,
    metrics_every: u64,
    mut episode_index: u64,
    mut episode: Episode,
    mut ticker: Ticker,
    latest: &RwLock<Latest>,
    tx: &broadcast::Sender<Tick>,
    stop: &StdReceiver<()>,
    errors: OneshotSender<ServerError>,
) -> StopReason {
    let mut controller = FixedTime::new(plan);

    loop {
        match ticker.poll(Instant::now()) {
            Decision::Publish => {
                match stop.try_recv() {
                    Ok(()) | Err(TryRecvError::Disconnected) => return StopReason::Stopped,
                    Err(TryRecvError::Empty) => {}
                }

                if episode.sim().step_count() == steps_per_episode {
                    log_episode_ended(episode_index, &episode);
                    episode_index += 1;
                    let seed = base_seed.wrapping_add(episode_index);
                    episode = Episode::new(scenario, seed);
                    log_episode_started(episode_index, seed);
                } else if let Err(err) = episode.step(&mut controller) {
                    tracing::error!(error = %err, "sim thread failed");
                    let _ = errors.send(ServerError::Sim(err));
                    return StopReason::Failed;
                }

                let tick =
                    match build_tick(&episode, episode_index, steps_per_episode, metrics_every) {
                        Ok(tick) => tick,
                        Err(err) => {
                            tracing::error!(error = %err, "sim thread failed to encode a message");
                            let _ = errors.send(err);
                            return StopReason::Failed;
                        }
                    };
                publish(latest, tx, tick);
            }
            Decision::Wait(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                match stop.recv_timeout(remaining) {
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => return StopReason::Stopped,
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
        }
    }
}

/// Logs the `info` event of an episode starting (R5.3).
pub(crate) fn log_episode_started(episode: u64, seed: u64) {
    tracing::info!(episode, seed, "episode started");
}

/// Logs the `info` event of an episode ending (R5.3): the mean and p99 wait, if any, and the
/// departed count.
fn log_episode_ended(episode: u64, ep: &Episode) {
    let summary = ep.summary();
    if let Some(wait) = &summary.wait {
        tracing::info!(
            episode,
            mean_wait_s = wait.mean_s,
            p99_wait_s = wait.p99_s,
            departed = summary.departed,
            "episode ended"
        );
    } else {
        tracing::info!(episode, departed = summary.departed, "episode ended");
    }
}
