# RL_Semaphore

RL_Semaphore trains reinforcement-learning agents to control the traffic lights of a small simulated city, and lets you watch them learn. It is a learning and portfolio project built to production standards: a deterministic Rust simulator, in-house DQN and PPO agents on Burn, a training pipeline with experiment tracking, an Axum server, and a Leptos web UI compiled to WebAssembly.

[![CI](https://github.com/CarlosEdDL/RL_Semaphore/actions/workflows/ci.yml/badge.svg?branch=trunk)](https://github.com/CarlosEdDL/RL_Semaphore/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

## Fairness objective

The primary objective is to minimize average vehicle waiting time, subject to a fairness constraint: no vehicle should "take the bullet for the team." A policy that lowers the average by letting a few vehicles wait almost forever is treated as a failure, not a win. Fairness is enforced in the reward (a super-linear individual-wait penalty), in a non-bypassable max-red safety mask, and in the metrics (mean, p95, p99 and max wait).

## Architecture

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

### Crate map

| Folder | Package | Responsibility |
|--------|---------|----------------|
| `crates/sim` | `rl-semaphore-sim` | Pure, deterministic traffic simulation |
| `crates/env` | `rl-semaphore-env` | RL environment, safety layer and baseline controllers |
| `crates/agents` | `rl-semaphore-agents` | DQN and PPO on Burn |
| `crates/trainer` | `rl-semaphore-trainer` | Training loop, evaluation, checkpointing |
| `crates/storage` | `rl-semaphore-storage` | Run registry and metrics (SQLite), checkpoints |
| `crates/protocol` | `rl-semaphore-protocol` | Shared REST and WebSocket DTOs |
| `crates/server` | `rl-semaphore-server` | HTTP/WS API and run management |
| `crates/web` | `rl-semaphore-web` | Leptos frontend compiled to WASM |
| `crates/cli` | `rl-semaphore` | `rl-semaphore` binary (`simulate`, `train`, `eval`, `serve`) |

Run configs live in `configs/`; `configs/single-intersection.toml` describes a full example scenario (time step, 4-way intersection geometry, signal plan, demand and fixed-time plan) and documents the schema.

## Getting started

Prerequisite: [`rustup`](https://rustup.rs). The pinned toolchain installs itself on first use.

```sh
cargo build
cargo run -p rl-semaphore -- --help
```

Run the fixed-time baseline on the example scenario and print its metrics (`--steps` defaults to 3600 and `--seed` to 0; `--output json` prints one JSON document instead of the table):

```sh
cargo run --release -p rl-semaphore -- simulate --config configs/single-intersection.toml
```

Logs go to stderr. Use `-v` / `-q` to change the level (or `RUST_LOG`), and `--log-format json` (or `RL_SEMAPHORE_LOG_FORMAT=json`) for one JSON object per line:

```sh
cargo run -p rl-semaphore -- --log-format json -v simulate
```

To reproduce CI locally (needs `cargo-nextest`):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p rl-semaphore-protocol --all-targets --features sim --locked -- -D warnings
cargo nextest run --workspace --locked
cargo nextest run -p rl-semaphore-protocol --features sim --locked
cargo test --workspace --doc --locked
cargo bench -p rl-semaphore-env --bench throughput --locked -- --test
cargo build -p rl-semaphore-protocol --target wasm32-unknown-unknown --locked
```

## Baseline

The fixed-time controller on `configs/single-intersection.toml` (seed 0, 3,600 steps of 1 s, demand of 1,100 veh/h) gives the first baseline. Every later controller has to beat it on the mean **and** on the tail.

| Metric | Value |
|--------|-------|
| Mean wait | 29.8 s |
| p95 wait | 69.0 s |
| p99 wait | 113.0 s |
| Max wait | 152.0 s |
| Throughput | 1,118 veh/h |

The plan serves the phases in list order with 10 s (`ns-left`), 30 s (`ns-through`), 10 s (`east`) and 10 s (`west`) of green, an 80 s cycle. The side-road phases wait the most: the west approach has a mean wait of 49.3 s and a max of 152.0 s. The full table is in the snapshot `crates/cli/tests/snapshots/simulate_snapshot__text_output_is_pinned.snap`.

## Performance

```sh
cargo bench -p rl-semaphore-env --bench throughput
```

Steps per second, at two layers (`sim_step`: spawn, `FixedTime::command`, `Simulation::step`; `episode`: the same plus the demand draw and the metrics) and three workloads built from `configs/single-intersection.toml` (only the demand changes):

| Workload | Demand | `sim_step` | `episode` |
|----------|--------|------------|-----------|
| `empty` | 0 veh/h | 10.7 M | 8.64 M |
| `example` | 1,100 veh/h | 2.11 M | 1.70 M |
| `heavy` | 1,430 veh/h | 1.58 M | 1.29 M |

Measured on an i7-12700 (20 threads), WSL2 on Windows, Rust 1.98.1, on 2026-09-27. These numbers are for demand the fixed-time plan can serve; the step rate falls sharply once queues grow without bound (tracked for 11.1).

## Project status

Stage 1, Simulator, is done. Stage 2, Visualization of the simulator, has started: 2.1 (protocol crate) is done, giving the versioned `Hello`/`Snapshot`/`Metrics` DTOs the server and web will share. Next is 2.2 (Axum server). See the [roadmap](specs/roadmap.md).

## Documentation

- [Mission](specs/mission.md)
- [Tech stack](specs/tech-stack.md)
- [Roadmap](specs/roadmap.md)
- [Contributing](CONTRIBUTING.md)
- [Architecture decision records](specs/adr/README.md)

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
