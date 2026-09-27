//! Episode metrics: wait, throughput and queue statistics.
//!
//! See ADR-0006 for the definitions.

use crate::road::{Direction, LaneId};
use crate::simulation::{Simulation, StepReport};

/// Why [`EpisodeMetrics::observe`] refused an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MetricsError {
    /// `observe` was not called once after every step: the simulation is at step
    /// `found`, but the next observation should be for step `expected`.
    #[error("observed step {found}, expected step {expected}")]
    StepMismatch {
        /// The step count the next observation must have.
        expected: u64,
        /// The step count of the simulation that was passed in.
        found: u64,
    },
}

/// Statistics of a set of waits. All values are in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct WaitStats {
    /// Number of vehicles in the population.
    pub count: u64,
    /// Mean wait, in seconds.
    pub mean_s: f64,
    /// Median wait (nearest rank), in seconds.
    pub p50_s: f64,
    /// 95th percentile of the wait (nearest rank), in seconds.
    pub p95_s: f64,
    /// 99th percentile of the wait (nearest rank), in seconds.
    pub p99_s: f64,
    /// Longest wait, in seconds.
    pub max_s: f64,
}

/// Queue length statistics: the number of stopped vehicles, sampled after every
/// observed step.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueStats {
    /// Time-averaged queue, in vehicles (0.0 when no step was observed).
    pub mean: f64,
    /// Largest queue seen after any observed step, in vehicles.
    pub max: u64,
}

/// The result of an episode.
#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeSummary {
    /// Observed steps.
    pub steps: u64,
    /// Observed time, in seconds (`steps × step_s`).
    pub duration_s: f64,
    /// Vehicles that departed during observed steps.
    pub departed: u64,
    /// Vehicles in the model (in a backlog or on a lane) when the summary was made.
    pub in_system: u64,
    /// Departures per hour (`departed × 3600 / duration_s`, 0.0 for 0 steps).
    pub throughput_veh_per_h: f64,
    /// Wait of every vehicle that departed plus every vehicle still in the model.
    /// `None` if there are none.
    pub wait: Option<WaitStats>,
    /// The same, per approach, indexed by [`Direction::index`].
    pub wait_by_approach: [Option<WaitStats>; 4],
    /// Queue per lane, in lane order (approaches in [`Direction::ALL`] order, then
    /// lane index).
    pub queue_by_lane: Vec<(LaneId, QueueStats)>,
    /// Queue per approach (the sum of its lanes at each step), indexed by
    /// [`Direction::index`].
    pub queue_by_approach: [QueueStats; 4],
    /// Queue of the whole intersection (the sum of all lanes at each step).
    pub queue_total: QueueStats,
}

/// Running sum and maximum of a sampled queue.
#[derive(Debug, Clone, Copy, Default)]
struct QueueAcc {
    sum: u64,
    max: u64,
}

impl QueueAcc {
    fn add(&mut self, sample: u64) {
        self.sum = self.sum.saturating_add(sample);
        self.max = self.max.max(sample);
    }

    fn stats(self, steps: u64) -> QueueStats {
        QueueStats {
            mean: if steps == 0 {
                0.0
            } else {
                // INVARIANT: precision only degrades above 2^53, far beyond any episode.
                #[allow(clippy::cast_precision_loss)]
                let mean = self.sum as f64 / steps as f64;
                mean
            },
            max: self.max,
        }
    }
}

