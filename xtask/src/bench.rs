//! `cargo xtask bench [--out DIR] [--corpus DIR] [--sample N] [--runs R] [--warmup W]
//! [--mutool PATH|none] [--pdfium PATH|none]`. Spec: specs/tooling/bench.md item 11.
//!
//! Builds `jpdf-cli` and `jpdf-bench` the way the judge builds `jpdf-cli`, then runs the bench
//! with the report at `DIR/bench.json`. jpdf-bench validates the forwarded values itself.

use std::ffi::OsString;
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
    let release = build(root)?;
    let report = opts.out.join("bench.json");
    let status = Command::new(release.join("jpdf-bench"))
        .current_dir(root)
        .args(bench_args(
            &release.join("jpdf-cli"),
            &report,
            &opts.forwarded,
        ))
        .status()
        .map_err(|e| format!("cannot run jpdf-bench: {e}"))?;
    // A signal (no code) is a failure like any other non-zero exit.
    Ok(status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(1))
}

/// jpdf-bench's arguments: `--cli CLI --out REPORT` then the forwarded flags.
pub fn bench_args(cli: &Path, report: &Path, forwarded: &[String]) -> Vec<OsString> {
    let mut a: Vec<OsString> = vec!["--cli".into(), cli.into(), "--out".into(), report.into()];
    a.extend(forwarded.iter().map(OsString::from));
    a
}

/// Builds `jpdf-cli` and `jpdf-bench` (release, locked); returns the release directory that
/// holds both.
pub fn build(root: &Path) -> Result<PathBuf, String> {
    // The cargo that runs xtask, and its target dir (CARGO_TARGET_DIR may move it).
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
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
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    Ok(root.join(target).join("release"))
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
