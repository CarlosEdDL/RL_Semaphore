# 0.3 Observability & errors baseline: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

## R1. Dependencies

- **R1.1** `[workspace.dependencies]` MUST declare `tracing`, `tracing-subscriber` (features `env-filter`, `fmt`, `json`), `anyhow`, and `thiserror`, each with a major-version requirement only (for example `"1"`), matching the existing `clap` entry.
- **R1.2** Only `crates/cli` MUST depend on `tracing-subscriber` and `anyhow` in this phase. `thiserror` stays declared in the workspace and unused until a library crate needs it (R4.1).
- **R1.3** `cargo deny --locked check` MUST pass. If a new dependency brings a license not in `deny.toml`, it MAY be added to the allowlist only if it is permissive, with a comment naming the crate that needs it (R3.2 of phase 0.2).
- **R1.4** `Cargo.lock` MUST be committed with the new entries.

## R2. Logging in the CLI

- **R2.1** All log output MUST go to stderr. Stdout is reserved for command results (metrics, tables, JSON reports in later phases).
- **R2.2** The CLI MUST accept a global `--log-format <pretty|json>` option, valid before or after the subcommand (clap `global = true`). The default MUST be `pretty`.
- **R2.3** The same option MUST be settable through the environment variable `RL_SEMAPHORE_LOG_FORMAT`. The flag MUST take precedence over the variable.
- **R2.4** The CLI MUST accept global, repeatable `-v`/`--verbose` and `-q`/`--quiet` flags. With neither, the default level is `info`. Each `-v` raises it one step (`debug`, then `trace`), each `-q` lowers it one step (`warn`, `error`, then off). `-v` and `-q` MAY be combined, and their counts net out. The level MUST saturate at `trace` and off.
- **R2.5** If `RUST_LOG` is set and non-empty, it MUST be used as the `EnvFilter` directive and the `-v`/`-q` flags MUST be ignored. An invalid `RUST_LOG` MUST NOT panic: the CLI MUST fall back to the flag-derived level and emit a `warn` event saying the variable was ignored.
- **R2.6** The subscriber MUST be initialized once, in a dedicated module of `crates/cli` (for example `logging.rs`), before any other work in `main`. The pretty format SHOULD disable ANSI colors when stderr is not a terminal. The JSON format MUST emit one object per line, with at least timestamp, level, target, and message fields.
- **R2.7** The mapping from flags to a level MUST be a pure function with unit tests. An integration test in `crates/cli/tests/` MUST run the built binary (through `env!("CARGO_BIN_EXE_rl-semaphore")` and `std::process::Command`, without extra dev-dependencies) and check the exit code and that every stderr line is valid JSON when `--log-format json` is used. If a JSON parser is needed in tests, `serde_json` MAY be added as a dev-dependency.
- **R2.8** On start-up, the CLI MUST emit one `debug` event with the crate version and the chosen subcommand.

## R3. Error handling in the CLI

- **R3.1** The command dispatch MUST be a function returning `anyhow::Result<()>`. `main` MUST return `std::process::ExitCode`, mapping `Ok` to `SUCCESS` and `Err` to `FAILURE` (code 1). `main` MUST NOT return `Result` (that prints the `Debug` format).
- **R3.2** On error, `main` MUST report the full cause chain (the `{:#}` format of `anyhow::Error`) once, through a `tracing::error!` event, so it respects `--log-format`. It MUST NOT print to stdout.
- **R3.3** Each unimplemented subcommand MUST return an `anyhow` error whose message contains `not implemented yet` and the subcommand name. The direct `eprintln!` in `main` MUST be removed.
- **R3.4** Clap usage errors (unknown flag, missing subcommand) keep clap's own output and exit code 2. They MUST NOT go through the tracing path.
- **R3.5** Code in `crates/cli` MUST add context to fallible calls with `anyhow::Context` (`.context(...)` / `.with_context(...)`) when the underlying error alone would not tell the user what was being attempted. This is a convention for later phases; there are no fallible calls yet besides the subscriber setup.

