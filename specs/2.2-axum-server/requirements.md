# 2.2 Axum server: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

Notation: the *example* is `configs/single-intersection.toml`. A *tick* is what the sim thread publishes for one step: the encoded `Snapshot` of that step and, when due, the encoded `Metrics` of the same step. The *key* of a `Snapshot` or `Metrics` is the pair `(episode, step)`, compared lexicographically. The *wire* is as defined in 2.1. `sim::X`, `env::X` and `protocol::X` name types of the respective crates.

## R1. Crate layout and compatibility

- **R1.1** The server MUST live in `crates/server` as a library (`rl-semaphore-server`). It SHOULD be split into modules: `lib.rs` (crate doc, re-exports), `config.rs` (R4.1–R4.3), `error.rs` (R4.8), `sim_loop.rs` (R5), `pace.rs` (R5.2), `ws.rs` (R6), `health.rs` (R7) and `app.rs` (router, bind, serve). Module names MAY differ if the public API of R4 is kept. `#![warn(clippy::pedantic)]` MUST be set in `lib.rs`.
- **R1.2** `rl-semaphore-server` MUST be added to `[workspace.dependencies]` with `path` and `version`. New third-party dependencies MUST be declared in `[workspace.dependencies]` and used via `workspace = true`:
  - `axum` (current 0.8 release) with the `ws` feature;
  - `tokio` with only the features the code needs (expected: `rt-multi-thread`, `net`, `sync`, `time`, `macros`, `signal`);
  - `tower-http` with the `trace` feature only.

  The server's normal dependencies MUST also include `rl-semaphore-sim`, `rl-semaphore-env`, `rl-semaphore-protocol` with `features = ["sim"]`, `serde`, `serde_json`, `thiserror` and `tracing`, all through the workspace.
- **R1.3** Test-only dependencies (a WebSocket client such as `tokio-tungstenite`, `futures-util`, and `tower` with `util` for `oneshot`) MUST be dev-dependencies, declared in `[workspace.dependencies]`. The WebSocket client SHOULD use the same `tungstenite` version as `axum`, so that the lockfile does not gain a duplicate.
- **R1.4** `cli` MUST gain normal dependencies on `rl-semaphore-server` and on `tokio` (runtime and signal features only). No other crate gains a dependency on `server`.
- **R1.5** No source file under `crates/sim/src` may change. Every existing `insta` snapshot under `crates/sim` and `crates/cli` MUST stay byte-identical. Every existing test MUST keep passing, and no existing assertion may be weakened or removed, except the golden `protocol` snapshots that R3.5 updates and the CLI parse test that R8.7 updates.
- **R1.6** Code outside tests MUST NOT call `unwrap`, `expect`, `panic!`, or index a slice in a way that can panic. Every public item MUST have a doc comment, and every fallible public function MUST have an `# Errors` section.
- **R1.7** `cargo deny --locked check` and `cargo audit` MUST pass. Every new crate's license MUST already be in the allow list of `deny.toml`. If one is not, the PR MUST say why the list changes.
- **R1.8** With default features, `protocol` MUST still build for `wasm32-unknown-unknown`, and `cargo tree -p rl-semaphore-protocol -e normal` MUST NOT list `rl-semaphore-sim`.

## R2. Episode stepper in env

- **R2.1** `env` MUST provide a public `Episode` type (in a new module `episode.rs`, re-exported from the crate root) that owns one episode's `sim::Simulation`, `sim::Demand`, `sim::EpisodeMetrics` and `SignalCounts`, and does not own a controller.
- **R2.2** `Episode` MUST provide at least:
  - `Episode::new(scenario: &Scenario, seed: u64) -> Episode`: a fresh simulation at step 0 and a demand seeded with `seed`;
  - `step(&mut self, controller: &mut impl Controller) -> Result<(), EnvError>`: exactly one iteration of the current `run_episode` loop, in the same order (draw and spawn the arrivals, ask the controller for a command, step the simulation, count the signal outcome, observe the metrics);
  - `sim(&self) -> &Simulation`, `seed(&self) -> u64` and `signal_counts(&self) -> SignalCounts`;
  - `summary(&self) -> EpisodeSummary`, the metrics collector's summary at the current step;
  - `report(&self) -> EpisodeReport`, the same value `run_episode` returns after the same steps.
