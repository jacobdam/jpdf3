//! Tripwire for the #146 gate (specs/core/memory-budget.md, "Gate"): no decode or raster phase
//! runs on the jpdf-cli path until the cross-phase heap build-up is measured or fixed. Today the
//! path only parses, so its whole normal and build dependency graph is the parse-only crates
//! below. A change that pulls in a filter, backend or render crate (or any other crate) fails
//! here and must first satisfy the gate, then update this allowlist in the same PR.
//!
//! Limits: the graph check sees crates, not phases. Decode or raster code written inside an
//! allowed crate is caught only if it names one of the backend entry points in
//! `PHASE_ENTRY_POINTS` (a best-effort text scan); an in-crate decoder under another name passes.
//! Review of each PR against the gate covers that.

use std::collections::BTreeSet;
use std::process::Command;

/// Every crate the jpdf-cli binary may link while the #146 gate is closed.
const PARSE_ONLY: [&str; 5] = [
    "jpdf-cli",
    "jpdf-cos",
    "jpdf-engine",
    "jpdf-syntax",
    "jpdf-traits",
];

/// Names that start a decode or raster phase through jpdf-traits: the decode and raster limit
/// builders, the scene check, and the backend traits.
const PHASE_ENTRY_POINTS: [&str; 15] = [
    "decode_limits",
    "DecodeLimits",
    "raster_limits",
    "RasterLimits",
    "check_scene",
    "Rasterizer",
    "Inflate",
    "inflate",
    "FlateDecode",
    "JpegDecoder",
    "Jpeg2000Decoder",
    "Jbig2Decoder",
    "CcittDecoder",
    "FontEngine",
    "FontFace",
];

/// Source directories of the allowed crates other than jpdf-traits (which defines the names).
const SCANNED: [&str; 4] = [
    "src",
    "../../crates/jpdf-engine/src",
    "../../crates/jpdf-cos/src",
    "../../crates/jpdf-syntax/src",
];

fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            rust_files(&path, out)?;
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

#[test]
fn parse_crates_name_no_decode_or_raster_entry_point_until_the_146_gate_opens() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in SCANNED {
        rust_files(&root.join(dir), &mut files).expect("crate sources are readable");
    }
    // Fail closed: the scan really found the crates' sources.
    assert!(
        files
            .iter()
            .any(|f| f.ends_with("jpdf-syntax/src/lexer.rs")),
        "{files:?}"
    );
    let mut hits = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        for name in PHASE_ENTRY_POINTS {
            if text.contains(name) {
                hits.push(format!("{} names {name}", f.display()));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "{hits:?}: a decode or raster phase on the jpdf-cli path needs the #146 gate \
         (specs/core/memory-budget.md) first; then update this test"
    );
}

#[test]
fn jpdf_cli_links_only_parse_crates_until_the_146_gate_opens() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let out = Command::new(cargo)
        .args([
            "tree",
            "-p",
            "jpdf-cli",
            "-e",
            "normal,build",
            "--prefix",
            "none",
            "--offline",
            "--locked",
            // Every feature and every target: an optional or platform-only dependency counts.
            "--all-features",
            "--target",
            "all",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree runs");
    // Fail closed: a cargo tree failure is not an empty graph.
    assert!(
        out.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("cargo tree prints UTF-8");
    let crates: BTreeSet<&str> = stdout
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    // The graph is really listed (the binary's own crate and the parser are in it).
    assert!(
        crates.contains("jpdf-cli") && crates.contains("jpdf-syntax"),
        "{crates:?}"
    );
    let extra: Vec<&&str> = crates.iter().filter(|c| !PARSE_ONLY.contains(c)).collect();
    assert!(
        extra.is_empty(),
        "jpdf-cli now links {extra:?}. The #146 gate (specs/core/memory-budget.md) must be \
         satisfied before a decode or raster phase reaches the jpdf-cli path; then update \
         PARSE_ONLY in this test"
    );
}
