//! `cargo xtask ci linux --out DIR` (specs/tooling/ci-linux.md, ops/judge/CONTRACT.md §2):
//! five checks, each run even when an earlier one failed, in the order [`drive`] fixes
//! (ADR 0013, docs/PRINCIPLES.md P4). The exit code is `steps::exit_code` of the log.

use crate::dashboard::{self, Copied};
use crate::steps::{Log, Outcome};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The checks of `ci linux`, called only by [`drive`]; none decides the order. Each prints
/// its own lines and returns its outcome.
pub trait Steps {
    /// The web build and Playwright run: removes an earlier `DIR/site/`, writes `e2e.json`
    /// and, after a successful build, `DIR/site/`.
    fn web(&mut self) -> Outcome;
    /// The committed snapshot into `DIR/site/dashboard/` (specs/tooling/dashboard.md item 2).
    fn dashboard(&mut self) -> Outcome;
    /// specs/tooling/check-specs.md.
    fn check_specs(&mut self) -> Outcome;
    /// specs/tooling/fuzz.md: the short fuzz run and the `fuzz/` format check.
    fn fuzz(&mut self) -> Outcome;
    /// specs/tooling/arlington.md.
    fn arlington(&mut self) -> Outcome;
}

/// The order (specs/tooling/ci-linux.md): every step runs; `dashboard` after `web`, which
/// removes and rewrites `DIR/site/`; `check-specs` after `web` has written `e2e.json`.
pub fn drive(s: &mut dyn Steps, mut log: Log) -> Log {
    log.push("web", s.web());
    log.push("dashboard", s.dashboard());
    log.push("check-specs", s.check_specs());
    log.push("fuzz", s.fuzz());
    log.push("arlington", s.arlington());
    log
}

/// The dashboard step's outcome and the note it prints (specs/tooling/dashboard.md, Errors).
fn dashboard_outcome(copied: Result<Copied, String>) -> (Outcome, Option<&'static str>) {
    match copied {
        Ok(Copied::Yes) => (Outcome::Ok, None),
        Ok(Copied::NoPage) => (Outcome::Ok, Some("no page, nothing copied")),
        Ok(Copied::NoSnapshot) => (Outcome::Ok, Some("no snapshot, nothing copied")),
        Err(e) => (Outcome::Failed(format!("dashboard: {e}")), None),
    }
}

/// Prints a `Failed` reason as `xtask: <reason>` (the log is not a printing one: the other
/// checks print their own errors in their own form).
fn print_failed(o: &Outcome) {
    if let Outcome::Failed(why) = o {
        eprintln!("xtask: {why}");
    }
}

/// The real checks, run in the repository `repo` with output under `out` (`DIR`).
pub struct Real {
    pub repo: PathBuf,
    pub out: PathBuf,
}

impl Steps for Real {
    fn web(&mut self) -> Outcome {
        let o = match crate::web::ci(&self.repo, &self.out) {
            Ok(()) => {
                eprintln!("xtask: web: ok");
                Outcome::Ok
            }
            Err(e) => Outcome::Failed(format!("web: {e}")),
        };
        print_failed(&o);
        o
    }

    fn dashboard(&mut self) -> Outcome {
        let (o, note) = dashboard_outcome(dashboard::copy_into_site(
            &self.repo,
            &self.out.join("site"),
        ));
        if let Some(note) = note {
            eprintln!("xtask: dashboard: {note}");
        }
        print_failed(&o);
        o
    }

    fn check_specs(&mut self) -> Outcome {
        // `check_specs` prints its own problems.
        failed_unless_success(crate::check_specs(&self.repo), "check-specs failed")
    }

    fn fuzz(&mut self) -> Outcome {
        let o = match crate::fuzz::ci(&self.repo) {
            Ok(()) => {
                eprintln!("xtask: fuzz: no crash");
                Outcome::Ok
            }
            Err(e) => Outcome::Failed(format!("fuzz: {e}")),
        };
        print_failed(&o);
        o
    }

    fn arlington(&mut self) -> Outcome {
        // `arlington_cmd` prints its own summary or error.
        failed_unless_success(crate::arlington_cmd(&self.repo), "arlington failed")
    }
}

/// `Ok` for a successful exit code, else `Failed(why)`.
fn failed_unless_success(code: ExitCode, why: &str) -> Outcome {
    if code == ExitCode::SUCCESS {
        Outcome::Ok
    } else {
        Outcome::Failed(why.to_owned())
    }
}

