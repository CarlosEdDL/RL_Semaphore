//! Shared, versioned DTOs for the WebSocket messages the server (2.2) sends the browser (2.3–2.5).
//!
//! # Messages
//!
//! Everything travels as one tagged [`ServerMessage`] enum, encoded as JSON
//! (`"type": "hello" | "snapshot" | "metrics"`):
//!
//! - [`Hello`]: sent once at the start of every connection. Carries [`PROTOCOL_VERSION`] and the
//!   static [`Layout`] (approaches, lanes, cells, phases, signal timings).
//! - [`Snapshot`]: the dynamic state after a step (signal, lights, vehicles on the lanes,
//!   backlogs).
//! - [`Metrics`]: the running episode summary (mean and tail wait, throughput, queue statistics)
//!   plus the current queue of every lane.
//!
//! A connection sends exactly one `Hello` first, then any sequence of `Snapshot` and `Metrics`
//! messages, each `Metrics` describing the same step as the latest `Snapshot` or an earlier one.
//! This phase only defines that order; 2.2 (the Axum server) enforces it. There is no
//! client-to-server message in this phase.
//!
//! # Versioning
//!
//! See the doc comment on [`PROTOCOL_VERSION`] for the compatibility rule.
//!
//! # Finiteness
//!
//! Every `f64` on the wire must be finite. A non-finite value encodes as JSON `null` and then
//! fails to decode (`f64` is not nullable), so a non-finite value never round-trips silently.
//! The `sim` feature's conversions only ever produce finite values.
//!
//! # The `sim` feature
//!
//! With the optional `sim` feature, this crate also builds these messages from a real
//! `rl_semaphore_sim::Scenario`, `Simulation` and `EpisodeSummary`: [`Layout::from_scenario`],
//! [`Snapshot::from_sim`] and [`Metrics::from_sim`]. It is off by default, so `web`'s WASM bundle
//! never pulls in the simulator; the server enables it in 2.2.
//!
//! See ADR-0008 (`specs/adr/0008-wire-protocol.md`) for the design rationale.
//!
//! # Example
//!
//! ```
//! use rl_semaphore_protocol::ServerMessage;
//!
//! let text = r#"{
//!     "type": "hello",
//!     "protocol_version": 1,
//!     "layout": {
//!         "step_s": 1.0,
//!         "cell_length_m": 7.5,
//!         "approaches": {
//!             "north": { "length_m": 100.0, "lanes": [] },
//!             "east": { "length_m": 100.0, "lanes": [] },
//!             "south": { "length_m": 100.0, "lanes": [] },
//!             "west": { "length_m": 100.0, "lanes": [] }
//!         },
//!         "signal": {
//!             "yellow_s": 3.0,
//!             "all_red_s": 2.0,
//!             "min_green_s": 5.0,
//!             "max_red_s": 90.0,
//!             "phases": []
//!         }
//!     }
//! }"#;
//!
//! match ServerMessage::from_json(text) {
//!     Ok(ServerMessage::Hello(hello)) => hello.check_version().expect("version matches"),
//!     other => panic!("expected a Hello message, got {other:?}"),
//! }
//! ```

#![warn(clippy::pedantic)]

mod common;
#[cfg(feature = "sim")]
mod from_sim;
mod layout;
mod message;
mod metrics;
mod snapshot;

pub use common::{ByApproach, Direction, LaneRef, Light, Movement, MovementRef};
pub use layout::{ApproachLayout, LaneLayout, Layout, PhaseLayout, SignalLayout};
pub use message::{Hello, ProtocolError, ServerMessage};
pub use metrics::{LaneQueueNow, LaneQueueStats, Metrics, QueueStats, Summary, WaitStats};
pub use snapshot::{
    ApproachLights, LaneState, SignalStateView, SignalView, Snapshot, VehicleCounts, VehicleView,
};

/// The wire protocol version, checked once at the handshake ([`Hello::check_version`]).
///
/// - **Compatible (no version bump):** adding a field with `#[serde(default)]`, or adding a new
///   [`ServerMessage`] variant.
/// - **Breaking (version bump):** removing, renaming or retyping a field, removing a variant, or
///   changing a field's meaning, units or ordering.
pub const PROTOCOL_VERSION: u32 = 1;