/// Collects the statistics of one episode by observing a [`Simulation`].
///
/// The loop, at step `t`: draw and spawn the arrivals, call
/// [`step`](Simulation::step), then [`observe`](Self::observe) with the report
/// the step returned. `observe` must be called after every step.
///
/// ```
/// use rl_semaphore_sim::{Command, Demand, EpisodeMetrics, Scenario, Simulation};
///
/// let text = include_str!("../../../configs/single-intersection.toml");
/// let scenario = Scenario::from_toml_str(text).unwrap();
/// let mut demand = Demand::new(&scenario, 42);
/// let mut sim = Simulation::new(scenario);
/// let mut metrics = EpisodeMetrics::new(&sim);
/// for _ in 0..100 {
///     for m in demand.arrivals() {
///         sim.spawn(m.approach, m.movement).unwrap();
///     }
///     let report = sim.step(Command::Hold);
///     metrics.observe(&sim, &report).unwrap();
/// }
/// let summary = metrics.summary(&sim);
/// assert_eq!(summary.steps, 100);
/// ```
#[derive(Debug, Clone)]
pub struct EpisodeMetrics {
    t0: u64,
    last: u64,
    /// Lane ids in lane order.
    lanes: Vec<LaneId>,
    /// Index into `lanes` of lane 0 of each approach, by `Direction::index`.
    lane_offsets: [usize; 4],
    /// Waits, in steps, of the vehicles that departed, by approach.
    departed: [Vec<u64>; 4],
    lane_queue: Vec<QueueAcc>,
    approach_queue: [QueueAcc; 4],
    total_queue: QueueAcc,
    /// Per-lane queue of the step being observed, reused between calls.
    scratch: Vec<u64>,
}

impl EpisodeMetrics {
    /// Starts an episode at the current step of `sim`, with empty statistics.
    ///
    /// The simulation may already be running. Vehicles in the model now count in
    /// the wait population while they are still there, with their wait since
    /// spawn, but departures that happened before this call are not seen.
    #[must_use]
    pub fn new(sim: &Simulation) -> Self {
        let mut lanes = Vec::new();
        let mut lane_offsets = [0; 4];
        for approach in sim.scenario().intersection().approaches() {
            lane_offsets[approach.direction().index()] = lanes.len();
            lanes.extend(approach.lanes().iter().map(crate::road::Lane::id));
        }
        let count = lanes.len();
        Self {
            t0: sim.step_count(),
            last: sim.step_count(),
            lanes,
            lane_offsets,
            departed: Default::default(),
            lane_queue: vec![QueueAcc::default(); count],
            approach_queue: [QueueAcc::default(); 4],
            total_queue: QueueAcc::default(),
            scratch: vec![0; count],
        }
    }

    /// Records one step: the waits of `report`'s departures, and the queue of
    /// every lane after the step.
    ///
    /// # Errors
    ///
    /// [`MetricsError::StepMismatch`] if `sim` is not exactly one step after the
    /// previous observation (or after the start of the episode). The collector is
    /// unchanged.
    pub fn observe(&mut self, sim: &Simulation, report: &StepReport) -> Result<(), MetricsError> {
        let expected = self.last.saturating_add(1);
        if sim.step_count() != expected {
            return Err(MetricsError::StepMismatch {
                expected,
                found: sim.step_count(),
            });
        }

        for d in &report.departures {
            self.departed[d.lane.approach().index()].push(d.wait_steps);
        }

        self.scratch.iter_mut().for_each(|q| *q = 0);
        for v in sim.vehicles().filter(|v| v.is_stopped()) {
            let lane = v.lane();
            let slot = self.lane_offsets[lane.approach().index()] + lane.index();
            if let Some(q) = self.scratch.get_mut(slot) {
                *q += 1;
            }
        }

        let mut approach = [0u64; 4];
        let mut total = 0u64;
        for (slot, (&q, id)) in self.scratch.iter().zip(&self.lanes).enumerate() {
            self.lane_queue[slot].add(q);
            approach[id.approach().index()] += q;
            total += q;
        }
        for (acc, q) in self.approach_queue.iter_mut().zip(approach) {
            acc.add(q);
        }
        self.total_queue.add(total);

        self.last = expected;
        Ok(())
    }

