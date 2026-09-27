//! Traffic signal: a validated [`SignalPlan`] and the [`Signal`] state machine that runs it.
//!
//! The signal is safe by construction. It shows only conflict-free sets of
//! movements, inserts yellow and all-red clearance, refuses early switches, and
//! forces a switch when a phase would otherwise wait longer than max-red (see
//! ADR-0003).
//!
//! # Time and step conventions
//!
//! Time is a sequence of *entries*: entry 0 is the initial state, and every call to
//! [`Signal::step`] produces the next entry. [`Signal::lights`] shows the current
//! entry. All guarantees are stated on the sequence of lights across entries.
//!
//! `elapsed` counts the entries spent in a state, **including the current one**, so
//! the initial state is `Green { phase: 0, elapsed: 1 }`. Consequently:
//!
//! - A `SwitchTo(k)` received in green is accepted only if `elapsed >= min_green`. The
//!   step that accepts it already shows yellow (`Yellow { elapsed: 1 }`), so a phase is
//!   green for at least `min_green` entries in a row.
//! - A yellow lasts exactly `yellow` entries, then an all-red lasts exactly `all_red`
//!   entries (none if 0), and then the next entry is green on the target phase.
//! - A phase's red age is the number of consecutive entries, up to and including the
//!   current one, in which it is not the green phase. It is 0 while the phase is
//!   green. A phase that has never been green starts at 1 (it is not green at entry
//!   0). It grows by 1 per step, and a movement is never red for more consecutive
//!   entries than the red age of any phase that grants it.
//!
//! # Max-red scheduling
//!
//! With `G` = min-green, `Y` = yellow, `A` = all-red and `M` = max-red, a switch from
//! `p` to `k` at entry `now` makes `k` green at `now + 1 + (Y + A)`. Serving the other
//! phases `j1, j2, …` longest-red first, each for `G` entries and each switch costing
//! `Y + A`, makes `j_i` green at `now + 1 + (i + 1)(Y + A) + i·G`. The switch is
//! admitted only if every phase `j` (and `k`) then has a red age of at most `M` at
//! the last entry before it turns green:
//!
//! ```text
//! age(j_i) + (i + 1)(Y + A) + i·G <= M        for i = 0.. (i = 0 is k itself)
//! ```
//!
//! If holding for one more step would make that schedule infeasible for the
//! longest-red phase, the signal starts that switch itself.

use std::cmp::Reverse;
use std::fmt;
use std::sync::Arc;

use crate::road::MovementId;

/// Maximum number of phases in a plan.
pub const MAX_PHASES: usize = 8;

/// Identifies a phase of a [`SignalPlan`] by its position in the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhaseId(usize);

impl PhaseId {
    /// The phase at `index` in the plan. It may be out of range for a given
    /// plan, in which case a signal ignores it as an unknown phase.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Position in the plan's phase list.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A validated phase: a name and the movements that are green together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phase {
    name: String,
    /// Bit `m.index()` is set when the phase grants movement `m`.
    mask: u16,
}

impl Phase {
    pub(crate) fn new(name: String, mask: u16) -> Self {
        Self { name, mask }
    }

    /// The phase name, unique within the plan.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The movements this phase makes green, in [`MovementId::ALL`] order.
    pub fn movements(&self) -> impl Iterator<Item = MovementId> + '_ {
        MovementId::ALL.into_iter().filter(|m| self.grants(*m))
    }

    /// Whether the phase grants `movement`.
    #[must_use]
    pub const fn grants(&self, movement: MovementId) -> bool {
        self.mask & (1 << movement.index()) != 0
    }

    pub(crate) const fn mask(&self) -> u16 {
        self.mask
    }
}

/// A validated, immutable signal plan: phases plus timings in whole steps.
///
/// The first phase is the initial one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalPlan {
    phases: Vec<Phase>,
    yellow: u32,
    all_red: u32,
    min_green: u32,
    max_red: u32,
}

impl SignalPlan {
    pub(crate) fn new(
        phases: Vec<Phase>,
        yellow: u32,
        all_red: u32,
        min_green: u32,
        max_red: u32,
    ) -> Self {
        Self {
            phases,
            yellow,
            all_red,
            min_green,
            max_red,
        }
    }

    /// The phases in list order.
    #[must_use]
    pub fn phases(&self) -> &[Phase] {
        &self.phases
    }

