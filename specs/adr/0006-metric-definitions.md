# 0006. Metric definitions

**Status:** Accepted
**Date:** 2026-09-26

## Context

Every controller in the project, from the fixed-time baseline (1.6) to the RL agents (Stage 3+), is judged on the same few numbers: wait, throughput and queue length. The definitions must be exact, reproducible and honest about vehicles that never get served, and the tail (p95, p99, max) matters as much as the mean, because the mission is fairness, not only average delay.

Alternatives considered:

- **Time in system (spawn to crossing).** Trivial to compute, but it includes the free-flow travel time, which depends on the lane length and not on the controller.
- **Delay computed from the free-flow time** (`departed_at − spawned_at − (n + 1)`). Correct in the cell model, but it hard-codes ADR-0004 into the metric, so it breaks when Stage 6 changes the geometry, and it cannot be read for a vehicle still in the model.
- **Departed-only statistics.** A policy that never serves a side street would look better, because its starved vehicles drop out of the tail.
- **Both populations reported** (departed and in-system separately). Complete, but every consumer would have to combine them, and a comparison between controllers needs one number.
- **Metrics inside `Simulation`.** A training loop that does not need episode statistics would pay for them, and the simulator would carry episode state.
- **A collector that diffs positions between steps.** Needs a copy of the previous state, and re-derives what `step` already knows.
- **A histogram or streaming quantiles.** Bounded memory, but the numbers become approximate and depend on the bucket layout or the algorithm.
- **Interpolated percentiles.** Smooth, but they report waits nobody experienced, and different libraries interpolate differently.
- **A queue as all vehicles, or as the backlog only.** All vehicles counts moving traffic as queue, and the backlog alone hides the vehicles standing on the lane.

## Decision

**Wait is stopped time.** A vehicle *moves* in a step if it enters cell 0 from its backlog, advances one cell, or crosses the stop line. Every step in which it does none of these adds one to its wait, in a backlog or on a lane. In the cell model a vehicle needs exactly `n + 1` moves to cross a lane of `n` cells, so for a departed vehicle wait equals delay against free flow: `departed_at − spawned_at = n + 1 + wait_steps`. The simulation keeps an explicit counter and the tests check the identity, so the definition survives a change of geometry.

**The counter lives on the vehicle.** `Vehicle::wait_steps` and `Vehicle::is_stopped` are updated by `Simulation::step` inside its existing per-lane pass, and `Departure::wait_steps` carries the final value out.

**A separate collector aggregates.** `EpisodeMetrics::observe` is called after every step, records the departures' waits and samples the queues. `summary` can be called at any time. A skipped or repeated step is an error.

**Vehicles still in the system count.** The wait population is every vehicle that departed during the episode plus every vehicle in the model at the time of the summary, with its wait so far.

**Exact nearest-rank percentiles.** The collector keeps each departed vehicle's wait in steps and sorts once per summary. The p-th percentile of `n` values is the value at rank `⌈p × n / 100⌉`, computed with integers, and is reported in seconds.

**A queue is the stopped vehicles.** A lane's queue after a step is the number of its vehicles, on the lane or in its backlog, that did not move in that step, so the queue sums the wait being added. It is reported as a time average and a maximum per lane, per approach and in total, where an approach or the total is summed at each step before taking the maximum.

## Consequences

### Positive

- Wait is independent of geometry, can be read live (fair reward 3.4, starvation highlighting 2.5), and equals delay in the current model.
- A starved vehicle raises the max and p99 instead of disappearing, so policies cannot look good by ignoring a road.
- The numbers are exact and reproducible, with no dependency on a statistics crate, and they are pinned by a snapshot.
- `Simulation` pays only for a counter and a flag per vehicle, and the collector is optional and cloneable to fork a run.

### Negative

- Memory grows with the vehicles of an episode (one `u64` per departure). A streaming estimator would change the numbers and need its own ADR.
- A vehicle already in the model when an episode starts counts its wait from its spawn, and earlier departures are not seen.
- The caller must call `observe` after every step, and the collector cannot repair a missed one.
- Stops are not counted: a vehicle that stops twice for 3 steps each has the same wait as one that stops once for 6.
- Unfinished vehicles make the wait depend on where the episode ends, so episodes compared must have the same length.
