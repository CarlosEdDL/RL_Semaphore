# 1.5 Metrics: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

Notation: `t` is the simulation step count. A vehicle *moves* during a step if, in that step, it enters cell 0 from its backlog, advances one cell, or crosses the stop line. For a lane, `n` is its number of cells. `w(v)` is a vehicle's wait in steps. `t0` is the step count when an `EpisodeMetrics` was created.

## R1. Crate layout and compatibility

- **R1.1** The collector and its result types SHOULD live in a new `metrics` module of `sim`. New public types MUST be re-exported from the crate root.
- **R1.2** `sim` MUST NOT gain any new dependency.
- **R1.3** The step rule MUST NOT change. The 1.4 snapshot `demand_snapshot__example_trajectory_is_pinned.snap` MUST stay byte-identical.
- **R1.4** Existing 1.1–1.4 tests MUST keep passing. Test helpers in `crates/sim/tests/common` MAY be extended (for example to record waits in `Snapshot`), but no existing assertion may be weakened or removed.

## R2. Per-vehicle wait tracking

- **R2.1** `Vehicle` MUST gain `wait_steps(&self) -> u64`: the number of steps since the vehicle was spawned during which it did not move. It MUST be 0 at spawn.
- **R2.2** `Vehicle` MUST gain `is_stopped(&self) -> bool`: `true` if and only if the vehicle did not move during the most recent call to `Simulation::step`. For a vehicle spawned since that call it MUST be `false`.
- **R2.3** During `Simulation::step`, every vehicle that is in the model before the step and still in the model after it MUST have `wait_steps` increased by 1 and `is_stopped` set if it did not move, and `wait_steps` unchanged and `is_stopped` cleared if it moved. This covers vehicles in a backlog that do not enter the lane.
- **R2.4** `Departure` MUST gain a public field `wait_steps: u64`: the vehicle's wait when it crossed. Crossing counts as a move, so the step in which a vehicle departs never adds to its wait.
- **R2.5** Delay identity: for every departure from a lane of `n` cells, `departed_at − spawned_at = n + 1 + wait_steps`. For every vehicle in the model at step `t`, `t − spawned_at = m + wait_steps`, where `m` is 0 for a vehicle in a backlog and `cell + 1` for one on its lane. The rustdoc of `wait_steps` MUST state the definition and the identity.
- **R2.6** Wait tracking MUST keep `Simulation::step` at `O(cells + vehicles)` and MUST NOT allocate per vehicle.

## R3. The metrics collector

- **R3.1** `EpisodeMetrics::new(sim: &Simulation) -> EpisodeMetrics` MUST start an episode at `t0 = sim.step_count()`, with the lane layout of `sim`'s scenario and empty statistics.
- **R3.2** `EpisodeMetrics::observe(&mut self, sim: &Simulation, report: &StepReport) -> Result<(), MetricsError>` MUST be called once after each `step`, with the report that step returned. It MUST record the lane, approach and `wait_steps` of every departure in `report`, and sample every lane's queue (R5.2) from `sim`.
- **R3.3** `observe` MUST return `MetricsError::StepMismatch { expected, found }` if `sim.step_count()` is not the step count after the previous observation plus 1 (`t0 + 1` for the first call). On error the collector MUST be unchanged. `MetricsError` MUST derive `thiserror::Error`, be `#[non_exhaustive]`, and have rustdoc on every variant.
- **R3.4** `EpisodeMetrics::summary(&self, sim: &Simulation) -> EpisodeSummary` MUST take `&self`, so it can be called mid-episode and again later. It MUST use `sim` only for the vehicles still in the model and for `step_s`.
- **R3.5** The rustdoc on `EpisodeMetrics` MUST state the loop: at step `t`, draw and spawn arrivals, call `step`, then `observe` with the returned report. It SHOULD include a doctest that runs the example scenario with `Demand` for a few steps and reads a summary.
- **R3.6** `EpisodeMetrics` MUST be `Clone` and `Debug`, so a run can be forked with its statistics.

## R4. Wait statistics

- **R4.1** `WaitStats` MUST have `count: u64`, `mean_s: f64`, `p50_s: f64`, `p95_s: f64`, `p99_s: f64` and `max_s: f64`.
- **R4.2** The population of a summary MUST be every vehicle that departed during an observed step (with its `Departure::wait_steps`) plus every vehicle still in the model when `summary` is called (with its current `wait_steps`). No vehicle may be counted twice.
- **R4.3** `EpisodeSummary::wait` MUST be the `WaitStats` of the whole population, and `wait_by_approach: [Option<WaitStats>; 4]` those of the vehicles of each approach, indexed by `Direction::index`. A population with no vehicles MUST give `None`.
- **R4.4** Percentiles MUST use the nearest-rank method on the waits in steps, sorted ascending: for `n ≥ 1` values and `p ∈ {50, 95, 99}`, the result is the value at 1-based rank `⌈p × n / 100⌉`, computed with integer arithmetic. `max` is the value at rank `n`. There is no interpolation.
- **R4.5** Seconds MUST be computed as `steps as f64 × step_s`, with the mean as `(sum of steps) as f64 / n as f64 × step_s`. Sums MUST be accumulated in `u64` (or wider) and use checked or saturating arithmetic.
- **R4.6** The percentile routine SHOULD be a private function with its own unit tests.

