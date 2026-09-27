# 1.2 Signal model: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

## R1. Dependencies

- **R1.1** `[workspace.dependencies]` MUST declare `proptest` (major version only). `crates/sim/Cargo.toml` MUST list it under `[dev-dependencies]` with `.workspace = true`.
- **R1.2** No new runtime dependency is needed. `sim` MUST still have no `anyhow`, I/O, async, or randomness crate.
- **R1.3** `cargo deny --locked check` MUST pass, and `Cargo.lock` MUST be committed. A new license MAY be added to `deny.toml` only if it is permissive, with a comment naming the crate.

## R2. Movements and the conflict matrix

- **R2.1** A `MovementId` MUST identify a movement by its approach `Direction` and its `Movement`. It MUST be `Copy`, `Eq`, `Ord`, `Hash`, `Debug`, and serde-serializable, and MUST provide `MovementId::ALL` (12 entries, approaches in `Direction::ALL` order, then movements in `Left, Through, Right` order) and a dense `index()` in `0..12` matching that order, so per-movement state can live in `[T; 12]` arrays.
- **R2.2** There MUST be a pure function `MovementId::conflicts_with(self, other) -> bool` for right-hand traffic with protected turns only. Two movements conflict if their paths cross or they exit to the same side.
- **R2.3** The relation MUST be irreflexive and symmetric, and two movements from the same approach MUST NOT conflict.
- **R2.4** Name the other approaches relative to approach `d`: *opposing* is the approach across (`d` rotated by 2), *left-side* is the approach on the driver's left (`d` rotated one step clockwise in N, E, S, W order, for example East for North), and *right-side* is the approach on the driver's right (`d` rotated three steps, for example West for North). The conflicts of a movement from `d` MUST be exactly:

  | Movement from `d` | Opposing | Left-side | Right-side |
  |---|---|---|---|
  | `Left` | `Through`, `Right` | `Through`, `Left` | `Through`, `Left` |
  | `Through` | `Left` | `Through`, `Left` | `Through`, `Left`, `Right` |
  | `Right` | `Left` | `Through` | none |

  This gives 14 conflicts per approach and 28 unordered conflicting pairs in total. Rustdoc MUST include this table. The implementation SHOULD use the relative-position rule (rotation) rather than a 144-entry literal, but tests MUST check all 144 ordered pairs.

## R3. Scenario config and time

- **R3.1** A raw `ScenarioConfig` MUST mirror this schema, with `#[serde(deny_unknown_fields)]` on every struct and no defaults except where stated:

  ```toml
  step_s = 1.0

  [intersection]
  cell_length_m = 7.5
  [intersection.approaches.north]
  length_m = 150.0
  lanes = [ { movements = ["left"] }, { movements = ["through", "right"] } ]
  # ...east, south, west as in 1.1

  [signal]
  yellow_s = 3.0
  all_red_s = 2.0
  min_green_s = 5.0
  max_red_s = 90.0

  [[signal.phases]]
  name = "ns-left"
  green = { north = ["left"], south = ["left"] }

  [[signal.phases]]
  name = "ns-through"
  green = { north = ["through", "right"], south = ["through", "right"] }
  ```

  `intersection` MUST reuse `IntersectionConfig` from 1.1 unchanged. Inside a phase's `green` table, each of `north`, `east`, `south`, `west` is optional and defaults to no movements; other keys MUST be rejected.
- **R3.2** The first phase in the list is the initial phase, shown green at step 0.
- **R3.3** `step_s` MUST be finite and strictly positive.
- **R3.4** Durations MUST be converted to whole steps once, at load time. `yellow_s`, `all_red_s`, and `min_green_s` MUST round up (`ceil(value / step_s)`), and `max_red_s` MUST round down (`floor(value / step_s)`). Before rounding, a quotient within `1e-9` of an integer MUST snap to that integer, so `0.9 / 0.3` gives 3 steps, not 4. Converted counts MUST fit in `u32`. Casts follow the 1.1 rule: range-check first, then a local `#[allow(clippy::cast_*)]` with an `// INVARIANT:` comment.
- **R3.5** Parsing MUST be a function over `&str` (for example `ScenarioConfig::from_toml_str`), and validation MUST turn a `ScenarioConfig` into a validated `Scenario` (holding `step_s`, the `Intersection`, and a `SignalPlan`). `Scenario::from_toml_str` MAY combine both. `sim` MUST NOT open files.
- **R3.6** `Scenario::to_config` (or keeping the source config) MUST give back the config, and `config → TOML → config → Scenario` MUST give an equal `Scenario`. Phase `green` lists MUST serialize in a fixed order (approaches in `Direction::ALL` order, movements sorted).
- **R3.7** `IntersectionConfig::from_toml_str` and `Intersection::from_toml_str` from 1.1 MUST keep working on a bare intersection document.

