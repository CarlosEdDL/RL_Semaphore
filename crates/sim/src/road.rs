//! Static geometry of a single 4-way intersection.
//!
//! A TOML description ([`IntersectionConfig`]) is validated into an immutable,
//! discretized [`Intersection`]. Lanes are split into fixed-length cells (see
//! ADR-0002), with a stop line at the downstream end of every lane.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Maximum number of lanes in one approach.
pub const MAX_LANES_PER_APPROACH: usize = 4;

/// A side of the intersection.
///
/// An approach is named by the side vehicles come *from*: the `North` approach
/// carries southbound traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

impl Direction {
    /// All directions in clockwise order: north, east, south, west.
    pub const ALL: [Direction; 4] = [
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ];

    /// Lowercase name, as used in the TOML schema and in field paths.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Direction::North => "north",
            Direction::East => "east",
            Direction::South => "south",
            Direction::West => "west",
        }
    }

    /// Position in [`Direction::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The direction `steps` quarter turns clockwise (north, east, south, west order).
    #[must_use]
    pub const fn rotate_cw(self, steps: usize) -> Direction {
        Self::ALL[(self.index() + steps) % 4]
    }

    /// The side a vehicle exits to when it enters from `self` and performs
    /// `movement`, for right-hand traffic.
    ///
    /// From `North`, `Left` exits `East`, `Through` exits `South` and `Right`
    /// exits `West`; the other approaches are rotations of that pattern.
    #[must_use]
    pub const fn destination(self, movement: Movement) -> Direction {
        // Clockwise steps from the entry side: left is +1, through +2, right +3.
        let steps = match movement {
            Movement::Left => 1,
            Movement::Through => 2,
            Movement::Right => 3,
        };
        self.rotate_cw(steps)
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What a vehicle does at the intersection. U-turns are not a movement.
///
/// The declaration order (`Left < Through < Right`) is used by the lane
/// consistency rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Movement {
    /// Turn left.
    Left,
    /// Go straight.
    Through,
    /// Turn right.
    Right,
}

impl Movement {
    /// All movements in declaration order: left, through, right.
    pub const ALL: [Movement; 3] = [Movement::Left, Movement::Through, Movement::Right];

    /// Position in [`Movement::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for Movement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Movement::Left => "left",
            Movement::Through => "through",
            Movement::Right => "right",
        })
    }
}

/// One of the 12 movements of the intersection: a [`Movement`] made from an approach.
///
/// Per-movement state can live in `[T; 12]` arrays indexed by [`MovementId::index`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MovementId {
    /// The approach the vehicle comes from.
    pub approach: Direction,
    /// What the vehicle does at the intersection.
    pub movement: Movement,
}

impl MovementId {
    /// Number of movements.
    pub const COUNT: usize = 12;

    /// All movements: approaches in [`Direction::ALL`] order, then movements in
    /// `Left, Through, Right` order. `ALL[i].index() == i`.
    pub const ALL: [MovementId; Self::COUNT] = {
        let mut all = [MovementId {
            approach: Direction::North,
            movement: Movement::Left,
        }; Self::COUNT];
        let mut i = 0;
        while i < Self::COUNT {
            all[i] = MovementId {
                approach: Direction::ALL[i / 3],
                movement: Movement::ALL[i % 3],
            };
            i += 1;
        }
        all
    };

    /// Builds a movement id.
    #[must_use]
    pub const fn new(approach: Direction, movement: Movement) -> Self {
        Self { approach, movement }
    }

