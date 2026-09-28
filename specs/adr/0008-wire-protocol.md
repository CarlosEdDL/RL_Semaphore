# 0008. Wire protocol

**Status:** Accepted
**Date:** 2026-09-27

## Context

Stage 2 puts a browser in front of the simulator: the server (2.2) streams state over a
WebSocket, and the web frontend (2.3–2.5) renders it. Both sides need a shared, versioned
definition of what goes over the wire, decided once and stable enough to build a canvas
renderer and a metrics panel against, before either of them exists. The format also has to
survive a normal refactor of `sim` without silently changing what a client already deployed
expects.

Alternatives considered:

- **Reuse `sim`'s own types on the wire**, `#[derive(Serialize)]`'d directly. Less code now, but
  it ties the wire format to internal representations (`LaneId`'s private fields, `Signal`'s
  entry-counting `elapsed`, `VehicleId`'s newtype) that are free to change for reasons that have
  nothing to do with the browser. A refactor in `sim` would silently change the wire format, and
  `web`'s WASM bundle would pull in the whole simulator (demand sampling, TOML parsing, `rand`)
  to get a handful of struct definitions.
- **One message per changed field (a diff protocol).** Smaller frames, but it needs a
  server-side "what changed since last time" pass and a client-side merge, both stateful, for a
  gain that only matters once frame rate and vehicle counts are measured (11.1 territory, not
  now).
- **Binary encoding (`postcard`, `bincode`) from the start.** Smaller and faster, but not
  inspectable in the browser's devtools while the renderer is being built, and `tech-stack.md`
  already plans JSON first with a binary option kept for later if bandwidth requires it.
- **A single message with everything (layout, state and metrics combined every frame).**
  Resending static geometry every frame wastes bandwidth for no benefit; the layout does not
  change after the handshake.
- **Vehicles in a backlog listed individually, like on-lane vehicles.** A backlog can grow
  without bound under overload (a policy that starves an approach), so a snapshot's size would
  grow with exactly the failure the mission cares about catching. A count is enough: the canvas
  has nothing to animate for a vehicle that has not entered a lane yet.
- **No explicit version field, relying on the client and server always being deployed together.**
  Works only as long as there is one deployment. A version field costs one `u32` and one check.

## Decision

**JSON over `serde_json`, with exact floats.** `serde_json`'s `float_roundtrip` feature is
enabled, so every finite `f64` a message carries decodes to the same bits it was encoded from;
round-trip tests use exact equality, not a tolerance. A non-finite value encodes as `null` and
then fails to decode, which the crate docs state as the finiteness rule.

**`protocol` defines its own DTOs**, independent of `sim`'s internal types. By default it depends
on nothing but `serde`, `serde_json` and `thiserror`. An optional `sim` feature (off by default)
adds `Layout::from_scenario`, `Snapshot::from_sim` and `Metrics::from_sim`, which map from
`sim::Scenario` / `Simulation` / `EpisodeSummary` using only `sim`'s public API. The server (2.2)
enables the feature; `web`'s WASM bundle never links the simulator. The mapping lives next to the
wire types it produces and is tested where it is written (`from_sim.rs`), not scattered across
`sim`.

**One handshake, then deltas of state.** Every connection gets exactly one `Hello` first, tagged
with `PROTOCOL_VERSION` and carrying the static `Layout` (approaches, lanes, cell counts, phases,
signal timings). After that, any sequence of `Snapshot` (the state after a step: signal, lights,
on-lane vehicles, per-lane backlog counts) and `Metrics` (the running episode summary plus the
current per-lane queue) messages follows; nothing static is resent. All three are variants of one
internally tagged `ServerMessage` enum (`"type": "hello" | "snapshot" | "metrics"`), so a client
matches on one type. This phase only defines the order; 2.2 enforces it and picks the rates.

**On-lane vehicles are listed, backlog vehicles are counted.** A `Snapshot` lists every vehicle
on a lane with its id, cell, movement, wait and `stopped` flag — the canvas needs those to place
and color it, and the ids let it interpolate motion between frames. Vehicles still waiting to
enter a lane are only counted per lane (`Snapshot::lanes[i].backlog`), so an unbounded backlog
under overload does not make the frame grow with it.

**Metrics mirror `sim::EpisodeSummary` field by field** (ADR-0006): the same names, meanings and
values, so nothing is re-derived or renamed in transit. `queue_now` adds the instantaneous queue
(stopped vehicles now, on the lane or in the backlog) per lane, which the episode statistics do
not carry. `Metrics` is a separate message from `Snapshot` so the server can send it less often.

**Units in the name, fixed ordering, versioned by one rule.** Durations are `f64` seconds named
`*_s`, lengths are `f64` meters named `*_m`; discrete quantities are integers. Every collection
has a fixed order (approaches north/east/south/west, lanes by index, vehicles by ascending id,
movements in `MovementId::ALL` order), so the same value always encodes to the same bytes and
`insta` snapshots are stable. `PROTOCOL_VERSION` is one `u32`, checked once at the handshake via
`Hello::check_version`. Adding a `#[serde(default)]` field or a new `ServerMessage` variant is
compatible; removing, renaming or retyping a field, removing a variant, or changing a field's
meaning, units or ordering is not, and bumps the version. Decoders ignore unknown fields, so a
newer server talking to an older-but-compatible client (or vice versa) does not break on an
added field.

## Consequences

### Positive

- The wire format is stable under a `sim` refactor: nothing in `sim`'s internals is exposed, and
  `crates/sim/src`, `crates/env/src` and `crates/cli/src` do not change for this phase at all.
- `web`'s WASM bundle stays free of the simulator, demand sampling, TOML parsing and `rand`.
- The format is readable in the browser's devtools while 2.2–2.5 are being built, and every
  message type has a golden JSON snapshot reviewed by eye, so a change to the format is visible
  in a PR diff rather than discovered at runtime.
- Exact-float round-trip tests and a property test over arbitrary messages catch a serialization
  bug (buffering of nested internally tagged enums, in particular) before 2.2 ever sends a byte.
- A one-`u32` version check is cheap to implement and enough for a single-deployment system; the
  rule is written down once, in the crate docs and here, before there is a second version to
  confuse it with.

### Negative

- Every wire type is a duplicate, by hand, of the corresponding `sim` concept (`Direction`,
  `Movement`, `Light`, and the mapping in `from_sim.rs`). A new movement or light state needs a
  matching change in two places, caught only by the `from_sim` test, not by the type system.
- JSON is larger and slower to parse than a binary encoding would be; `postcard` stays an option
  for 11.1 if bandwidth or CPU on the client becomes a problem.
- `u64` ids and step counts are exact only because the client is Rust/WASM using `serde_json`. A
  JavaScript client reading the wire directly would lose precision above 2^53; not a problem
  today (there is no such client), but a real constraint if one is ever added.
- The instantaneous `queue_now` and the episode-averaged `queue_by_lane` (inside `summary`) are
  two different numbers about the same lanes, sent in the same message; a careless reader of the
  wire format could conflate them if the field names and docs are not read carefully.
