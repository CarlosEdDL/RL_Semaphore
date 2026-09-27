//! Pure, deterministic traffic simulation: road graph, vehicles, signals, demand generation and metrics. No I/O and no async.

#![warn(clippy::pedantic)]

pub mod road;

pub use road::{
    Approach, ApproachConfig, ApproachesConfig, ConfigError, Direction, Intersection,
    IntersectionConfig, Lane, LaneConfig, LaneId, MAX_LANES_PER_APPROACH, Movement,
};
