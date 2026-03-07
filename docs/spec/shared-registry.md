# Shared Type Registry

This registry catalogs every reusable type, trait, and pattern in the CogWorks
workspace. It is updated incrementally as each PR adds new definitions.

**Coders**: before creating a new type, check here to avoid duplication.
**Reviewers**: verify that new types are registered here before approving.

---

## Core Identifiers

All live in `crates/pipeline/src/identifiers.rs` and re-exported from `pipeline`.
Spec: `docs/spec/interfaces/shared-types.md` §Identifiers.

| Type | Wraps | Notes |
|------|-------|-------|
| `WorkItemId` | `u64` | GitHub Issue number (unit of work) |
| `SubWorkItemId` | `u64` | GitHub Issue number (planning sub-task) |
| `MilestoneId` | `u64` | GitHub Milestone number |
| `PullRequestId` | `u64` | GitHub PR number |
| `PipelineRunId` | `Uuid` | Generated per CLI invocation |
| `NodeId` | `String` | Pipeline node name |
| `EdgeId` | `String` | Pipeline edge name |
| `PipelineName` | `String` | Named pipeline configuration |
| `BranchName` | `String` | Git branch name |
| `CommitSha` | `String` | 40-char hex git commit SHA |
| `GitObjectSha` | `String` | Git object SHA (blob or tree) as returned by the GitHub Contents API. Not a commit SHA. |
| `RepositoryId` | `String` | `"owner/repo"` format |
| `DomainServiceName` | `String` | Key in `.cogworks/services.toml` |
| `ArtifactPath` | `String` | Repo-relative file path |
| `InterfaceId` | `String` | Interface contract ID |
| `ContextPackId` | `String` | Context Pack directory name |
| `SkillName` | `String` | Skill identifier |
| `ToolName` | `String` | Tool identifier |
| `ProfileName` | `String` | Tool profile identifier |

---

## Core Value Types

All live in `crates/pipeline/src/types.rs` and re-exported from `pipeline`.
Spec: `docs/spec/interfaces/shared-types.md` §Value Types.

| Type | Purpose |
|------|---------|
| `TokenCount` | LLM token count (non-negative integer) |
| `TokenCost` | LLM call cost in USD (`f64`) |
| `CostBudget` | Maximum allowed cost cap (`f64`) |
| `SatisfactionScore` | Scenario satisfaction score in `[0.0, 1.0]` |
| `AlignmentScore` | Alignment verification score in `[0.0, 1.0]` |
| `DiagnosticSeverity` | `Blocking` / `Warning` / `Informational` |
| `DiagnosticCategory` | Category tag string (open set) |
| `Diagnostic` | Structured finding from domain service / review / alignment |
| `ApiVersion` | Extension API semantic version `{ major, minor }` |
| `Timestamp` | UTC wall-clock timestamp (wraps `chrono::DateTime<Utc>`) |

---

## Core Error Types

All live in `crates/pipeline/src/errors.rs` and re-exported from `pipeline`.
Spec: `docs/spec/interfaces/shared-types.md` §Error Types.

| Type | Purpose |
|------|---------|
| `RetryPolicy` | `Retryable { after }` / `NonRetryable` — cross-cutting retry decision |
| `CogWorksError` | Pipeline-halting conditions (injection, budget, scope, config) |

---

## Domain Types (added in subsequent PRs)

The following domains will add entries here as work proceeds:

### Pipeline Graph (`pipeline/src/graph.rs`)

All types re-exported from `pipeline`.
Spec: `docs/spec/interfaces/pipeline-graph.md`.

**Auxiliary scalars**

| Type | Purpose |
|------|---------|
| `Expression` | Newtype — deterministic boolean predicate string |
| `NaturalLanguageCondition` | Newtype — LLM-evaluated condition description string |
| `TimeoutSeconds` | Newtype — serialisable timeout (wraps `u64` seconds) |
| `SchemaVersion` | Newtype — serde-enforced version token for `PipelineStateComment`; rejects unknown values at deserialisation |

**Graph structure enums**

| Type | Purpose |
|------|---------|
| `NodeType` | `Llm` / `Deterministic` / `Spawning` |
| `NodeGate` | `AutoProceed` / `HumanGated` |
| `ValidationKind` | `None` / `DomainService` / `Scenario` |
| `EvaluationMode` | `AllMatching` / `FirstMatching` / `Explicit` |
| `ReworkSemantics` | `Retry` / `Rework` |
| `OverflowBehaviour` | `HaltWithError` / `Escalate` / `TakeEdge(EdgeId)` |
| `EdgeConditionKind` | `Deterministic(Expression)` / `LlmEvaluated` / `Composite` |
| `CompositeCondition` | `And` / `Or` / `Not` combinator |

**Graph structure structs**

