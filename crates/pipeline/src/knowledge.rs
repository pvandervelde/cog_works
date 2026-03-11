//! Knowledge and configuration traits.
//!
//! This module defines port traits for the four knowledge/configuration
//! systems that the pipeline business logic depends on:
//!
//! - [`SummaryCache`] — pyramid-style summaries of repository artifacts.
//! - [`PipelineConfigurationLoader`] — loads `.cogworks/pipeline.toml`.
//! - [`ToolProfileStore`] — loads tool and skill availability per node.
//! - [`AdapterSpecLoader`] — loads Extension API adapter specifications.
//!
//! It also defines all supporting data types for these traits, including
//! [`ToolProfile`], [`ToolOverrides`], [`ScopeParameters`], [`ApiSpec`],
//! [`SpecInfo`], [`SummaryLevel`], and [`PyramidSummary`].
//!
//! ## Architectural Layer
//!
//! **Trait definitions (port layer).** Traits live in `pipeline`; infrastructure
//! crates supply the concrete implementations (TOML file readers, in-memory
//! caches, etc.) wired together in `cli`.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/domain-traits.md` §Knowledge & Configuration Traits
//! for the full contract.

use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    graph::{PipelineConfiguration, PipelineGraph},
    identifiers::{
        ArtifactPath, CommitSha, DomainServiceName, NodeId, PipelineName, ProfileName, SkillName,
        ToolName,
    },
    types::{ApiVersion, TokenCount},
};

// ─── Summary cache types ────────────────────────────────────────────────────

/// Granularity of an artifact summary in the pyramid cache.
///
/// The four levels form a pyramid: `OneLine` is the most compact (highest
/// priority when token budget is tight); `Source` is the full raw content
/// (lowest priority). The context assembler selects the finest level that
/// fits within the remaining token budget.
///
/// See `docs/spec/interfaces/domain-traits.md` §SummaryLevel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SummaryLevel {
    /// A single line — function signature or module headline only.
    OneLine = 0,
    /// A short paragraph — key types/exports + the most important constraint.
    Paragraph = 1,
    /// The complete public interface — all signatures with doc comments.
    FullInterface = 2,
    /// The entire source file, unmodified.
    Source = 3,
}

impl std::fmt::Display for SummaryLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OneLine => write!(f, "one-line"),
            Self::Paragraph => write!(f, "paragraph"),
            Self::FullInterface => write!(f, "full-interface"),
            Self::Source => write!(f, "source"),
        }
    }
}

// ---------------------------------------------------------------------------

/// A cached summary of a repository artifact at a specific granularity level.
///
/// The pipeline context assembler retrieves summaries via [`SummaryCache`] and
/// assembles them into a [`ContextPackage`] (defined in PR 6).
///
/// See `docs/spec/interfaces/domain-traits.md` §PyramidSummary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PyramidSummary {
    /// Repo-relative path of the summarised artifact.
    pub path: ArtifactPath,
    /// Granularity level of this summary.
    pub level: SummaryLevel,
    /// The summary text at the requested level.
    pub content: String,
    /// Git commit SHA of the artifact when this summary was generated.
    ///
    /// Used by `SummaryCache::is_stale` to detect stale entries.
    pub commit_sha: CommitSha,
    /// Approximate token count of `content` (used for budget accounting).
    pub token_count: TokenCount,
}

// ─── Summary cache errors ───────────────────────────────────────────────────

/// Errors returned by [`SummaryCache`] operations.
///
/// See `docs/spec/interfaces/domain-traits.md` §CacheError.
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum CacheError {
    /// The cache backend (e.g. GitHub comment store) was not reachable.
    #[error("Summary cache backend unavailable: {message}")]
    Unavailable {
        /// Description of the connectivity failure.
        message: String,
    },

    /// A cache entry could not be serialised or deserialised.
    #[error("Summary cache serialisation error for '{path}': {message}")]
    SerialisationError {
        /// The artifact path that triggered the error.
        path: String,
        /// Description of the serialisation failure.
        message: String,
    },
}

