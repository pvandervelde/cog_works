# Component Responsibilities

This document uses CRC-style (Class-Responsibility-Collaborator) notes to define what each component knows, what it does, and who it delegates to. These responsibilities inform the interface designer on what types and functions to create.

---

## Pipeline Executor

The central coordinator. Knows the stage sequence and decides what to do next.

**Responsibilities:**

- Knows: Stage sequence (1-7), stage gate configuration (auto/human per stage), current pipeline state (reconstructed from GitHub), constitutional rules file path
- Does: Loads constitutional rules at the start of every run (unconditional, before any other action), determines the next action for a given work item, delegates to the appropriate stage executor, enforces stage gate rules, manages the processing lock

**Collaborators:**

- Constitutional Rules Loader (loads constitutional rules from well-known path)
- GitHub Client (reads current state: labels, issues, PRs)
- Stage Executors (delegates execution of each stage)
- Audit Recorder (logs stage transitions)
- Configuration Manager (reads gate configuration)

**Roles:**

- Orchestrator: Coordinates the pipeline flow
- State machine: Enforces valid stage transitions
- Lock manager: Applies/removes processing lock
- Constitutional enforcer: Ensures constitutional rules are loaded before any LLM interaction

**Key behavior:**

- Load constitutional rules before any other action (REQ-CONST-001) — failure to load is a pipeline-halting error
- Given a work item, reconstruct its pipeline state from GitHub (labels, sub-issues, PR status)
- Determine which stage the pipeline is in
- Determine whether the current stage gate has been passed
- If the gate is passed, advance to the next stage by delegating to the appropriate executor
- If the gate is pending (human-gated), do nothing (exit and wait for next invocation)
- Handle the special case of stages 5-7 looping over sub-work-items in dependency order

---

## Task Classifier (Stage 1 Executor)

Analyzes a work item to determine its type, scope, and safety impact.

**Responsibilities:**

- Knows: Classification output schema, safety-critical module registry, scope thresholds
- Does: Invokes LLM to classify the work item, cross-validates safety classification against registry, checks scope against threshold, posts classification summary

**Collaborators:**

- LLM Gateway (sends classification prompt, receives structured response)
- GitHub Client (reads issue body, posts classification comment, applies labels)
- Configuration Manager (reads safety registry path, scope thresholds)

**Roles:**

- Classifier: Determines task type and scope
- Safety validator: Enforces safety classification override rules
- Escalation trigger: Raises scope escalation when threshold exceeded

**Key behavior:**

