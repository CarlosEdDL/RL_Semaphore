# 1.4 Demand generation: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

Notation: `t` is the simulation step count. For an approach `d`, `q_d` is its flow in veh/h, `r_d(m)` its turn ratio for movement `m`, and `λ_d = q_d × step_s / 3600` its mean number of arrivals per step. `U` is a uniform draw in `[0, 1)`.

## R1. Dependencies and crate layout

- **R1.1** `rand_chacha` MUST be added as a workspace dependency with `default-features = false` and used by `sim`. `sim` MUST use only its `ChaCha8Rng` and the `rand_core` traits it re-exports (`SeedableRng`, `RngCore`). `sim` MUST NOT depend on `rand`, `rand_distr`, or `getrandom`, and MUST NOT enable any feature that seeds from the OS.
- **R1.2** `insta` MUST be added as a workspace dev-dependency and used by `sim`'s tests. Snapshot files MUST be committed under `crates/sim/tests/snapshots/`.
- **R1.3** `cargo deny check` MUST pass with the new packages (licenses, advisories including `unmaintained`, sources). If `rand_chacha` is flagged as unmaintained, the plan's fallback applies (see [plan.md](plan.md#notes-and-risks)).
- **R1.4** Demand types SHOULD live in a new `demand` module. New public types MUST be re-exported from the crate root.
- **R1.5** The 1.1–1.3 tests MUST keep passing unchanged. The `Simulation` API MUST NOT change.

## R2. Demand config

- **R2.1** `ScenarioConfig` MUST gain a `demand: DemandConfig` field that defaults to empty when the `[demand]` table is missing (`#[serde(default)]`). `DemandConfig` MUST have one optional field per approach (`north`, `east`, `south`, `west`) and reject unknown keys.
- **R2.2** Each approach entry (`ApproachDemandConfig`) MUST have a required `veh_per_h: f64` and optional `left`, `through`, `right: f64` that default to 0. It MUST reject unknown keys.
- **R2.3** A missing approach entry MUST mean no arrivals on that approach.
- **R2.4** `configs/single-intersection.toml` MUST get a `[demand]` section with a flow for every approach and ratios using only movements its lanes allow, below the lanes' discharge capacity. The schema comment at the top of the file MUST document the section, its units, and its rules.
- **R2.5** `Scenario::to_config()` MUST return the demand section as configured, and a config serialized with `to_toml_string` and parsed again MUST validate to an equal `Scenario`.

## R3. Demand validation

- **R3.1** `ScenarioConfig::validate` MUST validate the demand section after all 1.2 checks, approaches in `Direction::ALL` order, and fields in the order `veh_per_h`, then `left`, `through`, `right`, then the sum and the per-step mean. Validation stops at the first error, and the order MUST be documented on `ConfigError`.
- **R3.2** `veh_per_h` MUST be finite and `>= 0`. Otherwise: `ConfigError::InvalidFlow { path, value }`.
- **R3.3** Each ratio MUST be finite and within `[0, 1]`. Otherwise: `ConfigError::InvalidTurnRatio { path, value }`.
- **R3.4** A ratio above 0 for a movement that no lane of the approach allows MUST be rejected with the existing `ConfigError::MovementNotInGeometry { path, movement }`.
- **R3.5** The ratios of an entry MUST sum to 1 within `1e-9`. Otherwise: `ConfigError::TurnRatioSum { path, sum }`, where `path` names the approach entry. This applies even when `veh_per_h` is 0, so a written entry is always well-formed.
- **R3.6** `λ_d` MUST be at most `MAX_MEAN_ARRIVALS_PER_STEP = 10.0` (a public constant). Otherwise: `ConfigError::FlowTooHigh { path, veh_per_h, step_s, mean_per_step, max }`.
- **R3.7** Paths MUST follow the existing style, for example `demand.north.veh_per_h` and `demand.west.left`. New variants MUST have rustdoc and error messages that name the path and the rule.
- **R3.8** The validated result MUST be a `DemandPlan`, available through `Scenario::demand()`. It MUST expose per approach the flow in veh/h, the ratio for each movement, and `λ_d` (`mean_per_step(direction)`). An approach with no entry MUST report flow 0 and all ratios 0.

## R4. The demand generator

- **R4.1** `Demand::new(scenario: &Scenario, seed: u64) -> Demand` MUST build a generator from the scenario's `DemandPlan`. It MUST expose the seed it was built with.
- **R4.2** Random streams: the generator MUST hold one `ChaCha8Rng` per approach, built with `ChaCha8Rng::seed_from_u64(seed)` and then `set_stream(d.index() as u64)`. All draws for approach `d` MUST come from its own stream only.
- **R4.3** Uniforms: `U` MUST be computed as `(next_u64() >> 11) as f64 × 2^-53`, so it is exact and in `[0, 1)`. Neither `rand`'s `Standard` distribution nor any other library routine may be used for it.
- **R4.4** Counts: each call MUST draw, per approach in `Direction::ALL` order, a count `N_d ~ Poisson(λ_d)` by inversion: one `U`, then walk `k = 0, 1, 2, …` adding `P(N = k)` (computed by the recurrence `p_0 = e^-λ`, `p_k = p_(k-1) × λ / k`) until the running sum exceeds `U`. The walk MUST stop at a fixed cap (at least 100) even if floating-point error keeps the sum below `U`. When `λ_d = 0`, the stream MUST NOT be advanced.
- **R4.5** Turns: for each of the `N_d` arrivals, the generator MUST draw one `U` from the same stream and pick the first movement, in `Movement::ALL` order, whose cumulative ratio exceeds `U`. If rounding leaves no match, it MUST pick the last movement with a ratio above 0. A movement with ratio 0 MUST never be picked. When only one movement has a ratio above 0, the draw MAY be skipped; the choice MUST be documented and fixed by the snapshot.
- **R4.6** `Demand::arrivals(&mut self) -> Vec<MovementId>` MUST return the arrivals of one step: approaches in `Direction::ALL` order, and within an approach in draw order.
- **R4.7** `Demand` MUST be `Clone` and `Debug`. A clone MUST continue with exactly the same arrivals as the original.
- **R4.8** The sampler SHOULD be a private function with its own unit tests, so it can be tested without a scenario.

