//! A full scenario: time step, intersection geometry and signal plan.
//!
//! The TOML description ([`ScenarioConfig`]) is validated into an immutable
//! [`Scenario`]. Signal timings are written in seconds and converted once, at load
//! time, to whole simulation steps.

use serde::{Deserialize, Serialize};

use crate::road::{ConfigError, Direction, Intersection, IntersectionConfig, Movement, MovementId};
use crate::signal::{MAX_PHASES, Phase, SignalPlan};

/// Tolerance under which a quotient snaps to the nearest integer, so that
/// `0.9 / 0.3` gives 3 steps and not 4.
const SNAP: f64 = 1e-9;

/// How a duration in seconds is converted to whole steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rounding {
    /// Never shorter than configured.
    Up,
    /// Never longer than configured.
    Down,
}

/// Converts seconds to whole steps, or `None` if the result does not fit in `u32`
/// or is not finite.
fn to_steps(value_s: f64, step_s: f64, rounding: Rounding) -> Option<u32> {
    let quotient = value_s / step_s;
    if !quotient.is_finite() {
        return None;
    }
    let nearest = quotient.round();
    let count = if (quotient - nearest).abs() <= SNAP {
        nearest
    } else {
        match rounding {
            Rounding::Up => quotient.ceil(),
            Rounding::Down => quotient.floor(),
        }
    };
    if !(0.0..=f64::from(u32::MAX)).contains(&count) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // INVARIANT: `count` is finite, integral and within 0..=u32::MAX (checked above),
    // so the conversion is exact.
    let count = count as u32;
    Some(count)
}

/// Raw, unvalidated scenario, as written in TOML.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioConfig {
    /// Length of one simulation step, in seconds.
    pub step_s: f64,
    /// The road geometry.
    pub intersection: IntersectionConfig,
    /// The signal plan.
    pub signal: SignalConfig,
}

/// Raw signal plan: timings in seconds and the phase list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalConfig {
    /// Yellow duration. Rounded up to whole steps, at least 1.
    pub yellow_s: f64,
    /// All-red clearance duration. Rounded up to whole steps, may be 0.
    pub all_red_s: f64,
    /// Minimum green duration. Rounded up to whole steps, at least 1.
    pub min_green_s: f64,
    /// Maximum red duration. Rounded down to whole steps, at least 1.
    pub max_red_s: f64,
    /// The phases. The first one is green at step 0.
    pub phases: Vec<PhaseConfig>,
}

/// Raw description of one phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseConfig {
    /// A unique, non-empty name.
    pub name: String,
    /// The movements that are green in this phase, per approach.
    pub green: GreenConfig,
}

/// The movements a phase makes green, per approach. Missing approaches grant nothing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GreenConfig {
    /// Movements from the north approach.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub north: Vec<Movement>,
    /// Movements from the east approach.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub east: Vec<Movement>,
    /// Movements from the south approach.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub south: Vec<Movement>,
    /// Movements from the west approach.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub west: Vec<Movement>,
}

impl GreenConfig {
    fn get(&self, d: Direction) -> &[Movement] {
        match d {
            Direction::North => &self.north,
            Direction::East => &self.east,
            Direction::South => &self.south,
            Direction::West => &self.west,
        }
    }

    /// Builds the config that grants the movements set in `mask`, in canonical order.
    fn from_mask(mask: u16) -> Self {
        let movements = |d: Direction| {
            Movement::ALL
                .into_iter()
                .filter(|&m| mask & (1 << MovementId::new(d, m).index()) != 0)
                .collect()
        };
        Self {
            north: movements(Direction::North),
            east: movements(Direction::East),
            south: movements(Direction::South),
            west: movements(Direction::West),
        }
    }
}

impl ScenarioConfig {
    /// Parses a TOML string without validating it.
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

    /// Checks every rule and builds the validated scenario.
    ///
    /// # Errors
    ///
    /// Returns the first [`ConfigError`] found, in the order documented on [`ConfigError`].
    pub fn validate(&self) -> Result<Scenario, ConfigError> {
        let step_s = self.step_s;
        if !(step_s.is_finite() && step_s > 0.0) {
            return Err(ConfigError::InvalidStepLength { value: step_s });
        }
        let intersection = self.intersection.validate_at("intersection.")?;
        let signal_plan = validate_signal(&self.signal, step_s, &intersection)?;
        Ok(Scenario {
            step_s,
            intersection,
            signal_plan,
            config: self.normalized(),
        })
    }

