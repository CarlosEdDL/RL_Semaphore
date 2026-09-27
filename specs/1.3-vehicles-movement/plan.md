# 1.3 Vehicles & movement: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feat/1.3-vehicles-movement`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Vehicle types
- [ ] Create a `vehicle` module with `VehicleId(u64)` (`Copy`, `Eq`, `Ord`, `Hash`, `Debug`, `Display`) ([R2.1](requirements.md#r2-vehicles-and-spawning)).
- [ ] Add `Vehicle { id, movement: MovementId, lane: LaneId, spawned_at: u64, position }` with private fields and accessors, and a `Position` enum (`Backlog` / `OnLane { cell: u32 }`) (R2.2).
- [ ] Add `Departure { vehicle, movement, lane, spawned_at, departed_at }` (R3.7).
- [ ] Add `SpawnError::NoLaneForMovement { movement }` with `thiserror`, `#[non_exhaustive]` (R2.5).

### 2. Simulation state
- [ ] Create a `simulation` module with `Simulation { scenario, signal, t, next_id, vehicles, lanes }` ([R4.1](requirements.md#r4-observation-api)).
- [ ] Store vehicles in a `BTreeMap<VehicleId, Vehicle>` (ordered, deterministic). Store per-lane state in a `Vec` indexed in `Direction::ALL`, then lane-index order: `cells: Vec<Option<VehicleId>>` sized to `len_cells()`, and `backlog: VecDeque<VehicleId>` (R5.1).
- [ ] Keep counters for spawned, backlog, on-lane, and departed (R4.3).
- [ ] Re-export the new public types from `lib.rs` (R1.2).

### 3. Spawning
- [ ] `spawn(approach, movement)`: collect lanes that allow the movement, pick the one with the fewest vehicles (cells occupied + backlog length), ties to the lowest index; error without consuming an id if none ([R2.3–R2.5](requirements.md#r2-vehicles-and-spawning)).
- [ ] Track an occupied-cell count per lane so lane choice does not scan cells.

### 4. Step rule
- [ ] `step(command)`: step the signal, then read `lights()` once ([R3.1](requirements.md#r3-the-step-rule)).
- [ ] Per lane, process cells from `last` down to 0: cross the head if green (R3.2), then move each vehicle into the next cell if that cell was empty in `C(t)` (R3.3). Walking front to back and keeping a `was_occupied` flag for the cell ahead (taken *before* any change this step) gives the parallel rule without copying the lane.
- [ ] Then let the backlog front enter cell 0 if cell 0 was empty in `C(t)` (R3.4).
- [ ] Update vehicle positions, counters, and `t`. Return `StepReport { signal: StepOutcome, departures: Vec<Departure> }` (R3.7).
- [ ] Rustdoc on `step` with the order of operations, the parallel rule, and the 2-step headway (R3.8).

### 5. Observation API
- [ ] Accessors: `scenario()`, `signal()`, `lights()`, `step_count()`, `vehicles()`, `vehicle(id)`, `lane_cells(LaneId)`, `backlog(LaneId)`, and the counters ([R4.2–R4.4](requirements.md#r4-observation-api)).
- [ ] Derive `Clone` and `Debug` on `Simulation` (R5.4).

### 6. Unit tests
- [ ] Add `crates/sim/tests/vehicles.rs` with the cases of [R6.1](requirements.md#r6-tests). Keep the example scenario for timing tests, and small inline scenarios for lane choice, `NoLaneForMovement`, and head-of-line blocking.
- [ ] Hand-trace the discharge headway first (a queue of 3 at the stop line, then switch to green) and pin the exact departure steps before writing the property tests.
- [ ] For "no crossing on yellow", run until a transition starts and check that the stop-line vehicle is still there while its movement is yellow.

### 7. Property tests
- [ ] Add `crates/sim/tests/vehicles_props.rs` ([R6.2](requirements.md#r6-tests)). Generate per step a small `Vec` of spawn requests drawn from the movements the geometry allows, plus a `Command` (`Hold` weighted higher than `SwitchTo(k)`, so both short and long greens appear).
- [ ] Record a per-step snapshot (vehicle positions, lights, departures, counters) and write each invariant as a function over two consecutive snapshots (R6.3).
- [ ] Add the shared-lane scenario as a second strategy input.

### 8. ADR-0004
- [ ] Copy `specs/adr/0000-template.md` to `specs/adr/0004-vehicle-update-rule.md` and fill it in ([R7.1](requirements.md#r7-adr-and-roadmap)).
- [ ] Add it to the index table in `specs/adr/README.md` (R7.2).

### 9. Roadmap
- [ ] Mark 1.3 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R7.3).

### 10. Verify locally

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
cargo build -p rl-semaphore-web --target wasm32-unknown-unknown --locked
cargo deny --locked check
cargo audit

# spot checks
cargo nextest run -p rl-semaphore-sim --locked
PROPTEST_CASES=2000 cargo nextest run -p rl-semaphore-sim --locked
cargo doc -p rl-semaphore-sim --no-deps
```

## Notes and risks

- **Parallel update done in place.** The easiest bug is letting a vehicle move into a cell that another vehicle vacated in the same step, which turns the parallel rule into a sequential one and makes a whole queue move at once (1 veh/step discharge). Record occupancy of the cell ahead before changing it, and pin the 2-step headway with a unit test.
- **Lights before or after the signal step.** Vehicles use `L(t + 1)`, the lights after this step's signal update. Using `L(t)` would also be safe, but the report would then show lights that do not match the crossings. Keep one convention and test it (a crossing is never reported in the step where its movement turns yellow).
- **Example config hides head-of-line blocking.** Every shared lane in `single-intersection.toml` has all its movements green together, so the FIFO rule is never exercised there. The inline shared-lane scenario is the only coverage; do not skip it.
- **Example config hides lane choice.** Each movement has exactly one lane in the example, so the tie-break needs an inline scenario with two lanes allowing the same movement.
- **Unbounded backlogs.** Under a long red and heavy spawning, backlogs grow without limit. Keep the property-test spawn rate small (for example 0–2 spawns per step) so memory and run time stay bounded; a cap belongs with demand in 1.4 if it is needed at all.
- **Scope creep toward 1.5.** It is tempting to add "steps stopped" or wait counters to `Vehicle`. Keep only spawn and departure steps here; metrics are 1.5.
- **Scope creep toward 1.4.** Do not add an arrival process or turn ratios. Tests spawn explicitly.
- **proptest run time.** 1,000+ steps × 256 cases with up to ~100 vehicles is fast, but keep the step count bounded so CI stays quick. Commit `proptest-regressions/` if it appears.