- **R2.3** `run_episode(scenario, controller, seed, steps)` MUST keep its signature, its docs and its results. It MUST be implemented as `Episode::new`, then `steps` calls to `step`, then `report`. There MUST be exactly one copy of the step loop in the workspace.
- **R2.4** `Episode` MUST NOT log, read the clock or do I/O, like `run_episode` today.

## R3. Protocol additions

- **R3.1** `protocol::Snapshot` and `protocol::Metrics` MUST each gain two fields, declared after `step`, with `#[serde(default)]`:
  - `episode: u64`, the index of the episode in the server's run, starting at 0;
  - `seed: u64`, the demand seed of that episode.

  No other DTO changes.
- **R3.2** `PROTOCOL_VERSION` MUST stay 1: the change is compatible under 2.1's R2.7. A message without these fields MUST decode, with both fields 0.
- **R3.3** `Snapshot::from_sim` and `Metrics::from_sim` MUST keep their signatures and set both fields to 0. Their docs MUST say that the caller sets them.
- **R3.4** The crate doc of `protocol` MUST describe the message order as enforced by the server, with episodes:
  - exactly one `Hello` first on every connection;
  - the keys of the `Snapshot`s strictly increase;
  - every `Metrics` has a key no greater than the key of the latest `Snapshot` sent before it;
  - a new episode shows as a larger `episode` with `step` back at 0.

  It MUST no longer say that 2.2 will enforce the order.
- **R3.5** The golden snapshots under `crates/protocol/tests/snapshots/` whose messages gain the fields MUST be updated with `cargo insta review` and checked by eye: only the two new fields may differ. The fixture builders and the proptest strategies MUST cover the new fields.

## R4. Server configuration and lifecycle

- **R4.1** The crate MUST provide `ServerConfig` with public fields, or a builder, holding:
  - `scenario: sim::Scenario`, which must have a fixed-time plan;
  - `seed: u64`, the seed of episode 0;
  - `steps_per_episode: NonZeroU64`;
  - `metrics_every: NonZeroU64`, in steps;
  - `pace: Pace`;
  - `bind: SocketAddr`;
  - `broadcast_capacity: NonZeroUsize`, in ticks, with a documented default of 64.
- **R4.2** `Pace` MUST be an enum with the variants `RealTime { speed: f64 }` and `Unthrottled`. A constructor or validation step MUST reject a `speed` that is not finite or not `> 0`.
- **R4.3** Building the server MUST fail with a `ServerError` when the scenario has no fixed-time plan, or when `speed` is invalid. This check MUST happen before anything binds a socket or starts a thread.
- **R4.4** The crate MUST let a caller bind first and serve later, so tests can bind port 0 and learn the port. For example, `Server::bind(config).await -> Result<Server, ServerError>`, `Server::local_addr(&self) -> SocketAddr` and `Server::serve(self, shutdown: impl Future<Output = ()> + Send + 'static).await -> Result<(), ServerError>`. A convenience `run(config, shutdown)` MAY combine the two.
- **R4.5** Before the socket accepts connections, the latest state MUST already hold the step-0 tick of episode 0, so a handler never sees an empty state (R6.2, R7.1).
- **R4.6** When `shutdown` completes:
  - the server MUST stop accepting connections;
  - every open WebSocket MUST be sent a close frame with code 1001 (going away), and its task MUST end;
  - the sim thread MUST stop within one step period plus a small constant (it MUST NOT sleep through a whole period without seeing the stop request);
  - `serve` MUST join the sim thread and return `Ok(())`.
- **R4.7** If the sim thread fails (an `EnvError` or an encode error), it MUST log an `error` event, and `serve` MUST shut down as in R4.6 and return that error. A panic of the sim thread MUST be caught when joining and returned as an error, not propagated.
- **R4.8** `ServerError` MUST be a `thiserror` enum. Its variants MUST cover at least: a missing fixed-time plan, an invalid speed, a bind failure (with the address), a serve I/O failure, a sim failure (`EnvError` as its source), an encode failure (`ProtocolError` as its source), and a panicked sim thread. Messages MUST be lowercase without a trailing period, as in `CONTRIBUTING.md`.