    /// The same config with each phase's movements in canonical order.
    fn normalized(&self) -> Self {
        let mut config = self.clone();
        for phase in &mut config.signal.phases {
            for movements in [
                &mut phase.green.north,
                &mut phase.green.east,
                &mut phase.green.south,
                &mut phase.green.west,
            ] {
                movements.sort_unstable();
            }
        }
        config
    }
}

/// A validated, immutable scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
    step_s: f64,
    intersection: Intersection,
    signal_plan: SignalPlan,
    config: ScenarioConfig,
}

impl Scenario {
    /// Parses and validates a TOML description.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if the text is not a valid config or breaks a
    /// validation rule.
    pub fn from_toml_str(s: &str) -> Result<Self, ConfigError> {
        ScenarioConfig::from_toml_str(s)?.validate()
    }

    /// Length of one simulation step, in seconds.
    #[must_use]
    pub const fn step_s(&self) -> f64 {
        self.step_s
    }

    /// The road geometry.
    #[must_use]
    pub const fn intersection(&self) -> &Intersection {
        &self.intersection
    }

    /// The signal plan, with timings in steps.
    #[must_use]
    pub const fn signal_plan(&self) -> &SignalPlan {
        &self.signal_plan
    }

    /// The config this scenario was built from, so a run can record it exactly.
    ///
    /// Phase movements are listed in canonical order, and the durations are the
    /// configured seconds, not the rounded steps.
    #[must_use]
    pub fn to_config(&self) -> ScenarioConfig {
        let mut config = self.config.clone();
        // The plan is the source of truth for the phases.
        config.signal.phases = self
            .signal_plan
            .phases()
            .iter()
            .map(|p| PhaseConfig {
                name: p.name().to_owned(),
                green: GreenConfig::from_mask(p.mask()),
            })
            .collect();
        config
    }
}

impl TryFrom<ScenarioConfig> for Scenario {
    type Error = ConfigError;

    fn try_from(config: ScenarioConfig) -> Result<Self, Self::Error> {
        config.validate()
    }
}

/// Converts one duration field, checking it against `min_steps`.
fn convert(
    path: &str,
    value: f64,
    step_s: f64,
    rounding: Rounding,
    min_steps: u32,
) -> Result<u32, ConfigError> {
    if !(value.is_finite() && value >= 0.0) {
        return Err(ConfigError::InvalidDuration {
            path: path.to_owned(),
            value,
        });
    }
    match to_steps(value, step_s, rounding) {
        Some(n) if n >= min_steps => Ok(n),
        other => Err(ConfigError::DurationOutOfRange {
            path: path.to_owned(),
            value,
            step_s,
            steps: other.map_or_else(|| value / step_s, f64::from),
            min_steps,
        }),
    }
}

