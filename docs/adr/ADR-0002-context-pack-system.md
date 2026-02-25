# ADR-0002: Context Pack System for Domain Knowledge

**Status:** Accepted
**Date:** 2026-02-24
**Deciders:** Architecture

---

## Context

CogWorks generates code across multiple engineering domains — embedded firmware, electrical design, mechanical design — each with domain-specific conventions, safety patterns, and anti-patterns that an LLM cannot reliably discover from repository context alone.

Currently, each pipeline run assembles context from the repository's source code, ADRs, coding standards, and architectural constraints. This context is technical but generic — it does not include domain-specific knowledge such as:

- Rust `no_std` safety patterns and allocation constraints for embedded targets
- Swerve drive kinematics coordinate conventions and force analysis requirements
- CAN bus protocol timing constraints and error frame handling patterns
- ROS2 real-time executor model constraints and memory allocation restrictions
- ISO 9001 traceability requirements for safety-affecting work items

Without this domain knowledge loaded before generation, the LLM produces code that may be syntactically correct but physically or architecturally wrong in ways that static analysis and even LLM review cannot reliably catch. The gap widens as CogWorks operates more autonomously on safety-critical components.

---

## Decision

CogWorks will support a **Context Pack** system: structured, version-controlled directories of domain knowledge loaded deterministically at the Architecture stage (Stage 2), before any code generation begins.

### Pack Structure

Each Context Pack is a directory at a well-known path (default: `.cogworks/context-packs/<pack-name>/`) containing:

- **trigger.toml** — Declares when the pack is loaded (matching component tags, issue labels, and/or safety classification)
- **domain-knowledge.md** — Core domain knowledge, conventions, and constraints
- **safe-patterns.md** — Recommended patterns with rationale
- **anti-patterns.md** — Patterns to avoid with explanations of why each is unsafe
- **required-artefacts.toml** — Artefacts that must be present in pipeline output for the pack's domain requirements to be satisfied

### Trigger File Schema

`trigger.toml` is a structured TOML file with the following schema. All top-level condition fields are optional; a pack is loaded when **all specified conditions are satisfied** (logical AND across condition types; logical OR within a list value).

```toml
# trigger.toml — example for the rust-embedded-safety pack

[trigger]
# Pack is loaded if the work item has ANY of these component tags (OR matching within the list).
component_tags = ["firmware", "bootloader"]

# Pack is loaded if the work item has ANY of these issue labels (OR matching within the list).
labels = ["component:firmware", "component:bootloader"]

# Pack is loaded if the work item's safety classification matches.
# If true, the pack loads for any safety-affecting work item regardless of tags/labels.
# If false (or omitted), safety classification is not a trigger criterion.
safety_affecting = true

# Pack is loaded if the primary language matches any entry in the list (OR matching).
# Optional field; omit if the pack is language-agnostic.
languages = ["rust"]
```

**Matching semantics:**

- `component_tags` and `labels` both use OR matching within the list — the pack loads if the work item has *any* matching entry.
- Conditions across different fields are combined with AND — specifying both `component_tags` and `safety_affecting = true` loads the pack only for safety-affecting work items that also have a matching component tag.
- A `trigger.toml` with no fields defined is invalid (silently skipping an unconstrained pack would load it for every work item — a misconfiguration, not an intent).

### Reference Documents

Context Pack content (`domain-knowledge.md`, `safe-patterns.md`, `anti-patterns.md`) may reference external documents for additional background. Referenced documents are not automatically loaded — only the pack files themselves are included in the context window. If a referenced document is intended as the primary source for a section, its key content must be summarised directly in the pack file.

Documents in `docs/standards/` are candidate source material for pack domain knowledge. They follow the same content lifecycle as context packs (version-controlled, reviewed in PRs) but are maintained separately to allow multiple packs to reference the same standards document. Once completed, their content is incorporated into the relevant pack's `domain-knowledge.md` rather than loaded by reference.

### Loading Rules

1. **Deterministic selection** — Pack loading is driven by the work item's classification labels, component tags, and safety classification. The LLM does not choose which packs to load.
2. **Multiple simultaneous packs** — A work item may trigger multiple packs (e.g., `rust-embedded-safety` + `can-bus-protocol` for a CAN firmware change).
3. **Conflict resolution** — Where packs contain contradictory guidance, the more restrictive rule applies.
4. **Unconditional when matched** — If a work item matches a pack's trigger, that pack is loaded. There is no option to skip it.

