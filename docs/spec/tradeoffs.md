# Design Tradeoffs

This document records the significant design alternatives considered and the rationale for each decision.

---

## 1. GitHub as Sole Durable State vs. Local Database

**Decision**: GitHub is the only durable state store.

| Factor | GitHub-Only | Local Database |
|--------|-------------|----------------|
| **Crash recovery** | Trivial — restart and read GitHub | Requires DB recovery, migration, backup |
| **Observability** | Full state visible in GitHub UI | State split between GitHub and DB |
| **Deployment** | Single binary, no DB dependency | Binary + database (Postgres/SQLite) |
| **API cost** | High — state reconstruction requires many API calls per invocation | Low — fast local reads |
| **Concurrency** | Labels as locks (race condition window) | Proper transactions |
| **Query capability** | Limited — GitHub search and label filtering | Full SQL queries |

**Rationale**: The simplicity and observability benefits outweigh the API cost. CogWorks processes a small number of work items (not thousands), so the API budget is sufficient. The race condition window for label-based locks is acceptable at expected concurrency levels (< 10 simultaneous pipelines). If API costs become a problem, ephemeral in-memory caching within a single invocation can reduce calls without adding persistent state.

**Risk**: GitHub API rate limit (5000 req/hr authenticated) could become a bottleneck with many concurrent pipelines. Mitigation: efficient API usage, batched reads, and proactive rate limit tracking from response headers.

---

## 2. CLI-First vs. Service-First Execution Model

**Decision**: CLI-first. Each invocation is a stateless step function.

| Factor | CLI-First | Service-First |
|--------|-----------|---------------|
| **Simplicity** | Each invocation is independent | Needs connection pools, graceful shutdown, health checks |
| **Testability** | Test one step at a time | Must test lifecycle, concurrency, state management |
| **Crash recovery** | Restart and re-invoke | Must handle in-flight requests, reconnection |
| **Resource efficiency** | Reconstruction cost per invocation | Persistent connections, amortized setup |
| **Latency** | Cold start on each invocation | Hot paths for frequent operations |
| **Future service mode** | Add loop/event handler around step function | Already done |

**Rationale**: CLI-first is simpler to build, test, and debug. The reconstruction cost (reading GitHub state) is a few API calls — negligible compared to LLM latency. Service mode is additive: poll mode wraps the step function in a timer loop; webhook mode wraps it in an HTTP handler. The core logic is identical in all modes.

**Constraint**: Component construction must be explicit (dependency injection, not global state) so that a service wrapper can construct long-lived components with connection pools.

---

## 3. Single CogWorks Binary + External Domain Services vs. Microservices

**Decision**: Single Rust binary for CogWorks orchestrator, with domain services as separate external processes.

| Factor | Single Binary + External Services | Full Microservices |
|--------|--------------------------------------|---------------------|
| **Deployment** | CogWorks = one binary, domain services = separate binaries (one per domain) | Multiple artifacts with complex orchestration |
| **Type safety** | Shared types within CogWorks; JSON Schema at Extension API boundary | Serialization at every boundary |
| **Extensibility** | New domains added by implementing Extension API, no CogWorks changes | New services added but require orchestration changes |
| **Latency** | Function calls within CogWorks; socket/HTTP to domain services | Network calls everywhere |
| **Independent evolution** | Domain services evolve independently via versioned API | All services evolve independently |
| **Complexity** | Moderate — Extension API protocol + health checking | High — service discovery, circuit breakers, distributed tracing |

**Rationale**: CogWorks does not need to be split into microservices internally — it's bounded by LLM API and domain service throughput, not internal compute. The Extension API provides the extensibility boundary: domain services are external processes that can be written in any language and deployed independently. This gives the extensibility benefits of microservices (add new domains without changing CogWorks) with the simplicity of a monolithic orchestrator.

**Key constraint**: The Rust domain service MUST use the Extension API like any third-party service. No built-in privileged path. If the API is insufficient, improve the API.

---

## 4. Sequential vs. Parallel Sub-Work-Item Processing

**Decision**: Sequential, in topological dependency order.

| Factor | Sequential | Parallel |
|--------|------------|----------|
| **Context quality** | Each SWI has all prior outputs | Independent SWIs lack each other's outputs |
| **Complexity** | Simple loop | Dependency graph scheduling, merge conflicts |
| **Cost** | Optimal — each SWI benefits from prior context | May generate redundant/conflicting code |
| **Speed** | Slower for independent SWIs | Faster when SWIs are independent |

