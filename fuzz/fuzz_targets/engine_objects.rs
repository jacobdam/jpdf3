//! Fuzzed object bodies inside a file whose xref table is always correct
//! (`jpdf_testgen::from_fuzz_chunks`), so even an empty corpus reaches object parsing and the
//! page tree instead of failing at `startxref`. Spec: specs/tooling/fuzz.md.
#![no_main]
#![forbid(unsafe_code)]

use jpdf_engine::{Document, Limits};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let file = jpdf_testgen::from_fuzz_chunks(data);
    let _ = Document::open(&file, &Limits::default());
});