## R5. Simulation loop

- **R5.1** The simulation MUST run on one dedicated OS thread named `sim` (`std::thread::Builder`), not on a Tokio worker. The thread MUST drive an `env::Episode` with an `env::FixedTime` controller built from the scenario's plan.
- **R5.2** Pacing:
  - with `Pace::RealTime { speed }`, the period is `step_s / speed` seconds, and tick `k` of the run is published no earlier than `start + k × period`, measured with `std::time::Instant` from an absolute reference;
  - when the loop is more than one period late, it MUST reset its reference to now instead of publishing a burst of late ticks, and it SHOULD log a `debug` event;
  - with `Pace::Unthrottled`, the loop MUST NOT sleep;
  - the deadline logic MUST be a pure function or type that takes the time as input, so it can be unit-tested without a clock (R9.4).
- **R5.3** Episodes:
  - episode 0 uses `seed`, and episode `e` uses `seed.wrapping_add(e)`;
  - an episode publishes its step-0 tick (a fresh simulation), then one tick after each of its `steps_per_episode` steps;
  - after the tick of step `steps_per_episode`, the next tick is the step-0 tick of episode `e + 1`;
  - every tick, including a step-0 tick, takes one period;
  - the loop MUST log an `info` event when an episode starts (with `episode` and `seed`) and when it ends (with `episode`, the mean and p99 wait if any, and `departed`).
- **R5.4** A tick MUST include `Metrics` at step 0, at every step that is a multiple of `metrics_every`, and at step `steps_per_episode`, and at no other step. A `Metrics` MUST be built with `Metrics::from_sim(&episode.summary(), episode.sim())`, and both messages MUST carry the episode's `episode` and `seed`.
- **R5.5** Each message MUST be encoded to JSON once per tick, on the sim thread, with `ServerMessage::to_json`. Connections MUST forward the encoded text without decoding or re-encoding it, and without copying the payload per client (for example, `axum::extract::ws::Utf8Bytes` or `Arc<str>`).
- **R5.6** For each tick, the sim thread MUST first store it as the latest state (keeping the latest `Metrics` of the episode too, when the tick has none), and then send it on the broadcast channel. A send with no receivers is not an error.
- **R5.7** The `Hello` MUST be encoded once, at startup, from `Hello::new(Layout::from_scenario(&scenario))`.

## R6. WebSocket endpoint

- **R6.1** `GET /ws` MUST upgrade to a WebSocket. A request without the upgrade headers MUST get a 4xx response and no stream.
- **R6.2** On a new connection, the handler MUST, in this order:
  1. subscribe to the broadcast channel;
  2. read the latest state, with key `L`;
  3. send `Hello`;
  4. send the latest `Snapshot`, then the latest `Metrics`;
  5. forward every received tick whose key is greater than `L`, the `Snapshot` first, then its `Metrics` if any, and drop every tick with a key of `L` or less.

  Subscribing before reading the latest state means that no tick published after `L` is missed, except under R6.3.
- **R6.3** When the receiver reports lag (`RecvError::Lagged(n)`), the handler MUST log a `debug` event with `n`, keep the connection, and go on with the next tick received. The key filter of R6.2 still applies.
- **R6.4** Text and binary frames from the client MUST be ignored (a `debug` event MAY be logged). Ping frames MUST be answered as the WebSocket library does by default. A close frame from the client, a read error or a write error MUST end the connection's task without affecting other connections.
- **R6.5** The sim thread MUST NOT wait on any connection, and a connection MUST NOT wait on another.
- **R6.6** On shutdown (R4.6) the handler MUST send a close frame with code 1001 and end.
- **R6.7** The server MUST keep a count of open WebSocket connections. The count MUST be decremented on every exit path (a drop guard, not a manual decrement), and it is what `/healthz` reports. The handler MUST log `info` events on connect and disconnect with the count.