**Rationale**: The spec explicitly requires sequential processing (REQ-CODE-001). The quality benefit is significant — each sub-work-item benefits from seeing the actual implementation of its dependencies, not just the interface. Parallel processing could be added later for independent sub-work-items (no dependency between them), but is not needed for initial release.

---

## 5. Labels as Concurrency Locks vs. External Coordination

**Decision**: GitHub labels as lightweight processing locks.

| Factor | Labels | External (Redis, etcd) |
|--------|--------|------------------------|
| **Infrastructure** | None — uses existing GitHub | Additional service to deploy and maintain |
| **Correctness** | Race condition window between check and set | Proper atomic compare-and-set |
| **Observability** | Visible in GitHub UI | Separate monitoring |
| **Failure mode** | Stale lock if process crashes | Lock TTL with automatic expiry |

**Rationale**: At expected concurrency levels (< 10 pipelines), the probability of a label race condition is negligible. The check-and-set window is milliseconds. If a process crashes with the lock held, a human (or a subsequent invocation with a staleness check) can remove the label. External coordination adds infrastructure complexity that is not justified by the risk.

**Mitigation for stale locks**: The processing label should include a timestamp (e.g., in a corresponding issue comment). If a lock is older than a configurable timeout (default: 30 minutes), it can be considered stale and overridden.

---

## 6. Working Copy Management: Domain Service-Owned vs. CogWorks-Owned

**Decision**: Domain services manage their own working copies. CogWorks provides shared libraries.

| Factor | Domain Service-Owned | CogWorks-Owned | Hybrid (old) |
|--------|---------------------|----------------|---------------|
| **Toolchain support** | Full support (service controls its own FS) | Full, but CogWorks must know file paths | Full |
| **Remote services** | Works (service clones remotely) | Broken (CogWorks clone is local only) | Local only |
| **Isolation** | Each service has independent clone | Shared clone = potential conflicts | Shared clone |
| **Complexity** | Services handle their own cloning | CogWorks handles cloning centrally | Split responsibility |
| **Shared libraries** | CogWorks provides optional clone library | Required CogWorks logic | CogWorks logic |

**Rationale**: Since domain services are external processes, CogWorks cannot assume they have filesystem access to a CogWorks-managed clone. Especially for future remote domain services (HTTP/gRPC), the clone must be on the service's side. CogWorks provides repository information (URL, branch, ref) via the Extension API context, and domain services manage their own clones. A shared library for common git operations (shallow clone, branch management, cleanup) is published for convenience but is optional.

**Change from previous design**: Previously, CogWorks managed the working copy centrally with a hybrid GitHub API + git clone approach. The domain generalisation shifts clone responsibility to domain services, enabling remote services and better isolation.

---

## 7. LLM Output Validation: JSON Schema vs. Type-Driven Parsing

**Decision**: JSON Schema for validation, with typed parsing after validation.

| Factor | JSON Schema | Direct Type Parsing (serde) |
|--------|-------------|----------------------------|
| **Declarative** | Schema files are human-readable and auditable | Validation embedded in code |
| **Version control** | Schema changes are visible in diffs | Type changes require code review |
| **LLM guidance** | Schemas can be included in prompts to guide output | Types are internal to CogWorks |
| **Performance** | Two-pass (validate then parse) | Single-pass |
| **Flexibility** | Can express constraints beyond types (min/max, patterns) | Limited to what types express |

**Rationale**: JSON Schemas serve double duty — they validate LLM output and can be included in prompts to guide the LLM toward producing valid output. The two-pass overhead (validate against schema, then deserialize into types) is negligible compared to LLM latency.

---

## 8. Four Review Passes (One Deterministic + Three LLM) vs. One Combined Review

**Decision**: Four review passes: one deterministic (cross-domain constraint validation) + three LLM passes (quality, architecture, security).

| Factor | Four Passes (1 deterministic + 3 LLM) | One Combined Pass |
|--------|--------------------------------------|------------------|
| **Focus** | Each pass has a narrow focus → higher quality findings | Broad prompt → may miss depth |
| **Cost** | 3x LLM calls per sub-work-item (1 deterministic pass is free) | 1x LLM call |
| **Prompt size** | Smaller, focused prompts | Larger prompt with multiple concerns |
| **Independent evolution** | Each pass can be tuned independently | One monolithic prompt |
| **Parallelism** | LLM passes can run in parallel (future) | N/A |