fn validate_signal(
    signal: &SignalConfig,
    step_s: f64,
    intersection: &Intersection,
) -> Result<SignalPlan, ConfigError> {
    let yellow = convert("signal.yellow_s", signal.yellow_s, step_s, Rounding::Up, 1)?;
    let all_red = convert(
        "signal.all_red_s",
        signal.all_red_s,
        step_s,
        Rounding::Up,
        0,
    )?;
    let min_green = convert(
        "signal.min_green_s",
        signal.min_green_s,
        step_s,
        Rounding::Up,
        1,
    )?;
    let max_red = convert(
        "signal.max_red_s",
        signal.max_red_s,
        step_s,
        Rounding::Down,
        1,
    )?;

    let count = signal.phases.len();
    if !(1..=MAX_PHASES).contains(&count) {
        return Err(ConfigError::PhaseCount {
            path: "signal.phases".to_owned(),
            count,
        });
    }

    let mut phases: Vec<Phase> = Vec::with_capacity(count);
    for (i, phase) in signal.phases.iter().enumerate() {
        let base = format!("signal.phases[{i}]");
        phases.push(validate_phase(&base, phase, intersection, &phases)?);
    }

    // Coverage: every movement some lane allows is granted by some phase.
    let covered = phases.iter().fold(0u16, |acc, p| acc | p.mask());
    for m in MovementId::ALL {
        let allowed = intersection
            .approach(m.approach)
            .lanes()
            .iter()
            .any(|l| l.movements().contains(&m.movement));
        if allowed && covered & (1 << m.index()) == 0 {
            return Err(ConfigError::UncoveredMovement { movement: m });
        }
    }

    // Feasibility: with n phases, a phase that leaves green waits for the n - 1 others
    // to be served (min-green each) and for n switches (yellow + all-red each).
    if count > 1 {
        let n = count as u64;
        let (y_a, g) = (u64::from(yellow) + u64::from(all_red), u64::from(min_green));
        let required_steps = n * y_a + (n - 1) * g;
        if u64::from(max_red) < required_steps {
            #[allow(clippy::cast_precision_loss)]
            // INVARIANT: `required_steps` is at most 8 * 2 * u32::MAX + 7 * u32::MAX,
            // far below 2^53, so it is exact as an `f64`.
            let required_s = required_steps as f64 * step_s;
            return Err(ConfigError::MaxRedTooShort {
                phases: count,
                required_s,
                required_steps,
                got_steps: max_red,
            });
        }
    }

    Ok(SignalPlan::new(phases, yellow, all_red, min_green, max_red))
}

/// Validates one phase against the geometry and against the phases before it.
fn validate_phase(
    base: &str,
    phase: &PhaseConfig,
    intersection: &Intersection,
    earlier: &[Phase],
) -> Result<Phase, ConfigError> {
    let name_path = format!("{base}.name");
    if phase.name.is_empty() {
        return Err(ConfigError::EmptyPhaseName { path: name_path });
    }
    if earlier.iter().any(|p| p.name() == phase.name) {
        return Err(ConfigError::DuplicatePhaseName {
            path: name_path,
            name: phase.name.clone(),
        });
    }

    let green_path = format!("{base}.green");
    let mut mask = 0u16;
    for d in Direction::ALL {
        for (j, &movement) in phase.green.get(d).iter().enumerate() {
            let path = format!("{green_path}.{d}[{j}]");
            let id = MovementId::new(d, movement);
            let bit = 1u16 << id.index();
            if mask & bit != 0 {
                return Err(ConfigError::DuplicatePhaseMovement { path, movement });
            }
            let in_geometry = intersection
                .approach(d)
                .lanes()
                .iter()
                .any(|l| l.movements().contains(&movement));
            if !in_geometry {
                return Err(ConfigError::MovementNotInGeometry { path, movement });
            }
            mask |= bit;
        }
    }
    if mask == 0 {
        return Err(ConfigError::EmptyPhase { path: green_path });
    }

    for (a, &first) in MovementId::ALL.iter().enumerate() {
        for &second in &MovementId::ALL[a + 1..] {
            if mask & (1 << first.index()) != 0
                && mask & (1 << second.index()) != 0
                && first.conflicts_with(second)
            {
                return Err(ConfigError::ConflictingMovements {
                    path: green_path,
                    first,
                    second,
                });
            }
        }
    }

    if let Some(other) = earlier.iter().find(|p| p.mask() == mask) {
        return Err(ConfigError::DuplicatePhase {
            path: green_path,
            other: other.name().to_owned(),
        });
    }

    Ok(Phase::new(phase.name.clone(), mask))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_directions_and_snap() {
        assert_eq!(to_steps(0.9, 0.3, Rounding::Up), Some(3));
        assert_eq!(to_steps(0.9, 0.3, Rounding::Down), Some(3));
        assert_eq!(to_steps(2.5, 1.0, Rounding::Up), Some(3));
        assert_eq!(to_steps(2.5, 1.0, Rounding::Down), Some(2));
        assert_eq!(to_steps(0.0, 1.0, Rounding::Up), Some(0));
        assert_eq!(to_steps(1e12, 1.0, Rounding::Up), None);
        assert_eq!(to_steps(f64::NAN, 1.0, Rounding::Up), None);
    }
}
