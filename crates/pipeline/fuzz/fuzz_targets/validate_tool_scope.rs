#![no_main]

use libfuzzer_sys::fuzz_target;
use pipeline::identifiers::ToolName;
use pipeline::knowledge::ScopeParameters;
use pipeline::security::{ToolParams, validate_tool_scope};
use std::collections::HashMap;

// Fuzz target for `validate_tool_scope`.
//
// Input layout (all conversions via `from_utf8_lossy` — no panic on bad UTF-8):
//
//   Byte 0:  control byte
//     bit 0    → whether to set max_file_changes
//     bit 1    → whether the param value is a string (1) or a number (0)
//     bits 2-7 → raw value for max_file_changes (saturating +1 so it is never 0)
//   Bytes 1..: divided into 5 equal parts
//     part 0 → tool name
//     part 1 → param key (empty key skips inserting any param)
//     part 2 → param value (string path or integer string)
//     part 3 → allowed artifact glob pattern for ScopeParameters
//     part 4 → prohibited artifact glob pattern for ScopeParameters
//
// Goal: confirm that `validate_tool_scope` never panics on any input,
// including malformed glob patterns and arbitrary JSON-value strings.
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let ctrl = data[0];
    let has_max = (ctrl & 0x01) != 0;
    let max_file_changes: Option<u32> = if has_max {
        Some(u32::from(ctrl >> 2).saturating_add(1))
    } else {
        None
    };
    let param_is_string = (ctrl & 0x02) != 0;

    let rest = &data[1..];
    if rest.is_empty() {
        return;
    }

    // Split into 5 parts
    let fifth = (rest.len() / 5).max(1);
    let s1 = fifth.min(rest.len());
    let s2 = (fifth * 2).min(rest.len());
    let s3 = (fifth * 3).min(rest.len());
    let s4 = (fifth * 4).min(rest.len());

    let tool_str = String::from_utf8_lossy(&rest[..s1]);
    let tool = match ToolName::new(tool_str.as_ref()) {
        Some(t) => t,
        None => return,
    };

    let param_key = String::from_utf8_lossy(&rest[s1..s2]).to_string();
    let param_raw = String::from_utf8_lossy(&rest[s2..s3]).to_string();
    let allowed_pattern = String::from_utf8_lossy(&rest[s3..s4]).to_string();
    let prohibited_pattern = String::from_utf8_lossy(&rest[s4..]).to_string();

    let mut params_map = HashMap::new();
    if !param_key.is_empty() {
        let value = if param_is_string {
            serde_json::Value::String(param_raw.clone())
        } else {
            param_raw
                .trim()
                .parse::<u64>()
                .map_or(serde_json::Value::Number(0.into()), |n| {
                    serde_json::Value::Number(n.into())
                })
        };
        params_map.insert(param_key, value);
    }

    let params = ToolParams { params: params_map };

    let scope = ScopeParameters {
        max_file_changes,
        allowed_artifact_patterns: if allowed_pattern.is_empty() {
            vec![]
        } else {
            vec![allowed_pattern]
        },
        prohibited_artifact_patterns: if prohibited_pattern.is_empty() {
            vec![]
        } else {
            vec![prohibited_pattern]
        },
        max_new_files: 0,
    };

    let _ = validate_tool_scope(&tool, &params, &scope);
});
