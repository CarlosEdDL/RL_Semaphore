//! The output of `simulate` on the example scenario is pinned by snapshots.

use std::process::Command;

fn simulate(output: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rl-semaphore"))
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .env_remove("RUST_LOG")
        .args([
            "simulate",
            "--config",
            "configs/single-intersection.toml",
            "--seed",
            "0",
            "--steps",
            "3600",
            "--output",
            output,
        ])
        .output()
        .unwrap_or_else(|e| panic!("failed to run rl-semaphore: {e}"));
    assert_eq!(out.status.code(), Some(0));
    String::from_utf8(out.stdout).unwrap_or_else(|e| panic!("stdout is not UTF-8: {e}"))
}

#[test]
fn text_output_is_pinned() {
    insta::assert_snapshot!(simulate("text"));
}

#[test]
fn json_output_is_pinned() {
    insta::assert_snapshot!(simulate("json"));
}
