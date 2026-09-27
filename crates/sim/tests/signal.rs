//! Tests for movements, the scenario config and the signal state machine.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use rl_semaphore_sim::{
    Command, ConfigError, Direction, IgnoredReason, Light, MAX_PHASES, Movement, MovementId,
    PhaseId, Scenario, ScenarioConfig, Signal, SignalState, StepOutcome,
};

const EXAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/single-intersection.toml"
));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The example file up to (not including) its `[signal]` table.
fn head() -> &'static str {
    EXAMPLE.split("\n[signal]\n").next().unwrap()
}

/// The example geometry with the given `[signal]` table (timings and phases).
fn doc(signal: &str) -> String {
    format!("{}\n[signal]\n{signal}", head())
}

const PHASES_EXAMPLE: &str = r#"
[[signal.phases]]
name = "ns-left"
green = { north = ["left"], south = ["left"] }

[[signal.phases]]
name = "ns-through"
green = { north = ["through", "right"], south = ["through", "right"] }

[[signal.phases]]
name = "east"
green = { east = ["left", "through", "right"] }

[[signal.phases]]
name = "west"
green = { west = ["left", "through", "right"] }
"#;

/// Five phases, two of which share movements (`north.through`, `north.right`).
const PHASES_OVERLAP: &str = r#"
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

fn timings(yellow: u32, all_red: u32, min_green: u32, max_red: u32) -> String {
    format!(
        "yellow_s = {yellow}.0\nall_red_s = {all_red}.0\nmin_green_s = {min_green}.0\nmax_red_s = {max_red}.0\n"
    )
}

fn scenario_with(phases: &str, y: u32, a: u32, g: u32, m: u32) -> Scenario {
    Scenario::from_toml_str(&doc(&format!("{}{phases}", timings(y, a, g, m)))).unwrap()
}

/// (yellow, all-red, min-green, max-red) in steps.
fn steps(text: &str) -> (u32, u32, u32, u32) {
    let s = Scenario::from_toml_str(text).unwrap();
    let plan = s.signal_plan();
    (
        plan.yellow_steps(),
        plan.all_red_steps(),
        plan.min_green_steps(),
        plan.max_red_steps(),
    )
}

fn err(text: &str) -> ConfigError {
    Scenario::from_toml_str(text).expect_err("config should be rejected")
}

/// An error for the example with `from` replaced by `to`.
fn err_replacing(from: &str, to: &str) -> ConfigError {
    assert!(EXAMPLE.contains(from), "example does not contain {from:?}");
    err(&EXAMPLE.replacen(from, to, 1))
}

fn mv(approach: Direction, movement: Movement) -> MovementId {
    MovementId::new(approach, movement)
}

const N: Direction = Direction::North;
const E: Direction = Direction::East;
const S: Direction = Direction::South;
const W: Direction = Direction::West;
const L: Movement = Movement::Left;
const T: Movement = Movement::Through;
const R: Movement = Movement::Right;

// ---------------------------------------------------------------------------
// R2: movements and the conflict matrix
// ---------------------------------------------------------------------------

#[test]
fn movement_ids_are_dense_and_ordered() {
    assert_eq!(MovementId::ALL.len(), 12);
    for (i, m) in MovementId::ALL.into_iter().enumerate() {
        assert_eq!(m.index(), i);
    }
    assert_eq!(MovementId::ALL[0], mv(N, L));
    assert_eq!(MovementId::ALL[1], mv(N, T));
    assert_eq!(MovementId::ALL[3], mv(E, L));
    assert_eq!(MovementId::ALL[11], mv(W, R));
    assert_eq!(mv(N, L).to_string(), "north.left");
}

/// The conflicts of a movement, written out per relative position (R2.4).
/// `rel` is the number of clockwise quarter turns from the movement's approach.
fn table(movement: Movement, rel: usize) -> &'static [Movement] {
    match (movement, rel) {
        (L, 2) => &[T, R],
        (L, 1 | 3) => &[T, L],
        (T, 2) => &[L],
        (T, 1) => &[T, L],
        (T, 3) => &[T, L, R],
        (R, 2) => &[L],
        (R, 1) => &[T],
        _ => &[],
    }
}

