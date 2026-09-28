# 1.7 Sim benchmark: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feature/sim-benchmark`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Dependencies and manifest
- [x] Workspace: add `criterion = { version = "0.8", default-features = false, features = ["cargo_bench_support"] }` to `[workspace.dependencies]` ([R1.2](requirements.md#r1-crate-layout-and-compatibility)).
- [x] `crates/env/Cargo.toml`: `criterion.workspace = true` under `[dev-dependencies]`, `[lib] bench = false` (R1.3), and `[[bench]] name = "throughput"`, `harness = false` (R1.1).
- [x] Run `cargo deny --locked check` and `cargo audit` right away, before writing the bench, so a license or advisory problem shows up early ([R6.4](requirements.md#r6-robustness)).

### 2. Workloads
- [x] `crates/env/benches/throughput.rs`: constants `STEPS`, `SEED`, `HEAVY_FACTOR` with their comments ([R2.3–R2.4](requirements.md#r2-workloads)).
- [x] A `workloads() -> [(&'static str, Scenario); 3]` (or similar) that parses the example once, clones the config three times, sets or scales `demand`, and validates (R2.1–R2.2). Scale through the `Option<ApproachDemandConfig>` fields of `DemandConfig`.
- [x] The sanity check of R2.5 as a function called once per workload before its benchmarks are registered.

### 3. Benchmark groups
- [x] `sim_step` group: `Throughput::Elements(STEPS)`, `iter_batched` whose setup builds the `Simulation` and a `Vec<Vec<MovementId>>` of pre-drawn arrivals, and whose routine runs spawn → `FixedTime::command` → `step`, putting each report through `black_box` ([R3.2–R3.3](requirements.md#r3-benchmark-groups), R3.5).
- [x] `episode` group: `Throughput::Elements(STEPS)`, `iter` over `run_episode` with the result through `black_box` (R3.4–R3.5).
- [x] The module doc describing both groups (R3.6), and `criterion_group!` / `criterion_main!`. Fix any `missing_docs` warning as R6.1 says.

### 4. Measure
- [x] Run `cargo bench -p rl-semaphore-env --bench throughput` with the machine idle, twice, and check the two runs agree within a few percent.
- [x] Cross-check `episode/example` against the `steps_per_s` that `simulate --release` logs for 3,600 steps on the example. They measure the same loop, so they should be in the same range (about 1.7 M steps/s on the i7-12700 this spec was written on). A large gap means the bench is measuring something else.
- [x] Check `sim_step` ≥ `episode` for every workload, and `empty` ≥ `example` ≥ `heavy` within each group. If not, find out why before recording anything.
- [x] Check the full run time (R3.7).

### 5. CI
- [x] Add the `bench` job to `.github/workflows/ci.yml` ([R4.1](requirements.md#r4-ci)).
- [x] Add the smoke command to the CI command list in the README (R4.3).

### 6. Docs
- [x] README: the "Performance" section with the numbers from task 4 and the machine details ([R5.1](requirements.md#r5-docs-and-roadmap)), and "Project status" (R5.2).
- [x] `CONTRIBUTING.md`: the "Benchmarks" section (R5.3).
- [x] `specs/tech-stack.md`: the "Benchmarks" row (R5.4).

### 7. Roadmap
- [x] Mark 1.7 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R5.5).

### 8. Verify locally

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
cargo build -p rl-semaphore-web --target wasm32-unknown-unknown --locked
cargo deny --locked check
cargo audit

# the CI smoke run: every benchmark once, including the workload checks
cargo bench -p rl-semaphore-env --bench throughput --locked -- --test

# the real measurement
cargo bench -p rl-semaphore-env --bench throughput

# baseline workflow from CONTRIBUTING.md
cargo bench -p rl-semaphore-env --bench throughput -- --save-baseline before
cargo bench -p rl-semaphore-env --bench throughput -- --baseline before

# nothing outside the bench, manifests, CI and docs moved
git diff --stat -- crates/sim/src crates/env/src crates/cli/src   # empty
git diff --exit-code crates/sim/tests/snapshots/ crates/cli/tests/snapshots/
```

## Notes and risks

- **Why the heavy factor is 1.3.** Measured with `simulate` (fixed-time plan, seed 0) while writing this spec, with the example's flows scaled:

  | Factor | Demand | Mean wait at 3,600 / 36,000 steps | Max wait at 3,600 / 36,000 steps |
  |--------|--------|-----------------------------------|----------------------------------|
  | 1.0 | 1,100 veh/h | 29.8 s / 28.1 s | 152 s / 194 s |
  | 1.3 | 1,430 veh/h | 44.6 s / 39.6 s | 338 s / 338 s |
  | 1.4 | 1,540 veh/h | 54.2 s / 54.9 s | 364 s / 512 s (east grows) |
  | 1.5 | 1,650 veh/h | 68.3 s / 197.2 s | 495 s / 1,330 s |

  The side road (10 s of green in an 80 s cycle) saturates first. 1.3 is the last tested factor where nothing grows. Re-check these numbers when writing the `HEAVY_FACTOR` comment (R2.3).
- **Step cost grows with queues.** In the same runs, over-saturated demand made the step rate collapse: at 2× the example, 485 k steps/s over 3,600 steps but 57 k steps/s over 36,000, and at 3×, 4.6 k steps/s over 36,000. Something in the step, spawn or metrics path scales with the number of vehicles in the system, likely with the backlog. This phase does not investigate or fix it. Record it in the README sentence of R5.1 and bring it into 11.1. It matters before Stage 6 (grids) and Stage 10 (rush-hour demand), where queues can grow.
- **`criterion_group!` and `missing_docs`.** The macro generates a `pub fn` without docs, and the workspace sets `missing_docs = "warn"`, which `-D warnings` turns into an error for `--all-targets`. If it fires, allow it on the smallest scope that works (a file-level `#![allow(missing_docs)]` is acceptable if nothing narrower does), with a comment.
- **Criterion flags and libtest.** Without `[lib] bench = false`, `cargo bench -p rl-semaphore-env -- --save-baseline x` also runs the library's libtest harness, which rejects the unknown flag. Always pass `--bench throughput` in docs and CI anyway, so the command is unambiguous.
- **`--test` mode.** `cargo bench -- --test` runs every benchmark routine once, without measuring. The R2.5 checks run when the benchmarks are registered, so they run in CI too. Keep them outside the timed closures.
- **Setup out of the measurement.** In `sim_step`, pre-drawing arrivals and cloning the scenario must stay in the `iter_batched` setup, or the group measures the RNG and allocation it is meant to exclude. `episode` deliberately includes building the `Simulation`, because `run_episode` does it. At 3,600 steps that cost is small next to the stepping.
- **Noise.** WSL2, CPU frequency scaling and background load all move the numbers. Measure on an idle machine, run twice, and treat the README table as indicative. Criterion's change report (`--baseline`) on the same machine is the tool for regressions, not the README.
- **Dependency tree.** With default features off, `criterion` 0.8 still brings its own dependencies (for example `ciborium`, `itertools`, `oorandom`, `tinytemplate`). They are expected to be MIT or Apache-2.0, but run `cargo deny` before writing the bench (task 1). `multiple-versions = "warn"` in `deny.toml` means a duplicate does not fail CI. Mention any in the PR (R6.4).
- **Scope creep toward 11.1.** Do not remove the per-step `Vec` in `Demand::arrivals`, change containers in `Simulation`, or touch the profiles, even if the fix looks obvious. This PR records the baseline that such a change will be measured against.
