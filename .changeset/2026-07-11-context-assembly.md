---
"pipeline": minor
---

Implement context assembly functions (`select_context_packs`, `merge_pack_guidance`, `enforce_scenario_holdout`, `apply_priority_truncation`, `assemble_context`) in `crates/pipeline`.

These five functions complete the context assembly subsystem that prepares the information window fed to every LLM node call:

- **`select_context_packs`**: evaluates Context Pack triggers using glob matching against work item labels and affected module paths; OR semantics across trigger fields.
- **`merge_pack_guidance`**: union-merges safe patterns, anti-patterns, and required artifacts across all matched packs with path-based deduplication.
- **`enforce_scenario_holdout`**: removes scenario specification files from context items before code generation (hard safety constraint); returns an opaque `HoldoutFilteredItems` type that enforces correct call order at compile time.
- **`apply_priority_truncation`**: sorts items by `ContextPriority` (highest first, then alphabetical), greedily fills the token budget, and always includes required artifacts even on budget overflow.
- **`assemble_context`**: async orchestrator that fetches pyramid summaries from `SummaryCache`, adds interface definitions and pack guidance, enforces holdout, and applies priority truncation. `SummaryCache` errors cause artifact skip (not propagation) with the error recorded in `ContextPackage.assembly_errors`.

Also adds `PartialOrd` and `Ord` to `ArtifactPath` and all `string_id!` newtypes for deterministic sorting.

Security: upgrades `crossbeam-epoch` to 0.9.20 (RUSTSEC-2026-0204).