// ─── Summary cache trait ────────────────────────────────────────────────────

/// Cache of pyramid-summary artifacts for context assembly.
///
/// Implementations provide read-through caching of artifact summaries so
/// the context assembler does not need to regenerate them on every pipeline
/// run. The cache is keyed on `(ArtifactPath, SummaryLevel)`.
///
/// ## Staleness
///
/// A cache entry is stale when the artifact has changed since the summary
/// was generated. Callers should call `is_stale` before using a cached
/// summary and `invalidate` when an artifact is known to have changed.
///
/// ## Architectural Layer
///
/// Trait defined in `pipeline`; implemented in GitHub comment adapter or an
/// in-process LRU cache in infrastructure crates.
///
/// See `docs/spec/interfaces/domain-traits.md` §SummaryCache.
#[async_trait]
pub trait SummaryCache: Send + Sync {
    /// Retrieves a cached summary for the given artifact at the given level.
    ///
    /// # Returns
    /// - `Ok(Some(summary))` — cache hit.
    /// - `Ok(None)` — cache miss; caller should generate the summary.
    /// - `Err(_)` — storage backend error.
    ///
    /// # Errors
    /// - [`CacheError::Unavailable`] — backend not reachable.
    /// - [`CacheError::SerialisationError`] — cached entry cannot be parsed.
    async fn get_summary(
        &self,
        path: &ArtifactPath,
        level: SummaryLevel,
    ) -> Result<Option<PyramidSummary>, CacheError>;

    /// Checks whether the cached summary for an artifact is stale.
    ///
    /// A summary is stale when `current_sha` differs from the SHA stored in
    /// the cached [`PyramidSummary`].
    ///
    /// # Arguments
    /// - `path` — the artifact to check.
    /// - `current_sha` — the HEAD commit SHA of the artifact.
    ///
    /// # Returns
    /// - `Ok(true)` — cached entry is stale or absent.
    /// - `Ok(false)` — cached entry is current.
    ///
    /// # Errors
    /// - [`CacheError::Unavailable`] — backend not reachable.
    async fn is_stale(
        &self,
        path: &ArtifactPath,
        current_sha: &CommitSha,
    ) -> Result<bool, CacheError>;

    /// Invalidates all cached summaries for an artifact at all levels.
    ///
    /// # Errors
    /// - [`CacheError::Unavailable`] — backend not reachable.
    async fn invalidate(&self, path: &ArtifactPath) -> Result<(), CacheError>;
}

// ─── Pipeline configuration loader types ───────────────────────────────────

/// Errors returned by [`PipelineConfigurationLoader`] operations.
///
/// See `docs/spec/interfaces/domain-traits.md` §ConfigError.
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum ConfigError {
    /// The `.cogworks/pipeline.toml` file was not found.
    ///
    /// Callers should fall back to the built-in default pipeline via
    /// `get_default_pipeline` rather than treating this as a fatal error.
    #[error("Pipeline configuration not found at '{path}'")]
    NotFound {
        /// Expected path to the configuration file.
        path: String,
    },

    /// The configuration file was found but could not be parsed.
    #[error("Failed to parse pipeline configuration at '{path}': {message}")]
    ParseError {
        /// Path to the configuration file.
        path: String,
        /// Reason parsing failed.
        message: String,
    },

    /// The configuration was parsed but failed semantic validation.
    ///
    /// This typically means a pipeline graph failed `validate_pipeline_graph`.
    #[error("Invalid pipeline configuration: {message}")]
    InvalidConfiguration {
        /// Human-readable description of the configuration violation.
        message: String,
    },
}

// ─── Pipeline configuration loader trait ───────────────────────────────────