### Required Artefact File Schema

`required-artefacts.toml` declares the artefacts the Review stage must verify are present in the pipeline output. Each artefact entry has three fields:

```toml
# required-artefacts.toml — example for the rust-embedded-safety pack

[[artefact]]
# Human-readable name used in blocking findings when the artefact is missing.
name = "Unsafe block justification"

# Type classifies how presence is verified.
# "file" — a file matching the path_pattern must exist in the generated output.
# "section" — a Markdown section matching the heading_pattern must exist in the named file.
# "annotation" — every instance of code_pattern in the output must have an adjacent annotation_pattern.
type = "annotation"

# code_pattern: regex matched against generated source files. Required for type "annotation".
code_pattern = 'unsafe\s*\{'

# annotation_pattern: regex that must appear within N lines of each code_pattern match.
# Required for type "annotation".
annotation_pattern = '# SAFETY:'

[[artefact]]
name = "Panic path analysis document"
type = "file"

# path_pattern: glob matched against the generated output file set. Required for type "file".
path_pattern = "docs/safety/panic-path-analysis*.md"

[[artefact]]
name = "Stack usage analysis"
type = "section"

# file_pattern: glob identifying which output file to inspect. Required for type "section".
file_pattern = "docs/safety/*.md"

# heading_pattern: regex matched against Markdown headings (## ...) in the target file.
heading_pattern = 'Stack (Usage|Budget) Analysis'
```

**Verification semantics:**

- `file` — at least one file in the generated output matches the glob. Missing → blocking finding.
- `section` — at least one file matching `file_pattern` contains a heading matching `heading_pattern`. Missing → blocking finding.
- `annotation` — every occurrence of `code_pattern` in the generated source has `annotation_pattern` within 5 lines (configurable). Any unadorned occurrence → blocking finding.
- Malformed `required-artefacts.toml` (missing required fields for the declared type) is a configuration error reported at pack load time, not at review time.

### Required Artefact Enforcement

Each pack may declare required artefacts. At the Review stage, CogWorks verifies all declared artefacts are present. Missing artefacts produce blocking findings identifying the pack and the missing artefact.

---

## Consequences

### Positive

- **Domain knowledge is structured and versioned** — Changes to domain knowledge are tracked in git, reviewed in PRs, and traceable to the pipeline runs they influenced.
- **Reduced domain error rate** — The LLM receives explicit guidance about domain conventions before generating code, reducing the probability of physically incorrect outputs.
- **Extensible** — New packs can be added without modifying CogWorks pipeline code. Any team can contribute a context pack for their domain.
- **Auditable** — The set of loaded packs is recorded in the audit trail and PR description.

### Negative

- **Pack maintenance burden** — Domain knowledge documents must be kept current. Stale or incorrect pack content could actively mislead the LLM.
- **Context window pressure** — Pack content consumes context window tokens. Multiple packs for a complex work item may require aggressive truncation of other context.
- **Trigger definition complexity** — Trigger rules must be precise enough to load the right packs without false positives.

---

## Alternatives Considered

### Alternative A: Embed domain knowledge in prompt templates

Include domain-specific guidance directly in stage prompt templates (e.g., a Rust-specific code generation template).

**Rejected because:** Prompt templates become unwieldy with embedded domain knowledge. Templates are stage-specific; domain knowledge is cross-cutting. Mixing them couples template evolution to domain knowledge evolution.

### Alternative B: LLM-inferred knowledge selection

Let the LLM decide which domain knowledge to load based on the work item description.

**Rejected because:** Non-deterministic selection is unacceptable for safety-critical domains. A missed pack could mean missing safety patterns. Deterministic selection based on classification labels ensures completeness and auditability.

### Alternative C: Runtime knowledge retrieval (RAG)

Use retrieval-augmented generation to dynamically fetch relevant domain knowledge from a knowledge base.

**Rejected because:** RAG retrieval is non-deterministic and may miss critical domain knowledge. For safety-critical domains, complete and predictable knowledge loading is required. RAG may be useful as a supplement in the future but cannot replace deterministic pack loading.
