//! `cargo xtask arlington`. Spec: specs/tooling/arlington.md, ADR 0007.
//!
//! Offline: loads the vendored Arlington data in `tools/arlington/data`, checks its git ids
//! against the pins, parses and checks every TSV file, writes `target/arlington/tables.rs`
//! and makes sure it compiles. The generated tables are not committed.

use std::path::Path;
use std::process::Command;

use jpdf_arlington::{Source, generate, load, vendor};

/// Returns a one-line summary on success.
pub fn run(root: &Path) -> Result<String, String> {
    // Fail fast outside the repository, so a stray run writes nothing.
    let manifest = root.join("tools").join("arlington").join("Cargo.toml");
    if !manifest.is_file() {
        return Err(format!(
            "{} not found (run from the repository root)",
            manifest.display()
        ));
    }
    let cache = root.join("target").join("arlington");
    std::fs::create_dir_all(&cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    let source = Source::Vendored(root.join(vendor::DATA_DIR));
    let model = load(&source).map_err(|e| e.to_string())?;
    let out = cache.join("tables.rs");
    std::fs::write(&out, generate::render(&model, &source.describe()))
        .map_err(|e| format!("{}: {e}", out.display()))?;
    // Type-check only (`--emit=metadata`): the tables are data, so this is fast.
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let status = Command::new(rustc)
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
