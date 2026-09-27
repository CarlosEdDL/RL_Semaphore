# 1.6 Fixed-time controller + CLI

**Stage:** 1 (Simulator, single intersection, no RL) · **Status:** ☑ done · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R9) · [plan.md](plan.md) (tasks).

## Goal

Produce the first baseline number. At the end of this phase a scenario can carry a fixed-time timing plan (one green duration per phase), the `env` crate has a minimal `Controller` trait, a `FixedTime` controller that cycles through the phases on that plan, and a `run_episode` function that drives the demand → command → step → observe loop and returns the 1.5 `EpisodeSummary`. `rl-semaphore simulate --config <PATH>` runs it for N steps with a seed and prints the metrics on stdout, as a readable table or as JSON. The example scenario gets a timing plan, an `insta` snapshot pins the command's output, and the README records the result: the number every later controller must beat on mean **and** tail wait.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-crate-layout-and-compatibility) | Where the code lives, dependencies, 1.1–1.5 behaviour unchanged |
| [R2](requirements.md#r2-fixed-time-plan-in-the-scenario) | The optional `[fixed_time]` table: schema, conversion to steps, validation against max-red |
| [R3](requirements.md#r3-the-controller-trait-and-fixedtime) | `Controller` trait and the `FixedTime` controller |
| [R4](requirements.md#r4-the-episode-runner) | `run_episode`: loop order, result, signal counters, errors |
| [R5](requirements.md#r5-the-simulate-command) | `simulate` arguments, exit codes, stdout vs stderr |
| [R6](requirements.md#r6-output-formats) | Text table and `--output json` |
| [R7](requirements.md#r7-determinism-and-robustness) | Determinism, no panics, clippy pedantic, WASM build |
| [R8](requirements.md#r8-tests) | Plan validation, cycle timing, no forced switches, runner, CLI and snapshots |
| [R9](requirements.md#r9-docs-adr-and-roadmap) | Example config, README baseline, ADR-0007, roadmap |

## Decisions taken

- **The timing plan is scenario data, the controller is policy.** The green durations live in a new optional `[fixed_time]` table of the scenario TOML (`green_s = [...]`, one entry per phase, in phase order) and are validated by `sim` into a `FixedTimePlan` of whole steps, like the signal timings and the demand. The controller that *uses* them lives in `env`, next to where the actuated baseline (3.5) and the RL environment (3.1) will go, as `tech-stack.md` requires. `ScenarioConfig` rejects unknown keys, so the table must be part of the sim schema anyway; a scenario without it still loads, but `simulate` refuses to run it.
- **The cycle is checked against max-red at load time.** A fixed-time plan whose cycle would keep a phase red longer than `max_red` is a config error, not something the signal silently fixes by forcing switches. So a valid plan runs exactly as written: the baseline never has its timing overridden by the safety layer, and the number it produces measures the plan, not the plan plus corrections. The runner still counts forced switches and ignored commands, and the tests check both stay at 0.
- **The controller has no clock of its own.** `FixedTime` reads the signal state each step: while phase `p` is green for fewer than `green[p]` entries it holds, and when it reaches `green[p]` it asks for the next phase in list order. During yellow and all-red it holds. Phase `p` is therefore green for exactly `green[p]` entries in a row, the cycle is `Σ (green[i] + yellow + all_red)` steps, and the controller gives the right command even if it is attached to a simulation that is already running. There are no offsets; phase 0 starts green at step 0, as the signal already does.
- **A minimal `Controller` trait now, the Env trait later.** `Controller::command(&mut self, sim: &Simulation) -> Command` is all the fixed-time and actuated baselines need. 3.1 may wrap or extend it, but this phase does not anticipate observations, action masks or rewards.
- **One runner for everyone.** `env::run_episode(scenario, controller, seed, steps)` builds the `Demand`, `Simulation` and `EpisodeMetrics`, and runs, at every step: draw and spawn arrivals, ask the controller for a command, step, observe. This is the loop order of 1.4 and 1.5 with the controller inserted after spawning, so the controller sees the vehicles that arrived this step. The CLI, 2.2's streaming server and 3.5's `eval` all reuse it instead of copying the loop.
- **stdout is the result, stderr is the log.** `simulate` prints only its result on stdout and logs through `tracing` on stderr (including the wall-clock run time and steps/s, which never appear in the result). The default output is a fixed-layout text table; `--output json` prints one JSON document with a `schema_version`. The JSON is built from serde structs private to the CLI, so the `sim` types stay serde-free until the versioned `protocol` DTOs of 2.1.
- **Explicit, reproducible arguments.** `simulate --config <PATH> [--steps 3600] [--seed 0] [--output text|json]`. `--config` is required: no hidden default scenario. The output repeats the config path, seed, steps and timing plan, so a printed result says how to reproduce it.

## Out of scope

- The actuated (queue-threshold) controller and the multi-seed `eval` command (3.5).
- The Gymnasium-like `Env` trait, observations, action masks and rewards (3.1–3.4).
- Signal offsets, green waves and coordination (6.5).
- Automatic timing (Webster or any demand-based split), and any timing search.
- Warm-up periods or discarding the start of an episode.
- Serde on `sim` types and the `protocol` DTOs (2.1). The CLI's JSON schema is its own.
- The `criterion` benchmark (1.7). The steps/s log line is informative only.
- Any change to the step rule, the signal, demand generation or the metric definitions.

## Acceptance criteria

1. A scenario may contain `[fixed_time] green_s = [...]`. Validation rejects a list whose length is not the number of phases, a green shorter than min-green, and a cycle that would keep any phase red longer than max-red. `Scenario::to_config` round-trips the table, and a scenario without it still loads. (R2)
2. `FixedTime` holds each phase green for exactly its configured number of steps, cycles through the phases in list order, and over several cycles on the example never causes a `SwitchForced` or an `Ignored` outcome. (R3, R8)
3. `run_episode` runs the demand → command → step → observe loop for `steps` steps and returns the `EpisodeSummary` and the signal counters. Same scenario, controller and seed give an identical result. (R4, R7)
4. `rl-semaphore simulate --config configs/single-intersection.toml` exits 0 and prints the table: config, seed, steps, the timing plan, departures, in-system and throughput, wait stats (overall and per approach), queue stats (per lane, per approach, total) and the signal counters. `--output json` prints the same numbers as one JSON document. Logs never reach stdout. (R5, R6)
5. A missing or invalid config, or a scenario without `[fixed_time]`, exits 1 with a message naming the problem. Invalid arguments (such as `--steps 0`) exit 2. (R5)
6. `insta` snapshots pin the text and JSON output of the example (seed 0, 3600 steps). The 1.4 and 1.5 snapshots are unchanged. (R1, R8)
7. The README shows the command and the example's baseline numbers. `specs/adr/0007-fixed-time-controller.md` exists with status `Accepted`, the ADR index lists it, CI is green, and roadmap entry 1.6 is marked ☑. (R9)
