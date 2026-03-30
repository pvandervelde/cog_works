//! Cloud queue-based event source.
//!
//! Provides [`QueueEventSource`], which consumes GitHub webhook payloads
//! forwarded to a cloud message queue via [`queue_runtime`] and delivers
//! parsed [`pipeline::GitHubEvent`] values to the pipeline event loop.
//!
//! ## Supported Providers
//!
//! | Provider | Status |
//! |----------|--------|
//! | Azure Service Bus | Available in `queue-runtime` |
//! | AWS SQS | Planned in `queue-runtime`; not yet available |
//!
//! ## Message Format
//!
//! Each queue message body is a JSON-encoded GitHub webhook payload — the
//! same payload that GitHub sends to webhook endpoints. An Azure Event Grid
//! subscription or an AWS SNS→SQS bridge forwards the original payload.
//!
//! After receiving a message the implementation:
//!
//! 1. Deserialises the message body → [`pipeline::GitHubEvent`].
//! 2. On success: acknowledges (completes) the message; returns the event.
//! 3. On parse failure: calls `abandon_message` to trigger
//!    `queue-runtime`'s built-in retry and dead-letter logic. Returns
//!    [`pipeline::EventSourceError::ParseError`].
//! 4. On connectivity failure: returns
//!    [`pipeline::EventSourceError::QueueError`].
//! 5. On timeout: returns `Ok(None)`.
//!
//! Dead-lettering after `config.max_retry_attempts` failed deliveries is
//! handled entirely by `queue-runtime`; `QueueEventSource` does not
//! implement its own dead-letter logic.
//!
//! ## Session Ordering
//!
//! When `config.use_session_ordering` is `true`, the `WorkItemId` extracted
//! from each message's session key is used with `queue-runtime`'s session
//! API. This guarantees FIFO delivery for all events belonging to the same
//! work item even under concurrent message consumers.
//!
//! The session key is set by the component that enqueues the message
//! (typically the Azure Event Grid subscription or SNS→SQS bridge
//! configuration), not by `QueueEventSource` itself.
//!
//! ## AWS SQS Note
//!
//! AWS SQS support is planned in `pvandervelde/queue-runtime` but is not yet
//! available. The `QueueEventConfig::provider_config` field accepts any
//! `serde_json::Value`; when SQS support lands, the provider config shape for
//! SQS will be documented in `queue-runtime`'s README.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/github-traits.md` §QueueEventSource and
//! `docs/spec/interfaces/infrastructure.md` §listener.

use std::time::Duration;

use async_trait::async_trait;
use tracing::instrument;

use pipeline::github::{EventSource, EventSourceError, GitHubEvent, QueueEventConfig};

// ─── QueueEventSource ────────────────────────────────────────────────────────

/// Cloud-queue-based [`EventSource`] implementation.
///
/// Receives GitHub webhook payloads from Azure Service Bus (or AWS SQS, when
/// available in `queue-runtime`) and converts them to [`GitHubEvent`] values.
///
/// ## Construction
///
/// ```rust,ignore
/// use listener::queue::QueueEventSource;
/// use pipeline::QueueEventConfig;
///
/// let source = QueueEventSource::new(config);
/// // Connection is established lazily on first next_event call.
/// ```
///
/// See `docs/spec/interfaces/infrastructure.md` §QueueEventSource.
pub struct QueueEventSource {
    /// Configuration for the cloud queue connection and retry behaviour.
    #[allow(dead_code)]
    config: QueueEventConfig,
    // Fields added during implementation:
    //   queue_client: queue_runtime::QueueClient
}

impl QueueEventSource {
    /// Construct a queue consumer from the given configuration.
    ///
    /// Does not perform any I/O at construction time; the connection to the
    /// cloud queue is established lazily on the first call to
    /// [`EventSource::next_event`].
    ///
    /// # Arguments
    ///
    /// * `config` — Queue connection configuration; parsed from
    ///   `config.trigger_mode` in `cli`.
    pub fn new(config: QueueEventConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl EventSource for QueueEventSource {
    /// Receive the next message from the queue and parse it as a [`GitHubEvent`].
    ///
    /// Blocks for at most `timeout`. Returns `Ok(None)` if no message is
    /// delivered within the timeout window.
    ///
    /// ## Message Lifecycle
    ///
    /// 1. Receive message with `timeout` (blocking receive).
    /// 2. Deserialise body (JSON GitHub webhook payload) → [`GitHubEvent`].
    /// 3. On success: complete (acknowledge) the message; return `Ok(Some(event))`.
    /// 4. On parse failure: call `abandon_message` to increment the delivery
    ///    count; after `config.max_retry_attempts` the broker dead-letters
    ///    the message automatically. Return `Err(EventSourceError::ParseError)`.
    /// 5. On connectivity failure: return `Err(EventSourceError::QueueError)`.
    ///
    /// ## Session Ordering
    ///
    /// When `config.use_session_ordering` is `true`, the receive call uses
    /// `queue-runtime`'s session-aware receive API, which ensures messages
    /// sharing the same session key (= `WorkItemId`) are delivered in FIFO
    /// order.
    ///
    /// ## Errors
    ///
    /// * `EventSourceError::ParseError { raw }` — message body could not be
    ///   parsed. `raw` contains the first 512 bytes (truncated).
    /// * `EventSourceError::QueueError { provider }` — broker connectivity
    ///   failure; caller should apply exponential backoff before retrying.
    /// * `EventSourceError::Timeout` — no message within `timeout`.
    ///
    /// See `docs/spec/interfaces/infrastructure.md` §QueueEventSource.
    #[instrument(skip(self))]
    async fn next_event(
        &mut self,
        _timeout: Duration,
    ) -> Result<Option<GitHubEvent>, EventSourceError> {
        todo!(
            "QueueEventSource::next_event — \
               see docs/spec/interfaces/infrastructure.md §QueueEventSource"
        )
    }
}
