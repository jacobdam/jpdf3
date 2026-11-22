//! Arbitrary bytes into `Document::open`, the path behind `jpdf-cli info`.
//! Spec: specs/tooling/fuzz.md. Seeds: `cargo xtask fuzz` writes them from jpdf-testgen.
#![no_main]
#![forbid(unsafe_code)]

use jpdf_engine::{Document, Limits};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Any result is fine; a panic, abort, timeout or > 2 GiB RSS is the bug.
    let _ = Document::open(data, &Limits::default());
});
