//! Validation of the optional `[fixed_time]` plan.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::EXAMPLE;
use rl_semaphore_sim::{ConfigError, Scenario, ScenarioConfig};

/// Three phases with yellow 2 s, all-red 1 s, min-green 3 s and the given max-red,
/// followed by `tail` (the `[fixed_time]` table, or nothing). With greens `g`, the
/// cycle is `sum(g) + 9` and phase `j` is red for `cycle - g[j]` steps.
fn doc(max_red_s: f64, tail: &str) -> String {
    format!(
        r#"
step_s = 1.0

[intersection]
cell_length_m = 7.5

[intersection.approaches.north]
length_m = 30.0
lanes = [{{ movements = ["through"] }}]

[intersection.approaches.south]
length_m = 30.0
lanes = [{{ movements = ["through"] }}]

[intersection.approaches.east]
length_m = 30.0
lanes = [{{ movements = ["through"] }}]

[intersection.approaches.west]
length_m = 30.0
lanes = [{{ movements = ["through"] }}]

[signal]
yellow_s = 2.0
all_red_s = 1.0
min_green_s = 3.0
max_red_s = {max_red_s}

[[signal.phases]]
name = "ns"
green = {{ north = ["through"], south = ["through"] }}

[[signal.phases]]
name = "east"
green = {{ east = ["through"] }}

[[signal.phases]]
name = "west"
green = {{ west = ["through"] }}

{tail}
"#
    )
}

fn with_greens(max_red_s: f64, greens: &str) -> Result<Scenario, ConfigError> {
    Scenario::from_toml_str(&doc(
        max_red_s,
        &format!("[fixed_time]\ngreen_s = {greens}"),
    ))
}

#[test]
fn valid_plan_converts_to_steps() {
    let s = with_greens(30.0, "[5.0, 6.5, 4.0]").unwrap();
    let plan = s.fixed_time().unwrap();
    // 6.5 s is rounded up, like min_green_s.
    assert_eq!(plan.green_steps(), [5, 7, 4]);
    assert_eq!(plan.cycle_steps(), 5 + 7 + 4 + 9);
}

#[test]
fn missing_table_loads_without_a_plan() {
    let s = Scenario::from_toml_str(&doc(30.0, "")).unwrap();
    assert!(s.fixed_time().is_none());
}

#[test]
fn example_has_the_documented_plan() {
    let s = Scenario::from_toml_str(EXAMPLE).unwrap();
    let plan = s.fixed_time().unwrap();
    assert_eq!(plan.green_steps(), [10, 30, 10, 10]);
    assert_eq!(plan.cycle_steps(), 80);
}

#[test]
fn wrong_length_is_rejected() {
    for greens in ["[]", "[5.0, 5.0]", "[5.0, 5.0, 5.0, 5.0]"] {
        let err = with_greens(60.0, greens).unwrap_err();
        assert!(
            matches!(err, ConfigError::FixedTimeLength { expected: 3, .. }),
            "{greens}: {err}"
        );
        assert!(err.to_string().contains("fixed_time.green_s"));
    }
}

#[test]
fn invalid_values_are_rejected() {
    for value in ["0.0", "-5.0", "nan", "inf", "-inf", "1e12"] {
        let err = with_greens(60.0, &format!("[5.0, {value}, 5.0]")).unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidFixedTimeGreen { .. }),
            "{value}: {err}"
        );
        assert!(err.to_string().contains("fixed_time.green_s[1]"), "{err}");
    }
}

#[test]
fn green_below_min_green_is_rejected() {
    let err = with_greens(60.0, "[5.0, 2.0, 5.0]").unwrap_err();
    assert!(
        matches!(
            err,
            ConfigError::FixedTimeGreenTooShort {
                steps: 2,
                min_steps: 3,
                ..
            }
        ),
        "{err}"
    );
    assert!(err.to_string().contains("fixed_time.green_s[1]"));
    // 2.1 s rounds up to 3 steps: exactly min-green, accepted.
    assert!(with_greens(60.0, "[5.0, 2.1, 5.0]").is_ok());
    assert!(with_greens(60.0, "[3.0, 3.0, 3.0]").is_ok());
}