#[test]
fn conflict_matrix_matches_the_table() {
    let mut unordered = 0;
    let mut pairs = 0;
    for a in MovementId::ALL {
        for b in MovementId::ALL {
            pairs += 1;
            let rel = (b.approach.index() + 4 - a.approach.index()) % 4;
            let expected = table(a.movement, rel).contains(&b.movement);
            assert_eq!(a.conflicts_with(b), expected, "{a} vs {b}");
            // Symmetry.
            assert_eq!(a.conflicts_with(b), b.conflicts_with(a), "{a} vs {b}");
            if a.conflicts_with(b) {
                assert_ne!(a, b, "irreflexive");
                assert_ne!(a.approach, b.approach, "same approach never conflicts");
                if a < b {
                    unordered += 1;
                }
            }
        }
    }
    assert_eq!(pairs, 144);
    assert_eq!(unordered, 28);
}

#[test]
fn conflict_spot_checks() {
    // Crossing, opposing left vs through, merging into the same exit.
    assert!(mv(N, T).conflicts_with(mv(E, T)));
    assert!(mv(N, L).conflicts_with(mv(S, T)));
    assert!(mv(N, L).conflicts_with(mv(W, T))); // both exit east
    assert!(mv(S, R).conflicts_with(mv(W, T))); // both exit east... via right turn
    // Compatible.
    assert!(!mv(N, T).conflicts_with(mv(S, T)));
    assert!(!mv(N, L).conflicts_with(mv(S, L)));
    assert!(!mv(N, R).conflicts_with(mv(E, R)));
    assert!(!mv(N, R).conflicts_with(mv(S, R)));
}

// ---------------------------------------------------------------------------
// R3: scenario config and time
// ---------------------------------------------------------------------------

#[test]
fn example_scenario_loads() {
    let s = Scenario::from_toml_str(EXAMPLE).unwrap();
    assert!((s.step_s() - 1.0).abs() < f64::EPSILON);
    assert_eq!(s.intersection().approaches().count(), 4);

    let plan = s.signal_plan();
    assert_eq!(plan.yellow_steps(), 3);
    assert_eq!(plan.all_red_steps(), 2);
    assert_eq!(plan.min_green_steps(), 5);
    assert_eq!(plan.max_red_steps(), 90);

    let names: Vec<_> = plan.phases().iter().map(|p| p.name()).collect();
    assert_eq!(names, ["ns-left", "ns-through", "east", "west"]);
    let granted = |i: usize| plan.phases()[i].movements().collect::<Vec<_>>();
    assert_eq!(granted(0), [mv(N, L), mv(S, L)]);
    assert_eq!(granted(1), [mv(N, T), mv(N, R), mv(S, T), mv(S, R)]);
    assert_eq!(granted(2), [mv(E, L), mv(E, T), mv(E, R)]);
    assert_eq!(granted(3), [mv(W, L), mv(W, T), mv(W, R)]);
    assert_eq!(plan.phase_ids().count(), 4);
    assert!(plan.phase(PhaseId::new(4)).is_none());
}

#[test]
fn durations_convert_with_the_right_rounding() {
    let text = EXAMPLE
        .replace("step_s = 1.0", "step_s = 0.5")
        .replace("yellow_s = 3.0", "yellow_s = 3.3") // 6.6 -> 7 (up)
        .replace("all_red_s = 2.0", "all_red_s = 0.0") // 0
        .replace("min_green_s = 5.0", "min_green_s = 5.1") // 10.2 -> 11 (up)
        .replace("max_red_s = 90.0", "max_red_s = 90.3"); // 180.6 -> 180 (down)
    let plan = steps(&text);
    assert_eq!(plan, (7, 0, 11, 180));
}

#[test]
fn near_integer_quotients_snap() {
    // 0.9 / 0.3 is 3.0000000000000004 in floating point: it must give 3, not 4.
    let text = EXAMPLE
        .replace("step_s = 1.0", "step_s = 0.3")
        .replace("yellow_s = 3.0", "yellow_s = 0.9")
        .replace("all_red_s = 2.0", "all_red_s = 0.6")
        .replace("min_green_s = 5.0", "min_green_s = 1.5")
        .replace("max_red_s = 90.0", "max_red_s = 30.0");
    let plan = steps(&text);
    assert_eq!(plan, (3, 2, 5, 100));
}

