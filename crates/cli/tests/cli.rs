//! Runs the built binary and checks exit codes and the shape of its output.

use std::process::{Command, Output};

fn run(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rl-semaphore"));
    cmd.args(args)
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
    let out = run(&["simulate"], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("not implemented yet"));
    assert!(out.stdout.is_empty());
}

#[test]
fn json_flag_emits_json_lines() {
    let out = run(&["--log-format", "json", "simulate"], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert_json_error_lines(&out);
    assert!(out.stdout.is_empty());
}

#[test]
fn json_env_var_emits_json_lines() {
    let out = run(&["simulate"], &[("RL_SEMAPHORE_LOG_FORMAT", "json")]);
    assert_eq!(out.status.code(), Some(1));
    assert_json_error_lines(&out);
    assert!(out.stdout.is_empty());
}

#[test]
fn flag_takes_precedence_over_env_var() {
    let out = run(
        &["--log-format", "pretty", "simulate"],
        &[("RL_SEMAPHORE_LOG_FORMAT", "json")],
    );
    assert!(!stderr(&out).trim_start().starts_with('{'));
}

#[test]
fn verbose_shows_startup_debug_event() {
    let quiet_default = run(&["simulate"], &[]);
    assert!(!stderr(&quiet_default).contains("starting"));

    let verbose = run(&["-v", "simulate"], &[]);
    assert!(stderr(&verbose).contains("starting"));
    assert!(verbose.stdout.is_empty());
}

#[test]
fn quiet_hides_info_but_rust_log_overrides_flags() {
    let quiet = run(&["-qq", "simulate"], &[]);
    // -qq keeps errors, so the failure is still reported.
    assert!(stderr(&quiet).contains("not implemented yet"));

    let off = run(&["-qqq", "simulate"], &[]);
    assert_eq!(off.status.code(), Some(1));
    assert!(stderr(&off).is_empty());

    let rust_log = run(&["-qqq", "simulate"], &[("RUST_LOG", "debug")]);
    assert!(stderr(&rust_log).contains("starting"));
}

#[test]
fn invalid_rust_log_warns_and_falls_back() {
    let out = run(&["simulate"], &[("RUST_LOG", "info,=bad[")]);
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
