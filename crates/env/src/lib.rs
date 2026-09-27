//! Controllers and the episode runner: the `Controller` trait, the fixed-time baseline and `run_episode`.
//!
//! The RL environment trait (`reset`/`step`), observations, action masking and reward
//! definitions will join them in Stage 3.

#![warn(clippy::pedantic)]

pub mod controller;
pub mod fixed_time;
pub mod runner;

pub use controller::Controller;
pub use fixed_time::FixedTime;
pub use runner::{EnvError, EpisodeReport, SignalCounts, run_episode};
