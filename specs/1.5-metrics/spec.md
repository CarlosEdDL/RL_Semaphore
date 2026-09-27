# 1.5 Metrics

**Stage:** 1 (Simulator, single intersection, no RL) · **Status:** ☑ done · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R8) · [plan.md](plan.md) (tasks).

## Goal

Measure a run. At the end of this phase every vehicle carries a live wait counter: the number of steps since it was spawned during which it did not move. `Simulation::step` keeps it up to date. A separate `EpisodeMetrics` collector observes the simulation after each step and produces an `EpisodeSummary` with the mean, p50, p95, p99 and max wait (overall and per approach), throughput, and the time-averaged and maximum queue length per lane, per approach and for the whole intersection. Vehicles still in the system at the end count with the wait they have so far, so a starved vehicle can never drop out of the tail. An `insta` snapshot pins the summary of the example scenario, so these become the numbers that 1.6 prints and every later controller is compared on.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-crate-layout-and-compatibility) | A new `metrics` module, no new dependencies, 1.1–1.4 behaviour unchanged |
| [R2](requirements.md#r2-per-vehicle-wait-tracking) | `Vehicle::wait_steps`, `Vehicle::is_stopped`, and `Departure::wait_steps` |
| [R3](requirements.md#r3-the-metrics-collector) | `EpisodeMetrics`: `new`, `observe`, `summary`, and the loop contract |
| [R4](requirements.md#r4-wait-statistics) | `WaitStats`: which vehicles count, nearest-rank percentiles, units |
| [R5](requirements.md#r5-throughput-and-queue-statistics) | Throughput, and `QueueStats` per lane, per approach and in total |
| [R6](requirements.md#r6-determinism-and-robustness) | Determinism, no panics, cost per step, clippy pedantic |
| [R7](requirements.md#r7-tests) | Hand-traced waits, the delay identity, percentile units, the `insta` summary snapshot |
| [R8](requirements.md#r8-adr-and-roadmap) | ADR-0006 and roadmap |

## Decisions taken

- **Wait is stopped time.** A vehicle *moves* in a step if it enters cell 0 from its backlog, advances one cell, or crosses the stop line. Every step in which it does none of these adds one to its wait, whether it is in the backlog or on its lane. A vehicle that never meets a red light or a queue waits 0. In the cell model (ADR-0004) a vehicle needs exactly `n + 1` moves to cross a lane of `n` cells, so for a departed vehicle the wait equals its delay against free flow: `departed_at − spawned_at − (n + 1)`. The tests check this identity, but the simulation keeps an explicit counter, so the definition stays valid when Stage 6 changes the geometry.
- **The counter lives on the vehicle.** `Vehicle::wait_steps()` can be read at any step, which the fair reward (3.4) and the starvation highlighting (2.5) need. `Vehicle::is_stopped()` tells whether the vehicle moved in the most recent step. `Departure` carries the final wait, so a vehicle's wait is not lost when it leaves the model.
- **Aggregation lives in a separate collector.** `EpisodeMetrics::new(&sim)` starts an episode at the current step. `observe(&sim, &report)` is called once after every `step`: it records the departures' waits and samples the queues. `summary(&sim)` can be called at any time without consuming the collector. `Simulation` itself only gains the per-vehicle counter and flag, so a training loop that does not need episode statistics pays almost nothing.
- **Unfinished vehicles count.** The wait distribution in a summary covers every vehicle that departed during the episode (final wait) plus every vehicle still in the model (wait so far). A policy that never serves a side street therefore shows a growing max and p99 instead of looking better. The summary also reports how many vehicles are still in the system.
- **Exact percentiles, nearest rank.** Waits are whole steps, so the collector keeps every departed vehicle's wait and sorts once per summary. The p-th percentile of `n` sorted values is the value at rank `⌈p × n / 100⌉`, computed with integers. Values are reported in seconds (`steps × step_s`). There is no histogram crate and no interpolation, so the numbers are exact and reproducible.
- **A queue is the stopped vehicles.** A lane's queue after a step is the number of its vehicles, on the lane or in its backlog, that did not move during that step. That matches the wait definition: the queue sums the wait being added at that step. Queues are sampled after every observed step and reported as a time average and a maximum, per lane, per approach (the sum of its lanes at each step) and for the whole intersection.
- **Per-approach breakdown.** The mission requires per-approach wait, so wait statistics come overall and for each approach. Queue statistics also go down to the lane.

## Out of scope

- Printing, the fixed-time controller, and the `simulate` command (1.6). Tests drive the loop and format the summary themselves.
- Serde on metrics types and the `Metrics` DTO (2.1).
- Number of stops, fuel or emission models, and travel-time statistics.
- Per-step time series, rolling windows, or streaming percentile estimators.
- Reward computation (3.4) and trip-level city metrics (6.6).
- Any change to the step rule, the signal, or demand generation.

## Acceptance criteria

1. Every `Vehicle` exposes `wait_steps()` and `is_stopped()`, and every `Departure` carries `wait_steps`. A lone vehicle under constant green waits 0. A vehicle held at a red light for `k` steps waits `k`. A vehicle that cannot leave its backlog accumulates wait. (R2)
2. Over random runs, each step a vehicle's wait grows by exactly 1 if it did not move and by 0 if it did, and every departure satisfies `departed_at − spawned_at = n + 1 + wait_steps`. (R2, R7)
3. `EpisodeMetrics` produces an `EpisodeSummary` with mean, p50, p95, p99 and max wait in seconds, overall and per approach. It includes vehicles still in the model, and reports `None` for an approach (or episode) with no vehicles. (R3, R4)
4. The summary reports departures, vehicles still in the system, throughput in veh/h, and the time-averaged and maximum queue per lane, per approach and in total. A queue never exceeds the lane's cells plus its backlog. (R5)
5. Calling `observe` out of order returns an error and leaves the collector unchanged. Nothing panics, and nothing depends on `HashMap` order or the clock. (R3, R6)
6. An `insta` snapshot of the example scenario's summary (seed 2024, 600 steps, `Command::Hold`) is committed. The 1.4 trajectory snapshot is unchanged. (R1, R7)
7. `sim` adds no dependencies, passes clippy pedantic, and the WASM build still passes. (R1, R6)
8. `specs/adr/0006-metric-definitions.md` exists with status `Accepted`, the ADR index lists it, CI is green, and roadmap entry 1.5 is marked ☑. (R8)