    /// The phase with the given id, if it exists.
    #[must_use]
    pub fn phase(&self, id: PhaseId) -> Option<&Phase> {
        self.phases.get(id.index())
    }

    /// The ids of all phases, in list order.
    pub fn phase_ids(&self) -> impl Iterator<Item = PhaseId> + use<> {
        (0..self.phases.len()).map(PhaseId::new)
    }

    /// Yellow duration in steps (at least 1).
    #[must_use]
    pub const fn yellow_steps(&self) -> u32 {
        self.yellow
    }

    /// All-red clearance duration in steps (may be 0).
    #[must_use]
    pub const fn all_red_steps(&self) -> u32 {
        self.all_red
    }

    /// Minimum green duration in steps (at least 1).
    #[must_use]
    pub const fn min_green_steps(&self) -> u32 {
        self.min_green
    }

    /// Maximum red duration in steps (at least 1).
    #[must_use]
    pub const fn max_red_steps(&self) -> u32 {
        self.max_red
    }
}

/// What a signal head shows for one movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Light {
    /// Stop.
    Red,
    /// Clearing: the movement is about to turn red.
    Yellow,
    /// Go.
    Green,
}

/// What the controller asks for in one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Keep the current phase.
    Hold,
    /// Switch to another phase.
    SwitchTo(PhaseId),
}

/// The state machine's state. `elapsed` counts entries in the state, including the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalState {
    /// A phase is green.
    Green {
        /// The green phase.
        phase: PhaseId,
        /// Entries spent green so far.
        elapsed: u32,
    },
    /// Movements that lose green show yellow.
    Yellow {
        /// The phase being left.
        from: PhaseId,
        /// The phase that will be green next.
        to: PhaseId,
        /// Entries spent yellow so far.
        elapsed: u32,
    },
    /// Everything that is not green in both phases is red.
    AllRed {
        /// The phase being left.
        from: PhaseId,
        /// The phase that will be green next.
        to: PhaseId,
        /// Entries spent all-red so far.
        elapsed: u32,
    },
}

/// Why a switch command was not carried out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoredReason {
    /// The current phase has not been green for `min_green` entries.
    MinGreenNotReached,
    /// A yellow or all-red interval is running.
    TransitionInProgress,
    /// The phase id is out of range.
    UnknownPhase,
    /// The requested phase is already green.
    AlreadyOnPhase,
    /// Switching would make some phase miss its max-red deadline.
    MaxRedDeadline,
}

/// What a call to [`Signal::step`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// No switch happened and nothing was refused.
    Held,
    /// The command was accepted and a transition to the phase started.
    SwitchStarted(PhaseId),
    /// The signal started a transition to `to` because a max-red deadline was
    /// about to be missed, whatever `overrode` asked for.
    SwitchForced {
        /// The phase the signal switched to.
        to: PhaseId,
        /// The command that was overridden.
        overrode: Command,
    },
    /// A switch command was refused, for the given reason.
    Ignored(IgnoredReason),
}

/// The decision of one step, before it is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    Hold,
    Ignore(IgnoredReason),
    Start { to: PhaseId, forced: bool },
}

/// A running signal. Cloning it gives an independent copy of the state, which
/// is how a caller can probe what a command would do.
#[derive(Debug, Clone)]
pub struct Signal {
    plan: Arc<SignalPlan>,
    state: SignalState,
    lights: [Light; MovementId::COUNT],
    phase_red_age: Vec<u32>,
    movement_red_age: [u32; MovementId::COUNT],
}

impl Signal {
    /// Starts green on the plan's first phase at entry 0.
    #[must_use]
    pub fn new(plan: &SignalPlan) -> Self {
        let state = SignalState::Green {
            phase: PhaseId::new(0),
            elapsed: 1,
        };
        let mut signal = Self {
            plan: Arc::new(plan.clone()),
            state,
            lights: [Light::Red; MovementId::COUNT],
            phase_red_age: vec![0; plan.phases.len()],
            movement_red_age: [0; MovementId::COUNT],
        };
        signal.refresh();
        signal
    }

    /// The plan this signal runs.
    #[must_use]
    pub fn plan(&self) -> &SignalPlan {
        &self.plan
    }

    /// The current state.
    #[must_use]
    pub const fn state(&self) -> SignalState {
        self.state
    }