## R5. Using demand with the simulation

- **R5.1** The rustdoc on `Demand::arrivals` MUST state the loop order: at step `t`, draw arrivals, spawn each with `Simulation::spawn(m.approach, m.movement)` (so its spawn step is `t`), then call `Simulation::step`. It SHOULD include a short example (a doctest).
- **R5.2** For a `Demand` and a `Simulation` built from the same scenario, `spawn` MUST never fail for a generated arrival (R3.4 guarantees a lane exists).
- **R5.3** A helper that runs the loop for a fixed number of steps SHOULD live in `crates/sim/tests/common` for the tests, not in the public API. The public loop driver belongs to 1.6.

## R6. Determinism and robustness

- **R6.1** Arrivals MUST depend only on the `DemandPlan`, the seed, and the number of previous calls. No wall-clock time, OS randomness, or `HashMap` iteration.
- **R6.2** `Demand::new` and `Demand::arrivals` MUST NOT panic for any validated scenario and any seed. Casts follow the 1.1 rule: range-check first, then a local `#[allow(clippy::cast_*)]` with an `// INVARIANT:` comment.
- **R6.3** `arrivals` SHOULD cost `O(approaches + arrivals)` and allocate only the returned vector.
- **R6.4** Clippy pedantic MUST pass, and the WASM build of `web` MUST still pass.

## R7. Tests

- **R7.1** Sampler unit tests (in the `demand` module) MUST cover:
  - `λ = 0` always gives 0 and does not advance the stream;
  - for `λ ∈ {0.05, 0.5, 2.0, 10.0}` and a fixed seed, over at least 100,000 draws, the sample mean and the sample variance are both within 5 standard errors of `λ`;
  - for `λ = 0.5`, the frequencies of 0, 1 and 2 arrivals are within 5 standard errors of the Poisson probabilities;
  - uniforms are in `[0, 1)`.
- **R7.2** Config tests (in `crates/sim/tests/demand.rs`) MUST cover:
  - the example scenario loads with the configured demand, and `mean_per_step` matches `veh_per_h × step_s / 3600`;
  - a scenario without `[demand]` loads with zero demand, and one with only some approaches listed gives zero for the others;
  - every new error (R3.2–R3.6) and `MovementNotInGeometry` for a ratio, each with its exact path, plus the validation order where two errors are present;
  - the `to_config` / TOML round trip of R2.5.
- **R7.3** Generator tests MUST cover:
  - determinism: two generators with the same seed give identical arrivals for 10,000 steps, and a clone taken mid-run continues identically;
  - different seeds give different arrivals;
  - stream independence: changing the flow and ratios of one approach leaves the arrivals of every other approach identical;
  - only allowed movements appear, and an approach with zero flow never appears;
  - turn shares over many arrivals are within 5 standard errors of the configured ratios;
  - every arrival spawns into a `Simulation` of the same scenario without error (R5.2).
- **R7.4** A determinism snapshot test MUST run the example scenario with a fixed seed for 600 steps, with `Command::Hold` every step (max-red makes the signal cycle), and `insta::assert_snapshot!` a text trajectory: one line per step that has arrivals or departures (step, arrivals with their vehicle ids and movements, departures with their vehicle ids), then the final counters (spawned, in backlog, on lane, departed). The format MUST be plain text built by the test (no serde feature of `insta` needed) and stable to read in a diff.
- **R7.5** A property test SHOULD run arbitrary valid demand (flows up to a bound below capacity, ratios over allowed movements) and arbitrary seeds through the simulation for at least 1,000 steps, and check the 1.3 invariants (reusing `tests/common`) plus: every arrival spawned, and spawned = total arrivals.
- **R7.6** Statistical tests MUST use fixed seeds, so they are deterministic and cannot flake.

## R8. ADR and roadmap

- **R8.1** `specs/adr/0005-demand-model.md` MUST follow the template and record: demand in the scenario TOML, flow per approach with turn ratios, Poisson counts per step drawn by an owned inversion sampler, one ChaCha8 stream per approach, a generator separate from `Simulation`, and no backlog cap. Its Context MUST name the alternatives considered (a separate demand file, per-movement flows, `rand_distr::Poisson`, Bernoulli arrivals, a single shared RNG, `Simulation` owning the RNG, a backlog cap with dropped arrivals). Its Consequences MUST include the costs (homogeneous demand only, the 10 arrivals-per-step limit, owning a sampler, unbounded backlogs under overload, the caller must spawn arrivals itself) and the benefits.
- **R8.2** Status `Accepted`, date of the PR. The index table in `specs/adr/README.md` MUST list it.
- **R8.3** `specs/tech-stack.md` SHOULD be corrected where it lists `rand` for `sim`, to say `rand_chacha` only.
- **R8.4** Roadmap entry 1.4 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.