**Rationale**: The spec requires four passes — one deterministic constraint check followed by three separate LLM passes — (REQ-REVIEW-002). The focused approach produces better findings because each review prompt can include specific checklists and examples for its domain. The cost is bounded (3 LLM calls × sub-work-item count × remediation cycles), and the review stage uses a high-quality model anyway.

---

## 9. Anthropic API vs. Multi-Provider from Day One

**Decision**: Anthropic API initially, with a provider-agnostic trait for future expansion.

| Factor | Anthropic Only | Multi-Provider |
|--------|---------------|----------------|
| **Simplicity** | One API to implement and test | Multiple APIs, response normalization |
| **Cost optimization** | N/A | Route different stages to cheapest provider |
| **Resilience** | Single point of failure | Failover between providers |
| **Time to market** | Faster | Slower |

**Rationale**: The LLM Provider trait is provider-agnostic. The initial implementation targets Anthropic. Adding providers later requires only a new trait implementation, not changes to business logic. This is the correct order — build one provider well, then add more.

---

## 10. External Process Domain Services vs. In-Process Trait Implementations

**Decision**: Domain services are external processes communicating via Extension API.

| Factor | External Process | In-Process Trait |
|--------|-----------------|-----------------|
| **Language freedom** | Domain services can be any language | Must be Rust (or use FFI) |
| **Extensibility** | New domains without CogWorks changes | Requires CogWorks rebuild/release |
| **Isolation** | Process-level isolation (no shared memory) | Shared address space |
| **Latency** | Socket/HTTP overhead (~1ms per call) | Function call overhead (~1μs) |
| **Security** | CogWorks doesn't need domain toolchains | CogWorks process has all toolchains |
| **Debugging** | Two processes to inspect | Single process |
| **Deployment** | Multiple binaries to distribute | Single binary |

**Rationale**: The extensibility benefit is decisive. CogWorks must support domains beyond software (electrical, mechanical, etc.) without source changes. External processes allow domain services to be written by different teams in different languages, evolve independently, and be deployed independently. The socket overhead is negligible compared to the operations domain services perform (compilation, simulation, etc. take seconds to minutes). The Rust domain service serves as the reference implementation that validates the Extension API is complete and usable.

**Risk**: The Extension API must be sufficient for all domain service needs. If the API is missing capabilities, domain services are blocked. Mitigation: the Rust domain service is built first and uses the same API, surfacing gaps early.

---

## 11. Unix Domain Socket vs. HTTP/gRPC as Default Transport

**Decision**: Unix domain socket as default, HTTP/gRPC as optional alternative.

| Factor | Unix Socket | HTTP/gRPC |
|--------|------------|-----------|
| **Latency** | ~0.1ms per message | ~1-5ms per message |
| **Remote services** | Local only | Works across machines |
| **Authentication** | File permissions | Need auth mechanism (tokens, mTLS) |
| **Tooling** | Limited debugging tools | curl, Postman, grpcurl |
| **Ecosystem** | Standard on Linux/macOS, limited on Windows | Universal |
| **Complexity** | Simple | More infrastructure (ports, TLS, etc.) |

**Rationale**: For the initial deployment (single machine, co-located services), Unix sockets are simpler, faster, and naturally secured by file permissions. HTTP/gRPC support is configurable per domain service for future remote deployment scenarios. The transport layer is behind an abstraction, so adding new transports does not change business logic.

**Windows consideration**: Named pipes provide equivalent functionality to Unix domain sockets. The transport abstraction must accommodate platform differences.

---

## 12. Polling vs. Streaming for Long-Running Domain Service Operations

**Decision**: Polling initially, designed for future streaming.

| Factor | Polling | Streaming |
|--------|---------|-----------|
| **Simplicity** | Simple request/response + poll endpoint | Bidirectional stream management |
| **Progress visibility** | Periodic polling for updates | Real-time progress |
| **Transport compatibility** | Works with any transport | Requires streaming-capable transport |
| **Resource usage** | Polling wastes cycles when no progress | Efficient push-based |
| **Implementation** | Simpler domain service implementation | More complex event emitting |

**Rationale**: Most domain service operations complete in seconds to minutes. Polling with a reasonable interval (e.g., every 5 seconds) is sufficient. The protocol is designed with an `operation_id` concept that can be extended to support server-sent events or bidirectional streaming when needed. Domain services are not required to implement polling — synchronous request/response is the baseline.

---

## 13. CogWorks-Mediated vs. Direct Cross-Domain Communication

**Decision**: CogWorks mediates all cross-domain interactions. Domain services never communicate directly.