#[test]
fn scenario_round_trips_through_toml() {
    let s = Scenario::from_toml_str(EXAMPLE).unwrap();
    let text = s.to_config().to_toml_string().unwrap();
    assert_eq!(Scenario::from_toml_str(&text).unwrap(), s);
    assert_eq!(ScenarioConfig::from_toml_str(&text).unwrap(), s.to_config());

    // Movements listed out of order are written back in canonical order.
    let shuffled = EXAMPLE.replace(
        r#"north = ["through", "right"], south = ["through", "right"]"#,
        r#"north = ["right", "through"], south = ["right", "through"]"#,
    );
    assert_ne!(shuffled, EXAMPLE);
    let s2 = Scenario::from_toml_str(&shuffled).unwrap();
    assert_eq!(s2.signal_plan(), s.signal_plan());
    let text2 = s2.to_config().to_toml_string().unwrap();
    assert_eq!(Scenario::from_toml_str(&text2).unwrap(), s2);
    assert_eq!(text2, text);
}

// ---------------------------------------------------------------------------
// R4: validation
// ---------------------------------------------------------------------------

#[test]
fn rejects_bad_step() {
    for bad in ["0.0", "-1.0", "nan", "inf"] {
        let e = err_replacing("step_s = 1.0", &format!("step_s = {bad}"));
        assert!(
            matches!(e, ConfigError::InvalidStepLength { .. }),
            "{bad}: {e}"
        );
        assert!(e.to_string().contains("step_s"), "{e}");
    }
}

#[test]
fn intersection_errors_carry_the_prefix() {
    let e = err_replacing("cell_length_m = 7.5", "cell_length_m = 0.0");
    assert!(matches!(e, ConfigError::InvalidCellLength { .. }), "{e}");
    assert!(e.to_string().contains("intersection.cell_length_m"), "{e}");

    let e = err_replacing("length_m = 150.0", "length_m = -1.0");
    assert!(matches!(e, ConfigError::InvalidLength { .. }), "{e}");
    assert!(
        e.to_string()
            .contains("intersection.approaches.north.length_m"),
        "{e}"
    );
}

#[test]
fn rejects_bad_durations() {
    for field in ["yellow_s", "all_red_s", "min_green_s", "max_red_s"] {
        for bad in ["-1.0", "nan", "inf"] {
            let line = EXAMPLE
                .lines()
                .find(|l| l.starts_with(field))
                .unwrap()
                .to_owned();
            let e = err_replacing(&line, &format!("{field} = {bad}"));
            assert!(
                matches!(e, ConfigError::InvalidDuration { .. }),
                "{field} {bad}: {e}"
            );
            assert!(e.to_string().contains(&format!("signal.{field}")), "{e}");
        }
    }
}

#[test]
fn rejects_durations_out_of_range() {
    for (from, to, field) in [
        ("yellow_s = 3.0", "yellow_s = 0.0", "yellow_s"),
        ("min_green_s = 5.0", "min_green_s = 0.0", "min_green_s"),
        ("max_red_s = 90.0", "max_red_s = 0.5", "max_red_s"), // rounds down to 0
        ("max_red_s = 90.0", "max_red_s = 1e12", "max_red_s"), // does not fit in u32
    ] {
        let e = err_replacing(from, to);
        assert!(
            matches!(e, ConfigError::DurationOutOfRange { .. }),
            "{to}: {e}"
        );
        assert!(e.to_string().contains(&format!("signal.{field}")), "{e}");
    }
    // Zero all-red is allowed.
    let text = EXAMPLE.replace("all_red_s = 2.0", "all_red_s = 0.0");
    assert_eq!(
        Scenario::from_toml_str(&text)
            .unwrap()
            .signal_plan()
            .all_red_steps(),
        0
    );
}

#[test]
fn rejects_bad_phase_count() {
    let e = err(&doc(&format!("{}phases = []\n", timings(3, 2, 5, 90))));
    assert!(matches!(e, ConfigError::PhaseCount { count: 0, .. }), "{e}");
    assert!(e.to_string().contains("signal.phases"), "{e}");

    let one = r#"
[[signal.phases]]
name = "p"
green = { north = ["left"] }
"#;
    let nine = one.repeat(MAX_PHASES + 1);
    let e = err(&doc(&format!("{}{nine}", timings(3, 2, 5, 90))));
    assert!(matches!(e, ConfigError::PhaseCount { count: 9, .. }), "{e}");
}

