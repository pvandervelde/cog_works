#![no_main]

use libfuzzer_sys::fuzz_target;
use pipeline::identifiers::ArtifactPath;
use pipeline::security::{validate_scope, ApprovedScope, ProtectedPath};

// Fuzz target for `validate_scope`.
//
// Input layout (all conversions via `from_utf8_lossy` — no panic on bad UTF-8):
//
//   Byte 0:  control byte
//     bits 0-1  → number of artifact paths to build (0–3)
//     bit  2    → whether to set a max_files limit
//     bits 3-7  → raw value for max_files (saturating +1 so it is never 0)
//   Bytes 1..: divided into equal chunks
//     chunks 0..n_artifacts → artifact path strings
//     next chunk            → allowed glob pattern for ApprovedScope
//     remaining bytes       → protected path glob pattern
//
// Goal: confirm that `validate_scope` never panics on any input, including
// malformed glob patterns and arbitrary path strings.
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let ctrl = data[0];
    let n_artifacts = (ctrl & 0x03) as usize;
    let has_max = (ctrl & 0x04) != 0;
    let max_files: Option<u32> = if has_max {
        Some(u32::from(ctrl >> 3).saturating_add(1))
    } else {
        None
    };

    let rest = &data[1..];
    if rest.is_empty() {
        let scope = ApprovedScope {
            artifact_patterns: vec![],
            max_files,
            max_new_files: 0,
        };
        let _ = validate_scope(&[], &scope, &[]);
        return;
    }

    // Divide rest into: n_artifacts artifact chunks + 1 allowed-pattern chunk + 1 protected-pattern chunk
    let n_sections = n_artifacts + 2;
    let chunk = (rest.len() / n_sections).max(1);

    let mut offset = 0;
    let mut artifacts = Vec::with_capacity(n_artifacts);
    for _ in 0..n_artifacts {
        let end = (offset + chunk).min(rest.len());
        let s = String::from_utf8_lossy(&rest[offset..end]);
        if let Some(ap) = ArtifactPath::new(s.as_ref()) {
            artifacts.push(ap);
        }
        offset = end;
        if offset >= rest.len() {
            break;
        }
    }

    let allowed_end = (offset + chunk).min(rest.len());
    let allowed_pattern = String::from_utf8_lossy(&rest[offset..allowed_end]).to_string();
    let protected_pattern = String::from_utf8_lossy(&rest[allowed_end..]).to_string();

    let approved_scope = ApprovedScope {
        artifact_patterns: if allowed_pattern.is_empty() {
            vec![]
        } else {
            vec![allowed_pattern]
        },
        max_files,
        max_new_files: 0,
    };

    let protected_paths = if protected_pattern.is_empty() {
        vec![]
    } else {
        vec![ProtectedPath {
            pattern: protected_pattern,
            reason: "fuzz".to_string(),
        }]
    };

    let _ = validate_scope(&artifacts, &approved_scope, &protected_paths);
});
