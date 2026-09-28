//! `GET /ws` integration tests (R9.5): handshake, message order across episodes, replay
//! agreement, simultaneous clients, catch-up, lag, ignored client frames, and shutdown.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::time::Duration;

use rl_semaphore_protocol::{Layout, Metrics, ServerMessage, Snapshot};
use tokio_tungstenite::tungstenite::Message as WsMessage;

use common::Running;

#[tokio::test]
async fn handshake_then_current_state() {
    let running = Running::start(common::config(50)).await;
    let mut client = common::connect(&running.ws_url()).await;

    let hello = match common::recv(&mut client).await {
        ServerMessage::Hello(hello) => hello,
        other => panic!("expected the first message to be a Hello, got {other:?}"),
    };
    hello
        .check_version()
        .expect("protocol version should match");
    assert_eq!(hello.layout, Layout::from_scenario(&common::example()));

    match common::recv(&mut client).await {
        ServerMessage::Snapshot(_) => {}
        other => panic!("expected the second message to be a Snapshot, got {other:?}"),
    }
    match common::recv(&mut client).await {
        ServerMessage::Metrics(_) => {}
        other => panic!("expected the third message to be Metrics, got {other:?}"),
    }

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn ordering_holds_across_several_episodes() {
    let steps_per_episode = 50u64;
    let base_seed = 7u64;
    let mut config = common::config(steps_per_episode);
    config.seed = base_seed;
    // This test asserts a client-observed invariant ("a new episode starts at step 0") that
    // only holds without lag; an unthrottled sim can lag a fast-reading client right past a
    // rollover tick. A bounded real-time pace keeps this test lag-free (see plan.md's note on
    // unthrottled tests) while still finishing in well under a second.
    config.pace = rl_semaphore_server::Pace::RealTime { speed: 200.0 };
    let running = Running::start(config).await;
    let mut client = common::connect(&running.ws_url()).await;
    common::recv(&mut client).await; // Hello

    let mut last_snapshot_key: Option<(u64, u64)> = None;
    let mut episodes_seen = std::collections::BTreeSet::new();

    let total = (steps_per_episode + 1) * 4;
    for _ in 0..total {
        match common::recv(&mut client).await {
            ServerMessage::Snapshot(s) => {
                let this_key = (s.episode, s.step);
                assert!(
                    s.step <= steps_per_episode,
                    "step must not exceed steps_per_episode"
                );
                assert_eq!(s.seed, base_seed.wrapping_add(s.episode));
                if let Some(prev) = last_snapshot_key {
                    assert!(
                        this_key > prev,
                        "snapshot keys must strictly increase: {prev:?} -> {this_key:?}"
                    );
                    if this_key.0 > prev.0 {
                        assert_eq!(s.step, 0, "a new episode must start at step 0");
                    }
                }
                episodes_seen.insert(s.episode);
                last_snapshot_key = Some(this_key);
            }
            ServerMessage::Metrics(m) => {
                let this_key = (m.episode, m.step);
                let latest = last_snapshot_key.expect("a Metrics must follow some Snapshot");
                assert!(
                    this_key <= latest,
                    "metrics key {this_key:?} must be at most the latest snapshot key {latest:?}"
                );
                assert_eq!(m.seed, base_seed.wrapping_add(m.episode));
            }
            ServerMessage::Hello(_) => panic!("unexpected extra Hello"),
        }
    }
    assert!(
        episodes_seen.len() >= 3,
        "expected at least three distinct episodes, saw {episodes_seen:?}"
    );

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn snapshots_and_metrics_match_an_independent_replay() {
    let steps_per_episode = 40u64;
    let base_seed = 3u64;
    let mut config = common::config(steps_per_episode);
    config.seed = base_seed;
    let running = Running::start(config).await;
    let mut client = common::connect(&running.ws_url()).await;
    common::recv(&mut client).await; // Hello

    let mut replay = common::Replay::new(common::example(), base_seed);
    let total = (steps_per_episode + 1) * 3;
    for _ in 0..total {
        match common::recv(&mut client).await {
            ServerMessage::Snapshot(mut s) => {
                let (episode, step) = (s.episode, s.step);
                s.episode = 0;
                s.seed = 0;
                // A Snapshot's own step never goes backward relative to its episode's replay.
                let ep = replay
                    .advance(episode, step)
                    .expect("snapshot steps never go backward");
                assert_eq!(
                    s,
                    Snapshot::from_sim(ep.sim()),
                    "episode={episode} step={step}"
                );
            }
            ServerMessage::Metrics(mut m) => {
                let (episode, step) = (m.episode, m.step);
                m.episode = 0;
                m.seed = 0;
                // The catch-up pair may carry a `Metrics` from an earlier step than the
                // `Snapshot` just sent before it (R5.6); skip verifying that one message.
                if let Some(ep) = replay.advance(episode, step) {
                    let expected = Metrics::from_sim(&ep.summary(), ep.sim());
                    assert_eq!(m, expected, "episode={episode} step={step}");
                }
            }
            ServerMessage::Hello(_) => panic!("unexpected extra Hello"),
        }
    }

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn two_simultaneous_clients_see_byte_identical_ticks() {
    let mut config = common::config(30);
    // A bounded pace keeps ticks arriving at a rate both connections can actually keep up
    // with, so their received keys substantially overlap (an unthrottled sim can outpace two
    // concurrently-read connections enough that each lags to a different, barely-overlapping
    // point).
    config.pace = rl_semaphore_server::Pace::RealTime { speed: 500.0 };
    let running = Running::start(config).await;
    let mut a = common::connect(&running.ws_url()).await;
    let mut b = common::connect(&running.ws_url()).await;
    common::recv(&mut a).await; // Hello
    common::recv(&mut b).await; // Hello

    async fn collect(client: &mut common::WsClient, n: usize) -> HashMap<(u64, u64), String> {
        let mut map = HashMap::new();
        for _ in 0..n {
            let text = common::recv_text(client).await;
            let msg = ServerMessage::from_json(&text).expect("decode failed");
            map.insert(common::key(&msg), text);
        }
        map
    }

    // Read both concurrently: reading one to completion before starting the other would let
    // the second fall arbitrarily far behind the (unthrottled) broadcast in the meantime.
    let (map_a, map_b) = tokio::join!(collect(&mut a, 100), collect(&mut b, 100));

    let mut common_keys = 0;
    for (key, text_a) in &map_a {
        if let Some(text_b) = map_b.get(key) {
            assert_eq!(
                text_a, text_b,
                "tick {key:?} must be byte-identical for both clients"
            );
            common_keys += 1;
        }
    }
    assert!(
        common_keys >= 20,
        "expected substantial overlap between the two clients' ticks, got {common_keys}"
    );

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn a_late_client_catches_up_past_the_start() {
    let running = Running::start(common::config(1000)).await;
    // Let the (unthrottled) sim advance well past step 0 before connecting.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = common::connect(&running.ws_url()).await;
    common::recv(&mut client).await; // Hello
    match common::recv(&mut client).await {
        ServerMessage::Snapshot(s) => {
            assert!(
                (s.episode, s.step) > (0, 0),
                "expected the late client's first snapshot to be past the start, got episode={} step={}",
                s.episode,
                s.step
            );
        }
        other => panic!("expected a Snapshot, got {other:?}"),
    }

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn a_slow_client_skips_lag_but_stays_connected_and_ordered() {
    let mut config = common::config(2000);
    config.broadcast_capacity = NonZeroUsize::new(4).expect("4 is nonzero");
    // A bounded pace (rather than unthrottled) guarantees the client can catch back up after
    // its stall instead of racing a CPU-bound producer that could keep it lagged forever.
    config.pace = rl_semaphore_server::Pace::RealTime { speed: 500.0 };
    let running = Running::start(config).await;
    let mut client = common::connect(&running.ws_url()).await;
    common::recv(&mut client).await; // Hello
    common::recv(&mut client).await; // initial Snapshot
    common::recv(&mut client).await; // initial Metrics

    // Stop reading for a while: with an unthrottled sim and a capacity of 4, this laps the
    // broadcast buffer many times over.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut last_key: Option<(u64, u64)> = None;
    for _ in 0..30 {
        match common::recv(&mut client).await {
            ServerMessage::Snapshot(s) => {
                let this_key = (s.episode, s.step);
                if let Some(prev) = last_key {
                    assert!(
                        this_key > prev,
                        "keys must still strictly increase after lag"
                    );
                }
                last_key = Some(this_key);
            }
            ServerMessage::Metrics(m) => {
                let this_key = (m.episode, m.step);
                let latest = last_key.expect("a Metrics must follow some Snapshot");
                assert!(this_key <= latest);
            }
            ServerMessage::Hello(_) => panic!("unexpected extra Hello"),
        }
    }

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn client_frames_are_ignored_and_the_stream_continues() {
    let running = Running::start(common::config(50)).await;
    let mut client = common::connect(&running.ws_url()).await;
    common::recv(&mut client).await; // Hello
    common::recv(&mut client).await; // initial Snapshot
    common::recv(&mut client).await; // initial Metrics

    common::send_ignored(&mut client, WsMessage::text("hello server")).await;
    common::send_ignored(&mut client, WsMessage::binary(vec![1, 2, 3])).await;

    for _ in 0..5 {
        common::recv(&mut client).await;
    }

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn shutdown_sends_close_1001_and_serve_returns() {
    let running = Running::start(common::config(500)).await;
    let mut a = common::connect(&running.ws_url()).await;
    let mut b = common::connect(&running.ws_url()).await;
    common::recv(&mut a).await; // Hello
    common::recv(&mut b).await; // Hello

    running
        .shutdown()
        .await
        .expect("serve should return Ok within a bounded time");

    for client in [&mut a, &mut b] {
        loop {
            match common::recv_frame(client).await {
                WsMessage::Close(Some(frame)) => {
                    assert_eq!(
                        u16::from(frame.code),
                        1001,
                        "expected close code 1001 (going away)"
                    );
                    break;
                }
                WsMessage::Close(None) => panic!("expected a close frame with code 1001, got none"),
                _ => {} // a trailing Snapshot/Metrics that raced the close is fine to skip
            }
        }
    }
}
