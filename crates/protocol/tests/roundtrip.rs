//! Every fixture round-trips through JSON to an equal value (R8.1).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use rl_semaphore_protocol::ServerMessage;

#[test]
fn every_fixture_round_trips() {
    for fixture in common::all_fixtures() {
        let text = fixture.to_json().expect("encode");
        let decoded = ServerMessage::from_json(&text).expect("decode");
        assert_eq!(decoded, fixture, "round trip changed the value: {text}");
    }
}
