//! Runs the built binary and checks exit codes and the shape of its output.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::{Command, Output};

/// The workspace root, so config paths in arguments (and in the output) are relative.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const EXAMPLE: &str = "configs/single-intersection.toml";

fn run(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rl-semaphore"));
    cmd.current_dir(ROOT)
        .args(args)
        .env_remove("RUST_LOG")
        .env_remove("RL_SEMAPHORE_LOG_FORMAT");
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output()
        .unwrap_or_else(|e| panic!("failed to run rl-semaphore: {e}"))
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn assert_json_error_lines(out: &Output) {
    let text = stderr(out);
    let mut saw_error = false;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("stderr line is not JSON ({e}): {line}"));
        saw_error |= value["level"] == "ERROR";
    }
    assert!(saw_error, "no ERROR event in stderr: {text}");
}

#[test]
fn unimplemented_command_exits_with_code_1() {
    let out = run(&["train"], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("not implemented yet"));
    assert!(out.stdout.is_empty());
}

#[test]
fn json_flag_emits_json_lines() {
    let out = run(&["--log-format", "json", "train"], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert_json_error_lines(&out);
    assert!(out.stdout.is_empty());
}

#[test]
fn json_env_var_emits_json_lines() {
    let out = run(&["train"], &[("RL_SEMAPHORE_LOG_FORMAT", "json")]);
    assert_eq!(out.status.code(), Some(1));
    assert_json_error_lines(&out);
    assert!(out.stdout.is_empty());
}

#[test]
fn flag_takes_precedence_over_env_var() {
    let out = run(
        &["--log-format", "pretty", "train"],
        &[("RL_SEMAPHORE_LOG_FORMAT", "json")],
    );
    assert!(!stderr(&out).trim_start().starts_with('{'));
}

#[test]
fn verbose_shows_startup_debug_event() {
    let quiet_default = run(&["train"], &[]);
    assert!(!stderr(&quiet_default).contains("starting"));

    let verbose = run(&["-v", "train"], &[]);
    assert!(stderr(&verbose).contains("starting"));
    assert!(verbose.stdout.is_empty());
}

#[test]
fn quiet_hides_info_but_rust_log_overrides_flags() {
    let quiet = run(&["-qq", "train"], &[]);
    // -qq keeps errors, so the failure is still reported.
    assert!(stderr(&quiet).contains("not implemented yet"));

    let off = run(&["-qqq", "train"], &[]);
    assert_eq!(off.status.code(), Some(1));
    assert!(stderr(&off).is_empty());

    let rust_log = run(&["-qqq", "train"], &[("RUST_LOG", "debug")]);
    assert!(stderr(&rust_log).contains("starting"));
}

#[test]
fn invalid_rust_log_warns_and_falls_back() {
    let out = run(&["train"], &[("RUST_LOG", "info,=bad[")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("ignoring invalid RUST_LOG"));
}

#[test]
fn usage_errors_keep_clap_exit_code() {
    let out = run(&["--bogus"], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}

#[test]
fn help_documents_logging_flags() {
    let out = run(&["--help"], &[]);
    let text = String::from_utf8_lossy(&out.stdout);
    for needle in ["--log-format", "--verbose", "--quiet"] {
        assert!(text.contains(needle), "missing {needle} in help");
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn simulate(extra: &[&str]) -> Output {
    let mut args = vec!["simulate", "--config", EXAMPLE];
    args.extend(extra);
    run(&args, &[])
}

fn assert_failed_cleanly(out: &Output, needle: &str) {
    assert_eq!(out.status.code(), Some(1), "{}", stderr(out));
    assert!(out.stdout.is_empty());
    assert!(stderr(out).contains(needle), "stderr: {}", stderr(out));
}

/// A scenario file in the target directory, from the example with `edit` applied.
fn scenario_file(name: &str, edit: impl FnOnce(String) -> String) -> String {
    let text = std::fs::read_to_string(format!("{ROOT}/{EXAMPLE}")).unwrap();
    let path = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(&path, edit(text)).unwrap();
    path
}

#[test]
fn simulate_prints_a_text_table_and_nothing_else_on_stdout() {
    let out = simulate(&["--steps", "300"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.starts_with("config:      configs/single-intersection.toml\n"));
    for needle in [
        "seed:",
        "steps:       300",
        "fixed_time",
        "throughput:",
        "wait (s)",
        "queue (vehicles)",
        "switches_forced:   0",
    ] {
        assert!(text.contains(needle), "missing {needle} in\n{text}");
    }
    assert!(!text.contains("INFO"));
    // The run time is logged, not printed.
    assert!(stderr(&out).contains("steps_per_s"));
    assert!(!text.contains("steps_per_s"));
}

#[test]
fn simulate_json_parses_and_matches_the_text() {
    let out = simulate(&["--steps", "300", "--output", "json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.ends_with("}\n") && !text.ends_with("\n\n"));
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["config"], EXAMPLE);
    assert_eq!(json["seed"], 0);
    assert_eq!(json["steps"], 300);
    assert_eq!(json["controller"]["kind"], "fixed_time");
    assert_eq!(json["controller"]["cycle_s"], 80.0);
    assert_eq!(json["controller"]["phases"][1]["name"], "ns-through");
    assert_eq!(json["controller"]["phases"][1]["green_s"], 30.0);
    assert_eq!(json["signal"]["switches_forced"], 0);
    assert_eq!(
        json["summary"]["queue_by_lane"].as_array().unwrap().len(),
        6
    );
    assert!(json["summary"]["wait_by_approach"]["north"]["p99_s"].is_number());

    // The same numbers in both formats.
    let table = stdout(&simulate(&["--steps", "300"]));
    let departed = json["summary"]["departed"].as_u64().unwrap();
    assert!(table.contains(&format!("departed:    {departed}\n")));
    let throughput = json["summary"]["throughput_veh_per_h"].as_f64().unwrap();
    assert!(table.contains(&format!("throughput:  {throughput:.1} veh/h")));
}

#[test]
fn simulate_is_deterministic_and_seeded() {
    for format in ["text", "json"] {
        let a = simulate(&["--steps", "300", "--seed", "5", "--output", format]);
        let b = simulate(&["--steps", "300", "--seed", "5", "--output", format]);
        let c = simulate(&["--steps", "300", "--seed", "6", "--output", format]);
        assert_eq!(a.stdout, b.stdout, "{format}");
        assert_ne!(a.stdout, c.stdout, "{format}");
    }
}

#[test]
fn simulate_keeps_stdout_clean_with_json_logs() {
    let out = simulate(&["--steps", "100", "--log-format", "json"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).starts_with("config:"));
    let lines: Vec<_> = stderr(&out).lines().map(str::to_owned).collect();
    assert!(!lines.is_empty());
    for line in lines {
        serde_json::from_str::<serde_json::Value>(&line).unwrap();
    }
}

#[test]
fn simulate_usage_errors_exit_2() {
    for extra in [
        &["--steps", "0"][..],
        &["--seed", "abc"],
        &["--output", "xml"],
    ] {
        let out = simulate(extra);
        assert_eq!(out.status.code(), Some(2), "{extra:?}");
        assert!(out.stdout.is_empty());
    }
    let out = run(&["simulate"], &[]);
    assert_eq!(out.status.code(), Some(2), "--config is required");
}

#[test]
fn simulate_reports_a_missing_file() {
    let out = run(&["simulate", "--config", "configs/nope.toml"], &[]);
    assert_failed_cleanly(&out, "configs/nope.toml");
}

#[test]
fn simulate_reports_an_invalid_scenario() {
    let path = scenario_file("invalid.toml", |t| {
        t.replace("step_s = 1.0", "step_s = -1.0")
    });
    let out = run(&["simulate", "--config", &path], &[]);
    assert_failed_cleanly(&out, "step_s must be finite and greater than 0");
    assert!(stderr(&out).contains("invalid.toml"));
}

#[test]
fn simulate_requires_the_fixed_time_table() {
    let path = scenario_file("no-plan.toml", |t| {
        t.split("\n[fixed_time]").next().unwrap().to_owned()
    });
    let out = run(&["simulate", "--config", &path], &[]);
    assert_failed_cleanly(&out, "[fixed_time]");
    assert!(stderr(&out).contains("requires"));
}
