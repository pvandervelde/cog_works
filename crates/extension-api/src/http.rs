//! HTTP/1.1 transport for the Extension API protocol.
//!
//! Provides [`HttpTransport`], the internal transport used by
//! [`crate::ExtensionApiClient`] when `TransportKind::Http` is configured.
//!
//! ## Protocol
//!
//! Requests are HTTP POST to `{base_url}/api/v1` with
//! `Content-Type: application/json`. The body is the JSON-serialised
//! [`crate::RequestEnvelope`]. The response body is the JSON-deserialised
//! [`crate::ResponseEnvelope`].
//!
//! ## Authentication
//!
//! Authentication for the HTTP transport is TBD. The open decision is tracked
//! in `docs/adr/ADR-0006-extension-api-http-transport-authentication.md`.
//! Until that ADR is resolved, the transport sends unauthenticated requests.
//! **Do not deploy the HTTP transport on a network boundary without
//! authentication.** Use the Unix socket transport (default) for local
//! domain services; apply a network-layer control (VPN, private subnet,
//! firewall) as a compensating control if HTTP must be used remotely.
//!
//! ## TLS
//!
//! `reqwest` is built with `rustls-tls`; no native TLS dependency.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/infrastructure.md` §HttpTransport.

use crate::{RequestEnvelope, ResponseEnvelope, TransportError};

// ─── HttpTransport ───────────────────────────────────────────────────────────

/// HTTP/1.1 transport for one domain service.
///
/// Internal to the `extension-api` crate; not part of the public API.
/// [`crate::ExtensionApiClient`] constructs this when
/// [`crate::TransportKind::Http`] is configured.
///
/// The underlying `reqwest::Client` is shared via `Arc` within a single
/// `ExtensionApiClient` instance; it is not shared across multiple clients.
///
/// See `docs/spec/interfaces/infrastructure.md` §HttpTransport.
pub(crate) struct HttpTransport {
    /// Base URL for the domain service HTTP API (e.g. `http://localhost:8080`).
    base_url: String,
    /// Shared HTTP client (connection pooling).
    client: reqwest::Client,
    /// Maximum number of retry attempts on connection failure or timeout.
    max_retries: u32,
    /// Delay between retry attempts in milliseconds.
    retry_delay_ms: u64,
}

impl HttpTransport {
    /// Creates a new `HttpTransport`.
    ///
    /// Builds a `reqwest::Client` with `rustls-tls` at construction time.
    /// Returns an error if the client cannot be built.
    ///
    /// # Arguments
    ///
    /// * `base_url` — Base URL of the domain service (no trailing slash).
    /// * `max_retries` — Maximum retry attempts on connection failure or timeout.
    /// * `retry_delay_ms` — Delay between retries (milliseconds).
    ///
    /// # Errors
    ///
    /// Returns [`reqwest::Error`] if the HTTP client cannot be built (e.g.
    /// unusual TLS configuration).
    pub(crate) fn new(
        base_url: impl Into<String>,
        max_retries: u32,
        retry_delay_ms: u64,
    ) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder().build()?;
        Ok(Self {
            base_url: base_url.into(),
            client,
            max_retries,
            retry_delay_ms,
        })
    }

    /// Sends `envelope` to the domain service via HTTP POST and returns the
    /// response envelope.
    ///
    /// ## Endpoint
    ///
    /// `POST {base_url}/api/v1`
    /// `Content-Type: application/json`
    ///
    /// ## Retry Behaviour
    ///
    /// - `TransportError::ConnectionFailed` — retried up to `max_retries` times
    ///   with `retry_delay_ms` delay between attempts.
    /// - `TransportError::Timeout` — retried up to `max_retries` times.
    /// - `TransportError::ResponseParseError` — not retried.
    /// - HTTP 4xx/5xx — mapped to `TransportError::RequestFailed`; not retried.
    ///
    /// See `docs/spec/interfaces/infrastructure.md` §HttpTransport.
    pub(crate) async fn send(
        &self,
        _envelope: &RequestEnvelope,
    ) -> Result<ResponseEnvelope, TransportError> {
        todo!(
            "HttpTransport::send — implemented in full coder phase; \
               see docs/spec/interfaces/infrastructure.md §HttpTransport"
        )
    }
}
