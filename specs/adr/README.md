# Architecture decision records

An ADR is a short note that records one significant decision: the context, what we chose, and what it costs. It lets a future reader (including the author) understand why the code looks the way it does, without digging through history.

## When to write one

Write an ADR when a decision:

- is hard to reverse,
- affects several crates, or
- picks between real alternatives.

Small, local choices do not need one. Write the ADR in the PR of the phase that takes the decision.

## Naming

`NNNN-kebab-title.md`, numbered in order and never renumbered. `0000-template.md` is the template. Copy it to start a new one.

## Statuses

- `Proposed`: under discussion.
- `Accepted`: in force.
- `Superseded by NNNN`: replaced by a later ADR. Keep the old file and link to the new one.
- `Deprecated`: no longer relevant, and not replaced.

## Index

| Number | Title | Status | Date |
|--------|-------|--------|------|
| [0001](0001-rust-burn-leptos.md) | Rust, Burn and Leptos for the whole system | Accepted | 2026-09-26 |
| [0002](0002-discrete-cell-model.md) | Discrete cell model for lanes | Accepted | 2026-09-26 |
| [0003](0003-signal-safety-model.md) | Signal safety model | Accepted | 2026-09-26 |
| [0004](0004-vehicle-update-rule.md) | Vehicle update rule | Accepted | 2026-09-26 |
| [0005](0005-demand-model.md) | Demand model | Accepted | 2026-09-26 |
| [0006](0006-metric-definitions.md) | Metric definitions | Accepted | 2026-09-26 |
