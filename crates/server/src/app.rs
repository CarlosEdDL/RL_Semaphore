//! The shared application state, the router, and [`Server`]: bind, `local_addr` and `serve`
//! (R4.4-R4.7, R7.2-R7.3).

use std::future::Future;
use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::ws::Utf8Bytes;
use axum::routing::get;
use rl_semaphore_env::Episode;
use rl_semaphore_protocol::{Hello, Layout, ServerMessage};
use rl_semaphore_sim::{FixedTimePlan, Scenario};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tower_http::trace::TraceLayer;

use crate::config::ServerConfig;
use crate::error::ServerError;
use crate::pace::{Decision, Ticker};
use crate::sim_loop::{self, Latest, Tick};
use crate::{health, ws};

/// How long `serve` waits for every open WebSocket to finish sending its close frame, on top of
/// axum's own graceful shutdown, before giving up (R4.6).
const SHUTDOWN_TASK_TIMEOUT: Duration = Duration::from_secs(10);

/// State shared by every request handler.
#[derive(Clone)]
pub(crate) struct AppState {
    /// The latest published tick and the latest `Metrics` of its episode.
    pub(crate) latest: Arc<RwLock<Latest>>,
    /// The broadcast channel every connection subscribes to.
    pub(crate) tx: broadcast::Sender<Tick>,
    /// The encoded `Hello`, sent once per connection.
    pub(crate) hello: Utf8Bytes,
    /// The number of open WebSocket connections (R6.7).
    pub(crate) clients: Arc<AtomicUsize>,
    /// `true` once the server is shutting down; WebSocket handlers watch it (R6.6).
    pub(crate) shutdown: watch::Receiver<bool>,
    /// Held by every open WebSocket handler for its whole lifetime, so `serve` can tell, by
    /// waiting for every clone to drop, when every connection has actually finished (R4.6).
    pub(crate) task_tx: mpsc::Sender<()>,
}