    /// The statistics so far. It can be called at any time, and again later.
    ///
    /// `sim` supplies the vehicles still in the model, which count in the wait
    /// statistics with their wait so far, and `step_s`.
    #[must_use]
    pub fn summary(&self, sim: &Simulation) -> EpisodeSummary {
        let step_s = sim.scenario().step_s();
        let observed = self.last - self.t0;
        let mut waits = self.departed.clone();
        let mut in_system = 0u64;
        for v in sim.vehicles() {
            waits[v.lane().approach().index()].push(v.wait_steps());
            in_system += 1;
        }
        let departed: u64 = self.departed.iter().map(|w| w.len() as u64).sum();

        let mut all = Vec::new();
        for w in &mut waits {
            w.sort_unstable();
            all.extend_from_slice(w);
        }
        all.sort_unstable();

        // INVARIANT: precision only degrades above 2^53 steps.
        #[allow(clippy::cast_precision_loss)]
        let duration_s = observed as f64 * step_s;
        #[allow(clippy::cast_precision_loss)]
        let throughput_veh_per_h = if observed == 0 || duration_s <= 0.0 {
            0.0
        } else {
            departed as f64 * 3600.0 / duration_s
        };

        EpisodeSummary {
            steps: observed,
            duration_s,
            departed,
            in_system,
            throughput_veh_per_h,
            wait: wait_stats(&all, step_s),
            wait_by_approach: Direction::ALL.map(|d| wait_stats(&waits[d.index()], step_s)),
            queue_by_lane: self
                .lanes
                .iter()
                .zip(&self.lane_queue)
                .map(|(&id, acc)| (id, acc.stats(observed)))
                .collect(),
            queue_by_approach: self.approach_queue.map(|acc| acc.stats(observed)),
            queue_total: self.total_queue.stats(observed),
        }
    }
}

/// The value at 1-based rank `⌈p × n / 100⌉` of `sorted` (ascending). Returns 0
/// for an empty slice.
fn nearest_rank(sorted: &[u64], p: u64) -> u64 {
    let n = sorted.len() as u64;
    let rank = p.saturating_mul(n).div_ceil(100).max(1);
    // INVARIANT: `rank <= n` for `p <= 100`, so the index is in range; `min` guards `p > 100`.
    #[allow(clippy::cast_possible_truncation)]
    let index = (rank.min(n).max(1) - 1) as usize;
    sorted.get(index).copied().unwrap_or(0)
}

/// Statistics of `sorted` waits in steps, in seconds. `None` if it is empty.
fn wait_stats(sorted: &[u64], step_s: f64) -> Option<WaitStats> {
    let n = sorted.len() as u64;
    if n == 0 {
        return None;
    }
    let sum = sorted.iter().fold(0u64, |a, &w| a.saturating_add(w));
    // INVARIANT: precision only degrades above 2^53, far beyond any episode.
    #[allow(clippy::cast_precision_loss)]
    let secs = |steps: u64| steps as f64 * step_s;
    #[allow(clippy::cast_precision_loss)]
    let mean_s = sum as f64 / n as f64 * step_s;
    Some(WaitStats {
        count: n,
        mean_s,
        p50_s: secs(nearest_rank(sorted, 50)),
        p95_s: secs(nearest_rank(sorted, 95)),
        p99_s: secs(nearest_rank(sorted, 99)),
        max_s: secs(nearest_rank(sorted, 100)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_value() {
        for p in [50, 95, 99, 100] {
            assert_eq!(nearest_rank(&[7], p), 7);
        }
    }

    #[test]
    fn one_to_hundred() {
        let v: Vec<u64> = (1..=100).collect();
        assert_eq!(nearest_rank(&v, 50), 50);
        assert_eq!(nearest_rank(&v, 95), 95);
        assert_eq!(nearest_rank(&v, 99), 99);
        assert_eq!(nearest_rank(&v, 100), 100);
    }

    #[test]
    fn rank_rounds_up() {
        // n = 3: p50 -> ceil(1.5) = 2, p95 -> ceil(2.85) = 3, p99 -> 3.
        let v = [10, 20, 30];
        assert_eq!(nearest_rank(&v, 50), 20);
        assert_eq!(nearest_rank(&v, 95), 30);
        assert_eq!(nearest_rank(&v, 99), 30);
    }

    #[test]
    fn all_equal() {
        let v = [4; 9];
        for p in [50, 95, 99, 100] {
            assert_eq!(nearest_rank(&v, p), 4);
        }
    }

    #[test]
    fn empty_is_none_and_zero() {
        assert_eq!(nearest_rank(&[], 50), 0);
        assert_eq!(wait_stats(&[], 1.0), None);
    }

    #[test]
    fn stats_scale_by_step_length() {
        let s = wait_stats(&[0, 2, 4], 0.5).unwrap();
        assert_eq!(s.count, 3);
        assert!((s.mean_s - 1.0).abs() < 1e-12);
        assert!((s.p50_s - 1.0).abs() < 1e-12);
        assert!((s.max_s - 2.0).abs() < 1e-12);
    }
}
