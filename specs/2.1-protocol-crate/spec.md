# 2.1 Protocol crate

**Stage:** 2 (Visualization of the simulator) · **Status:** ☑ done · **Size:** one PR

Related files: [requirements.md](requirements.md) (requirements R1–R10) · [plan.md](plan.md) (tasks).

## Goal

Define the language the server and the browser will speak. At the end of this phase the `protocol` crate has versioned, serde-serializable DTOs for everything Stage 2 streams: a `Hello` message sent once per connection with the protocol version and the static layout of the intersection (approaches, lanes, cells, phases, signal timings), a `Snapshot` of the dynamic state after a step (signal, lights, vehicles on the lanes, backlogs), and a `Metrics` message with the running episode summary (mean and tail wait, throughput, queue statistics) plus the current queue of every lane. All three travel as one tagged `ServerMessage` enum encoded as JSON. With the optional `sim` feature, the crate also builds these messages from a real `Scenario`, `Simulation` and `EpisodeSummary`. Round-trip tests (hand-built fixtures and property tests), golden JSON snapshots, and a test that converts a real run of the example scenario make the wire format explicit and pinned before 2.2 (Axum server) starts sending it and 2.3–2.5 (web) start reading it.

## Scope summary

| Requirement | Topic |
|-------------|-------|
| [R1](requirements.md#r1-crate-layout-and-compatibility) | Module layout, dependencies, the optional `sim` feature, WASM build, nothing else changes |
| [R2](requirements.md#r2-wire-conventions) | JSON, naming, units, ordering, forward compatibility, the versioning rule |
| [R3](requirements.md#r3-messages-and-handshake) | `ServerMessage`, `Hello`, encode/decode, version check, errors |
| [R4](requirements.md#r4-layout) | Static geometry and signal plan, sent once |
| [R5](requirements.md#r5-snapshot) | Per-step state: signal, lights, counts, backlogs, vehicles |
| [R6](requirements.md#r6-metrics) | Running episode summary and current per-lane queues |
| [R7](requirements.md#r7-conversions-from-sim-feature-sim) | `Layout::from_scenario`, `Snapshot::from_sim`, `Metrics::from_sim` behind `sim` |
| [R8](requirements.md#r8-tests) | Round-trip, property, golden, decoding-rule and from-sim tests |
| [R9](requirements.md#r9-ci) | Clippy and tests with `--features sim`, WASM build of `protocol` |
| [R10](requirements.md#r10-docs-adr-and-roadmap) | ADR-0008, crate docs, `tech-stack.md`, README, roadmap |

## Decisions taken

- **Own DTOs, and an opt-in `sim` feature for the conversions.** `protocol` defines its own wire types (`Direction`, `Movement`, `Light`, `WaitStats`, …) instead of reusing the `sim` types. A refactor in `sim` therefore cannot silently change what goes over the wire, and `web` does not pull the simulator into its WASM bundle. By default the crate depends only on `serde`, `serde_json` and `thiserror`. The conversions from `sim` live in the same crate behind a `sim` feature (off by default). The server will enable it in 2.2. In this phase the feature is exercised by tests against the real example scenario. The mapping between the two sets of types sits next to the wire types it produces, and is tested where it is written.
- **A handshake, then deltas of state.** Messages form one internally tagged `ServerMessage` enum (`"type": "hello" | "snapshot" | "metrics"`). `Hello` is sent first on every connection and carries `PROTOCOL_VERSION` and the static `Layout`: approaches, lanes, cell counts, phases and signal timings. `Snapshot` and `Metrics` then carry only what changes. Nothing static is resent every frame.
- **Versioned by one constant and one rule.** `PROTOCOL_VERSION = 1` is checked once, at the handshake (`Hello::check_version`). The rule is written in the crate docs and ADR-0008. Adding an optional field (`#[serde(default)]`) or a new message type is compatible. Removing, renaming or retyping a field, or changing its meaning or units, bumps the version. Decoders ignore unknown fields. The golden snapshots make any change to the format visible in review.
- **Vehicles as a list, backlogs as counts.** A `Snapshot` lists every vehicle that is *on a lane* with its stable id, lane, cell, movement, wait in seconds and `stopped` flag. The ids let the canvas (2.4) interpolate motion, and the wait drives the colour coding and starvation highlight (2.5). Vehicles still waiting to enter a lane are only counted per lane. A backlog can grow without bound under overload, and a list would make the frame size grow with it.
- **Metrics mirror the episode summary, plus live queues.** `Metrics` carries the ADR-0006 `EpisodeSummary` field by field: wait count, mean, p50, p95, p99 and max, overall and per approach, then throughput, departed, in-system, and queue statistics per lane, per approach and in total. It adds the current queue of every lane, meaning its stopped vehicles on the lane or in its backlog, as ADR-0006 defines a queue. The tail metrics that the mission puts first are always on the wire. `Metrics` is a separate message so the server can send it less often than snapshots (2.2 decides the rates).
- **JSON with exact floats.** The wire format is JSON (`serde_json`), as `tech-stack.md` plans, and is readable in the browser's devtools. `serde_json`'s `float_roundtrip` feature is enabled, so every finite `f64` decodes to the same bits it was encoded from. This makes the round-trip tests exact equality, with no tolerance. Durations are in seconds (`_s`), lengths in meters (`_m`), and discrete quantities (steps, cells, ids, counts) are integers. A binary encoding (`postcard`) stays an option for 11.1.
- **Deterministic output.** Every collection has a fixed order: approaches north, east, south, west; lanes by approach, then index; vehicles by id; phases in plan order. There are no maps with unordered keys, so the same value always produces byte-identical JSON, and golden snapshots are stable.
- **An ADR.** The wire format and its versioning rule affect `protocol`, `server` and `web`, and are costly to change once a client exists. ADR-0008 records them.

## Out of scope

- The Axum server, the WebSocket endpoint, message rates and back-pressure (2.2).
- Any client-to-server message (commands, subscriptions, pause/resume). `ServerMessage` is the only message type in this phase.
- Making `web` or `server` depend on `protocol` (2.2, 2.3).
- Grid layouts with several intersections (6.1, 6.3). The layout describes one intersection. Generalizing it is a version bump.
- Training, run, and REST DTOs (Stage 5, Stage 8).
- Per-phase red age and max-red progress on the wire. They can be added later as an optional field.
- A binary encoding, compression or delta encoding of snapshots (11.1).
- Any change to `sim`, `env` or `cli`.

## Acceptance criteria

1. `crates/protocol` exposes `ServerMessage` (`Hello`, `Snapshot`, `Metrics`), `PROTOCOL_VERSION`, `to_json` / `from_json` and `Hello::check_version`. With default features its only normal dependencies are `serde`, `serde_json` and `thiserror`, and it builds for `wasm32-unknown-unknown`. (R1, R3)
2. The JSON follows the conventions of R2: a `type` tag, snake_case names, units in the field names, fixed ordering, unknown fields ignored. Golden `insta` snapshots pin one message of each type and each signal state. (R2, R4–R6, R8)
3. Every hand-built fixture and every generated message round-trips through JSON to an equal value, with exact floats. Unknown `type`, missing fields, `null` floats and a version mismatch are rejected with a `ProtocolError`. (R3, R8)
4. With `--features sim`, `Layout::from_scenario`, `Snapshot::from_sim` and `Metrics::from_sim` agree with the simulation at every step of a run of the example scenario. That covers counts, vehicles, lanes, lights and every signal state, and the summary equals `EpisodeSummary` field by field. The example's `Hello` and one of its snapshots are pinned by golden snapshots. (R7, R8)
5. CI runs clippy and the tests of `protocol` with `--features sim`, and builds `protocol` for WASM. All jobs are green, and no file in `sim`, `env` or `cli` changes. (R1, R9)
6. ADR-0008 records the wire format and versioning rule, the crate docs describe the message order and the rule, `tech-stack.md` and the README are updated, and roadmap entry 2.1 is marked ☑. (R10)
