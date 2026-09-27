# 1.3 Vehicles & movement: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

Notation: `t` is the step count (0 before the first `step`). `C(t)` is the occupancy of every lane cell at `t`, `B(t)` the backlogs, and `L(t)` the signal's `lights()` at `t`. For a lane, `last` is its `stop_line_cell()`.

## R1. Dependencies and crate layout

- **R1.1** No new dependency is needed. `sim` MUST still have no `anyhow`, I/O, async, or randomness crate, and `Cargo.lock` MUST NOT gain new packages.
- **R1.2** Vehicle types SHOULD live in a new `vehicle` module and the `Simulation` in a new `simulation` module. New public types MUST be re-exported from the crate root, as in 1.1 and 1.2.
- **R1.3** The scenario schema and `configs/single-intersection.toml` MUST NOT change. The 1.1 and 1.2 tests MUST keep passing unchanged.

## R2. Vehicles and spawning

- **R2.1** `VehicleId` MUST be a `Copy + Eq + Ord + Hash + Debug` newtype over `u64`, with `Display`. Ids MUST be assigned from 0 in spawn order and never reused within a `Simulation`.
- **R2.2** A `Vehicle` MUST expose read-only: its id, its `MovementId` (approach and movement), its `LaneId`, the step at which it was spawned, and its position: either in the backlog of its lane or on its lane at a cell index. The step count MUST be `u64`.
- **R2.3** `Simulation::spawn(approach: Direction, movement: Movement) -> Result<VehicleId, SpawnError>` MUST create the vehicle at the current step `t` (so its spawn step is `t`) and push it to the back of its lane's backlog. It MUST NOT place the vehicle on the lane directly; entry happens in `step` (R3.4).
- **R2.4** Lane choice: among the lanes of `approach` whose movements include `movement`, the vehicle MUST go to the lane with the fewest vehicles, counting vehicles on the lane and in its backlog at the moment of the call (so vehicles spawned earlier in the same step count). Ties MUST go to the lowest lane index. The lane MUST NOT change afterwards.
- **R2.5** If no lane of `approach` allows `movement`, `spawn` MUST return `SpawnError::NoLaneForMovement` naming the `MovementId`, and the simulation MUST be unchanged (no id is consumed). `SpawnError` MUST be a `thiserror` enum marked `#[non_exhaustive]`.
- **R2.6** Backlogs are unbounded in this phase.

## R3. The step rule

`Simulation::step(&mut self, command: Command) -> StepReport` advances from `t` to `t + 1` in this order:

- **R3.1** **Signal.** The signal steps with `command`, giving `L(t + 1)`. The `StepOutcome` MUST be returned in the report.
- **R3.2** **Crossing.** For each lane, if a vehicle is in cell `last` in `C(t)` and its movement is `Green` in `L(t + 1)`, it crosses: it is removed from the simulation and recorded as a departure. `Yellow` and `Red` MUST NOT allow crossing.
- **R3.3** **Advance.** Every vehicle on a lane in cell `i < last` MUST move to `i + 1` if and only if cell `i + 1` is empty in `C(t)`. A cell emptied during this step (by a crossing or a move) MUST NOT be filled in the same step.
- **R3.4** **Entry.** For each lane, the front vehicle of the backlog MUST enter cell 0 if and only if cell 0 is empty in `C(t)`. At most one vehicle enters a lane per step.
- **R3.5** Then `t` becomes `t + 1`. Every decision in R3.2–R3.4 MUST depend only on `C(t)`, `B(t)`, and `L(t + 1)`, never on the order in which lanes or vehicles are processed.
- **R3.6** Consequences that MUST hold (and are tested in R6): a vehicle spawned at `t` onto an empty lane of `n` cells, with its movement green throughout, enters cell 0 at `t + 1`, reaches cell `last` at `t + n`, and crosses at `t + n + 1`. A queue standing at a green light discharges exactly one vehicle every 2 steps.
- **R3.7** `StepReport` MUST contain the signal `StepOutcome` and the departures of the step, ordered by lane (approaches in `Direction::ALL` order, then lane index). A departure MUST record the vehicle id, its `MovementId`, its `LaneId`, its spawn step, and its departure step (`t + 1`).
- **R3.8** The step rule MUST be documented in rustdoc on `Simulation::step`, including the parallel-update rule and the 2-step discharge headway.

