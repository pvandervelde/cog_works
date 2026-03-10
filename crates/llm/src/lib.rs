//! CogWorks LLM provider infrastructure adapter.
//!
//! Implements the [`pipeline::LlmProvider`] trait for Anthropic's Messages API.
//! Additional providers (e.g. OpenAI) are added as new `impl` blocks in this
//! crate without any changes to the `pipeline` crate.
//!
//! ## Architectural Layer
//!
//! **Infrastructure.** All HTTP transport, request formatting, response parsing,
//! rate-limit header tracking, and exponential back-off live here. The
//! [`pipeline`] crate sees only [`pipeline::LlmProvider`].
//!
//! ## Rate Limiting
//!
//! Anthropic returns rate-limit state in `x-ratelimit-*` response headers.
//! The `AnthropicProvider` tracks these headers and propagates back-off
//! signals as [`pipeline::LlmError::RateLimited`]. Shared rate-limit state
//! is managed by the `LlmGateway` in the `nodes` crate using an
//! `Arc<Mutex<RateLimitState>>` so parallel nodes respect the same limit.
//!
//! ## Multi-Provider Support
//!
//! Adding a provider is additive: implement `pipeline::LlmProvider` for a new
//! struct in this crate. No changes to `pipeline` or `nodes` are required.
//!
//! ## Security
//!
//! The API key is stored in `AnthropicConfig` and MUST NOT be logged or
//! included in error messages. The `Debug` impl redacts the key field.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/infrastructure.md` §llm for the full contract.
//!
//! *Method bodies are added in PR 10.*

use async_trait::async_trait;

use pipeline::{ChatMessage, LlmError, LlmProvider, ModelConfig, OutputSchema, StructuredResponse};

// ─── Anthropic configuration ─────────────────────────────────────────────────

/// Configuration for the Anthropic Messages API provider.
///
/// Constructed from environment variables or a secrets manager by the `cli`
/// composition root. The API key MUST NOT be logged.
///
/// See `docs/spec/interfaces/infrastructure.md` §AnthropicConfig.
pub struct AnthropicConfig {
    /// Anthropic API key.
    ///
    /// **Security**: Never log, print, or include in error messages.
    /// `Debug` is intentionally not derived on this field; the `Debug`
    /// impl for [`AnthropicConfig`] redacts this value.
    ///
    /// Used in PR 10 implementation.
    #[allow(dead_code)]
    api_key: String,
    /// Anthropic API base URL. Defaults to `https://api.anthropic.com`.
    ///
    /// Configurable to allow pointing at a proxy or a testing endpoint.
    pub base_url: String,
}

impl AnthropicConfig {
    /// Creates a new [`AnthropicConfig`].
    ///
    /// # Arguments
    /// - `api_key` — Anthropic API key (never logged).
    /// - `base_url` — API base URL; use `"https://api.anthropic.com"` for
    ///   production.
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
        }
    }
}

/// Redacts the API key in debug output to prevent accidental credential leakage.
impl std::fmt::Debug for AnthropicConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicConfig")
            .field("api_key", &"***")
            .field("base_url", &self.base_url)
            .finish()
    }
}

// ─── AnthropicProvider ───────────────────────────────────────────────────────

/// Anthropic Messages API implementation of [`LlmProvider`].
///
/// Uses `reqwest` for HTTPS requests and `serde_json` for request/response
/// serialisation. Rate-limit headers (`x-ratelimit-requests-remaining`,
/// `x-ratelimit-requests-reset`) are read from each response and propagated
/// as [`LlmError::RateLimited`].
///
/// See `docs/spec/interfaces/infrastructure.md` §AnthropicProvider.
#[derive(Debug)]
pub struct AnthropicProvider {
    /// Provider configuration (base URL; API key is redacted in Debug output).
    /// Used in PR 10 implementation.
    #[allow(dead_code)]
    config: AnthropicConfig,
    /// Shared HTTP client. Reusing a single client allows connection pooling
    /// across calls. Used in PR 10 implementation.
    #[allow(dead_code)]
    client: reqwest::Client,
}

impl AnthropicProvider {
    /// Creates a new [`AnthropicProvider`].
    ///
    /// # Arguments
    /// - `config` — Anthropic API configuration.
    ///
    /// # Panics
    ///
    /// Panics if the internal `reqwest::Client` cannot be built (this should
    /// not happen in practice with the default configuration).
    pub fn new(config: AnthropicConfig) -> Self {
        let client = reqwest::Client::builder()
            .build()
            .expect("failed to build reqwest client for AnthropicProvider");
        Self { config, client }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
        _schema: &OutputSchema,
        _model: &ModelConfig,
    ) -> Result<StructuredResponse, LlmError> {
        todo!("PR 10: implement Anthropic Messages API call")
    }
}
