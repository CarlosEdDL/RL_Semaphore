//! Hand-built scenarios with round timings, so the fixed-time cycle can be computed by hand.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use rl_semaphore_env::Controller;
use rl_semaphore_sim::{Command, ConfigError, Scenario, Simulation, StepOutcome};

pub const EXAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/single-intersection.toml"
));

/// Yellow, all-red and min-green of every scenario built by [`scenario`], in steps.
pub const YELLOW: u32 = 2;
pub const ALL_RED: u32 = 1;
pub const MIN_GREEN: u32 = 3;

/// A scenario with `phases` phases (1 to 4), `step_s = 1`, yellow 2 s, all-red 1 s and
/// min-green 3 s, and a `[fixed_time]` table with `greens_s`.
///
/// - 1 phase: every approach has one right-turn lane, all green together.
/// - 2 phases: north-south through, then east-west through.
/// - 3 phases: north-south through, then east, then west.
/// - 4 phases: north, south, east and west, each on its own.
///
/// The cycle is `sum(greens) + phases * 3` steps.
pub fn scenario(phases: usize, max_red_s: f64, greens_s: &[f64]) -> Result<Scenario, ConfigError> {
    let movement = if phases == 1 { "right" } else { "through" };
    let mut text = String::from("step_s = 1.0\n\n[intersection]\ncell_length_m = 7.5\n");
    for d in ["north", "east", "south", "west"] {
        text += &format!(
            "\n[intersection.approaches.{d}]\nlength_m = 30.0\nlanes = [{{ movements = [\"{movement}\"] }}]\n"
        );
    }
    text += &format!(
        "\n[signal]\nyellow_s = {YELLOW}.0\nall_red_s = {ALL_RED}.0\nmin_green_s = {MIN_GREEN}.0\nmax_red_s = {max_red_s}\n"
    );
    let groups: &[&[&str]] = match phases {
        1 => &[&["north", "east", "south", "west"]],
        2 => &[&["north", "south"], &["east", "west"]],
        3 => &[&["north", "south"], &["east"], &["west"]],
        _ => &[&["north"], &["south"], &["east"], &["west"]],
    };
    for (i, group) in groups.iter().enumerate() {
        let green: Vec<String> = group
            .iter()
            .map(|d| format!("{d} = [\"{movement}\"]"))
            .collect();
        text += &format!(
            "\n[[signal.phases]]\nname = \"p{i}\"\ngreen = {{ {} }}\n",
            green.join(", ")
        );
    }
    let greens: Vec<String> = greens_s.iter().map(|g| format!("{g:?}")).collect();
    text += &format!("\n[fixed_time]\ngreen_s = [{}]\n", greens.join(", "));
    Scenario::from_toml_str(&text)
}

/// Steps `sim` with `controller` for `steps` steps, without demand, and returns each
/// step's outcome.
pub fn drive(
    sim: &mut Simulation,
    controller: &mut impl Controller,
    steps: usize,
) -> Vec<StepOutcome> {
    (0..steps)
        .map(|_| {
            let command: Command = controller.command(sim);
            sim.step(command).signal
        })
        .collect()
}
