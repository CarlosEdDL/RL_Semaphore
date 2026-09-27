# 0.3 Observability & errors baseline

**Stage:** 0 (Foundations) · **Status:** ☑ done · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R7) · [plan.md](plan.md) (tasks).

## Goal

Set the conventions every later crate follows for logging, errors, contributing, and recording decisions. At the end of this phase the `rl-semaphore` binary initializes structured `tracing` output (pretty or JSON, with controllable verbosity), reports failures as an `anyhow` cause chain with a non-zero exit code, the repository has a `CONTRIBUTING.md` that writes down the error and logging rules, and `specs/adr/` holds the ADR process and the first ADR (why Rust + Burn + Leptos).

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-dependencies) | Workspace dependencies (`tracing`, `tracing-subscriber`, `anyhow`, `thiserror`) |
| [R2](requirements.md#r2-logging-in-the-cli) | Subscriber setup, `--log-format`, `-v`/`-q`, `RUST_LOG` |
| [R3](requirements.md#r3-error-handling-in-the-cli) | `anyhow` at the binary boundary, exit codes, cause chain |
| [R4](requirements.md#r4-conventions-for-library-crates) | Rules for libraries: `thiserror`, no subscriber, no printing |
| [R5](requirements.md#r5-contributingmd) | `CONTRIBUTING.md` |
| [R6](requirements.md#r6-architecture-decision-records) | ADR index, template, ADR-0001 |
| [R7](requirements.md#r7-docs-and-roadmap) | README links, roadmap update |

## Decisions taken

- **Log format:** global `--log-format pretty|json` flag, also settable through `RL_SEMAPHORE_LOG_FORMAT`. Default `pretty`. JSON is the format Docker and production will use later.
- **Verbosity:** `RUST_LOG` (tracing `EnvFilter`) wins when it is set. Otherwise the default level is `info`, and global `-v`/`-q` flags move it up or down.
- **Errors:** `thiserror` in library crates, `anyhow` only in the `cli` binary, as tech-stack already states. `main` prints the full cause chain and returns a non-zero `ExitCode`.
- **ADRs:** Nygard-style (Context / Decision / Consequences). `specs/adr/README.md` holds the index and process, `0000-template.md` the template, and `0001-rust-burn-leptos.md` the first decision.

## Out of scope

- Adding `tracing` or error types to library crates. Each crate adds them when it first has something to log or fail on (starting in 1.1). This phase only adds the workspace entries and the written rules.
- `tower-http` request tracing in the server (phase 2.2).
- OpenTelemetry, log shipping, metrics exporters, or any external service.
- Log files or rotation. Logs go to stderr only.
- Implementing the subcommands. They still fail with "not implemented yet", now as a proper `anyhow` error.
- ADRs for later decisions (simulation model, DQN details, storage). Those are written in the phase that takes the decision.

## Acceptance criteria

1. `cargo run -p rl-semaphore -- simulate` prints an `ERROR`-level log line or error message containing `not implemented yet` on stderr and exits with code 1. (R3.1–R3.3)
2. `cargo run -p rl-semaphore -- --log-format json simulate` writes one JSON object per line on stderr, each parseable as JSON. `RL_SEMAPHORE_LOG_FORMAT=json` has the same effect. (R2.2, R2.3)
3. With no flags and no `RUST_LOG`, `debug` events are not shown. With `-v` they are. With `-q`, `info` events are not shown. With `RUST_LOG=debug`, `debug` events are shown regardless of `-q`. (R2.4, R2.5)
4. Nothing is written to stdout by the logging or error path. (R2.1, R3.2)
5. `rl-semaphore --help` documents `--log-format`, `-v`, and `-q`. (R2.2, R2.4)
6. Unit tests cover the verbosity-to-level mapping and flag parsing, and an integration test runs the binary and checks the exit code and JSON output. All pass under `cargo nextest`. (R2.7)
7. `CONTRIBUTING.md` exists and covers the items in R5. (R5)
8. `specs/adr/README.md`, `specs/adr/0000-template.md`, and `specs/adr/0001-rust-burn-leptos.md` exist, and ADR-0001 has status `Accepted`. (R6)
9. `cargo deny check` still passes with the new dependencies. CI is green. (R1.3)
10. Roadmap entry 0.3 is marked ☑. (R7.2)
