//! CogWorks Extension API client adapter.
//!
//! Implements the [`pipeline::DomainServiceClient`] trait (and related traits)
//! over the Extension API: a JSON request/response protocol carried on Unix
//! domain sockets (default) or HTTP.
//!
//! ## Architectural Layer
//!
//! **Infrastructure.** Protocol framing, transport selection, handshake,
//! serialisation, and connection back-off all live here. The [`pipeline`] crate
//! sees only [`pipeline::DomainServiceClient`].
//!
//! ## Transport
//!
//! Transport is selected per domain service registration in `.cogworks/services.toml`:
//!
//! - `transport = "unix"` — Unix domain socket (default; file-system permissions
//!   provide access control).
//! - `transport = "http"` — HTTP/1.1 (configurable; authentication mechanism
//!   is to be determined).
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/domain-traits.md` and
//! `docs/spec/interfaces/infrastructure.md` §extension-api for the full contract.
//!
//! *This crate is a skeleton. Method bodies are added in PR 10.*

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use pipeline::{
    ArtifactPath, DependencyGraph, DependencyResult, Diagnostics, DomainServiceClient,
    DomainServiceError, DomainServiceName, FailureProfile, HandshakeResult, HealthStatus,
    InterfaceMap, NormaliseResult, Scenario, SimulationResults, TwinError, TwinHandle,
    TwinProvisioner, TwinSpec, ValidationResult,
};

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
