//! The simulation: a scenario, its signal and the vehicles on every lane.
//!
//! See ADR-0004 for the update rule.

use std::collections::{BTreeMap, VecDeque};

use crate::road::{Direction, LaneId, Movement, MovementId};
use crate::scenario::Scenario;
use crate::signal::{Command, Light, Signal, StepOutcome};
use crate::vehicle::{Departure, Position, SpawnError, Vehicle, VehicleId};

/// What one call to [`Simulation::step`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepReport {
    /// What the signal did with the command.
    pub signal: StepOutcome,
    /// Vehicles that crossed the stop line, ordered by lane (approaches in
    /// [`Direction::ALL`] order, then lane index).
    pub departures: Vec<Departure>,
}

/// Mutable state of one lane.
#[derive(Debug, Clone)]
struct LaneState {
    /// Occupancy by cell, cell 0 being the upstream entry.
    cells: Vec<Option<VehicleId>>,
    /// Spawned vehicles waiting to enter cell 0, front first.
    backlog: VecDeque<VehicleId>,
    /// Number of `Some` cells, so lane choice does not scan them.
    occupied: usize,
}

impl LaneState {
    fn load(&self) -> usize {
        self.occupied + self.backlog.len()
    }
}

/// Updates a vehicle's wait for one step: a vehicle that did not move waits one
/// more step and is marked stopped, one that moved is cleared (R2.3).
fn record_move(vehicles: &mut BTreeMap<VehicleId, Vehicle>, id: VehicleId, moved: bool) {
    if let Some(v) = vehicles.get_mut(&id) {
        v.stopped = !moved;
        if !moved {
            v.wait_steps = v.wait_steps.saturating_add(1);
        }
    }
}

/// A running simulation of one intersection.
#[derive(Debug, Clone)]
pub struct Simulation {
    scenario: Scenario,
    signal: Signal,
    t: u64,
    next_id: u64,
    vehicles: BTreeMap<VehicleId, Vehicle>,
    /// Per-lane state in `Direction::ALL`, then lane-index order.
    lanes: Vec<LaneState>,
    /// Index into `lanes` of lane 0 of each approach, by `Direction::index`.
    lane_offsets: [usize; 4],
    in_backlog: u64,
    on_lanes: u64,
    departed: u64,
}

impl Simulation {
    /// Builds a simulation with empty lanes and backlogs, the signal in its
    /// initial state and the step count at 0.
    #[must_use]
    pub fn new(scenario: Scenario) -> Self {
        let signal = Signal::new(scenario.signal_plan());
        let mut lanes = Vec::new();
        let mut lane_offsets = [0; 4];
        for approach in scenario.intersection().approaches() {
            lane_offsets[approach.direction().index()] = lanes.len();
            for lane in approach.lanes() {
                lanes.push(LaneState {
                    cells: vec![None; lane.len_cells() as usize],
                    backlog: VecDeque::new(),
                    occupied: 0,
                });
            }
        }
        Self {
            scenario,
            signal,
            t: 0,
            next_id: 0,
            vehicles: BTreeMap::new(),
            lanes,
            lane_offsets,
            in_backlog: 0,
            on_lanes: 0,
            departed: 0,
        }
    }

    /// The scenario being simulated.
    #[must_use]
    pub const fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    /// The signal, read-only.
    #[must_use]
    pub const fn signal(&self) -> &Signal {
        &self.signal
    }

    /// The lights shown now, indexed by [`MovementId::index`].
    #[must_use]
    pub const fn lights(&self) -> [Light; MovementId::COUNT] {
        self.signal.lights()
    }

    /// Number of steps taken so far.
    #[must_use]
    pub const fn step_count(&self) -> u64 {
        self.t
    }

    /// The vehicles in the model (in a backlog or on a lane), in id order.
    /// Vehicles that departed are not included.
    pub fn vehicles(&self) -> impl Iterator<Item = &Vehicle> {
        self.vehicles.values()
    }

    /// Looks up a vehicle that has not departed.
    #[must_use]
    pub fn vehicle(&self, id: VehicleId) -> Option<&Vehicle> {
        self.vehicles.get(&id)
    }

