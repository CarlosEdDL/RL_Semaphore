//! [`ServerError`], the crate's single error type.

use std::net::SocketAddr;

use rl_semaphore_env::EnvError;
use rl_semaphore_protocol::ProtocolError;

/// Why building or running the server failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ServerError {
    /// [`crate::ServerConfig::scenario`] has no `[fixed_time]` table.
    #[error("scenario has no fixed-time plan")]
    MissingFixedTimePlan,
    /// A [`crate::Pace::RealTime`] speed is not finite or not greater than 0.
    #[error("invalid pace speed {speed} (must be finite and greater than 0)")]
    InvalidSpeed {
        /// The rejected speed.
        speed: f64,
    },
    /// The listener could not bind `addr`.
    #[error("cannot bind {addr}")]
    Bind {
        /// The address that could not be bound.
        addr: SocketAddr,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Serving connections failed.
    #[error("cannot serve connections")]
    Serve(#[source] std::io::Error),
    /// The simulation thread hit an error running the episode.
    #[error("the simulation failed")]
    Sim(#[source] EnvError),
    /// The simulation thread could not encode a message.
    #[error("cannot encode a message")]
    Encode(#[source] ProtocolError),
    /// The simulation thread panicked.
    #[error("the simulation thread panicked")]
    SimPanicked,
}
