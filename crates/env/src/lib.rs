//! Controllers, the episode stepper and `run_episode`: the `Controller` trait, the fixed-time
//! baseline, and the episode stepper and `run_episode` on top of it.
//!
//! The RL environment trait (`reset`/`step`), observations, action masking and reward
//! definitions will join them in Stage 3.

#![warn(clippy::pedantic)]

pub mod controller;
pub mod episode;
pub mod fixed_time;
pub mod runner;

pub use controller::Controller;
pub use episode::Episode;
pub use fixed_time::FixedTime;
pub use runner::{EnvError, EpisodeReport, SignalCounts, run_episode};
