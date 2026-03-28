//! CogWorks Extension API client adapter.
//!
//! Implements the [`pipeline::DomainServiceClient`] and
//! [`pipeline::TwinProvisioner`] traits over the Extension API: a JSON
//! request/response protocol carried on Unix domain sockets (default) or HTTP.
//!
//! ## Module Layout
//!
//! ```text
//! lib.rs          ExtensionApiClient + DomainServiceClient/TwinProvisioner impls
//!                 TransportKind, ServiceTransportConfig, envelope types
//! unix_socket.rs  UnixSocketTransport (internal, Unix socket framing)
//! http.rs         HttpTransport       (internal, HTTP/1.1 POST)
//! ```
//!
//! ## Architectural Layer
//!
//! **Infrastructure.** Protocol framing, transport selection, handshake,
//! serialisation, and connection back-off all live here. The [`pipeline`] crate
//! sees only [`pipeline::DomainServiceClient`] and [`pipeline::TwinProvisioner`].
//!
//! ## Transport Selection
//!
//! Transport is selected per domain service registration in
//! `.cogworks/services.toml`:
//!
//! - `transport = "unix"` — Unix domain socket (default; file-system permissions
//!   provide access control).
//! - `transport = "http"` — HTTP/1.1 (configurable; authentication mechanism
//!   is TBD — do not use on a network boundary without adding authentication).
//!
//! ## JSON Envelope Protocol
//!
//! Every request is a JSON object:
//!
//! ```json
//! { "version": "1.0", "operation": "<op>", "payload": { ... } }
//! ```
//!
//! Every response is:
//!
//! ```json
//! { "version": "1.0", "status": "ok",    "payload": { ... } }
//! { "version": "1.0", "status": "error", "error":   "..." }
//! ```
//!
//! The [`RequestEnvelope`] and [`ResponseEnvelope`] types are defined in this
//! module and used by both transport sub-modules.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/domain-traits.md` and
//! `docs/spec/interfaces/infrastructure.md` §extension-api for the full contract.

// Stub phase: transport types and helpers are unused until method bodies are
// implemented. These attributes are removed when stubs are implemented.
#![allow(dead_code)]

pub mod http;
pub mod unix_socket;

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use pipeline::{
    ArtifactPath, DependencyGraph, DependencyResult, Diagnostics, DomainServiceClient,
    DomainServiceError, DomainServiceName, FailureProfile, HandshakeResult, HealthStatus,
    InterfaceMap, NormaliseResult, Scenario, SimulationResults, TwinError, TwinHandle,
    TwinProvisioner, TwinSpec, ValidationResult,
};

// ─── Internal JSON envelope types ────────────────────────────────────────────

/// JSON request envelope sent to a domain service.
///
/// Both [`crate::unix_socket::UnixSocketTransport`] and
/// [`crate::http::HttpTransport`] serialise this to JSON before sending.
///
/// See `docs/spec/interfaces/infrastructure.md` §JSON Envelope Protocol.
#[derive(Debug, Serialize)]
pub(crate) struct RequestEnvelope {
    /// Protocol version; always `"1.0"` at present.
    pub(crate) version: String,
    /// Operation name (e.g. `"validate"`, `"health_check"`).
    pub(crate) operation: String,
    /// Operation-specific payload.
    pub(crate) payload: serde_json::Value,
}

impl RequestEnvelope {
    /// Creates a new envelope for the given `operation` and `payload`.
    pub(crate) fn new(operation: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            version: "1.0".to_string(),
            operation: operation.into(),
            payload,
        }
    }
}

/// JSON response envelope received from a domain service.
///
/// Both transport implementations deserialise the raw response body into this
/// type before returning to the caller.
///
/// See `docs/spec/interfaces/infrastructure.md` §JSON Envelope Protocol.
#[derive(Debug, Deserialize)]
pub(crate) struct ResponseEnvelope {
    /// Protocol version; should be `"1.0"`.
    #[allow(dead_code)]
    pub(crate) version: String,
    /// `"ok"` on success; `"error"` on failure.
    pub(crate) status: String,
    /// Response payload present when `status == "ok"`.
    pub(crate) payload: Option<serde_json::Value>,
    /// Error message present when `status == "error"`.
    pub(crate) error: Option<String>,
}

// ─── Internal transport error ─────────────────────────────────────────────────

/// Errors that can occur at the transport layer, before domain-level mapping.
///
/// These are mapped to [`pipeline::DomainServiceError`] by
/// [`ExtensionApiClient`] before returning to callers.
///
/// See `docs/spec/interfaces/infrastructure.md` §transport error mapping.
#[derive(Debug)]
pub(crate) enum TransportError {
    /// The connection to the domain service could not be established.
    ///
    /// Retried up to `ServiceTransportConfig::max_retries` times.
    ConnectionFailed { message: String },
    /// The request was sent but no response was received within the deadline.
    ///
    /// Retried up to `ServiceTransportConfig::max_retries` times.
    Timeout,
    /// The response body could not be deserialised.
    ///
    /// Not retried; indicates a protocol mismatch.
    ResponseParseError { raw: String },
    /// The HTTP transport received a non-2xx response.
    ///
    /// Not retried.
    RequestFailed { status: u16, body: String },
}

// ─── Transport configuration ─────────────────────────────────────────────────