## R7. Health endpoint and HTTP

- **R7.1** `GET /healthz` MUST return 200 with `Content-Type: application/json` and a body with the fields:
  - `status`: the string `"ok"`;
  - `version`: the server crate's `CARGO_PKG_VERSION`;
  - `protocol_version`: `PROTOCOL_VERSION`;
  - `episode` and `step`: the key of the latest tick;
  - `clients`: the count of R6.7.

  The body type MUST be a `serde::Serialize` struct in `server` (not in `protocol`).
- **R7.2** Any other path MUST get 404. Only `GET` is routed. Other methods on `/healthz` and `/ws` MUST get 405.
- **R7.3** HTTP requests MUST be traced with `tower_http::trace::TraceLayer`. The server MUST log an `info` event `listening` with the bound address once it accepts connections.

## R8. CLI `serve` command

- **R8.1** `rl-semaphore serve` MUST accept:
  - `--config <PATH>`, required, env `RL_SEMAPHORE_CONFIG`;
  - `--seed <u64>`, default 0, env `RL_SEMAPHORE_SEED`;
  - `--steps <u64>`, the steps per episode, at least 1, default 3600, env `RL_SEMAPHORE_STEPS`;
  - `--speed <f64>`, finite, `> 0` and `<= 1000`, default 1, env `RL_SEMAPHORE_SPEED`;
  - `--metrics-every <u64>`, at least 1, default 5, env `RL_SEMAPHORE_METRICS_EVERY`;
  - `--bind <ADDR>`, a `SocketAddr`, default `127.0.0.1:3000`, env `RL_SEMAPHORE_BIND`.

  Invalid values MUST be rejected by clap, before the config file is read.
- **R8.2** `serve` MUST read and validate the config the same way `simulate` does, with the same error context, and MUST fail with a clear error when the scenario has no `[fixed_time]` table.
- **R8.3** `serve` MUST build a multi-threaded Tokio runtime itself (`tokio::runtime::Builder`, not `#[tokio::main]`), after logging is initialized, and block on the server.
- **R8.4** The shutdown future MUST complete on Ctrl-C, and on Unix also on SIGTERM. A clean shutdown MUST exit with code 0. A startup or runtime error MUST be logged and exit with a failure code, like other commands.
- **R8.5** `serve` MUST log an `info` event on start with the config path, seed, steps, speed, metrics interval and bind address.
- **R8.6** `serve` MUST be removed from the "not implemented yet" arm of `main.rs`. `train` and `eval` stay there.
- **R8.7** The existing `parses_each_subcommand` test MUST pass `--config x.toml` to `serve`. No other existing CLI assertion may change.

## R9. Tests and CI

- **R9.1** **Stepper.** `env` tests MUST check that a manual loop of `Episode::step` gives the same `report()` as `run_episode` for the example over at least 600 steps and two seeds, and that `Episode::summary` at intermediate steps equals the summary of a loop written by hand in the test over `Simulation`, `Demand` and `EpisodeMetrics`. The existing `cli` snapshots MUST NOT change, and the existing `env` tests MUST pass unchanged (R1.5).
- **R9.2** **Protocol.** Tests MUST check that a `Snapshot` and a `Metrics` JSON without `episode` and `seed` decode with both fields 0, and that the round-trip and property tests cover non-zero values (R3.2, R3.5).
- **R9.3** **Config.** Unit tests MUST check that a scenario without a fixed-time plan, and speeds of 0, negative, NaN and infinity, are rejected before binding (R4.3).
- **R9.4** **Pacing.** Unit tests MUST check the pure deadline logic of R5.2 without sleeping: deadlines at `start + k × period`, no drift over many ticks, the reference reset when more than one period late, and no waiting under `Unthrottled`.
- **R9.5** **WebSocket.** Integration tests in `crates/server/tests/` MUST start an in-process server on `127.0.0.1:0` with the example, `Pace::Unthrottled` unless a test needs otherwise, and a small `steps_per_episode` (for example 50). They MUST connect with a real WebSocket client and check that:
  - the first message is a `Hello` whose `check_version` passes and whose layout equals `Layout::from_scenario` of the example;
  - the second message is a `Snapshot`, and the third a `Metrics`;
  - over at least three episodes, `Snapshot` keys strictly increase, every `Metrics` key is at most the latest `Snapshot` key, `step <= steps_per_episode`, every episode `e` has `seed == base.wrapping_add(e)`, and a new episode starts at step 0;
  - every received `Snapshot`, with `episode` and `seed` set to 0, equals `Snapshot::from_sim` of an independent `env::Episode` with the same seed and a `FixedTime` controller, stepped to the same step;
  - every received `Metrics` equals the replay's `Metrics::from_sim` at the same step, compared the same way;
  - two clients connected at the same time receive byte-identical text for every tick they both receive;
  - a client that connects after the run has advanced gets a first `Snapshot` with a key greater than `(0, 0)`;
  - a client that stops reading while the sim runs with `broadcast_capacity` at 4, then reads again, still has an open connection, and the ordering checks above still hold;
  - a client that sends text and binary frames keeps receiving ticks;
  - on shutdown, each client receives a close frame with code 1001, and `serve` returns `Ok(())` within a bounded time.
