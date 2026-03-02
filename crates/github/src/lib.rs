//! CogWorks GitHub infrastructure adapter.
//!
//! Implements the GitHub-facing traits defined in the [`pipeline`] crate
//! (`IssueTracker`, `PullRequestManager`, `CodeRepository`, `ProjectBoard`,
//! `AuditStore`) using [`github_bot_sdk`](https://github.com/pvandervelde/github-bot-sdk).
//!
//! ## Architectural Layer
//!
//! **Infrastructure.** This crate must not contain domain rules.
//! All GitHub API details (rate limiting, pagination, authentication) are handled
//! here; the [`pipeline`] crate never sees them.
//!
//! ## SDK Gap Tracking
//!
//! Several trait methods require GitHub API capabilities not yet in
//! `github-bot-sdk`. Until those additions land, the affected methods panic with
//! `todo!()` and are documented with the SDK issue reference. See PR 3 of the
//! interface design plan for the full gap table.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/github-traits.md` for the full contract.
//!
//! *This crate is a skeleton. Method bodies are added in PR 10.*
