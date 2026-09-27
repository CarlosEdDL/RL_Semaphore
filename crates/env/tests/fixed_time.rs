//! The `FixedTime` controller and the episode runner.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{ALL_RED, EXAMPLE, YELLOW, drive, scenario};
use rl_semaphore_env::{Controller, FixedTime, run_episode};
use rl_semaphore_sim::{Command, PhaseId, Scenario, SignalState, Simulation, StepOutcome};

/// The phase that is green at each entry, `None` while a transition runs. Entry 0 is
/// the initial state, then one entry per step.
fn green_entries(
    sim: &mut Simulation,
    controller: &mut FixedTime,
    steps: usize,
) -> Vec<Option<usize>> {
    let green = |sim: &Simulation| match sim.signal().state() {
        SignalState::Green { phase, .. } => Some(phase.index()),
        _ => None,
    };
    let mut entries = vec![green(sim)];
    for _ in 0..steps {
        let command = controller.command(sim);
        sim.step(command);
        entries.push(green(sim));
    }
    entries
}

#[test]
fn command_for_every_state_kind() {
    // Greens 5, 4, 6 (cycle 15 + 9 = 24).
    let scenario = scenario(3, 60.0, &[5.0, 4.0, 6.0]).unwrap();
    let mut controller = FixedTime::new(scenario.fixed_time().unwrap());
    let mut sim = Simulation::new(scenario);

    // Green, fewer entries than planned: hold. The initial state has elapsed 1.
    for elapsed in 1..5 {
        assert_eq!(
            sim.signal().state(),
            SignalState::Green {
                phase: PhaseId::new(0),
                elapsed
            }
        );
        assert_eq!(controller.command(&sim), Command::Hold);
        sim.step(Command::Hold);
    }
    // Green for exactly the planned entries: ask for the next phase.
    assert_eq!(
        sim.signal().state(),
        SignalState::Green {
            phase: PhaseId::new(0),
            elapsed: 5
        }
    );
    let next = Command::SwitchTo(PhaseId::new(1));
    assert_eq!(controller.command(&sim), next);
    assert_eq!(
        sim.step(next).signal,
        StepOutcome::SwitchStarted(PhaseId::new(1))
    );

    // Yellow, then all-red: hold.
    for _ in 0..YELLOW {
        assert!(matches!(sim.signal().state(), SignalState::Yellow { .. }));
        assert_eq!(controller.command(&sim), Command::Hold);
        sim.step(Command::Hold);
    }
    for _ in 0..ALL_RED {
        assert!(matches!(sim.signal().state(), SignalState::AllRed { .. }));
        assert_eq!(controller.command(&sim), Command::Hold);
        sim.step(Command::Hold);
    }
    assert_eq!(
        sim.signal().state(),
        SignalState::Green {
            phase: PhaseId::new(1),
            elapsed: 1
        }
    );

    // The last phase wraps around to phase 0.
    let mut sim = Simulation::new(scenario_of(&[5.0, 4.0, 6.0]));
    let mut seen = Vec::new();
    for _ in 0..24 {
        if let Command::SwitchTo(p) = controller.command(&sim) {
            seen.push(p.index());
        }
        sim.step(controller.command(&sim));
    }
    assert_eq!(seen, [1, 2, 0]);
}

fn scenario_of(greens: &[f64]) -> Scenario {
    scenario(3, 60.0, greens).unwrap()
}