    /// Dense index in `0..12`, matching [`MovementId::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        self.approach.index() * 3 + self.movement.index()
    }

    /// Whether two movements cannot be green together: their paths cross, or they
    /// exit to the same side. Right-hand traffic, protected turns only (a left turn
    /// yields to nobody, so it conflicts with every movement it could meet).
    ///
    /// The relation is irreflexive and symmetric, and two movements of the same
    /// approach never conflict. For a movement from approach `d`, *opposing* is the
    /// approach across (`d` rotated by 2), *left-side* is the approach on the
    /// driver's left (rotated by 1, for example east for north) and *right-side*
    /// is the approach on the driver's right (rotated by 3). The conflicts are:
    ///
    /// | Movement from `d` | Opposing | Left-side | Right-side |
    /// |---|---|---|---|
    /// | `Left` | `Through`, `Right` | `Through`, `Left` | `Through`, `Left` |
    /// | `Through` | `Left` | `Through`, `Left` | `Through`, `Left`, `Right` |
    /// | `Right` | `Left` | `Through` | none |
    ///
    /// That is 14 conflicts per approach and 28 unordered conflicting pairs.
    #[must_use]
    pub const fn conflicts_with(self, other: MovementId) -> bool {
        use Movement::{Left, Right, Through};
        // Clockwise quarter turns from `self.approach` to `other.approach`.
        let rel = (other.approach.index() + 4 - self.approach.index()) % 4;
        matches!(
            (self.movement, rel, other.movement),
            (Left, 2, Through | Right)
                | (Left | Through, 1 | 3, Through | Left)
                | (Through | Right, 2, Left)
                | (Through, 3, Right)
                | (Right, 1, Through)
        )
    }
}

impl fmt::Display for MovementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.approach, self.movement)
    }
}

/// Identifies a lane by its approach and index. Index 0 is the leftmost lane
/// (closest to the centre line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LaneId {
    approach: Direction,
    index: u8,
}

impl LaneId {
    /// The approach the lane belongs to.
    #[must_use]
    pub const fn approach(self) -> Direction {
        self.approach
    }

    /// The lane index within the approach, 0 being the leftmost lane.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index as usize
    }
}

/// A validated lane.
///
/// Cell positions are integers: cell 0 is the upstream entry and cell
/// `len_cells() - 1` is the cell immediately behind the stop line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lane {
    id: LaneId,
    movements: BTreeSet<Movement>,
    len_cells: u32,
}

impl Lane {
    /// The lane's identifier.
    #[must_use]
    pub const fn id(&self) -> LaneId {
        self.id
    }

    /// The movements vehicles in this lane may perform (never empty).
    #[must_use]
    pub const fn movements(&self) -> &BTreeSet<Movement> {
        &self.movements
    }

    /// Length of the lane in cells (at least 2).
    #[must_use]
    pub const fn len_cells(&self) -> u32 {
        self.len_cells
    }

    /// The cell immediately behind the stop line, which is the last cell of
    /// the lane. Cell 0 is the upstream entry.
    #[must_use]
    pub const fn stop_line_cell(&self) -> u32 {
        self.len_cells - 1
    }
}

/// A validated approach: one side of the intersection with its lanes.
#[derive(Debug, Clone, PartialEq)]
pub struct Approach {
    direction: Direction,
    length_m: f64,
    lanes: Vec<Lane>,
}

impl Approach {
    /// The side vehicles come from.
    #[must_use]
    pub const fn direction(&self) -> Direction {
        self.direction
    }

    /// Length of the approach in meters, as configured (the cell count drops the remainder).
    #[must_use]
    pub const fn length_m(&self) -> f64 {
        self.length_m
    }

    /// Lanes in index order, leftmost first (never empty).
    #[must_use]
    pub fn lanes(&self) -> &[Lane] {
        &self.lanes
    }
}

/// A validated, immutable single 4-way intersection.
#[derive(Debug, Clone, PartialEq)]
pub struct Intersection {
    cell_length_m: f64,
    approaches: [Approach; 4],
}

impl Intersection {
    /// Length of one cell in meters.
    #[must_use]
    pub const fn cell_length_m(&self) -> f64 {
        self.cell_length_m
    }

    /// The approach coming from `direction`.
    #[must_use]
    pub fn approach(&self, direction: Direction) -> &Approach {
        &self.approaches[direction.index()]
    }

    /// The four approaches in [`Direction::ALL`] order.
    pub fn approaches(&self) -> impl Iterator<Item = &Approach> {
        self.approaches.iter()
    }

