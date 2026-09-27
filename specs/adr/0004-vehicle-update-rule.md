# 0004. Vehicle update rule

**Status:** Accepted
**Date:** 2026-09-26

## Context

Vehicles must move, queue at red and cross on green in a fixed time step, and the same rule must be cheap enough for the training loop (Stage 3+) and simple enough to check with property tests. ADR-0002 already fixed the space model (fixed-length cells, exits not simulated in Stage 1).

Alternatives considered:

- **Nagel–Schreckenberg with multi-cell speeds, with random slowdown.** Reproduces stop-and-go waves, but adds speed state, tunable parameters and randomness inside the simulator, which makes exact hand-traced tests and reproducible runs harder.
- **Nagel–Schreckenberg without random slowdown.** Deterministic, but still carries speed state, acceleration and braking distances that 1.3 does not need.
- **Sequential front-to-back update.** A vehicle may move into a cell vacated earlier in the same step, so a whole queue starts at once. That gives a 1-step headway (about 3,600 veh/h/lane), which is unrealistic, and the result depends on processing order.
- **Crossing on yellow.** More realistic, but the box would need clearing logic and the 1.2 clearance intervals would no longer bound when a crossing can happen.
- **Vehicles occupying the intersection box.** Needed for turning conflicts inside the box, but that is Stage 6 work (ADR-0002 leaves exits and the box out of Stage 1).
- **Rejecting spawns onto a full entry cell.** Demand would be silently dropped, and the spillback would be invisible to metrics.

## Decision

Time is discrete and a vehicle moves at most one cell per step, in a parallel update: every decision depends only on the state at the start of the step and on the lights after the signal update. Order within a step is: signal, crossing, advance, entry.

The vehicle in a lane's stop-line cell crosses, and leaves the model at once, only when its movement is `Green` (yellow counts as red). Lanes are strictly FIFO: a vehicle at the stop line blocks those behind it, even if their movements are green.

`spawn` creates the vehicle at once and puts it in a per-lane FIFO backlog outside the lane; the front of the backlog enters cell 0 when it is free. The simulation chooses the lane: among the lanes of the approach that allow the movement, the one with the fewest vehicles (on the lane plus in its backlog), ties to the lowest index. There are no lane changes.

## Consequences

### Positive

- The rule is deterministic and has no randomness, so runs are reproducible and hand-traced tests pin exact step counts (a lone vehicle crosses at `n + 1`).
- A stopped queue discharges one vehicle every 2 steps, about 1,800 veh/h/lane with `step_s = 1.0`, a realistic saturation flow, without speed state.
- Every step is `O(cells + vehicles)` and independent of processing order.
- Conservation, no overlap, no overtaking and crossing only on green are simple to state and to check as property tests.
- Backlogs make spillback visible and let 1.5 charge waiting time to vehicles that cannot enter yet.
- Crossing only on green means the 1.2 clearance intervals never need to clear the box.

### Negative

- Speed is quantized: free-flow speed is `cell_length_m / step_s` (7.5 m/s with the example) and there is no acceleration or braking.
- The 2-step headway is fixed by the model, not configured.
- Speed is tied to `cell_length_m` and `step_s`, so changing one changes traffic behaviour.
- Backlogs are unbounded, so heavy demand under a long red grows memory; a cap belongs with demand in 1.4 if needed.
- No permitted turns, crossing on yellow, or right-turn-on-red.
