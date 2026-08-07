---
"pipeline": minor
---

Implement review aggregation and cross-domain constraint validation (`aggregate_review_results`, `validate_cross_domain_constraints`) in `crates/pipeline`.

These two functions complete the review gate subsystem that decides whether generated code proceeds, needs rework, or must be escalated to a human:

- **`aggregate_review_results`**: folds the Quality, Architecture, and Security review-pass results into a single `AggregateReviewDecision`. Any `Blocking` diagnostic from any pass forces the outcome away from `Proceed`; if the rework budget is exhausted (`remediation_count >= limit`) the decision escalates to a human instead of requesting another remediation pass, preventing infinite rework loops.
- **`validate_cross_domain_constraints`**: checks extracted interface definitions from a domain service against the human-maintained interface registry, reporting every mismatch — missing interfaces and field-level schema differences alike — in one pass instead of stopping at the first violation, so all cross-domain issues can be fixed together.

Security: while reviewing `validate_cross_domain_constraints`, a HIGH-severity denial-of-service issue was found and fixed. Schema comparison used `serde_json`'s default (recursive, unbounded) comparison and stringification on JSON content sourced from an untrusted domain-service response; an adversarially deep nested payload (~10,000+ levels) could overflow the stack and crash the entire CogWorks orchestrator process. A new `json_guard` module adds an iterative, depth-bounded check (`exceeds_max_depth`, max depth 64) that short-circuits comparison to a "depth exceeded" finding before any vulnerable operation runs. This is defense-in-depth only — the complete fix additionally requires enforcing depth/size limits at the Extension API deserialization boundary, which is not yet implemented and is tracked separately.
