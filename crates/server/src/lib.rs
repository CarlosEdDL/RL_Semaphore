//! Puts the simulator on the network: an Axum HTTP/WebSocket server that streams a live,
//! looping fixed-time simulation.
//!
//! # Endpoints
//!
//! - `GET /healthz` returns 200 with a small JSON status ([`health::Health`]): `status`,
//!   `version`, `protocol_version`, the `(episode, step)` of the latest tick, and the number of
//!   open connections.
//! - `GET /ws` upgrades to a WebSocket that streams the [`rl_semaphore_protocol`] wire format.
//!   A request without the upgrade headers gets a 4xx response. Any other path gets 404; any
//!   method but `GET` on these two paths gets 405.
//!
//! # The sim thread
//!
//! One `rl_semaphore_env::Episode` runs on a dedicated OS thread (not a Tokio worker), driven by
//! a `FixedTime` controller built from [`ServerConfig::scenario`]'s plan. It is paced by
//! [`Pace`]: [`Pace::RealTime`] publishes tick `k` no earlier than `start + k * (step_s /
//! speed)`, resetting its reference instead of bursting through missed ticks when it falls more
//! than one period behind; [`Pace::Unthrottled`] never sleeps. After
//! [`ServerConfig::steps_per_episode`] steps, the run continues with a fresh episode, seeded
//! `seed.wrapping_add(episode)`, so the demo loops forever from one base seed. Each tick's
//! `Snapshot` (every step) and `Metrics` (step 0, every `metrics_every`th step, and the last
//! step of the episode) are encoded to JSON once, on the sim thread, and broadcast together;
//! connections forward the ready text without decoding or re-encoding it.
//!
//! # Message order and catch-up
//!
//! Every connection receives exactly one `Hello` first, whose layout matches the scenario, then
//! the current `Snapshot` and `Metrics` (so a client that joins late sees the live state right
//! away, not an empty one), then the live stream. `Snapshot` keys `(episode, step)` strictly
//! increase, and every `Metrics` has a key no greater than the latest `Snapshot` sent before it
//! ([`rl_semaphore_protocol`]'s crate docs state the full rule).
//!
//! # Lag
//!
//! A connection that falls behind the broadcast channel's capacity
//! ([`ServerConfig::broadcast_capacity`]) skips the ticks it missed and keeps going from the
//! oldest one still buffered; the simulation never waits for a slow client, and no client can
//! slow down another.
//!
//! # Shutdown
//!
//! [`Server::serve`] stops accepting connections, sends every open WebSocket a close frame
//! (code 1001, "going away"), stops the sim thread within one step period plus a small
//! constant, joins it, and returns. A sim thread failure or panic shuts everything down the
//! same way and is returned as an error.
//!
//! See ADR-0008 (`specs/adr/0008-wire-protocol.md`) for the wire format's rationale, and
//! ADR-0009 (`specs/adr/0009-live-simulation-streaming.md`) for this crate's design.

#![warn(clippy::pedantic)]

mod app;
mod config;
mod error;
mod health;
mod pace;
mod sim_loop;
mod ws;

pub use app::{Server, run};
pub use config::{Pace, ServerConfig};
pub use error::ServerError;
