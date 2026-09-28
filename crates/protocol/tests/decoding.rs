//! Decoding rules (R8.4): unknown fields are ignored, malformed messages are rejected, and
//! `check_version` accepts only the current version.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use rl_semaphore_protocol::{Hello, PROTOCOL_VERSION, ProtocolError, ServerMessage};
use serde_json::json;

#[test]
fn unknown_fields_are_ignored_at_every_level() {
    let fixture = common::hello();
    let mut value = serde_json::to_value(&fixture).expect("encode");
    value["extra_top_level"] = json!("ignored");
    value["layout"]["extra_nested"] = json!(123);
    let text = serde_json::to_string(&value).expect("reencode");

    let decoded = ServerMessage::from_json(&text).expect("decode");
    assert_eq!(decoded, fixture);
}

#[test]
fn unknown_type_is_a_decode_error() {
    let text = r#"{"type":"bogus"}"#;
    assert!(matches!(
        ServerMessage::from_json(text),
        Err(ProtocolError::Decode(_))
    ));
}

#[test]
fn missing_required_field_is_a_decode_error() {
    // `layout` is missing.
    let text = r#"{"type":"hello","protocol_version":1}"#;
    assert!(matches!(
        ServerMessage::from_json(text),
        Err(ProtocolError::Decode(_))
    ));
}

#[test]
fn null_for_a_float_is_a_decode_error() {
    let mut value = serde_json::to_value(common::hello()).expect("encode");
    value["layout"]["step_s"] = serde_json::Value::Null;
    let text = serde_json::to_string(&value).expect("reencode");

    assert!(matches!(
        ServerMessage::from_json(&text),
        Err(ProtocolError::Decode(_))
    ));
}

#[test]
fn trailing_garbage_is_a_decode_error() {
    let mut text = common::hello().to_json().expect("encode");
    text.push_str(" garbage");

    assert!(matches!(
        ServerMessage::from_json(&text),
        Err(ProtocolError::Decode(_))
    ));
}

#[test]
fn check_version_accepts_current_and_rejects_other() {
    let good = Hello::new(common::layout());
    assert!(good.check_version().is_ok());

    let mut bad = good;
    bad.protocol_version = PROTOCOL_VERSION + 1;
    match bad.check_version() {
        Err(ProtocolError::VersionMismatch { expected, found }) => {
            assert_eq!(expected, PROTOCOL_VERSION);
            assert_eq!(found, PROTOCOL_VERSION + 1);
        }
        other => panic!("expected VersionMismatch, got {other:?}"),
    }
}
