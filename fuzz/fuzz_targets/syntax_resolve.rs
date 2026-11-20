//! `XrefDocument::parse`, then `resolve` on every object number the xref table could name and
//! on the trailer's `/Root`, in file order and again (re-reads must be safe).
//! Spec: specs/tooling/fuzz.md.
#![no_main]
#![forbid(unsafe_code)]

use jpdf_syntax::{Limits, ObjRef, XrefDocument};
use libfuzzer_sys::fuzz_target;

/// Object numbers tried per input; fuzz inputs are small, so real tables are shorter.
const MAX_NUM: u32 = 256;

fuzz_target!(|data: &[u8]| {
    let file = jpdf_testgen::from_fuzz_chunks(data);
    for bytes in [data, file.as_slice()] {
        let Ok(doc) = XrefDocument::parse(bytes, &Limits::default()) else {
            continue;
        };
        if let Some(root) = doc.trailer().get(b"Root").and_then(|o| o.as_ref()) {
            let _ = doc.resolve(root);
        }
        for _pass in 0..2 {
            for num in 0..MAX_NUM {
                let _ = doc.resolve(ObjRef { num, generation: 0 });
            }
        }
    }
});
