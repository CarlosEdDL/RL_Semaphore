//! Shared test helpers: an in-process server on an ephemeral port, a WebSocket client that
//! decodes `ServerMessage`s, a timeout wrapper (R9.7), and the replay helper.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroU64;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rl_semaphore_env::{Episode, FixedTime};
use rl_semaphore_protocol::ServerMessage;
use rl_semaphore_server::{Pace, Server, ServerConfig, ServerError};
use rl_semaphore_sim::Scenario;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

/// The example scenario, also used by the CLI and the protocol tests.
pub const EXAMPLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../configs/single-intersection.toml"
));

/// Every await on the network is wrapped in this (R9.7).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// A parsed copy of the example scenario.
pub fn example() -> Scenario {
    Scenario::from_toml_str(EXAMPLE).unwrap()
}

/// A config for the example, bound to an ephemeral port, with `Pace::Unthrottled` and a small
/// `steps_per_episode` (so tests run in milliseconds and see several episodes quickly). Override
/// fields on the result as needed before starting it.
pub fn config(steps_per_episode: u64) -> ServerConfig {
    let mut config = ServerConfig::new(example(), "127.0.0.1:0".parse().unwrap());
    config.steps_per_episode = NonZeroU64::new(steps_per_episode).unwrap();
    config.pace = Pace::Unthrottled;
    config
}

/// Wraps `fut` with [`DEFAULT_TIMEOUT`], panicking if it does not resolve in time (R9.7).
pub async fn timeout<F: Future>(fut: F) -> F::Output {
    tokio::time::timeout(DEFAULT_TIMEOUT, fut)
        .await
        .expect("operation timed out")
}

/// A server running in the background, on its own bound address.
pub struct Running {
    /// The address it is actually bound to.
    pub addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    handle: JoinHandle<Result<(), ServerError>>,
}

impl Running {
    /// Binds and starts serving `config` in the background, and waits until it is bound.
    pub async fn start(config: ServerConfig) -> Self {
        let server = timeout(Server::bind(config)).await.expect("bind failed");
        let addr = server.local_addr();
        let (tx, rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(server.serve(async move {
            let _ = rx.await;
        }));
        Self {
            addr,
            shutdown: Some(tx),
            handle,
        }
    }

    /// This server's WebSocket URL.
    pub fn ws_url(&self) -> String {
        format!("ws://{}/ws", self.addr)
    }

    /// This server's HTTP URL for `path`.
    pub fn http_url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    /// Signals shutdown and waits, bounded by a timeout, for `serve` to return.
    pub async fn shutdown(mut self) -> Result<(), ServerError> {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        timeout(self.handle).await.expect("serve task panicked")
    }
}

/// A connected WebSocket test client.
pub type WsClient = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Connects to `url`.
pub async fn connect(url: &str) -> WsClient {
    let (stream, _response) = timeout(connect_async(url))
        .await
        .expect("ws connect failed");
    stream
}

/// Reads the next `Text` frame's raw payload, skipping any other frame type (there should be
/// none from this server, but skipping costs nothing).
pub async fn recv_text(client: &mut WsClient) -> String {
    loop {
        let msg = timeout(client.next())
            .await
            .expect("stream ended before a message arrived")
            .expect("websocket read error");
        if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
            return text.as_str().to_owned();
        }
    }
}

/// [`recv_text`], decoded as a `ServerMessage`.
pub async fn recv(client: &mut WsClient) -> ServerMessage {
    ServerMessage::from_json(&recv_text(client).await).expect("decode failed")
}

/// Reads the next frame, unfiltered (used to observe the shutdown close frame).
pub async fn recv_frame(client: &mut WsClient) -> tokio_tungstenite::tungstenite::Message {
    timeout(client.next())
        .await
        .expect("stream ended before a frame arrived")
        .expect("websocket read error")
}

/// Sends a frame the server should ignore (R6.4).
pub async fn send_ignored(client: &mut WsClient, msg: tokio_tungstenite::tungstenite::Message) {
    timeout(client.send(msg)).await.expect("send failed");
}

/// The `(episode, step)` key of a `ServerMessage::Snapshot` or `ServerMessage::Metrics`.
pub fn key(msg: &ServerMessage) -> (u64, u64) {
    match msg {
        ServerMessage::Snapshot(s) => (s.episode, s.step),
        ServerMessage::Metrics(m) => (m.episode, m.step),
        ServerMessage::Hello(_) => panic!("Hello has no key"),
    }
}

/// Sends a minimal HTTP/1.1 request over a fresh connection and returns the status code and
/// body. Sends `Connection: close`, so the server closes the connection once it has responded.
pub async fn http_request(addr: SocketAddr, method: &str, path: &str) -> (u16, String) {
    let mut stream = timeout(TcpStream::connect(addr))
        .await
        .expect("connect failed");
    let request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    timeout(stream.write_all(request.as_bytes()))
        .await
        .expect("write failed");
    let mut buf = Vec::new();
    timeout(stream.read_to_end(&mut buf))
        .await
        .expect("read failed");
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut parts = text.splitn(2, "\r\n\r\n");
    let head = parts.next().unwrap_or_default();
    let body = parts.next().unwrap_or_default().to_owned();
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .expect("status line");
    (status, body)
}

/// Replays episodes of `scenario` from `base_seed`, one `env::Episode` per episode index,
/// stepped forward on demand (never backward) with a `FixedTime` controller.
pub struct Replay {
    scenario: Scenario,
    base_seed: u64,
    episodes: HashMap<u64, (Episode, FixedTime)>,
}

impl Replay {
    /// A replay of `scenario`'s episodes, seeded `base_seed.wrapping_add(episode)`.
    pub fn new(scenario: Scenario, base_seed: u64) -> Self {
        Self {
            scenario,
            base_seed,
            episodes: HashMap::new(),
        }
    }

    /// Steps the replay of `episode` forward to `step` and returns it, or `None` if it is
    /// already past `step`.
    ///
    /// A connection's catch-up pair (R6.2) can send a `Metrics` from an earlier step than the
    /// `Snapshot` just sent before it (R5.6 carries the episode's latest `Metrics` forward when
    /// the current tick has none); that one message cannot be checked against this
    /// forward-only replay, so callers skip it when this returns `None`.
    pub fn advance(&mut self, episode: u64, step: u64) -> Option<&Episode> {
        let scenario = &self.scenario;
        let base_seed = self.base_seed;
        let (ep, controller) = self.episodes.entry(episode).or_insert_with(|| {
            let seed = base_seed.wrapping_add(episode);
            let plan = scenario.fixed_time().unwrap().clone();
            (Episode::new(scenario, seed), FixedTime::new(&plan))
        });
        if ep.sim().step_count() > step {
            return None;
        }
        while ep.sim().step_count() < step {
            ep.step(controller).unwrap();
        }
        Some(ep)
    }
}
