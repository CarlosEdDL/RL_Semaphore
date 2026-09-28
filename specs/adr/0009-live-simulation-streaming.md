# 0009. Live simulation streaming

**Status:** Accepted
**Date:** 2026-09-27

## Context

Phase 2.2 puts the simulator on the network: an Axum server that streams the 2.1 wire protocol
from a running fixed-time simulation to any number of browsers (2.3 onward), and to the training
dashboard of Stage 8. The simulation has to run somewhere other than inside a request handler,
has to keep running when nobody is watching, and has to serve a client that joins after it has
already started. It also has to run forever for a demo, which a single episode cannot do without
either freezing at the end or growing an unbounded metrics collection. Several designs had to be
picked before any handler code could be written: how many simulations run, how state reaches a
socket, what a late joiner sees, how a slow client is treated, how time is paced, and how the
stepping loop is shared with `run_episode` and the evaluations of Stage 3.

Alternatives considered:

- **One simulation per connection.** Every browser would get its own episode, at its own pace,
  which fits a "control this instance" tool but not a demo of one running policy. CPU and memory
  would grow with the number of viewers, and two viewers could never compare notes on "the same
  moment." Stage 8's training dashboard needs exactly one shared run per training job, so this
  design would have to be replaced there anyway.
- **A `tokio::sync::watch` channel of only the latest tick.** Cheap and simple, but a client that
  reads slower than the tick rate would silently skip *every* tick between its reads, not just
  the ones it fell behind on, and the sim would have no signal at all of how far behind a client
  is (useful for the `debug` lag event of R6.3).
- **Splitting `Snapshot` and `Metrics` into two broadcast channels.** Would let a `Metrics` arrive
  in a client's stream ahead of the `Snapshot` of the same or a later step, if the two channels
  drop ticks independently under lag, which breaks the ordering rule 2.1 already put in the wire
  format's docs. Bundling them into one broadcast item keeps that rule true by construction.
  See ADR-0008's "lag with bundled ticks" note.
- **Re-encoding each message per connection.** Correct, but CPU cost then grows with the number
  of clients instead of staying at one encode per tick; at 11.1's scale (many training-monitor
  viewers) that matters.
- **Running the sim on a Tokio task instead of a dedicated OS thread.** `tech-stack.md` already
  places long-running, CPU-bound work off the async runtime, so the sim never competes with
  request handling for a worker thread, and a slow step never delays an HTTP response.
- **One long episode instead of looping.** A demo would either stop after `steps_per_episode`
  steps or run one arbitrarily long episode, whose `EpisodeMetrics` keeps every departed
  vehicle's wait in memory for the life of the process and whose statistics stop reflecting
  recent behaviour. Looping with a fresh episode and the next seed keeps memory bounded and the
  demo running indefinitely.
- **A separate "episode started" control message.** Rejected: a client that drops exactly that
  one message under lag would have no way to learn that `step` went back to 0. Stamping every
  `Snapshot` and `Metrics` with `episode` and `seed` survives any lag pattern.
- **A second copy of the step loop in `server`.** `run_episode` already runs a whole episode in
  one call; the server needs to stop after each step to publish it. Copying the loop would give
  the workspace two places that could drift apart on the exact order of "spawn arrivals, ask the
  controller, step, observe."

## Decision

**One simulation, on a dedicated thread, broadcast to every connection.** A `std::thread`, not a
Tokio task, drives one `rl_semaphore_env::Episode` with a `FixedTime` controller. It publishes
each step to every connection through a `tokio::sync::broadcast` channel. Every viewer sees the
same world at the same moment; CPU cost does not grow with the number of viewers. `tokio::sync`
channels and a `std::sync::RwLock` are usable from a plain thread with no runtime handle, so the
thread needs nothing async.

**A tick is a `Snapshot`, encoded once, with an optional `Metrics` alongside it.** Each step the
sim thread encodes that step's `Snapshot` to JSON, and, when due (step 0, every `metrics_every`th
step, and the last step of the episode), that step's `Metrics`, and treats the pair as one
broadcast item, a *tick*. Connections forward the ready text (an `axum::extract::ws::Utf8Bytes`,
cheap to clone) without decoding or re-encoding it. Because a tick is one item, lag drops a whole
tick, so a `Metrics` can never be delivered ahead of the `Snapshot` of its own step, keeping
2.1's ordering rule.

