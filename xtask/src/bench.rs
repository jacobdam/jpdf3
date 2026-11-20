//! `cargo xtask bench [--out DIR] [--corpus DIR] [--sample N] [--runs R] [--warmup W]
//! [--mutool PATH|none] [--pdfium PATH|none]`. Spec: specs/tooling/bench.md item 11.
//!
//! Builds `jpdf-cli` and `jpdf-bench` the way the judge builds `jpdf-cli`, then runs the bench
//! with the report at `DIR/bench.json`. jpdf-bench validates the forwarded values itself.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Flags passed through to jpdf-bench unchanged (each takes one value).
const FORWARDED: &[&str] = &[
    "--corpus", "--sample", "--runs", "--warmup", "--mutool", "--pdfium",
];

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub out: PathBuf,
    pub forwarded: Vec<String>,
}

/// Parses the arguments after `bench`.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut out = PathBuf::from("target/xtask-out");
    let mut forwarded = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = iter.next().ok_or_else(|| format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--out" => out = PathBuf::from(value),
            a if FORWARDED.contains(&a) => forwarded.extend([arg.clone(), value.clone()]),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(Options { out, forwarded })
}

/// Builds and runs the bench. `Ok(code)` is jpdf-bench's exit code.
pub fn run(root: &Path, opts: &Options) -> Result<u8, String> {
    let status = Command::new("cargo")
        .current_dir(root)
        .args([
            "build",
            "--release",
            "--locked",
            "-p",
            "jpdf-cli",
            "-p",
            "jpdf-bench",
        ])
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("release build failed ({status})"));
    }
    let release = root.join("target/release");
    let report = opts.out.join("bench.json");
    let status = Command::new(release.join("jpdf-bench"))
        .current_dir(root)
        .arg("--cli")
        .arg(release.join("jpdf-cli"))
        .arg("--out")
        .arg(&report)
        .args(&opts.forwarded)
        .status()
        .map_err(|e| format!("cannot run jpdf-bench: {e}"))?;
    // A signal (no code) is a failure like any other non-zero exit.
    Ok(status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Options, String> {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn defaults_and_forwarding() {
        assert_eq!(
            p(&[]).unwrap(),
            Options {
                out: PathBuf::from("target/xtask-out"),
                forwarded: vec![]
            }
        );
        let o = p(&[
            "--runs", "3", "--out", "d", "--corpus", "c", "--mutool", "none",
        ])
        .unwrap();
        assert_eq!(o.out, PathBuf::from("d"));
        assert_eq!(
            o.forwarded,
            ["--runs", "3", "--corpus", "c", "--mutool", "none"]
        );
    }

    #[test]
    fn rejects_unknown_and_missing_values() {
        assert!(p(&["--runs"]).is_err());
        assert!(p(&["--cli", "x"]).is_err());
        assert!(p(&["--work", "x"]).is_err());
        assert!(p(&["extra", "x"]).is_err());
    }
}
