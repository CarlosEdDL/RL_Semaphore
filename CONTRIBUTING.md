# Contributing

## Setup

1. Install [`rustup`](https://rustup.rs). The toolchain pinned in `rust-toolchain.toml` (with the `wasm32-unknown-unknown` target) installs itself on first use.
2. Install the extra tools:

   ```sh
   cargo install --locked cargo-nextest cargo-deny cargo-audit cargo-llvm-cov
   ```

## Checking your changes locally

CI runs the same commands. The list is in the README under [Getting started](README.md#getting-started). Also run `cargo deny --locked check` and `cargo audit` when you touch dependencies.

## Workflow

- One roadmap phase is about one PR.
- Write the spec files in `specs/<phase>/` (`spec.md`, `requirements.md`, `plan.md`) before the code.
- Branch names: `feat/<phase>-<slug>`, for example `feat/0.3-observability-errors`.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org). The PR is squash-merged into `trunk`.
- A red CI check means the PR is not mergeable.

## Errors

- **Libraries** expose errors as `thiserror` enums (one per crate or module). They do not depend on `anyhow` and do not return `Box<dyn Error>` in public APIs.
- **Binaries** (`cli`, later `server`) use `anyhow` and add context with `.context(...)` or `.with_context(...)` when the underlying error would not say what was being attempted.
- **Panics are for bugs, errors are for conditions a caller can hit.** Bad config, a missing file or invalid user input is an error, never a panic.
- **`unwrap()` and `expect()`** outside tests need a comment starting with `// INVARIANT:` that explains why they cannot fail. Do not use `#[allow(clippy::unwrap_used)]` without one.

```rust
// In a library crate
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read config {path}")]
    Read { path: PathBuf, #[source] source: std::io::Error },
    #[error("max_red must be greater than min_green, got {max_red} and {min_green}")]
    InvalidTiming { max_red: u32, min_green: u32 },
}

// In the binary
let config = load_config(&path)
    .with_context(|| format!("failed to load config {}", path.display()))?;
```

## Logging

- Everything goes to **stderr**. Stdout is reserved for command results.
- Libraries emit `tracing` events and spans only. They never install a subscriber and never print. Binaries own the subscriber (`crates/cli/src/logging.rs`).
- `sim` hot loops do not log per vehicle or per step at `info` or above. Per-step detail is `trace`, per-episode events are `debug`.
- Levels:

  | Level | Use for |
  |-------|---------|
  | `error` | An operation failed and the caller or user must know |
  | `warn` | Something unexpected that was handled (an ignored setting, a fallback) |
  | `info` | Milestones a user cares about (run started, episode batch finished) |
  | `debug` | Details useful when diagnosing, per-episode events |
  | `trace` | Very fine detail, per step |

- Users control verbosity with `-v` / `-q`, or with `RUST_LOG` (which wins), and the format with `--log-format pretty|json`.

## Determinism

The simulator must give the same result for the same `config + seed + code version`:

- All randomness comes from a seeded `ChaCha8Rng`.
- No wall-clock time in `sim`.
- No `HashMap` iteration order in `sim`. Use `BTreeMap`, `IndexMap`, or sorted keys where order matters.

## Benchmarks

`crates/env/benches/throughput.rs` measures steps per second with `criterion`, at two layers (`sim_step`, `episode`) and three demand workloads (`empty`, `example`, `heavy`), built from `configs/single-intersection.toml`.

- Run it: `cargo bench -p rl-semaphore-env --bench throughput`.
- CI only runs `cargo bench -p rl-semaphore-env --bench throughput --locked -- --test`, which executes every benchmark once (including the workload sanity checks) without measuring, so a broken benchmark fails CI but a slowdown does not.
- Compare a change against a saved local baseline:

  ```sh
  # before the change
  cargo bench -p rl-semaphore-env --bench throughput -- --save-baseline before
  # after the change
  cargo bench -p rl-semaphore-env --bench throughput -- --baseline before
  ```

- A change to the step, demand, metrics or runner code SHOULD be benchmarked this way, with the result stated in the PR.
- The README's "Performance" table is updated only when a change is meant to move it, with the machine it was measured on. Otherwise leave it as is: it is noisy (WSL2, frequency scaling, background load) and indicative, not a regression gate.

## Architecture decisions

Write an ADR when a decision is hard to reverse, affects several crates, or picks between real alternatives. See [specs/adr/README.md](specs/adr/README.md).
