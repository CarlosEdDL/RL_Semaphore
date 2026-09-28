# 2.2 Axum server

**Stage:** 2 (Visualization of the simulator) · **Status:** ☑ done · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R10) · [plan.md](plan.md) (tasks).

## Goal

Put the simulator on the network. At the end of this phase `rl-semaphore serve --config configs/single-intersection.toml` starts an Axum server with two endpoints. `GET /healthz` answers with a small JSON status. `GET /ws` is a WebSocket that streams the 2.1 protocol from one shared, live fixed-time simulation. The simulation runs on its own thread, paced to wall-clock time times a `--speed` factor. It loops episode after episode, each with the next seed, so the demo never stops. Every connection gets a `Hello`, then the current state, then the live stream of `Snapshot` and `Metrics` messages. A slow client skips frames it cannot keep up with, and the simulation never waits for it. The step loop of `run_episode` moves into a reusable `Episode` stepper in `env`, so the CLI and the server share one loop. The protocol gains two compatible fields, `episode` and `seed`, so a client can see when an episode restarts. Integration tests connect real WebSocket clients to an in-process server and check the handshake, the message order, the catch-up of a late joiner, the rollover to a new episode, lag, and shutdown. They also check that every streamed snapshot matches an independent replay of the same seed.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-crate-layout-and-compatibility) | Crates, dependencies, what may and may not change |
| [R2](requirements.md#r2-episode-stepper-in-env) | `env::Episode`, a step-by-step episode driver, and `run_episode` on top of it |
| [R3](requirements.md#r3-protocol-additions) | `episode` and `seed` on `Snapshot` and `Metrics`, the message order with episodes |
| [R4](requirements.md#r4-server-configuration-and-lifecycle) | `ServerConfig`, `Pace`, bind, serve, graceful shutdown, errors |
| [R5](requirements.md#r5-simulation-loop) | The sim thread: pacing, episode looping, encoding each tick once, publishing |
| [R6](requirements.md#r6-websocket-endpoint) | `/ws`: handshake, catch-up, live stream, lag skipping, client frames, close |
| [R7](requirements.md#r7-health-endpoint-and-http) | `/healthz`, 404s, request tracing |
| [R8](requirements.md#r8-cli-serve-command) | `rl-semaphore serve` flags, env overrides, runtime, signals |
| [R9](requirements.md#r9-tests-and-ci) | Stepper, protocol, pacing, WebSocket, health and CLI tests, and CI steps |
| [R10](requirements.md#r10-docs-adr-and-roadmap) | ADR-0009, crate docs, `tech-stack.md`, README, roadmap |

## Decisions taken

- **One shared simulation, broadcast to every client.** One fixed-time simulation runs on a dedicated OS thread, away from the Tokio runtime, as `tech-stack.md` requires for long-running work. It publishes each step to every connection through a `tokio::sync::broadcast` channel. Every viewer sees the same world at the same moment, CPU cost does not grow with the number of viewers, and the design carries over to 8.3, where viewers watch one training run. A client that joins late gets `Hello`, then the latest `Snapshot` and `Metrics`, then the live stream. It never waits for the next step to see something.
- **Wall-clock pacing with a speed factor.** One step takes `step_s / speed` seconds of wall-clock time (default `--speed 1`, so the example's 1 s step is real time). Deadlines are absolute, so the pace does not drift. When the loop falls more than one step behind, it resets its reference instead of rushing through a burst of steps. A `Snapshot` is sent every step, and a `Metrics` every `--metrics-every` steps (default 5), at the first step of each episode, and at its last step. The library also has an unthrottled pace, which the tests use so that they run in milliseconds.
- **Episodes loop with the next seed.** After `--steps` steps (default 3600, one simulated hour on the example), the loop starts a fresh episode. Episode `e` uses seed `seed + e` (wrapping), so the whole stream stays reproducible from the base seed. Without looping, the demo would either freeze or run one endless episode, whose `EpisodeMetrics` would keep every wait sample in memory and whose statistics would stop responding to change.
- **`episode` and `seed` on every frame.** `Snapshot` and `Metrics` gain `episode: u64` and `seed: u64`, both `#[serde(default)]`. This is compatible under the R2.7 rule of 2.1, so `PROTOCOL_VERSION` stays 1. Every frame describes itself, so a client that joins late, or that skipped frames under lag, still knows which episode it is looking at, and `(episode, step)` orders all frames. A separate "episode started" message was rejected because a client that drops that one message on lag could not tell that the step counter went back to 0.
- **Snapshot and metrics travel together, encoded once.** The sim thread encodes the step's `Snapshot` and, when due, its `Metrics` into JSON text once, and broadcasts them as one *tick*. Connections forward the ready bytes, so CPU cost per client is a copy, not a serialization. Lag drops a tick as a whole, so a `Metrics` can never arrive ahead of the `Snapshot` of its step. That keeps the order that 2.1 defined and 2.2 enforces.
- **Slow clients skip ahead.** When a connection falls behind the broadcast buffer, it drops the ticks it missed, logs a `debug` event with the count, and goes on from the oldest tick still buffered. `Metrics` are cumulative, so a skipped one loses nothing. The simulation never blocks on a client, and no client can slow down the others.
- **One step loop, in `env`.** `run_episode` runs a whole episode in one call, but the server has to step one step at a time and look at the state in between. A public `env::Episode` now holds the simulation, the demand, the metrics collector and the signal counts, and its `step` does one iteration of the existing loop. `run_episode` is rewritten on top of it. The CLI, the server and the evaluations of Stage 3 then share one loop, and the existing `simulate` snapshots and `env` tests show that nothing changed.
- **`rl-semaphore serve` in the CLI.** The existing `serve` subcommand builds a Tokio runtime and calls the `server` library, as `tech-stack.md` places it. The flags are `--config`, `--seed`, `--steps`, `--speed`, `--metrics-every` and `--bind` (default `127.0.0.1:3000`), and each has an `RL_SEMAPHORE_*` environment override for Docker (2.6). Ctrl-C and SIGTERM shut the server down gracefully: every WebSocket gets a close frame with code 1001 (going away), the sim thread stops, and the process exits 0. Logging stays in the CLI, and the server library only emits `tracing` events.
- **A JSON health endpoint.** `GET /healthz` returns 200 with `status`, `version`, `protocol_version`, `episode`, `step` and `clients`. It serves as the Docker healthcheck in 2.6 and as a quick way to debug a running server. The body is a server-local type, not a `protocol` DTO. It is not part of the WebSocket protocol and has its own, smaller stability promise.
- **An ADR.** Sharing one simulation, the tick model, lag skipping, pacing and episode looping all shape 2.3–2.5 and Stage 8, and are costly to change once a client depends on them. ADR-0009 records them.

## Out of scope

- The Leptos app, serving static files, and the `cargo-leptos` build (2.3). No CORS layer is needed yet, because 2.3 serves the app from the same origin.
- Any client-to-server message: pause, resume, changing speed or seed, choosing a scenario. The server ignores text and binary frames from clients.
- More than one simulation at once, other controllers, or training (5.5, Stage 8).
- REST endpoints other than `/healthz` (5.3).
- Connection limits, rate limiting, authentication and a read-only public mode (11.2).
- Compression, a binary encoding, or delta frames (11.1).
- The Dockerfile and a container healthcheck (2.6).
- Any change to `sim`.

## Acceptance criteria

1. `rl-semaphore serve --config configs/single-intersection.toml` starts a server on `127.0.0.1:3000` that answers `GET /healthz` with a 200 JSON status, and streams the example's fixed-time simulation on `GET /ws` in real time. `--speed`, `--seed`, `--steps`, `--metrics-every` and `--bind` work, as do their `RL_SEMAPHORE_*` overrides. (R4, R7, R8)
2. Every WebSocket connection receives exactly one `Hello` first, whose layout equals `Layout::from_scenario` of the scenario. Then it receives the current `Snapshot` and `Metrics`, then live ticks. `Snapshot` keys `(episode, step)` strictly increase, and every `Metrics` key is at most the key of the latest `Snapshot`. A client that joins late gets the current state right away. (R5, R6, R9)
3. After `--steps` steps the stream continues with `episode + 1`, `seed + episode`, and `step` back at 0. Every streamed `Snapshot` equals, apart from its episode fields, `Snapshot::from_sim` of an independent `env::Episode` replay with the same seed at the same step. (R3, R5, R9)
4. A client that stops reading keeps its connection, skips the ticks it missed, and then keeps the ordering guarantees. A client that sends text or binary frames is ignored, not disconnected. On shutdown every client receives a close frame with code 1001, and `serve` returns within a bounded time. (R4, R6, R9)
5. `run_episode` is built on `env::Episode` and returns the same reports as before: the `simulate` snapshots and every existing `env` and `cli` test pass unchanged. `protocol` stays at `PROTOCOL_VERSION = 1`, and a message without the new fields still decodes. (R1, R2, R3, R9)
6. CI covers the server's tests and still checks `protocol` with default features on its own. ADR-0009, the crate docs, `tech-stack.md` and the README are updated, and roadmap entry 2.2 is marked ☑. (R9, R10)
