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
