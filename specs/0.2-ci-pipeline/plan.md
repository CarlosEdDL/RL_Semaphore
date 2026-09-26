# 0.2 CI pipeline: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feat/0.2-ci-pipeline`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Supply-chain policy
- [ ] Install cargo-deny locally (`cargo install cargo-deny --locked`, or `cargo binstall`).
- [ ] Generate a starting point with `cargo deny init`, then trim it to [R3.1](requirements.md#r3-supply-chain-jobs).
- [ ] Run `cargo deny check` and add to the license allowlist only the licenses it reports for the current tree (R3.2).
- [ ] If workspace crates are reported, set `private = { ignore = true }` (R3.3).

### 2. Main workflow
- [ ] Create `.github/workflows/ci.yml` with triggers, `permissions`, `concurrency`, and `env` ([R1](requirements.md#r1-workflow-triggers-and-structure)).
- [ ] Add jobs `fmt`, `clippy`, `test`, `wasm` ([R2](requirements.md#r2-quality-jobs)).
  - Toolchain: run `rustup show` (or `rustup toolchain install` with no argument) so rustup installs what `rust-toolchain.toml` says. Do not hard-code a version.
  - Add `Swatinem/rust-cache@v2` with distinct `shared-key`s (R5.1).
- [ ] Add jobs `deny` and `audit` ([R3](requirements.md#r3-supply-chain-jobs)), using `taiki-e/install-action@v2`.
- [ ] Add job `coverage` ([R4](requirements.md#r4-coverage-job)): install `llvm-tools-preview`, run llvm-cov with nextest, upload the artifact, and write the summary.
- [ ] Add the weekly `schedule` trigger and the `if:` conditions so only `deny` and `audit` run on scheduled events (R1.2).

### 3. Dependabot
- [ ] Create `.github/dependabot.yml` for `github-actions` and `cargo`, weekly, grouped, with the `chore(deps)` prefix ([R6](requirements.md#r6-repository-hygiene)).

### 4. Docs and roadmap
- [ ] Replace the badge placeholder in `README.md` (R7.1). The repository is `CarlosEdDL/RL_Semaphore`.
- [ ] Add the CI-equivalent commands to the README "Getting started" (R7.3).
- [ ] Mark 0.2 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done.

### 5. Verify locally
Run what CI will run, before pushing:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
cargo build -p rl-semaphore-web --target wasm32-unknown-unknown --locked
cargo deny --locked check
cargo audit
cargo llvm-cov nextest --workspace --locked --lcov --output-path lcov.info
```

Optionally validate the workflow syntax with `actionlint` if it is installed.

### 6. Verify on GitHub (manual, by the author)
- [ ] Open the PR and confirm all seven jobs go green (spec criterion 2).
- [ ] On a throwaway branch (not merged), introduce a format error, a clippy warning, and a failing test, and confirm each makes the matching job fail (criterion 3).
- [ ] Push a second commit to the PR and confirm the earlier run is cancelled (criterion 4). Re-run and confirm the cache is restored (criterion 5).
- [ ] After merge, in **Settings → Branches**, protect `trunk`: require a PR, and require the checks `fmt`, `clippy`, `test`, `wasm`, `deny`, `audit`, `coverage`. Job names are only selectable after they have run once.

## Notes and risks

- **Toolchain from the file.** `rust-toolchain.toml` pins `1.98.1` with the `wasm32` target, so rustup downloads it on first use in every job. The cache action stores the toolchain-related build artifacts but not the toolchain itself, so expect about 30–60 seconds of setup per job. This is acceptable for now, and it keeps one source of truth.
- **`llvm-tools-preview`.** The pinned profile is `minimal`, so the component is added in the coverage job only. If cargo-llvm-cov asks for `llvm-tools` instead, use that name. Check the tool's output on the first run.
- **Doctests.** All crates are stubs with no public items, so `cargo test --doc` runs nothing yet. It stays in the workflow so doctests are covered once public APIs exist.
- **`audit` and `deny` overlap.** Both read the RustSec database. `audit` is kept on purpose (tech-stack lists both). If the noise is a problem, drop `audit` in a later phase and amend this spec.
- **Scheduled runs on forks.** Scheduled workflows are disabled on inactive repositories after 60 days. A new push to the repository re-enables them.
- **Coverage on wasm crate.** `cargo llvm-cov` builds the `web` crate for the host target, which works for the empty stub. When `web` gains browser-only code, exclude it with `--exclude rl-semaphore-web`, and record that change in the phase that adds it.
- **Cache key collisions.** If two jobs share a cache key with different flags, they thrash each other's cache. Keep the `shared-key`s distinct (R5.1).