    /// Parses and validates a TOML description.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if the text is not a valid config or breaks a
    /// validation rule.
    pub fn from_toml_str(s: &str) -> Result<Self, ConfigError> {
        IntersectionConfig::from_toml_str(s)?.validate()
    }

    /// The config this model was built from, so a run can record it exactly.
    #[must_use]
    pub fn to_config(&self) -> IntersectionConfig {
        let approach = |d: Direction| {
            let a = self.approach(d);
            ApproachConfig {
                length_m: a.length_m,
                lanes: a
                    .lanes
                    .iter()
                    .map(|l| LaneConfig {
                        movements: l.movements.iter().copied().collect(),
                    })
                    .collect(),
            }
        };
        IntersectionConfig {
            cell_length_m: self.cell_length_m(),
            approaches: ApproachesConfig {
                north: approach(Direction::North),
                east: approach(Direction::East),
                south: approach(Direction::South),
                west: approach(Direction::West),
            },
        }
    }
}

impl TryFrom<IntersectionConfig> for Intersection {
    type Error = ConfigError;

    fn try_from(config: IntersectionConfig) -> Result<Self, Self::Error> {
        config.validate()
    }
}

/// Raw, unvalidated intersection description, as written in TOML.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntersectionConfig {
    /// Length of one cell in meters.
    pub cell_length_m: f64,
    /// The four approaches.
    pub approaches: ApproachesConfig,
}

/// The four approaches of an [`IntersectionConfig`]; all are required.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApproachesConfig {
    /// Traffic coming from the north.
    pub north: ApproachConfig,
    /// Traffic coming from the east.
    pub east: ApproachConfig,
    /// Traffic coming from the south.
    pub south: ApproachConfig,
    /// Traffic coming from the west.
    pub west: ApproachConfig,
}

impl ApproachesConfig {
    fn get(&self, d: Direction) -> &ApproachConfig {
        match d {
            Direction::North => &self.north,
            Direction::East => &self.east,
            Direction::South => &self.south,
            Direction::West => &self.west,
        }
    }
}

/// Raw description of one approach.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApproachConfig {
    /// Approach length in meters.
    pub length_m: f64,
    /// Lanes from left to right.
    pub lanes: Vec<LaneConfig>,
}

/// Raw description of one lane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaneConfig {
    /// Allowed movements. A list, so duplicates can be reported.
    pub movements: Vec<Movement>,
}

