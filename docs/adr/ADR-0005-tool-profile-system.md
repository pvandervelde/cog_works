# ADR-0005: Tool Profile System for LLM Node Access Control

**Status:** Accepted
**Date:** 2026-02-28
**Deciders:** CogWorks Architecture Team
**References:** COGWORKS-TOOLSCOPE-001, COGWORKS-TOOLSCOPE-002, ADR-0003 (Constitutional Security Layer), CW-R11, CW-R12, CW-R18

---

## Context

When the orchestrator invokes an LLM to execute a pipeline node, it provides tools — functions the LLM can call to interact with the environment (read files, write files, run commands, commit code). Without explicit scoping, every node has access to everything the orchestrator process can do. This creates risks:

- **Accidental damage**: The LLM deletes files, force-pushes, or modifies files outside its scope.
- **Scope creep**: A code generation node searches the web or reads files from other repositories.
- **Security**: Prompt injection (CW-R12) is more dangerous when the node has access to git push, network, or credential stores.
- **Reproducibility**: Web access or unrestricted file access makes outputs non-deterministic.
- **Cost**: Unnecessary tool access leads to unnecessary tool use, wasting tokens.

We considered three approaches for controlling tool access.

---

## Decision

### Internal Tool Profiles (Not MCPs) for Internal Capabilities

Tool profiles are a configuration layer within the orchestrator. The orchestrator maintains a registry of available tools. Each pipeline node's configuration declares which tools it needs. The orchestrator constructs the tool list for each LLM call by filtering the registry against the node's profile. Boundary enforcement is implemented in the tool functions themselves, parameterised by the node's scope configuration.

### Adapter Generation (Not MCPs) for External Service Integrations

External API integrations (Inventree, test database, Azure services) use generated adapters — tool definitions auto-generated from API specifications (OpenAPI, EAB schema). The orchestrator's HTTP executor handles the actual API calls. MCPs remain available as a fallback for services requiring bidirectional communication or streaming responses, but adapters are the default integration mechanism.

### Defence in Depth (Three Enforcement Layers)

1. **Layer 1 — Tool filtering**: The LLM can only see tools in its profile. Tools not in the profile are never offered.
2. **Layer 2 — Scope enforcement**: Each tool validates scope parameters before executing, independently of the orchestrator's filtering.
3. **Layer 3 — OS-level sandboxing**: Network namespaces, filesystem permissions, and cgroups prevent bypass of both application layers.

### Code Generation is the Only Core Node with Direct Write Access

Architecture, Interface Design, and Planning nodes produce structured output that the orchestrator writes. The LLM at those stages does not need `fs.write` — preventing it from writing arbitrary files under the guise of producing specs. Code Generation is the only core node where the LLM directly writes files, because code generation inherently requires iterative file creation and modification.

---

## Alternatives Considered

### MCP Servers for Internal Tools

| Factor | Tool Profiles | MCP Servers |
|--------|---------------|-------------|
| **Processes** | Zero additional | 5-6 per node invocation |
| **Serialisation** | Direct function calls | JSON-RPC over IPC |
| **Deployment** | Config in orchestrator | Separate binaries to build, deploy, health-check |
| **Scoping** | Native (profile config) | Each MCP must implement its own access control |

**Rejected because:** MCP's value is standardising access to external services with their own APIs and auth. For internal capabilities (filesystem, git, domain services), tool profiles are simpler and cheaper.

### MCPs for External Services (Original TOOLSCOPE-001 §9)

The original design proposed MCP servers for Inventree, test databases, and cloud services. This was superseded by adapter generation because:

- Adapters are declarative (no code to maintain) — generated from API specs
- Adapters integrate with the existing tool profile scoping system
- Most external integrations are request-response, not bidirectional
- One fewer protocol layer reduces complexity

MCPs are retained as a fallback for integrations that genuinely need bidirectional communication, server-initiated events, or streaming responses.

### No Tool Scoping (Trust the Constitutional Layer)

The constitutional layer (ADR-0003) declares scope rules, but declaration without enforcement is insufficient. Tool scoping provides the mechanism that makes constitutional declarations enforceable at the tool level.

---

## Consequences

### Positive

- **Blast radius reduction**: Prompt injection can only exploit tools in the node's profile.
- **Reproducibility**: Nodes without web search cannot produce non-deterministic outputs from web content.
- **Cost reduction**: Nodes only see relevant tools, reducing token waste on tool schema context.
- **Auditability**: Every tool call is scoped and logged, enabling pattern detection and skill crystallisation.
- **External integration without maintenance burden**: Adapter generation keeps tool definitions in sync with API specs.

### Negative

- **Configuration overhead**: Each node type needs a tool profile definition (mitigated by sensible defaults for all 7 core nodes).
- **Scope debugging**: When a node legitimately needs access it doesn't have, the developer must update the profile (mitigated by clear scope violation error messages).
- **Adapter generator complexity**: The OpenAPI/EAB parser is ~2,500 lines of implementation.

### Neutral

- **OS-level sandboxing is SHOULD for Phase 1**: Adds deployment complexity. Becomes MUST when processing safety-critical code or untrusted work items.
- **Skill crystallisation is Phase 2+**: Requires audit trail data from real pipeline runs before patterns can be extracted.

---

## Implementation Phasing

**Phase 1** (with MVP pipeline):

- Tool registry, profiles, scope enforcement
- Scoped filesystem, git, domain service tools
- Default profiles for all 7 core nodes
- Tool call audit logging
- EAB adapter generation (domain service tools generated from schema)
- Basic skill framework (manual skill creation, executor, `skill.run` tool)

**Phase 2** (after pipeline running):

- OpenAPI adapter generation (Inventree integration)
- Skill extraction CLI (analyse audit trail, propose skills)
- Skill performance tracking
- Scope violation alerting and usage reporting
- Progressive discovery with keyword matching
- Custom tool registration

**Phase 3** (optimisation):

- OS-level sandboxing for code generation nodes
- Embedding-based semantic search for progressive discovery
- GraphQL/gRPC adapter generators
- Automatic skill deprecation on success rate decline

---

## Related Decisions

- ADR-0003: Constitutional Security Layer (declarations that tool profiles enforce)
- ADR-0004: Graph-Structured Pipeline (tool profiles are per-node in the graph)
- CW-R11: Credential exposure or scope creep (tool profiles are a primary mitigation)
- CW-R12: Malicious work item injection (tool profiles limit blast radius)
- CW-R18: CogWorks modifies its own prompts (protected paths enforced at tool level)
