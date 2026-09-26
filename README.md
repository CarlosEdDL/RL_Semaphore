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

## Getting started

Prerequisite: [`rustup`](https://rustup.rs). The pinned toolchain installs itself on first use.

```sh
cargo build
cargo run -p rl-semaphore -- --help
```

To reproduce CI locally (needs `cargo-nextest`):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
```

## Project status

Stage 0, Foundations. See the [roadmap](specs/roadmap.md).

## Documentation

- [Mission](specs/mission.md)
- [Tech stack](specs/tech-stack.md)
- [Roadmap](specs/roadmap.md)

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