#[test]
fn each_phase_is_green_for_its_planned_entries_over_three_cycles() {
    let greens = [5.0, 4.0, 6.0];
    let s = scenario_of(&greens);
    let plan = s.fixed_time().unwrap().clone();
    let cycle = plan.cycle_steps() as usize;
    assert_eq!(cycle, 15 + 9);
    let mut controller = FixedTime::new(&plan);
    let mut sim = Simulation::new(s);
    let entries = green_entries(&mut sim, &mut controller, 3 * cycle + 5);

    // Runs of the same green phase, in order.
    let mut runs: Vec<(usize, usize, usize)> = Vec::new(); // (phase, start entry, length)
    for (t, e) in entries.iter().enumerate() {
        let Some(p) = *e else { continue };
        match runs.last_mut() {
            Some((q, start, len)) if *q == p && *start + *len == t => *len += 1,
            _ => runs.push((p, t, 1)),
        }
    }
    assert!(runs.len() >= 9);
    for (i, &(phase, start, len)) in runs.iter().enumerate() {
        assert_eq!(phase, i % 3, "phases are served in list order");
        // The last run may be cut by the end of the run.
        if i + 1 < runs.len() {
            assert_eq!(len, plan.green_steps()[phase] as usize, "run {i}");
        }
        // Phase 0 turns green every `cycle` entries, starting at entry 0.
        if phase == 0 {
            assert_eq!(start, i / 3 * cycle);
        }
    }
}

#[test]
fn no_forced_or_ignored_outcomes_at_the_max_red_limit() {
    // Greens 5, 5, 5: cycle 24, every phase is red for 19 steps, which is the limit.
    let s = scenario(3, 19.0, &[5.0, 5.0, 5.0]).unwrap();
    let mut controller = FixedTime::new(s.fixed_time().unwrap());
    let mut sim = Simulation::new(s);
    let outcomes = drive(&mut sim, &mut controller, 24 * 10);
    assert!(
        outcomes
            .iter()
            .all(|o| matches!(o, StepOutcome::Held | StepOutcome::SwitchStarted(_))),
        "{outcomes:?}"
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| matches!(o, StepOutcome::SwitchStarted(_)))
            .count(),
        30
    );
}

#[test]
fn a_single_phase_plan_always_holds() {
    let s = scenario(1, 1.0, &[8.0]).unwrap();
    let mut controller = FixedTime::new(s.fixed_time().unwrap());
    let mut sim = Simulation::new(s);
    for _ in 0..100 {
        assert_eq!(controller.command(&sim), Command::Hold);
        assert_eq!(sim.step(Command::Hold).signal, StepOutcome::Held);
    }
    assert_eq!(controller.name(), "fixed_time");
}

fn example_run(seed: u64, steps: u64) -> rl_semaphore_env::EpisodeReport {
    let s = Scenario::from_toml_str(EXAMPLE).unwrap();
    let mut controller = FixedTime::new(s.fixed_time().unwrap());
    run_episode(&s, &mut controller, seed, steps).unwrap()
}

#[test]
fn example_run_reports_no_forced_switches_and_the_planned_number_of_switches() {
    let r = example_run(7, 3600);
    assert_eq!(r.summary.steps, 3600);
    assert_eq!(r.signal.switches_forced, 0);
    assert_eq!(r.signal.commands_ignored, 0);
    // 45 cycles of 4 switches each.
    assert_eq!(r.signal.switches_started, 180);
}

#[test]
fn switches_started_matches_the_plan_for_partial_cycles() {
    // Cycle of 80 steps. Switches are decided at t = 9, 44, 59, 74 (+80 per cycle):
    // phase 0 is green for 10 entries (t = 0..=9), then 5 steps of transition,
    // phase 1 for 30 entries, and so on.
    let cases = [
        (9, 0),
        (10, 1),
        (44, 1),
        (45, 2),
        (80, 4),
        (89, 4),
        (90, 5),
        (169, 8),
        (170, 9),
    ];
    for (steps, expected) in cases {
        assert_eq!(
            example_run(0, steps).signal.switches_started,
            expected,
            "steps={steps}"
        );
    }
}

#[test]
fn same_seed_gives_the_same_report_and_seeds_differ() {
    assert_eq!(example_run(3, 600), example_run(3, 600));
    assert_ne!(example_run(3, 600), example_run(4, 600));
}

#[test]
fn zero_steps_returns_an_empty_report() {
    let r = example_run(0, 0);
    assert_eq!(r.summary.steps, 0);
    assert_eq!(r.summary.departed, 0);
    assert_eq!(r.summary.in_system, 0);
    assert_eq!(r.signal, rl_semaphore_env::SignalCounts::default());
}