#[test]
fn rejects_bad_phase_names() {
    let e = err_replacing(r#"name = "east""#, r#"name = """#);
    assert!(matches!(e, ConfigError::EmptyPhaseName { .. }), "{e}");
    assert!(e.to_string().contains("signal.phases[2].name"), "{e}");

    let e = err_replacing(r#"name = "west""#, r#"name = "ns-left""#);
    assert!(matches!(e, ConfigError::DuplicatePhaseName { .. }), "{e}");
    assert!(e.to_string().contains("signal.phases[3].name"), "{e}");
    assert!(e.to_string().contains("ns-left"), "{e}");
}

#[test]
fn rejects_bad_phase_movements() {
    let e = err_replacing(
        r#"green = { west = ["left", "through", "right"] }"#,
        "green = {}",
    );
    assert!(matches!(e, ConfigError::EmptyPhase { .. }), "{e}");
    assert!(e.to_string().contains("signal.phases[3].green"), "{e}");

    let e = err_replacing(
        r#"green = { west = ["left", "through", "right"] }"#,
        r#"green = { west = ["left", "through", "left"] }"#,
    );
    assert!(
        matches!(e, ConfigError::DuplicatePhaseMovement { movement: L, .. }),
        "{e}"
    );
    assert!(
        e.to_string().contains("signal.phases[3].green.west[2]"),
        "{e}"
    );
}

#[test]
fn rejects_movement_not_in_geometry() {
    // The east lane only allows through, so no phase may grant east.left.
    let east_lane = r#"lanes = [
  { movements = ["left", "through", "right"] },
]"#;
    let text = EXAMPLE.replacen(east_lane, r#"lanes = [ { movements = ["through"] } ]"#, 1);
    // The first match is the east approach (the file lists it before west).
    let e = err(&text);
    assert!(
        matches!(e, ConfigError::MovementNotInGeometry { movement: L, .. }),
        "{e}"
    );
    assert!(
        e.to_string().contains("signal.phases[2].green.east[0]"),
        "{e}"
    );
}

#[test]
fn rejects_conflicting_movements() {
    let e = err_replacing(
        r#"green = { west = ["left", "through", "right"] }"#,
        r#"green = { north = ["through"], east = ["through"] }"#,
    );
    match &e {
        ConfigError::ConflictingMovements { first, second, .. } => {
            assert_eq!((*first, *second), (mv(N, T), mv(E, T)));
        }
        other => panic!("unexpected error: {other}"),
    }
    let text = e.to_string();
    assert!(text.contains("signal.phases[3].green"), "{text}");
    assert!(
        text.contains("north.through") && text.contains("east.through"),
        "{text}"
    );
}

#[test]
fn rejects_duplicate_phases() {
    let e = err_replacing(
        r#"green = { west = ["left", "through", "right"] }"#,
        r#"green = { east = ["right", "left", "through"] }"#,
    );
    assert!(matches!(e, ConfigError::DuplicatePhase { .. }), "{e}");
    assert!(e.to_string().contains("signal.phases[3].green"), "{e}");
    assert!(e.to_string().contains("\"east\""), "{e}");
}

#[test]
fn rejects_uncovered_movements() {
    // Drop the west phase: the first uncovered movement is west.left.
    let cut = EXAMPLE.find("[[signal.phases]]\nname = \"west\"").unwrap();
    let e = err(&EXAMPLE[..cut]);
    assert!(
        matches!(e, ConfigError::UncoveredMovement { movement } if movement == mv(W, L)),
        "{e}"
    );
    assert!(e.to_string().contains("west.left"), "{e}");
}

#[test]
fn rejects_short_max_red() {
    // 4 phases: 4 * (3 + 2) + 3 * 5 = 35 steps at least.
    let e = err_replacing("max_red_s = 90.0", "max_red_s = 34.0");
    match &e {
        ConfigError::MaxRedTooShort {
            phases,
            required_steps,
            got_steps,
            ..
        } => assert_eq!((*phases, *required_steps, *got_steps), (4, 35, 34)),
        other => panic!("unexpected error: {other}"),
    }
    assert!(e.to_string().contains("35"), "{e}");
    assert!(
        Scenario::from_toml_str(&EXAMPLE.replace("max_red_s = 90.0", "max_red_s = 35.0")).is_ok()
    );
}

#[test]
fn rejects_unknown_keys() {
    for (from, to) in [
        (r#"green = { west ="#, r#"green = { up = ["left"], west ="#),
        ("[signal]\n", "[signal]\nextra = 1\n"),
        ("step_s = 1.0", "step_s = 1.0\nextra = 1"),
    ] {
        let e = err_replacing(from, to);
        assert!(matches!(e, ConfigError::Parse(_)), "{to}: {e}");
    }
}

#[test]
fn first_error_follows_the_fixed_order() {
    // step_s beats the intersection, which beats timings, which beat phases.
    let text = EXAMPLE
        .replace("step_s = 1.0", "step_s = 0.0")
        .replace("cell_length_m = 7.5", "cell_length_m = 0.0");
    assert!(matches!(err(&text), ConfigError::InvalidStepLength { .. }));
    let text = EXAMPLE
        .replace("cell_length_m = 7.5", "cell_length_m = 0.0")
        .replace("yellow_s = 3.0", "yellow_s = -1.0");
    assert!(matches!(err(&text), ConfigError::InvalidCellLength { .. }));
    let text = EXAMPLE
        .replace("yellow_s = 3.0", "yellow_s = -1.0")
        .replace(r#"name = "east""#, r#"name = """#);
    assert!(matches!(err(&text), ConfigError::InvalidDuration { .. }));
    let text = EXAMPLE
        .replace("max_red_s = 90.0", "max_red_s = 1.0")
        .replace(r#"name = "east""#, r#"name = """#);
    assert!(matches!(err(&text), ConfigError::EmptyPhaseName { .. }));
}

// ---------------------------------------------------------------------------
// R5, R6: the state machine
// ---------------------------------------------------------------------------

/// Example plan with yellow 2, all-red 1, min-green 3.
fn signal_yag(max_red: u32) -> Signal {
    let s = scenario_with(PHASES_EXAMPLE, 2, 1, 3, max_red);
    Signal::new(s.signal_plan())
}

fn p(i: usize) -> PhaseId {
    PhaseId::new(i)
}

fn lights(signal: &Signal, movements: &[MovementId]) -> Vec<Light> {
    movements.iter().map(|&m| signal.light(m)).collect()
}

#[test]
fn initial_state() {
    let signal = signal_yag(100);
    assert_eq!(
        signal.state(),
        SignalState::Green {
            phase: p(0),
            elapsed: 1
        }
    );
    assert_eq!(signal.light(mv(N, L)), Light::Green);
    assert_eq!(signal.light(mv(S, L)), Light::Green);
    assert_eq!(signal.light(mv(N, T)), Light::Red);
    assert_eq!(signal.phase_red_age(p(0)), Some(0));
    assert_eq!(signal.phase_red_age(p(1)), Some(1));
    assert_eq!(signal.phase_red_age(p(9)), None);
    assert_eq!(signal.movement_red_age(mv(N, L)), 0);
    assert_eq!(signal.movement_red_age(mv(N, T)), 1);
}

#[test]
fn hand_traced_switch_with_yellow_2_all_red_1_min_green_3() {
    let mut signal = signal_yag(100);
    let watched = [mv(N, L), mv(S, L), mv(N, T), mv(E, T)];
    use Light::{Green as G, Red as Rd, Yellow as Y};

    // Entries 0-2: min-green is not reached yet, so the request is refused.
    assert_eq!(
        signal.step(Command::SwitchTo(p(1))),
        StepOutcome::Ignored(IgnoredReason::MinGreenNotReached)
    );
    assert_eq!(
        signal.state(),
        SignalState::Green {
            phase: p(0),
            elapsed: 2
        }
    );
    assert!(!signal.can_switch_to(p(1)));
    assert_eq!(
        signal.step(Command::SwitchTo(p(1))),
        StepOutcome::Ignored(IgnoredReason::MinGreenNotReached)
    );
    assert_eq!(
        signal.state(),
        SignalState::Green {
            phase: p(0),
            elapsed: 3
        }
    );
    assert!(signal.can_switch_to(p(1)));

    // Entry 3: the step that accepts the switch already shows yellow.
    assert_eq!(
        signal.step(Command::SwitchTo(p(1))),
        StepOutcome::SwitchStarted(p(1))
    );
    assert_eq!(
        signal.state(),
        SignalState::Yellow {
            from: p(0),
            to: p(1),
            elapsed: 1
        }
    );
    assert_eq!(lights(&signal, &watched), [Y, Y, Rd, Rd]);

    // Entry 4: commands during a transition are ignored, and the yellow is 2 entries.
    assert_eq!(
        signal.step(Command::SwitchTo(p(2))),
        StepOutcome::Ignored(IgnoredReason::TransitionInProgress)
    );
    assert_eq!(
        signal.state(),
        SignalState::Yellow {
            from: p(0),
            to: p(1),
            elapsed: 2
        }
    );
    assert_eq!(lights(&signal, &watched), [Y, Y, Rd, Rd]);
    assert_eq!(signal.phase_red_age(p(0)), Some(2));

    // Entry 5: all-red for exactly one entry.
    assert_eq!(signal.step(Command::Hold), StepOutcome::Held);
    assert_eq!(
        signal.state(),
        SignalState::AllRed {
            from: p(0),
            to: p(1),
            elapsed: 1
        }
    );
    assert_eq!(lights(&signal, &watched), [Rd, Rd, Rd, Rd]);

    // Entry 6: the target phase is green.
    assert_eq!(signal.step(Command::Hold), StepOutcome::Held);
    assert_eq!(
        signal.state(),
        SignalState::Green {
            phase: p(1),
            elapsed: 1
        }
    );
    assert_eq!(lights(&signal, &watched), [Rd, Rd, G, Rd]);
    assert_eq!(signal.phase_red_age(p(1)), Some(0));
    assert_eq!(signal.phase_red_age(p(0)), Some(4));
    assert_eq!(signal.movement_red_age(mv(N, T)), 0);
    // Red at entries 5 (all-red) and 6.
    assert_eq!(signal.movement_red_age(mv(N, L)), 2);
}

#[test]
fn zero_all_red_goes_straight_to_green() {
    let s = scenario_with(PHASES_EXAMPLE, 2, 0, 1, 100);
    let mut signal = Signal::new(s.signal_plan());
    assert_eq!(
        signal.step(Command::SwitchTo(p(1))),
        StepOutcome::SwitchStarted(p(1))
    );
    signal.step(Command::Hold);
    assert!(matches!(
        signal.state(),
        SignalState::Yellow { elapsed: 2, .. }
    ));
    signal.step(Command::Hold);
    assert_eq!(
        signal.state(),
        SignalState::Green {
            phase: p(1),
            elapsed: 1
        }
    );
}

#[test]
fn refuses_unknown_and_current_phases() {
    let mut signal = signal_yag(100);
    for _ in 0..3 {
        signal.step(Command::Hold);
    }
    assert_eq!(
        signal.step(Command::SwitchTo(p(0))),
        StepOutcome::Ignored(IgnoredReason::AlreadyOnPhase)
    );
    assert_eq!(
        signal.step(Command::SwitchTo(p(4))),
        StepOutcome::Ignored(IgnoredReason::UnknownPhase)
    );
    assert!(!signal.can_switch_to(p(0)));
    assert!(!signal.can_switch_to(p(4)));
}

#[test]
fn movements_green_in_both_phases_stay_green() {
    let s = scenario_with(PHASES_OVERLAP, 2, 1, 3, 100);
    let mut signal = Signal::new(s.signal_plan());
    let stays = [mv(N, T), mv(N, R)];
    let leaves = [mv(S, T), mv(S, R)];
    let arrives = [mv(N, L)];

    for _ in 0..2 {
        signal.step(Command::Hold);
    }
    assert_eq!(
        signal.step(Command::SwitchTo(p(1))),
        StepOutcome::SwitchStarted(p(1))
    );
    // Two yellow entries and one all-red entry.
    for _ in 0..3 {
        assert_eq!(lights(&signal, &stays), [Light::Green; 2]);
        assert_eq!(lights(&signal, &arrives), [Light::Red]);
        signal.step(Command::Hold);
    }
    // Yellow shown to the movements that leave, then red.
    let mut signal = Signal::new(s.signal_plan());
    for _ in 0..2 {
        signal.step(Command::Hold);
    }
    signal.step(Command::SwitchTo(p(1)));
    assert_eq!(lights(&signal, &leaves), [Light::Yellow; 2]);
    signal.step(Command::Hold);
    assert_eq!(lights(&signal, &leaves), [Light::Yellow; 2]);
    signal.step(Command::Hold);
    assert!(matches!(signal.state(), SignalState::AllRed { .. }));
    assert_eq!(lights(&signal, &leaves), [Light::Red; 2]);
    assert_eq!(lights(&signal, &stays), [Light::Green; 2]);
    assert_eq!(lights(&signal, &arrives), [Light::Red]);
    signal.step(Command::Hold);
    assert_eq!(
        signal.state(),
        SignalState::Green {
            phase: p(1),
            elapsed: 1
        }
    );
    assert_eq!(lights(&signal, &arrives), [Light::Green]);
    assert_eq!(lights(&signal, &stays), [Light::Green; 2]);
    assert_eq!(lights(&signal, &leaves), [Light::Red; 2]);
}

#[test]
fn forces_a_switch_when_max_red_gets_tight() {
    // 4 phases with Y + A = 3 and G = 3: the bound is 4 * 3 + 3 * 3 = 21, which is tight.
    let mut signal = signal_yag(21);
    let mut forced_at = None;
    for step in 1..=20 {
        match signal.step(Command::Hold) {
            StepOutcome::Held => {}
            StepOutcome::SwitchForced { to, overrode } => {
                assert_eq!(overrode, Command::Hold);
                forced_at = Some((step, to));
                break;
            }
            other => panic!("unexpected outcome {other:?} at step {step}"),
        }
    }
    // All other phases are tied, so the lowest id (1) is served first.
    assert_eq!(forced_at, Some((6, p(1))));
    assert!(matches!(signal.state(), SignalState::Yellow { to, .. } if to == p(1)));
}

#[test]
fn forced_switch_overrides_a_refused_command() {
    let mut signal = signal_yag(21);
    // Ask for the current phase over and over: it is refused, until the deadline forces a switch.
    let mut outcomes = Vec::new();
    for _ in 0..6 {
        outcomes.push(signal.step(Command::SwitchTo(p(0))));
    }
    assert_eq!(
        outcomes.last(),
        Some(&StepOutcome::SwitchForced {
            to: p(1),
            overrode: Command::SwitchTo(p(0))
        })
    );
    assert!(
        outcomes[..5]
            .iter()
            .all(|o| *o == StepOutcome::Ignored(IgnoredReason::AlreadyOnPhase))
    );
}

#[test]
fn refuses_a_switch_that_would_starve_another_phase() {
    let mut signal = signal_yag(23);
    for _ in 0..2 {
        signal.step(Command::Hold);
    }
    assert_eq!(
        signal.step(Command::SwitchTo(p(1))),
        StepOutcome::SwitchStarted(p(1))
    );
    for _ in 0..3 {
        signal.step(Command::Hold);
    }
    for _ in 0..2 {
        signal.step(Command::Hold);
    }
    assert_eq!(
        signal.state(),
        SignalState::Green {
            phase: p(1),
            elapsed: 3
        }
    );
    assert_eq!(signal.phase_red_age(p(0)), Some(6));
    assert_eq!(signal.phase_red_age(p(2)), Some(9));

    // Going back to phase 0 would leave phases 2 and 3 waiting too long.
    assert!(!signal.can_switch_to(p(0)));
    assert_eq!(
        signal.step(Command::SwitchTo(p(0))),
        StepOutcome::Ignored(IgnoredReason::MaxRedDeadline)
    );
}

#[test]
fn can_switch_to_matches_step_on_a_clone() {
    let mut signal = signal_yag(30);
    for i in 0..200_usize {
        for k in 0..6 {
            let mut probe = signal.clone();
            let outcome = probe.step(Command::SwitchTo(p(k)));
            let started = matches!(
                outcome,
                StepOutcome::SwitchStarted(to) | StepOutcome::SwitchForced { to, .. } if to == p(k)
            );
            assert_eq!(signal.can_switch_to(p(k)), started, "step {i}, phase {k}");
        }
        signal.step(if i % 7 == 0 {
            Command::SwitchTo(p(i % 4))
        } else {
            Command::Hold
        });
    }
}
