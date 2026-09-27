//! Demand: how many vehicles arrive at each approach, and where they want to go.
//!
//! The `[demand]` table of the scenario TOML ([`DemandConfig`]) is validated into a
//! [`DemandPlan`]. A [`Demand`] generator draws, for every step, a Poisson number of
//! arrivals per approach and gives each one a movement by the turn ratios. See ADR-0005.

use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::road::{ConfigError, Direction, Intersection, Movement, MovementId};
use crate::scenario::Scenario;

/// The largest mean number of arrivals per approach per step a scenario may ask for.
///
/// It bounds the work of the Poisson sampler.
pub const MAX_MEAN_ARRIVALS_PER_STEP: f64 = 10.0;

/// The ratios of an entry must sum to 1 within this tolerance.
const RATIO_SUM_TOLERANCE: f64 = 1e-9;

/// The Poisson walk stops here even if rounding keeps the running sum below `U`.
const MAX_POISSON_WALK: u32 = 200;

/// Raw demand of the whole intersection, as written in the `[demand]` TOML table.
///
/// A missing approach has no arrivals.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemandConfig {
    /// Demand of the north approach.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub north: Option<ApproachDemandConfig>,
    /// Demand of the east approach.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub east: Option<ApproachDemandConfig>,
    /// Demand of the south approach.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub south: Option<ApproachDemandConfig>,
    /// Demand of the west approach.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub west: Option<ApproachDemandConfig>,
}

impl DemandConfig {
    /// True when no approach has an entry.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.north.is_none() && self.east.is_none() && self.south.is_none() && self.west.is_none()
    }

    const fn get(&self, d: Direction) -> Option<&ApproachDemandConfig> {
        match d {
            Direction::North => self.north.as_ref(),
            Direction::East => self.east.as_ref(),
            Direction::South => self.south.as_ref(),
            Direction::West => self.west.as_ref(),
        }
    }
}

/// Raw demand of one approach: a flow and the turn ratios.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApproachDemandConfig {
    /// Mean flow in vehicles per hour.
    pub veh_per_h: f64,
    /// Fraction of vehicles that turn left.
    #[serde(default)]
    pub left: f64,
    /// Fraction of vehicles that go through.
    #[serde(default)]
    pub through: f64,
    /// Fraction of vehicles that turn right.
    #[serde(default)]
    pub right: f64,
}

/// Validated demand: per approach, the flow, the turn ratios and the mean number of
/// arrivals per step.
#[derive(Debug, Clone, PartialEq)]
pub struct DemandPlan {
    flow: [f64; 4],
    ratios: [[f64; 3]; 4],
    mean: [f64; 4],
}

impl DemandPlan {
    /// The plan of a scenario without demand.
    const NONE: Self = Self {
        flow: [0.0; 4],
        ratios: [[0.0; 3]; 4],
        mean: [0.0; 4],
    };

    /// Mean flow of an approach, in vehicles per hour. 0 without an entry.
    #[must_use]
    pub const fn veh_per_h(&self, d: Direction) -> f64 {
        self.flow[d.index()]
    }

    /// Fraction of the approach's vehicles that make `movement`. 0 without an entry.
    #[must_use]
    pub const fn ratio(&self, d: Direction, movement: Movement) -> f64 {
        self.ratios[d.index()][movement.index()]
    }

    /// Mean number of arrivals per step on an approach: `veh_per_h × step_s / 3600`.
    #[must_use]
    pub const fn mean_per_step(&self, d: Direction) -> f64 {
        self.mean[d.index()]
    }
}

