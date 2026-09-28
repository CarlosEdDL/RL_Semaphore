//! The one message type the server sends the browser ([`ServerMessage`]), and the version
//! handshake ([`Hello`]).

use serde::{Deserialize, Serialize};

use crate::PROTOCOL_VERSION;
use crate::layout::Layout;
use crate::metrics::Metrics;
use crate::snapshot::Snapshot;

/// One message sent from the server to the browser, internally tagged by `type`.
///
/// A connection sends exactly one [`Hello`] first, then any sequence of [`Snapshot`] and
/// [`Metrics`] messages, each `Metrics` describing the same or an earlier step than the latest
/// `Snapshot`. This phase only defines that order; 2.2 enforces it. There is no
/// client-to-server message in this phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
// One message is held at a time (never a long-lived collection of them), so the size
// difference between variants does not matter, and R3.1 asks for these exact variant types.
#[allow(clippy::large_enum_variant)]
pub enum ServerMessage {
    /// The handshake, sent once at the start of every connection.
    Hello(Hello),
    /// The dynamic state after a step.
    Snapshot(Snapshot),
    /// A running episode summary.
    Metrics(Metrics),
}

impl ServerMessage {
    /// Encodes the message as compact JSON.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::Encode`] if serialization fails. This cannot happen for a
    /// message built only from finite values (see the crate docs).
    pub fn to_json(&self) -> Result<String, ProtocolError> {
        serde_json::to_string(self).map_err(ProtocolError::Encode)
    }

    /// Decodes a message from JSON text.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::Decode`] if `text` is not valid JSON, has an unknown `type`, is
    /// missing a required field, gives the wrong type for a field (including `null` where a
    /// float is expected), or has trailing non-whitespace content.
    pub fn from_json(text: &str) -> Result<Self, ProtocolError> {
        serde_json::from_str(text).map_err(ProtocolError::Decode)
    }
}

/// The handshake message, sent once at the start of every connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    /// The protocol version this message was built with.
    pub protocol_version: u32,
    /// The intersection's static geometry and signal plan.
    pub layout: Layout,
}

impl Hello {
    /// Builds a `Hello` for `layout`, stamped with [`PROTOCOL_VERSION`].
    #[must_use]
    pub const fn new(layout: Layout) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            layout,
        }
    }

    /// Checks that this message's version matches [`PROTOCOL_VERSION`].
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::VersionMismatch`] if it does not.
    pub const fn check_version(&self) -> Result<(), ProtocolError> {
        if self.protocol_version == PROTOCOL_VERSION {
            Ok(())
        } else {
            Err(ProtocolError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                found: self.protocol_version,
            })
        }
    }
}

/// What can go wrong encoding, decoding or version-checking a [`ServerMessage`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProtocolError {
    /// Serialization to JSON failed.
    #[error("failed to encode message")]
    Encode(#[source] serde_json::Error),
    /// Parsing or deserializing JSON failed.
    #[error("failed to decode message")]
    Decode(#[source] serde_json::Error),
    /// A `Hello`'s `protocol_version` does not match [`PROTOCOL_VERSION`].
    #[error("protocol version mismatch: expected {expected}, found {found}")]
    VersionMismatch {
        /// This crate's version.
        expected: u32,
        /// The version found in the message.
        found: u32,
    },
}
