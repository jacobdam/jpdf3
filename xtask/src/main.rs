//! Project task runner (NOT protected; the agent extends it). Contract: ops/judge/CONTRACT.md §2.
#![forbid(unsafe_code)]

mod admit;
mod arlington;
mod bench;
mod ci_linux;
mod dashboard;
mod fuzz;
mod issue_lint;
mod limits;
mod macos;
mod metrics;
mod next_session;
mod nightly;
mod specs;
mod steps;
mod web;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask ci <linux|macos> --out DIR \
     | nightly [--out DIR] [--cache DIR] [--limit N] [--previous FILE] [--max-minutes M] [--bench-minutes B] [--fuzz-seconds S] [--file-issues] | limits [--out DIR] [--mib N] | arlington | check-specs [--root DIR] \
     | fuzz [--short] [--seconds N] [--target NAME] \
     | bench [--out DIR] [--corpus DIR] [--sample N] [--runs R] [--warmup W] [--mutool PATH|none] [--pdfium PATH|none] \
     | dashboard [--until YYYY-MM-DD] [--out FILE] [--from DIR] [--index FILE] \
     | metrics [--since YYYY-MM-DD] [--until YYYY-MM-DD] [--out DIR] [--from DIR] [--index FILE] \
     | next-session [--from DIR] | status [--from DIR] | issue-lint [--from DIR] \
     | admit <crate>@<version> --work DIR [--features A,B] [--no-default-features] [--python PATH]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i.saturating_add(1)))
            .map(PathBuf::from)
    };
    let out = flag("--out").unwrap_or_else(|| PathBuf::from("target/xtask-out"));
    match (
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        // specs/tooling/ci-linux.md: every check runs, in the order `ci_linux::drive` fixes.
        // Like check-specs, the web build uses the current directory as the repo root.
        (Some("ci"), Some("linux")) => ci_linux::run(Path::new("."), &out),
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
        // specs/tooling/nightly.md item 10: exit 1 = a tool failure, 75 = corpus fetch infra
        // failure, 4 = a failed limits case; what jpdf-cli did with a regress file never
        // changes the exit code. The code is `steps::exit_code` of the driver's log.
        (Some("nightly"), _) => {
            let opts = match nightly::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: nightly: {e}\n{USAGE}");
                    return ExitCode::from(2);
                }
            };
            ExitCode::from(nightly::run(Path::new("."), &opts))
        }
        // specs/tooling/limits.md: exit 4 = a case failed, 1 = a tool failure.
        (Some("limits"), _) => {
            let opts = match limits::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: limits: {e}\n{USAGE}");
                    return ExitCode::from(2);
                }
            };
            match limits::run(Path::new("."), &opts) {
                Ok(code) => ExitCode::from(code),
                Err(e) => {
                    eprintln!("xtask: limits: {e}");
                    ExitCode::from(1)
                }
            }
        }
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
        // specs/tooling/bench.md. `cargo xtask nightly` runs the same build (`bench::build`).
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
        // specs/tooling/dashboard.md: the snapshot the Pages dashboard renders (read-only gh/git).
        (Some("dashboard"), _) => {
            let opts = match dashboard::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: dashboard: {e}\n{USAGE}");
                    return ExitCode::from(2);
                }
            };
            match dashboard::run(Path::new("."), &opts) {
                Ok(path) => {
                    eprintln!("xtask: dashboard: wrote {}", path.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("xtask: dashboard: {e}");
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
        // specs/tooling/status.md: the generated status (#386), from next-session's inputs and
        // pick, with its exit codes: 2 = unknown (a command failed, nothing printed), 1 = broken.
        (Some("status"), _) => {
            let opts = match next_session::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: status: {e}\n{USAGE}");
                    return ExitCode::from(1);
                }
            };
            match next_session::status(Path::new("."), &opts) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(next_session::Error::Unknown(e)) => {
                    eprintln!("xtask: status: unknown: {e}");
                    ExitCode::from(2)
                }
                Err(next_session::Error::Broken(e)) => {
                    eprintln!("xtask: status: {e}");
                    ExitCode::from(1)
                }
            }
        }
        // specs/tooling/issue-lint.md: area:/origin: labels (ADR 0011 §4). Exit 1 = problems
        // found, 2 = the lint could not run.
        (Some("issue-lint"), _) => {
            let opts = match issue_lint::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: issue-lint: {e}\n{USAGE}");
                    return ExitCode::from(2);
                }
            };
            match issue_lint::run(Path::new("."), &opts) {
                Ok(report) => {
                    print!("{}", report.text);
                    ExitCode::from(u8::from(report.problems > 0))
                }
                Err(e) => {
                    eprintln!("xtask: issue-lint: {e}");
                    ExitCode::from(2)
                }
            }
        }
        // specs/tooling/admit.md: exit 0 admitted, 4 rejected, 1 a step failed, 2 usage.
        (Some("admit"), _) => {
            let opts = match admit::parse(args.get(1..).unwrap_or_default()) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("xtask: admit: {e}\n{USAGE}");
                    return ExitCode::from(2);
                }
            };
            match admit::run(Path::new("."), &opts) {
                Ok(code) => ExitCode::from(code),
                Err(e) => {
                    eprintln!("xtask: admit: {e}");
                    ExitCode::from(2)
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
