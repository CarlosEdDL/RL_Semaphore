# 1.4 Demand generation

**Stage:** 1 (Simulator, single intersection, no RL) · **Status:** ☐ not started · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R8) · [plan.md](plan.md) (tasks).

## Goal

Make traffic arrive by itself. At the end of this phase a scenario TOML can carry a `[demand]` section that gives, per approach, a flow in vehicles per hour and the turn ratios (left / through / right). It is validated with the rest of the scenario into a `DemandPlan`. A `Demand` generator, built from the scenario and a `u64` seed, draws a Poisson number of arrivals per approach for every step and assigns each one a movement by the turn ratios. The caller spawns those arrivals into the 1.3 `Simulation` before stepping it. The same seed always gives the same arrivals, and an `insta` snapshot pins one full trajectory of the example scenario, so any change to the sampler, the stream layout, or the step rule shows up as a snapshot diff.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-dependencies-and-crate-layout) | `rand_chacha` in `sim`, `insta` as a dev-dependency, a new `demand` module |
| [R2](requirements.md#r2-demand-config) | The `[demand]` TOML section: flow per approach and turn ratios |
| [R3](requirements.md#r3-demand-validation) | Validation into `DemandPlan`, new `ConfigError` variants |
| [R4](requirements.md#r4-the-demand-generator) | `Demand`: per-approach ChaCha8 streams, Poisson counts, turn choice |
| [R5](requirements.md#r5-using-demand-with-the-simulation) | How arrivals are spawned into the `Simulation`, and the time convention |
| [R6](requirements.md#r6-determinism-and-robustness) | Determinism, no panics, clippy pedantic |
| [R7](requirements.md#r7-tests) | Sampler statistics, config tests, stream independence, the `insta` trajectory snapshot |
| [R8](requirements.md#r8-adr-and-roadmap) | ADR-0005 and roadmap |

## Decisions taken

- **Demand lives in the scenario TOML.** A new `[demand]` section sits next to `[intersection]` and `[signal]`, so one file describes a whole run and `Scenario::to_config()` records it. The section and each approach in it are optional: a missing approach has no demand, and a scenario with no `[demand]` has no demand at all. So every scenario written for 1.1–1.3 still loads unchanged.
- **Flow per approach plus turn ratios.** Each approach gives `veh_per_h` and the fractions `left`, `through`, `right` (missing ones are 0). The fractions must sum to 1, and a fraction above 0 is only allowed for a movement some lane of that approach allows. So every generated arrival can be spawned.
- **Poisson per step, with a sampler we own.** For each approach and step the number of arrivals is Poisson with mean `λ = veh_per_h × step_s / 3600`. Several arrivals may come in one step; the 1.3 backlog absorbs them. Each arrival then draws its movement from the turn ratios (a Poisson split, so each movement's arrivals are Poisson too). The sampler is a small inversion routine over uniforms built from `ChaCha8Rng` output. We do not use `rand_distr`, so a dependency upgrade can never silently change the arrivals, and we do not depend on `rand` itself.
- **One random stream per approach.** Every approach has its own `ChaCha8Rng`, seeded with the run seed and set to the stream number of its approach (`Direction::index()`). Changing the flow or ratios of one approach leaves the arrivals of every other approach identical, which keeps comparisons between demand configs clean.
- **The generator is separate from the simulation.** `Demand::new(&scenario, seed)` owns the random state. Each call to `Demand::arrivals()` returns the arrivals of one step, and the caller spawns them with `Simulation::spawn` and then calls `step`. `Simulation` stays free of randomness, every 1.3 test stays as it is, and time-varying demand (10.1) can swap in a different generator. `Demand` is `Clone`, so a run can be forked and replayed.
- **Arrivals spawn at the step they are drawn for.** At step `t` the caller draws arrivals, spawns them (their spawn step is `t`), then calls `step`. That is the only supported order, and it is documented on `Demand::arrivals`.
- **No backlog cap.** Demand never drops vehicles: every arrival is spawned, and spillback stays visible to 1.5 metrics. Validation only rejects absurd flows (a mean of more than 10 arrivals per approach per step), which bounds the sampler's work.

## Out of scope

- Time-varying demand and rush-hour profiles (10.1), and origin–destination demand with routes (6.2).
- A backlog cap or rejected-demand counter.
- Wait, delay, throughput and queue metrics (1.5).
- The fixed-time controller and the `simulate` CLI (1.6). Tests drive the loop themselves.
- Vehicle types, platoons, or any arrival process other than Poisson.
- Seeding from the OS or the clock: every seed is an explicit `u64`.

## Acceptance criteria

1. `configs/single-intersection.toml` has a documented `[demand]` section and still loads. A scenario without `[demand]` loads with zero demand, and every 1.1–1.3 test passes unchanged. (R2, R3)
2. Invalid demand (negative or non-finite flow, ratio outside 0–1, ratios not summing to 1, a ratio above 0 for a movement no lane allows, a mean above 10 arrivals per step) is rejected with a `ConfigError` naming the field path. `to_config()` round-trips the demand section. (R3)
3. `Demand::new(&scenario, seed)` generates arrivals that only use allowed movements, and every one of them spawns without error into a `Simulation` of the same scenario. (R4, R5)
4. Sampler tests show, for several means including 0, a sample mean and variance matching `λ` within tight tolerances, and a frequency of 0, 1 and 2 arrivals matching the Poisson probabilities. Turn shares match the configured ratios. (R7)
5. The same seed gives identical arrivals; different seeds give different arrivals; changing one approach's demand leaves the other approaches' arrivals identical. (R4, R7)
6. An `insta` snapshot of the example scenario under a fixed seed and command script (arrivals and departures per step, then the final counters) is committed, and CI fails if it changes. (R7)
7. `sim` still has no `rand`/`rand_distr`, no OS randomness, no wall-clock time and no `HashMap` iteration; it never panics and passes clippy pedantic; the WASM build still passes. (R1, R6)
8. `specs/adr/0005-demand-model.md` exists with status `Accepted`, the ADR index lists it, CI is green, and roadmap entry 1.4 is marked ☑. (R8)