**Catch-up on connect, lag skipping while connected.** A new connection subscribes to the
broadcast *before* reading the latest published state, so no tick published after that read is
missed; it then sends `Hello`, the latest `Snapshot`, and the latest `Metrics` of the current
episode (kept even when the very latest tick has none), before forwarding the live stream. A
tick already covered by that catch-up read is filtered out by comparing keys, so the same tick is
never delivered twice. When a connection's receiver reports lag, it logs a `debug` event with the
count and continues from the oldest tick still buffered; the simulation never waits for a slow
client, and no client can slow down another.

**Wall-clock pacing with a speed factor, and a pure deadline function.** `Pace::RealTime { speed
}` publishes tick `k` no earlier than `start + k * (step_s / speed)`, computed from an absolute
reference so the schedule does not drift; falling more than one period behind resets the
reference to now instead of bursting through the backlog. `Pace::Unthrottled` never sleeps, for
tests that need to run in milliseconds. The deadline logic (`Pacer`/`Ticker`) is a plain type
that takes an `Instant` and returns a decision, so it is unit-tested without a real clock.

**Episodes loop with the next seed, stamped on every message.** After `steps_per_episode` steps,
the run starts a fresh episode with `seed.wrapping_add(episode)`, so the whole stream stays
reproducible from one base seed and memory stays bounded. `Snapshot` and `Metrics` each carry
`episode` and `seed` (2.1, added compatibly under its `#[serde(default)]` rule), so a client
always knows which episode a message belongs to, including one it reached only after skipping
frames under lag.

**One step loop, in `env`.** A public `env::Episode` now owns one episode's `Simulation`,
`Demand`, `EpisodeMetrics` and signal counts, with no controller of its own. Its `step` runs one
iteration of the loop `run_episode` used to run inline: draw and spawn the arrivals, ask the
controller for a command, step, count the signal's outcome, observe the metrics. `run_episode` is
now `Episode::new`, `steps` calls to `step`, then its report. The CLI's `simulate`, the server's
sim thread, and Stage 3's evaluations all share this one loop.

## Consequences

### Positive

- CPU and memory for the sim thread do not grow with the number of connections; encoding happens
  once per tick regardless of how many clients are watching.
- The design carries over to Stage 8 without change: a training run is one more simulation on
  one more dedicated thread, broadcast to whoever is watching that run.
- A late joiner sees the current state immediately, never an empty one, and never has to wait for
  the next step to render something.
- The pacing logic is fully unit-tested without sleeping, so its drift and reset behaviour is
  checked in milliseconds, not by timing a real clock in CI.
- `Episode` gives Stage 3's evaluations the same one-step-at-a-time control the server needs,
  instead of forcing a choice between `run_episode`'s all-at-once loop and a second copy of it.
- The demo runs indefinitely with bounded memory, with no operator action needed between
  episodes.

### Negative

- A connection's forwarding task can still block on its own slow socket write; that is by design
  (R6.5: it never blocks the sim or another connection), but it means a single very slow client
  can accumulate a large backlog of unsent bytes in its own task before the kernel's socket
  buffer or the next lag event catches up with it.
- `axum`'s own graceful shutdown does not reliably wait for a connection already upgraded to a
  WebSocket (hyper's upgrade handling can consider the HTTP exchange "done" as soon as the
  upgrade completes). `Server::serve` therefore tracks open WebSocket handlers itself and waits
  for them, bounded by a timeout, instead of trusting `with_graceful_shutdown` alone.
- Two numbers about the same lane travel in one `Metrics` message: the instantaneous `queue_now`
  and the episode-averaged `queue_by_lane` inside `summary`. ADR-0008 already flagged this; 2.2
  does not add a new one, but the server is one more place a careless reader could conflate them.
- The sim thread's pacing is wall-clock time, so a heavily loaded host can make `RealTime` ticks
  arrive later than their nominal schedule; the reset-on-lateness rule bounds the damage (no
  burst of queued ticks) but does not remove the delay itself.
