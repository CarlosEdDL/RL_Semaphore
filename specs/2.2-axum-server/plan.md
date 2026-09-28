# 2.2 Axum server: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feature/stage-2` (the current branch), or a per-phase branch if preferred.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Episode stepper in env
- [ ] `crates/env/src/episode.rs`: `Episode` with `new`, `step`, `sim`, `seed`, `signal_counts`, `summary` and `report` ([R2](requirements.md#r2-episode-stepper-in-env)). Move the body of the `run_episode` loop into `step` unchanged: same order, same counters.
- [ ] Rewrite `run_episode` as `Episode::new` → `steps` × `step` → `report` (R2.3). Keep its doc and doctest as they are.
- [ ] Re-export `Episode` from `lib.rs`, and update the crate doc (the runner is now "the episode stepper and `run_episode`").
- [ ] Tests for R9.1 in `crates/env/tests/`. Then check that nothing moved: `cargo nextest run -p rl-semaphore-env -p rl-semaphore`, `git diff --exit-code crates/cli/tests/snapshots`, and a local bench against the saved baseline (the stepper adds no work per step, so the steps/sec should not move).

### 2. Protocol additions
- [ ] `episode: u64` and `seed: u64` with `#[serde(default)]` on `Snapshot` and `Metrics`, after `step`, with doc comments ([R3.1](requirements.md#r3-protocol-additions)).
- [ ] `from_sim` sets both to 0, and its docs say the caller fills them (R3.3).
- [ ] Update the fixture builders (`tests/common`) with non-zero values, and add both fields to the proptest strategies (R3.5).
- [ ] Decoding test: JSON without the fields decodes with 0 (R3.2, [R9.2](requirements.md#r9-tests-and-ci)).
- [ ] `cargo insta review`: accept only diffs that add the two fields (R3.5).
- [ ] The crate doc's message order, with episodes and "enforced by the server" (R3.4).

### 3. Manifests and dependencies
- [ ] Workspace `[workspace.dependencies]`: `rl-semaphore-server`, `axum` (`ws`), `tokio` (see [R1.2](requirements.md#r1-crate-layout-and-compatibility)), `tower-http` (`trace`), and for tests `tokio-tungstenite`, `futures-util`, and `tower` (`util`).
- [ ] Choose the `tokio-tungstenite` version that `axum` itself uses (`cargo tree -p rl-semaphore-server -i tungstenite` after adding `axum`), so the lockfile keeps one `tungstenite` (R1.3).
- [ ] `crates/server/Cargo.toml`: the normal and dev dependencies of R1.2–R1.3. `crates/cli/Cargo.toml`: `rl-semaphore-server` and `tokio` (R1.4).
- [ ] Early check: `cargo deny --locked check` (licenses of the new crates), `cargo audit`, `cargo tree -p rl-semaphore-protocol -e normal` (no `sim`), and the WASM build of `protocol` (R1.7, R1.8).

### 4. Config, errors and pacing
- [ ] `config.rs`: `ServerConfig`, `Pace`, the defaults (`broadcast_capacity` 64), and validation (R4.1–R4.3).
- [ ] `error.rs`: `ServerError` (R4.8).
- [ ] `pace.rs`: a pure `Pacer` that takes an `Instant` and returns "publish now" or "wait until *t*", and resets its reference when more than one period late (R5.2). Unit tests for R9.4, feeding it synthetic instants.
- [ ] Unit tests for R9.3.

### 5. Sim thread
- [ ] `sim_loop.rs`: the `Tick` type (`episode`, `step`, `snapshot` text, optional `metrics` text), the shared `Latest` state (the latest tick plus the latest metrics of the episode), and the loop of [R5](requirements.md#r5-simulation-loop). Build and publish the step-0 tick of episode 0 on the calling thread before spawning, so R4.5 holds.
- [ ] Wait for the next deadline in a way that a stop request interrupts: for example `std::sync::mpsc::Receiver::recv_timeout` on a stop channel, or a `Condvar` with `wait_timeout` (R4.6). Check the stop request between ticks under `Unthrottled` too.
- [ ] Store the latest state, then broadcast (R5.6). A `tokio::sync::broadcast::Sender` and a `std::sync::RwLock` (or `tokio::sync::watch`) are both usable from a plain thread, with no runtime handle needed.
- [ ] Encode once per message with `ServerMessage::to_json`, and wrap the text in the type the socket sends (`Utf8Bytes::from(String)`), so a clone per client is a reference-count bump (R5.5).
- [ ] The episode start and end `info` events (R5.3).

### 6. Router, handlers and serve
- [ ] `app.rs`: the shared `AppState` (latest state, broadcast sender, encoded `Hello`, client counter, shutdown signal for handlers), the router (`/healthz`, `/ws`, `TraceLayer`), `Server::bind`, `local_addr` and `serve` with `axum::serve(...).with_graceful_shutdown(...)` ([R4.4–R4.7](requirements.md#r4-server-configuration-and-lifecycle), R7.2–R7.3).
- [ ] `ws.rs`: the handler of [R6](requirements.md#r6-websocket-endpoint), a `tokio::select!` over the broadcast receiver, the socket's `recv()` and the shutdown signal. The client counter guard (R6.7).
- [ ] `health.rs`: the `Health` body and handler (R7.1).
- [ ] Sim failure path: the sim thread reports its error on a channel, which triggers the handlers' shutdown and makes `serve` return the error (R4.7).
- [ ] `lib.rs`: crate doc (R10.2), re-exports.

### 7. Server tests
- [ ] `crates/server/tests/common/mod.rs`: start a server on port 0 with a oneshot or `Notify` for shutdown, a WS client helper that decodes `ServerMessage`s with `from_json`, a timeout wrapper (R9.7), and the replay helper (an `env::Episode` for a given seed, stepped to a given step).
- [ ] `tests/ws.rs`: every bullet of [R9.5](requirements.md#r9-tests-and-ci). For the replay comparison, set `episode` and `seed` to 0 on the received message before comparing, and step one replay per episode forward only (the received steps increase).
- [ ] `tests/http.rs`: R9.6. Use `tower::ServiceExt::oneshot` on the router for the plain HTTP cases, and the real socket for `clients`.

### 8. CLI `serve`
- [ ] `crates/cli/src/serve.rs`: `ServeArgs` ([R8.1](requirements.md#r8-cli-serve-command)), a value parser for `--speed`, the config loading shared with `simulate` (factor out a small helper if both need it, without changing `simulate`'s messages), the runtime (R8.3), and the shutdown future on Ctrl-C and SIGTERM (R8.4).
- [ ] `main.rs`: `Command::Serve(ServeArgs)`, dispatch, and the change to the parse test (R8.6, R8.7).
- [ ] Tests for R9.8, including the end-to-end binary test in `crates/cli/tests/serve.rs` (`assert_cmd`-free: `std::process::Command` with `env!("CARGO_BIN_EXE_rl-semaphore")`, as the existing tests do).

### 9. CI
- [ ] `.github/workflows/ci.yml`: the two steps of [R9.10](requirements.md#r9-tests-and-ci).
- [ ] README "Getting started": add the two commands to the CI command list.

### 10. ADR and docs
- [ ] `specs/adr/0009-live-simulation-streaming.md` from the template, and a row in the ADR index ([R10.1](requirements.md#r10-docs-adr-and-roadmap)).
- [ ] `specs/tech-stack.md`: the `server` and `env` rows (R10.3).
- [ ] README: `serve` usage and "Project status" (R10.4).

### 11. Roadmap
- [ ] Mark 2.2 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R10.5).

### 12. Verify locally

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p rl-semaphore-protocol --all-targets --locked -- -D warnings
cargo clippy -p rl-semaphore-protocol --all-targets --features sim --locked -- -D warnings
cargo nextest run --workspace --locked
cargo nextest run -p rl-semaphore-protocol --locked
cargo nextest run -p rl-semaphore-protocol --features sim --locked
cargo test --workspace --doc --locked
cargo build -p rl-semaphore-web --target wasm32-unknown-unknown --locked
cargo build -p rl-semaphore-protocol --target wasm32-unknown-unknown --locked
cargo deny --locked check
cargo audit
cargo bench -p rl-semaphore-env --bench throughput --locked -- --test

# default features of protocol still pull in no simulator
cargo tree -p rl-semaphore-protocol -e normal | grep -c rl-semaphore-sim   # 0

# sim untouched, existing snapshots untouched
git diff --stat -- crates/sim/src   # empty
git diff --exit-code crates/sim/tests/snapshots/ crates/cli/tests/snapshots/

# by hand
cargo run --release -p rl-semaphore -- serve --config configs/single-intersection.toml --speed 10
curl -s http://127.0.0.1:3000/healthz
websocat ws://127.0.0.1:3000/ws | head -3
```

## Notes and risks

- **Feature unification hides the default build of `protocol`.** Once `server` depends on `protocol` with `sim`, `cargo nextest run --workspace` and `cargo clippy --workspace` build `protocol` with `sim`. The `-p rl-semaphore-protocol` steps of R9.10 are then the only check of the default build. The WASM build and `cargo tree -p` already resolve only that package's features, so they are unaffected.
- **Graceful shutdown and upgraded connections.** `axum::serve(...).with_graceful_shutdown` stops accepting and waits for open connections. Whether it waits for, or even knows about, connections upgraded to WebSocket depends on the hyper version. Do not count on it: each WS task must watch the shutdown signal itself and close with 1001 (R6.6). The shutdown test is what proves it. If `serve` returns while WS tasks are still flushing their close frames, keep a `TaskTracker`-like count (a `tokio::sync` counter or a `JoinSet` in state) and wait for it with a timeout. Avoid `tokio-util` unless it is clearly simpler.
- **The catch-up race.** Subscribe, then read the latest state, then filter ticks with key ≤ `L` (R6.2). The sim thread stores the latest state before it broadcasts (R5.6). With both orders fixed, a tick can arrive twice (once in the latest state and once in the broadcast), and the filter drops the second copy, but a tick can never be lost. A test with an unthrottled sim and many sequential connects exercises it.
- **Lag with bundled ticks.** Because a tick is one broadcast item, lag can never deliver a `Metrics` without its `Snapshot`. Do not split them into two broadcast items later without revisiting R3.4.
- **Unthrottled tests and CPU.** An unthrottled sim loops as fast as it can until shutdown. At the benchmarked speed of the sim this is many thousands of ticks per second. Keep test servers short-lived, and shut each one down at the end of its test. With `broadcast_capacity` 64 and fast ticks, fast clients also lag often. The ordering checks must hold anyway, which is a useful stress test, but a test that asserts "no lag" must use a real-time pace.
- **The replay comparison and lag.** A lagging client skips steps, so the replay must be able to jump forward: keep one `env::Episode` per episode index, and call `step` until it reaches the received step. Never compare against a replay that is ahead.
- **Pacing at high speed.** At `--speed 1000` with `step_s = 1` the period is 1 ms. `std::thread::sleep` resolution on Linux is fine for that, and the reset rule of R5.2 prevents bursts when it falls behind. JSON encoding of a ~4 kB snapshot takes microseconds, well inside the budget (see the frame sizes measured in 2.1's plan).
- **Frame rate.** At 1× the example sends one ~3–4 kB snapshot per second, plus a metrics message every 5 s. At 10× that is about 40 kB/s per client, which is fine on localhost and on any real link.
- **Memory over a long run.** `EpisodeMetrics` grows with the vehicles of one episode and is dropped when the episode ends. Episode looping keeps memory bounded, which is one reason for it.
- **SIGTERM on non-Unix.** `tokio::signal::unix` only exists on Unix. Gate it with `#[cfg(unix)]` and fall back to Ctrl-C alone elsewhere. CI is Linux only.
- **Scope creep.** No static file serving, no CORS, no client messages, no REST beyond `/healthz`, no Dockerfile. Those belong to 2.3, 5.3 and 2.6. The server runs exactly one fixed-time simulation.
