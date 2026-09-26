# 0.2 CI pipeline: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

## R1. Workflow triggers and structure

- **R1.1** A single workflow file `.github/workflows/ci.yml` named `CI` MUST run on `pull_request` and on `push` to `trunk`.
- **R1.2** The workflow MUST also run on a weekly `schedule` so that new security advisories are detected without a code change. Only the `deny` and `audit` jobs MUST run on scheduled events, and the other jobs MUST be skipped.
- **R1.3** The workflow MUST declare `permissions: contents: read` at the top level, and jobs MUST NOT request more unless a requirement below says so.
- **R1.4** The workflow MUST use a `concurrency` group keyed on the workflow and ref, with `cancel-in-progress: true` for pull requests only. Runs on `trunk` MUST NOT be cancelled.
- **R1.5** Every job MUST run on `ubuntu-latest` and MUST set `timeout-minutes` (15 by default, 30 for `coverage`).
- **R1.6** The workflow MUST set `CARGO_TERM_COLOR: always` and `CARGO_INCREMENTAL: 0` as environment variables.
- **R1.7** Every job MUST use the Rust version pinned in `rust-toolchain.toml`. The workflow MUST NOT hard-code a Rust version, so the toolchain file remains the single source of truth.
- **R1.8** Jobs MUST be independent and run in parallel. A single failing job MUST NOT hide the results of the others (`fail-fast` does not apply, because there is no matrix).
- **R1.9** Third-party actions MUST be referenced by a major-version tag (for example `actions/checkout@v4`). Dependabot keeps them current (R6.1).

## R2. Quality jobs

- **R2.1** Job `fmt` MUST run `cargo fmt --all --check`.
- **R2.2** Job `clippy` MUST run `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- **R2.3** Job `test` MUST run `cargo nextest run --workspace --locked`, and then `cargo test --workspace --doc --locked`, because nextest does not run doctests.
- **R2.4** Job `wasm` MUST run `cargo build -p rl-semaphore-web --target wasm32-unknown-unknown --locked`.
- **R2.5** All cargo invocations that read the lockfile MUST pass `--locked`, so a stale `Cargo.lock` fails CI.
- **R2.6** Rustc warnings MUST fail the build. This is done through `-D warnings` in the clippy step only, per the 0.1 convention that manifests do not `deny` warnings.

## R3. Supply-chain jobs

- **R3.1** A `deny.toml` MUST exist at the repository root with these sections configured:
  - `[advisories]`: vulnerabilities and unmaintained crates are errors, yanked crates are errors.
  - `[licenses]`: an explicit allowlist of permissive licenses (at least `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Unicode-3.0`, `Zlib`). Copyleft licenses are not allowed unless added deliberately in a later phase.
  - `[bans]`: `multiple-versions = "warn"` and `wildcards = "deny"`.
  - `[sources]`: `unknown-registry = "deny"` and `unknown-git = "deny"`, with only crates.io allowed.
- **R3.2** The allowlist SHOULD contain only licenses that appear in the current dependency tree plus those listed in R3.1. It SHOULD NOT be padded with licenses nothing uses.
- **R3.3** Workspace crates use `publish = false`, so `deny.toml` MUST NOT need a `[licenses.private]` exception for them. If cargo-deny reports them, the config MUST set `private = { ignore = true }`.
- **R3.4** Job `deny` MUST run `cargo deny --locked check` with all four checks (advisories, bans, licenses, sources).
- **R3.5** Job `audit` MUST run `cargo audit` against `Cargo.lock`, and it MUST fail on any vulnerability. Warnings for unmaintained or yanked crates MUST NOT fail the job.
- **R3.6** Ignoring an advisory in `deny.toml` or in `audit.toml` MUST come with a comment that gives the advisory ID, the reason, and a re-check date.

## R4. Coverage job

- **R4.1** Job `coverage` MUST run `cargo llvm-cov nextest --workspace --locked --lcov --output-path lcov.info`.
- **R4.2** The job MUST add the `llvm-tools-preview` component through `rustup component add`. It MUST NOT change `rust-toolchain.toml` for this.
- **R4.3** The job MUST upload `lcov.info` as a workflow artifact (retention 14 days), and MUST write a text summary (`cargo llvm-cov report --summary-only`) to `$GITHUB_STEP_SUMMARY`.
- **R4.4** The job MUST NOT enforce a coverage threshold in this phase. It only reports.
- **R4.5** The job MUST NOT send data to any external service.

## R5. Caching and tool installation

- **R5.1** Each job that compiles code MUST use `Swatinem/rust-cache@v2`. Jobs with different build profiles or targets MUST use different cache keys (`shared-key` or `key`), so `wasm` and `coverage` don't overwrite the cache of `clippy` and `test`.
- **R5.2** Cargo tools (`cargo-nextest`, `cargo-deny`, `cargo-audit`, `cargo-llvm-cov`) MUST be installed as prebuilt binaries with `taiki-e/install-action@v2`. They MUST NOT be built from source with `cargo install` in CI.
- **R5.3** The `wasm32-unknown-unknown` target and the `rustfmt` and `clippy` components come from `rust-toolchain.toml`. Jobs MUST NOT install them by hand.
- **R5.4** Jobs that don't compile code (`fmt`, `deny`, `audit`) SHOULD skip the cache action when it gives no benefit.

## R6. Repository hygiene

- **R6.1** `.github/dependabot.yml` MUST enable weekly updates for the `github-actions` ecosystem and the `cargo` ecosystem (directory `/`). Cargo updates SHOULD be grouped into one PR per week.
- **R6.2** Dependabot PRs MUST use the Conventional Commits prefix `chore(deps)` (`commit-message.prefix`).

## R7. Docs and roadmap

- **R7.1** The badge placeholder comment `<!-- badges: CI, license (added in 0.2) -->` in `README.md` MUST be replaced with two badges: the CI workflow status for `trunk`, and the license (`MIT OR Apache-2.0`). The position in the README stays the same (after the pitch, R6.3 of phase 0.1).
- **R7.2** Roadmap entry 0.2 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase.
- **R7.3** The README "Getting started" section MUST list the exact commands that CI runs (fmt, clippy, nextest, doc tests) so contributors can reproduce CI locally.