/// Loads and provides access to the pipeline graph configuration.
///
/// The configuration is stored in `.cogworks/pipeline.toml` within the
/// repository working directory. If no file exists, the built-in default
/// linear 7-node pipeline is used.
///
/// ## Architectural Layer
///
/// Trait defined in `pipeline`; implemented by a TOML file reader in
/// infrastructure or the `cli` composition root.
///
/// See `docs/spec/interfaces/domain-traits.md` §PipelineConfigurationLoader.
#[async_trait]
pub trait PipelineConfigurationLoader: Send + Sync {
    /// Reads and validates the pipeline configuration from the given directory.
    ///
    /// # Arguments
    /// - `working_dir` — the repository root (where `.cogworks/` lives).
    ///
    /// # Returns
    /// The parsed and validated [`PipelineConfiguration`].
    ///
    /// # Errors
    /// - [`ConfigError::NotFound`] — `.cogworks/pipeline.toml` not present.
    /// - [`ConfigError::ParseError`] — file present but TOML is malformed.
    /// - [`ConfigError::InvalidConfiguration`] — semantically invalid graph.
    async fn load_pipeline_config(
        &self,
        working_dir: &Path,
    ) -> Result<PipelineConfiguration, ConfigError>;

    /// Returns the named pipeline from a loaded configuration.
    ///
    /// This is a pure, synchronous lookup — no I/O.
    ///
    /// # Returns
    /// - `Some(&PipelineGraph)` if the named pipeline exists.
    /// - `None` if not found.
    fn get_named_pipeline<'a>(
        &self,
        config: &'a PipelineConfiguration,
        name: &PipelineName,
    ) -> Option<&'a PipelineGraph>;

    /// Returns the default pipeline from a loaded configuration, or the
    /// built-in default if the configuration contains no default entry.
    ///
    /// The built-in default is the canonical 7-node linear pipeline
    /// (Intake → Architecture → Interface Design → Planning →
    /// Code Generation → Review → Integration).
    fn get_default_pipeline(&self, config: &PipelineConfiguration) -> PipelineGraph;
}

// ─── Tool profile types ──────────────────────────────────────────────────────

/// Scope enforcement parameters for a tool profile.
///
/// Constrains the set of artifacts that a node may read or write and limits
/// the magnitude of changes. Used by `fn validate_tool_scope` (defined in
/// PR 5 security module) to prevent nodes from operating outside their
/// declared scope.
///
/// See `docs/spec/interfaces/domain-traits.md` §ScopeParameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeParameters {
    /// Maximum number of files a single code generation run may modify.
    ///
    /// `None` means unlimited (not recommended for production).
    pub max_file_changes: Option<u32>,

    /// Glob patterns of artifact paths the node is allowed to create or
    /// modify. An empty list means no write access is allowed.
    pub allowed_artifact_patterns: Vec<String>,

    /// Glob patterns of artifact paths the node is prohibited from touching.
    /// Takes precedence over `allowed_artifact_patterns` if both match.
    pub prohibited_artifact_patterns: Vec<String>,

    /// Maximum number of new files a single code generation run may create.
    ///
    /// `0` means no new files may be created.
    pub max_new_files: u32,
}

// ---------------------------------------------------------------------------

/// Tool and skill availability profile for a pipeline node.
///
/// Loaded by [`ToolProfileStore`] and injected into node execution to restrict
/// which tools and skills the LLM and domain service calls may use.
///
/// See `docs/spec/interfaces/domain-traits.md` §ToolProfile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProfile {
    /// Human-readable profile name (matches the TOML key in pipeline config).
    pub name: ProfileName,
    /// The node this profile is scoped to, or `None` for a shared/default
    /// profile.
    pub node_id: Option<NodeId>,
    /// Tools that may be invoked during execution under this profile.
    pub allowed_tools: Vec<ToolName>,
    /// Skills (pre-composed tool sequences) that may be invoked.
    pub allowed_skills: Vec<SkillName>,
    /// Artifact scope constraints enforced during execution.
    pub scope_parameters: ScopeParameters,
}

