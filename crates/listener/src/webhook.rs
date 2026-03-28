//! GitHub webhook-based event source.
//!
//! Provides [`GitHubWebhookEventSource`], which receives GitHub webhook
//! payloads via an HTTP server (using `github-bot-sdk`'s webhook responder)
//! and delivers parsed [`pipeline::GitHubEvent`] values to the pipeline
//! event loop.
//!
//! ## HMAC Validation
//!
//! Every incoming HTTP POST is HMAC-SHA256 verified **before** parsing.
//! The `X-Hub-Signature-256` header is compared to
//! `HMAC-SHA256(config.secret, body)` using a **constant-time comparison**
//! to prevent timing-based secret recovery. Requests that fail verification
//! return [`pipeline::EventSourceError::AuthError`] and are **never** parsed.
//!
//! ## Event Parsing
//!
//! After HMAC verification the raw JSON body is parsed based on the
//! `X-GitHub-Event` header value:
//!
//! | Header value | Payload `action` | [`pipeline::GitHubEvent`] variant |
//! |---|---|---|
//! | `issues` | `"labeled"` | `LabelApplied` |
//! | `issue_comment` | `"created"` | `CommentPosted` |
//! | `issues` | `"closed"` (sub-issue) | `SubIssueStateChanged` |
//! | `pull_request_review` | `"submitted"` | `PullRequestReviewed` |
//! | *(other)* | *(any)* | silently skipped (not an error) |
//!
//! ## Internal Channel
//!
//! The webhook HTTP server runs in a background Tokio task. Verified and
//! parsed events are sent over a bounded `mpsc` channel. `next_event` reads
//! from the receiver side, blocking up to the specified `timeout`.
//!
//! ## Local Development
//!
//! Use [smee.io](https://smee.io/) as a reverse proxy when GitHub cannot
//! reach the local instance:
//!
//! ```bash
//! smee --url https://smee.io/<channel-id> --port <bind_port>
//! ```
//!
//! The webhook server is unaware of the proxy; no code changes are needed.
//!
//! ## Security
//!
//! `WebhookConfig::secret` MUST NOT appear in tracing events or log lines.
//! The `Debug` impl for `WebhookConfig` prints `"[REDACTED]"` for the secret
//! field.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/github-traits.md` §GitHubWebhookEventSource
//! and `docs/spec/interfaces/infrastructure.md` §listener.

use std::time::Duration;

use async_trait::async_trait;
use tracing::instrument;

use pipeline::github::{EventSource, EventSourceError, GitHubEvent, WebhookConfig};

// ─── GitHubWebhookEventSource ────────────────────────────────────────────────

/// GitHub webhook-based [`EventSource`] implementation.
///
/// Binds an HTTP server on `config.bind_address` using `github-bot-sdk`'s
/// webhook responder. Every incoming POST is HMAC-SHA256 verified against
/// `config.secret` before being parsed into a [`GitHubEvent`].
///
/// ## Construction
///
/// ```rust,ignore
/// use listener::webhook::GitHubWebhookEventSource;
/// use pipeline::WebhookConfig;
///
/// let source = GitHubWebhookEventSource::new(config)?;
/// // `source` owns the webhook server handle; drop to shut down the server.
/// ```
///
/// ## Security
///
/// `config.secret` is stored in memory only and is excluded from the `Debug`
/// output of [`WebhookConfig`]. Never log `config.secret` or include it in
/// error messages.
///
/// See `docs/spec/interfaces/infrastructure.md` §GitHubWebhookEventSource.
pub struct GitHubWebhookEventSource {
    /// Configuration for the webhook HTTP server (secret is never logged).
    #[allow(dead_code)]
    config: WebhookConfig,
    // Fields added during implementation:
    //   receiver: tokio::sync::mpsc::Receiver<GitHubEvent>
    //   _server_handle: tokio::task::JoinHandle<()>
}

impl GitHubWebhookEventSource {
    /// Construct and start the webhook HTTP server.
    ///
    /// Binds `config.bind_address` and spawns a background Tokio task that
    /// accepts incoming webhook requests. The background task runs for the
    /// lifetime of this struct.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP server cannot bind to `config.bind_address`
    /// (e.g. port already in use, insufficient permissions).
    ///
    /// See `docs/spec/interfaces/infrastructure.md` §GitHubWebhookEventSource.
    pub fn new(config: WebhookConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl EventSource for GitHubWebhookEventSource {
    /// Await the next webhook event.
    ///
    /// Blocks for at most `timeout`. Returns `Ok(None)` if no event arrives
    /// within the timeout window.
    ///
    /// ## HMAC Guarantee
    ///
    /// Only events that have passed HMAC-SHA256 verification are placed in the
    /// internal channel. This method never returns an unauthenticated event.
    ///
    /// ## Errors
    ///
    /// * `EventSourceError::AuthError` — HMAC verification failed.
    /// * `EventSourceError::ParseError { raw }` — JSON payload could not be
    ///   parsed; `raw` contains the first 512 bytes of the failing body
    ///   (truncated to avoid logging unbounded content).
    /// * `EventSourceError::ConnectionLost { .. }` — The background server task
    ///   exited unexpectedly.
    ///
    /// See `docs/spec/interfaces/infrastructure.md` §GitHubWebhookEventSource.
    #[instrument(skip(self))]
    async fn next_event(
        &mut self,
        _timeout: Duration,
    ) -> Result<Option<GitHubEvent>, EventSourceError> {
        todo!(
            "GitHubWebhookEventSource::next_event — \
               see docs/spec/interfaces/infrastructure.md §GitHubWebhookEventSource"
        )
    }
}
