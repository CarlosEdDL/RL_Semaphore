//! Vehicles, their identifiers and the records the simulation hands out.
//!
//! Vehicles are created by [`Simulation::spawn`](crate::Simulation::spawn) and
//! move only inside [`Simulation::step`](crate::Simulation::step) (ADR-0004).

use std::fmt;

use crate::road::{LaneId, MovementId};

/// Identifies a vehicle. Ids are assigned from 0 in spawn order and never reused
/// within a simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VehicleId(u64);

impl VehicleId {
    /// Wraps a raw id. Only the simulation assigns ids to vehicles.
    pub(crate) const fn new(id: u64) -> Self {
        Self(id)
    }

    /// The raw id.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for VehicleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

/// Where a vehicle is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// Spawned, but waiting outside its lane for cell 0 to be free.
    Backlog,
    /// On its lane at `cell` (0 is the upstream entry, the lane's stop-line cell is the last).
    OnLane {
        /// The cell index along the lane.
        cell: u32,
    },
}

/// A vehicle in the simulation. Its lane and movement are fixed for its life.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vehicle {
    pub(crate) id: VehicleId,
    pub(crate) movement: MovementId,
    pub(crate) lane: LaneId,
    pub(crate) spawned_at: u64,
    pub(crate) position: Position,
}

impl Vehicle {
    /// The vehicle's id.
    #[must_use]
    pub const fn id(&self) -> VehicleId {
        self.id
    }

    /// The approach and movement the vehicle wants to perform.
    #[must_use]
    pub const fn movement(&self) -> MovementId {
        self.movement
    }

    /// The lane the vehicle was assigned at spawn.
    #[must_use]
    pub const fn lane(&self) -> LaneId {
        self.lane
    }

    /// The step count at which the vehicle was spawned.
    #[must_use]
    pub const fn spawned_at(&self) -> u64 {
        self.spawned_at
    }

    /// Where the vehicle is now.
    #[must_use]
    pub const fn position(&self) -> Position {
        self.position
    }
}

/// A vehicle that crossed the stop line and left the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Departure {
    /// The vehicle that left.
    pub vehicle: VehicleId,
    /// Its approach and movement.
    pub movement: MovementId,
    /// The lane it crossed from.
    pub lane: LaneId,
    /// The step count at which it was spawned.
    pub spawned_at: u64,
    /// The step count after the step in which it crossed.
    pub departed_at: u64,
}

/// Why a vehicle could not be spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SpawnError {
    /// No lane of the approach allows the movement.
    #[error("no lane of the {} approach allows the {} movement", .movement.approach, .movement.movement)]
    NoLaneForMovement {
        /// The requested approach and movement.
        movement: MovementId,
    },
}
