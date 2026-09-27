# 0005. Demand model

**Status:** Accepted
**Date:** 2026-09-26

## Context

The simulator needs traffic that arrives by itself, reproducibly, so that the same seed gives the same run (Stage 3+ trains and compares policies on it). Arrivals must be described next to the rest of a scenario, be cheap to draw, and not depend on the version of a random-number library.

Alternatives considered:

- **A separate demand file.** Keeps scenarios small, but one run would need two files that can drift apart, and `Scenario::to_config()` could no longer record a run exactly.
- **Per-movement flows.** More direct, but twelve numbers instead of four flows plus ratios, and comparing "same flow, different turning" gets harder.
- **`rand_distr::Poisson`.** Less code, but its algorithm and draw pattern may change between releases and silently change every trajectory. We would also depend on `rand` itself.
- **Bernoulli arrivals (at most one per step).** Simple, but caps the flow at one vehicle per step and distorts it at high rates.
- **A single shared RNG.** Changing the demand of one approach would change the arrivals of all the others, which makes comparisons between demand configs noisy.
- **`Simulation` owning the RNG.** Puts randomness into the simulator, whose step rule is deterministic and hand-traced in tests (ADR-0004), and blocks time-varying demand (10.1) from swapping in another generator.
- **A backlog cap with dropped arrivals.** Bounds memory, but hides spillback from the metrics of 1.5.

## Decision

Demand lives in a `[demand]` table of the scenario TOML, with one optional entry per approach: a flow in veh/h and the turn ratios `left`, `through`, `right`. A missing entry or table means no demand, so older scenarios load unchanged. Validation makes a `DemandPlan`: ratios in 0–1 that sum to 1, positive only for movements a lane allows, and a mean of at most 10 arrivals per approach per step.

A `Demand` generator, built from a scenario and a `u64` seed, draws for each approach and step a Poisson count with mean `veh_per_h × step_s / 3600`, then one movement per arrival from the turn ratios. The sampler is our own inversion routine over uniforms built from `ChaCha8Rng` output (`(next_u64() >> 11) × 2^-53`). Each approach has its own ChaCha8 stream, numbered by `Direction::index()`, and an approach with mean 0 never draws. The turn draw is always made, even when only one movement has a ratio above 0.

The generator is separate from `Simulation`: the caller draws the arrivals of step `t`, spawns them, then steps. Nothing is dropped and there is no backlog cap.

## Consequences

### Positive

- One file describes a whole run, and `to_config()` records the demand.
- Runs are reproducible, and a snapshot test pins the sampler, the stream layout and the step rule together.
- Changing one approach's demand leaves the arrivals of the others identical.
- No dependency on `rand` or `rand_distr`, so an upgrade cannot change the arrivals silently.
- `Simulation` stays free of randomness, and a clone of `Demand` can fork a run.
- Spillback stays visible to metrics, because no arrival is dropped.

### Negative

- Demand is homogeneous: constant flow and fixed ratios, with no rush-hour profiles (10.1) or routes (6.2).
- A flow above 10 arrivals per approach per step is rejected.
- We own and maintain a sampler, and its tests.
- Backlogs are unbounded under overload, so memory grows with a long red and heavy demand.
- The caller must draw and spawn the arrivals itself; there is no public run loop until 1.6.
