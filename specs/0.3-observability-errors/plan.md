# 0.3 Observability & errors baseline: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feat/0.3-observability-errors`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Dependencies
- [x] Add `tracing`, `tracing-subscriber` (`env-filter`, `fmt`, `json`), `anyhow`, `thiserror` to `[workspace.dependencies]` ([R1.1](requirements.md#r1-dependencies)).
- [x] Add `tracing`, `tracing-subscriber`, `anyhow` to `crates/cli/Cargo.toml` with `.workspace = true` (R1.2).
- [x] Run `cargo deny --locked check`. Extend the license allowlist only if needed, with a comment (R1.3).

### 2. Logging module
- [x] Add `LogFormat` (`ValueEnum`: `pretty`, `json`) and global args to `Cli`: `--log-format` with `env = "RL_SEMAPHORE_LOG_FORMAT"`, `-v` and `-q` with `ArgAction::Count` ([R2.2–R2.4](requirements.md#r2-logging-in-the-cli)). The `env` attribute needs clap's `env` feature on the workspace `clap` entry.
- [x] Create `crates/cli/src/logging.rs`:
  - `fn level_from_verbosity(verbose: u8, quiet: u8) -> LevelFilter` (pure, saturating).
  - `fn init(format: LogFormat, level: LevelFilter) -> anyhow::Result<()>`: build the `EnvFilter` from `RUST_LOG` if set and valid, otherwise from `level`; remember a parse failure and emit the `warn` after init (R2.5). Writer is `std::io::stderr`. Pretty: `with_ansi(std::io::stderr().is_terminal())`. JSON: `.json()` (R2.6).
- [x] Unit tests for `level_from_verbosity` (defaults, each step, saturation, netting) and for parsing `--log-format`, `-v`/`-vv`, `-q`, flags after the subcommand (R2.7).

### 3. Error path
- [x] Add `fn run(cli: &Cli) -> anyhow::Result<()>` that dispatches each subcommand to `anyhow::bail!("{name}: not implemented yet")` ([R3.1, R3.3](requirements.md#r3-error-handling-in-the-cli)).
- [x] Rewrite `main`: parse, init logging (if init fails, fall back to `eprintln!` of the error and return `FAILURE`), emit the start-up `debug` event (R2.8), call `run`, on `Err(e)` emit `tracing::error!(error = format!("{e:#}"), "command failed")` or equivalent and return `ExitCode::FAILURE` (R3.2). Remove the old `eprintln!`.
- [x] Integration test `crates/cli/tests/cli.rs` (R2.7):
  - `simulate` exits with code 1 and stderr contains `not implemented yet`.
  - `--log-format json simulate` exits with code 1 and every non-empty stderr line parses as JSON (add `serde_json` as a dev-dependency) with a `level` of `ERROR` present.
  - `RL_SEMAPHORE_LOG_FORMAT=json` gives the same JSON output.
  - `-v` shows the start-up `debug` event, the default does not. Clear `RUST_LOG` in each test with `Command::env_remove`.
  - stdout is empty in every case.

### 4. CONTRIBUTING.md
- [x] Write `CONTRIBUTING.md` covering R5.1–R5.6 ([R5](requirements.md#r5-contributingmd)). Keep the error and logging rules short, with a code example of a `thiserror` enum and of `anyhow::Context` in the binary.

### 5. ADRs
- [x] `specs/adr/README.md`: purpose, when to write one, naming, statuses, index table ([R6.1](requirements.md#r6-architecture-decision-records)).
- [x] `specs/adr/0000-template.md` (R6.2).
- [x] `specs/adr/0001-rust-burn-leptos.md` with alternatives and honest costs (R6.3). Reuse reasoning from `specs/mission.md` and `specs/tech-stack.md`, do not copy them.

### 6. Docs and roadmap
- [x] README: link `CONTRIBUTING.md` and the ADR index under "Documentation", add a logging example to "Getting started" ([R7.1](requirements.md#r7-docs-and-roadmap)).
- [x] Mark 0.3 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R7.2).

### 7. Verify locally

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
cargo build -p rl-semaphore-web --target wasm32-unknown-unknown --locked
cargo deny --locked check
cargo audit

# manual checks for the acceptance criteria
cargo run -q -p rl-semaphore -- simulate; echo "exit=$?"
cargo run -q -p rl-semaphore -- --log-format json -v simulate
RL_SEMAPHORE_LOG_FORMAT=json cargo run -q -p rl-semaphore -- simulate
RUST_LOG=debug cargo run -q -p rl-semaphore -- -q simulate
cargo run -q -p rl-semaphore -- simulate 2>/dev/null | wc -c   # expect 0
```

## Notes and risks

- **Clap `env` feature.** `#[arg(env = ...)]` requires `features = ["derive", "env"]` on the workspace `clap` entry. Without it the attribute fails to compile.
- **Warning emitted before the subscriber exists.** An invalid `RUST_LOG` is detected while building the subscriber, so the `warn` event must be emitted right after `init`, not during it.
- **Tests and the global subscriber.** A global subscriber can only be set once per process, and nextest runs each test in its own process, but `cargo test --doc` and plain `cargo test` share one. Keep `init` out of unit tests; test it only through the binary in the integration test.
- **New transitive licenses.** `tracing-subscriber` pulls `regex`, `sharded-slab`, `nu-ansi-term`, `serde_json`, and others. They are expected to be MIT/Apache, but `cargo deny` is the source of truth.
- **Duplicate versions.** `multiple-versions = "warn"` may report duplicates (for example two `windows-sys` versions). They are warnings, not failures; note them but do not chase them in this phase.
- **Missing docs lint.** `missing_docs = "warn"` applies to the binary crate's public items too. Keep `logging` items private or documented, since clippy runs with `-D warnings`.
- **Coverage.** The integration test spawns the binary; `cargo llvm-cov nextest` instruments it automatically, so no workflow change is needed.
