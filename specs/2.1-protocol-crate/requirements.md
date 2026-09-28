# 2.1 Protocol crate: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

Notation: the *wire* is the JSON text produced by `ServerMessage::to_json`. A *DTO* is any public type of `protocol` that is serialized. The *example* is `configs/single-intersection.toml`. A *fixture* is a message built by hand in a test. `sim::X` names a type of `rl-semaphore-sim` and `X` alone names a type of `protocol`.

## R1. Crate layout and compatibility

- **R1.1** All code MUST live in `crates/protocol`. The crate SHOULD be split into modules: `lib.rs` (crate doc, `PROTOCOL_VERSION`, re-exports), `common.rs` (`Direction`, `Movement`, `Light`, `LaneRef`, `MovementRef`, `ByApproach`), `layout.rs` (R4), `snapshot.rs` (R5), `metrics.rs` (R6), `message.rs` (R3) and `from_sim.rs` (R7, compiled only with the `sim` feature). Module names MAY differ if the public API of R3–R7 is kept. Every public type MUST be re-exported from the crate root.
- **R1.2** With default features, the normal dependencies of `protocol` MUST be exactly `serde` (with `derive`), `serde_json` and `thiserror`, all through `[workspace.dependencies]`. `serde_json` MUST enable the `float_roundtrip` feature in `protocol`'s manifest. `rl-semaphore-sim` MUST be an optional dependency, enabled only by a feature `sim = ["dep:rl-semaphore-sim"]`, and `default` MUST be empty.
- **R1.3** `rl-semaphore-protocol` MUST be added to `[workspace.dependencies]` with `path` and `version`, like `rl-semaphore-sim`. No other crate gains a dependency on `protocol` in this phase.
- **R1.4** With default features, `protocol` MUST build for `wasm32-unknown-unknown`, and `cargo tree -p rl-semaphore-protocol -e normal` MUST NOT list `rl-semaphore-sim`.
- **R1.5** No source file under `crates/sim/src`, `crates/env/src` or `crates/cli/src` may change. Every existing snapshot MUST stay byte-identical, every existing test MUST keep passing, and no existing assertion may be weakened or removed.
- **R1.6** Code outside tests MUST NOT call `unwrap`, `expect`, `panic!`, or index a slice in a way that can panic. `#![warn(clippy::pedantic)]` MUST stay in `lib.rs`. Every public item MUST have a doc comment (`missing_docs` is on), and every fallible public function MUST have an `# Errors` section.
- **R1.7** `cargo deny --locked check` and `cargo audit` MUST pass. No new crate may enter the dependency tree (`serde`, `serde_json` and `thiserror` are already in it).

## R2. Wire conventions

- **R2.1** The wire format MUST be JSON, produced and parsed by `serde_json`. Every DTO MUST derive `Debug`, `Clone`, `PartialEq`, `Serialize` and `Deserialize`. Enums without data SHOULD also derive `Copy`, `Eq` and `Hash`.
- **R2.2** Field names MUST be snake_case, the Rust field names unchanged. Unit-like enum values MUST be serialized as snake_case strings (`"north"`, `"all_red"`). No DTO may use `#[serde(flatten)]` or `#[serde(untagged)]`.
- **R2.3** Units MUST be stated in the name and used consistently:
  - durations are `f64` seconds, with names ending in `_s`;
  - lengths are `f64` meters, with names ending in `_m`;
  - step counts, vehicle ids and vehicle counts are `u64`;
  - cell indices, cell counts, lane indices and phase indices are `u32`.
- **R2.4** Every `f64` on the wire MUST be finite. The conversions of R7 MUST only produce finite values. The crate doc MUST state that a non-finite value encodes as `null` and then fails to decode.
- **R2.5** Order MUST be deterministic, and the same value MUST always encode to byte-identical JSON:
  - approaches are fields of `ByApproach` (`north`, `east`, `south`, `west`, in that order);
  - lanes are ordered by approach (north, east, south, west), then by lane index;
  - vehicles are ordered by ascending id;
  - phases follow the order of the signal plan;
  - movements in a list are ordered north, east, south, west, then left, through, right (the order of `sim::MovementId::ALL`).

  No DTO may contain a `HashMap`, a `HashSet` or any other collection whose iteration order is not fixed.
- **R2.6** No DTO may use `#[serde(deny_unknown_fields)]`. Decoding MUST ignore fields it does not know. An `Option` field MUST be written as `null` when `None`, never omitted, unless it is a field added later under R2.7.
- **R2.7** `lib.rs` MUST define `pub const PROTOCOL_VERSION: u32 = 1;`. Its doc comment MUST state the versioning rule:
  - **Compatible (no bump):** adding a field with `#[serde(default)]`, or adding a `ServerMessage` variant.
  - **Breaking (bump):** removing, renaming or retyping a field, removing a variant, or changing a field's meaning, units or ordering.

