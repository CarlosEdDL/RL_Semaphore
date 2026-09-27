# 1.5 Metrics: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feature/metrics`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Per-vehicle wait
- [x] Add `wait_steps: u64` and `stopped: bool` to `Vehicle` (both start at 0 / `false` in `spawn`), with the accessors `wait_steps()` and `is_stopped()` ([R2.1–R2.2](requirements.md#r2-per-vehicle-wait-tracking)). Put the definition and the delay identity in their rustdoc (R2.5).
- [x] Add `pub wait_steps: u64` to `Departure`, filled from the vehicle when it crosses (R2.4).

### 2. Wait updates in `step`
- [x] Update waits inside the existing per-lane pass, without a second loop over all vehicles (R2.6):
  - stop-line cell: a vehicle that crosses departs with its current wait; one that does not is stopped;
  - advance loop: a vehicle that moves is cleared; an occupied cell whose next cell was occupied is stopped;
  - backlog: the vehicle that enters cell 0 is cleared, every other backlog vehicle is stopped.
- [x] Check that nothing else in `step` changes, and that the 1.4 snapshot stays byte-identical (R1.3).
- [x] Extend `tests/common::Snapshot` with each vehicle's `wait_steps` and `is_stopped`, and `check_transition` with the wait rule (R2.3) and the delay identity (R2.5), so every existing 1.3–1.4 run checks them for free (R1.4).

### 3. Collector
- [x] New `metrics` module: `EpisodeMetrics`, `EpisodeSummary`, `WaitStats`, `QueueStats`, `MetricsError`, re-exported from `lib.rs` ([R1.1](requirements.md#r1-crate-layout-and-compatibility), [R3](requirements.md#r3-the-metrics-collector)).
- [x] State: `t0`, last observed step count, `step_s`, the lane layout (lane ids in order, and each lane's approach), departed waits kept per approach as `[Vec<u64>; 4]`, and per lane / per approach / total the queue sum (`u64`) and max.
- [x] `observe`: check the step count first (R3.3), then record the departures and sample the queues in one pass over `sim.vehicles()` (vehicles know their lane, so a queue is a count of stopped vehicles per lane slot). Nothing is mutated before the check passes.
- [x] `summary`: per approach, copy the departed waits, add the waits of vehicles still in the model, sort, and compute `WaitStats`. Merge the four sorted lists (or concatenate and sort) for the overall stats ([R4](requirements.md#r4-wait-statistics)). Then compute throughput and the queue stats ([R5](requirements.md#r5-throughput-and-queue-statistics)).
- [x] Private `nearest_rank(sorted: &[u64], p: u64) -> u64` and a `wait_stats(sorted: &[u64], step_s: f64) -> Option<WaitStats>` helper, with unit tests (R4.4, R4.6, [R7.1](requirements.md#r7-tests)).
- [x] Rustdoc on `EpisodeMetrics` with the loop order and a doctest (R3.5).

### 4. Tests
- [x] `crates/sim/tests/metrics.rs`: the hand-traced wait cases of R7.2 and the collector cases of R7.3. Reuse `common::inline` scenarios and fixed command scripts, so every expected number can be traced by hand.
- [x] Add a `run_with_metrics` helper to `tests/common` (or extend `run_with_demand` to take an optional collector) that calls `observe` after each step.
- [x] `crates/sim/tests/metrics_props.rs`: the property test of R7.4. Use the same flow bounds as `demand_props.rs`, so the run time stays similar.
- [x] `crates/sim/tests/metrics_snapshot.rs`: the summary snapshot of R7.5. Format floats with `{:.3}`. Run `cargo insta test` once, review the `.snap` by hand (the wait of a through vehicle on the example's green should be small, and the max should not be above about `max_red_s` plus a few cycles), and commit it.

### 5. ADR-0006
- [x] Copy `specs/adr/0000-template.md` to `specs/adr/0006-metric-definitions.md` and fill it in ([R8.1](requirements.md#r8-adr-and-roadmap)).
- [x] Add it to the index table in `specs/adr/README.md` (R8.2).

### 6. Roadmap
- [x] Mark 1.5 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R8.3).

### 7. Verify locally

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
CI=true cargo nextest run -p rl-semaphore-sim --locked   # insta must not write snapshots in CI
git diff --exit-code crates/sim/tests/snapshots/demand_snapshot__example_trajectory_is_pinned.snap
cargo doc -p rl-semaphore-sim --no-deps
```

## Notes and risks

- **Stopped vs moved must match the step rule exactly.** The wait update has to use the same "occupied at the start of the step" facts as the parallel update, or the delay identity breaks. That identity is the main guard: it ties the counter to positions and spawn times, which 1.3 already checks. If it fails, fix the update, not the identity.
- **Backlog vehicles and cost.** Every backlog vehicle but the front one is stopped on every step. Marking them walks the backlogs, which is `O(vehicles)` and fine. Do not add a per-lane "last touched" trick unless the 1.7 benchmark shows a need.
- **Changing `Departure` and `Vehicle`.** Adding public fields changes public types, but `Departure` is only built inside `sim` and `Vehicle` has private fields, so no caller breaks. `Vehicle`'s `PartialEq` now compares waits too; check no test relied on comparing vehicles across steps.
- **Vehicles present at `t0`.** An episode can start on a running simulation. Such vehicles count in `in_system` and in the wait population if still present, with their wait since spawn, but their earlier departures are not seen. Document it on `EpisodeMetrics::new`; 1.6 always starts from a fresh simulation.
- **Float formatting in the snapshot.** Means and throughput are floats. Print them with a fixed number of decimals so a formatting change in `std` or a harmless reordering of a sum cannot change the snapshot. Waits are integer steps times `step_s = 1.0`, so the percentiles print exactly.
- **Memory.** The collector keeps one `u64` per departed vehicle. A 1-hour episode at the example demand is about 4,000 values, so this is fine for training. A streaming estimator can come later if long runs need it; it would change the numbers, so it would need its own ADR.
- **Scope creep toward 1.6 and 2.1.** Do not add a `Display` impl meant for the CLI, serde derives, or a public run loop. The snapshot test formats the summary itself.
- **Scope creep toward 3.4.** Do not add a reward or any weighting of waits. `wait_steps` on `Vehicle` is all the reward will need from this phase.