- Read the issue body and any structured fields
- Load the list of repository modules and the safety-critical registry
- Invoke LLM with classification prompt and validate response against schema
- If LLM says "not safety-affecting" but affected modules include a registered safety-critical module → override to safety-affecting
- If estimated scope exceeds threshold → escalate (don't proceed)
- Post classification summary as issue comment, apply stage and safety labels

---

## Specification Generator (Stage 2 Executor)

Produces a technical specification document from the classified work item.

**Responsibilities:**

- Knows: Specification document structure, architectural constraint rules, Context Pack trigger rules
- Does: Loads applicable Context Packs based on classification labels/tags, assembles context (including loaded pack knowledge), invokes LLM to generate spec, validates references and dependency changes, runs cross-domain constraint validation against interface registry at architecture stage, creates specification PR

**Collaborators:**

- Context Pack Loader (loads domain knowledge packs based on classification labels and component tags)
- Context Assembler (builds context package from repo files, ADRs, constraints, and loaded Context Pack knowledge)
- LLM Gateway (sends spec generation prompt, receives Markdown)
- Constraint Validator (checks proposed architecture against cross-domain interface contracts)
- GitHub Client (creates branch, commits spec, creates PR, updates labels)

**Roles:**

- Generator: Produces the specification document
- Validator: Ensures referenced modules exist and dependency changes are valid
- Constraint checker: Verifies proposed architecture doesn't violate cross-domain contracts
- Retry coordinator: Feeds validation errors back to LLM on failure
- Pack loader: Triggers deterministic Context Pack loading based on classification

---

## Interface Generator (Stage 3 Executor)

Produces concrete interface definitions in the target domain's artifact format.

**Responsibilities:**

- Knows: Interface definition conventions for the target domain
- Does: Reads approved spec, assembles context, invokes LLM to generate interfaces, validates via domain service, creates interface PR

**Collaborators:**

- Context Assembler (builds context from spec, existing interfaces, domain patterns)
- LLM Gateway (sends interface generation prompt, receives source files)
- Domain Service Client (validates generated interfaces via domain service's `validate` method)
- GitHub Client (creates branch, commits interfaces, creates PR, updates labels)

**Roles:**

- Generator: Produces interface source files
- Validator: Ensures interfaces are syntactically and semantically correct per domain
- Retry coordinator: Feeds domain service errors back to LLM on failure

---

## Work Planner (Stage 4 Executor)

Decomposes the approved spec and interfaces into sub-work-items.

**Responsibilities:**

- Knows: Sub-work-item schema, granularity limits, dependency graph rules
- Does: Invokes LLM to decompose work, validates dependency graph (topological sort, cycle detection), checks granularity limits, verifies interface coverage, creates sub-work-item issues

**Collaborators:**

- LLM Gateway (sends planning prompt, receives structured plan)
- GitHub Client (creates sub-work-item issues with labels and dependencies)
- Configuration Manager (reads granularity limits)

**Roles:**

- Decomposer: Breaks work into discrete sub-work-items
- Graph validator: Ensures dependency graph is a DAG (no cycles)
- Coverage checker: Verifies every interface is covered by at least one sub-work-item
- Escalation trigger: Raises granularity escalation if too many sub-work-items

---

## Code Generator (Stage 5 Executor)

Generates implementation artifacts for a single sub-work-item through an iterative refinement loop.

**Responsibilities:**

- Knows: Iterative refinement loop (generate → validate → simulate → retry), retry budget
- Does: Assembles context (spec + interfaces + prior SWI outputs + sub-work-item description), invokes LLM to generate artifacts, runs deterministic checks via domain service, feeds errors back to LLM, iterates until pass or budget exhausted

**Collaborators:**

- Context Assembler (builds context with full SWI history)
- LLM Gateway (sends code generation prompt, receives source files)
- Domain Service Client (normalise → validate → review_rules → simulate)
- Budget Tracker (checks cost budget before each LLM call)

**Roles:**

- Generator: Produces implementation artifacts and tests
- Feedback loop coordinator: Routes deterministic errors back to LLM with structured context
- Budget enforcer: Halts when retry or cost budget exceeded

**Key behavior (the refinement loop):**

1. Assemble context package
2. Invoke LLM to generate artifacts
3. Run domain service: normalise → validate → review_rules
4. If deterministic checks fail → feed structured errors to LLM, retry from step 2
5. If deterministic checks pass → run simulation/tests via domain service `simulate`
6. If simulation fails → assess if failure is self-explanatory (deterministic heuristic); if yes, feed directly to LLM; if no, invoke LLM to interpret failure, then retry from step 2
7. If simulation passes → hand off to Review Executor
8. If retry budget exceeded → escalate

---

## Review Executor (Stage 6 Executor)

Performs multi-dimensional review of generated artifacts.

**Responsibilities:**

- Knows: Four review dimensions (cross-domain constraint validation, quality, architecture compliance, security), severity levels (blocking/warning/informational), aggregation rules, required artefacts declared by loaded Context Packs
- Does: Runs deterministic cross-domain constraint validation first, verifies all required artefacts declared by loaded Context Packs are present (missing artefacts produce blocking findings), then three independent LLM review passes, aggregates results, determines overall pass/fail, feeds blocking findings back to Code Generator for remediation

**Collaborators:**

- Constraint Validator (deterministic cross-domain constraint checking)
- LLM Gateway (sends three review prompts, receives structured review results)
- Audit Recorder (logs review results)

**Roles:**

- Constraint checker: Runs deterministic cross-domain constraint validation before LLM reviews (cheapest check first)
- Artefact checker: Verifies required artefacts from Context Packs are present (blocking if missing)
- Reviewer: Executes three independent LLM review passes
- Aggregator: Combines results deterministically (any blocking = fail)
- Feedback provider: Routes blocking findings back to Code Generator with context

**Key behavior:**

- Cross-Domain Constraint Validation (deterministic, runs first): Validates generated artifacts against interface registry contracts using domain service's `extract_interfaces` method
- Code Quality Review (LLM): coding standards, idioms, error handling, naming, documentation
- Architecture Compliance Review (LLM): matches spec, respects boundaries, no unplanned dependencies, interfaces implemented correctly
- Security Review (LLM): input validation, auth boundaries, unsafe code, vulnerability patterns, dependency security

---

## Integration Manager (Stage 7 Executor)

Creates Pull Requests for completed sub-work-items.

**Responsibilities:**

- Knows: PR structure (description template, required references), commit conventions
- Does: Creates PR with generated code, posts non-blocking review findings as PR comments, links PR to sub-work-item and parent work item

**Collaborators:**

- GitHub Client (creates PR, posts comments)
- Working Copy Manager (commits files to branch)
- Audit Recorder (logs PR creation)

**Roles:**

- PR creator: Produces well-structured Pull Requests
- Comment poster: Attaches non-blocking findings as review comments
- Linker: Ensures full traceability (sub-work-item → parent → spec PR → interface PR)

---

## LLM Gateway

Thin abstraction over LLM API calls with validation, cost tracking, budget enforcement, and constitutional rules injection.

**Responsibilities:**

- Knows: Model capabilities (context window size, token limits), cost per token per model, output schemas for each stage, rate limits, loaded constitutional rules
- Does: Injects constitutional rules as a privileged, non-overridable component of the system prompt before any other context, sends prompts to LLM API, validates responses against output schemas, tracks token usage and cost, enforces pipeline cost budget, retries on API failures and schema validation failures, routes to different models per stage configuration

**Collaborators:**

- LLM API provider (external: Anthropic initially)
- Budget Tracker (reports token usage per call)
- Configuration Manager (reads model selection per stage)

**Roles:**

- API abstraction: Hides provider-specific details behind a consistent interface
- Schema enforcer: Rejects invalid outputs and retries automatically
- Cost tracker: Accumulates token usage per call
- Budget gate: Refuses calls that would exceed pipeline budget
- Rate limiter: Applies backoff on API-level failures

---

## Context Assembler

Deterministic service that builds context packages for LLM calls. Contains **zero** LLM logic.

**Responsibilities:**

- Knows: Pyramid summary levels (1-4), context priority order (per vocabulary.md), token budget per model, file relevance rules, scenario separation rules, loaded Context Pack content
- Does: Identifies relevant files based on affected modules, loads constraint documents (ADRs, standards, architectural rules), incorporates loaded Context Pack knowledge (domain knowledge, safe patterns, anti-patterns) as high-priority context, computes transitive dependencies via domain service dependency graph, selects appropriate summary level per module based on dependency distance, applies priority-based truncation when context exceeds window, enforces scenario holdout (never includes scenarios in code generation context), includes relevant interface registry entries for cross-domain context

**Collaborators:**

- GitHub Client (reads file contents via API for lightweight access)
- Domain Service Client (computes dependency graph for dependency-based relevance)
- Configuration Manager (reads constraint file paths, ADR locations, scenario directory exclusions)
- Summary Cache (reads cached pyramid summaries)
- Interface Registry Manager (provides relevant cross-domain interface contracts)

**Roles:**

- File selector: Determines which files are relevant to a given stage/sub-work-item
- Constraint injector: Loads and includes project rules as hard requirements
- Pack injector: Includes loaded Context Pack domain knowledge, safe patterns, and anti-patterns
- Summary selector: Chooses appropriate detail level per module (Level 1/2/3/4 based on dependency distance)
- Truncator: Applies deterministic priority-based truncation with progressive level demotion to fit context window
- Holdout enforcer: Ensures scenario specifications are never included in code generation context

**Key behavior (pyramid-based assembly):**

1. Direct targets (files being modified): Include full source (Level 4)
2. Direct dependencies: Include Level 3 (full interface detail)
3. Transitive dependencies: Include Level 2 (paragraph summary)
4. Broader context (affected area): Include Level 1 (one-line summary)
5. If budget exceeded: Progressively demote distant modules to lower levels before excluding entirely

---

## Domain Service (External Process)

An external process providing domain-specific tooling capabilities. Domain services communicate with CogWorks through the Extension API.

**Responsibilities:**

- Knows: Domain-specific tooling commands, how to parse structured output from those tools, how to manage its own working copy
- Does: Validates artifacts, normalises formatting, applies domain rules, executes simulations/tests, validates dependencies, extracts public interfaces from artifacts, computes dependency graphs, manages its own local clone of the repository

**Collaborators:**

- Local toolchain (subprocess execution within the domain service process)
- Shared clone management library (optional, provided by CogWorks for common git operations)

**Roles:**

- Validator: Invokes domain validation and returns structured diagnostics
- Normaliser: Applies canonical formatting and reports changes
- Rule reviewer: Runs domain-specific rules and returns structured findings
- Simulator: Executes tests/simulations and returns structured results
- Interface extractor: Parses artifacts and extracts public interface surface
- Graph builder: Computes dependency relationships between artifacts
- Scenario runner: Executes scenario trajectories with given environment setup

**Extension API methods:**

| Method | Purpose | Invoked during |
|---|---|---|
| `validate(artifacts)` | Check domain rules (compile, DRC, tolerance analysis, etc.) | Code generation refinement, architecture validation |
| `normalise(artifacts)` | Apply canonical formatting | Code generation (before validate) |
| `review_rules(artifacts)` | Best practices and style rules | Code generation (after validate passes) |
| `simulate(filter)` | Run tests/simulations | Test execution, scenario validation |
| `validate_deps()` | Check dependency validity | Architecture validation, planning |
| `extract_interfaces(artifacts)` | Parse and extract public interface definitions | Context assembly, cross-domain constraint validation, pyramid summaries |
| `dependency_graph()` | Compute artifact dependency relationships | Context assembly, planning (topological sort) |

**All outputs are structured data** — artifact, location, severity, category, message — in a common diagnostic format. CogWorks does not interpret results beyond the structured format.

---

## Domain Service Client

CogWorks-side client that communicates with external domain service processes via the Extension API.

**Responsibilities:**

- Knows: Extension API protocol (request/response envelopes, JSON schemas, handshake protocol), service socket/URL, API version compatibility, standardised diagnostic categories and error codes
- Does: Performs handshake to discover service capabilities and negotiate API version, sends method invocations to domain services, receives and validates structured responses against Extension API schemas, handles connection failures with backoff

**Collaborators:**

- Domain Service Registry (resolves which service handles which artifacts/domains)
- Domain Service processes (external, via Unix socket or HTTP/gRPC)

**Roles:**

- Protocol handler: Serialises requests (with envelope: request_id, api_version, method, caller, repository, params, interface_contracts) and deserialises responses per Extension API schema
- Handshake coordinator: Performs handshake to discover capabilities, artifact types, interface types, domain, and negotiate API version before method calls
- Error mapper: Maps transport errors (connection refused, timeout) and Extension API error codes (tool_not_found, tool_failed, etc.) to domain-level errors with retryability information

---

## Domain Service Registry

Manages registered domain services and routes operations to the correct one.

**Responsibilities:**

- Knows: Registered domain services (from configuration: name + connection endpoint), their dynamically discovered capabilities (from handshake: domain, artifact types, interface types, supported methods)
- Does: Performs handshake on each registered service to discover capabilities, selects the appropriate domain service for given artifacts or operations, determines primary vs. secondary domain services for a sub-work-item, caches handshake results and re-queries on error, reports unavailable services

**Collaborators:**

- Configuration Manager (reads `[[services]]` entries from config)
- Domain Service Client (routes invocations to selected service)

**Roles:**

- Selector: Routes operations to the correct domain service based on artifact types and domains
- Capability resolver: Determines if a domain service supports a requested method
- Availability reporter: Reports which services are primary (required) vs. secondary (optional for cross-domain validation)

---

## Interface Registry Manager

Loads, validates, and provides access to the cross-domain interface registry.

**Responsibilities:**

- Knows: Interface definition schema, registry location (`.cogworks/interfaces/`), validation rules
- Does: Loads all interface definitions from the registry directory, validates schema conformance, checks cross-references between interfaces, detects conflicting constraints, verifies all referenced domains have registered services (or are marked external)

**Collaborators:**

- GitHub Client (reads interface definition files from repository)
- Domain Service Registry (verifies referenced domains have registered services)
- Configuration Manager (reads registry directory path)

**Roles:**

- Loader: Reads and parses interface definition files (TOML)
- Schema validator: Ensures all definitions conform to the interface definition schema
- Cross-reference checker: Validates all references between interfaces resolve
- Conflict detector: Ensures no two interfaces define conflicting constraints for the same physical parameter
- Version checker: Detects domain service / interface version mismatches

**Key behavior:**

- Registry validation is deterministic (no LLM involved)
- Runs before any pipeline stage on every pipeline invocation
- CogWorks MUST NOT create or modify interface definitions autonomously
- CogWorks MAY suggest interface additions as recommendations for human review

---

## Constraint Validator

Performs deterministic cross-domain constraint validation against the interface registry.

**Responsibilities:**

- Knows: Interface registry contents, constraint comparison rules, computed constraint formulas
- Does: Identifies relevant cross-domain interfaces for the current sub-work-item, extracts actual values from generated artifacts via domain service, compares extracted values against registry contracts, reports violations as structured findings, validates computed constraints (e.g., total bus load = sum of message rates × sizes ÷ bandwidth)

**Collaborators:**

- Interface Registry Manager (provides relevant interface contracts)
- Domain Service Client (invokes `extract_interfaces` to get actual values from artifacts)
- Audit Recorder (logs constraint validation results)

**Roles:**

- Interface matcher: Determines which cross-domain interfaces are relevant based on modified artifacts and participating domains
- Value extractor: Uses domain service to extract actual interface values from artifacts
- Constraint checker: Compares extracted values against contract parameters (bounds, enumerations, booleans)
- Computed constraint checker: Evaluates derived constraints deterministically (no formulas in registry; computed in validator)
- Violation reporter: Produces structured findings with interface ID, parameter, expected vs. actual, owning vs. violating domain

**Key behavior:**

- Runs during review gate (first pass, before LLM reviews) and during architecture stage validation
- Validates against registry contracts only — does not require other domain services to be running
- Hard constraint violations (min/max bounds) are blocking; nominal value violations are warnings
- Works even with a single domain service registered (validates that domain's artifacts against declared contracts)

---

## GitHub Client

Handles all GitHub API interaction. The system's sole interface to durable state.

**Responsibilities:**

- Knows: GitHub API (REST and GraphQL), authentication, rate limit headers
- Does: CRUD on issues (including sub-issues), labels, comments; creates and reads PRs; reads file contents; creates branches; commits files; reads PR review status

**Collaborators:**

- GitHub REST/GraphQL API (external)

**Roles:**

- State reader: Reconstructs pipeline state from GitHub
- State writer: Updates labels, creates issues, posts comments, creates PRs
- File reader: Reads repository file contents via Contents API
- Rate limit manager: Tracks remaining API budget, applies backoff when needed

---

## Working Copy Manager (Shared Library)

Provides shared working copy management capabilities that domain services can use. Published as a library, not built into CogWorks core.

**Responsibilities:**

- Knows: Git operations (clone, branch, commit, push), temporary directory management, branch naming conventions
- Does: Creates shallow clones to temp directories, creates and switches branches, commits files, pushes to remote, cleans up temp directories

**Collaborators:**

- Git CLI (subprocess execution)
- Filesystem (temporary directory creation/deletion)

**Roles:**

- Clone manager: Creates and manages temporary repository checkouts
- Branch manager: Handles branch creation per the naming convention
- Commit creator: Produces well-structured commits following repository conventions
- Cleanup handler: Ensures temp directories are removed after use

**Note**: This is a shared library that domain services may optionally use. CogWorks does not manage working copies directly. Domain services are responsible for their own file system operations.

---

## Configuration Manager

Loads and provides typed access to repository configuration.

**Responsibilities:**

- Knows: Configuration schema, default values for all settings
- Does: Loads configuration from `.cogworks/config.toml` in the target repository, validates against schema, provides typed access to settings

**Collaborators:**

- GitHub Client (reads config file from repository)

**Roles:**

- Loader: Reads and parses configuration file
- Validator: Ensures configuration is complete and valid
- Default provider: Supplies sensible defaults for optional settings

---

## Budget Tracker

Tracks LLM token consumption and enforces cost limits.

**Responsibilities:**

- Knows: Token costs per model, pipeline budget, current accumulated cost
- Does: Records token usage per LLM call, computes cumulative cost, checks budget before each call, produces cost reports (per-stage, per-sub-work-item)

**Collaborators:**

- LLM Gateway (reports token counts per call)
- Audit Recorder (includes cost data in audit trail)

**Roles:**

- Accumulator: Tracks running total of tokens consumed and cost
- Gate: Prevents LLM calls that would exceed budget
- Reporter: Produces structured cost breakdowns

---

## Scenario Validator (Stage 5b Executor)

Executes scenario validation after deterministic checks and tests pass, before the review gate.

**Responsibilities:**

- Knows: Scenario specifications (loaded from holdout location), satisfaction threshold, trajectory count per scenario, twin provisioning requirements
- Does: Loads applicable scenarios for the current sub-work-item, provisions required Digital Twins, executes each scenario multiple times (trajectories), evaluates acceptance criteria (deterministic assertions, LLM-as-judge, or statistical checks), computes satisfaction scores, feeds failures back to Code Generator for remediation

**Collaborators:**

- Configuration Manager (reads scenario directory, satisfaction threshold, judge model)
- Domain Service Client (executes scenario trajectories via domain service's `simulate` method)
- LLM Gateway (for LLM-as-judge evaluation of acceptance criteria)
- Twin Provisioner (starts/stops Digital Twin instances)
- Audit Recorder (logs scenario results)

**Roles:**

- Scenario selector: Determines which scenarios apply to the current sub-work-item based on interface coverage
- Twin orchestrator: Provisions and manages Digital Twin instances for scenario execution
- Trajectory executor: Runs each scenario multiple times with fresh state
- Judge coordinator: Evaluates acceptance criteria using deterministic assertions, LLM-as-judge, or statistical checks as appropriate
- Scorer: Computes per-scenario and overall satisfaction scores
- Feedback provider: Feeds failing scenarios and observed behaviors back to Code Generator

**Key behavior:**

- Only runs for sub-work-items with applicable scenarios (others skip this stage)
- Each trajectory runs in isolation (fresh twin state)
- Satisfaction score must meet threshold (default 0.95) to proceed
- Any explicit failure criterion violation fails the sub-work-item immediately

---

## Audit Recorder

Records all pipeline activity for traceability and debugging.

**Responsibilities:**

- Knows: Audit event schema, formatting conventions, constitutional event types
- Does: Records LLM calls (model, input hash, output, tokens, latency), validation results, state transitions, cost data, scenario validation results (satisfaction scores, trajectory outcomes), constitutional layer events (INJECTION_DETECTED, SCOPE_UNDERSPECIFIED, SCOPE_AMBIGUOUS, PROTECTED_PATH_VIOLATION), Context Pack loading events (which packs loaded, trigger matches); writes audit trail to GitHub

**Collaborators:**

- GitHub Client (posts audit comments or creates artifacts)

**Roles:**

- Logger: Records every significant event in the pipeline
- Formatter: Produces human-readable summaries from structured data
- Writer: Persists audit trail to GitHub (issue comments or artifacts)
- Safety event recorder: Records all constitutional layer events with full context for post-hoc review

---

## Context Pack Loader

Loads domain knowledge packs based on work item classification. Contains zero LLM logic.

**Responsibilities:**

- Knows: Context Pack directory structure, trigger file schema, well-known pack path (default: `.cogworks/context-packs/`), trigger matching rules
- Does: Scans available packs, evaluates each pack's trigger definition against the work item's classification labels, component tags, and safety classification, loads matching packs (domain knowledge, safe patterns, anti-patterns, required artefacts), reports loaded packs to audit trail

**Collaborators:**

- Configuration Manager (reads pack directory path)
- GitHub Client (reads pack files from repository)
- Audit Recorder (records which packs were loaded and why)

**Roles:**

- Scanner: Discovers available Context Packs from the configured directory
- Trigger evaluator: Matches trigger rules against classification output deterministically
- Loader: Reads and parses pack contents into structured domain knowledge
- Reporter: Reports loaded packs for audit and PR description inclusion

**Key behavior:**

- Loading is deterministic: same classification always loads same packs
- Multiple packs may match simultaneously
- A matched pack is always loaded (no option to skip)
- Trigger evaluation is a pure function: classification data in, list of matched packs out

---

## Constitutional Rules Loader

Loads and validates the constitutional rules file. Contains zero LLM logic.

**Responsibilities:**

- Knows: Constitutional rules file path (default: `.cogworks/constitutional-rules.md`), file format expectations, minimum required rule set
- Does: Reads constitutional rules from the well-known path, validates that the file exists and contains the required core rules, validates that the file comes from a reviewed/merged branch (not an unreviewed branch), produces the rules payload to be injected into the LLM system prompt

**Collaborators:**

- GitHub Client (reads constitutional rules file, verifies branch/merge status)
- Configuration Manager (reads constitutional rules file path)

**Roles:**

- Loader: Reads the constitutional rules document
- Validator: Ensures required rules are present and the source is from a reviewed branch
- Formatter: Produces the system prompt component for injection by LLM Gateway

**Key behavior:**

- Loading is unconditional — runs on every pipeline invocation
- Failure to load (file missing, validation failed, unreviewed source) halts the pipeline immediately
- The loaded rules are treated as immutable for the duration of the pipeline run
- No content in the context package can override the rules

---

## Injection Detector

Analyzes external content for prompt injection patterns. May use heuristic and/or LLM-based detection.

**Responsibilities:**

- Knows: Known injection patterns (persona overrides, instruction injections, behavioral modifications), detection heuristics, LLM-based detection prompt (if used)
- Does: Scans external content (issue bodies, specs, dependency docs, API responses) for injection patterns, emits `INJECTION_DETECTED` event when patterns found, triggers pipeline halt and hold state

**Collaborators:**

- Constitutional Rules Loader (provides the boundary definition between instructions and data)
- Audit Recorder (records injection detection events with full context)
- Pipeline Executor (receives halt signal)

**Roles:**

- Pattern scanner: Checks external content against known injection patterns
- Event emitter: Produces structured INJECTION_DETECTED events with source document, offending text
- Halt trigger: Signals the pipeline to stop and put the work item into hold state

**Key behavior:**

- Invoked before external content is included in any LLM prompt
- Detection triggers immediate pipeline halt (not a warning)
- Work item enters hold state — no automatic requeue
- False positive resolution requires explicit human review with justification recorded

---

## Scope Enforcer

Validates that generated artifacts stay within the approved specification scope.

**Responsibilities:**

- Knows: Approved specification scope, interface document, authorised file set (derived from spec and interface documents), protected path patterns
- Does: Validates generated artifacts against the authorised file set, checks for unauthorized capabilities (network calls, file system access, IPC, etc.), validates generated files do not match protected path patterns, emits SCOPE_UNDERSPECIFIED or SCOPE_AMBIGUOUS events when scope issues are detected

**Collaborators:**

- Configuration Manager (reads protected path patterns)
- Audit Recorder (records scope enforcement events)
- Pipeline Executor (receives halt signal for scope violations)

**Roles:**

- File set validator: Checks generated files against the authorised file set
- Capability scanner: Detects unauthorized capabilities in generated artifacts
- Protected path checker: Ensures no generated file matches protected patterns (pre-PR validation)
- Event emitter: Produces structured scope violation events

**Key behavior:**

- Runs before PR creation to catch scope violations
- SCOPE_UNDERSPECIFIED: specification incomplete for the work item's needs — halt and request human input
- SCOPE_AMBIGUOUS: safety-affecting specification is ambiguous — halt and request human clarification
- PROTECTED_PATH_VIOLATION: generated artifacts touch protected paths — fail pre-PR validation
