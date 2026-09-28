# Tech Stack

Everything is written in **Rust**, from the simulator and learning algorithms to the backend and the frontend (compiled to WebAssembly). One language means shared, type-checked data structures across the whole system.

## Architecture overview

```
┌────────────────────────────────────────────────────────────────┐
│                    Browser (Leptos → WASM)                     │
│   City canvas · training curves · run list · controls         │
└───────────────▲───────────────────────────┬────────────────────┘
                │ WebSocket (snapshots,     │ REST (runs, configs,
                │ live metrics)             │ start/stop training)
┌───────────────┴───────────────────────────▼────────────────────┐
│                     Server (Axum + Tokio)                      │
│   run manager · WS broadcaster · REST API · static assets      │
└──────┬─────────────────────────┬───────────────────────────────┘
       │                         │
┌──────▼───────┐   ┌─────────────▼─────────────┐   ┌─────────────┐
│   Trainer    │──▶│  Storage (SQLite + files) │   │  CLI (clap) │
│ (Burn agents)│   │  runs · metrics · ckpts   │◀──│ train / eval│
└──────┬───────┘   └───────────────────────────┘   └─────────────┘
       │ Env trait (reset / step / observe / action mask)
┌──────▼───────────────────────────────────────────────────────┐
│             Simulator core (pure, deterministic)             │
└──────────────────────────────────────────────────────────────┘
```

## Cargo workspace layout

| Crate | Responsibility | Key dependencies |
|-------|----------------|------------------|
| `sim` | Pure traffic simulation: road graph, vehicles, signals, demand generation, metrics. No I/O and no async. | `rand_chacha`, `serde` |
| `env` | RL environment trait (Gymnasium-like `reset`/`step`), observation/action/reward definitions, action masking and safety layer, baseline controllers. | `sim` |
| `agents` | DQN and PPO implemented on Burn, plus replay buffer, rollout buffer, and schedules. | `burn`, `env` |
| `trainer` | Training loop, evaluation, checkpointing, metric emission. | `agents`, `storage`, `tracing` |
| `storage` | Run registry and metrics in SQLite, checkpoints on the filesystem. | `sqlx` (SQLite, compile-time checked queries) |
| `protocol` | Shared DTOs for REST and WebSocket messages, versioned. Used by both server and web. | `serde` |
| `server` | HTTP/WS API, run management, serves the frontend bundle. | `axum`, `tokio`, `tower-http` |
| `web` | Leptos frontend compiled to WASM. | `leptos`, `web-sys`, `plotters` + `plotters-canvas` |
| `cli` | `rl-semaphore train | eval | simulate | serve` entry point. | `clap` |

The dependency direction is strict: `sim` ← `env` ← `agents` ← `trainer`. `sim` never depends on anything above it.

## Core technologies

### Simulation
- **Custom Rust simulator** with a fixed time step, running on a discrete cell model: lanes are split into fixed-length cells with integer positions and one vehicle per cell ([ADR-0002](adr/0002-discrete-cell-model.md)).
- **Determinism:** all randomness flows through a seeded `ChaCha8Rng`, and there is no dependency on wall-clock time or `HashMap` iteration order (use `BTreeMap`/`IndexMap`, or sorted keys, wherever order matters).
- Grid-city presets (1×1, 2×2, 3×3, 5×5) defined in **TOML** configs.

### Reinforcement learning
- **[Burn](https://burn.dev)** deep-learning framework.
  - `NdArray` backend for CPU (default, CI, Docker).
  - `Wgpu` backend (optional feature) for GPU training.
- Algorithms implemented in-house: **DQN** first (with target network, Double DQN, and Huber loss), then **PPO** (with GAE, clipped objective, and action masking).
- Multi-agent: independent learners per intersection, with an optional parameter-shared policy. Coordination (neighbor observations) comes later.
- **Fairness:** the reward includes a super-linear individual-wait penalty, and a non-bypassable max-red/min-green safety mask (see [mission.md](mission.md)).

### Backend
- **Axum** on **Tokio**, with `tower-http` for CORS, compression, tracing, and static files.
- WebSocket streaming of sim snapshots and metrics. Starts as JSON over versioned `protocol` types, with the option to switch to a binary format (`postcard`) if bandwidth requires it.
- Training runs execute in dedicated blocking threads (`spawn_blocking` / a worker thread) so they never block the async runtime. They communicate over channels.

### Frontend
- **Leptos** (CSR or SSR+hydration, built with `cargo-leptos`), compiled to WASM.
- City rendering on an HTML `<canvas>` through `web-sys`.
- Charts through `plotters` with the canvas backend.

### Storage and experiment tracking
- **SQLite** through `sqlx`, with migrations in `migrations/`. It stores runs, configs, git SHA, seeds, per-episode metrics, and evaluation results.
- **Filesystem** stores checkpoints (Burn recorder files) and recorded episodes for replay, under `data/runs/<run_id>/`.
- The dashboard is the experiment tracker. No external tracking service is used.

## Engineering practices (production grade)

| Area | Tooling / rule |
|------|----------------|
| Toolchain | Pinned via `rust-toolchain.toml` (stable), with the `wasm32-unknown-unknown` target |
| Formatting | `rustfmt`, checked in CI |
| Linting | `clippy` with `-D warnings`, and `pedantic` enabled for the core crates |
| Errors | `thiserror` in libraries, `anyhow` only in binaries. No `unwrap()`/`expect()` outside tests without justification |
| Logging | `tracing` + `tracing-subscriber` (structured, JSON in production) |
| Config | TOML files + `serde`, validated on load. Env overrides for server settings |
| Testing | Unit tests, integration tests, **property-based tests** (`proptest`) for sim invariants (vehicle conservation, no collisions, signal safety), and golden/snapshot tests (`insta`) for determinism |
| Test runner | `cargo-nextest` |
| Coverage | `cargo-llvm-cov`, reported in CI |
| Benchmarks | `criterion` in `crates/env/benches`, numbers recorded in the README, a smoke run in CI (`-- --test`, no timing gate), regressions checked locally against a saved baseline |
| Supply chain | `cargo-deny` (licenses, advisories, duplicate crates) and `cargo-audit` |
| Docs | `rustdoc` for public APIs, and ADRs in `specs/adr/` for significant decisions |
| Commits | Conventional Commits, one roadmap phase ≈ one PR |

## Delivery

- **Docker:** a multi-stage build (`cargo-chef` for layer caching). It produces a small runtime image (`debian:bookworm-slim` or distroless) containing the server binary and the WASM bundle. `docker compose` is used for local runs with a mounted `data/` volume.
- **GitHub Actions CI:** fmt → clippy → nextest → coverage → cargo-deny → WASM build → Docker build. Cargo is cached with `Swatinem/rust-cache`.
- **CD (later phase):** tagged releases push the image to GHCR and deploy a public demo (for example Fly.io).
