//! HTTP integration tests (R9.6): `/healthz`, 404s on unknown paths, and 405s on the wrong
//! method.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::Running;

#[tokio::test]
async fn healthz_returns_200_with_the_expected_fields() {
    let running = Running::start(common::config(1000)).await;

    let (status, body) = common::http_request(running.addr, "GET", "/healthz").await;
    assert_eq!(status, 200);
    let value: serde_json::Value = serde_json::from_str(&body).expect("valid json body");
    assert_eq!(value["status"], "ok");
    assert!(value["version"].is_string());
    assert_eq!(
        value["protocol_version"],
        rl_semaphore_protocol::PROTOCOL_VERSION
    );
    assert!(value["episode"].is_u64());
    assert!(value["step"].is_u64());
    assert!(value["clients"].is_u64());

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn healthz_episode_and_step_move_forward() {
    let running = Running::start(common::config(1000)).await;

    let (_, first) = common::http_request(running.addr, "GET", "/healthz").await;
    let first: serde_json::Value = serde_json::from_str(&first).expect("valid json body");
    let first_key = (
        first["episode"].as_u64().expect("episode"),
        first["step"].as_u64().expect("step"),
    );

    tokio::time::sleep(Duration::from_millis(50)).await;

    let (_, second) = common::http_request(running.addr, "GET", "/healthz").await;
    let second: serde_json::Value = serde_json::from_str(&second).expect("valid json body");
    let second_key = (
        second["episode"].as_u64().expect("episode"),
        second["step"].as_u64().expect("step"),
    );

    assert!(
        second_key > first_key,
        "expected the key to move forward: {first_key:?} -> {second_key:?}"
    );

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn healthz_clients_reflects_open_websocket_connections() {
    let running = Running::start(common::config(1000)).await;

    let (_, body) = common::http_request(running.addr, "GET", "/healthz").await;
    let value: serde_json::Value = serde_json::from_str(&body).expect("valid json body");
    assert_eq!(value["clients"], 0);

    let mut client = common::connect(&running.ws_url()).await;
    common::recv(&mut client).await; // Hello

    let (_, body) = common::http_request(running.addr, "GET", "/healthz").await;
    let value: serde_json::Value = serde_json::from_str(&body).expect("valid json body");
    assert_eq!(value["clients"], 1);

    drop(client);
    // The server notices the drop asynchronously; poll briefly for the count to fall back to 0.
    let mut clients_after_close = 1;
    for _ in 0..50 {
        let (_, body) = common::http_request(running.addr, "GET", "/healthz").await;
        let value: serde_json::Value = serde_json::from_str(&body).expect("valid json body");
        clients_after_close = value["clients"].as_u64().expect("clients");
        if clients_after_close == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(clients_after_close, 0);

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn plain_get_ws_is_rejected_with_a_4xx() {
    let running = Running::start(common::config(1000)).await;

    let (status, _) = common::http_request(running.addr, "GET", "/ws").await;
    assert!(
        (400..500).contains(&status),
        "expected a 4xx status, got {status}"
    );

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn unknown_path_is_404() {
    let running = Running::start(common::config(1000)).await;

    let (status, _) = common::http_request(running.addr, "GET", "/nope").await;
    assert_eq!(status, 404);

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn post_healthz_is_405() {
    let running = Running::start(common::config(1000)).await;

    let (status, _) = common::http_request(running.addr, "POST", "/healthz").await;
    assert_eq!(status, 405);

    running.shutdown().await.expect("serve should return Ok");
}

#[tokio::test]
async fn post_ws_is_405() {
    let running = Running::start(common::config(1000)).await;

    let (status, _) = common::http_request(running.addr, "POST", "/ws").await;
    assert_eq!(status, 405);

    running.shutdown().await.expect("serve should return Ok");
}
