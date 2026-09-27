# 1.2 Signal model: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feat/1.2-signal-model`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Dependencies
- [x] Add `proptest = "1"` to `[workspace.dependencies]` and to `[dev-dependencies]` of `crates/sim` with `.workspace = true` ([R1.1](requirements.md#r1-dependencies)).
- [x] Run `cargo deny --locked check` (R1.3).

### 2. Movements and conflicts
- [x] Add `MovementId { approach, movement }` with `ALL`, `index()`, and `Display` (for example `north.left`) ([R2.1](requirements.md#r2-movements-and-the-conflict-matrix)). It can live in `road.rs` next to `Direction` and `Movement`, or in a new `signal` module.
- [x] Add a small rotation helper on `Direction` (for example `rotate_cw(self, n)`), and reuse it in `destination` if it simplifies that.
- [x] Implement `conflicts_with` with the relative-position rule of R2.4: compute whether `other.approach` is opposing, left-side, or right-side of `self.approach`, then look up a 3×3 table (R2.2–R2.4).
- [x] Put the R2.4 table in rustdoc.

### 3. Scenario config and time
- [x] Create a `scenario` module with `ScenarioConfig { step_s, intersection: IntersectionConfig, signal: SignalConfig }`, `SignalConfig { yellow_s, all_red_s, min_green_s, max_red_s, phases: Vec<PhaseConfig> }`, `PhaseConfig { name, green: GreenConfig }`, and `GreenConfig { north, east, south, west }` with `#[serde(default)]` on each `Vec<Movement>` field ([R3.1](requirements.md#r3-scenario-config-and-time)).
- [x] Add `skip_serializing_if = "Vec::is_empty"` on the `GreenConfig` fields so round-tripped TOML stays tidy (R3.6).
- [x] Write a `to_steps(value_s, step_s, Rounding::Up | Rounding::Down) -> Option<u32>` helper with the `1e-9` snap and range checks (R3.4).
- [x] `Scenario { step_s, intersection, signal_plan }` with `from_toml_str`, `TryFrom<ScenarioConfig>`, and `to_config` (R3.5, R3.6).

### 4. Validation
- [x] Add the new `ConfigError` variants, for example `InvalidStepLength`, `InvalidDuration { path, value }`, `PhaseCount { path, count }`, `EmptyPhaseName { path }`, `DuplicatePhaseName { path, name }`, `EmptyPhase { path }`, `DuplicatePhaseMovement { path, movement }`, `MovementNotInGeometry { path, movement }`, `ConflictingMovements { path, first, second }`, `DuplicatePhase { path, other }`, `UncoveredMovement { movement }`, `MaxRedTooShort { required_s, required_steps, got_steps }` ([R4](requirements.md#r4-signal-plan-validation)).
- [x] Prefix intersection errors with `intersection.` when validating a scenario. The simplest way is to pass a path prefix into the 1.1 validation, keeping the bare-intersection paths unchanged (R3.7).
- [x] Implement the checks in the fixed order of R4.
- [x] `MAX_PHASES = 8`, exported from the crate root.

### 5. Signal state machine
- [x] `SignalPlan` (validated, immutable) and `PhaseId` ([R5.1](requirements.md#r5-signal-state-machine)).
- [x] `Signal`, `Command`, `StepOutcome` (with an `IgnoredReason` enum), `Light`, and a public `SignalState` enum (`Green { phase, elapsed }`, `Yellow { from, to, elapsed }`, `AllRed { from, to, elapsed }`) (R5.2, R5.3).
- [x] Keep per-phase red ages in a `Vec<u32>` indexed by `PhaseId`, and per-movement red runs in `[u32; 12]`. Store phase membership as a 12-bit mask (`u16`) so `p ∩ k` and conflict checks are bit operations.
- [x] Transition logic with the overlap rule (R5.4, R5.5). Document the step conventions in rustdoc (R5.6).

### 6. Max-red enforcement
- [x] A private `schedule_is_feasible(first: PhaseId) -> bool` that sorts the other phases by red age (descending, ties by `PhaseId`) and checks each deadline with the formula of R6.3. With at most 8 phases this is cheap enough to run every step.
- [x] Use it both for the admission check (R6.3) and the forced switch (R6.4), and base `can_switch_to` on the same code path as `step` (R6.5).
- [x] Handle the single-phase plan (R6.6).

### 7. Example config and tests
- [x] Update `configs/single-intersection.toml` to the scenario layout and extend its comment header ([R7.1](requirements.md#r7-example-config-and-tests)).
- [x] Update `crates/sim/tests/road.rs` to take the example's `intersection` table, for example by parsing the scenario, or with `toml::Value` if you want the road tests to stay independent of the scenario module (R7.4).
- [x] Add `crates/sim/tests/signal.rs` (or `conflicts.rs`, `scenario.rs`, `signal.rs`) with the unit tests of R7.2.
- [x] Add property tests (R7.3). Write the invariant checks as a function over a recorded `Vec<[Light; 12]>` trace, so unit tests can reuse them. Generate timings in steps directly and derive seconds with `step_s = 1.0`, keeping only those that pass R4.10.

### 8. ADR-0003
- [x] Copy `specs/adr/0000-template.md` to `specs/adr/0003-signal-safety-model.md` and fill it in ([R8.1](requirements.md#r8-adr-docs-and-roadmap)).
- [x] Add it to the index table in `specs/adr/README.md` (R8.2).

### 9. Docs and roadmap
- [x] Update the `configs/` line in the README (R8.3).
- [x] Mark 1.2 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R8.4).

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
PROPTEST_CASES=2000 cargo nextest run -p rl-semaphore-sim --locked
cargo doc -p rl-semaphore-sim --no-deps
```

## Notes and risks

- **Off-by-one in step conventions.** Most bugs will be here: whether the step that receives a switch shows yellow, and whether red ages count the current step. Pick one convention, write it in rustdoc, and pin it with a hand-traced unit test (for example `Y = 2`, `A = 1`, `G = 3`) before writing the property tests.
- **Feasibility bound vs. forcing rule.** The R4.10 bound assumes the forced-switch schedule in R6.3–R6.4. If a property test finds a max-red violation, fix the rule, not the bound, and record the change in ADR-0003. The EDF argument: all phases have the same minimum service (`G + Y + A`), so serving the longest-red phase first is optimal, and forcing exactly when the schedule becomes tight keeps it feasible by induction.
- **Phase-level vs. movement-level red.** The guarantee is on phases; movements that belong to several phases are served more often. The property test checks movements, which is the observable contract.
- **Float rounding in durations.** `0.9 / 0.3` is `3.0000000000000004`, which `ceil` turns into 4. The `1e-9` snap avoids this; test it explicitly.
- **Config schema change.** Moving the geometry under `[intersection]` breaks the 1.1 layout of the example file. Nothing outside `sim` tests reads it yet, so this is the cheapest moment to change it.
- **Error paths inside the intersection.** The scenario path is `intersection.approaches.north...`, while the bare 1.1 path is `approaches.north...`. Keep both correct and test one prefixed case.
- **Scope creep toward 1.3.** It is tempting to add "can the vehicle at the head of this lane go". Keep lane-level logic for 1.3. This phase stops at per-movement lights.
- **Scope creep toward 3.3.** Do not build the action mask here. `can_switch_to` is the whole interface the env needs.
- **proptest run time.** 1,000+ steps × 256 cases is fast for a 12-movement state machine, but keep the step count bounded so CI stays quick. Seed failures are saved in `proptest-regressions/`; commit that directory if it appears.
