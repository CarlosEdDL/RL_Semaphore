# 0007. Fixed-time controller and the simulate command

**Status:** Accepted
**Date:** 2026-09-26

## Context

Phase 1.6 produces the first baseline number: a controller that cycles through the phases on a fixed timing plan, a loop that runs an episode with it, and a command that prints the metrics of ADR-0006. Several things had to be placed: where the timing plan lives, where the controller lives, what happens to a plan the signal cannot honour, and how the result leaves the process.

Alternatives considered:

- **The controller in `sim`, or only in the CLI.** In `sim` it would put policy in the pure simulator. In the CLI the actuated baseline (3.5), the RL environment (3.1) and the server (2.2) could not reuse it. `tech-stack.md` puts baseline controllers in `env`.
- **A uniform `--green-s` flag.** Simple, but it cannot express a plan that gives the main road more green, and the printed result would not say which plan produced it.
- **Webster timing computed from the demand.** It is a search for a good plan, and this phase records a baseline, not the best plan a human could write. It belongs to a later phase, if ever.
- **A separate controller config file.** Two files must then be kept consistent (one green per phase, in the phase order), and a scenario would no longer describe a run by itself.
- **A controller with its own timer.** It can drift from the signal, and it gives the wrong command if it is attached to a simulation that is already running.
- **Accepting plans the signal has to correct.** The signal would silently force switches, and the baseline number would measure the plan plus the corrections instead of the plan.
- **Serde on the `sim` types for the JSON output.** It would freeze the shape of the simulator types before the versioned `protocol` DTOs of 2.1.

## Decision

**The timing plan is scenario data, the controller is policy.** The green durations are an optional `[fixed_time]` table of the scenario TOML (`green_s`, one per phase, in phase order). `sim` validates it into a `FixedTimePlan` of whole steps, with the rounding of `min_green_s`. `Controller`, `FixedTime` and `run_episode` live in `env`. A scenario without the table still loads, and `simulate` refuses to run it.

**The cycle is validated against max-red at load time.** With `C = Σ (g[i] + yellow + all_red)`, phase `j` is not green for `C − g[j]` entries in each cycle, and its red age reaches that value on the entry before it turns green. The plan is valid when `C − g[j] ≤ max_red` for every phase (for two or more phases), so a valid plan is never overridden by the safety layer. The runner still counts forced switches and ignored commands, and the tests check that they stay at 0.

**A stateless controller that reads the signal state.** `FixedTime` holds while phase `p` has been green for fewer than `g[p]` entries and asks for the next phase in list order when it reaches `g[p]`. It holds during yellow and all-red. Phase `p` is green for exactly `g[p]` entries in a row, and the cycle is exactly `C` steps.

**One trait and one loop.** `Controller::command(&mut self, &Simulation) -> Command` is all the baselines need. `run_episode` runs, at each step: draw and spawn the arrivals, ask the controller for a command, step, and observe. The controller therefore sees the vehicles that arrived in this step. The CLI, the streaming server and the evaluation command reuse the loop instead of copying it.

**stdout is the result, stderr is the log.** `simulate` prints only its result on stdout, as a fixed-layout text table or, with `--output json`, as one JSON document with a `schema_version`. Logs, including the wall-clock time and steps per second, go to stderr through `tracing`. The JSON is built from serde structs private to the CLI. The output repeats the config path, seed, steps and plan, so a result says how to reproduce it.

## Consequences

### Positive

- The baseline measures the plan, because a valid plan is never corrected by the signal.
- A misconfigured plan fails at load time with a message naming the field, instead of showing up as odd numbers.
- The controller is independent of the clock and of how long the simulation has been running.
- The episode loop exists once, and it does no I/O, so the CLI, the server and the evaluation share it and the results are deterministic.
- The output is byte-identical for the same config, seed and steps, and it is pinned by snapshots.

### Negative

- The `sim` schema now carries a controller concern (`[fixed_time]`), although `sim` never uses it to simulate.
- The `Controller` trait may change in 3.1, when observations, action masks and rewards arrive.
- The CLI JSON schema is local to the CLI and has to be kept in sync by hand until the `protocol` crate of 2.1 replaces it (with a `schema_version` bump).
- Validation rejects some plans that would run with a correction from the signal, so a plan that is close to max-red has to be written as a valid one.
- The plan is uniform: there are no offsets, and phase 0 always starts green at step 0.