## R3. Messages and handshake

- **R3.1** `ServerMessage` MUST be an enum with the variants `Hello(Hello)`, `Snapshot(Snapshot)` and `Metrics(Metrics)`. It is internally tagged with `#[serde(tag = "type", rename_all = "snake_case")]`, so the wire has `"type": "hello" | "snapshot" | "metrics"` next to the fields of the inner struct.
- **R3.2** `Hello` MUST have the fields `protocol_version: u32` and `layout: Layout`. `Hello::new(layout: Layout) -> Hello` MUST set `protocol_version` to `PROTOCOL_VERSION`.
- **R3.3** `Hello::check_version(&self) -> Result<(), ProtocolError>` MUST return `Err(ProtocolError::VersionMismatch { expected: PROTOCOL_VERSION, found })` when `protocol_version != PROTOCOL_VERSION`, and `Ok(())` otherwise.
- **R3.4** `ServerMessage::to_json(&self) -> Result<String, ProtocolError>` MUST return compact JSON (`serde_json::to_string`). `ServerMessage::from_json(text: &str) -> Result<ServerMessage, ProtocolError>` MUST parse it. A decode MUST fail on an unknown `type`, a missing required field, a value of the wrong type (including `null` where a float is expected), and trailing non-whitespace.
- **R3.5** `ProtocolError` MUST be a `thiserror` enum with at least:
  - `Encode(serde_json::Error)`, which has the `serde_json` error as its `source`;
  - `Decode(serde_json::Error)`, which has the `serde_json` error as its `source`;
  - `VersionMismatch { expected: u32, found: u32 }`.

  The messages MUST be lowercase and without a trailing period, like the examples in `CONTRIBUTING.md` and the existing errors in `sim`.
- **R3.6** The crate doc MUST describe the message order the server will follow: one `Hello` first on every connection, then any sequence of `Snapshot` and `Metrics`, each `Metrics` describing the same or an earlier step than the latest `Snapshot`. This phase defines the order and does not enforce it (2.2 does).
- **R3.7** There MUST be no client-to-server message type in this phase.

## R4. Layout

- **R4.1** `Layout` MUST describe one intersection with the fields:
  - `step_s: f64`;
  - `cell_length_m: f64`;
  - `approaches: ByApproach<ApproachLayout>`;
  - `signal: SignalLayout`.
- **R4.2** `ByApproach<T>` MUST be a generic struct with the fields `north`, `east`, `south` and `west`, each of type `T`, declared in that order. It MUST provide `get(&self, Direction) -> &T`.
- **R4.3** `ApproachLayout` MUST have the fields `length_m: f64` and `lanes: Vec<LaneLayout>`, with the lanes in index order (left to right, as in the config). `LaneLayout` MUST have:
  - `index: u32`;
  - `len_cells: u32`, the number of cells, where cell 0 is the upstream entry and cell `len_cells - 1` is the stop-line cell;
  - `movements: Vec<Movement>`, the allowed movements in `left`, `through`, `right` order.
- **R4.4** `SignalLayout` MUST have the fields `yellow_s`, `all_red_s`, `min_green_s` and `max_red_s` (all `f64`) and `phases: Vec<PhaseLayout>`. The durations MUST be the effective ones: the plan's step counts × `step_s`, not the TOML input before rounding.
- **R4.5** `PhaseLayout` MUST have the fields `name: String` and `green: Vec<MovementRef>`, the movements the phase grants, in the order of R2.5. `MovementRef` MUST have the fields `approach: Direction` and `movement: Movement`.
- **R4.6** `Direction` MUST have the values `north`, `east`, `south` and `west`. `Movement` MUST have the values `left`, `through` and `right`. `Light` MUST have the values `red`, `yellow` and `green`. Their meaning is the one in `sim`.

## R5. Snapshot

- **R5.1** `Snapshot` MUST describe the state after a step (or the initial state at step 0), with the fields:
  - `step: u64`, the simulation's step count;
  - `time_s: f64`, equal to `step × step_s`;
  - `signal: SignalView`;
  - `counts: VehicleCounts`;
  - `lanes: Vec<LaneState>`;
  - `vehicles: Vec<VehicleView>`.
