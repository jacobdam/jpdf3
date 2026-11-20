//! Project task runner (NOT protected; the agent extends it). Contract: ops/judge/CONTRACT.md §2.
#![forbid(unsafe_code)]

mod fuzz;
mod json;
mod macos;
mod specs;
mod web;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask ci <linux|macos> --out DIR | nightly --out DIR | check-specs [--root DIR] \
     | fuzz [--short] [--seconds N] [--target NAME]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
    };
    let out = flag("--out").unwrap_or_else(|| PathBuf::from("target/xtask-out"));
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        (Some("ci"), Some("linux")) => {
            // Run every check even if an earlier one failed, so one run shows all problems.
            // Like check-specs, the web build uses the current directory as the repo root.
            let web = match web::ci(Path::new("."), &out) {
                Ok(()) => {
                    eprintln!("xtask: web: ok");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("xtask: web: {e}");
                    ExitCode::from(1)
                }
            };
            let specs = check_specs(Path::new("."));
            // PLAN §6.3: a short fuzz run on every PR (specs/tooling/fuzz.md).
            let fuzz = match fuzz::ci(Path::new(".")) {
                Ok(()) => {
                    eprintln!("xtask: fuzz: no crash");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("xtask: fuzz: {e}");
                    ExitCode::from(1)
                }
            };
            [web, specs, fuzz]
                .into_iter()
                .find(|c| *c != ExitCode::SUCCESS)
                .unwrap_or(ExitCode::SUCCESS)
        }
        (Some("ci"), Some("macos")) => match macos::ci(Path::new("."), &out) {
            // specs/ui-macos/app-skeleton.md. check-specs runs in `ci linux`.
            Ok(()) => {
                eprintln!("xtask: macos: ok");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("xtask: macos: {e}");
                ExitCode::from(1)
            }
        },
        (Some("nightly"), _) => ExitCode::SUCCESS,
        // specs/tooling/fuzz.md. Needs nightly + cargo-fuzz; not part of `ci linux`.
        (Some("fuzz"), _) => {
            let result = fuzz::parse(args.get(1..).unwrap_or_default())
                .and_then(|opts| fuzz::run(Path::new("."), &opts));
            match result {
                Ok(()) => {
                    eprintln!("xtask: fuzz: no crash");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("xtask: fuzz: {e}");
                    ExitCode::from(1)
                }
            }
        }
        (Some("check-specs"), _) => {
            let known = match args.len() {
                1 => true,
                3 => args.get(1).is_some_and(|a| a == "--root"),
                _ => false,
            };
            if !known {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
            check_specs(&flag("--root").unwrap_or_else(|| PathBuf::from(".")))
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// specs/tooling/check-specs.md
fn check_specs(root: &Path) -> ExitCode {
    let problems = specs::check(root);
    if problems.is_empty() {
        eprintln!("check-specs: ok");
        return ExitCode::SUCCESS;
    }
    for p in &problems {
        eprintln!("{p}");
    }
    eprintln!("check-specs: {} problem(s)", problems.len());
    ExitCode::from(1)
}