    /// The light shown to `movement`.
    #[must_use]
    pub const fn light(&self, movement: MovementId) -> Light {
        self.lights[movement.index()]
    }

    /// All lights, indexed by [`MovementId::index`].
    #[must_use]
    pub const fn lights(&self) -> [Light; MovementId::COUNT] {
        self.lights
    }

    /// The red age of a phase (see the module docs), or `None` for an unknown phase.
    #[must_use]
    pub fn phase_red_age(&self, phase: PhaseId) -> Option<u32> {
        self.phase_red_age.get(phase.index()).copied()
    }

    /// Consecutive entries, up to and including the current one, in which
    /// `movement` is red.
    #[must_use]
    pub const fn movement_red_age(&self, movement: MovementId) -> u32 {
        self.movement_red_age[movement.index()]
    }

    /// Whether `step(Command::SwitchTo(phase))` would start a transition to `phase` right now.
    #[must_use]
    pub fn can_switch_to(&self, phase: PhaseId) -> bool {
        matches!(
            self.decide(Command::SwitchTo(phase)),
            Decision::Start { to, .. } if to == phase
        )
    }

    /// Advances one entry, applying `command` as the module docs describe.
    pub fn step(&mut self, command: Command) -> StepOutcome {
        let decision = self.decide(command);
        let outcome = match decision {
            Decision::Hold => StepOutcome::Held,
            Decision::Ignore(reason) => StepOutcome::Ignored(reason),
            Decision::Start { to, forced: false } => StepOutcome::SwitchStarted(to),
            Decision::Start { to, forced: true } => StepOutcome::SwitchForced {
                to,
                overrode: command,
            },
        };
        self.state = match (decision, self.state) {
            (Decision::Start { to, .. }, SignalState::Green { phase, .. }) => SignalState::Yellow {
                from: phase,
                to,
                elapsed: 1,
            },
            (_, state) => self.advance(state),
        };
        self.refresh();
        outcome
    }

    /// The state that follows `state` when no switch starts.
    fn advance(&self, state: SignalState) -> SignalState {
        let (yellow, all_red) = (self.plan.yellow, self.plan.all_red);
        match state {
            SignalState::Green { phase, elapsed } => SignalState::Green {
                phase,
                elapsed: elapsed.saturating_add(1),
            },
            SignalState::Yellow { from, to, elapsed } if elapsed < yellow => SignalState::Yellow {
                from,
                to,
                elapsed: elapsed + 1,
            },
            SignalState::Yellow { from, to, .. } if all_red > 0 => SignalState::AllRed {
                from,
                to,
                elapsed: 1,
            },
            SignalState::AllRed { from, to, elapsed } if elapsed < all_red => SignalState::AllRed {
                from,
                to,
                elapsed: elapsed + 1,
            },
            SignalState::Yellow { to, .. } | SignalState::AllRed { to, .. } => SignalState::Green {
                phase: to,
                elapsed: 1,
            },
        }
    }

    /// Recomputes lights and red ages for the current state.
    fn refresh(&mut self) {
        self.lights = self.compute_lights();
        let green = match self.state {
            SignalState::Green { phase, .. } => Some(phase.index()),
            _ => None,
        };
        for (i, age) in self.phase_red_age.iter_mut().enumerate() {
            *age = if green == Some(i) {
                0
            } else {
                age.saturating_add(1)
            };
        }
        for (age, light) in self.movement_red_age.iter_mut().zip(self.lights) {
            *age = if light == Light::Red {
                age.saturating_add(1)
            } else {
                0
            };
        }
    }

    fn mask(&self, phase: PhaseId) -> u16 {
        // INVARIANT: a state only holds phase ids that exist in the plan.
        self.plan.phases.get(phase.index()).map_or(0, Phase::mask)
    }

    fn compute_lights(&self) -> [Light; MovementId::COUNT] {
        let (green, yellow) = match self.state {
            SignalState::Green { phase, .. } => (self.mask(phase), 0),
            SignalState::Yellow { from, to, .. } => {
                let (p, k) = (self.mask(from), self.mask(to));
                (p & k, p & !k)
            }
            SignalState::AllRed { from, to, .. } => (self.mask(from) & self.mask(to), 0),
        };
        let mut lights = [Light::Red; MovementId::COUNT];
        for (i, light) in lights.iter_mut().enumerate() {
            let bit = 1u16 << i;
            if green & bit != 0 {
                *light = Light::Green;
            } else if yellow & bit != 0 {
                *light = Light::Yellow;
            }
        }
        lights
    }

