# 1.3 Vehicles & movement

**Stage:** 1 (Simulator, single intersection, no RL) · **Status:** ☑ done · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R7) · [plan.md](plan.md) (tasks).

## Goal

Put vehicles on the road. At the end of this phase the `sim` crate has a `Simulation` that owns a validated scenario, its `Signal`, and the vehicles on every lane. A caller spawns vehicles by approach and movement, and each `step(command)` advances the signal and then every vehicle by one fixed time step: vehicles move forward one cell at a time, queue behind each other and at a red light, and cross the stop line (leaving the model) only when their movement is green. Property tests show that, for any sequence of spawns and signal commands, vehicles are conserved, never share a cell, never overtake each other in a lane, and only cross on green.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-dependencies-and-crate-layout) | No new dependencies, new modules and exports |
| [R2](requirements.md#r2-vehicles-and-spawning) | `VehicleId`, `Vehicle`, spawning, lane choice, per-lane backlog |
| [R3](requirements.md#r3-the-step-rule) | The step rule: signal first, then a parallel one-cell move, crossing on green |
| [R4](requirements.md#r4-observation-api) | Read-only access to vehicles, lanes, backlogs, lights, and step reports |
| [R5](requirements.md#r5-determinism-and-robustness) | Determinism, no panics, clippy pedantic |
| [R6](requirements.md#r6-tests) | Hand-traced unit tests and property tests |
| [R7](requirements.md#r7-adr-and-roadmap) | ADR-0004 and roadmap |

## Decisions taken

- **One cell per step, parallel update.** A vehicle moves at most one cell per step, and only into a cell that was empty at the *start* of the step. All vehicles decide from the same snapshot, so a stopped queue starts moving one vehicle at a time: a queue at a green light discharges one vehicle every 2 steps (about 1,800 veh/h/lane with `step_s = 1.0`, a realistic saturation flow). With the example `cell_length_m = 7.5` and `step_s = 1.0`, free-flow speed is 7.5 m/s (27 km/h). There are no speed states, no new config keys, and no randomness.
- **Spawning goes through a per-lane backlog.** `spawn(approach, movement)` creates the vehicle immediately and puts it at the back of a FIFO backlog outside its lane. The front of the backlog enters cell 0 when that cell is free. A vehicle that cannot enter still exists, so it counts for conservation, and 1.5 can charge it waiting time. Spillback is visible instead of silently dropping demand.
- **The simulation picks the lane.** Among the lanes of the approach that allow the movement, the vehicle goes to the one with the fewest vehicles (on the lane plus in its backlog), with ties going to the lowest lane index. The lane is fixed for the vehicle's life: there are no lane changes.
- **Crossing on green only, and instant exit.** The vehicle in a lane's stop-line cell crosses only when its own movement shows `Green`. Yellow is treated like red for crossing, so no vehicle enters on yellow and the 1.2 clearance intervals are never needed to clear the box. A vehicle that crosses leaves the model in that step (ADR-0002: exits are not simulated space in Stage 1).
- **Lanes are strict FIFO.** A vehicle waiting at the stop line blocks every vehicle behind it, even if their movements are green. With the example config this never happens (movements that share a lane are always green together), but it is the defined behaviour for shared lanes whose movements are split across phases.
- **The signal steps first.** Within one step, the signal takes the command and updates its lights, and then vehicles move under those new lights. So the lights reported after a step are exactly the lights under which that step's crossings happened.
- **`spawn` is the seam for demand.** 1.3 has no arrival process. Tests spawn vehicles explicitly. Seeded Poisson demand in 1.4 will call the same `spawn`.

## Out of scope

- Arrival processes, turn ratios, and randomness (1.4).
- Wait times, delay, throughput, and queue-length metrics (1.5). This phase only records when each vehicle was spawned and when it crossed.
- The fixed-time controller and the `simulate` CLI (1.6), and benchmarks (1.7).
- Multi-cell speeds, acceleration, random slowdown, and lane changes.
- Vehicles occupying the intersection box, exit lanes, and routes beyond the stop line (6.x).
- Permitted turns, crossing on yellow, and right-turn-on-red.
- A cap on backlog length.

## Acceptance criteria

1. `Simulation::new` builds from the example scenario with empty lanes and backlogs, the signal in its initial state, and step count 0. (R1, R4)
2. `spawn` assigns increasing `VehicleId`s, picks the lane by the fewest-vehicles rule with the lowest-index tie-break, and returns an error (no panic) for a movement that no lane of the approach allows. (R2)
3. Hand-traced unit tests pin the step rule: a lone vehicle on an empty lane of `n` cells under constant green crosses on step `n + 1`; a queue at a green light discharges one vehicle every 2 steps; vehicles stop at a red light and fill the lane from the stop line back; a full lane spills into the backlog; a head vehicle on red in a shared lane blocks a green vehicle behind it. (R3, R6.1)
4. A `proptest` over arbitrary spawn and command sequences, for at least 1,000 steps, shows after every step: spawned = in backlog + on lane + departed; no two vehicles share a cell; every vehicle moves 0 or 1 cells forward and stays in its lane; no vehicle overtakes another in the same lane; every crossing happens from the stop-line cell with the vehicle's movement shown green; no two crossings in the same step conflict. (R6.2)
5. The simulation is deterministic (two runs of the same script give identical vehicle states and reports), uses no `HashMap` iteration, randomness, or wall-clock time, never panics, and passes clippy pedantic. (R5)
6. `specs/adr/0004-vehicle-update-rule.md` exists with status `Accepted`, and the ADR index lists it. (R7)
7. CI is green, and roadmap entry 1.3 is marked ☑. (R7)
