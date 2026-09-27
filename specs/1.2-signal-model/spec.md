# 1.2 Signal model

**Stage:** 1 (Simulator, single intersection, no RL) · **Status:** ☐ not started · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R8) · [plan.md](plan.md) (tasks).

## Goal

Give the intersection a traffic signal that is safe by construction. At the end of this phase the `sim` crate knows which movements conflict, can load a signal plan (phases plus timings) from the same TOML file as the geometry, validate it against the intersection, and run it as a step-by-step state machine. The state machine takes a command each step ("hold" or "switch to phase *k*") and enforces min-green, yellow, all-red clearance, and **max-red** itself: it ignores switches that are too early or that would make some phase miss its max-red deadline, inserts the clearance intervals, and forces a switch when a deadline is about to be missed. Property tests show that, for any sequence of commands, conflicting movements are never shown together, clearance intervals have exact lengths, and no movement stays red longer than max-red.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-dependencies) | `proptest` as a workspace dev-dependency |
| [R2](requirements.md#r2-movements-and-the-conflict-matrix) | `MovementId` and the protected-only conflict matrix |
| [R3](requirements.md#r3-scenario-config-and-time) | Scenario config (`step_s`, `[intersection]`, `[signal]`) and seconds-to-steps conversion |
| [R4](requirements.md#r4-signal-plan-validation) | Phase and timing validation, new `ConfigError` variants |
| [R5](requirements.md#r5-signal-state-machine) | `Signal` state machine: commands, transitions, lights |
| [R6](requirements.md#r6-max-red-enforcement) | Max-red deadlines, forced switches, and the admission check |
| [R7](requirements.md#r7-example-config-and-tests) | Example config update, unit tests, property tests |
| [R8](requirements.md#r8-adr-docs-and-roadmap) | ADR-0003, README, roadmap |

## Decisions taken

- **Phases are defined in config.** A phase is a named set of movements (approach + turn) that are green together. `sim` validates every phase against a built-in conflict matrix and against the geometry, so a phase can only grant movements that exist and do not conflict.
- **Protected turns only.** A left turn conflicts with opposing through and right traffic, so it is only green when those are red. There is no "permitted, yield to oncoming" state; this keeps the matrix static and safety easy to test, and 1.3 needs no yield logic.
- **Time: seconds in config, integer steps in `sim`.** The scenario declares `step_s` (the simulation time step). Signal timings are written in seconds and converted to whole steps once, at load time, like meters to cells in 1.1. Clearance and min-green round **up** (never shorter than configured), and max-red rounds **down** (never longer).
- **The signal enforces its own safety.** The state machine refuses early or unsafe switches and forces switches on max-red deadlines. The env safety mask in 3.3 mirrors this through a query (`can_switch_to`) instead of reimplementing it, so the two cannot drift apart.
- **Max-red is guaranteed at the phase level, checked at the movement level.** Each phase has a red age (steps since it was last green). Switches are admitted only if every other phase can still be served before its deadline in longest-red-first order (earliest-deadline-first). When the schedule becomes tight, the signal forces a switch to the longest-red phase (ties go to the lowest phase index). A validation rule makes the deadline always reachable: `max_red ≥ (phases − 1) × (min_green + yellow + all_red)`, in steps.
- **Every movement must be served.** Validation rejects a plan in which some movement present in the geometry is never green in any phase; otherwise max-red could not hold.
- **One scenario file.** `configs/single-intersection.toml` gains a top-level `step_s` and a `[signal]` table. The road geometry moves under `[intersection]`, because serde's `flatten` does not work with `deny_unknown_fields`. `IntersectionConfig` itself is unchanged and still parses on its own.
- **Movements that are green in both the old and new phase stay green** through the transition. Only movements that lose green go yellow, then red. This is safe because both phases are conflict-free, and it avoids needless stops.

## Out of scope

- Vehicles reacting to lights, lane-level "can this vehicle go" logic, and shared-lane head-of-queue blocking (1.3).
- Permitted (yielding) left turns, right-turn-on-red, pedestrian phases, and overlaps defined separately from phases.
- Per-phase timings (min-green per phase, max-green). Timings are global in this phase.
- Fixed-time cycle controllers and the CLI (1.6). This phase only tests the state machine with command sequences.
- The env action mask and observation (3.x). This phase only exposes the query the mask will use.
- Multiple intersections or offsets between signals (6.x).

## Acceptance criteria

1. The conflict relation between the 12 movements is symmetric, never marks two movements of the same approach as conflicting, has exactly the 26 unordered conflicting pairs listed in R2.4, and a test checks all 144 ordered pairs against that table. (R2)
2. `configs/single-intersection.toml` loads into a validated scenario with `step_s`, the intersection, and a 4-phase signal plan, and its timings convert to the expected step counts. (R3, R7.1)
3. Every validation rule in R4 has a failing config test that returns the matching `ConfigError` variant with the field path in its `Display`. (R4, R7.2)
4. Unit tests cover min-green refusal, exact yellow and all-red lengths, movements kept green across a transition, commands ignored during a transition, a forced switch on a max-red deadline, and the tie-break rule. (R5, R6, R7.2)
5. A `proptest` over arbitrary command sequences (and generated valid timings) shows: no two conflicting movements are ever non-red at the same step; every yellow lasts exactly `yellow` steps and every all-red exactly `all_red` steps; every green lasts at least `min_green` steps; no movement is red for more than `max_red` consecutive steps; and `can_switch_to(k)` is true exactly when `SwitchTo(k)` would be accepted. (R6, R7.3)
6. Loading and running the signal is deterministic: no `HashMap`, no randomness, no wall-clock time, and no panics on any config or command. Clippy pedantic passes. (R5.7)
7. `specs/adr/0003-signal-safety-model.md` exists with status `Accepted`, and the ADR index lists it. (R8)
8. CI is green, and roadmap entry 1.2 is marked ☑. (R8)
