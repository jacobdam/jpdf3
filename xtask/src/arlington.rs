//! `cargo xtask arlington`. Spec: specs/tooling/arlington.md, ADR 0006.
//!
//! Fetches the pinned Arlington commit into `target/arlington/` (once; later runs reuse the
//! cache), parses and checks every TSV file, writes `target/arlington/tables.rs` and makes
//! sure it compiles. Nothing it writes is committed.

use std::path::Path;
use std::process::Command;

use jpdf_arlington::{Source, generate, load};

/// Returns a one-line summary on success.
pub fn run(root: &Path) -> Result<String, String> {
    // Fail fast outside the repository, so a stray run neither fetches nor writes.
    let manifest = root.join("tools").join("arlington").join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!(
            "{} not found (run from the repository root)",
            manifest.display()
        ));
    }
    let cache = root.join("target").join("arlington");
    let source = Source::Pinned(cache.clone());
    let model = load(&source).map_err(|e| e.to_string())?;
    let out = cache.join("tables.rs");
    std::fs::write(&out, generate::render(&model, &source.describe()))
        .map_err(|e| format!("{}: {e}", out.display()))?;
    // Type-check only (`--emit=metadata`): the tables are data, so this is fast.
    let status = Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit=metadata",
        ])
        .arg("--crate-name")
        .arg("arlington_tables")
        .arg("-o")
        .arg(cache.join("tables.rmeta"))
        .arg(&out)
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run rustc: {e}"))?;
    if !status.success() {
        return Err(format!("{} does not compile", out.display()));
    }
    Ok(format!(
        "{}: {} objects, {} keys; wrote {}",
        source.describe(),
        model.objects.len(),
        model.key_count(),
        out.display()
    ))
}