/// Validates the demand section, in the order documented on [`ConfigError`].
pub(crate) fn validate_demand(
    demand: &DemandConfig,
    step_s: f64,
    intersection: &Intersection,
) -> Result<DemandPlan, ConfigError> {
    let mut plan = DemandPlan::NONE;
    for d in Direction::ALL {
        let Some(entry) = demand.get(d) else { continue };
        let base = format!("demand.{d}");
        let i = d.index();

        let flow = entry.veh_per_h;
        if !(flow.is_finite() && flow >= 0.0) {
            return Err(ConfigError::InvalidFlow {
                path: format!("{base}.veh_per_h"),
                value: flow,
            });
        }

        let given = [entry.left, entry.through, entry.right];
        for movement in Movement::ALL {
            let value = given[movement.index()];
            let path = format!("{base}.{movement}");
            if !(value.is_finite() && (0.0..=1.0).contains(&value)) {
                return Err(ConfigError::InvalidTurnRatio { path, value });
            }
            let allowed = intersection
                .approach(d)
                .lanes()
                .iter()
                .any(|l| l.movements().contains(&movement));
            if value > 0.0 && !allowed {
                return Err(ConfigError::MovementNotInGeometry { path, movement });
            }
        }

        let sum: f64 = given.iter().sum();
        if (sum - 1.0).abs() > RATIO_SUM_TOLERANCE {
            return Err(ConfigError::TurnRatioSum { path: base, sum });
        }

        let mean = flow * step_s / 3600.0;
        if mean > MAX_MEAN_ARRIVALS_PER_STEP {
            return Err(ConfigError::FlowTooHigh {
                path: format!("{base}.veh_per_h"),
                veh_per_h: flow,
                step_s,
                mean_per_step: mean,
                max: MAX_MEAN_ARRIVALS_PER_STEP,
            });
        }

        plan.flow[i] = flow;
        plan.ratios[i] = given;
        plan.mean[i] = mean;
    }
    Ok(plan)
}

/// A uniform draw in `[0, 1)`, exact: the top 53 bits of the next `u64`.
fn uniform(rng: &mut ChaCha8Rng) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    // INVARIANT: the shifted value is below 2^53, so it converts to `f64` exactly.
    let bits = (rng.next_u64() >> 11) as f64;
    bits * (1.0 / 9_007_199_254_740_992.0)
}

/// A Poisson(`lambda`) draw by inversion: one uniform, then walk the probabilities
/// `p_0 = e^-λ`, `p_k = p_(k-1) × λ / k` until their running sum exceeds it.
///
/// Returns 0 without touching the stream when `lambda` is 0. The walk stops at
/// [`MAX_POISSON_WALK`].
fn poisson(rng: &mut ChaCha8Rng, lambda: f64) -> u32 {
    if lambda <= 0.0 {
        return 0;
    }
    let u = uniform(rng);
    let mut p = (-lambda).exp();
    let mut cumulative = p;
    let mut k = 0u32;
    while cumulative <= u && k < MAX_POISSON_WALK {
        k += 1;
        p *= lambda / f64::from(k);
        cumulative += p;
    }
    k
}

/// What `arrivals` needs to know about one approach.
#[derive(Debug, Clone)]
struct ApproachSampler {
    direction: Direction,
    mean: f64,
    /// Running sums of the ratios over [`Movement::ALL`].
    cumulative: [f64; 3],
    /// The last movement with a ratio above 0, for when rounding leaves no match.
    fallback: Movement,
    rng: ChaCha8Rng,
}

impl ApproachSampler {
    fn pick_movement(&mut self) -> Movement {
        let u = uniform(&mut self.rng);
        Movement::ALL
            .into_iter()
            .find(|m| self.cumulative[m.index()] > u)
            .unwrap_or(self.fallback)
    }
}

/// A seeded generator of arrivals.
///
/// It holds one `ChaCha8Rng` per approach, seeded with the run seed and set to the
/// stream number `Direction::index()`, so changing the demand of one approach never
/// changes the arrivals of another. An approach with mean 0 never draws.
///
/// For every approach in [`Direction::ALL`] order, [`arrivals`](Self::arrivals) draws
/// the count (one uniform), then one uniform per arrival for its movement, all from
/// that approach's stream. The turn draw is made even when only one movement has a
/// ratio above 0. This order is part of the contract, and the snapshot test pins it.
///
/// The generator is separate from [`Simulation`](crate::Simulation), which stays free
/// of randomness. A clone continues with exactly the same arrivals.
#[derive(Debug, Clone)]
pub struct Demand {
    seed: u64,
    samplers: [ApproachSampler; 4],
}