/// Why a config could not be loaded.
///
/// Validation stops at the first error. For an intersection it checks the cell
/// length, then the approaches in [`Direction::ALL`] order, then lanes in index
/// order. A scenario checks `step_s`, the intersection (paths prefixed with
/// `intersection.`), the signal timings, the phases, movement coverage and the
/// max-red feasibility rule, in that order, and then the demand: approaches in
/// [`Direction::ALL`] order, and for each `veh_per_h`, then `left`, `through` and
/// `right` (range, then geometry), then the sum of the ratios and the mean number
/// of arrivals per step. Last comes the optional fixed-time plan: the number of
/// entries, then each green in order (finite and positive, then at least
/// min-green), then the cycle against max-red (phases in order), then the size of
/// the cycle.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// The text is not valid TOML or does not match the schema (missing or unknown keys).
    #[error("invalid config: {0}")]
    Parse(#[from] toml::de::Error),
    /// `cell_length_m` is not finite and strictly positive.
    #[error("{path} must be finite and greater than 0, got {value}")]
    InvalidCellLength {
        /// Field path.
        path: String,
        /// The offending value.
        value: f64,
    },
    /// An approach length is not finite and strictly positive.
    #[error("{path} must be finite and greater than 0, got {value}")]
    InvalidLength {
        /// Field path.
        path: String,
        /// The offending value.
        value: f64,
    },
    /// An approach has fewer than 2 cells, or too many to index.
    #[error(
        "{path} gives {cells} cells of the configured size; it needs at least 2 (and at most {})",
        u32::MAX
    )]
    BadCellCount {
        /// Field path.
        path: String,
        /// The number of cells (`floor(length_m / cell_length_m)`).
        cells: f64,
    },
    /// An approach has no lanes or more than [`MAX_LANES_PER_APPROACH`].
    #[error("{path} must have between 1 and {MAX_LANES_PER_APPROACH} lanes, got {count}")]
    LaneCount {
        /// Field path.
        path: String,
        /// The number of lanes.
        count: usize,
    },
    /// A lane allows no movement.
    #[error("{path} must list at least one movement")]
    EmptyMovements {
        /// Field path.
        path: String,
    },
    /// A lane lists the same movement twice.
    #[error("{path} lists \"{movement}\" more than once")]
    DuplicateMovement {
        /// Field path.
        path: String,
        /// The repeated movement.
        movement: Movement,
    },
    /// Two adjacent lanes would make turn paths cross: with right-hand traffic
    /// the lane on the left must not allow a movement further right than the
    /// lane on its right.
    #[error(
        "{path}: lane {left_lane} allows \"{left_max}\" but the lane to its right (lane {}) only starts at \"{right_min}\"; \
         movements must not cross, so they must go left to right",
        left_lane + 1
    )]
    CrossingMovements {
        /// Field path of the right-hand lane's movements.
        path: String,
        /// Index of the left lane of the pair.
        left_lane: usize,
        /// Greatest movement of the left lane.
        left_max: Movement,
        /// Smallest movement of the right lane.
        right_min: Movement,
    },
    /// `step_s` is not finite and strictly positive.
    #[error("step_s must be finite and greater than 0, got {value}")]
    InvalidStepLength {
        /// The offending value.
        value: f64,
    },
    /// A signal duration is not finite, or is negative.
    #[error("{path} must be finite and not negative, got {value}")]
    InvalidDuration {
        /// Field path.
        path: String,
        /// The offending value, in seconds.
        value: f64,
    },
    /// A signal duration converts to too few or too many steps.
    #[error(
        "{path} = {value} s is {steps} steps of {step_s} s; it must be between {min_steps} and {} steps",
        u32::MAX
    )]
    DurationOutOfRange {
        /// Field path.
        path: String,
        /// The offending value, in seconds.
        value: f64,
        /// The simulation step, in seconds.
        step_s: f64,
        /// The converted number of steps (rounded as the field requires).
        steps: f64,
        /// The smallest allowed number of steps.
        min_steps: u32,
    },
    /// The plan has no phases or more than [`MAX_PHASES`](crate::MAX_PHASES).
    #[error(
        "{path} must have between 1 and {} phases, got {count}",
        crate::MAX_PHASES
    )]
    PhaseCount {
        /// Field path.
        path: String,
        /// The number of phases.
        count: usize,
    },
    /// A phase name is empty.
    #[error("{path} must not be empty")]
    EmptyPhaseName {
        /// Field path.
        path: String,
    },
    /// Two phases have the same name.
    #[error("{path}: another phase is already named \"{name}\"")]
    DuplicatePhaseName {
        /// Field path.
        path: String,
        /// The repeated name.
        name: String,
    },
    /// A phase grants no movement.
    #[error("{path} must grant at least one movement")]
    EmptyPhase {
        /// Field path of the phase's `green` table.
        path: String,
    },
    /// A phase lists the same movement twice for an approach.
    #[error("{path} lists \"{movement}\" more than once")]
    DuplicatePhaseMovement {
        /// Field path of the repeated entry.
        path: String,
        /// The repeated movement.
        movement: Movement,
    },
    /// A phase grants a movement that no lane of the approach allows.
    #[error("{path}: no lane of that approach allows \"{movement}\"")]
    MovementNotInGeometry {
        /// Field path of the entry.
        path: String,
        /// The movement.
        movement: Movement,
    },
    /// A phase grants two movements that conflict.
    #[error("{path}: {first} and {second} conflict and cannot be green together")]
    ConflictingMovements {
        /// Field path of the phase's `green` table.
        path: String,
        /// The first movement, in [`MovementId::ALL`] order.
        first: MovementId,
        /// The second movement.
        second: MovementId,
    },
    /// Two phases grant exactly the same movements.
    #[error("{path} grants the same movements as phase \"{other}\"")]
    DuplicatePhase {
        /// Field path of the phase's `green` table.
        path: String,
        /// The name of the earlier phase.
        other: String,
    },
    /// A movement that some lane allows is never green.
    #[error("no phase grants {movement}, which a lane of the intersection allows")]
    UncoveredMovement {
        /// The first uncovered movement, in [`MovementId::ALL`] order.
        movement: MovementId,
    },
    /// `max_red_s` is too short for the phases to take turns.
    #[error(
        "signal.max_red_s is too short: {phases} phases need at least {required_s} s ({required_steps} steps) \
         to serve all of them, but it converts to {got_steps} steps"
    )]
    MaxRedTooShort {
        /// The number of phases.
        phases: usize,
        /// The required minimum, in seconds.
        required_s: f64,
        /// The required minimum, in steps.
        required_steps: u64,
        /// What `max_red_s` converts to, in steps.
        got_steps: u32,
    },
    /// `fixed_time.green_s` does not have one entry per phase.
    #[error("{path} must have one entry per phase ({expected}), got {got}")]
    FixedTimeLength {
        /// Field path.
        path: String,
        /// The number of phases.
        expected: usize,
        /// The number of entries.
        got: usize,
    },
    /// A fixed-time green is not finite, not greater than 0, or too long to count in steps.
    #[error(
        "{path} must be finite, greater than 0 and at most {} steps long, got {value}",
        u32::MAX
    )]
    InvalidFixedTimeGreen {
        /// Field path.
        path: String,
        /// The offending value, in seconds.
        value: f64,
    },
    /// A fixed-time green is shorter than min-green.
    #[error("{path} = {value} s is {steps} steps, below signal.min_green_s ({min_steps} steps)")]
    FixedTimeGreenTooShort {
        /// Field path.
        path: String,
        /// The offending value, in seconds.
        value: f64,
        /// The value in steps.
        steps: u32,
        /// Min-green in steps.
        min_steps: u32,
    },
    /// The fixed-time cycle keeps a phase red for longer than max-red.
    #[error(
        "{path}: the cycle keeps this phase red for {red_steps} steps, above signal.max_red_s ({max_red_steps} steps)"
    )]
    FixedTimeCycleExceedsMaxRed {
        /// Field path of the phase's green.
        path: String,
        /// Entries the phase stays not green in each cycle (`C - green`).
        red_steps: u64,
        /// Max-red in steps.
        max_red_steps: u32,
    },
    /// The fixed-time cycle does not fit in `u32` steps.
    #[error("{path}: the cycle is longer than {} steps", u32::MAX)]
    FixedTimeCycleTooLong {
        /// Field path.
        path: String,
    },
    /// A flow is not finite, or is negative.
    #[error("{path} must be finite and not negative, got {value}")]
    InvalidFlow {
        /// Field path.
        path: String,
        /// The offending value, in vehicles per hour.
        value: f64,
    },
    /// A turn ratio is not finite or is outside 0 to 1.
    #[error("{path} must be finite and between 0 and 1, got {value}")]
    InvalidTurnRatio {
        /// Field path.
        path: String,
        /// The offending value.
        value: f64,
    },
    /// The turn ratios of an approach do not sum to 1.
    #[error("{path}: left + through + right must sum to 1, got {sum}")]
    TurnRatioSum {
        /// Field path of the approach entry.
        path: String,
        /// The sum of the three ratios.
        sum: f64,
    },
    /// A flow gives more arrivals per step than the sampler accepts.
    #[error(
        "{path} = {veh_per_h} veh/h is {mean_per_step} arrivals per step of {step_s} s; \
         it must be at most {max}"
    )]
    FlowTooHigh {
        /// Field path.
        path: String,
        /// The offending value, in vehicles per hour.
        veh_per_h: f64,
        /// The simulation step, in seconds.
        step_s: f64,
        /// The mean number of arrivals per step.
        mean_per_step: f64,
        /// The largest allowed mean.
        max: f64,
    },
}

