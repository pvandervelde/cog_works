# CogWorks Architecture Specification

This folder contains the living architecture specification for CogWorks — a deterministic orchestration system that manages autonomous code generation through a structured SDLC pipeline.

## How to Read This Spec

Start with **overview.md** for the big picture, then **vocabulary.md** to understand the domain language. From there, the documents can be read in any order depending on what you need.

| Document | Purpose |
|----------|---------|
| [overview.md](overview.md) | System context, scope, design philosophy, high-level data flow |
| [vocabulary.md](vocabulary.md) | Domain concepts with precise definitions and constraints |
| [requirements.md](requirements.md) | Functional requirements (REQ-* IDs) — what the system must do |
| [responsibilities.md](responsibilities.md) | CRC-style responsibility breakdown for each component |
| [architecture.md](architecture.md) | Clean architecture boundaries — business logic, abstractions, infrastructure |
| [assertions.md](assertions.md) | Behavioral assertions (Given/When/Then) for key system behaviors |
| [constraints.md](constraints.md) | Type system, module boundary, error handling, testing, and performance rules |
| [tradeoffs.md](tradeoffs.md) | Design alternatives evaluated with pros/cons |
| [testing.md](testing.md) | Strategy for testing CogWorks itself |
| [security.md](security.md) | Threat model and mitigations |
| [edge-cases.md](edge-cases.md) | Non-standard flows and failure modes |
| [operations.md](operations.md) | Deployment model, monitoring, cost alerting, runbook |

## Source Requirements

Functional requirements are defined in [requirements.md](requirements.md), which catalogues ~50 requirements across 14 categories (PIPE, CLASS, ARCH, PLAN, CODE, SCEN, REVIEW, INT, AUDIT, BOUND, DTU, EXT, XDOM, XVAL). Each `REQ-*` identifier in `assertions.md` traces to a corresponding entry in `requirements.md`.

## Key Capabilities

CogWorks provides advanced validation, context management, and extensibility capabilities:

1. **Scenario Validation** — Probabilistic behavior testing using scenarios held out from code generation context. Scenarios define desired behaviors as trajectories with satisfaction scores (default 0.95 threshold). This validates that code exhibits expected behaviors without overfitting to test cases.

2. **Digital Twin Universe** — High-fidelity behavioral clones of external dependencies, built using the same CogWorks pipeline. Twins enable testing against realistic external system behaviors without network calls or rate limits.

3. **Pyramid Summaries** — Multi-level context representation (L1: one-line, L2: paragraph, L3: full interface, L4: source) that enables efficient LLM context assembly. Dependencies further from the work scope get higher-level summaries; direct dependencies get full source.

4. **Domain Generalisation** — CogWorks is domain-ignorant. It does not contain code for any specific domain. Domain services (software, electrical, mechanical, etc.) are external processes that communicate through the Extension API. The Rust domain service ships as a reference implementation.

5. **Cross-Domain Interface Registry** — A version-controlled, human-authored repository of interface contracts that span domains (CAN bus, power rails, mounting points, etc.). Enables deterministic validation that changes in one domain respect constraints from others.

6. **Extension API** — A protocol for external domain services to register with and be invoked by CogWorks. Supports Unix domain sockets (default) and HTTP/gRPC. Capabilities are discovered dynamically via handshake. Standardised diagnostic categories and error codes enable consumers to process results generically across domains. Any team can build a domain service without modifying CogWorks.

7. **Capability Profiles** — Machine-readable definitions of what a domain service for a specific engineering domain must provide (required methods, artifact types, interface types). Profiles are a development-time tool for domain service authors and a conformance-testing artifact — CogWorks does not enforce profiles at runtime.

## Key Architectural Decisions

1. **CLI-first execution model** — each invocation is a stateless step function; service modes are additive wrappers
2. **GitHub as sole durable state** — no local database; pipeline state reconstructed from labels, issues, PRs
3. **Deterministic-first** — LLMs invoked only where reasoning/synthesis is genuinely required
4. **Result-based error handling** — business errors are values; exceptions only for unrecoverable infrastructure failures
5. **Domain-ignorant orchestrator** — CogWorks contains no domain-specific code; all domain operations delegated to external domain services via Extension API
6. **Domain services as external processes** — domain services run as separate binaries communicating over Unix sockets (default) or HTTP/gRPC; no built-in privileged path
7. **Clean architecture** — business logic depends only on abstractions; infrastructure implementations are swappable
8. **Cross-domain interface registry** — human-authored, version-controlled interface contracts enable deterministic cross-domain constraint validation
