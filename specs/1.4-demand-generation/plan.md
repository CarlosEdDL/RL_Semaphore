# 1.4 Demand generation: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feature/demand-generation`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Dependencies
- [ ] Add `rand_chacha = { version = "…", default-features = false }` and `insta = "1"` to `[workspace.dependencies]`, then `rand_chacha.workspace = true` in `sim`'s `[dependencies]` and `insta.workspace = true` in its `[dev-dependencies]` ([R1.1–R1.2](requirements.md#r1-dependencies-and-crate-layout)).
- [ ] Run `cargo deny --locked check` and `cargo audit` right away, before writing code, so a license or `unmaintained` problem shows up first (R1.3).
- [ ] Check that `cargo tree -p rl-semaphore-sim` shows no `rand`, `rand_distr` or `getrandom` (R1.1).

### 2. Config types
- [ ] Add `DemandConfig { north, east, south, west: Option<ApproachDemandConfig> }` and `ApproachDemandConfig { veh_per_h, left, through, right }`, both `#[serde(deny_unknown_fields)]`, ratios `#[serde(default)]` ([R2.1–R2.3](requirements.md#r2-demand-config)).
- [ ] Add `demand: DemandConfig` to `ScenarioConfig` with `#[serde(default)]`, and skip it when serializing if empty, so round trips of 1.1–1.3 configs do not change (R2.5).
- [ ] Re-export the new types from `lib.rs` (R1.4).

### 3. Validation
- [ ] Add `ConfigError::{InvalidFlow, InvalidTurnRatio, TurnRatioSum, FlowTooHigh}` with rustdoc, and extend the validation-order doc on `ConfigError` ([R3](requirements.md#r3-demand-validation)).
- [ ] Add `MAX_MEAN_ARRIVALS_PER_STEP = 10.0`.
- [ ] Write `validate_demand(&DemandConfig, step_s, &Intersection) -> Result<DemandPlan, ConfigError>` and call it last in `ScenarioConfig::validate`. Reuse `MovementNotInGeometry` for ratios on movements no lane allows (R3.4).
- [ ] Add `DemandPlan` (per approach: flow, ratios by `Movement::index()`, `λ`) and `Scenario::demand()`. Store the config for `to_config()` (R2.5, R3.8).

### 4. Example config
- [ ] Add `[demand]` to `configs/single-intersection.toml` and document it in the schema comment (R2.4). Suggested values, well below capacity:
  ```toml
  [demand]
  north = { veh_per_h = 400.0, left = 0.2, through = 0.7, right = 0.1 }
  south = { veh_per_h = 400.0, left = 0.2, through = 0.7, right = 0.1 }
  east  = { veh_per_h = 150.0, left = 0.2, through = 0.6, right = 0.2 }
  west  = { veh_per_h = 150.0, left = 0.2, through = 0.6, right = 0.2 }
  ```
- [ ] Check that the 1.1–1.3 tests that include this file still pass (they read geometry and signal only) (R1.5).

### 5. Sampler
- [ ] In a new `demand` module, add the private helpers `uniform(&mut ChaCha8Rng) -> f64` (R4.3) and `poisson(&mut ChaCha8Rng, lambda: f64) -> u32` by inversion with the `p_k` recurrence and an iteration cap (R4.4).
- [ ] Return 0 without touching the stream when `lambda == 0.0`.
- [ ] Unit tests for the sampler in the same module ([R7.1](requirements.md#r7-tests)).

### 6. Generator
- [ ] Add `Demand { seed, plan: DemandPlan (or the per-approach λ and cumulative ratios), rngs: [ChaCha8Rng; 4] }`, deriving `Clone` and `Debug` ([R4.1–R4.2, R4.7](requirements.md#r4-the-demand-generator)).
- [ ] Precompute, per approach, `λ` and the cumulative ratios over `Movement::ALL` in `new`, so `arrivals` does no division or validation (R6.3).
- [ ] `arrivals()`: per approach in `Direction::ALL` order, draw the count, then one turn draw per arrival (R4.4–R4.6). Decide whether the single-movement case skips the turn draw, document it, and keep it that way (R4.5).
- [ ] Rustdoc on `Demand` and `arrivals` with the stream layout and the loop order, plus a doctest that loads the example scenario and runs a few steps (R5.1).

### 7. Tests
- [ ] `crates/sim/tests/demand.rs`: config cases of R7.2 (small inline scenarios; extend `common::inline` or add a sibling that takes a `[demand]` string) and generator cases of R7.3.
- [ ] Add a `run_with_demand(sim, demand, steps, command_fn)` helper to `tests/common` that spawns arrivals, steps, and records snapshots for the 1.3 invariant checks (R5.3).
- [ ] `crates/sim/tests/demand_snapshot.rs`: the 600-step trajectory of R7.4. Run `cargo insta test` (or `INSTA_UPDATE=always cargo nextest run`) once, review the `.snap` file by hand (counts should look like ~0.3 arrivals per step with the example demand), and commit it.
- [ ] `crates/sim/tests/demand_props.rs`: the property test of R7.5. Keep flows bounded (for example at most 600 veh/h on north/south and 250 veh/h on east/west) so backlogs stay small and the run time stays close to the 1.3 property tests.

### 8. ADR-0005
- [ ] Copy `specs/adr/0000-template.md` to `specs/adr/0005-demand-model.md` and fill it in ([R8.1](requirements.md#r8-adr-and-roadmap)).
- [ ] Add it to the index table in `specs/adr/README.md` (R8.2).
- [ ] Fix the `sim` row in `specs/tech-stack.md` to list `rand_chacha` instead of `rand` (R8.3).

### 9. Roadmap
- [ ] Mark 1.4 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R8.4).

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
cargo tree -p rl-semaphore-sim -e normal
CI=true cargo nextest run -p rl-semaphore-sim --locked   # insta must not write snapshots in CI
cargo doc -p rl-semaphore-sim --no-deps
```

## Notes and risks

- **`rand_chacha` status.** The tech stack names `rand_chacha`, but newer `rand` releases may move their ChaCha generator to another crate, and `deny.toml` sets `unmaintained = "all"`. If `cargo deny` flags it, pick the maintained ChaCha8 crate that `rand` itself uses, keep the same stream layout (seed from `u64`, one stream per approach), and record the choice in ADR-0005. Whatever crate is chosen, check its docs promise value stability for the generator, since the snapshot depends on it.
- **`seed_from_u64` stability.** `rand_core` documents `seed_from_u64` as portable, but it is still a dependency's code between the seed and the stream. If a future upgrade changes it, the snapshot catches it; do not "fix" the snapshot without reading why it changed.
- **Inversion sampler and floating point.** For `λ` near the cap, `e^-λ` is about `4.5e-5`, fine for `f64`, but the running sum can stop just below a `U` close to 1. The iteration cap stops the walk; return the last `k` reached. Test `λ = 10` for mean and variance to make sure the cap never bites in practice.
- **Draw order is part of the contract.** Counts and turn draws come from the same stream in a fixed order (count, then one turn draw per arrival). Changing that order, or skipping a draw for single-movement approaches after the snapshot is committed, changes every trajectory. Decide once (R4.5) and write it in the rustdoc.
- **Stream independence depends on `λ = 0` not drawing.** If a zero-flow approach consumed draws, it still would not affect other approaches (separate streams), but it would make "adding flow to an idle approach" change that approach's future in a surprising way. The rule "no draw when `λ = 0`" keeps it simple.
- **Statistical tests with fixed seeds.** A fixed seed makes them deterministic, but a bad tolerance can still fail on a lucky seed after an unrelated change to the draw order. Use 5 standard errors and at least 100,000 draws; if one fails after a legitimate change, check the tolerance math before changing the seed.
- **Snapshot size.** 600 steps at ~0.3 arrivals per step gives a few hundred lines, which stays reviewable. Do not snapshot per-cell occupancy; the arrivals, departures and final counters already pin the trajectory, because the step rule is deterministic.
- **Round trip of old configs.** Adding a `demand` field must not change the TOML output of scenarios without demand, or 1.1–1.2 round-trip tests may see a new `[demand]` table. Skip serializing it when empty.
- **Scope creep toward 1.5 and 1.6.** Do not add wait tracking, a public run loop, or a `simulate` command. The loop helper stays in `tests/common`.
- **Scope creep toward 10.1.** Do not add a time argument to `arrivals` or rate profiles. A time-varying generator can take the same seed and stream layout later.
