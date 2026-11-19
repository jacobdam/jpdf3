//! Project task runner (NOT protected; the agent extends it). Contract: ops/judge/CONTRACT.md §2.
#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .map_or_else(|| PathBuf::from("target/xtask-out"), PathBuf::from);
    match args.first().map(String::as_str) {
        Some("ci") => write_e2e(&out),
        Some("nightly") => ExitCode::SUCCESS,
        _ => {
            eprintln!("usage: cargo xtask ci <linux|macos> --out DIR | nightly --out DIR");
            ExitCode::from(2)
        }
    }
}

/// Bootstrap stub: no E2E suites exist yet, so report an empty result set.
fn write_e2e(out: &PathBuf) -> ExitCode {
    if fs::create_dir_all(out).is_err()
        || fs::write(out.join("e2e.json"), "{\"results\": {}}\n").is_err()
    {
        eprintln!("xtask: cannot write {}", out.display());
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
