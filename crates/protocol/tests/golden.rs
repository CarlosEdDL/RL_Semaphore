//! Golden wire format: `insta` pins the pretty JSON of one fixture of each message type, and of
//! a `Snapshot` in each signal state. Compact and pretty encodings of the same value must parse
//! to the same `serde_json::Value` (R8.3).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use rl_semaphore_protocol::ServerMessage;

fn assert_golden(name: &str, msg: &ServerMessage) {
    let pretty = serde_json::to_string_pretty(msg).expect("encode pretty");
    let compact = msg.to_json().expect("encode compact");
    let compact_value: serde_json::Value = serde_json::from_str(&compact).expect("parse compact");
    let pretty_value: serde_json::Value = serde_json::from_str(&pretty).expect("parse pretty");
    assert_eq!(
        compact_value, pretty_value,
        "compact and pretty encodings disagree"
    );
    insta::assert_snapshot!(name, pretty);
}

#[test]
fn hello_wire_format() {
    assert_golden("hello", &common::hello());
}

#[test]
fn snapshot_green_wire_format() {
    assert_golden("snapshot_green", &common::snapshot_green());
}

#[test]
fn snapshot_yellow_wire_format() {
    assert_golden("snapshot_yellow", &common::snapshot_yellow());
}

#[test]
fn snapshot_all_red_wire_format() {
    assert_golden("snapshot_all_red", &common::snapshot_all_red());
}

#[test]
fn metrics_wire_format() {
    assert_golden("metrics", &common::metrics());
}
