//! Pure, deterministic traffic simulation: road graph, vehicles, signals, demand generation and metrics. No I/O and no async.

#![warn(clippy::pedantic)]

pub mod road;
pub mod scenario;
pub mod signal;
pub mod simulation;
pub mod vehicle;

pub use road::{
    Approach, ApproachConfig, ApproachesConfig, ConfigError, Direction, Intersection,
    IntersectionConfig, Lane, LaneConfig, LaneId, MAX_LANES_PER_APPROACH, Movement, MovementId,
};
pub use scenario::{GreenConfig, PhaseConfig, Scenario, ScenarioConfig, SignalConfig};
pub use signal::{
    Command, IgnoredReason, Light, MAX_PHASES, Phase, PhaseId, Signal, SignalPlan, SignalState,
    StepOutcome,
};
pub use simulation::{Simulation, StepReport};
pub use vehicle::{Departure, Position, SpawnError, Vehicle, VehicleId};
