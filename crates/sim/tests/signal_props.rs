//! Property tests: for any command sequence, the signal never shows conflicting
//! movements, keeps its clearance intervals exact, and never leaves a movement red
//! for longer than max-red.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use proptest::prelude::*;
use rl_semaphore_sim::{
    Command, Light, MovementId, PhaseId, Scenario, Signal, SignalPlan, SignalState, StepOutcome,
};

const EXAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/single-intersection.toml"
));

/// The example plan (4 phases, no shared movements).
const PHASES_EXAMPLE: usize = 4;

/// A 5-phase plan where two phases share movements.
const OVERLAP_PHASES: &str = r#"
[[signal.phases]]
name = "ns"
green = { north = ["through", "right"], south = ["through", "right"] }

[[signal.phases]]
name = "n"
green = { north = ["left", "through", "right"] }

[[signal.phases]]
name = "s-left"
green = { south = ["left"] }

[[signal.phases]]
name = "east"
green = { east = ["left", "through", "right"] }

[[signal.phases]]
name = "west"
green = { west = ["left", "through", "right"] }
"#;

/// Builds a plan from timings given directly in steps (`step_s = 1.0`).
fn plan(overlap: bool, y: u32, a: u32, g: u32, m: u32) -> SignalPlan {
    let signal =
        format!("yellow_s = {y}.0\nall_red_s = {a}.0\nmin_green_s = {g}.0\nmax_red_s = {m}.0\n");
    let phases = if overlap {
        OVERLAP_PHASES.to_owned()
    } else {
        // The example's own `[fixed_time]` plan is dropped: these timings are not its.
        EXAMPLE
            .split("\n# Fixed-time plan")
            .next()
            .unwrap()
            .split("\n[signal]\n")
            .nth(1)
            .unwrap()
            .split_once("\n[[")
            .map_or_else(String::new, |(_, rest)| format!("\n[[{rest}"))
    };
    let head = EXAMPLE.split("\n[signal]\n").next().unwrap();
    let text = format!("{head}\n[signal]\n{signal}{phases}");
    Scenario::from_toml_str(&text)
        .unwrap_or_else(|e| panic!("plan ({y}, {a}, {g}, {m}): {e}"))
        .signal_plan()
        .clone()
}

/// One recorded entry of a run.
struct Entry {
    lights: [Light; MovementId::COUNT],
    state: SignalState,
}

/// Checks the safety guarantees on a recorded trace.
fn check_trace(trace: &[Entry], y: u32, a: u32, g: u32, m: u32) {
    // No two conflicting movements are non-red together.
    for (t, e) in trace.iter().enumerate() {
        for x in MovementId::ALL {
            for z in MovementId::ALL {
                if x.conflicts_with(z) {
                    assert!(
                        e.lights[x.index()] == Light::Red || e.lights[z.index()] == Light::Red,
                        "entry {t}: {x} and {z} are both not red"
                    );
                }
            }
        }
    }

    // Per-movement runs: yellow lasts exactly `y`, red at most `m`.
    for mid in MovementId::ALL {
        let i = mid.index();
        let (mut yellow, mut red) = (0u32, 0u32);
        for (t, e) in trace.iter().enumerate() {
            yellow = if e.lights[i] == Light::Yellow {
                yellow + 1
            } else {
                0
            };
            red = if e.lights[i] == Light::Red {
                red + 1
            } else {
                0
            };
            assert!(yellow <= y, "entry {t}: {mid} yellow for {yellow} > {y}");
            assert!(red <= m, "entry {t}: {mid} red for {red} > {m}");
            if yellow > 0
                && trace
                    .get(t + 1)
                    .is_none_or(|n| n.lights[i] != Light::Yellow)
            {
                assert!(
                    t + 1 == trace.len() || yellow == y,
                    "entry {t}: {mid} yellow ended after {yellow} != {y}"
                );
            }
        }
    }

    // State runs: exact yellow and all-red lengths, and min-green before each transition.
    let mut run = 0u32;
    for (t, e) in trace.iter().enumerate() {
        run += 1;
        let next = trace.get(t + 1).map(|n| n.state);
        let same_kind = |a: SignalState, b: SignalState| {
            std::mem::discriminant(&a) == std::mem::discriminant(&b) && phases_of(a) == phases_of(b)
        };
        if next.is_some_and(|n| same_kind(e.state, n)) {
            continue;
        }
        if next.is_some() {
            match e.state {
                SignalState::Green { .. } => assert!(run >= g, "entry {t}: green for {run} < {g}"),
                SignalState::Yellow { .. } => assert_eq!(run, y, "entry {t}: yellow length"),
                SignalState::AllRed { .. } => assert_eq!(run, a, "entry {t}: all-red length"),
            }
        }
        run = 0;
    }
}