impl Demand {
    /// Builds a generator from the demand of `scenario` and a seed.
    #[must_use]
    pub fn new(scenario: &Scenario, seed: u64) -> Self {
        let plan = scenario.demand();
        let samplers = Direction::ALL.map(|d| {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            rng.set_stream(d.index() as u64);
            let mut running = 0.0;
            let cumulative = Movement::ALL.map(|m| {
                running += plan.ratio(d, m);
                running
            });
            let fallback = Movement::ALL
                .into_iter()
                .rev()
                .find(|&m| plan.ratio(d, m) > 0.0)
                .unwrap_or(Movement::Through);
            ApproachSampler {
                direction: d,
                mean: plan.mean_per_step(d),
                cumulative,
                fallback,
                rng,
            }
        });
        Self { seed, samplers }
    }

    /// The seed this generator was built with.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// The arrivals of one step: approaches in [`Direction::ALL`] order, and within
    /// an approach in draw order.
    ///
    /// The loop order is: at step `t`, draw the arrivals, spawn each one (so its
    /// spawn step is `t`), then call [`Simulation::step`](crate::Simulation::step).
    /// A scenario's demand only uses movements its lanes allow, so `spawn` never
    /// fails for a `Simulation` of the same scenario.
    ///
    /// ```
    /// use rl_semaphore_sim::{Command, Demand, Scenario, Simulation};
    ///
    /// let text = include_str!("../../../configs/single-intersection.toml");
    /// let scenario = Scenario::from_toml_str(text).unwrap();
    /// let mut demand = Demand::new(&scenario, 42);
    /// let mut sim = Simulation::new(scenario);
    /// for _ in 0..100 {
    ///     for m in demand.arrivals() {
    ///         sim.spawn(m.approach, m.movement).unwrap();
    ///     }
    ///     sim.step(Command::Hold);
    /// }
    /// assert!(sim.spawned_count() > 0);
    /// ```
    pub fn arrivals(&mut self) -> Vec<MovementId> {
        let mut out = Vec::new();
        for s in &mut self.samplers {
            let count = poisson(&mut s.rng, s.mean);
            for _ in 0..count {
                let movement = s.pick_movement();
                out.push(MovementId::new(s.direction, movement));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: u32 = 200_000;

    fn rng() -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(7)
    }

    #[test]
    fn uniforms_are_in_the_unit_interval() {
        let mut r = rng();
        for _ in 0..N {
            let u = uniform(&mut r);
            assert!((0.0..1.0).contains(&u));
        }
    }

    #[test]
    fn zero_mean_gives_zero_and_leaves_the_stream_alone() {
        let mut r = rng();
        let mut untouched = r.clone();
        for _ in 0..1000 {
            assert_eq!(poisson(&mut r, 0.0), 0);
        }
        assert_eq!(r.next_u64(), untouched.next_u64());
    }

    #[test]
    fn sample_mean_and_variance_match_lambda() {
        for lambda in [0.05, 0.5, 2.0, 10.0] {
            let mut r = rng();
            let (mut sum, mut sum_sq) = (0.0, 0.0);
            for _ in 0..N {
                let x = f64::from(poisson(&mut r, lambda));
                sum += x;
                sum_sq += x * x;
            }
            let n = f64::from(N);
            let mean = sum / n;
            let var = (sum_sq - n * mean * mean) / (n - 1.0);
            let se_mean = (lambda / n).sqrt();
            let se_var = ((lambda + 2.0 * lambda * lambda) / n).sqrt();
            assert!(
                (mean - lambda).abs() < 5.0 * se_mean,
                "lambda {lambda}: mean {mean}"
            );
            assert!(
                (var - lambda).abs() < 5.0 * se_var,
                "lambda {lambda}: variance {var}"
            );
        }
    }

    #[test]
    fn small_counts_match_the_poisson_probabilities() {
        let lambda: f64 = 0.5;
        let mut r = rng();
        let mut freq = [0u32; 3];
        for _ in 0..N {
            if let Some(f) = freq.get_mut(poisson(&mut r, lambda) as usize) {
                *f += 1;
            }
        }
        let n = f64::from(N);
        let mut p = (-lambda).exp();
        for (k, &f) in (1u32..).zip(&freq) {
            let observed = f64::from(f) / n;
            let se = (p * (1.0 - p) / n).sqrt();
            assert!(
                (observed - p).abs() < 5.0 * se,
                "k={}: {observed} vs {p}",
                k - 1
            );
            p *= lambda / f64::from(k);
        }
    }
}