impl IntersectionConfig {
    /// Parses a TOML string without validating geometry.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Parse`] on invalid TOML, missing keys or unknown keys.
    pub fn from_toml_str(s: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(s)?)
    }

    /// Serializes the config to TOML.
    ///
    /// # Errors
    ///
    /// Returns the serializer error, which cannot happen for a config that
    /// holds finite numbers.
    pub fn to_toml_string(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }

    /// Checks every rule and builds the discretized model.
    ///
    /// The number of cells of an approach is `floor(length_m / cell_length_m)`;
    /// any remainder is dropped.
    ///
    /// # Errors
    ///
    /// Returns the first [`ConfigError`] found.
    pub fn validate(&self) -> Result<Intersection, ConfigError> {
        self.validate_at("")
    }

    /// Like [`validate`](Self::validate), with `prefix` (for example
    /// `"intersection."`) put in front of every field path in errors.
    pub(crate) fn validate_at(&self, prefix: &str) -> Result<Intersection, ConfigError> {
        let cell = self.cell_length_m;
        if !(cell.is_finite() && cell > 0.0) {
            return Err(ConfigError::InvalidCellLength {
                path: format!("{prefix}cell_length_m"),
                value: cell,
            });
        }
        let build = |d: Direction| validate_approach(d, self.approaches.get(d), cell, prefix);
        Ok(Intersection {
            cell_length_m: cell,
            approaches: [
                build(Direction::North)?,
                build(Direction::East)?,
                build(Direction::South)?,
                build(Direction::West)?,
            ],
        })
    }
}

