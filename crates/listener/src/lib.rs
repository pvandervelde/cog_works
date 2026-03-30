//! CogWorks trigger event source infrastructure.
//!
//! Implements the [`pipeline::EventSource`] trait with two backends:
//!
//! - [`GitHubWebhookEventSource`] (`webhook` module) — binds an HTTP server
//!   and receives GitHub webhook payloads directly (or via smee.io in
//!   development). Validates the HMAC-SHA256 signature of every incoming
//!   request before parsing using `github-bot-sdk`'s webhook responder.
//!
//! - [`QueueEventSource`] (`queue` module) — consumes messages from a cloud
//!   message queue via `queue-runtime` (Azure Service Bus today; AWS SQS
//!   planned). Each message body is a JSON-encoded GitHub webhook payload
//!   forwarded by an Azure Event Grid subscription or AWS SNS→SQS bridge.
//!   Uses `queue-runtime`'s session API with the `WorkItemId` as the session
//!   key, ensuring all events for one work item are processed in order.
//!
//! ## Phase 1 Note
//!
//! In Phase 1 CLI mode (`TriggerMode::SingleShot`), neither event source is
//! used. `cli` synthesises a single `GitHubEvent` from `--issue-url` directly
//! and calls `run_step` once. The `listener` crate is only engaged when
//! running in service mode.
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
//! deserialisation all live here. The [`pipeline`] crate sees only
//! [`pipeline::EventSource`] and [`pipeline::GitHubEvent`].
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/github-traits.md` §EventSource and
//! `docs/spec/interfaces/infrastructure.md` §listener for the full contract.

pub mod queue;
pub mod webhook;

// ─── Re-exports ──────────────────────────────────────────────────────────────

pub use queue::QueueEventSource;
pub use webhook::GitHubWebhookEventSource;
