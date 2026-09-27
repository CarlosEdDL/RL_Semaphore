# 0003. Signal safety model

**Status:** Accepted
**Date:** 2026-09-26

## Context

The intersection needs a traffic signal that an RL agent controls. The agent must not be able to cause a collision (conflicting movements green together), skip clearance, or starve a phase forever. Safety should hold for any command sequence, not only for a well-behaved controller.

Alternatives considered:

- **Permitted (yielding) left turns.** More realistic capacity, but the conflict relation becomes dynamic (a left turn is compatible with oncoming traffic only if it yields), which needs gap acceptance in the vehicle model and makes the safety property harder to state and test.
- **A fixed, built-in phase plan.** The simplest option, but the plan could not follow the geometry in the config, and other intersections (Stage 6) would need code changes.
- **Enforcement only in the env layer** (an action mask in 3.3). The signal itself could then show unsafe states if another caller used it directly (the fixed-time controller, the web UI, tests), and the mask and the signal could drift apart.
- **Forcing the next phase in cycle order** when a deadline is near. Simple, but it skips the phase that has waited longest, and it breaks when the agent has chosen a non-cyclic order.

## Decision

Phases are defined in the scenario config as sets of movements (approach and turn), and only protected turns exist. `sim` validates every phase against a fixed, symmetric conflict matrix (14 conflicts per approach, 28 unordered pairs, derived by rotation from the relative position of the two approaches) and against the geometry. Every movement the lanes allow must be granted by some phase.

The `Signal` state machine enforces safety itself. It takes a command per step (hold or switch to phase *k*) and refuses switches before min-green or during a transition, inserts yellow and all-red clearance of exact lengths, and keeps movements that are green in both the old and the new phase green. `can_switch_to` uses the same decision code as `step`, so the env mask in 3.3 can query it instead of reimplementing the rules.

Max-red is enforced by an earliest-deadline-first rule. Each phase has a red age. A switch is accepted only if, after serving the requested phase for min-green, the other phases can still be served longest-red first, each within max-red. When holding one more step would make that schedule infeasible, the signal starts the switch to the longest-red phase itself (ties go to the lowest phase index) and reports it as forced. Since every phase needs the same minimum service, serving the longest-red phase first is optimal, and forcing exactly when the schedule becomes tight keeps it feasible by induction. Validation requires `max_red >= n * (yellow + all_red) + (n - 1) * min_green` steps for `n` phases, which is the shortest time in which a phase that leaves green can come back after every other phase has had its turn. (The first draft of the spec used `(n - 1) * (min_green + yellow + all_red)`, which is short by one clearance interval, so a plan could pass validation and still be impossible to serve.)

## Consequences

### Positive

- Conflicting movements are never shown together, for any command sequence, and this is checked with property tests.
- The safety rules live in one place, and the env mask, the fixed-time controller and the UI all see the same behaviour.
- Phases follow the config, so other intersections only need a new file.
- No phase can wait longer than max-red, so a learned policy cannot starve a side road.

### Negative

- Protected-only turns give less realistic left-turn capacity than permitted turns.
- A side road with one shared lane needs split phases (one per side), because its left turns conflict with the opposing through traffic.
- Max-red may override the agent, so the agent's action is not always the one applied. The outcome reports when this happens.
- Validation rejects plans whose max-red is shorter than the feasibility bound, even if a specific command sequence would have been fine.