## R4. Observation API

- **R4.1** `Simulation::new(scenario: Scenario)` (or `&Scenario`) MUST build a simulation with empty lanes and backlogs, a `Signal` in its initial state, and `t = 0`.
- **R4.2** The simulation MUST expose at least: the scenario, the signal (read-only), `lights()`, the step count, all vehicles in `VehicleId` order, a lookup by `VehicleId`, the occupancy of a lane as a slice or iterator of `Option<VehicleId>` indexed by cell, and each lane's backlog in FIFO order.
- **R4.3** It MUST expose counters for vehicles spawned, currently in backlogs, currently on lanes, and departed in total, so conservation can be checked without recomputing.
- **R4.4** Nothing in the API may let a caller move a vehicle, change a lane, or change the signal except through `spawn` and `step`.

## R5. Determinism and robustness

- **R5.1** `sim` MUST stay deterministic: no `HashMap`/`HashSet` iteration where order is observable, no randomness, no wall-clock time. Per-lane state SHOULD be indexed by `LaneId` in `Direction::ALL`, then lane-index order.
- **R5.2** Neither `spawn` nor `step` may panic on any input. Arithmetic on ids and step counts MUST NOT overflow in practice (`u64`); casts follow the 1.1 rule: range-check first, then a local `#[allow(clippy::cast_*)]` with an `// INVARIANT:` comment.
- **R5.3** `step` SHOULD cost `O(cells + vehicles)` and allocate at most the departures vector.
- **R5.4** Clippy pedantic MUST pass. `Simulation` SHOULD be `Clone` so tests can run the same script twice.

## R6. Tests

- **R6.1** Unit tests (in `crates/sim/tests/`) MUST cover:
  - building from the example scenario (through `include_str!`): empty lanes and backlogs, `t = 0`;
  - id assignment and the lane-choice rule, including the tie-break, using an inline scenario with two lanes that allow the same movement;
  - `SpawnError::NoLaneForMovement` using an inline scenario where some movement has no lane, with the simulation unchanged afterwards;
  - the lone-vehicle timing of R3.6 on the example's north approach (20 cells, crossing at step 21);
  - the 2-step discharge headway of R3.6;
  - vehicles stopping at red and filling the lane from `last` backwards, then spilling into the backlog, then entering the lane as it frees up;
  - no crossing on yellow;
  - head-of-line blocking in a shared lane, using an inline scenario where a lane allows two movements that are green in different phases;
  - determinism: the same spawn and command script run twice gives identical vehicles and reports.
- **R6.2** Property tests (`proptest`) MUST run the example scenario, and SHOULD also run a scenario with a shared lane split across phases, for at least 1,000 steps under arbitrary scripts (each step: zero or more spawns of movements the geometry allows, then an arbitrary `Command`), and check after every step:
  - conservation: spawned = in backlog + on lane + departed, and the counters of R4.3 agree with the vehicles actually stored;
  - no two vehicles occupy the same cell, and every cell index is below the lane length;
  - each vehicle on a lane moved 0 or 1 cells forward since the previous step (or entered cell 0 from the backlog), and never changed lane;
  - no overtaking: in each lane, vehicles cross in the order they entered, and they enter in the order they were spawned into that lane;
  - every departure was in cell `last` before the step and its movement is `Green` in the lights after the step;
  - no two departures in the same step have conflicting movements.
- **R6.3** The invariant checks SHOULD be written as functions over recorded per-step snapshots, so unit tests can reuse them.

## R7. ADR and roadmap

- **R7.1** `specs/adr/0004-vehicle-update-rule.md` MUST follow the template and record: one cell per step with parallel update, crossing on green only with instant exit, the per-lane backlog, the fewest-vehicles lane choice, and strict FIFO lanes. Its Context MUST name the alternatives considered (Nagel–Schreckenberg with multi-cell speeds, with and without random slowdown; sequential front-to-back update; crossing on yellow; vehicles occupying the intersection box; rejecting spawns on a full entry cell). Its Consequences MUST include the costs (quantized speed, a 2-step headway fixed by the model rather than configured, speed tied to `cell_length_m / step_s`, unbounded backlogs) and the benefits.
- **R7.2** Status `Accepted`, date of the PR. The index table in `specs/adr/README.md` MUST list it.
- **R7.3** Roadmap entry 1.3 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.