fn validate_approach(
    direction: Direction,
    config: &ApproachConfig,
    cell_length_m: f64,
    prefix: &str,
) -> Result<Approach, ConfigError> {
    let base = format!("{prefix}approaches.{direction}");

    let length = config.length_m;
    if !(length.is_finite() && length > 0.0) {
        return Err(ConfigError::InvalidLength {
            path: format!("{base}.length_m"),
            value: length,
        });
    }
    let cells = (length / cell_length_m).floor();
    if !(2.0..=f64::from(u32::MAX)).contains(&cells) {
        return Err(ConfigError::BadCellCount {
            path: format!("{base}.length_m"),
            cells,
        });
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // INVARIANT: `cells` is finite, integral and within 2..=u32::MAX (checked above),
    // so the conversion is exact.
    let len_cells = cells as u32;

    let count = config.lanes.len();
    if !(1..=MAX_LANES_PER_APPROACH).contains(&count) {
        return Err(ConfigError::LaneCount {
            path: format!("{base}.lanes"),
            count,
        });
    }

    let mut lanes: Vec<Lane> = Vec::with_capacity(count);
    for (i, lane) in config.lanes.iter().enumerate() {
        let path = format!("{base}.lanes[{i}].movements");
        let mut movements = BTreeSet::new();
        for &m in &lane.movements {
            if !movements.insert(m) {
                return Err(ConfigError::DuplicateMovement { path, movement: m });
            }
        }
        let Some(&min) = movements.first() else {
            return Err(ConfigError::EmptyMovements { path });
        };
        // INVARIANT: every stored lane has a non-empty movement set (checked above),
        // so `last()` on the previous lane is always `Some`.
        if let Some(prev) = lanes.last()
            && let Some(&left_max) = prev.movements.last()
            && left_max > min
        {
            return Err(ConfigError::CrossingMovements {
                path,
                left_lane: i - 1,
                left_max,
                right_min: min,
            });
        }
        lanes.push(Lane {
            id: LaneId {
                approach: direction,
                // INVARIANT: `i < MAX_LANES_PER_APPROACH` (4), which fits in `u8`.
                index: u8::try_from(i).unwrap_or(u8::MAX),
            },
            movements,
            len_cells,
        });
    }

    Ok(Approach {
        direction,
        length_m: length,
        lanes,
    })
}