// ---------------------------------------------------------------------------

/// Per-node tool/skill availability overrides applied on top of a base
/// [`ToolProfile`].
///
/// Used by [`ToolProfileStore::get_node_overrides`] to layer node-specific
/// customisations over the default profile without replacing it entirely.
///
/// See `docs/spec/interfaces/domain-traits.md` §ToolOverrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOverrides {
    /// Tools to add to the base profile's `allowed_tools` for this node.
    pub additional_tools: Vec<ToolName>,
    /// Tools to remove from the base profile's `allowed_tools` for this node.
    pub removed_tools: Vec<ToolName>,
    /// Scope parameter overrides for this node; `None` means use the base
    /// profile's `scope_parameters` unchanged.
    pub scope_overrides: Option<ScopeParameters>,
}

// ─── Tool profile store errors ───────────────────────────────────────────────

/// Errors returned by [`ToolProfileStore`] operations.
///
/// See `docs/spec/interfaces/domain-traits.md` §ProfileError.
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum ProfileError {
    /// The profile directory or file could not be read.
    #[error("Failed to load tool profiles: {message}")]
    LoadFailed {
        /// Reason the load failed.
        message: String,
    },

    /// No profile exists for the requested node.
    ///
    /// Callers should fall back to `get_default_profiles` rather than
    /// treating this as a fatal error.
    #[error("No tool profile found for node '{node_id}'")]
    NotFound {
        /// The node ID that has no profile entry.
        node_id: String,
    },
}

// ─── Tool profile store trait ────────────────────────────────────────────────

/// Provides access to tool and skill availability profiles for pipeline nodes.
///
/// Profiles are declared in `.cogworks/profiles/` (TOML files) or embedded
/// in the pipeline configuration. The infrastructure implementation reads
/// these files and resolves per-node profiles.
///
/// ## Architectural Layer
///
/// Trait defined in `pipeline`; implemented by a TOML file reader in `cli`
/// or a dedicated config crate.
///
/// See `docs/spec/interfaces/domain-traits.md` §ToolProfileStore.
#[async_trait]
pub trait ToolProfileStore: Send + Sync {
    /// Loads all tool profile definitions from the profile store.
    ///
    /// # Returns
    /// All defined profiles. An empty vec means no profiles are configured;
    /// callers should then use hard-coded defaults.
    ///
    /// # Errors
    /// - [`ProfileError::LoadFailed`] — profile files could not be read.
    async fn load_profiles(&self) -> Result<Vec<ToolProfile>, ProfileError>;

    /// Returns the effective tool profile for a specific node.
    ///
    /// The implementation merges base profiles and node-specific overrides.
    ///
    /// # Errors
    /// - [`ProfileError::NotFound`] — no profile configured for this node.
    /// - [`ProfileError::LoadFailed`] — store is not loaded.
    async fn get_node_profile(&self, node_id: &NodeId) -> Result<ToolProfile, ProfileError>;

    /// Returns any node-specific tool overrides layered on top of the base
    /// profile.
    ///
    /// # Returns
    /// - `Ok(Some(overrides))` — overrides exist for this node.
    /// - `Ok(None)` — no node-specific overrides; use base profile as-is.
    ///
    /// # Errors
    /// - [`ProfileError::LoadFailed`] — store is not loaded.
    async fn get_node_overrides(
        &self,
        node_id: &NodeId,
    ) -> Result<Option<ToolOverrides>, ProfileError>;

    /// Returns the default tool profiles for nodes that have no explicit
    /// configuration.
    ///
    /// The returned profiles are ordered by priority (most-restrictive first).
    ///
    /// # Errors
    /// - [`ProfileError::LoadFailed`] — store is not loaded.
    async fn get_default_profiles(&self) -> Result<Vec<ToolProfile>, ProfileError>;
}

// ─── Adapter spec types ──────────────────────────────────────────────────────