- **R5.2** `SignalView` MUST have the fields `state: SignalStateView` and `lights: ByApproach<ApproachLights>`. `ApproachLights` MUST have the fields `left`, `through` and `right`, each a `Light`. Every movement has a light, even one that no lane allows.
- **R5.3** `SignalStateView` MUST be an enum internally tagged with `#[serde(tag = "kind", rename_all = "snake_case")]`, with the struct variants:
  - `Green { phase: u32, elapsed_s: f64 }`;
  - `Yellow { from: u32, to: u32, elapsed_s: f64 }`;
  - `AllRed { from: u32, to: u32, elapsed_s: f64 }`.

  `elapsed_s` is `sim::SignalState`'s `elapsed` × `step_s`, so it counts the current step as `elapsed` does.
- **R5.4** `VehicleCounts` MUST have the fields `spawned`, `backlog`, `on_lane` and `departed`, all `u64`, with the meanings of the `Simulation` counters of the same names.
- **R5.5** `LaneState` MUST have the fields `lane: LaneRef` and `backlog: u64`, the number of vehicles waiting to enter the lane. There is one `LaneState` per lane of the layout, in the order of R2.5. `LaneRef` MUST have the fields `approach: Direction` and `index: u32`.
- **R5.6** `VehicleView` MUST have the fields:
  - `id: u64`;
  - `lane: LaneRef`;
  - `cell: u32`;
  - `movement: Movement`;
  - `wait_s: f64`, the vehicle's `wait_steps × step_s`;
  - `stopped: bool`.

  `vehicles` MUST list every vehicle on a lane and no other: vehicles in a backlog are only counted (R5.4, R5.5), and vehicles that departed are not listed.

## R6. Metrics

- **R6.1** `Metrics` MUST have the fields:
  - `step: u64`, the simulation's step count;
  - `time_s: f64`;
  - `summary: Summary`;
  - `queue_now: Vec<LaneQueueNow>`.
- **R6.2** `Summary` MUST mirror `sim::EpisodeSummary` field by field, with the same names, meanings (ADR-0006) and values (the same `f64` bits):
  - `steps: u64`;
  - `duration_s: f64`;
  - `departed: u64`;
  - `in_system: u64`;
  - `throughput_veh_per_h: f64`;
  - `wait: Option<WaitStats>`;
  - `wait_by_approach: ByApproach<Option<WaitStats>>`;
  - `queue_by_lane: Vec<LaneQueueStats>`;
  - `queue_by_approach: ByApproach<QueueStats>`;
  - `queue_total: QueueStats`.
- **R6.3** `WaitStats` MUST have the fields `count: u64`, `mean_s`, `p50_s`, `p95_s`, `p99_s` and `max_s` (all `f64`). `QueueStats` MUST have the fields `mean: f64` and `max: u64`. `LaneQueueStats` MUST have the fields `lane: LaneRef`, `mean: f64` and `max: u64`, in the order of R2.5.
- **R6.4** `LaneQueueNow` MUST have the fields `lane: LaneRef` and `stopped: u64`: the number of the lane's vehicles, on the lane or in its backlog, whose `is_stopped()` is true now. This is the ADR-0006 queue at the current step. There is one entry per lane, in the order of R2.5.

## R7. Conversions from sim (feature `sim`)

- **R7.1** With the `sim` feature, the crate MUST provide:
  - `Layout::from_scenario(scenario: &sim::Scenario) -> Layout`;
  - `Snapshot::from_sim(sim: &sim::Simulation) -> Snapshot`;
  - `Metrics::from_sim(summary: &sim::EpisodeSummary, sim: &sim::Simulation) -> Metrics`, which takes `step` and `time_s` from `sim`, copies `summary`, and computes `queue_now` from `sim`.
- **R7.2** The conversions MUST be infallible and MUST NOT panic. A narrowing cast from `usize` to `u32` MAY use `as` where the source is bounded by a `sim` constant (`MAX_PHASES`, `MAX_LANES_PER_APPROACH`), with a local `#[allow(clippy::cast_possible_truncation)]` and a comment naming the bound. A `u64`-to-`f64` cast for times MAY allow `clippy::cast_precision_loss` locally, with a comment.
- **R7.3** The conversions MUST use only the public API of `sim` (R1.5).
- **R7.4** Without the `sim` feature, no symbol of R7.1 may exist, and the crate MUST compile and pass its tests.

## R8. Tests

- **R8.1** **Round-trip fixtures.** Unit or integration tests MUST check `ServerMessage::from_json(&m.to_json()?)? == m` for fixtures that together cover:
  - each `ServerMessage` variant and each `SignalStateView` variant;
  - `wait` both `None` and `Some`, and `wait_by_approach` with a mix of both;
  - an empty `vehicles` list and a non-empty one;
  - a lane with a non-zero backlog.