## R4. Signal plan validation

Validation MUST stop at the first error and check in this fixed order: `step_s`, the intersection (as in 1.1, with paths prefixed by `intersection.`), timings in the order `yellow_s`, `all_red_s`, `min_green_s`, `max_red_s`, then phases in list order (within a phase: name, then approaches in `Direction::ALL` order, then movements in list order), then coverage, then the max-red feasibility rule. New variants MUST be added to the existing `ConfigError` (already `#[non_exhaustive]`), each carrying the field path and showing it and the offending value in `Display`.

- **R4.1** `step_s` not finite or not strictly positive → error.
- **R4.2** Any duration not finite or negative → error. `yellow_s` and `min_green_s` MUST convert to at least 1 step; `all_red_s` MAY be 0 steps; `max_red_s` MUST convert to at least 1 step.
- **R4.3** The plan MUST have between 1 and `MAX_PHASES` phases, where the constant is 8.
- **R4.4** Phase names MUST be non-empty and unique within the plan.
- **R4.5** A phase MUST grant at least one movement and MUST NOT list the same movement twice for an approach.
- **R4.6** A phase MUST NOT grant a movement that no lane of that approach allows (for example `east = ["left"]` when the east lanes only allow `through`).
- **R4.7** A phase MUST NOT grant two conflicting movements. The error MUST name both.
- **R4.8** Two phases MUST NOT grant the same set of movements.
- **R4.9** Every movement allowed by some lane of the intersection MUST be granted by at least one phase. The error MUST name the first uncovered movement in `MovementId::ALL` order.
- **R4.10** With `n` phases, `G` = min-green, `Y` = yellow, `A` = all-red, and `M` = max-red, all in steps, the plan MUST satisfy `M ≥ n × (Y + A) + (n − 1) × G` (a phase that starts leaving green at step *t* is next green at *t* + `n(Y+A)` + `(n−1)G` at the earliest). It applies only when `n ≥ 2`. The error MUST show the required minimum in seconds and in steps.
- **R4.11** Validation MUST NOT panic on any input.

## R5. Signal state machine

- **R5.1** The validated `SignalPlan` MUST be immutable, with private fields and read-only accessors: its phases (name and granted movements), timings in steps, and a `PhaseId` for each phase. `PhaseId` MUST be a `Copy + Ord` newtype over the phase index.
- **R5.2** A runtime `Signal` MUST be built from a `SignalPlan` and start in green on the initial phase. Red ages count entries in which a phase is not green, so the initial phase has age 0 and every other phase has age 1 at step 0. It MUST expose at least:
  - `step(&mut self, command: Command) -> StepOutcome`, advancing exactly one time step;
  - `light(MovementId) -> Light` where `Light` is `Green`, `Yellow`, or `Red`, and `lights() -> [Light; 12]` indexed by `MovementId::index()`;
  - the current state: green on a phase (with steps elapsed), or a transition (from, to, yellow or all-red, steps elapsed);
  - `can_switch_to(PhaseId) -> bool`;
  - the red age of each phase and of each movement.
- **R5.3** `Command` MUST be `Hold` or `SwitchTo(PhaseId)`. `StepOutcome` MUST tell the caller what happened: held, switch started (to which phase), switch forced (to which phase, and that it overrode the command), or command ignored with a reason (min-green not reached, transition in progress, unknown phase, already on that phase, or max-red deadline). No command may cause a panic.
- **R5.4** A `SwitchTo(k)` while green on phase `p ≠ k` MUST start a transition only if `p` has been green for at least `min_green` steps and the admission check of R6.3 passes. Otherwise the signal holds and reports why.
- **R5.5** A transition from `p` to `k`: movements green in both `p` and `k` MUST stay green throughout. Movements green in `p` only MUST show yellow for exactly `yellow` steps, then red. Then all movements not in `p ∩ k` MUST be red for exactly `all_red` steps (none if 0). Then `k` is green. Commands received during a transition MUST be ignored.
- **R5.6** The step-count conventions (for example whether the step that receives a switch command already shows yellow) MUST be stated in rustdoc and pinned by unit tests. The observable guarantees in R6 and R7.3 are defined on the sequence of `lights()` after each `step`, including the initial state.
- **R5.7** `sim` MUST stay deterministic: no `HashMap`/`HashSet` iteration where order is observable, no randomness, no wall-clock time. Per-movement and per-phase state SHOULD live in fixed arrays or `Vec`s indexed by `MovementId::index()` and `PhaseId`.

