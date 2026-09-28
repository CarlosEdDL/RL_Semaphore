//! `GET /ws` (R6): the handshake, catch-up, live stream, lag skipping, client frames and
//! shutdown close frame.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use tokio::sync::broadcast::error::RecvError;

use crate::app::AppState;

/// Upgrades the connection and hands it to [`handle_socket`].
pub(crate) async fn handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Sends `text` on `socket`. Returns `false` on a write error, which ends the connection
/// without affecting any other (R6.4, R6.5).
async fn send(socket: &mut WebSocket, text: Utf8Bytes) -> bool {
    socket.send(Message::Text(text)).await.is_ok()
}

/// Drives one connection: the handshake and catch-up of R6.2, then the live stream, lag
/// skipping (R6.3), ignoring client frames (R6.4), and the shutdown close frame (R6.6).
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let AppState {
        latest,
        tx,
        hello,
        clients,
        mut shutdown,
        // Held for the lifetime of this future: `Server::serve` waits for every clone of this
        // sender to drop before it returns, which is how it knows every connection's close
        // frame has actually been sent (R4.6), since axum's own graceful shutdown does not
        // reliably wait for an upgraded connection.
        task_tx: _task_tx,
    } = state;

    // R6.2 step 1: subscribe before reading the latest state, so no tick published after it is
    // missed.
    let mut rx = tx.subscribe();
    // R6.2 step 2: the latest state, with key `latest_key`.
    let (latest_key, snapshot0, metrics0) = {
        let guard = latest
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            guard.tick.key(),
            guard.tick.snapshot.clone(),
            guard.metrics.clone(),
        )
    };

    // R6.2 steps 3-4: `Hello`, then the latest `Snapshot` and `Metrics`.
    if !send(&mut socket, hello).await {
        return;
    }
    if !send(&mut socket, snapshot0).await {
        return;
    }
    if !send(&mut socket, metrics0).await {
        return;
    }

    if *shutdown.borrow() {
        let _ = send_close(&mut socket).await;
        return;
    }

    let count = clients.fetch_add(1, Ordering::SeqCst) + 1;
    tracing::info!(clients = count, "client connected");
    let _guard = ConnectionGuard {
        clients: clients.clone(),
    };

    // R6.2 step 5: forward every later tick, dropping any with a key of `latest_key` or less.
    loop {
        tokio::select! {
            tick = rx.recv() => match tick {
                Ok(tick) => {
                    if tick.key() > latest_key {
                        if !send(&mut socket, tick.snapshot).await {
                            break;
                        }
                        if let Some(metrics) = tick.metrics
                            && !send(&mut socket, metrics).await
                        {
                            break;
                        }
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    tracing::debug!(lagged = n, "connection lagged behind the broadcast");
                }
                Err(RecvError::Closed) => break,
            },
            msg = socket.recv() => match msg {
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {
                    tracing::debug!("ignoring a client frame");
                }
            },
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    let _ = send_close(&mut socket).await;
                    break;
                }
            }
        }
    }
}

/// Sends the shutdown close frame, code 1001 ("going away", R4.6, R6.6).
async fn send_close(socket: &mut WebSocket) -> Result<(), axum::Error> {
    socket
        .send(Message::Close(Some(CloseFrame {
            code: 1001,
            reason: Utf8Bytes::from_static("going away"),
        })))
        .await
}

/// Decrements the open-connection count and logs on every exit path (R6.7).
struct ConnectionGuard {
    clients: Arc<AtomicUsize>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let count = self.clients.fetch_sub(1, Ordering::SeqCst) - 1;
        tracing::info!(clients = count, "client disconnected");
    }
}
