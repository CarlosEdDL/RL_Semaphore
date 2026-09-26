# 0.1 Workspace skeleton: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

## R1. Workspace layout

- **R1.1** The repository root MUST contain a virtual workspace manifest (`Cargo.toml` with `[workspace]` and no `[package]`).
- **R1.2** The workspace MUST contain exactly these crates:

  | Folder | Package name | Kind |
  |--------|--------------|------|
  | `crates/sim` | `rl-semaphore-sim` | lib |
  | `crates/env` | `rl-semaphore-env` | lib |
  | `crates/agents` | `rl-semaphore-agents` | lib |
  | `crates/trainer` | `rl-semaphore-trainer` | lib |
  | `crates/storage` | `rl-semaphore-storage` | lib |
  | `crates/protocol` | `rl-semaphore-protocol` | lib |
  | `crates/server` | `rl-semaphore-server` | lib |
  | `crates/web` | `rl-semaphore-web` | lib |
  | `crates/cli` | `rl-semaphore` | bin (`rl-semaphore`) |

- **R1.3** The workspace MUST use `resolver = "3"`.
- **R1.4** `[workspace.package]` MUST define `version = "0.1.0"`, `edition = "2024"`, `license = "MIT OR Apache-2.0"`, `rust-version`, `repository` and `authors`, and every crate MUST inherit them with `*.workspace = true`.
- **R1.5** Third-party dependency versions MUST be declared once in `[workspace.dependencies]` and referenced with `.workspace = true`. In this phase the only third-party dependency MUST be `clap`.
- **R1.6** Every crate MUST set `publish = false`.
- **R1.7** `Cargo.lock` MUST be committed.
- **R1.8** Crates MUST NOT depend on each other in this phase.

## R2. Toolchain

- **R2.1** `rust-toolchain.toml` MUST pin an exact stable version (`X.Y.Z`), and that version SHOULD be the latest stable at implementation time.
- **R2.2** The toolchain file MUST include `components = ["rustfmt", "clippy"]`, `targets = ["wasm32-unknown-unknown"]` and `profile = "minimal"`.
- **R2.3** `rust-version` in `[workspace.package]` MUST match the pinned version.

## R3. Formatting and lints

- **R3.1** A `rustfmt.toml` MUST exist and MUST NOT use nightly-only options.
- **R3.2** `[workspace.lints]` MUST set:
  - `rust`: `unsafe_code = "forbid"`, `missing_docs = "warn"`.
  - `clippy`: `unwrap_used = "warn"`, `expect_used = "warn"`.
- **R3.3** Every crate MUST opt in with `[lints] workspace = true`.
- **R3.4** `clippy.toml` MUST set `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true`.
- **R3.5** The core crates (`sim`, `env`, `agents`, `protocol`) MUST enable `clippy::pedantic` with a crate-level `#![warn(clippy::pedantic)]`. Other crates MUST NOT enable it in this phase.
- **R3.6** Manifests SHOULD NOT use `deny` for warnings. Warnings are turned into errors at check time with `-D warnings`.

## R4. Library crate stubs

- **R4.1** Each library crate's `lib.rs` MUST start with a `//!` doc comment of one or two sentences that states the crate's responsibility as given in `tech-stack.md`.
- **R4.2** Each library crate MUST contain at least one `#[cfg(test)]` unit test.
- **R4.3** Library crates MUST NOT expose public items or declare dependencies in this phase.
- **R4.4** `rl-semaphore-web` MUST compile for `wasm32-unknown-unknown`.

## R5. CLI stub

- **R5.1** The `rl-semaphore` package MUST build a binary named `rl-semaphore` that uses `clap` with the derive API.
- **R5.2** The CLI MUST define exactly these subcommands, each with the given one-line `about` text:

  | Subcommand | About |
  |------------|-------|
  | `simulate` | Run the traffic simulator with a baseline controller and print metrics |
  | `train` | Train an RL agent from a config file |
  | `eval` | Evaluate a policy or baseline over several seeds |
  | `serve` | Start the web server and dashboard |

- **R5.3** `rl-semaphore --help` MUST list all four subcommands.
- **R5.4** `rl-semaphore --version` MUST print `rl-semaphore 0.1.0`.
- **R5.5** Running `rl-semaphore` with no subcommand MUST print usage and exit with code 2.
- **R5.6** Each subcommand MUST print `rl-semaphore <name>: not implemented yet` to stderr and exit with code 1.
- **R5.7** The CLI code MUST NOT use `unwrap`/`expect` outside tests, and `main` MUST return `std::process::ExitCode`.
- **R5.8** The CLI MUST have unit tests that parse each subcommand and call `Cli::command().debug_assert()`.

## R6. Repository files

- **R6.1** `.gitignore` MUST ignore `/target`, `/data` and `*.db`, and SHOULD ignore common editor and OS files.
- **R6.2** The repository MUST contain `LICENSE-MIT` and `LICENSE-APACHE`, each naming Carlos Duque as the copyright holder.
- **R6.3** `README.md` MUST contain these sections in order:
  1. Title and a one-paragraph pitch (from `mission.md`).
  2. A badge placeholder: `<!-- badges: CI, license (added in 0.2) -->`.
  3. The fairness objective in two or three sentences.
  4. The architecture diagram from `tech-stack.md`.
  5. A crate map table (folder, package, responsibility).
  6. Getting started: prerequisites (`rustup`; the toolchain installs itself), then `cargo build`, `cargo test`, and `cargo run -p rl-semaphore -- --help`.
  7. Project status: "Stage 0, Foundations", with a link to the roadmap.
  8. Links to `specs/mission.md`, `specs/tech-stack.md` and `specs/roadmap.md`.
  9. License section (dual MIT/Apache-2.0).
- **R6.4** Roadmap entry 0.1 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase.