#[test]
fn cycle_at_the_max_red_limit_is_accepted_and_one_step_over_is_rejected() {
    // Greens 5, 5, 5: cycle 24, every phase is red for 19 steps.
    assert!(with_greens(19.0, "[5.0, 5.0, 5.0]").is_ok());
    let err = with_greens(18.0, "[5.0, 5.0, 5.0]").unwrap_err();
    assert!(
        matches!(
            err,
            ConfigError::FixedTimeCycleExceedsMaxRed {
                red_steps: 19,
                max_red_steps: 18,
                ..
            }
        ),
        "{err}"
    );

    // Uneven greens 4, 8, 5: cycle 26, so the phases are red for 22, 18 and 21 steps.
    // The shortest green waits the longest, so the limit is 22.
    assert!(with_greens(22.0, "[4.0, 8.0, 5.0]").is_ok());
    let err = with_greens(21.0, "[4.0, 8.0, 5.0]").unwrap_err();
    assert!(
        matches!(
            err,
            ConfigError::FixedTimeCycleExceedsMaxRed { red_steps: 22, .. }
        ),
        "{err}"
    );
    assert!(err.to_string().contains("fixed_time.green_s[0]"));
}

/// One phase: every approach has a single right-turn lane, and right turns do not conflict.
fn single_phase(tail: &str) -> String {
    let approach = |d: &str| {
        format!(
            "[intersection.approaches.{d}]\nlength_m = 30.0\nlanes = [{{ movements = [\"right\"] }}]\n\n"
        )
    };
    format!(
        r#"
step_s = 1.0

[intersection]
cell_length_m = 7.5

{}{}{}{}
[signal]
yellow_s = 2.0
all_red_s = 1.0
min_green_s = 3.0
max_red_s = 1.0

[[signal.phases]]
name = "all"
green = {{ north = ["right"], east = ["right"], south = ["right"], west = ["right"] }}

{tail}
"#,
        approach("north"),
        approach("east"),
        approach("south"),
        approach("west"),
    )
}

#[test]
fn a_single_phase_skips_the_cycle_check() {
    // max-red is 1 step and the cycle is 15 + 3 = 18, but the only phase is never red.
    let s = Scenario::from_toml_str(&single_phase("[fixed_time]\ngreen_s = [15.0]")).unwrap();
    let plan = s.fixed_time().unwrap();
    assert_eq!(plan.green_steps(), [15]);
    assert_eq!(plan.cycle_steps(), 18);

    // The length and min-green checks still apply.
    let err =
        Scenario::from_toml_str(&single_phase("[fixed_time]\ngreen_s = [5.0, 5.0]")).unwrap_err();
    assert!(matches!(
        err,
        ConfigError::FixedTimeLength {
            expected: 1,
            got: 2,
            ..
        }
    ));
    let err = Scenario::from_toml_str(&single_phase("[fixed_time]\ngreen_s = [2.0]")).unwrap_err();
    assert!(matches!(err, ConfigError::FixedTimeGreenTooShort { .. }));
}

#[test]
fn round_trip_with_and_without_the_table() {
    for text in [
        doc(30.0, "[fixed_time]\ngreen_s = [5.0, 6.5, 4.0]"),
        doc(30.0, ""),
        EXAMPLE.to_owned(),
    ] {
        let scenario = Scenario::from_toml_str(&text).unwrap();
        let config = scenario.to_config();
        let toml = config.to_toml_string().unwrap();
        assert_eq!(
            toml.contains("[fixed_time]"),
            scenario.fixed_time().is_some()
        );
        let again = Scenario::from_toml_str(&toml).unwrap();
        assert_eq!(again, scenario);
        // The configured seconds are kept, not the rounded steps.
        assert_eq!(again.to_config(), config);
    }
    let scenario =
        Scenario::from_toml_str(&doc(30.0, "[fixed_time]\ngreen_s = [5.0, 6.5, 4.0]")).unwrap();
    assert_eq!(
        scenario.to_config().fixed_time.unwrap().green_s,
        [5.0, 6.5, 4.0]
    );
}

#[test]
fn unknown_keys_in_the_table_are_rejected() {
    let text = doc(
        30.0,
        "[fixed_time]\ngreen_s = [5.0, 5.0, 5.0]\noffset_s = 3.0",
    );
    assert!(matches!(
        ScenarioConfig::from_toml_str(&text),
        Err(ConfigError::Parse(_))
    ));
}