- **R8.2** **Property test.** A `proptest` test MUST generate arbitrary `ServerMessage`s (strategies for every DTO, finite `f64`s over the whole finite range, arbitrary `String` phase names, collections up to a small bound) and check the round trip gives an equal value. `proptest` MUST be a dev-dependency through the workspace.
- **R8.3** **Golden wire format.** `insta` snapshots MUST pin the `serde_json::to_string_pretty` output of one fixture of each message type, and of a `Snapshot` in each signal state. They live in `crates/protocol/tests/snapshots/`. The tests MUST also check that `to_json` is the compact form of the same value (both parse to the same `serde_json::Value`).
- **R8.4** **Decoding rules.** Tests MUST check that:
  - an extra unknown field at the top level and inside a nested object decodes and is ignored;
  - an unknown `type`, a missing required field, `null` for an `f64`, and trailing garbage each give `ProtocolError::Decode`;
  - `check_version` accepts `PROTOCOL_VERSION` and rejects any other value with `VersionMismatch`.
- **R8.5** **Doc test.** The crate doc MUST contain a runnable example that decodes a short `hello` JSON string, checks its version, and matches on the result.
- **R8.6** **From sim.** A test file `crates/protocol/tests/from_sim.rs`, declared with `[[test]] required-features = ["sim"]`, MUST run the example from step 0 for at least 600 steps with a fixed seed. Each step spawns the arrivals from `sim::Demand`, then calls `Simulation::step` with a command sequence that reaches `Green`, `Yellow` and `AllRed` (a test assertion MUST confirm all three occurred). After every step, the test MUST check that:
  - the `Snapshot` counts equal the `Simulation` counters;
  - `vehicles.len() == on_lane`, and the `backlog` of all lanes sums to `counts.backlog`;
  - vehicle ids are strictly ascending, and each vehicle's lane exists in the layout, its `cell` is below that lane's `len_cells`, and the lane allows its `movement`;
  - every light equals `sim::Simulation::lights()` for that movement;
  - `Metrics::from_sim` with `EpisodeMetrics::summary` equals the summary field by field, and `queue_now` sums to the number of stopped vehicles in the model;
  - every `f64` in the three messages is finite;
  - every message round-trips (R8.1).
- **R8.7** **Golden from sim.** `insta` snapshots MUST pin the pretty JSON of `Hello::new(Layout::from_scenario(example))`, and of the example's `Snapshot` and `Metrics` at one fixed step of the R8.6 run, chosen so that there are vehicles on lanes and at least one non-empty backlog. The chosen step, seed and command sequence MUST be named constants in the test.
- **R8.8** Test files MAY allow `clippy::unwrap_used` and `clippy::expect_used` at the file level, as existing test files do.

## R9. CI

- **R9.1** The `clippy` job in `.github/workflows/ci.yml` MUST gain the step `cargo clippy -p rl-semaphore-protocol --all-targets --features sim --locked -- -D warnings`.
- **R9.2** The `test` job MUST gain the step `cargo nextest run -p rl-semaphore-protocol --features sim --locked`. The workspace-wide run still covers the default-feature tests.
- **R9.3** The `wasm` job MUST gain the step `cargo build -p rl-semaphore-protocol --target wasm32-unknown-unknown --locked`.
- **R9.4** The CI commands listed in the README under "Getting started" MUST include the three commands of R9.1–R9.3. No new job or third-party action may be added.

## R10. Docs, ADR and roadmap

- **R10.1** `specs/adr/0008-wire-protocol.md` MUST record, in the ADR template's format:
  - JSON over serde;
  - own DTOs with an opt-in `sim` feature, and why they are not the `sim` types;
  - the `Hello` handshake and the static/dynamic split;
  - on-lane vehicles listed and backlogs counted;
  - units and ordering;
  - the versioning rule of R2.7.

  Its status is `Accepted`, and it MUST be added to the index in `specs/adr/README.md`.
- **R10.2** The crate doc (`//!` in `lib.rs`) MUST describe the three messages, the message order (R3.6), the versioning rule (or a link to where `PROTOCOL_VERSION` states it), the finiteness rule (R2.4), the `sim` feature, and link to ADR-0008.
- **R10.3** The `protocol` row of the crate table in `specs/tech-stack.md` MUST list `serde`, `serde_json`, and the optional `sim` feature for conversions.
- **R10.4** README "Project status" MUST say Stage 2 has started, 2.1 is done, and the next phase is 2.2.
- **R10.5** Roadmap entry 2.1 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.