- **R9.6** **HTTP.** Tests MUST check that `/healthz` returns 200 JSON with the fields of R7.1 (`clients` reflecting the open connections, `episode` and `step` moving forward), that a plain `GET /ws` gets a 4xx, that an unknown path gets 404, and that `POST /healthz` gets 405.
- **R9.7** **No hangs.** Every await on the network in a test MUST be wrapped in a timeout (for example 10 s), so a bug fails the test instead of hanging CI.
- **R9.8** **CLI.** Tests MUST check the parsing of every `serve` flag, the defaults, at least one `RL_SEMAPHORE_*` override, and the rejection of `--speed 0`, `--speed -1`, `--steps 0` and an invalid `--bind`. An end-to-end test SHOULD run the built binary with `--bind 127.0.0.1:0 --log-format json`, read the bound address from the `listening` event on stderr, get `/healthz`, and then kill the process.
- **R9.9** Test files MAY allow `clippy::unwrap_used` and `clippy::expect_used` at the file level, as existing test files do.
- **R9.10** **CI.** Because `server` enables `protocol/sim`, workspace-wide runs now build `protocol` with `sim`. To keep checking the default-feature build on its own, `.github/workflows/ci.yml` MUST gain:
  - in the `clippy` job, `cargo clippy -p rl-semaphore-protocol --all-targets --locked -- -D warnings`;
  - in the `test` job, `cargo nextest run -p rl-semaphore-protocol --locked`.

  The existing steps stay. No new job or third-party action may be added. The README's list of CI commands MUST include the two new commands.

## R10. Docs, ADR and roadmap

- **R10.1** `specs/adr/0009-live-simulation-streaming.md` MUST record, in the template's format:
  - one shared simulation on a dedicated thread, broadcast to all clients, versus one simulation per connection;
  - the tick model (snapshot and metrics encoded once and sent together);
  - catch-up on connect and lag skipping;
  - wall-clock pacing with a speed factor;
  - episode looping with `seed + e`, and the `episode` and `seed` fields;
  - the `env::Episode` stepper.

  Its status is `Accepted`, and it MUST be added to the index in `specs/adr/README.md`.
- **R10.2** The `server` crate doc (`//!` in `lib.rs`) MUST describe the endpoints, the sim thread and its pacing, the message order and catch-up, lag behaviour, shutdown, and link to ADR-0008 and ADR-0009.
- **R10.3** The `server` row of the crate table in `specs/tech-stack.md` MUST list `axum`, `tokio`, `tower-http`, and the `protocol` (`sim` feature) and `env` dependencies. The `env` row MUST mention the `Episode` stepper.
- **R10.4** The README "Getting started" MUST show how to run `serve` and check it (`curl http://127.0.0.1:3000/healthz`, and a WebSocket client such as `websocat ws://127.0.0.1:3000/ws`). "Project status" MUST say that 2.2 is done and that the next phase is 2.3.
- **R10.5** Roadmap entry 2.2 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.
