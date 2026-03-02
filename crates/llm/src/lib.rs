//! CogWorks LLM provider infrastructure adapter.
//!
//! Implements the [`pipeline::LlmProvider`] trait for Anthropic's API.
//! Additional providers (e.g. OpenAI) are added as new `impl` blocks in this
//! crate without any changes to the `pipeline` crate.
//!
//! ## Architectural Layer
//!
//! **Infrastructure.** All HTTP transport, request formatting, response parsing,
//! rate-limit header tracking, and exponential back-off live here. The
//! [`pipeline`] crate sees only [`pipeline::LlmProvider`].
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/infrastructure.md` §llm for the full contract.
//!
//! *This crate is a skeleton. Method bodies are added in PR 10.*