    /// The occupancy of a lane, indexed by cell (0 is the upstream entry, the
    /// last cell is the stop-line cell). `None` if the lane is not in this scenario.
    #[must_use]
    pub fn lane_cells(&self, lane: LaneId) -> Option<&[Option<VehicleId>]> {
        self.slot(lane).map(|s| self.lanes[s].cells.as_slice())
    }

    /// The vehicles waiting to enter a lane, in FIFO order (front first).
    /// `None` if the lane is not in this scenario.
    #[must_use]
    pub fn backlog(&self, lane: LaneId) -> Option<impl ExactSizeIterator<Item = VehicleId> + '_> {
        self.slot(lane)
            .map(|s| self.lanes[s].backlog.iter().copied())
    }

    /// Vehicles spawned so far.
    #[must_use]
    pub const fn spawned_count(&self) -> u64 {
        self.next_id
    }

    /// Vehicles currently waiting in a backlog.
    #[must_use]
    pub const fn backlog_count(&self) -> u64 {
        self.in_backlog
    }

    /// Vehicles currently on a lane.
    #[must_use]
    pub const fn on_lane_count(&self) -> u64 {
        self.on_lanes
    }

    /// Vehicles that crossed the stop line so far.
    #[must_use]
    pub const fn departed_count(&self) -> u64 {
        self.departed
    }

    /// Spawns a vehicle for `approach` and `movement` at the current step.
    ///
    /// The vehicle goes to the lane of the approach that allows the movement and
    /// has the fewest vehicles (on the lane plus in its backlog), ties going to
    /// the lowest lane index. It joins the back of that lane's backlog and enters
    /// the lane during [`step`](Self::step). Its lane never changes.
    ///
    /// # Errors
    ///
    /// [`SpawnError::NoLaneForMovement`] if no lane of the approach allows the
    /// movement. The simulation is unchanged.
    pub fn spawn(
        &mut self,
        approach: Direction,
        movement: Movement,
    ) -> Result<VehicleId, SpawnError> {
        let movement_id = MovementId::new(approach, movement);
        let offset = self.lane_offsets[approach.index()];
        let mut best: Option<(usize, LaneId, usize)> = None;
        for lane in self.scenario.intersection().approach(approach).lanes() {
            if !lane.movements().contains(&movement) {
                continue;
            }
            let slot = offset + lane.id().index();
            let load = self.lanes[slot].load();
            if best.is_none_or(|(_, _, b)| load < b) {
                best = Some((slot, lane.id(), load));
            }
        }
        let Some((slot, lane, _)) = best else {
            return Err(SpawnError::NoLaneForMovement {
                movement: movement_id,
            });
        };

        let id = VehicleId::new(self.next_id);
        self.next_id += 1;
        self.vehicles.insert(
            id,
            Vehicle {
                id,
                movement: movement_id,
                lane,
                spawned_at: self.t,
                position: Position::Backlog,
                wait_steps: 0,
                stopped: false,
            },
        );
        self.lanes[slot].backlog.push_back(id);
        self.in_backlog += 1;
        Ok(id)
    }

    /// Advances the simulation from step `t` to `t + 1`.
    ///
    /// In order:
    ///
    /// 1. **Signal.** The signal takes `command` and updates its lights to
    ///    `L(t + 1)`. Everything below sees these lights, so the lights reported
    ///    after the step are the ones under which its crossings happened.
    /// 2. **Crossing.** In each lane, the vehicle in the stop-line cell crosses
    ///    and leaves the model if its movement is `Green`. `Yellow` and `Red` do
    ///    not allow crossing.
    /// 3. **Advance.** Every other vehicle on a lane moves one cell forward if,
    ///    and only if, the next cell was empty at the *start* of the step. This is
    ///    a parallel update: a cell vacated during this step is not filled during
    ///    this step.
    /// 4. **Entry.** In each lane, the front of the backlog enters cell 0 if
    ///    cell 0 was empty at the start of the step. At most one vehicle enters a
    ///    lane per step.
    ///
    /// A vehicle that does not move in the step (it waits at the stop line, is
    /// blocked by the cell ahead, or stays in its backlog) is marked stopped and
    /// its [`wait_steps`](Vehicle::wait_steps) grows by one. One that moves is
    /// cleared.
    ///
    /// Every decision depends only on the state at the start of the step and on
    /// `L(t + 1)`, never on the order lanes or vehicles are processed in.
    ///
    /// Because of the parallel update, a queue standing at a green light
    /// discharges one vehicle every 2 steps (the *headway*), and a lone vehicle
    /// spawned onto an empty lane of `n` cells under constant green enters at
    /// `t + 1`, reaches the stop-line cell at `t + n` and crosses at `t + n + 1`.
    ///
    /// Lanes are strictly first-in first-out: a vehicle waiting at the stop line
    /// blocks the vehicles behind it even if their movements are green.
    pub fn step(&mut self, command: Command) -> StepReport {
        let signal = self.signal.step(command);
        let lights = self.signal.lights();
        let departed_at = self.t + 1;
        let mut departures = Vec::new();

        let mut slot = 0;
        for approach in self.scenario.intersection().approaches() {
            for lane in approach.lanes() {
                let state = &mut self.lanes[slot];
                slot += 1;
                let Some(last) = state.cells.len().checked_sub(1) else {
                    continue;
                };

                // Crossing (R3.2). `was_occupied` describes the cell ahead of
                // the one being processed, as it was at the start of the step.
                let mut was_occupied = state.cells[last].is_some();
                let mut crossed = false;
                if let Some(id) = state.cells[last]
                    && let Some(v) = self.vehicles.get(&id)
                    && lights[v.movement.index()] == Light::Green
                    && let Some(v) = self.vehicles.remove(&id)
                {
                    crossed = true;
                    state.cells[last] = None;
                    state.occupied -= 1;
                    self.on_lanes -= 1;
                    self.departed += 1;
                    departures.push(Departure {
                        vehicle: id,
                        movement: v.movement,
                        lane: lane.id(),
                        spawned_at: v.spawned_at,
                        departed_at,
                        wait_steps: v.wait_steps,
                    });
                }
                if !crossed && let Some(id) = state.cells[last] {
                    record_move(&mut self.vehicles, id, false);
                }

                // Advance (R3.3), front to back.
                for i in (0..last).rev() {
                    let occupied = state.cells[i].is_some();
                    if let Some(id) = state.cells[i] {
                        if was_occupied {
                            record_move(&mut self.vehicles, id, false);
                        } else {
                            state.cells[i] = None;
                            state.cells[i + 1] = Some(id);
                            if let Some(v) = self.vehicles.get_mut(&id) {
                                // INVARIANT: `i + 1 <= last`, and lane lengths fit in `u32`.
                                #[allow(clippy::cast_possible_truncation)]
                                let cell = (i + 1) as u32;
                                v.position = Position::OnLane { cell };
                            }
                            record_move(&mut self.vehicles, id, true);
                        }
                    }
                    was_occupied = occupied;
                }

                // Entry (R3.4): `was_occupied` is now the state of cell 0 at `t`.
                if !was_occupied && let Some(id) = state.backlog.pop_front() {
                    state.cells[0] = Some(id);
                    state.occupied += 1;
                    self.in_backlog -= 1;
                    self.on_lanes += 1;
                    if let Some(v) = self.vehicles.get_mut(&id) {
                        v.position = Position::OnLane { cell: 0 };
                    }
                    record_move(&mut self.vehicles, id, true);
                }
                // Whoever is still in the backlog did not move.
                for &id in &state.backlog {
                    record_move(&mut self.vehicles, id, false);
                }
            }
        }

        self.t = departed_at;
        StepReport { signal, departures }
    }

    /// Index into `lanes` for a lane id, if the lane exists in this scenario.
    fn slot(&self, lane: LaneId) -> Option<usize> {
        let count = self
            .scenario
            .intersection()
            .approach(lane.approach())
            .lanes()
            .len();
        (lane.index() < count).then(|| self.lane_offsets[lane.approach().index()] + lane.index())
    }
}
