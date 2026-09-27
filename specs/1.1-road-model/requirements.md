# 1.1 Road model: requirements

See [spec.md](spec.md) for goal, scope, and acceptance criteria, and [plan.md](plan.md) for tasks.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

## R1. Dependencies

- **R1.1** `[workspace.dependencies]` MUST declare `serde` (feature `derive`) and `toml`, each with a major-version requirement only (for `toml`, whose major is 0, the minor version, for example `"0.9"`), matching the existing entries.
- **R1.2** `crates/sim/Cargo.toml` MUST depend on `serde`, `toml`, and `thiserror` with `.workspace = true`. `sim` MUST NOT depend on `anyhow`, `tracing-subscriber`, or any async or I/O crate.
- **R1.3** `cargo deny --locked check` MUST pass. If a new dependency brings a license not in `deny.toml`, it MAY be added to the allowlist only if it is permissive, with a comment naming the crate that needs it.
- **R1.4** `Cargo.lock` MUST be committed with the new entries.

## R2. Domain types

All types live in a `road` module of `crates/sim` (for example `crates/sim/src/road.rs`, or `road/` with submodules), re-exported from the crate root where useful. All public items MUST have rustdoc.

- **R2.1** `Direction` MUST be an enum with `North`, `East`, `South`, `West`. An approach is named by the side of the intersection vehicles come *from* (the `north` approach carries southbound traffic). `Direction` MUST derive `Copy`, `Eq`, `Ord`, `Hash`, `Debug`, and serde traits (lowercase names), and MUST provide `Direction::ALL` in the order N, E, S, W.
- **R2.2** `Movement` MUST be an enum with `Left`, `Through`, `Right`, ordered `Left < Through < Right` (the order is used by R4.5). Serde names MUST be lowercase.
- **R2.3** There MUST be a pure function (for example `Direction::destination(self, Movement) -> Direction`) that returns the exit side for right-hand traffic: from `North`, `Left` exits `East`, `Through` exits `South`, `Right` exits `West`, and the same pattern rotated for the other approaches. U-turns are not a movement.
- **R2.4** `LaneId` MUST identify a lane by approach `Direction` and a lane index, where index 0 is the leftmost lane (closest to the centre line). It MUST be `Copy`, `Eq`, `Ord`, and `Hash`.
- **R2.5** The validated model MUST be:
  - `Lane`: its `LaneId`, its set of allowed movements (a `BTreeSet<Movement>` or a small bitset, non-empty), and its length in cells.
  - `Approach`: its `Direction` and its lanes in index order.
  - `Intersection`: its cell length in meters and exactly four approaches, retrievable by `Direction` and iterable in `Direction::ALL` order.
  Fields MUST be private with read-only accessors, so a validated model cannot be made invalid after construction.
- **R2.6** Cell positions along a lane MUST be integers, with cell 0 at the upstream entry and cell `len - 1` the cell immediately behind the stop line. The model MUST expose the stop-line cell of each lane (for example `Lane::stop_line_cell()`), and the rustdoc MUST state this convention, since 1.3 and the renderer (2.4) depend on it.
- **R2.7** `sim` MUST stay deterministic: no `HashMap`/`HashSet` iteration where order is observable (use `BTreeMap`, arrays indexed by `Direction`, or `Vec`), no wall-clock time, no randomness in this phase.

## R3. TOML configuration

- **R3.1** The raw config types (`IntersectionConfig`, `ApproachConfig`, `LaneConfig`) MUST derive `Deserialize` and `Serialize` and mirror this schema:

  ```toml
  cell_length_m = 7.5

  [approaches.north]
  length_m = 150.0
  lanes = [
    { movements = ["left"] },
    { movements = ["through", "right"] },
  ]

  [approaches.east]
  # ...same shape; south and west likewise
  ```

- **R3.2** `approaches` MUST have exactly the keys `north`, `east`, `south`, `west`. The config MUST be modelled so that a missing approach is a parse error (for example a struct with four named fields), not a runtime lookup.
- **R3.3** All raw config structs MUST use `#[serde(deny_unknown_fields)]`. All fields are required; there are no defaults in this phase, so a config file fully states the geometry.
- **R3.4** Parsing MUST be exposed as a function that takes `&str` (for example `IntersectionConfig::from_toml_str`) and a conversion to the model (for example `Intersection::try_from(IntersectionConfig)` or `IntersectionConfig::validate`). A convenience `Intersection::from_toml_str` MAY combine both. `sim` MUST NOT open files.
- **R3.5** There MUST be a way to get the `IntersectionConfig` back from an `Intersection` (or to keep the source config alongside it), so later phases can record the exact config of a run (phase 5.1). Round-tripping `config → TOML string → config → Intersection` MUST give an equal `Intersection`.