## R5. Throughput and queue statistics

- **R5.1** `EpisodeSummary` MUST report `steps: u64` (observed steps), `duration_s: f64` (`steps × step_s`), `departed: u64` (departures observed), `in_system: u64` (vehicles in the model at `summary`), and `throughput_veh_per_h: f64` (`departed × 3600 / duration_s`, and `0.0` when `steps = 0`).
- **R5.2** The queue of a lane after a step MUST be the number of its vehicles, on the lane or in its backlog, with `is_stopped() == true`.
- **R5.3** `QueueStats` MUST have `mean: f64` (the sum of the sampled queues divided by the number of observed steps, `0.0` when there are none) and `max: u64`.
- **R5.4** `EpisodeSummary` MUST report `queue_by_lane: Vec<(LaneId, QueueStats)>` in lane order (approaches in `Direction::ALL` order, then lane index), `queue_by_approach: [QueueStats; 4]` indexed by `Direction::index`, and `queue_total: QueueStats`. An approach's queue at a step is the sum of its lanes' queues at that step, and the intersection's is the sum over all lanes, so a `max` is the maximum of the sums, not the sum of the maxima.
- **R5.5** Summary types (`EpisodeSummary`, `WaitStats`, `QueueStats`) MUST derive `Debug`, `Clone` and `PartialEq`, with rustdoc on every public field stating its unit.

## R6. Determinism and robustness

- **R6.1** Metrics MUST depend only on the observed simulation states and reports. No wall-clock time, OS randomness, or `HashMap` iteration.
- **R6.2** Nothing in this phase may panic for a validated scenario, any seed and any command sequence, including an episode with 0 steps or 0 vehicles. Casts follow the 1.1 rule: range-check first, then a local `#[allow(clippy::cast_*)]` with an `// INVARIANT:` comment.
- **R6.3** `observe` SHOULD cost `O(lanes + vehicles in the model + departures)` and allocate only to store the departures' waits. `summary` SHOULD cost `O(k log k)` for `k` vehicles in its population.
- **R6.4** Clippy pedantic MUST pass, and the WASM build of `web` MUST still pass.

## R7. Tests

- **R7.1** Percentile unit tests (in the `metrics` module) MUST cover: one value; `n = 100` values `1..=100` (p50 = 50, p95 = 95, p99 = 99, max = 100); a small `n` where the rank rounds up (for example `n = 3`); all values equal.
- **R7.2** Hand-traced wait tests (in `crates/sim/tests/metrics.rs`) MUST cover:
  - a lone vehicle under constant green departs with `wait_steps = 0` and is never stopped;
  - a vehicle reaching the stop-line cell under red accumulates exactly one wait step per red step, and its departure reports the total;
  - a vehicle behind a stopped vehicle, and one kept in a backlog by a full cell 0, both accumulate wait;
  - a vehicle spawned since the last step has `wait_steps = 0` and `is_stopped() == false`.
- **R7.3** Collector tests MUST cover:
  - a hand-built episode with known waits gives the expected `WaitStats`, overall and per approach, including `None` for an approach with no vehicles;
  - a vehicle still in the model at the end is counted with its wait so far, and raises `max_s` above every departed vehicle's wait;
  - queue mean and max on a hand-traced lane, and the approach `max` being the max of the sums;
  - throughput and `duration_s`, and a summary with 0 steps;
  - `observe` with a skipped or repeated step returns `StepMismatch` and leaves the summary unchanged;
  - `summary` called twice gives equal results.
- **R7.4** A property test MUST run arbitrary valid demand and seeds through the simulation for at least 1,000 steps, with arbitrary commands, and check at every step: the wait rule of R2.3, the delay identity of R2.5 for vehicles in the model and for departures, each lane's queue at most `n` plus its backlog, and at the end `departed + in_system` equals the vehicles spawned during the episode plus those in the model at `t0`.
- **R7.5** A snapshot test MUST run the example scenario with seed 2024 for 600 steps with `Command::Hold` (the same run as the 1.4 snapshot), and `insta::assert_snapshot!` the summary as plain text built by the test: counts, throughput, overall and per-approach wait stats, and queue stats per lane, per approach and in total. Floats MUST be formatted with a fixed number of decimals.
- **R7.6** Every test MUST be deterministic (fixed seeds).

## R8. ADR and roadmap

- **R8.1** `specs/adr/0006-metric-definitions.md` MUST follow the template and record: wait as stopped steps and its equality with delay in the cell model, the counter on `Vehicle`, a separate collector, counting vehicles still in the system, exact nearest-rank percentiles, and a queue as the stopped vehicles. Its Context MUST name the alternatives considered (time in system, delay computed from the free-flow time, departed-only statistics, both populations reported, metrics inside `Simulation`, a collector that diffs positions, a histogram or streaming quantiles, interpolated percentiles, a queue as all vehicles or as the backlog only). Its Consequences MUST include the costs (memory grows with the vehicles of an episode, a vehicle's wait at an episode start counts from its spawn, the caller must call `observe` after every step, stops are not counted) and the benefits.
- **R8.2** Status `Accepted`, date of the PR. The index table in `specs/adr/README.md` MUST list it.
- **R8.3** Roadmap entry 1.5 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.
