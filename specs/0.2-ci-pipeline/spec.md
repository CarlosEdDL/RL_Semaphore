# 0.2 CI pipeline

**Stage:** 0 (Foundations) · **Status:** ☑ done · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R7) · [plan.md](plan.md) (tasks).

## Goal

Make every later PR prove itself automatically. At the end of this phase a GitHub Actions workflow runs on every pull request and every push to `trunk`, checks formatting, lints, tests, the WASM build, supply-chain policy and coverage, and the README shows live status badges. A red check means the change is not mergeable.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-workflow-triggers-and-structure) | Triggers, permissions, concurrency, toolchain |
| [R2](requirements.md#r2-quality-jobs) | fmt, clippy, tests (nextest + doctests), WASM build |
| [R3](requirements.md#r3-supply-chain-jobs) | cargo-deny policy and cargo-audit |
| [R4](requirements.md#r4-coverage-job) | cargo-llvm-cov report |
| [R5](requirements.md#r5-caching-and-tool-installation) | Caching and tool installation |
| [R6](requirements.md#r6-repository-hygiene) | Dependabot |
| [R7](requirements.md#r7-docs-and-roadmap) | README badges, roadmap update |

## Decisions taken

- **Jobs:** fmt, clippy, test, WASM build, cargo-deny, cargo-audit, coverage.
- **OS matrix:** `ubuntu-latest` only. The deployment target is a Linux image.
- **cargo-deny:** full policy (licenses, advisories, bans, sources).
- **Triggers:** pull requests and pushes to `trunk`, with concurrency cancellation. Badges and Dependabot are included.

## Out of scope

- Docker image build in CI (phase 2.6).
- Release and deploy workflows (phases 11.3, 11.4).
- Benchmark regression tracking (phase 1.7).
- `tracing`, error conventions, `CONTRIBUTING.md`, ADRs (phase 0.3).
- Coverage thresholds and third-party coverage services (Codecov and similar). Coverage is only reported in this phase.
- Configuring GitHub branch protection. It is a repository setting done by the author, and is listed in the plan as a manual step.

## Acceptance criteria

1. `.github/workflows/ci.yml` exists and is valid. It triggers on `pull_request` and on `push` to `trunk`. (R1.1)
2. A PR that opens against `trunk` runs all seven jobs (`fmt`, `clippy`, `test`, `wasm`, `deny`, `audit`, `coverage`), and all pass on the current code. (R2–R4)
3. A deliberate formatting error makes `fmt` fail. A deliberate clippy warning makes `clippy` fail. A deliberate failing test makes `test` fail. Checked once on a throwaway branch and not merged. (R2.1–R2.3)
4. Pushing a second commit to a PR cancels the still-running run of the first commit. (R1.4)
5. Second run of an unchanged workspace restores the Cargo cache (visible in the `Swatinem/rust-cache` log). (R5.1)
6. `deny.toml` exists, and `cargo deny check` passes locally and in CI. (R3.1–R3.3)
7. The coverage job uploads an `lcov.info` artifact and writes a summary to the job summary page. (R4.1–R4.3)
8. `.github/dependabot.yml` exists and covers `github-actions` and `cargo`. (R6.1)
9. `README.md` shows the CI and license badges in place of the placeholder comment. (R7.1)
10. Roadmap entry 0.2 is marked ☑. (R7.2)
