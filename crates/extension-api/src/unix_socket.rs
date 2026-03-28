//! Unix domain socket transport for the Extension API protocol.
//!
//! Provides [`UnixSocketTransport`], the internal transport used by
//! [`crate::ExtensionApiClient`] when `TransportKind::UnixSocket` is
//! configured.
//!
//! ## Protocol
//!
//! Requests and responses are length-prefixed JSON frames:
//!
//! ```text
//! ┌──────────────────────┬──────────────────────────────────────────┐
//! │  4-byte length (BE)  │  UTF-8 JSON body (RequestEnvelope JSON)  │
//! └──────────────────────┴──────────────────────────────────────────┘
//! ```
//!
//! A new `UnixStream` is opened for each operation call. Connection
//! multiplexing is not used; simplicity is preferred over connection reuse
//! at this stage of development.
//!
//! ## Access Control
//!
//! The Unix socket file's permissions are set by the domain service process.
//! `UnixSocketTransport` connects as a client and does not change permissions.
//! Operators configure the socket path so only the CogWorks process user
//! can write to it.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/infrastructure.md` §UnixSocketTransport.

use std::path::PathBuf;

use crate::{RequestEnvelope, ResponseEnvelope, TransportError};

// ─── UnixSocketTransport ────────────────────────────────────────────────────

/// Unix domain socket transport for one domain service.
///
/// Internal to the `extension-api` crate; not part of the public API.
/// [`crate::ExtensionApiClient`] constructs this when
/// [`crate::TransportKind::UnixSocket`] is configured.
///
/// See `docs/spec/interfaces/infrastructure.md` §UnixSocketTransport.
pub(crate) struct UnixSocketTransport {
    /// Absolute path to the Unix domain socket file.
    path: PathBuf,
    /// Maximum number of connection retry attempts before giving up.
    max_retries: u32,
    /// Delay between retry attempts in milliseconds.
    retry_delay_ms: u64,
}

impl UnixSocketTransport {
    /// Creates a new `UnixSocketTransport`.
    ///
    /// Does not open a connection; the connection is established lazily on the
    /// first call to [`send`](Self::send).
    ///
    /// # Arguments
    ///
    /// * `path` — Absolute path to the Unix domain socket file.
    /// * `max_retries` — Maximum connection retry attempts.
    /// * `retry_delay_ms` — Delay between retries (milliseconds).
    pub(crate) fn new(path: PathBuf, max_retries: u32, retry_delay_ms: u64) -> Self {
        Self {
            path,
            max_retries,
            retry_delay_ms,
        }
    }

    /// Sends `envelope` to the domain service and returns the response.
    ///
    /// ## Retry Behaviour
    ///
    /// - `TransportError::ConnectionFailed` — retried up to `max_retries` times
    ///   with `retry_delay_ms` delay between attempts.
    /// - `TransportError::Timeout` — retried up to `max_retries` times.
    /// - `TransportError::ResponseParseError` — not retried; the response is
    ///   malformed.
    ///
    /// ## Connection Lifecycle
    ///
    /// A new `tokio::net::UnixStream` is established for each call.
    /// The stream is closed when this function returns (success or error).
    ///
    /// See `docs/spec/interfaces/infrastructure.md` §UnixSocketTransport.
    pub(crate) async fn send(
        &self,
        _envelope: &RequestEnvelope,
    ) -> Result<ResponseEnvelope, TransportError> {
        todo!(
            "UnixSocketTransport::send — implemented in full coder phase; \
               see docs/spec/interfaces/infrastructure.md §UnixSocketTransport"
        )
    }
}
