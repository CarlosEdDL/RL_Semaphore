# 0.1 Workspace skeleton: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feat/0.1-workspace-skeleton`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Toolchain and root config
- [ ] Find the latest stable version (`rustup check`, or the Rust release notes) and write `rust-toolchain.toml` with an exact `channel`, `profile = "minimal"`, `rustfmt` + `clippy`, and the `wasm32-unknown-unknown` target.
- [ ] Add `rustfmt.toml` (stable options only).
- [ ] Add `clippy.toml` with `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true`.
- [ ] Add `.gitignore`.

### 2. Workspace manifest
- [ ] Write the root `Cargo.toml`: `[workspace]` with `members = ["crates/*"]` and `resolver = "3"`, plus `[workspace.package]`, `[workspace.dependencies]` (`clap` with the `derive` feature) and `[workspace.lints]`.

### 3. Library crates (×8)
For each of `sim`, `env`, `agents`, `trainer`, `storage`, `protocol`, `server`, `web`:
- [ ] Write `crates/<name>/Cargo.toml` with `name = "rl-semaphore-<name>"`, the inherited package fields, `publish = false`, and `[lints] workspace = true`.
- [ ] Write `crates/<name>/src/lib.rs` with a `//!` doc comment and one `#[cfg(test)]` test.
- [ ] In `sim`, `env`, `agents` and `protocol` only, add `#![warn(clippy::pedantic)]`.

### 4. CLI crate
- [ ] Write `crates/cli/Cargo.toml` for package `rl-semaphore`, with a `[[bin]]` named `rl-semaphore` and `clap.workspace = true`.
- [ ] In `crates/cli/src/main.rs`, add a `Cli` struct (`#[command(version, about)]`) and a `Command` enum with `Simulate`, `Train`, `Eval` and `Serve`. `main() -> ExitCode` prints the stub message to stderr and returns `ExitCode::FAILURE`.
- [ ] Add a unit test for argument parsing (`Cli::try_parse_from`) covering each subcommand, plus `Cli::command().debug_assert()`.

### 5. Docs and licenses
- [ ] Add `LICENSE-MIT` and `LICENSE-APACHE`.
- [ ] Write `README.md` following [requirements.md R6.3](requirements.md#r6-repository-files).
- [ ] Mark 0.1 as ☑ in `specs/roadmap.md`.

### 6. Verify
Run every acceptance-criteria command from spec.md:

```sh
cargo build --workspace
cargo build -p rl-semaphore-web --target wasm32-unknown-unknown
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p rl-semaphore -- --help
cargo run -p rl-semaphore -- --version
cargo run -p rl-semaphore -- simulate; echo "exit=$?"   # expect exit=1
```

## Notes and risks

- **Per-crate pedantic.** A crate that uses `[lints] workspace = true` can't add its own lint entries in `Cargo.toml`, so pedantic is enabled with a crate attribute. If pedantic ever needs to cover more crates, consider moving it into `[workspace.lints]` with targeted `allow`s.
- **`missing_docs`.** This lint only fires on public items. The stubs have none yet, but the crate-level `//!` comment sets the convention.
- **The `env` package name.** Using the `rl-semaphore-` prefix avoids confusing it with `std::env`. Its Rust crate name becomes `rl_semaphore_env`.