/// Builds the router: `/healthz`, `/ws`, and request tracing (R7.2, R7.3).
fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health::handler))
        .route("/ws", get(ws::handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// A bound, not-yet-serving server (R4.4).
pub struct Server {
    listener: TcpListener,
    local_addr: SocketAddr,
    scenario: Scenario,
    plan: FixedTimePlan,
    seed: u64,
    steps_per_episode: u64,
    metrics_every: u64,
    episode: Episode,
    ticker: Ticker,
    latest: Arc<RwLock<Latest>>,
    tx: broadcast::Sender<Tick>,
    hello: Utf8Bytes,
    clients: Arc<AtomicUsize>,
}

impl Server {
    /// Validates `config`, builds the step-0 tick of episode 0 (R4.5), and binds the listener.
    ///
    /// Nothing runs on a dedicated thread yet, and no connection is accepted yet: that starts
    /// in [`Server::serve`].
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::MissingFixedTimePlan`] or [`ServerError::InvalidSpeed`] if
    /// `config` is invalid (checked before anything else), [`ServerError::Bind`] if the address
    /// cannot be bound, or [`ServerError::Encode`] if the initial messages cannot be encoded.
    pub async fn bind(config: ServerConfig) -> Result<Self, ServerError> {
        config.validate()?;
        // INVARIANT: `validate` above already checked this is `Some`.
        let plan = config
            .scenario
            .fixed_time()
            .cloned()
            .ok_or(ServerError::MissingFixedTimePlan)?;
        let steps_per_episode = config.steps_per_episode.get();
        let metrics_every = config.metrics_every.get();

        let hello = ServerMessage::Hello(Hello::new(Layout::from_scenario(&config.scenario)))
            .to_json()
            .map(Utf8Bytes::from)
            .map_err(ServerError::Encode)?;

        let episode = Episode::new(&config.scenario, config.seed);
        let start = Instant::now();
        let mut ticker = Ticker::new(config.pace, start);
        // Tick 0's deadline is `start` itself, already due.
        debug_assert!(matches!(ticker.poll(start), Decision::Publish));
        let tick0 = sim_loop::build_tick(&episode, 0, steps_per_episode, metrics_every)?;
        let metrics0 = tick0
            .metrics
            .clone()
            .unwrap_or_else(|| unreachable!("step 0 always carries metrics (R5.4)"));

        let (tx, _rx) = broadcast::channel(config.broadcast_capacity.get());
        let latest = Arc::new(RwLock::new(Latest {
            tick: tick0,
            metrics: metrics0,
        }));

        let listener =
            TcpListener::bind(config.bind)
                .await
                .map_err(|source| ServerError::Bind {
                    addr: config.bind,
                    source,
                })?;
        let local_addr = listener.local_addr().map_err(|source| ServerError::Bind {
            addr: config.bind,
            source,
        })?;

        sim_loop::log_episode_started(0, config.seed);

        Ok(Self {
            listener,
            local_addr,
            scenario: config.scenario,
            plan,
            seed: config.seed,
            steps_per_episode,
            metrics_every,
            episode,
            ticker,
            latest,
            tx,
            hello,
            clients: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// The address actually bound, useful when `config.bind` had port 0.
    #[must_use]
    pub const fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Serves connections until `shutdown` completes, then shuts down gracefully (R4.6, R4.7).
    ///
    /// Every open WebSocket receives a close frame with code 1001, the sim thread stops within
    /// one step period plus a small constant, and this call joins it before returning.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Serve`] if accepting connections fails, [`ServerError::Sim`] or
    /// [`ServerError::Encode`] if the sim thread hit that error, or
    /// [`ServerError::SimPanicked`] if it panicked.
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), ServerError> {
        let Self {
            listener,
            local_addr,
            scenario,
            plan,
            seed,
            steps_per_episode,
            metrics_every,
            episode,
            ticker,
            latest,
            tx,
            hello,
            clients,
        } = self;

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let (err_tx, mut err_rx) = oneshot::channel::<ServerError>();
        let (task_tx, mut task_rx) = mpsc::channel::<()>(1);

        let state = AppState {
            latest: Arc::clone(&latest),
            tx: tx.clone(),
            hello,
            clients,
            shutdown: shutdown_rx,
            task_tx,
        };
        let router = router(state);

        tracing::info!(addr = %local_addr, "listening");

        let sim_thread = std::thread::Builder::new()
            .name("sim".to_owned())
            .spawn(move || {
                sim_loop::run(
                    &scenario,
                    &plan,
                    seed,
                    steps_per_episode,
                    metrics_every,
                    0,
                    episode,
                    ticker,
                    &latest,
                    &tx,
                    &stop_rx,
                    err_tx,
                )
            })
            .map_err(ServerError::Serve)?;

        let sim_error: Arc<std::sync::Mutex<Option<ServerError>>> =
            Arc::new(std::sync::Mutex::new(None));
        let sim_error_writer = Arc::clone(&sim_error);
        let combined_shutdown = async move {
            tokio::select! {
                () = shutdown => {}
                res = &mut err_rx => {
                    if let Ok(err) = res
                        && let Ok(mut guard) = sim_error_writer.lock()
                    {
                        *guard = Some(err);
                    }
                }
            }
            let _ = shutdown_tx.send(true);
        };

        let serve_result = axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(combined_shutdown)
            .await
            .map_err(ServerError::Serve);

        // Tell the sim thread to stop (harmless if it already has), then wait, bounded, for
        // every open WebSocket handler to finish sending its close frame (R4.6's "notes and
        // risks": axum's own graceful shutdown does not reliably wait for an upgraded
        // connection).
        let _ = stop_tx.send(());
        let _ = tokio::time::timeout(SHUTDOWN_TASK_TIMEOUT, async {
            while task_rx.recv().await.is_some() {}
        })
        .await;

        let join_result = tokio::task::spawn_blocking(move || sim_thread.join()).await;

        let sim_error = sim_error.lock().ok().and_then(|mut guard| guard.take());
        if let Some(err) = sim_error {
            return Err(err);
        }
        serve_result?;
        match join_result {
            Ok(Ok(_stop_reason)) => Ok(()),
            Ok(Err(_)) | Err(_) => Err(ServerError::SimPanicked),
        }
    }
}

/// Binds `config` and serves until `shutdown` completes.
///
/// # Errors
///
/// See [`Server::bind`] and [`Server::serve`].
pub async fn run(
    config: ServerConfig,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), ServerError> {
    Server::bind(config).await?.serve(shutdown).await
}