## R4. Validation

Validation turns an `IntersectionConfig` into an `Intersection` or returns a `ConfigError`.

- **R4.1** `ConfigError` MUST be a `thiserror` enum in `sim`. It MUST have a variant wrapping the TOML parse error (with `#[from]` or `#[source]`) and one variant per validation rule below. Each validation variant MUST carry the field path as a string (for example `approaches.north.lanes[1].movements`) and its `Display` MUST include that path and the offending value.
- **R4.2** `cell_length_m` MUST be finite and strictly positive.
- **R4.3** Each approach `length_m` MUST be finite and strictly positive, and MUST yield at least 2 cells (`floor(length_m / cell_length_m) >= 2`), so every lane has at least one cell behind the stop-line cell. The number of cells is `floor(length_m / cell_length_m)`; any remainder is dropped and documented in rustdoc. The cell count MUST fit in `u32`.
- **R4.4** Each approach MUST have between 1 and `MAX_LANES_PER_APPROACH` lanes, where the constant is 4.
- **R4.5** Each lane's `movements` MUST be non-empty and MUST NOT contain duplicates. Across an approach, lanes MUST be consistent left to right: for every pair of adjacent lanes `i` and `i + 1`, the greatest movement of lane `i` MUST be less than or equal to the smallest movement of lane `i + 1` (so turn paths never cross inside the intersection). Shared movements across adjacent lanes are allowed (for example `["left", "through"]` then `["through"]`).
- **R4.6** Validation MUST NOT panic on any input. Float-to-integer conversions MUST happen only after the range checks, and any clippy pedantic `cast_*` allowance MUST be local and carry an `// INVARIANT:` comment explaining why the cast is safe.
- **R4.7** Validation MAY stop at the first error. If it does, it MUST check fields in a fixed order (cell length, then approaches in `Direction::ALL` order, then lanes in index order) so the reported error is deterministic.

## R5. Example config and tests

- **R5.1** `configs/single-intersection.toml` at the repository root MUST be a valid config with a comment header explaining the schema and units. It SHOULD use a realistic, asymmetric layout (for example a main road with two lanes including a dedicated left-turn lane, and a side road with one shared lane) so later phases have something interesting to control.
- **R5.2** Tests in `sim` MUST cover:
  - loading `configs/single-intersection.toml` (through `include_str!`) and checking approach count, lane counts, movements, and cell counts;
  - each rule in R4.2–R4.5 with a failing config and a match on the `ConfigError` variant and field path;
  - a missing approach, a missing field, and an unknown field, each producing the parse error variant;
  - `Direction::destination` for all 12 approach–movement pairs;
  - the stop-line cell convention (R2.6);
  - the round trip in R3.5.
- **R5.3** Tests MAY use `proptest` to generate valid configs and check the round trip and the cell-count formula. If used, `proptest` MUST be a dev-dependency declared in `[workspace.dependencies]`.

## R6. ADR-0002: discrete cell model

- **R6.1** `specs/adr/0002-discrete-cell-model.md` MUST follow the template and record the decision to model lanes as fixed-length cells with integer positions.
- **R6.2** Its Context MUST name the alternatives considered: at least a continuous car-following model (for example IDM with `f64` positions) and a macroscopic or cell-transmission flow model. Its Consequences MUST include the costs (coarser spatial resolution, speed quantized to cells per step, less realistic acceleration) as well as the benefits (simple collision-freedom, integer determinism, fast stepping, easy rendering).
- **R6.3** Status `Accepted`, date of the PR. The index table in `specs/adr/README.md` MUST list it.

## R7. Docs and roadmap

- **R7.1** `specs/tech-stack.md` MUST replace "a discrete lane/cell or car-following model (decided in the simulator phase, starting simple)" with the decision and a link to ADR-0002.
- **R7.2** The README SHOULD mention `configs/` and the example config in one line (for example in the project layout or "Getting started").
- **R7.3** Roadmap entry 1.1 in `specs/roadmap.md` MUST be marked ☑ in the PR that implements this phase, and the spec status set to ☑ done.
