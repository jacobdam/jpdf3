//! Project task runner (NOT protected; the agent extends it). Contract: ops/judge/CONTRACT.md §2.
#![forbid(unsafe_code)]

mod arlington;
mod bench;
mod fuzz;
mod macos;
mod metrics;
mod next_session;
mod specs;
mod web;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask ci <linux|macos> --out DIR | nightly --out DIR | arlington | check-specs [--root DIR] \
     | fuzz [--short] [--seconds N] [--target NAME] \
     | bench [--out DIR] [--corpus DIR] [--sample N] [--runs R] [--warmup W] [--mutool PATH|none] [--pdfium PATH|none] \
     | metrics [--since YYYY-MM-DD] [--until YYYY-MM-DD] [--out DIR] [--from DIR] [--index FILE] \
     | next-session [--from DIR]";

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
            // specs/tooling/arlington.md: the vendored model still matches the pins, parses, checks and compiles (offline).
            let arl = arlington_cmd(Path::new("."));
            [web, specs, fuzz, arl]
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
        // specs/tooling/fuzz.md. Needs the pinned nightly + cargo-fuzz; `ci linux` runs the
        // short variant through `fuzz::ci`.
        (Some("fuzz"), _) => {
            let opts = match fuzz::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: fuzz: {e}\n{USAGE}");
                    return ExitCode::from(2);
                }
            };
            match fuzz::run(Path::new("."), &opts) {
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
        // specs/tooling/bench.md. `cargo xtask nightly` (#19) will call this.
        (Some("bench"), _) => {
            let opts = match bench::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: bench: {e}\n{USAGE}");
                    return ExitCode::from(2);
                }
            };
            match bench::run(Path::new("."), &opts) {
                Ok(code) => ExitCode::from(code),
                Err(e) => {
                    eprintln!("xtask: bench: {e}");
                    ExitCode::from(1)
                }
            }
        }
        // specs/tooling/metrics.md: retro input (AUTOMATION §11); read-only git/gh queries.
        (Some("metrics"), _) => {
            let opts = match metrics::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: metrics: {e}\n{USAGE}");
                    return ExitCode::from(2);
                }
            };
            match metrics::run(Path::new("."), &opts) {
                Ok(md) => {
                    print!("{md}");
                    eprintln!("xtask: metrics: wrote {}", opts.out.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("xtask: metrics: {e}");
                    ExitCode::from(1)
                }
            }
        }
        // specs/tooling/next-session.md: the scheduler (ADR 0011 §2). Exit 2 = unknown (a
        // command failed: the session does no work); 1 = the scheduler is broken.
        (Some("next-session"), _) => {
            let opts = match next_session::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: next-session: {e}\n{USAGE}");
                    return ExitCode::from(1);
                }
            };
            match next_session::run(Path::new("."), &opts) {
                Ok(brief) => {
                    print!("{brief}");
                    ExitCode::SUCCESS
                }
                Err(next_session::Error::Unknown(e)) => {
                    eprintln!("xtask: next-session: unknown: {e}");
                    ExitCode::from(2)
                }
                Err(next_session::Error::Broken(e)) => {
                    eprintln!("xtask: next-session: {e}");
                    ExitCode::from(1)
                }
            }
        }
        (Some("arlington"), None) => arlington_cmd(Path::new(".")),
        (Some("arlington"), Some(_)) => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
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

/// specs/tooling/arlington.md
fn arlington_cmd(root: &Path) -> ExitCode {
    match arlington::run(root) {
        Ok(summary) => {
            eprintln!("xtask: arlington: {summary}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("xtask: arlington: {e}");
            ExitCode::from(1)
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