fn phases_of(state: SignalState) -> (usize, usize) {
    match state {
        SignalState::Green { phase, .. } => (phase.index(), phase.index()),
        SignalState::Yellow { from, to, .. } | SignalState::AllRed { from, to, .. } => {
            (from.index(), to.index())
        }
    }
}

/// Runs `commands` (each repeated) through a signal, checking `can_switch_to` before every
/// step and phase ages after it, and returns the trace.
fn run(plan: &SignalPlan, commands: &[(u8, usize)]) -> Vec<Entry> {
    let n = plan.phases().len();
    let m = plan.max_red_steps();
    let mut signal = Signal::new(plan);
    let record = |s: &Signal| Entry {
        lights: s.lights(),
        state: s.state(),
    };
    let mut trace = vec![record(&signal)];
    for &(code, repeat) in commands {
        // Codes below 4 hold; the others switch, sometimes to an unknown phase.
        let command = match usize::from(code).checked_sub(4) {
            None => Command::Hold,
            Some(k) => Command::SwitchTo(PhaseId::new(k % (n + 1))),
        };
        for _ in 0..repeat {
            for k in 0..=n {
                let id = PhaseId::new(k);
                let mut probe = signal.clone();
                let outcome = probe.step(Command::SwitchTo(id));
                let started = matches!(
                    outcome,
                    StepOutcome::SwitchStarted(to) | StepOutcome::SwitchForced { to, .. } if to == id
                );
                assert_eq!(signal.can_switch_to(id), started, "phase {k}");
            }
            signal.step(command);
            for id in plan.phase_ids() {
                assert!(
                    signal.phase_red_age(id).unwrap() <= m,
                    "phase {id:?} red too long"
                );
            }
            trace.push(record(&signal));
        }
    }
    trace
}

fn commands() -> impl Strategy<Value = Vec<(u8, usize)>> {
    // About 1,000 to 1,600 steps: many short bursts and a few long holds.
    proptest::collection::vec(
        (0u8..12, prop_oneof![3 => 1usize..4, 1 => 10usize..40]),
        300..500,
    )
}

fn total_steps(commands: &[(u8, usize)]) -> usize {
    commands.iter().map(|c| c.1).sum()
}

proptest! {
    #[test]
    fn example_plan_is_safe(
        y in 1u32..6, a in 0u32..5, g in 1u32..12, extra in prop_oneof![Just(0u32), 0u32..60],
        commands in commands(),
    ) {
        let n = PHASES_EXAMPLE as u32;
        let m = n * (y + a) + (n - 1) * g + extra;
        let plan = plan(false, y, a, g, m);
        prop_assume!(total_steps(&commands) >= 1000);
        check_trace(&run(&plan, &commands), y, a, g, m);
    }

    #[test]
    fn overlapping_plan_is_safe(
        y in 1u32..6, a in 0u32..5, g in 1u32..12, extra in prop_oneof![Just(0u32), 0u32..60],
        commands in commands(),
    ) {
        let m = 5 * (y + a) + 4 * g + extra;
        let plan = plan(true, y, a, g, m);
        prop_assume!(total_steps(&commands) >= 1000);
        check_trace(&run(&plan, &commands), y, a, g, m);
    }
}

/// With nobody asking for anything, the signal still serves every phase in time.
#[test]
fn holding_forever_still_serves_every_phase() {
    let plan = plan(false, 3, 2, 5, 55);
    let trace = run(&plan, &[(0, 3000)]);
    check_trace(&trace, 3, 2, 5, 55);
    for phase in 0..PHASES_EXAMPLE {
        assert!(
            trace.iter().any(
                |e| matches!(e.state, SignalState::Green { phase: p, .. } if p.index() == phase)
            ),
            "phase {phase} never green"
        );
    }
}