| Factor | CogWorks-Mediated | Direct Domain-to-Domain |
|--------|-------------------|------------------------|
| **Coupling** | Domain services independent | Services must discover each other |
| **Observability** | All interactions visible in CogWorks audit trail | Distributed tracing needed |
| **Control** | CogWorks enforces ordering and validation | Services manage their own coordination |
| **Scalability** | CogWorks is potential bottleneck | Services scale independently |
| **Complexity** | Simpler domain services | More complex service infrastructure |

**Rationale**: Domain services should not need to know about each other. The interface registry provides the shared vocabulary, and CogWorks provides the relevant contract parameters to each service via the Extension API context. This keeps domain services simple and focused on their domain, with CogWorks handling all orchestration.

---

## 14. Deterministic vs. LLM-Based Injection Detection

**Decision**: Heuristic-only injection detection (LLM-based secondary pass deferred to future enhancement).

| Factor | Heuristic / Regex | LLM-Based |
|--------|------------------|-----------|
| **Speed** | Fast (sub-millisecond) | Slow (LLM latency) |
| **Determinism** | Deterministic — same input, same result | Non-deterministic |
| **Coverage** | Limited to known patterns | Theoretically broader |
| **Adversarial resistance** | Can be evaded by novel patterns | Better generalization |
| **Cost** | Free | Token cost per call |
| **False positives** | Pattern-dependent | May have different false positive profile |

**Rationale**: Heuristic detection (regex + known pattern matching) is applied first: it is fast, deterministic, and catches the majority of obvious injection attempts. The constitutional rules themselves are the true primary defense — even if detection misses an injection, the behavioral rules limit what the LLM can be induced to do.

**Key constraint**: Heuristic detection does not provide guarantees. The constitutional layer (system-level behavioral rules) is the true primary defense; detection is an early-warning system that enables halting before generation, not a correctness guarantee.

**LLM-based secondary pass**: An LLM-based secondary detection pass for borderline cases is *deferred as a future enhancement*. Injection Detection is classified as pure Business Logic (zero I/O), and an LLM secondary pass would require I/O, violating this boundary. If introduced in a future version, it must be delegated through an `InjectionDetector` abstraction (not inline I/O in business logic) and its non-determinism must be explicitly accepted.

---

## 15. Context Pack Loading: Architecture Stage vs. Every Stage

**Decision**: Context Packs are loaded once at the Architecture stage (Stage 2) and their content persists for the entire pipeline run.

| Factor | Load Once at Architecture | Load at Every Stage |
|--------|--------------------------|---------------------|
| **Consistency** | Same packs throughout run | Could vary per stage |
| **API calls** | One load per pipeline | Multiple loads |
| **Content currency** | Based on original classification | Could reflect updates |
| **Determinism** | Deterministic per run | Could change if packs are updated mid-run |
| **Auditability** | Single recorded pack set | Multiple pack sets to audit |

**Rationale**: Packs are loaded once and remain consistent for the entire pipeline run. This ensures that the code generator and reviewer see the same domain knowledge, anti-patterns, and required artefacts. Loading packs at each stage would complicate auditing (which packs were active when?) and could introduce inconsistency if a pack is updated between stages. The classification that triggers pack loading does not change during a pipeline run, so reloading would produce the same result anyway.

**Trade-off accepted**: Required artefact declarations from newly committed packs will not take effect for in-progress pipeline runs. This is acceptable — pack updates are rare and take effect on subsequent runs.

---

## 16. Constitutional Rules: System Prompt vs. Context Injection

**Decision**: Constitutional rules are injected as a privileged system prompt component, not as a regular context item.

| Factor | System Prompt | Context Item |
|--------|---------------|--------------|
| **Override resistance** | Non-overridable by design (model API) | Can be buried by subsequent context |
| **Position stability** | Always first, before any context | Depends on context assembly ordering |
| **Context window cost** | Fixed overhead per call | Counted against context budget |
| **Truncation resistance** | Not subject to truncation | Could be truncated under pressure |
| **Implementation** | Uses model API's system/user separation | Standard context assembly |

**Rationale**: The primary goal of the constitutional rules is that they cannot be overridden by external content. Placing them in the system prompt (separate from the user-role context) provides the strongest available boundary using the model API's own separation mechanism. A context-injected rule could theoretically be overridden or buried by subsequent items assembled from untrusted sources. System prompt placement is also not subject to context truncation — the rules are always present in full.

**Key constraint**: Constitutional rules token cost is not counted against the per-call context budget. They are overhead the system must absorb. This means effective context budget = model_context_window - constitutional_rules_tokens - output_reservation_tokens.
