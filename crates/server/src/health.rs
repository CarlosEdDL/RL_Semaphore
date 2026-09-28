//! `GET /healthz` (R7.1): a small JSON status.

use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::app::AppState;

/// The body of `GET /healthz`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Health {
    /// Always `"ok"`: the endpoint itself does not report a degraded state.
    status: &'static str,
    /// The server crate's `CARGO_PKG_VERSION`.
    version: &'static str,
    /// [`rl_semaphore_protocol::PROTOCOL_VERSION`].
    protocol_version: u32,
    /// The episode of the latest tick.
    episode: u64,
    /// The step of the latest tick.
    step: u64,
    /// The number of open WebSocket connections (R6.7).
    clients: usize,
}

/// Answers `GET /healthz` with the current status.
pub(crate) async fn handler(State(state): State<AppState>) -> Json<Health> {
    let (episode, step) = {
        let guard = state
            .latest
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.tick.key()
    };
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        protocol_version: rl_semaphore_protocol::PROTOCOL_VERSION,
        episode,
        step,
        clients: state.clients.load(Ordering::SeqCst),
    })
}