| Type | Purpose |
|------|---------|
| `NodeDefinition` | Static node declaration (id, type, inputs, outputs, timeout, gate, …) |
| `ReworkEdge` | Back-edge metadata (max traversals ≥ 1, semantics, overflow behaviour) |
| `EdgeDefinition` | Static edge declaration (source, target, condition, rework metadata) |
| `PipelineSettings` | Pipeline-level execution defaults |
| `PipelineGraph` | Validated graph (nodes + edges + eval modes + explicit-edge lists + settings + tool_profiles) |
| `PipelineToolProfileConfig` | Tool-profile overrides per node (scoped to one pipeline) |
| `PipelineConfiguration` | Full `.cogworks/pipeline.toml` contents; each pipeline carries its own tool_profiles |

**Runtime state enums**

| Type | Purpose |
|------|---------|
| `NodeStatus` | `Pending` / `Active` / `Completed` / `Failed` / `HumanGated` |
| `EvaluatorKind` | `Deterministic` / `LlmModel { model_id }` / `Composite` |

**Runtime state structs**

| Type | Purpose |
|------|---------|
| `NodeState` | Per-node mutable state (status, attempts, rework counts, error) |
| `PipelineState` | Full run state (node states, parallel branches, `cost_accumulator: TokenCost`) |
| `EdgeEvaluationRecord` | Audit record for one edge-condition evaluation; `input_snapshot` is `serde_json::Value` |
| `PipelineStateComment` | Self-contained GitHub comment payload; `schema_version: SchemaVersion` enforced at serde |

**Error types**

| Type | Purpose |
|------|---------|
| `CycleError` | Returned by `topological_sort` when forward-edge cycle detected |
| `GraphValidationError` | Single structural violation from `validate_pipeline_graph` |

**Pure functions**

| Function | Signature summary |
|----------|------------------|
| `topological_sort` | `(&[NodeDefinition], &[EdgeDefinition]) → Result<Vec<NodeId>, CycleError>` |
| `evaluate_deterministic_condition` | `(&Expression, &PipelineState) → bool` |
| `validate_pipeline_graph` | `(&PipelineGraph) → Result<(), Vec<GraphValidationError>>` |
| `compute_eligible_nodes` | `(&PipelineState, &PipelineGraph) → Vec<NodeId>` |

### GitHub & Events (`pipeline/src/github.rs`, `pipeline/src/templates.rs`, `pipeline/src/audit.rs`)

All types re-exported from `pipeline`.
Spec: `docs/spec/interfaces/github-traits.md`.

**Event trigger types** (`github.rs`)

| Type | Purpose |
|------|---------|
| `GitHubEvent` | `LabelApplied` / `CommentPosted` / `SubIssueStateChanged` / `PullRequestReviewed` |
| `EventSourceError` | `Timeout` / `ConnectionLost` / `ParseError` / `AuthError` / `QueueError` |
| `WebhookConfig` | Bind address, path prefix, HMAC secret |
| `QueueEventConfig` | Provider config (opaque JSON), queue name, session ordering, retry attempts |

**Issue types** (`github.rs`)

| Type | Purpose |
|------|---------|
| `IssueState` | `Open` / `Closed` |
| `Label` | Name + optional CSS hex colour |
| `Milestone` | Numeric ID, title, optional due date |
| `TypedLinkKind` | `Blocks` / `IsBlockedBy` |
| `TypedLink` | Source ID, target ID, kind |
| `Issue` | Full issue view (ID, repo, title, body, state, labels, milestone, timestamps) |
| `SubIssue` | Sub-task view (ID, parent ID, title, state, created_at) |

**Pull request types** (`github.rs`)

| Type | Purpose |
|------|---------|
| `ReviewDecision` | `Approved` / `ChangesRequested` / `Commented` / `Dismissed` |
| `ReviewStatus` | Approval count, `changes_requested` flag, `approved` flag |
| `PullRequest` | Full PR view (ID, repo, title, body, branches, SHA, open/merged, review status, created_at) |
| `PullRequestFilter` | Optional base/head branch and open-only filter |

**Repository types** (`github.rs`)

| Type | Purpose |
|------|---------|
| `FileContent` | Path, raw bytes, SHA, content type; `as_text() -> Option<&str>` |
| `DirectoryEntryKind` | `File` / `Directory` / `Symlink` / `Submodule` |
| `DirectoryEntry` | Name, path, kind, SHA |

**Error type** (`github.rs`)

| Type | Purpose |
|------|---------|
| `GitHubOperationError` | `NotFound` / `PermissionDenied` / `RateLimitExhausted` / `Transient` / `ParseFailure` / `SdkCapabilityMissing` |

**Port traits** (`github.rs`)