/// Transport mechanism used to communicate with a domain service.
///
/// Configured per domain service registration in `.cogworks/services.toml`.
/// See `docs/spec/interfaces/domain-traits.md` §Extension API Transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportKind {
    /// Unix domain socket transport (default).
    ///
    /// File-system permissions control access. Preferred for local domain
    /// services running on the same machine as CogWorks.
    UnixSocket {
        /// Absolute path to the Unix domain socket file.
        path: PathBuf,
    },

    /// HTTP/1.1 transport.
    ///
    /// Used when the domain service runs on a remote host or when Unix sockets
    /// are unavailable. Authentication mechanism is TBD; see
    /// `docs/adr/` for the relevant decision record.
    Http {
        /// Base URL for the domain service HTTP API
        /// (e.g. `http://localhost:8080`).
        base_url: String,
    },
}

// ---------------------------------------------------------------------------

/// Configuration for a single domain service's transport layer.
///
/// One `ServiceTransportConfig` per registered domain service. Constructed
/// from `.cogworks/services.toml` by the `cli` composition root.
///
/// See `docs/spec/interfaces/domain-traits.md` §ServiceTransportConfig.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceTransportConfig {
    /// The domain service this config applies to.
    pub service_name: DomainServiceName,
    /// Transport mechanism to use.
    pub transport: TransportKind,
    /// Maximum number of connection retry attempts before returning
    /// [`DomainServiceError::ServiceUnavailable`].
    pub max_retries: u32,
    /// Delay between connection retry attempts (milliseconds).
    pub retry_delay_ms: u64,
}

// ─── ExtensionApiClient ──────────────────────────────────────────────────────

/// Extension API client that implements [`DomainServiceClient`].
///
/// Each instance is scoped to a single domain service registration. The
/// `cli` composition root creates one `ExtensionApiClient` per registered
/// domain service and injects them as `Arc<dyn DomainServiceClient>` trait
/// objects.
///
/// ## Protocol
///
/// All requests and responses are JSON envelopes. The envelope format is:
///
/// ```json
/// {
///   "version": "1.0",
///   "operation": "<operation-name>",
///   "payload": { ... }
/// }
/// ```
///
/// The response envelope adds a `"status"` field (`"ok"` or `"error"`) and
/// optionally an `"error"` field on failure.
///
/// ## Method Bodies
///
/// All method bodies are `todo!()`. See PR 10 for the full implementation.
///
/// See `docs/spec/interfaces/infrastructure.md` §extension-api.
pub struct ExtensionApiClient {
    /// Transport and retry configuration for this domain service.
    config: ServiceTransportConfig,
}

impl ExtensionApiClient {
    /// Creates a new [`ExtensionApiClient`] with the given transport config.
    ///
    /// The client is not connected until the first API call is made.
    /// Connection failures surface as [`DomainServiceError::ConnectionFailed`].
    pub fn new(config: ServiceTransportConfig) -> Self {
        Self { config }
    }

    /// Returns the transport configuration for this client.
    pub fn config(&self) -> &ServiceTransportConfig {
        &self.config
    }
}

#[async_trait]
impl DomainServiceClient for ExtensionApiClient {
    async fn handshake(&self) -> Result<HandshakeResult, DomainServiceError> {
        todo!("PR 10: implement Extension API handshake")
    }

    async fn validate(
        &self,
        _artifacts: &[ArtifactPath],
    ) -> Result<ValidationResult, DomainServiceError> {
        todo!("PR 10: implement Extension API validate")
    }

    async fn normalise(
        &self,
        _artifacts: &[ArtifactPath],
    ) -> Result<NormaliseResult, DomainServiceError> {
        todo!("PR 10: implement Extension API normalise")
    }

    async fn review_rules(
        &self,
        _artifacts: &[ArtifactPath],
    ) -> Result<Diagnostics, DomainServiceError> {
        todo!("PR 10: implement Extension API review_rules")
    }

    async fn simulate(
        &self,
        _spec: &TwinSpec,
        _scenarios: &[Scenario],
    ) -> Result<SimulationResults, DomainServiceError> {
        todo!("PR 10: implement Extension API simulate")
    }

    async fn validate_deps(
        &self,
        _artifacts: &[ArtifactPath],
    ) -> Result<DependencyResult, DomainServiceError> {
        todo!("PR 10: implement Extension API validate_deps")
    }

    async fn extract_interfaces(
        &self,
        _artifacts: &[ArtifactPath],
    ) -> Result<InterfaceMap, DomainServiceError> {
        todo!("PR 10: implement Extension API extract_interfaces")
    }

    async fn dependency_graph(
        &self,
        _artifacts: &[ArtifactPath],
    ) -> Result<DependencyGraph, DomainServiceError> {
        todo!("PR 10: implement Extension API dependency_graph")
    }

    async fn health_check(&self) -> Result<HealthStatus, DomainServiceError> {
        todo!("PR 10: implement Extension API health_check")
    }
}

#[async_trait]
impl TwinProvisioner for ExtensionApiClient {
    async fn start_twin(&self, _spec: &TwinSpec) -> Result<TwinHandle, TwinError> {
        todo!("implement Extension API start_twin")
    }

    async fn stop_twin(&self, _handle: &TwinHandle) -> Result<(), TwinError> {
        todo!("implement Extension API stop_twin")
    }

    async fn configure_failure_injection(
        &self,
        _handle: &TwinHandle,
        _profile: &FailureProfile,
    ) -> Result<(), TwinError> {
        todo!("implement Extension API configure_failure_injection")
    }

    async fn reset_twin_state(&self, _handle: &TwinHandle) -> Result<(), TwinError> {
        todo!("implement Extension API reset_twin_state")
    }
}