/// High-level metadata about an Extension API adapter specification.
///
/// Used by [`AdapterSpecLoader::list_specs`] to enumerate registered services
/// without loading full operation schemas.
///
/// See `docs/spec/interfaces/domain-traits.md` §SpecInfo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecInfo {
    /// Human-readable service name (matches the TOML registration key).
    pub title: String,
    /// API version declared in the spec.
    pub version: ApiVersion,
    /// Short description of the domain service's purpose.
    pub description: String,
    /// The domain service name identifier (matches `DomainServiceRegistration.name`).
    pub service_name: DomainServiceName,
}

// ---------------------------------------------------------------------------

/// Parameter/return schema for a single Extension API operation.
///
/// Part of [`ApiSpec`]; describes one callable operation exposed by a domain
/// service.
///
/// See `docs/spec/interfaces/domain-traits.md` §OperationSpec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationSpec {
    /// The operation name as declared in the Extension API spec
    /// (e.g. `"validate"`, `"normalise"`).
    pub name: String,
    /// Human-readable description of what the operation does.
    pub description: String,
    /// JSON Schema for the operation's input message body.
    pub input_schema: serde_json::Value,
    /// JSON Schema for the operation's output message body.
    pub output_schema: serde_json::Value,
}

// ---------------------------------------------------------------------------

/// Full Extension API adapter specification for a domain service.
///
/// Contains all operations the domain service exposes, with their request
/// and response schemas. Used by infrastructure to validate envelope payloads
/// at the transport boundary.
///
/// See `docs/spec/interfaces/domain-traits.md` §ApiSpec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSpec {
    /// The domain service this spec describes.
    pub service_name: DomainServiceName,
    /// High-level metadata for this spec.
    pub info: SpecInfo,
    /// All operations defined in this spec.
    pub operations: Vec<OperationSpec>,
}

// ─── Adapter spec loader errors ──────────────────────────────────────────────

/// Errors returned by [`AdapterSpecLoader`] operations.
///
/// See `docs/spec/interfaces/domain-traits.md` §SpecError.
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum SpecError {
    /// The spec file could not be read or parsed.
    #[error("Failed to load adapter spec for '{service_name}': {message}")]
    LoadFailed {
        /// The service whose spec could not be loaded.
        service_name: String,
        /// Reason the load failed.
        message: String,
    },

    /// No spec is registered for the requested service.
    #[error("No adapter spec found for service '{service_name}'")]
    NotFound {
        /// The service name that has no registered spec.
        service_name: String,
    },
}

// ─── Adapter spec loader trait ───────────────────────────────────────────────

/// Loads Extension API adapter specifications for domain services.
///
/// Adapter specs describe the wire protocol for each domain service: their
/// operations, request/response schemas, and API version. The infrastructure
/// Extension API client uses them to validate envelopes at the transport
/// boundary.
///
/// ## Architectural Layer
///
/// Trait defined in `pipeline`; implemented in `extension-api` or a dedicated
/// config adapter.
///
/// See `docs/spec/interfaces/domain-traits.md` §AdapterSpecLoader.
#[async_trait]
pub trait AdapterSpecLoader: Send + Sync {
    /// Loads the full [`ApiSpec`] for a named domain service.
    ///
    /// # Errors
    /// - [`SpecError::NotFound`] — no spec registered for this service.
    /// - [`SpecError::LoadFailed`] — spec file unreadable or malformed.
    async fn load_spec(&self, service_name: &DomainServiceName) -> Result<ApiSpec, SpecError>;

    /// Lists metadata for all registered adapter specs.
    ///
    /// # Returns
    /// A `Vec` of [`SpecInfo`] summaries. An empty vec means no specs are
    /// registered — this is unusual and the caller should log a warning.
    ///
    /// # Errors
    /// - [`SpecError::LoadFailed`] — the spec directory could not be read.
    async fn list_specs(&self) -> Result<Vec<SpecInfo>, SpecError>;
}