    /// Decides what the next step does, without changing anything. `step` and
    /// `can_switch_to` both go through here, so they cannot disagree.
    fn decide(&self, command: Command) -> Decision {
        let SignalState::Green { phase, elapsed } = self.state else {
            return match command {
                Command::Hold => Decision::Hold,
                Command::SwitchTo(_) => Decision::Ignore(IgnoredReason::TransitionInProgress),
            };
        };

        let refusal = match command {
            Command::Hold => None,
            Command::SwitchTo(to) => {
                if to.index() >= self.plan.phases.len() {
                    Some(IgnoredReason::UnknownPhase)
                } else if to == phase {
                    Some(IgnoredReason::AlreadyOnPhase)
                } else if elapsed < self.plan.min_green {
                    Some(IgnoredReason::MinGreenNotReached)
                } else if !self.schedule_is_feasible(phase, to, 0) {
                    Some(IgnoredReason::MaxRedDeadline)
                } else {
                    return Decision::Start { to, forced: false };
                }
            }
        };

        // Not switching by request: force a switch if holding one more step would
        // leave the longest-red phase without a feasible schedule.
        if elapsed >= self.plan.min_green
            && let Some(first) = self.longest_red(phase)
            && !self.schedule_is_feasible(phase, first, 1)
        {
            return Decision::Start {
                to: first,
                forced: true,
            };
        }
        refusal.map_or(Decision::Hold, Decision::Ignore)
    }

    /// The phase other than `current` with the greatest red age (ties: lowest id).
    fn longest_red(&self, current: PhaseId) -> Option<PhaseId> {
        let mut best: Option<(u32, usize)> = None;
        for (i, &age) in self.phase_red_age.iter().enumerate() {
            if i != current.index() && best.is_none_or(|(a, _)| age > a) {
                best = Some((age, i));
            }
        }
        best.map(|(_, i)| PhaseId::new(i))
    }

    /// Whether, starting a switch from `current` to `first` after `wait` more holds,
    /// serving `first` and then every other phase longest-red first keeps every
    /// phase within max-red (see the module docs).
    fn schedule_is_feasible(&self, current: PhaseId, first: PhaseId, wait: u64) -> bool {
        let p = &*self.plan;
        let (y_a, g, m) = (
            u64::from(p.yellow) + u64::from(p.all_red),
            u64::from(p.min_green),
            u64::from(p.max_red),
        );
        // Red age of each phase when the switch starts; the current one is green.
        let age = |i: usize| {
            if i == current.index() {
                0
            } else {
                u64::from(self.phase_red_age[i]) + wait
            }
        };
        if age(first.index()) + y_a > m {
            return false;
        }
        let mut rest = [(0u64, 0usize); MAX_PHASES];
        let mut count = 0;
        for i in (0..p.phases.len()).filter(|&i| i != first.index()) {
            rest[count] = (age(i), i);
            count += 1;
        }
        let rest = &mut rest[..count];
        rest.sort_unstable_by_key(|&(a, i)| (Reverse(a), i));
        rest.iter()
            .zip(1u64..)
            .all(|(&(a, _), i)| a + (i + 1) * y_a + i * g <= m)
    }
}

impl fmt::Display for Light {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Light::Red => "red",
            Light::Yellow => "yellow",
            Light::Green => "green",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_phase_stays_green_and_ignores_switches() {
        let mask = 0b111;
        let plan = SignalPlan::new(vec![Phase::new("only".into(), mask)], 3, 2, 5, 10);
        let mut signal = Signal::new(&plan);
        for _ in 0..100 {
            assert_eq!(signal.step(Command::Hold), StepOutcome::Held);
            assert_eq!(
                signal.step(Command::SwitchTo(PhaseId::new(0))),
                StepOutcome::Ignored(IgnoredReason::AlreadyOnPhase)
            );
            assert_eq!(
                signal.step(Command::SwitchTo(PhaseId::new(1))),
                StepOutcome::Ignored(IgnoredReason::UnknownPhase)
            );
            assert!(!signal.can_switch_to(PhaseId::new(0)));
            assert_eq!(signal.phase_red_age(PhaseId::new(0)), Some(0));
        }
    }
}