## R4. Conventions for library crates

These rules are written in `CONTRIBUTING.md` (R5) and apply from phase 1.1 on. No library code changes in this phase.

- **R4.1** Library crates MUST expose errors as `thiserror` enums (one per crate or per module), MUST NOT depend on `anyhow`, and MUST NOT return `Box<dyn Error>` in public APIs.
- **R4.2** Library crates MUST NOT install a `tracing` subscriber and MUST NOT print to stdout or stderr. They emit `tracing` events and spans only. Binaries (`cli`, and later `server`) own the subscriber.
- **R4.3** `unwrap()`/`expect()` outside tests MUST carry a `// SAFETY:`-style comment (use `// INVARIANT:` wording) explaining why it cannot fail, or the clippy warning MUST be kept. `#[allow(clippy::unwrap_used)]` without a comment is not allowed.
- **R4.4** `sim` hot loops MUST NOT log per vehicle or per step at `info` or above. Per-step detail belongs at `trace`, and `debug` is for per-episode events. This protects the throughput benchmark (phase 1.7).
- **R4.5** Panics are for bugs (broken invariants), errors are for conditions a caller can hit (bad config, missing file). Invalid user input MUST produce an error, never a panic.

## R5. CONTRIBUTING.md

A `CONTRIBUTING.md` at the repository root MUST cover:

- **R5.1** Setup: rustup, the pinned toolchain, and the extra tools (`cargo-nextest`, `cargo-deny`, `cargo-audit`, `cargo-llvm-cov`).
- **R5.2** The local CI commands (the same list as the README "Getting started", linked rather than duplicated if possible).
- **R5.3** Workflow: one roadmap phase ≈ one PR, spec files in `specs/<phase>/` (`spec.md`, `requirements.md`, `plan.md`) written before code, branch naming `feat/<phase>-<slug>`, Conventional Commits, squash merge into `trunk`.
- **R5.4** Error conventions (R3.5, R4.1, R4.3, R4.5) and logging conventions (R2.1, R4.2, R4.4, including level guidance: `error` / `warn` / `info` / `debug` / `trace`).
- **R5.5** Determinism rules from tech-stack (seeded `ChaCha8Rng`, no wall-clock time or `HashMap` iteration order in `sim`).
- **R5.6** When to write an ADR, with a link to `specs/adr/README.md`.

## R6. Architecture decision records

- **R6.1** `specs/adr/README.md` MUST explain what an ADR is here, when one is required (a decision that is hard to reverse, affects several crates, or picks between real alternatives), the file naming (`NNNN-kebab-title.md`, numbered in order, never renumbered), the status values (`Proposed`, `Accepted`, `Superseded by NNNN`, `Deprecated`), and MUST contain an index table (number, title, status, date).
- **R6.2** `specs/adr/0000-template.md` MUST follow the Nygard format: title, Status, Date, Context, Decision, Consequences (positive and negative).
- **R6.3** `specs/adr/0001-rust-burn-leptos.md` MUST record the decision to write the whole system in Rust with Burn for RL and Leptos for the frontend. Its Context MUST name the alternatives considered (at least Python + PyTorch/Stable-Baselines3 for RL, `tch-rs`/`candle` for Rust ML, and a TypeScript frontend or Yew/Dioxus for the UI). Its Consequences MUST include the costs (smaller RL ecosystem, in-house DQN/PPO, WASM build complexity), not only the benefits. Status `Accepted`, date of the PR.
- **R6.4** ADRs MUST be written in the same plain style as the existing specs.

## R7. Docs and roadmap

- **R7.1** The README "Documentation" section MUST link to `CONTRIBUTING.md` and `specs/adr/README.md`. The README "Getting started" SHOULD show one logging example (`--log-format json` or `-v`).
- **R7.2** Roadmap entry 0.3 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.
