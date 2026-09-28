# 1.7 Sim benchmark

**Stage:** 1 (Simulator, single intersection, no RL) · **Status:** ☑ done · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R6) · [plan.md](plan.md) (tasks).

## Goal

Put a number on how fast the simulator runs, and make it repeatable. At the end of this phase a `criterion` benchmark measures steps per second at two layers: the bare simulation step (spawn, controller, `Simulation::step`), and the full episode loop of `run_episode` (which adds the demand draw and the metrics). Each layer runs on three workloads built from the example scenario: no demand, the example demand, and a heavier demand that the fixed-time plan can still serve. CI runs every benchmark once so it cannot rot, the README records the numbers and the machine they came from, and `CONTRIBUTING.md` explains how to compare a change against a saved baseline. This closes Stage 1.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-crate-layout-and-compatibility) | Where the benchmark lives, the `criterion` dependency, nothing else changes |
| [R2](requirements.md#r2-workloads) | The three workloads (empty, example, heavy), seed, episode length, sanity checks |
| [R3](requirements.md#r3-benchmark-groups) | The `sim_step` and `episode` groups: what is timed, throughput in steps/s |
| [R4](requirements.md#r4-ci) | A smoke run in CI, without timing gates |
| [R5](requirements.md#r5-docs-and-roadmap) | README performance section, `CONTRIBUTING.md`, `tech-stack.md`, roadmap |
| [R6](requirements.md#r6-robustness) | Lints, determinism of the workload, no panics outside the bench |

## Decisions taken

- **Two layers, one bench file, in `env`.** The benchmark reports two groups: `sim_step`, which times only the simulation (spawning pre-drawn arrivals, the controller's command and `Simulation::step`), and `episode`, which times `run_episode` end to end (the demand draw, the same stepping, and `EpisodeMetrics::observe`). The gap between them is the cost of the demand and the metrics. Both groups live in `crates/env/benches/throughput.rs`. `sim` cannot depend on `env` (the dependency direction is strict), so a bench in `sim` would need its own copy of the fixed-time controller and of the workload builder. With both groups in `env`, they use the real `FixedTime` and one set of workloads, and the `sim_step` trajectory is exactly the one `run_episode` produces.
- **A demand sweep, not a single number.** Every group runs on three workloads that share the example's geometry, signal and fixed-time plan: `empty` (no demand, the fixed cost of a step), `example` (1,100 veh/h, the README baseline), and `heavy` (every flow × 1.3, 1,430 veh/h). The heavy factor is the highest tested load at which the fixed-time plan stays stable. At 1.3× the mean and max wait do not grow between 3,600 and 36,000 steps, while at 1.5× the queues grow without bound. A stable load keeps the number independent of the episode length.
- **Steps per second is the unit.** Each benchmark iteration is one episode of 3,600 steps (the `simulate` default) with seed 0, and it declares `Throughput::Elements(3600)`, so criterion reports steps/s directly. The setup (a fresh `Simulation`, and for `sim_step` the pre-drawn arrivals) is not timed.
- **Measure only.** This phase changes no simulation, environment or CLI code. Every snapshot stays byte-identical. Hotspots the benchmark reveals (for example the `Vec` that `Demand::arrivals` allocates every step, or the collapse in step rate under over-saturation) are written down for 11.1 (performance pass), not fixed here.
- **CI keeps the benchmark alive but does not judge it.** Shared runners are too noisy for timing gates. CI runs `cargo bench -- --test`, which executes every benchmark once (including the workload sanity checks) and fails if one breaks. Regressions are checked locally against a saved criterion baseline, and the procedure is written in `CONTRIBUTING.md`. No result history, no gh-pages, no new third-party action.
- **The numbers are recorded with their context.** The README gets a "Performance" section: the command, a table of steps/s per group and workload, and the CPU, OS, toolchain and date. The table only says what one machine measured, not what any machine will get.
- **No ADR.** The choices here are easy to reverse (a bench file, a CI step, a README section) and affect only `env`'s dev setup, so the decisions are recorded in this spec, as the ADR guide allows.

## Out of scope

- Any optimization of `sim`, `env` or the CLI, including allocation changes (11.1).
- Benchmarks of individual components (`Demand::arrivals`, `EpisodeMetrics::observe`, spawning on its own).
- Over-saturated workloads, and benchmarks of how step cost grows with queue length. The finding is recorded for 11.1.
- Timing thresholds, result history or regression alerts in CI.
- Changes to the release or bench profiles (LTO, codegen units, target CPU), so that bench numbers stay comparable with `simulate --release`.
- Benchmarks of the grid (Stage 6), the trainer or the server.

## Acceptance criteria

1. `cargo bench -p rl-semaphore-env --bench throughput` runs the six benchmarks `sim_step/{empty,example,heavy}` and `episode/{empty,example,heavy}` and reports each in elements (steps) per second. (R1, R3)
2. The workloads come from `configs/single-intersection.toml` with only the demand changed: none, the example's demand, and every flow × 1.3. Before measuring, each is checked to run 3,600 steps with no forced switch and no ignored command. (R2)
3. No source file in `sim`, `env` or `cli` changes. The `sim`, `env` and `cli` snapshots are byte-identical, and all existing tests pass. (R1)
4. CI has a job that runs `cargo bench -p rl-semaphore-env --bench throughput --locked -- --test`, and it is green. Clippy with `--all-targets -D warnings` passes with the bench file. (R4, R6)
5. The README has a "Performance" section with the command, the six numbers, the machine and the date. `CONTRIBUTING.md` explains how to compare against a saved baseline, `tech-stack.md` describes the benchmark as it is, and roadmap entry 1.7 is marked ☑. (R5)
