//! Types shared by [`crate::layout`], [`crate::snapshot`] and [`crate::metrics`]: sides and
//! turns of the intersection, signal-head colors, lane and movement references, and the
//! four-approach container every static or dynamic per-approach value uses.

use serde::{Deserialize, Serialize};

/// A side of the intersection, named for where traffic comes *from*: the `North` approach
/// carries southbound traffic. Encoded as a lowercase string (`"north"`, `"east"`, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// The north side.
    North,
    /// The east side.
    East,
    /// The south side.
    South,
    /// The west side.
    West,
}

/// What a vehicle does at the intersection. Encoded as a lowercase string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Movement {
    /// Turn left.
    Left,
    /// Go straight.
    Through,
    /// Turn right.
    Right,
}

/// What a signal head shows for one movement. Encoded as a lowercase string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Light {
    /// Stop.
    Red,
    /// Clearing: the movement is about to turn red.
    Yellow,
    /// Go.
    Green,
}

/// Identifies a lane by its approach and index. Index 0 is the leftmost lane (closest to the
/// centre line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LaneRef {
    /// The approach the lane belongs to.
    pub approach: Direction,
    /// The lane's index within its approach.
    pub index: u32,
}

/// A [`Movement`] made from a given approach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MovementRef {
    /// The approach the vehicle comes from.
    pub approach: Direction,
    /// What the vehicle does at the intersection.
    pub movement: Movement,
}

/// One value per approach, always present in `north, east, south, west` order (R2.5). Used for
/// both static layout data and dynamic per-approach state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ByApproach<T> {
    /// The north approach.
    pub north: T,
    /// The east approach.
    pub east: T,
    /// The south approach.
    pub south: T,
    /// The west approach.
    pub west: T,
}

impl<T> ByApproach<T> {
    /// The value for `direction`.
    #[must_use]
    pub const fn get(&self, direction: Direction) -> &T {
        match direction {
            Direction::North => &self.north,
            Direction::East => &self.east,
            Direction::South => &self.south,
            Direction::West => &self.west,
        }
    }
}