| Trait | Implemented by | Purpose |
|-------|---------------|---------|
| `EventSource` | `GitHubWebhookEventSource`, `QueueEventSource`, CLI one-shot | Trigger source abstraction |
| `IssueTracker` | `GithubClient` | Issue / sub-issue / label / comment / milestone operations |
| `PullRequestManager` | `GithubClient` | PR lifecycle and review operations |
| `CodeRepository` | `GithubClient` | Read-only file and tree access |
| `ProjectBoard` | `GithubClient` | Projects V2 status/field sync (non-blocking) |

**Template types** (`templates.rs`)

| Type | Purpose |
|------|---------|
| `TemplateError` | `NotFound` / `MissingVariables` / `SyntaxError` / `ConstraintViolation` |
| `TemplateEngine` *(trait)* | `render(name, context) -> String`, `list_required_variables(name) -> Vec<String>` |

**Audit types** (`audit.rs`)

| Type | Purpose |
|------|---------|
| `LlmCallRecord` | Model ID, token counts, cost, latency, schema_validated, timestamp |
| `ValidationRecord` | Node ID, kind, passed, diagnostics, timestamp |
| `StateTransitionRecord` | Node ID, from/to status, reason, timestamp |
| `CostSnapshot` | Node ID, accumulated, budget, budget_exceeded, timestamp |
| `InjectionDetectionRecord` | Node ID, source label, offending text, pattern name, timestamp |
| `ScopeViolationRecord` | Node ID, artifact path, description, violation kind, timestamp |
| `AuditEvent` | Union of all above + `EdgeEvaluation(EdgeEvaluationRecord)` |
| `PipelineOutcome` | `Completed` / `Failed` / `HumanGated` / `Escalated` |
| `PipelineSummary` | Run ID, work item, outcome, cost, duration, node counts, rework count, terminal message |
| `AuditStoreError` | `Unavailable` / `SerialisationError` — non-fatal |
| `AuditStore` *(trait)* | `record_event(...)`, `write_summary(...)` |

### Domain Services (`pipeline/src/domain_services.rs`)

| Type | Purpose |
|------|---------|
| *(to be added)* | `DomainServiceClient` trait, `HandshakeResult`, `StructuredResponse`, etc. |

### Security (`pipeline/src/security.rs`)

| Type | Purpose |
|------|---------|
| *(to be added)* | `ConstitutionalRules`, `ValidatedPrompt`, `InjectionDetectionResult`, etc. |

### Context Assembly (`pipeline/src/context.rs`, `pipeline/src/labels.rs`)

| Type | Purpose |
|------|---------|
| *(to be added)* | `ContextPackage`, `ContextPack`, `PipelineLabel`, etc. |

### Execution (`pipeline/src/execution.rs` et al.)

| Type | Purpose |
|------|---------|
| *(to be added)* | `NextAction`, `BudgetAcquisition`, `ClassificationResult`, `AggregateReviewDecision`, etc. |

### Advanced Features

| Type | Purpose |
|------|---------|
| *(to be added)* | `AlignmentResult`, `TraceabilityMatrix`, `SkillManifest`, `CompactToolIndex`, etc. |

### Nodes (`nodes/src/`)

| Type | Purpose |
|------|---------|
| *(to be added)* | `NodeInput`, `NodeOutput`, `LlmGateway`, `PipelineExecutor`, `StepResult`, etc. |

---

## Infrastructure Types (added in PR 10)

| Crate | Type | Implements |
|-------|------|-----------|
| `github` | `GithubClient` | `IssueTracker`, `PullRequestManager`, `CodeRepository`, `ProjectBoard`, `AuditStore` |
| `llm` | `AnthropicProvider` | `LlmProvider` |
| `extension-api` | `ExtensionApiClient` | `DomainServiceClient` |
| `listener` | `GitHubWebhookEventSource` | `EventSource` |
| `listener` | `QueueEventSource` | `EventSource` |

---

## Patterns

### Error Handling

All domain operations return `Result<T, E>`.
Infrastructure errors implement a `retry_policy(&self) -> RetryPolicy` method.
`CogWorksError` variants are all `NonRetryable`.

### Validation at Boundaries

Newtype constructors validate invariants (non-empty strings, non-negative costs,
bounded scores). **Never bypass constructors** by accessing inner fields directly.

### Observability

All public operations in `pipeline` that may emit structured events use
`tracing::instrument` or explicit `tracing::Span::enter()` calls.
Field names follow OpenTelemetry semantic conventions where applicable.

### Async

All infrastructure trait methods are `async`. Business logic functions in
`pipeline` are synchronous (pure functions on data).

### Serialisation

All types that appear in `PipelineStateComment` (written to GitHub) derive
`Serialize` and `Deserialize`. The format is JSON (via `serde_json`). The set
of serialisable types grows with each PR; this registry notes which types are
serialisable.