/// Runs `ci linux` in `repo` with output under `out`.
pub fn run(repo: &Path, out: &Path) -> ExitCode {
    let mut real = Real {
        repo: repo.to_path_buf(),
        out: out.to_path_buf(),
    };
    ExitCode::from(crate::steps::exit_code(&drive(&mut real, Log::default())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const STEPS: [&str; 5] = ["web", "dashboard", "check-specs", "fuzz", "arlington"];

    /// A fake that records each call and fails the step named `fail`, if any.
    struct Fake {
        calls: Vec<&'static str>,
        fail: Option<&'static str>,
    }

    impl Fake {
        fn step(&mut self, name: &'static str) -> Outcome {
            self.calls.push(name);
            if self.fail == Some(name) {
                Outcome::Failed(format!("{name} broke"))
            } else {
                Outcome::Ok
            }
        }
    }

    impl Steps for Fake {
        fn web(&mut self) -> Outcome {
            self.step("web")
        }
        fn dashboard(&mut self) -> Outcome {
            self.step("dashboard")
        }
        fn check_specs(&mut self) -> Outcome {
            self.step("check-specs")
        }
        fn fuzz(&mut self) -> Outcome {
            self.step("fuzz")
        }
        fn arlington(&mut self) -> Outcome {
            self.step("arlington")
        }
    }

    fn drive_with(fail: Option<&'static str>) -> (Vec<&'static str>, Log) {
        let mut fake = Fake {
            calls: Vec::new(),
            fail,
        };
        let log = drive(&mut fake, Log::default());
        (fake.calls, log)
    }

    #[test]
    fn every_step_runs_in_order() {
        let (calls, log) = drive_with(None);
        assert_eq!(calls, STEPS);
        assert_eq!(log.names(), STEPS);
        assert!(log.entries().iter().all(|(_, o)| *o == Outcome::Ok));
        assert_eq!(crate::steps::exit_code(&log), 0);
    }

    /// One run per failing step: the later steps still run, in order, and the run fails.
    #[test]
    fn a_failed_step_fails_the_run_and_the_rest_still_run() {
        for fail in STEPS {
            let (calls, log) = drive_with(Some(fail));
            assert_eq!(calls, STEPS, "{fail} failing");
            assert_eq!(log.names(), STEPS, "{fail} failing");
            for (name, o) in log.entries() {
                let want = if *name == fail {
                    Outcome::Failed(format!("{fail} broke"))
                } else {
                    Outcome::Ok
                };
                assert_eq!(*o, want, "{name} when {fail} fails");
            }
            assert_eq!(crate::steps::exit_code(&log), 1, "{fail} failing");
        }
    }

    #[test]
    fn dashboard_outcomes() {
        assert_eq!(dashboard_outcome(Ok(Copied::Yes)), (Outcome::Ok, None));
        assert_eq!(
            dashboard_outcome(Ok(Copied::NoPage)),
            (Outcome::Ok, Some("no page, nothing copied"))
        );
        assert_eq!(
            dashboard_outcome(Ok(Copied::NoSnapshot)),
            (Outcome::Ok, Some("no snapshot, nothing copied"))
        );
        assert_eq!(
            dashboard_outcome(Err("x: not a regular file".into())),
            (
                Outcome::Failed("dashboard: x: not a regular file".into()),
                None
            )
        );
    }

    /// The real adapter copies into `DIR/site/dashboard/`, and its error is `Failed`.
    #[test]
    fn the_real_dashboard_step_copies_into_out_site() {
        let base =
            std::env::temp_dir().join(format!("xtask-ci-linux-dashboard-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (repo, out) = (base.join("repo"), base.join("out"));
        assert!(fs::create_dir_all(repo.join("ops/metrics")).is_ok());
        let snap = repo.join("ops/metrics/dashboard.json");
        assert!(fs::write(&snap, r#"{"version": 1}"#).is_ok());
        // A `dashboard/` directly under DIR (not DIR/site) is not the page.
        assert!(fs::create_dir_all(out.join("dashboard")).is_ok());
        let mut real = Real {
            repo: repo.clone(),
            out: out.clone(),
        };
        assert_eq!(real.dashboard(), Outcome::Ok, "no page yet");
        assert!(!out.join("dashboard/data.json").exists());
        assert!(fs::create_dir_all(out.join("site/dashboard")).is_ok());
        assert_eq!(real.dashboard(), Outcome::Ok);
        assert_eq!(
            fs::read_to_string(out.join("site/dashboard/data.json")).ok(),
            Some(r#"{"version": 1}"#.to_owned())
        );
        assert!(fs::remove_file(&snap).is_ok() && fs::create_dir_all(&snap).is_ok());
        let o = real.dashboard();
        assert!(
            matches!(&o, Outcome::Failed(why) if why.starts_with("dashboard: ") && why.ends_with("not a regular file")),
            "{o:?}"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn exit_codes_map_to_outcomes() {
        assert_eq!(failed_unless_success(ExitCode::SUCCESS, "x"), Outcome::Ok);
        assert_eq!(
            failed_unless_success(ExitCode::from(1), "x"),
            Outcome::Failed("x".into())
        );
    }
}
