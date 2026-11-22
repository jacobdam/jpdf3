//! Tripwire for the #146 gate (specs/core/memory-budget.md, "Gate"; see there for the exact
//! conditions): no decode or raster phase runs on the jpdf-cli path until the gate opens. Today the
//! path only parses, so its whole normal and build dependency graph is the parse-only crates
//! below. A change that pulls in a filter, backend or render crate (or any other crate) fails
//! here and must first satisfy the gate, then update this allowlist in the same PR.
//!
//! Limits: the graph check sees crates, not phases. Decode or raster code written inside an
//! allowed crate is caught only if it names one of the backend entry points in
//! `PHASE_ENTRY_POINTS` or calls a method in `PHASE_METHOD_CALLS` (a best-effort text scan); an
//! in-crate decoder under another name passes. Symlinked files and directories are skipped.
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

/// Names that start a decode or raster phase through jpdf-traits: the decode, raster and font
/// limit builders, the scene check, and the backend traits that produce bulk output (codecs,
/// image decoders, ciphers, fonts, the rasterizer).
const PHASE_ENTRY_POINTS: [&str; 22] = [
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
    "Type1Parser",
    "font_limits",
    "FontLimits",
    "Deflate",
    "Cipher",
    "rc4",
    "aes_cbc",
];

/// Package roots of the allowed crates other than jpdf-traits (which defines the names). The
/// whole root is scanned (not only `src`), so a module loaded with `#[path]` from elsewhere
/// in the package is seen too; `target` directories and this file are skipped.
const SCANNED: [&str; 4] = [
    ".",
    "../../crates/jpdf-engine",
    "../../crates/jpdf-cos",
    "../../crates/jpdf-syntax",
];

/// Backend method calls, matched as call syntax (`.render(`) so prose and unrelated functions
/// with the same word don't match. The receiver's type may be inferred (a factory's result), so
/// only the call shows that a backend runs.
const PHASE_METHOD_CALLS: [&str; 10] = [
    ".decode(",
    ".render(",
    ".outline(",
    ".inflate(",
    ".deflate(",
    ".rc4(",
    ".aes_cbc(",
    ".glyph_by_name(",
    ".cmap_lookup(",
    ".load(",
];

/// `text` without `//` and `/* */` comments (a rough cut: it doesn't know string literals,
/// which is why the scan also looks at the text with its comments kept).
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('/') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(after) = tail.strip_prefix("//") {
            rest = after.find('\n').map_or("", |j| &after[j..]);
        } else if let Some(after) = tail.strip_prefix("/*") {
            rest = after.find("*/").map_or("", |j| &after[j + 2..]);
        } else {
            out.push('/');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Every entry-point name and backend method call in `text`. A method call is matched with all
/// whitespace removed, with and without comments, so `r.render\n    (` and
/// `r.render /* x */ (` count too.
fn phase_names_in(text: &str) -> Vec<&'static str> {
    let squash = |t: &str| t.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let (plain, uncommented) = (squash(text), squash(&strip_comments(text)));
    let names = PHASE_ENTRY_POINTS.iter().filter(|n| text.contains(*n));
    let calls = PHASE_METHOD_CALLS
        .iter()
        .filter(|c| plain.contains(*c) || uncommented.contains(*c));
    names.chain(calls).copied().collect()
}

#[test]
fn phase_scan_sees_method_calls_on_inferred_receivers() {
    let src = "let (r, l) = backends::renderer();\nlet out = r\n    .render(&scene, &l);";
    assert_eq!(phase_names_in(src), vec![".render("]);
    assert_eq!(
        phase_names_in("let d = make(); d.decode(data)"),
        vec![".decode("]
    );
    assert_eq!(
        phase_names_in("let out = r.render\n    (&scene, &limits);"),
        vec![".render("]
    );
    assert_eq!(
        phase_names_in("r . decode /* the page */ (data)"),
        vec![".decode("]
    );
    assert_eq!(
        phase_names_in("e.load // font\n (data, prog, &l)"),
        vec![".load("]
    );
    // Prose naming the word without a call is not flagged.
    assert!(phase_names_in("// we render the page later").is_empty());
}

fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let (path, kind) = (entry.path(), entry.file_type()?);
        // Symlinks are skipped: they can point out of the package (or nowhere).
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
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
    let this = root.join("tests").join("phase_gate.rs");
    let mut hits = Vec::new();
    for f in files
        .iter()
        .filter(|f| f.canonicalize().ok() != this.canonicalize().ok())
    {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        for name in phase_names_in(&text) {
            hits.push(format!("{} names {name}", f.display()));
        }
    }
    assert!(
        hits.is_empty(),
        "{hits:?}: a decode or raster phase on the jpdf-cli path needs the #146 gate \
         (specs/core/memory-budget.md) first; then update this test"
    );
}

/// The allowed crates declare no features and no optional dependencies, so no feature a
/// workspace-wide build turns on (feature unification) can add a dependency to the jpdf-cli
/// graph that the `cargo tree` check below doesn't see.
#[test]
fn parse_crates_declare_no_features_until_the_146_gate_opens() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for dir in SCANNED
        .iter()
        .copied()
        .chain(std::iter::once("../../crates/jpdf-traits"))
    {
        let manifest = root.join(dir).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest).expect("manifest is readable");
        assert!(text.contains("[package]"), "{}", manifest.display());
        for word in ["features", "optional"] {
            assert!(
                !text.contains(word),
                "{} mentions `{word}`: a feature or optional dependency could bring a decode or \
                 raster phase onto the jpdf-cli path; the #146 gate (specs/core/memory-budget.md) \
                 comes first, then update this test",
                manifest.display()
            );
        }
    }
}

/// The path in a `cargo tree --prefix none` line, `name vX.Y.Z (path)` with an optional
/// ` (*)` for a repeated package. Name and version hold no parentheses, so the path runs from
/// the first `(` to the last `)` once the marker is removed; a path may contain `(` and `)`.
fn package_path(line: &str) -> Option<&str> {
    let line = line.trim_end();
    let line = line.strip_suffix(" (*)").unwrap_or(line);
    let (_, rest) = line.split_once(" (")?;
    rest.strip_suffix(')')
}

#[test]
fn package_path_handles_parentheses_and_the_repeat_marker() {
    assert_eq!(
        package_path("jpdf-cos v0.0.0 (/w/jpdf (review)/crates/jpdf-cos)"),
        Some("/w/jpdf (review)/crates/jpdf-cos")
    );
    assert_eq!(
        package_path("jpdf-syntax v0.0.0 (/w/a) b/crates/jpdf-syntax) (*)"),
        Some("/w/a) b/crates/jpdf-syntax")
    );
    // A registry package has no path.
    assert_eq!(package_path("serde v1.0.0"), None);
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
    // Identity, not only the name: every package resolves to its workspace directory, so a
    // fork that keeps an allowed name (or a registry crate, which has no path) fails.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let name = line.split_whitespace().next().unwrap_or_default();
        let path = package_path(line);
        let want = if name == "jpdf-cli" {
            root.to_path_buf()
        } else {
            root.join("../../crates").join(name)
        };
        let same = path.is_some_and(|p| {
            std::path::Path::new(p).canonicalize().ok() == want.canonicalize().ok()
                && want.canonicalize().is_ok()
        });
        assert!(
            same,
            "{line}: not the workspace package (#146 gate, specs/core/memory-budget.md)"
        );
    }
}
