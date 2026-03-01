//! CogWorks pipeline node implementations and LLM gateway.
//!
//! This crate provides the seven default pipeline node implementations (Intake
//! through Integration), the Spawning node, the LLM gateway that wraps all LLM
//! calls with constitutional rules and rate-limit tracking, and the
//! `PipelineExecutor` that drives the step-function loop.
//!
//! ## Architectural Layer
//!
//! **Orchestration layer.** Nodes sequence calls between business logic in the
//! [`pipeline`] crate and infrastructure traits (GitHub, LLM, domain services).
//! They contain no domain rules of their own.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/nodes.md` for the full contract.
//!
//! *This crate is a skeleton. Implementation is added in PR 9.*
