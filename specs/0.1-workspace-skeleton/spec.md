# 0.1 Workspace skeleton

**Stage:** 0 (Foundations) · **Status:** ☐ not started · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R6) · [plan.md](plan.md) (tasks).

## Goal

Create the Cargo workspace that every later phase builds on. At the end of this phase the repository compiles, lints cleanly, runs tests, and ships a `rl-semaphore` binary whose `--help` shows the intended command surface. The crates exist but hold no domain logic yet.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-workspace-layout) | Workspace layout, package names, shared metadata |
| [R2](requirements.md#r2-toolchain) | Pinned toolchain |
| [R3](requirements.md#r3-formatting-and-lints) | rustfmt and clippy configuration |
| [R4](requirements.md#r4-library-crate-stubs) | Library crate stubs |
| [R5](requirements.md#r5-cli-stub) | `rl-semaphore` CLI stub |
| [R6](requirements.md#r6-repository-files) | `.gitignore`, licenses, README, roadmap update |

## Out of scope

- CI (phase 0.2).
- `tracing`, error conventions (`thiserror`/`anyhow`), `CONTRIBUTING.md`, ADRs (phase 0.3).
- Internal dependency edges between crates (`sim` ← `env` ← …). Each crate adds its dependencies when it first needs them.
- `cargo-deny`, `nextest`, and coverage configuration (phase 0.2).
- Any simulator, RL, server, or frontend logic.

## Acceptance criteria

All of the following pass from a clean clone. The IDs in parentheses are the requirements each check verifies.

1. `cargo build --workspace` succeeds with no warnings. (R1.1–R1.6, R1.8, R4.3)
2. `cargo build -p rl-semaphore-web --target wasm32-unknown-unknown` succeeds. (R2.2, R4.4)
3. `cargo fmt --all --check` passes. (R3.1)
4. `cargo clippy --workspace --all-targets -- -D warnings` passes. (R3.2–R3.6, R4.1, R5.7)
5. `cargo test --workspace` passes, with at least one test per crate (9 in total). (R4.2, R5.8)
6. `cargo run -p rl-semaphore -- --help` lists `simulate`, `train`, `eval` and `serve` with their `about` text. (R5.1–R5.3)
7. `cargo run -p rl-semaphore -- --version` prints `rl-semaphore 0.1.0`. (R5.4)
8. `cargo run -p rl-semaphore` with no subcommand exits with code 2. (R5.5)
9. `cargo run -p rl-semaphore -- simulate` prints the not-implemented message to stderr and exits with code 1. (R5.6)
10. `rust-toolchain.toml` pins an exact `X.Y.Z` version that matches `rust-version`. (R2.1, R2.3)
11. `Cargo.lock` is tracked, and `.gitignore`, `LICENSE-MIT`, `LICENSE-APACHE` and `README.md` exist with the content required. Checked in review. (R1.7, R6.1–R6.3)
12. Roadmap entry 0.1 is marked ☑. (R6.4)
