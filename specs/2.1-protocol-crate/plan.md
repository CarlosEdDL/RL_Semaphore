# 2.1 Protocol crate: plan and tasks

See [spec.md](spec.md) for scope and acceptance criteria, and [requirements.md](requirements.md) for requirements.

## Branch and commits (handled by the author, not by Claude)

- Branch: `feature/stage-2` (the current branch), or a per-phase branch if preferred.
- Use Conventional Commits. One commit per task group below is fine, and the PR is squash-merged.

## Tasks

### 1. Manifest and dependencies
- [x] Workspace: add `rl-semaphore-protocol = { path = "crates/protocol", version = "0.1.0" }` to `[workspace.dependencies]` ([R1.3](requirements.md#r1-crate-layout-and-compatibility)).
- [x] `crates/protocol/Cargo.toml`: `serde.workspace = true`, `serde_json = { workspace = true, features = ["float_roundtrip"] }`, `thiserror.workspace = true`, and `rl-semaphore-sim = { workspace = true, optional = true }`. Add `[features] default = []` and `sim = ["dep:rl-semaphore-sim"]` (R1.2).
- [x] Dev-dependencies: `proptest` and `insta`. The from-sim tests reach `rl-semaphore-sim` through the optional dependency that the `sim` feature enables, so no dev-dependency on `sim` is needed.
- [x] `[[test]] name = "from_sim"`, `required-features = ["sim"]` ([R8.6](requirements.md#r8-tests)).
- [x] Check early that `cargo build -p rl-semaphore-protocol --target wasm32-unknown-unknown` and `cargo tree -p rl-semaphore-protocol -e normal` behave as R1.4 says, and that `cargo deny --locked check` passes (R1.7).

### 2. Common types and versioning
- [x] `common.rs`: `Direction`, `Movement`, `Light`, `LaneRef`, `MovementRef`, `ByApproach<T>` with `get` ([R4.2](requirements.md#r4-layout), R4.5–R4.6, R5.5).
- [x] `lib.rs`: `PROTOCOL_VERSION` with the versioning rule in its doc ([R2.7](requirements.md#r2-wire-conventions)), module declarations, and re-exports.

### 3. Layout, Snapshot, Metrics
- [x] `layout.rs`: `Layout`, `ApproachLayout`, `LaneLayout`, `SignalLayout`, `PhaseLayout` (R4).
- [x] `snapshot.rs`: `Snapshot`, `SignalView`, `SignalStateView` (tagged `kind`), `ApproachLights`, `VehicleCounts`, `LaneState`, `VehicleView` ([R5](requirements.md#r5-snapshot)).
- [x] `metrics.rs`: `Metrics`, `Summary`, `WaitStats`, `QueueStats`, `LaneQueueStats`, `LaneQueueNow` ([R6](requirements.md#r6-metrics)).
- [x] Check every DTO against R2.1–R2.6: the derives, no `flatten`, `untagged` or `deny_unknown_fields`, `Option`s written as `null`, and `Vec`s only in the orders of R2.5.

### 4. Messages and errors
- [x] `message.rs`: `ServerMessage` (tagged `type`), `Hello` with `new` and `check_version`, and `to_json` / `from_json` ([R3](requirements.md#r3-messages-and-handshake)).
- [x] `ProtocolError` with `Encode`, `Decode` and `VersionMismatch`, and `# Errors` docs on every fallible function (R3.5, R1.6).

### 5. Fixture and decoding tests
- [x] `tests/common/mod.rs` (or similar): fixture builders for a small layout, a snapshot in each signal state, and metrics with a mix of `None` / `Some` waits.
- [x] Round-trip tests of the fixtures (R8.1).
- [x] Golden `insta` snapshots of the pretty JSON, plus the compact vs pretty equivalence check (R8.3). Review each `.snap` by eye: field names, units, ordering and tags should read as the requirements say. This is the first review of the wire format.
- [x] Decoding-rule tests (R8.4).

### 6. Property test
- [x] `tests/roundtrip_props.rs`: proptest strategies for every DTO, finite `f64` over the whole finite range, and bounded collections. Check that the round trip gives an equal value (R8.2).

### 7. Conversions from sim
- [x] `from_sim.rs` (`#[cfg(feature = "sim")]`): `Layout::from_scenario`, `Snapshot::from_sim`, `Metrics::from_sim` ([R7](requirements.md#r7-conversions-from-sim-feature-sim)). Import the simulator as `use rl_semaphore_sim as sim;` so that `sim::Direction` and `Direction` stay distinct.
- [x] Private mapping helpers `Direction::from(sim::Direction)`, `Movement::from(sim::Movement)`, `Light::from(sim::Light)` and `LaneRef::from(sim::LaneId)`. `From` impls are fine because the target types are local, but keep them `pub(crate)` or document them if public.
- [x] `tests/from_sim.rs`: the stepping loop with the invariants of R8.6, and the golden snapshots of R8.7. Pick the fixed step after looking at a run: vehicles on lanes and a non-empty backlog.

### 8. Crate docs
- [x] The `//!` doc in `lib.rs`: the messages, the message order, the versioning rule, finiteness, the `sim` feature, a link to ADR-0008, and the runnable decode example (R8.5, [R10.2](requirements.md#r10-docs-adr-and-roadmap)).

### 9. CI
- [x] `.github/workflows/ci.yml`: the three steps of [R9.1–R9.3](requirements.md#r9-ci).
- [x] README "Getting started": add the three commands to the CI command list (R9.4).

### 10. ADR and docs
- [x] `specs/adr/0008-wire-protocol.md` from the template, and a row in the ADR index (R10.1).
- [x] `specs/tech-stack.md`: the `protocol` row (R10.3).
- [x] README "Project status" (R10.4).

### 11. Roadmap
- [x] Mark 2.1 as ☑ in `specs/roadmap.md` and set the spec status to ☑ done (R10.5).

### 12. Verify locally

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p rl-semaphore-protocol --all-targets --features sim --locked -- -D warnings
cargo nextest run --workspace --locked
cargo nextest run -p rl-semaphore-protocol --features sim --locked
cargo test --workspace --doc --locked
cargo build -p rl-semaphore-web --target wasm32-unknown-unknown --locked
cargo build -p rl-semaphore-protocol --target wasm32-unknown-unknown --locked
cargo deny --locked check
cargo audit
cargo bench -p rl-semaphore-env --bench throughput --locked -- --test

# default features pull in no simulator
cargo tree -p rl-semaphore-protocol -e normal | grep -c rl-semaphore-sim   # 0

# nothing in sim, env or cli moved
git diff --stat -- crates/sim/src crates/env/src crates/cli/src   # empty
git diff --exit-code crates/sim/tests/snapshots/ crates/cli/tests/snapshots/
```

## Notes and risks

- **Nested internally tagged enums and floats.** `ServerMessage` (tag `type`) contains `Snapshot`, which contains `SignalStateView` (tag `kind`). serde buffers internally tagged content through its private `Content` type before it dispatches on the tag. This works with nested tags. Check with the property test that `f64` values survive the buffering bit for bit with `float_roundtrip`, and that `u64` values above 2^53 survive too. If the floats do not round-trip exactly, look at the buffering path first, before loosening the test. Do not switch to a tolerance: exactness is a requirement.
- **`float_roundtrip` is workspace-wide in effect.** Cargo unifies features, so enabling it in `protocol` also turns it on for `cli` in builds that include both. That only affects parsing (`cli` only serializes), and the cost is about 2× on float parsing, which is negligible here. The `cli` JSON snapshot is serialization output and must stay byte-identical. The verify step checks it.
- **`u64` in the browser.** JavaScript numbers lose precision above 2^53, but the client is Rust/WASM decoding with `serde_json`, so `u64` is exact. If a JS client ever reads the wire directly, ids and steps above 2^53 would be a problem. That is noted in ADR-0008, not solved here.
- **Enough signal states in the from-sim run.** With `Command::Hold` alone, the only transitions come from max-red forcing. A simple rotation (ask for the next phase every N steps) reaches green, yellow and all-red quickly with the example's 3 s yellow and 2 s all-red. The assertion of R8.6 guards against a command sequence that never leaves green.
- **pedantic lints in conversions.** Expect `cast_possible_truncation` (`usize` → `u32` for lane and phase indices) and `cast_precision_loss` (`u64` → `f64` for `time_s`, `elapsed_s`, `wait_s`). Allow them on the smallest scope, with a comment naming the bound (`MAX_PHASES = 8`, `MAX_LANES_PER_APPROACH = 4`), as R7.2 says. `sim` already computes times this way, so the values match.
- **The two layers of queue data.** `Summary.queue_by_lane` holds the episode statistics (time-mean and max). `queue_now` holds the count at this instant. The names and doc comments must keep them apart. They differ from `LaneState.backlog` in the snapshot, which counts only the vehicles not yet on the lane.
- **Frame size for 2.2.** After task 7, record the compact JSON size of the example snapshot at the fixed step, and at the busiest step of the run, in this section. 2.2 uses it to pick a frame rate. On the example, at most about 106 vehicles fit on the lanes (20 cells × 4 lanes + 13 cells × 2 lanes), so a snapshot should stay in the low tens of kilobytes. No requirement depends on this number.

  Measured on the seed-2024, 600-step run of `configs/single-intersection.toml` used by the `from_sim` test (rotating phases every 20 steps): the pinned step (387, chosen for R8.7) gives a compact `ServerMessage::Snapshot` of **3,096 bytes**; the busiest step of the run (585, 24 on-lane vehicles) gives **4,078 bytes**. Both are far below "low tens of kilobytes."
- **Scope creep.** Do not add client messages, a `serve` command, server code, or a `protocol` dependency in `web` or `server`. Those belong to 2.2 and 2.3. The layout is single-intersection on purpose. Generalizing it for grids (6.1) is a version bump, and the ADR says so.