## R6. Max-red enforcement

- **R6.1** A phase's red age is the number of consecutive entries, including the current one, in which it is not the green phase (counting from the step where its transition began, or from step 0 if it has never been green). It is 0 while the phase is green.
- **R6.2** The signal MUST guarantee that, for any command sequence, every phase becomes green again within `max_red` steps of its red age starting, and so no movement is shown red for more than `max_red` consecutive steps.
- **R6.3** Admission check: a switch from `p` to `k` MUST be accepted only if, after serving `k` for `min_green`, every other phase can still be reached in time when served in descending red-age order (ties by lowest `PhaseId`), with each switch costing `yellow + all_red` steps and each phase then held for `min_green`. Concretely, if the remaining phases are `j1, j2, …` in that order, `j_i` turns green at `now + 1 + (i + 1) × (Y + A) + i × G` (with `now` the current step and the `k` service included), and its red age at the entry before that MUST be at most `M`, that is `age(j_i) + (i + 1)(Y + A) + i·G ≤ M`. The requested phase `k` is checked too (`i = 0`).
- **R6.4** Forced switch: while green on `p` with at least `min_green` steps elapsed, if holding one more step would make the schedule of R6.3 (starting with the longest-red phase) infeasible, the signal MUST start a transition to the longest-red phase (ties by lowest `PhaseId`), whatever the command, and report it as forced.
- **R6.5** `can_switch_to(k)` MUST return exactly whether `step(SwitchTo(k))` would start a transition to `k` right now. The env mask in 3.3 relies on this.
- **R6.6** With a single phase, the signal stays green forever and every command other than `Hold` is ignored with a reason.

## R7. Example config and tests

- **R7.1** `configs/single-intersection.toml` MUST become a full scenario: `step_s`, the 1.1 geometry under `[intersection]`, and a `[signal]` table with four phases: north/south left turns, north/south through and right, east (all movements), and west (all movements). The side road needs split phases because its single shared lane carries left turns that conflict with opposing through traffic. Suggested timings: `step_s = 1.0`, `yellow_s = 3.0`, `all_red_s = 2.0`, `min_green_s = 5.0`, `max_red_s = 90.0`. The comment header MUST document the new keys, units, and rounding.
- **R7.2** Unit tests MUST cover:
  - the conflict matrix: all 144 ordered pairs against R2.4, symmetry, irreflexivity, same-approach pairs, and the count of 28;
  - loading the example scenario (through `include_str!`) and checking phase names, granted movements, and step counts;
  - each rule in R4 with a failing inline config, matching the `ConfigError` variant and field path;
  - the seconds-to-steps conversion, including rounding direction and the `1e-9` snap;
  - the round trip of R3.6;
  - the state machine cases in spec acceptance criterion 4.
- **R7.3** Property tests (`proptest`) MUST run the example plan, and plans with generated valid timings, for at least 1,000 steps under arbitrary command sequences, and check on the `lights()` trace:
  - no two conflicting movements are non-red at the same step;
  - every yellow run lasts exactly `yellow` steps, and every all-red interval exactly `all_red` steps;
  - every green phase lasts at least `min_green` steps before its transition;
  - no movement is red for more than `max_red` consecutive steps;
  - `can_switch_to(k)` agrees with the outcome of `step(SwitchTo(k))` on a cloned signal.
- **R7.4** The 1.1 road tests MUST keep passing. Tests that load the example file MUST read the `intersection` table of the scenario.

## R8. ADR, docs and roadmap

- **R8.1** `specs/adr/0003-signal-safety-model.md` MUST follow the template and record: protected-only phasing, config-defined phases validated against a fixed conflict matrix, enforcement inside the signal state machine (rather than only in the env mask), and the earliest-deadline-first max-red rule with its feasibility bound. Its Context MUST name the alternatives considered (permitted left turns, a fixed built-in phase plan, enforcement only in the env layer, forcing the next phase in cycle order). Its Consequences MUST include the costs (less realistic left-turn capacity, a shared-lane side road needs split phases, max-red may override the agent) and the benefits.
- **R8.2** Status `Accepted`, date of the PR. The index table in `specs/adr/README.md` MUST list it.
- **R8.3** The README line about `configs/` SHOULD say the example file now describes a full scenario (geometry and signal plan).
- **R8.4** Roadmap entry 1.2 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.
