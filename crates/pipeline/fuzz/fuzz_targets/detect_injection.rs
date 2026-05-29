#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz target for `detect_injection`.
//
// Splits the input byte slice at the midpoint to produce two arbitrary string
// slices: one used as `content` and one as `source_label`. Both are derived
// via `from_utf8_lossy` so that all possible byte sequences reach the scanner
// without the harness ever panicking on invalid UTF-8.
//
// Goal: confirm that `detect_injection` never panics on any input.
fuzz_target!(|data: &[u8]| {
    let mid = data.len() / 2;
    let content = String::from_utf8_lossy(&data[..mid]);
    let source_label = String::from_utf8_lossy(&data[mid..]);
    let _ = pipeline::security::detect_injection(&content, &source_label);
});
