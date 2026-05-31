#![no_main]
//! Fuzz target for [`pipeline::validate_constitutional_prompt`].
//!
//! Goal: confirm the function never panics on arbitrary input.
//!
//! Input layout (null-delimited UTF-8 parts; non-UTF-8 input is skipped):
//!   0: rules content (the constitutional document text)
//!   1: source_hash   (arbitrary string; a valid 64-char hex would pass the
//!                     hash check, but any value exercises the error paths)
//!   2: source_branch name
//!   3: system_prompt
//!   4: user_content

use libfuzzer_sys::fuzz_target;
use pipeline::{ApprovedBranches, BranchName, ConstitutionalRules, PromptAssembly, validate_constitutional_prompt};

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    let parts: Vec<&str> = s.splitn(6, '\0').collect();
    let content = parts.first().copied().unwrap_or("");
    let source_hash = parts.get(1).copied().unwrap_or("");
    let branch_raw = parts.get(2).copied().unwrap_or("master");
    let system_prompt = parts.get(3).copied().unwrap_or("");
    let user_content = parts.get(4).copied().unwrap_or("");

    let Some(source_branch) = BranchName::new(branch_raw) else {
        return;
    };

    let rules = ConstitutionalRules {
        content: content.to_string(),
        source_hash: source_hash.to_string(),
        source_branch,
    };
    let prompt = PromptAssembly {
        system_prompt: system_prompt.to_string(),
        user_content: user_content.to_string(),
    };

    // Must never panic regardless of input.
    let _ = validate_constitutional_prompt(&rules, prompt, &ApprovedBranches::default());
});
