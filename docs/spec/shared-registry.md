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
| `ReworkEdge` | Back-edge metadata (max traversals, semantics, overflow behaviour) |
| `EdgeDefinition` | Static edge declaration (source, target, condition, rework metadata) |
| `PipelineSettings` | Pipeline-level execution defaults |
| `PipelineGraph` | Validated graph (nodes + edges + eval modes + settings) |
| `PipelineToolProfileConfig` | Tool-profile overrides per node |
| `PipelineConfiguration` | Full `.cogworks/pipeline.toml` contents |

**Runtime state enums**

| Type | Purpose |
|------|---------|
| `NodeStatus` | `Pending` / `Active` / `Completed` / `Failed` / `HumanGated` |
| `EvaluatorKind` | `Deterministic` / `LlmModel { model_id }` / `Composite` |

**Runtime state structs**

| Type | Purpose |
|------|---------|
| `NodeState` | Per-node mutable state (status, attempts, rework counts, error) |
| `PipelineState` | Full run state (node states, parallel branches, cost accumulator) |
| `EdgeEvaluationRecord` | Audit record for one edge-condition evaluation |
| `PipelineStateComment` | Self-contained GitHub comment payload for state persistence |

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

### GitHub & Events (`pipeline/src/github.rs`)

| Type | Purpose |
|------|---------|
| *(to be added)* | `GitHubEvent`, `EventSource` trait, `WebhookConfig`, `QueueEventConfig`, `IssueTracker` trait, etc. |

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
