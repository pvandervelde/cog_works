//! CogWorks trigger event source infrastructure.
//!
//! Implements the [`pipeline::EventSource`] trait with two backends:
//!
//! - [`GitHubWebhookEventSource`] — binds an HTTP server and receives GitHub
//!   webhook payloads directly (or via smee.io in development). Uses the
//!   webhook responder built into `github-bot-sdk` and validates the HMAC-SHA256
//!   signature of every incoming request.
//!
//! - [`QueueEventSource`] — consumes messages from a cloud message queue via
//!   `queue-runtime` (Azure Service Bus today; AWS SQS planned). Each message
//!   body is a JSON-encoded GitHub webhook payload forwarded by an Azure Event
//!   Grid subscription or AWS SNS→SQS bridge. Uses `queue-runtime`'s session
//!   API with the [`pipeline::WorkItemId`] as the session key, ensuring all
//!   events for one work item are processed in order.
//!
//! ## Deployment Scenarios
//!
//! | Scenario | EventSource | Notes |
//! |----------|-------------|-------|
//! | Phase 1 CLI | Single-shot (synthesised in `cli`) | No listener needed |
//! | Dev / Phase 3 webhook | `GitHubWebhookEventSource` + smee.io | |
//! | Production webhook | `GitHubWebhookEventSource` direct | Requires public HTTPS endpoint |
//! | Azure queue | `QueueEventSource` + Azure Service Bus | Managed identity recommended |
//! | AWS queue | `QueueEventSource` + AWS SQS | Planned in `queue-runtime` |
//!
//! ## Architectural Layer
//!
//! **Infrastructure.** Transport details, provider configuration, and message
//! deserialization all live here. The [`pipeline`] crate sees only
//! [`pipeline::EventSource`] and [`pipeline::GitHubEvent`].
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/github-traits.md` §EventSource and
//! `docs/spec/interfaces/infrastructure.md` §listener for the full contract.
//!
//! *This crate is a skeleton. Method bodies are filled in during PR 10.*

use std::time::Duration;

use async_trait::async_trait;
use tracing::instrument;

use pipeline::github::{
    EventSource, EventSourceError, GitHubEvent, QueueEventConfig, WebhookConfig,
};

// ─── Webhook event source ────────────────────────────────────────────────────

/// GitHub webhook-based [`EventSource`] implementation.
///
/// Binds an HTTP server on `config.bind_address` using `github-bot-sdk`'s
/// webhook responder. Every incoming POST is HMAC-SHA256 verified against
/// `config.secret` before being parsed into a [`GitHubEvent`].
///
/// ## Local Development
///
/// Use [smee.io](https://smee.io/) as a proxy: run `smee --url <channel>
/// --port <bind_port>` and set `bind_address` to the local port. No public
/// HTTPS endpoint is needed during development.
///
/// ## Specification
///
/// See `docs/spec/interfaces/github-traits.md` §GitHubWebhookEventSource.
pub struct GitHubWebhookEventSource {
    /// Configuration for the webhook HTTP server.
    #[allow(dead_code)]
    config: WebhookConfig,
    // Internal fields (channel receiver, server handle) filled in during PR 10.
}

impl GitHubWebhookEventSource {
    /// Construct and start the webhook HTTP server.
    ///
    /// # Panics
    ///
    /// Panics (via `todo!()`) until a later change provides the implementation.
    /// Binding to `config.bind_address` and server lifecycle management are
    /// implemented at that point.
    pub fn new(config: WebhookConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl EventSource for GitHubWebhookEventSource {
    /// Await the next webhook event, blocking for at most `timeout`.
    ///
    /// - Parses and HMAC-verifies the raw HTTP payload.
    /// - On signature failure, returns [`EventSourceError::AuthError`].
    /// - On parse failure, returns [`EventSourceError::ParseError`].
    /// - On timeout, returns `Ok(None)`.
    #[instrument(skip(self))]
    async fn next_event(
        &mut self,
        _timeout: Duration,
    ) -> Result<Option<GitHubEvent>, EventSourceError> {
        todo!("GitHubWebhookEventSource::next_event — implemented in PR 10")
    }
}

// ─── Queue event source ──────────────────────────────────────────────────────

/// Cloud-queue-based [`EventSource`] implementation.
///
/// Consumes messages from a cloud message queue via `queue-runtime`.
/// Supported providers:
///
/// | Provider | Status |
/// |----------|--------|
/// | Azure Service Bus | Available |
/// | AWS SQS | Planned in `queue-runtime` |
///
/// Each message body is expected to be a JSON-encoded GitHub webhook payload
/// forwarded by an Azure Event Grid subscription or AWS SNS→SQS bridge.
///
/// ## Session Ordering
///
/// When `config.use_session_ordering` is `true`, the [`pipeline::WorkItemId`]
/// is used as the session key. This ensures all events for a single work item
/// are processed in FIFO order even under concurrent load.
///
/// ## Specification
///
/// See `docs/spec/interfaces/github-traits.md` §QueueEventSource.
pub struct QueueEventSource {
    /// Configuration for the cloud queue connection.
    #[allow(dead_code)]
    config: QueueEventConfig,
    // Internal fields (queue_runtime client) filled in during PR 10.
}

impl QueueEventSource {
    /// Construct a queue consumer from the given configuration.
    ///
    /// Does not perform any I/O at construction time; the connection is
    /// established lazily on the first call to [`EventSource::next_event`].
    pub fn new(config: QueueEventConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl EventSource for QueueEventSource {
    /// Receive the next message from the queue and parse it as a [`GitHubEvent`].
    ///
    /// - Deserialises the message body (JSON GitHub webhook payload) into a
    ///   [`GitHubEvent`].
    /// - On parse failure, returns [`EventSourceError::ParseError`] and
    ///   dead-letters the message after `config.max_retry_attempts` attempts.
    /// - On queue connectivity failure, returns [`EventSourceError::QueueError`].
    /// - On timeout, returns `Ok(None)`.
    #[instrument(skip(self))]
    async fn next_event(
        &mut self,
        _timeout: Duration,
    ) -> Result<Option<GitHubEvent>, EventSourceError> {
        todo!("QueueEventSource::next_event — implemented in PR 10")
    }
}
