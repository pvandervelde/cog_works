//! Newtype domain identifiers.
//!
//! Every domain concept that has an identity is represented as a distinct newtype
//! wrapping a primitive. This prevents accidentally interchanging — for example —
//! a [`WorkItemId`] with a [`PullRequestId`] even though both are `u64` under the
//! hood.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/shared-types.md` §Identifiers for the full contract.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Macro for String-wrapped newtypes.
// Generates: struct, new() returning Option<Self>, as_str(), Display.
// ---------------------------------------------------------------------------
macro_rules! string_id {
    (
        $(#[$attr:meta])*
        $name:ident
    ) => {
        $(#[$attr])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            /// Creates a new identifier, returning `None` if the value is empty.
            pub fn new(value: impl Into<String>) -> Option<Self> {
                let v = value.into();
                if v.is_empty() { None } else { Some(Self(v)) }
            }

            /// Returns the identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Macro for u64-wrapped newtypes (GitHub-assigned integers).
// Generates: struct (Copy), new(), as_u64(), Display.
// ---------------------------------------------------------------------------
macro_rules! u64_id {
    (
        $(#[$attr:meta])*
        $name:ident
    ) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(u64);

        impl $name {
            /// Creates a new identifier from a raw integer.
            pub fn new(value: u64) -> Self {
                Self(value)
            }

            /// Returns the underlying integer value.
            pub fn as_u64(self) -> u64 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Identifiers — GitHub-integer-backed
// ---------------------------------------------------------------------------

u64_id! {
    /// Identifies a GitHub Issue that represents a unit of work for CogWorks.
    ///
    /// Wraps the GitHub Issue number assigned by GitHub (positive integer).
    WorkItemId
}

u64_id! {
    /// Identifies a GitHub Issue created by the Planning node for one
    /// implementation sub-task within a larger work item.
    SubWorkItemId
}

u64_id! {
    /// Identifies a GitHub Milestone associated with a work item.
    ///
    /// CogWorks inherits milestones; it does not create or modify them.
    MilestoneId
}

u64_id! {
    /// Identifies a GitHub Pull Request produced by the Integration node.
    PullRequestId
}

// ---------------------------------------------------------------------------
// Identifiers — UUID-backed (internally generated)
// ---------------------------------------------------------------------------

/// Identifies a single pipeline execution run (one invocation of the step function).
///
/// Generated fresh for every CLI invocation; propagated through spans and audit
/// events so all activity from a single run can be correlated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PipelineRunId(Uuid);

impl PipelineRunId {
    /// Generates a new random run identifier.
    pub fn new_random() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a [`PipelineRunId`] from an existing UUID (e.g. deserialised from state).
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Returns the underlying [`Uuid`].
    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for PipelineRunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Identifiers — String-backed (configuration / Git names)
// ---------------------------------------------------------------------------

string_id! {
    /// Identifies a pipeline node by its configured name within a pipeline graph.
    ///
    /// Node names are unique per pipeline and defined in `.cogworks/pipeline.toml`
    /// (or in the default pipeline configuration for the built-in 7-node graph).
    NodeId
}

string_id! {
    /// Identifies an edge between two nodes within a pipeline graph.
    ///
    /// Edge names are unique per pipeline and defined in `.cogworks/pipeline.toml`.
    EdgeId
}

string_id! {
    /// Identifies a named pipeline configuration (e.g. `"default"`, `"hotfix"`).
    ///
    /// Multiple named pipelines may be declared in `.cogworks/pipeline.toml`.
    PipelineName
}

string_id! {
    /// A Git branch name (e.g. `"main"`, `"feature/my-work-item-42"`).
    BranchName
}

string_id! {
    /// A Git commit SHA (40-character lowercase hex string).
    CommitSha
}

string_id! {
    /// Identifies a GitHub repository in `"owner/repo"` format.
    RepositoryId
}

string_id! {
    /// Identifies a domain service as declared in `.cogworks/services.toml`.
    ///
    /// Capabilities are discovered dynamically via the handshake; the name is the
    /// human-readable configuration key used for logging and routing decisions.
    DomainServiceName
}

string_id! {
    /// A file-system path relative to the repository root.
    ///
    /// Used to identify artefacts produced or consumed by pipeline nodes.
    ArtifactPath
}

string_id! {
    /// Identifies an interface contract in the human-maintained interface registry.
    ///
    /// CogWorks reads interface definitions; it does not create or modify them.
    InterfaceId
}

string_id! {
    /// Identifies a Context Pack by its directory name within `.cogworks/context-packs/`.
    ContextPackId
}

string_id! {
    /// Identifies a skill: a deterministic, reusable sequence of tool calls.
    SkillName
}

string_id! {
    /// Identifies a tool exposed to LLM nodes (built-in, adapter-generated, or skill).
    ToolName
}

string_id! {
    /// Identifies a tool profile that controls which tools are available to a node.
    ProfileName
}
