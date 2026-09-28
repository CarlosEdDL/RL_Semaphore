# 1.7 Sim benchmark: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

Notation: `STEPS = 3600` is the episode length of every benchmark iteration and `SEED = 0` its seed. The *example* is `configs/single-intersection.toml`. A *workload* is a validated `Scenario` built from the example (R2).

## R1. Crate layout and compatibility

- **R1.1** The benchmark MUST live in `crates/env/benches/throughput.rs`, declared in `crates/env/Cargo.toml` as `[[bench]] name = "throughput"` with `harness = false`.
- **R1.2** `criterion` MUST be added to `[workspace.dependencies]` (version `0.8`) and used only as a dev-dependency of `env`. It SHOULD be declared with `default-features = false` and only the features the bench needs (at least `cargo_bench_support`), to keep `rayon` and `plotters` out of the dependency tree. No crate may gain a normal dependency.
- **R1.3** The `env` library target MUST set `bench = false` (`[lib] bench = false`), so that `cargo bench -p rl-semaphore-env -- <criterion flags>` does not pass criterion's flags to the libtest harness of the library.
- **R1.4** No source file under `crates/sim/src`, `crates/env/src` or `crates/cli/src` may change, and no public API may change. The step rule, the signal, demand generation, the metric definitions and the fixed-time controller stay as they are.
- **R1.5** Every existing snapshot (`crates/sim/tests/snapshots/`, `crates/cli/tests/snapshots/`) MUST stay byte-identical, every existing test MUST keep passing, and no existing assertion may be weakened or removed.
- **R1.6** The release and bench profiles MUST NOT change, so that bench numbers stay comparable with `simulate` built with `--release`.

## R2. Workloads

- **R2.1** The bench MUST build its workloads from the example, read at compile time (`include_str!` with a path from `CARGO_MANIFEST_DIR`), parsed with `ScenarioConfig::from_toml_str`, changed only in its `demand` field, and validated with `ScenarioConfig::validate`. Geometry, signal plan and fixed-time plan MUST be the example's.
- **R2.2** There MUST be exactly three workloads, named in the benchmark ids as:
  - `empty`: `demand` is `DemandConfig::default()` (no arrivals);
  - `example`: the example's demand unchanged (1,100 veh/h);
  - `heavy`: every approach's `veh_per_h` multiplied by `HEAVY_FACTOR = 1.3` (1,430 veh/h), turn ratios unchanged.
- **R2.3** `HEAVY_FACTOR` MUST be a named constant with a comment saying why it was chosen: the highest tested load at which the fixed-time plan is stable (mean and max wait do not grow from 3,600 to 36,000 steps), with the `simulate` numbers that show it, and the load at which it stops being stable.
- **R2.4** `STEPS` and `SEED` MUST be named constants. Every iteration of every benchmark MUST use them.
- **R2.5** Before a workload is measured, the bench MUST run `run_episode` on it once with `FixedTime`, outside the timed code, and panic with a message naming the workload unless `summary.steps == STEPS`, `signal.switches_forced == 0` and `signal.commands_ignored == 0`. For `empty` it MUST also check that no vehicle departed. These checks run in CI through the smoke run (R4).

## R3. Benchmark groups

- **R3.1** The bench MUST define two criterion groups, `sim_step` and `episode`, each with one benchmark per workload, with ids `sim_step/<workload>` and `episode/<workload>`.
- **R3.2** Both groups MUST set `Throughput::Elements(STEPS)`, so criterion reports steps per second.
- **R3.3** `sim_step` MUST time only the simulation. Per iteration, the setup (not timed, through `iter_batched`) builds a fresh `Simulation::new(scenario.clone())` and pre-draws the arrivals of all `STEPS` steps from `Demand::new(&scenario, SEED)`. The timed routine runs, for each step: spawn that step's arrivals, `FixedTime::command`, `Simulation::step`. The loop order MUST be that of `run_episode`, without the metrics, so the trajectory is the one `run_episode` produces for the same seed.
- **R3.4** `episode` MUST time `run_episode(&scenario, &mut controller, SEED, STEPS)` with a `FixedTime` built from the workload's plan. The controller MAY be built outside the timed code (it keeps no state between steps).
- **R3.5** Results that the optimizer could otherwise remove (step reports, the episode report) MUST go through `std::hint::black_box`.
- **R3.6** The module doc (`//!`) of the bench MUST state what each group includes and excludes (the `sim_step` group excludes the demand RNG and the metrics; `episode` includes both, plus building the `Simulation`), and that the gap between them is their cost.
- **R3.7** The bench SHOULD use criterion's default sample size and measurement time. A full `cargo bench -p rl-semaphore-env --bench throughput` SHOULD finish in under about two minutes on a desktop CPU.

## R4. CI

- **R4.1** `.github/workflows/ci.yml` MUST gain a `bench` job, skipped on the weekly `schedule` event like the other build jobs, with a 15-minute timeout, `rustup show`, `Swatinem/rust-cache@v2` with its own `shared-key: bench`, and the step `cargo bench -p rl-semaphore-env --bench throughput --locked -- --test`.
- **R4.2** CI MUST NOT compare timings, store results, or fail on a slowdown. No new third-party action may be added.
- **R4.3** The CI commands listed in the README under "Getting started" MUST include the smoke command of R4.1.

## R5. Docs and roadmap

- **R5.1** The README MUST gain a "Performance" section after "Baseline" with:
  - the command to run the benchmark;
  - a table with one row per workload (name and demand in veh/h) and one column per group, giving criterion's middle throughput estimate in steps/s, rounded to three significant figures (for example `1.68 M`);
  - the machine: CPU model, core count, OS (say so if it is WSL2), the toolchain from `rust-toolchain.toml`, and the date;
  - one sentence saying the numbers are for demand the plan can serve, and that the step rate falls sharply when queues grow without bound (recorded for 11.1).
- **R5.2** README "Project status" MUST say Stage 1 is done and the next phase is 2.1.
- **R5.3** `CONTRIBUTING.md` MUST gain a "Benchmarks" section covering how to run the benchmark; how to compare a change against a saved baseline (`-- --save-baseline <name>` before the change, `-- --baseline <name>` after); that a change to the step, demand, metrics or runner code SHOULD be benchmarked this way and the result stated in the PR; and that the README table is updated only when a change is meant to move it, with the machine it was measured on.
- **R5.4** The "Benchmarks" row of the practices table in `specs/tech-stack.md` MUST describe the benchmark as built: `criterion` in `crates/env/benches`, numbers in the README, a smoke run in CI, and regressions checked locally against a saved baseline.
- **R5.5** Roadmap entry 1.7 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.

## R6. Robustness

- **R6.1** The bench file MUST start with a `//!` doc and MAY allow `clippy::unwrap_used` and `clippy::expect_used` at the file level, as the test files do. If the functions generated by `criterion_group!` trigger `missing_docs`, it MUST be allowed at the narrowest scope that works, with a comment saying why.
- **R6.2** `cargo clippy --workspace --all-targets --locked -- -D warnings` MUST pass with the bench.
- **R6.3** The workloads MUST be deterministic: the fixed seed, and no clock, environment variable or file read at run time. Only the timings vary between runs.
- **R6.4** `cargo deny --locked check` and `cargo audit` MUST pass with the new dependency tree. A new duplicate-version warning is acceptable only if it comes from `criterion` and is noted in the PR.
