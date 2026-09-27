# 0002. Discrete cell model for lanes

**Status:** Accepted
**Date:** 2026-09-26

## Context

The simulator needs a spatial model for vehicles on a lane. It must be deterministic (same config, seed and code version give the same result), fast enough to run many episodes for training, and easy to render and to snapshot over a WebSocket.

Alternatives considered:

- **Continuous car-following model** (for example IDM with `f64` positions and speeds). It is the most realistic, but collisions must be prevented by the model's parameters, floating-point results can differ across platforms, and stepping is costlier.
- **Macroscopic or cell-transmission flow model.** Traffic is a density per cell, not individual vehicles. It is very fast, but it has no individual vehicles to render, and per-vehicle metrics such as waiting time are only approximate.
- **Discrete cells** (Nagel-Schreckenberg family): each lane is a sequence of fixed-length cells, with at most one vehicle per cell and integer positions.

## Decision

Lanes are modelled as fixed-length cells with integer positions, one vehicle per cell. The config is written in meters, and `sim` converts each approach length to `floor(length_m / cell_length_m)` cells once, at load time. Cell 0 is the upstream entry and the last cell is the one immediately behind the stop line. Vehicles that cross the stop line leave the model in Stage 1.

## Consequences

### Positive

- Collision-freedom is simple: a vehicle only moves into an empty cell.
- Integer positions make the simulation deterministic, with no floating-point drift.
- Stepping is fast, since it works on small integer arrays.
- Rendering and snapshots are easy: a vehicle is a lane and a cell index.

### Negative

- Coarser spatial resolution: lengths are rounded down to whole cells and the remainder is dropped.
- Speed is quantized to cells per step, so it depends on the cell length and the time step.
- Acceleration and braking are less realistic than in a continuous model.
- Changing the model later (for example to IDM) would affect the vehicle, metrics and rendering code.
