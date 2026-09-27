# 1.1 Road model: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feat/1.1-road-model`.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Dependencies
- [x] Add `serde = { version = "1", features = ["derive"] }` and `toml = "0.9"` (or the current minor) to `[workspace.dependencies]` ([R1.1](requirements.md#r1-dependencies)).
- [x] Add `serde`, `toml`, `thiserror` to `crates/sim/Cargo.toml` with `.workspace = true` (R1.2).
- [x] Run `cargo deny --locked check`. Extend the license allowlist only if needed, with a comment (R1.3).

### 2. Domain types
- [x] Create the `road` module in `crates/sim` and declare it in `lib.rs` ([R2](requirements.md#r2-domain-types)).
- [x] `Direction` with `ALL` and `destination(self, Movement) -> Direction` (R2.1, R2.3). Implement `destination` by rotation (`Left` = +1 clockwise from the heading side, and so on) rather than a 12-arm match, but test all 12 cases explicitly.
- [x] `Movement` with the derived `Ord` in declaration order `Left, Through, Right` (R2.2).
- [x] `LaneId`, `Lane`, `Approach`, `Intersection` with private fields and accessors. Store approaches as `[Approach; 4]` indexed by `Direction as usize` (or a `BTreeMap<Direction, Approach>`) (R2.4, R2.5, R2.7).
- [x] `Lane::len_cells()` and `Lane::stop_line_cell()`, with rustdoc stating that cell 0 is upstream and `len - 1` is behind the stop line (R2.6).

### 3. Config and parsing
- [x] `IntersectionConfig { cell_length_m, approaches: ApproachesConfig }`, `ApproachesConfig { north, east, south, west }`, `ApproachConfig { length_m, lanes: Vec<LaneConfig> }`, `LaneConfig { movements: Vec<Movement> }`, all `deny_unknown_fields` ([R3.1–R3.3](requirements.md#r3-toml-configuration)). Use `Vec<Movement>` in the raw config so duplicates can be detected (a set would silently drop them).
- [x] `IntersectionConfig::from_toml_str(&str) -> Result<Self, ConfigError>` and `Intersection::try_from(IntersectionConfig)` (R3.4).
- [x] `Intersection::to_config(&self) -> IntersectionConfig` (or keep the config inside the model) for the round trip. Note that `length_m` is not recoverable from the cell count alone, so keep the original value (R3.5).

### 4. Validation
- [x] `ConfigError` (`thiserror`): `Parse(#[from] toml::de::Error)`, `InvalidCellLength { value }`, `InvalidLength { path, value }`, `TooShort { path, cells }`, `LaneCount { path, count }`, `EmptyMovements { path }`, `DuplicateMovement { path, movement }`, `CrossingMovements { path, left_lane, right_lane }` or equivalent names ([R4.1](requirements.md#r4-validation)).
- [x] Implement the checks in the fixed order of R4.7 (R4.2–R4.5).
- [x] Cell count: check finiteness and the `u32` range on the `f64` quotient before casting, with a local `#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]` and an `// INVARIANT:` comment (R4.3, R4.6).

### 5. Example config and tests
- [x] Write `configs/single-intersection.toml` with a comment header ([R5.1](requirements.md#r5-example-config-and-tests)). Suggested layout: north/south main road, 2 lanes (`["left"]`, `["through", "right"]`), 150 m; east/west side road, 1 lane (`["left", "through", "right"]`), 100 m; `cell_length_m = 7.5`.
- [x] Unit tests for every item in R5.2. Load the example with `include_str!("../../../configs/single-intersection.toml")` (adjust the relative path to the test file's location). Build failing configs as small inline TOML strings, not by editing the example.
- [x] Optional: a `proptest` for the round trip and cell-count formula (R5.3).
- [x] Remove the placeholder `crate_compiles` test from `lib.rs`.

### 6. ADR-0002
- [x] Copy `specs/adr/0000-template.md` to `specs/adr/0002-discrete-cell-model.md` and fill it in ([R6](requirements.md#r6-adr-0002-discrete-cell-model)).
- [x] Add it to the index table in `specs/adr/README.md`.

### 7. Docs and roadmap
- [x] Update the simulation paragraph in `specs/tech-stack.md` with the decision and a link to ADR-0002 ([R7.1](requirements.md#r7-docs-and-roadmap)).
- [x] Mention `configs/single-intersection.toml` in the README (R7.2).
- [x] Mark 1.1 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R7.3).

### 8. Verify locally

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
cargo doc -p rl-semaphore-sim --no-deps
```

## Notes and risks

- **Clippy pedantic casts.** `f64 → u32` triggers `cast_possible_truncation` and `cast_sign_loss`. Range-check first, then allow locally with an `// INVARIANT:` comment. Do not allow them crate-wide.
- **Float equality in the round trip.** `toml` writes floats so that they parse back to the same `f64`, but compare the round-tripped `Intersection` (integer cells) and the config fields with `==` only on values that came from parsing, not on computed floats.
- **`include_str!` path.** The example config lives outside the crate. The relative path from `crates/sim/src/...` is fragile if files move; a test in `crates/sim/tests/` with `concat!(env!("CARGO_MANIFEST_DIR"), "/../../configs/single-intersection.toml")` is sturdier.
- **Error ergonomics vs first-error validation.** Stopping at the first error keeps the code simple. If collecting all errors turns out to be cheap, it is allowed later without an API break only if `ConfigError` is not matched exhaustively by callers; mark it `#[non_exhaustive]`.
- **Lane-consistency rule.** R4.5 rejects layouts like `["right"]` left of `["through"]`. That is intentional for right-hand traffic, but the error message should say why, since it is the least obvious rule.
- **Scope creep toward 1.2.** It is tempting to add the movement conflict matrix here. Keep it for 1.2, where the signal safety tests use it.
- **`toml` versioning.** `toml` is pre-1.0, so a minor bump can break the API. Pin the minor in the workspace entry.
- **New transitive licenses.** `toml` pulls `serde_spanned`, `toml_datetime`, `toml_parser`/`winnow`, and others. They are expected to be MIT/Apache, but `cargo deny` is the source of truth.
