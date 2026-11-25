//! `cargo xtask fuzz [--short] [--seconds N] [--target NAME]`. Spec: specs/tooling/fuzz.md.
//!
//! Writes the jpdf-testgen seeds into `fuzz/corpus/<target>/`, builds the targets, then runs
//! every target from `cargo fuzz list` (or just `--target`) with the judge's memory limit.
//! Uses the dated nightly [`NIGHTLY`], so a broken nightly release cannot turn PRs red.

use std::path::Path;
use std::process::Command;

/// Seconds per target: `--short` for local and PR use, the default as in the nightly judge.
pub const SHORT_SECONDS: u32 = 30;
pub const NIGHTLY_SECONDS: u32 = 300;

/// The judge's address-space budget (CONTRACT §3 runs with `-rss_limit_mb=2048`).
const RSS_LIMIT_MB: u32 = 2048;
/// One input taking longer than this is a hang, which counts as a crash.
const INPUT_TIMEOUT_SECONDS: u32 = 10;

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub seconds: u32,
    pub target: Option<String>,
}

/// Parses the arguments after `fuzz`.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        seconds: NIGHTLY_SECONDS,
        target: None,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--short" => opts.seconds = SHORT_SECONDS,
            "--seconds" => {
                opts.seconds = iter
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|n| (1..=3600).contains(n))
                    .ok_or("--seconds needs an integer in 1..=3600")?;
            }
            "--target" => {
                let name = iter.next().ok_or("--target needs a name")?;
                opts.target = Some(name.clone());
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(opts)
}

/// Seed sets per target. A target missing here starts from an empty corpus.
pub fn seeds_for(target: &str) -> Vec<(&'static str, Vec<u8>)> {
    match target {
        "engine_open" => jpdf_testgen::engine_open_seeds(),
        // Both template targets read the chunk format.
        "engine_objects" => jpdf_testgen::engine_objects_seeds(),
        // Reads its input both raw and through the template: give it both seed sets.
        "syntax_resolve" => {
            let mut seeds = jpdf_testgen::engine_open_seeds();
            seeds.extend(jpdf_testgen::engine_objects_seeds());
            seeds
        }
        _ => Vec::new(),
    }
}

/// Writes seeds as `seed-<name>` files, overwriting earlier seed files and keeping whatever
/// the fuzzer added to the corpus.
pub fn write_seeds(fuzz_dir: &Path, target: &str) -> Result<usize, String> {
    let seeds = seeds_for(target);
    if seeds.is_empty() {
        return Ok(0);
    }
    let dir = fuzz_dir.join("corpus").join(target);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    for (name, bytes) in &seeds {
        let path = dir.join(format!("seed-{name}"));
        std::fs::write(&path, bytes)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(seeds.len())
}

/// The nightly toolchain for our fuzz runs (the judge's own nightly step is not pinned).
/// Bump it in an audit, like `rust-toolchain.toml`.
pub const NIGHTLY: &str = "nightly-2026-11-20";

fn plus_nightly() -> String {
    format!("+{NIGHTLY}")
}

/// cargo-fuzz version CI installs (`--locked`), so a new release cannot change PR results.
const CARGO_FUZZ_VERSION: &str = "0.13.2";

/// The short run inside `cargo xtask ci linux`. On CI (`CI` set) missing tools are installed
/// first; locally a missing tool is an error with the install hint.
pub fn ci(root: &Path) -> Result<(), String> {
    // Check before installing anything (the xtask tests run `ci linux` in fixture trees).
    require_project(root)?;
    if std::env::var_os("CI").is_some() {
        install_missing_tools(root)?;
    }
    // fuzz/ is outside the workspace, so `cargo fmt --all` does not see it.
    let fmt = Command::new("cargo")
        .args(["fmt", "--manifest-path", "fuzz/Cargo.toml", "--check"])
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run cargo fmt: {e}"))?;
    // Run the fuzz pass even when formatting fails, so one run shows both problems.
    let fuzzed = run(
        root,
        &Options {
            seconds: SHORT_SECONDS,
            target: None,
        },
    );
    ci_result(fuzzed, fmt.success())
}

/// How `ci` combines the fuzz pass and the `fuzz/` format check: either failing fails it, and
/// both messages are kept, the fuzz one first.
pub fn ci_result(fuzzed: Result<(), String>, formatted: bool) -> Result<(), String> {
    let fmt_problem = (!formatted)
        .then_some("fuzz/ is not formatted (run `cargo fmt --manifest-path fuzz/Cargo.toml`)");
    match (fuzzed, fmt_problem) {
        (Ok(()), None) => Ok(()),
        (Ok(()), Some(f)) => Err(f.to_owned()),
        (Err(e), None) => Err(e),
        (Err(e), Some(f)) => Err(format!("{e}; {f}")),
    }
}

/// Whether `rustup toolchain list` output names [`NIGHTLY`] (lines like
/// `nightly-2026-11-20-x86_64-unknown-linux-gnu` or the same with ` (active)`).
pub fn nightly_listed(list: &str) -> bool {
    list.lines().any(|l| {
        l.strip_prefix(NIGHTLY)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['-', ' ']))
    })
}

/// Whether `cargo fuzz --version` output is exactly `cargo-fuzz <CARGO_FUZZ_VERSION>`
/// (surrounding whitespace aside).
pub fn cargo_fuzz_pinned(version: &str) -> bool {
    version.trim() == format!("cargo-fuzz {CARGO_FUZZ_VERSION}")
}

/// Whether CI must install [`NIGHTLY`], given `rustup toolchain list` output (`None` when
/// the command failed).
pub fn needs_nightly(list: Option<&str>) -> bool {
    !list.is_some_and(nightly_listed)
}

/// Whether CI must install cargo-fuzz, given `cargo +NIGHTLY fuzz --version` output (`None`
/// when the command failed, e.g. no cargo-fuzz on PATH).
pub fn needs_cargo_fuzz(version: Option<&str>) -> bool {
    !version.is_some_and(cargo_fuzz_pinned)
}

const NIGHTLY_INSTALL: [&str; 6] = [
    "rustup",
    "toolchain",
    "install",
    NIGHTLY,
    "--profile",
    "minimal",
];
/// `--force`: a cargo-fuzz restored from the CI cache is not in cargo's install records, and
/// without it `cargo install` refuses to replace a wrong or broken one (PR #256, R1.1).
const CARGO_FUZZ_INSTALL: [&str; 7] = [
    "cargo",
    "install",
    "cargo-fuzz",
    "--force",
    "--locked",
    "--version",
    CARGO_FUZZ_VERSION,
];

fn output_of(root: &Path, program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Installs only what is missing, so a tool CI already provides (a prebuilt cargo-fuzz, a
/// cached toolchain) is not built or downloaded again. The nightly is looked up in the
/// `rustup toolchain list`, not by running it, which could make rustup install it itself.
pub fn install_missing_tools(root: &Path) -> Result<(), String> {
    if needs_nightly(output_of(root, "rustup", &["toolchain", "list"]).as_deref()) {
        install(&NIGHTLY_INSTALL)?;
    }
    // Checked after the nightly is there: `cargo +NIGHTLY` fails without it.
    let plus = plus_nightly();
    if needs_cargo_fuzz(output_of(root, "cargo", &[plus.as_str(), "fuzz", "--version"]).as_deref())
    {
        install(&CARGO_FUZZ_INSTALL)?;
    }
    Ok(())
}

fn install(step: &[&str]) -> Result<(), String> {
    let (program, rest) = step.split_first().ok_or("empty install step")?;
    let ok = Command::new(program)
        .args(rest)
        .status()
        .is_ok_and(|s| s.success());
    if ok {
        Ok(())
    } else {
        Err(format!("installing fuzz tools failed: {}", step.join(" ")))
    }
}

pub fn require_project(root: &Path) -> Result<(), String> {
    let manifest = root.join("fuzz").join("Cargo.toml");
    if manifest.is_file() {
        Ok(())
    } else {
        Err(format!(
            "{} not found (run from the repository root)",
            manifest.display()
        ))
    }
}

pub fn run(root: &Path, opts: &Options) -> Result<(), String> {
    let r = run_report(root, opts)?;
    run_result(match r.error {
        None => Ok(r.failed),
        Some(e) => Err((e, r.failed)),
    })
}

/// What one fuzz pass did, for `cargo xtask nightly` (specs/tooling/nightly.md item 13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The targets selected, in `cargo fuzz list` order.
    pub targets: Vec<String>,
    /// The targets whose run started, in the same order ([`run_all`] can stop before the rest).
    pub started: Vec<String>,
    /// The targets that crashed, hung or ran out of memory.
    pub failed: Vec<String>,
    /// A target that could not start, or a log that could not be created ([`run_all`]).
    pub error: Option<String>,
}

/// [`run`] without the summary: `Err` is a failure before any target ran (no project, `cargo
/// fuzz list` or the build failed, an unknown `--target`, a seed that can't be written).
pub fn run_report(root: &Path, opts: &Options) -> Result<Report, String> {
    require_project(root)?;
    let fuzz_dir = root.join("fuzz");
    let list = Command::new("cargo")
        .arg(plus_nightly())
        .args(["fuzz", "list"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !list.status.success() {
        return Err(format!(
            "`cargo +{NIGHTLY} fuzz list` failed (install: `rustup toolchain install {NIGHTLY} \
             --profile minimal` and `cargo install cargo-fuzz --force --locked --version \
             {CARGO_FUZZ_VERSION}`): {}",
            String::from_utf8_lossy(&list.stderr).trim()
        ));
    }
    let all: Vec<String> = String::from_utf8_lossy(&list.stdout)
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let targets: Vec<String> = match &opts.target {
        Some(t) if all.contains(t) => vec![t.clone()],
        Some(t) => return Err(format!("unknown fuzz target {t:?}; targets: {all:?}")),
        None => all,
    };
    // Build first, so a toolchain or compile failure is not reported as a crash.
    let build = Command::new("cargo")
        .arg(plus_nightly())
        .args(["fuzz", "build", "--debug-assertions"])
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run cargo fuzz: {e}"))?;
    if !build.success() {
        return Err(format!(
            "fuzz targets do not build with {NIGHTLY} (a build failure, not a crash)"
        ));
    }
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let jobs = jobs(cores, targets.len());
    let mut runs = Vec::new();
    for t in &targets {
        let n = write_seeds(&fuzz_dir, t)?;
        eprintln!(
            "xtask: fuzz: {t}: {n} seed(s), {} s ({jobs} at a time)",
            opts.seconds
        );
        let mut cmd = Command::new("cargo");
        cmd.args(fuzz_run_args(t, opts.seconds)).current_dir(root);
        runs.push((t.clone(), cmd));
    }
    let log_dir = root.join("target").join("fuzz-logs");
    let mut started = Vec::new();
    let outcome = run_all(runs, jobs, &log_dir, &mut std::io::stderr(), &mut started);
    let (failed, error) = match outcome {
        Ok(failed) => (failed, None),
        Err((e, failed)) => (failed, Some(e)),
    };
    Ok(Report {
        targets,
        started,
        failed,
        error,
    })
}

/// Maps [`run_all`]'s outcome to `run`'s result: any failed target or start error fails it,
/// and the crash message names every failed target.
pub fn run_result(outcome: Result<Vec<String>, (String, Vec<String>)>) -> Result<(), String> {
    let crashed = |failed: &[String]| {
        format!(
            "crash, hang or out-of-memory in {failed:?}; artifacts are in fuzz/artifacts/ \
             (minimize with `cargo +{NIGHTLY} fuzz tmin`, then add the repro to tests/regress/)"
        )
    };
    match outcome {
        Ok(failed) if failed.is_empty() => Ok(()),
        Ok(failed) => Err(crashed(&failed)),
        Err((e, failed)) if failed.is_empty() => Err(e),
        Err((e, failed)) => Err(format!("{e}; {}", crashed(&failed))),
    }
}

/// The arguments after `cargo` for one target's run, with the judge's limits (CONTRACT §3).
pub fn fuzz_run_args(target: &str, seconds: u32) -> Vec<String> {
    let mut args = vec![plus_nightly()];
    args.extend(["fuzz", "run", "--debug-assertions", target, "--"].map(str::to_owned));
    args.push(format!("-max_total_time={seconds}"));
    args.push(format!("-rss_limit_mb={RSS_LIMIT_MB}"));
    args.push(format!("-timeout={INPUT_TIMEOUT_SECONDS}"));
    args
}

/// The line `run_all` writes before a finished run's log.
pub fn log_header(name: &str, ok: bool) -> String {
    format!(
        "xtask: fuzz: ---- {name} ({}) ----",
        if ok { "ok" } else { "FAILED" }
    )
}

/// How many targets run at once: one per core, never more than there are targets, at least 1.
/// libFuzzer without `-jobs` uses one core, so more runs than cores would only split the
/// cores and fuzz each target less in the same `-max_total_time`.
pub fn jobs(cores: usize, targets: usize) -> usize {
    cores.min(targets).max(1)
}

/// Runs the named commands, at most `jobs` at a time, in the given order. Each command's
/// stdout and stderr go to `<log_dir>/<name>.log`, which is copied to `out` after a
/// [`log_header`] once the command ends, so parallel runs do not interleave their output.
/// Returns the names whose command exited non-zero or was killed by a signal. A command that
/// cannot start, or a log that cannot be created, is an error, returned with the names that
/// failed so far, only after every started command ended; no later batch starts. `started`
/// gets each name whose command was spawned, in order.
pub fn run_all(
    runs: Vec<(String, Command)>,
    jobs: usize,
    log_dir: &Path,
    out: &mut dyn std::io::Write,
    started_names: &mut Vec<String>,
) -> Result<Vec<String>, (String, Vec<String>)> {
    std::fs::create_dir_all(log_dir).map_err(|e| {
        (
            format!("cannot create {}: {e}", log_dir.display()),
            Vec::new(),
        )
    })?;
    let mut failed = Vec::new();
    let mut error = None;
    let mut pending = runs.into_iter();
    loop {
        // Start one batch. Batches, not a pool: the targets get the same time, so a batch
        // takes about `-max_total_time` and a pool would finish no sooner.
        let mut started = Vec::new();
        for (name, mut cmd) in pending.by_ref().take(jobs.max(1)) {
            let log = log_dir.join(format!("{name}.log"));
            let spawned = std::fs::File::create(&log)
                .and_then(|out| Ok((out.try_clone()?, out)))
                .map_err(|e| format!("cannot create {}: {e}", log.display()))
                .and_then(|(out, err)| {
                    cmd.stdout(out)
                        .stderr(err)
                        .spawn()
                        .map_err(|e| format!("cannot run {name}: {e}"))
                });
            match spawned {
                Ok(child) => {
                    started_names.push(name.clone());
                    started.push((name, log, child));
                }
                Err(e) => {
                    error.get_or_insert(e);
                    break;
                }
            }
        }
        if started.is_empty() {
            break;
        }
        for (name, log, mut child) in started {
            let ok = child.wait().is_ok_and(|s| s.success());
            // Writing to `out` (stderr) is best effort: a closed stderr must not hide a crash.
            writeln!(out, "{}", log_header(&name, ok)).ok();
            match std::fs::read(&log) {
                Ok(text) => write!(out, "{}", String::from_utf8_lossy(&text)).ok(),
                Err(e) => writeln!(out, "xtask: fuzz: cannot read {}: {e}", log.display()).ok(),
            };
            if !ok {
                failed.push(name);
            }
        }
        if error.is_some() {
            break;
        }
    }
    match error {
        Some(e) => Err((e, failed)),
        None => Ok(failed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut c = Command::new("sh");
        c.args(["-c", script]);
        c
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-fuzz-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    /// `name` creates its marker, then waits up to `tries` × 50 ms for `other`'s marker: it
    /// succeeds only if both commands run at the same time.
    fn rendezvous(dir: &Path, name: &str, other: &str, tries: u32) -> Command {
        sh(&format!(
            "cd '{}' && touch {name}.up && i=0; while [ ! -e {other}.up ]; do \
             i=$((i+1)); [ $i -gt {tries} ] && exit 1; sleep 0.05; done",
            dir.display()
        ))
    }

    #[test]
    fn finds_the_pinned_nightly_in_the_toolchain_list() {
        let host = "-x86_64-unknown-linux-gnu";
        assert!(nightly_listed(&format!(
            "1.100.0{host} (active)\n{NIGHTLY}{host}\n"
        )));
        assert!(nightly_listed(&format!("{NIGHTLY}{host} (active)")));
        assert!(nightly_listed(NIGHTLY));
        assert!(!nightly_listed(""));
        assert!(!nightly_listed(&format!("nightly{host}\n1.100.0{host}")));
        // Another date that shares the prefix, and the name inside another line.
        assert!(!nightly_listed(&format!("{NIGHTLY}5{host}")));
        assert!(!nightly_listed(&format!("x{NIGHTLY}{host}")));
    }

    #[test]
    fn accepts_only_the_pinned_cargo_fuzz() {
        assert!(cargo_fuzz_pinned(&format!(
            "cargo-fuzz {CARGO_FUZZ_VERSION}\n"
        )));
        assert!(!cargo_fuzz_pinned("cargo-fuzz 0.13.1\n"));
        assert!(!cargo_fuzz_pinned(&format!(
            "cargo-fuzz {CARGO_FUZZ_VERSION}.1"
        )));
        assert!(!cargo_fuzz_pinned(&format!(
            "other-tool {CARGO_FUZZ_VERSION}"
        )));
        assert!(!cargo_fuzz_pinned(&format!(
            "cargo-fuzz {CARGO_FUZZ_VERSION} extra"
        )));
        assert!(!cargo_fuzz_pinned(""));
    }

    #[test]
    fn installs_only_what_is_missing() {
        let listed = format!("{NIGHTLY}-x86_64-unknown-linux-gnu\n");
        assert!(!needs_nightly(Some(&listed)));
        assert!(needs_nightly(Some("1.100.0-x86_64-unknown-linux-gnu\n")));
        assert!(needs_nightly(Some(
            "nightly-2026-01-01-x86_64-unknown-linux-gnu\n"
        )));
        assert!(needs_nightly(None));
        let pinned = format!("cargo-fuzz {CARGO_FUZZ_VERSION}\n");
        assert!(!needs_cargo_fuzz(Some(&pinned)));
        assert!(needs_cargo_fuzz(Some("cargo-fuzz 0.12.0\n")));
        assert!(needs_cargo_fuzz(Some("cargo-fuzz 0.13.1\n")));
        assert!(needs_cargo_fuzz(Some(&format!(
            "cargo-fuzz {CARGO_FUZZ_VERSION}.1\n"
        ))));
        assert!(needs_cargo_fuzz(None));
        assert_eq!(
            NIGHTLY_INSTALL,
            [
                "rustup",
                "toolchain",
                "install",
                NIGHTLY,
                "--profile",
                "minimal"
            ]
        );
        assert_eq!(
            CARGO_FUZZ_INSTALL,
            [
                "cargo",
                "install",
                "cargo-fuzz",
                "--force",
                "--locked",
                "--version",
                "0.13.2"
            ]
        );
    }

    #[test]
    fn run_args_carry_the_judge_limits() {
        assert_eq!(
            fuzz_run_args("engine_open", 30),
            [
                "+nightly-2026-11-20",
                "fuzz",
                "run",
                "--debug-assertions",
                "engine_open",
                "--",
                "-max_total_time=30",
                "-rss_limit_mb=2048",
                "-timeout=10",
            ]
        );
    }

    #[test]
    fn jobs_is_cores_capped_by_targets() {
        assert_eq!(jobs(4, 3), 3);
        assert_eq!(jobs(3, 3), 3);
        assert_eq!(jobs(2, 3), 2);
        assert_eq!(jobs(1, 3), 1);
        assert_eq!(jobs(8, 1), 1);
        assert_eq!(jobs(0, 3), 1);
        assert_eq!(jobs(4, 0), 1);
    }

    #[test]
    fn runs_a_batch_at_the_same_time() {
        let dir = scratch("together");
        let runs = vec![
            ("a".to_owned(), rendezvous(&dir, "a", "b", 200)),
            ("b".to_owned(), rendezvous(&dir, "b", "a", 200)),
        ];
        assert_eq!(
            run_all(runs, 2, &dir.join("logs"), &mut Vec::new(), &mut Vec::new()),
            Ok(vec![])
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn one_job_runs_one_at_a_time_in_order() {
        // With jobs = 1, `a` never sees `b` start: `a` fails, then `b` (which waits for `a`'s
        // marker, already there) passes.
        let dir = scratch("serial");
        let runs = vec![
            ("a".to_owned(), rendezvous(&dir, "a", "b", 10)),
            ("b".to_owned(), rendezvous(&dir, "b", "a", 10)),
        ];
        assert_eq!(
            run_all(runs, 1, &dir.join("logs"), &mut Vec::new(), &mut Vec::new()),
            Ok(vec!["a".to_owned()])
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reports_every_failure_and_keeps_each_log() {
        let dir = scratch("fail");
        let logs = dir.join("logs");
        let runs = vec![
            ("ok".to_owned(), sh("echo out-ok; echo err-ok >&2")),
            ("exit".to_owned(), sh("echo out-exit; exit 3")),
            ("signal".to_owned(), sh("kill -9 $$")),
            ("later".to_owned(), sh("exit 1")),
        ];
        let mut out = Vec::new();
        let mut started = Vec::new();
        assert_eq!(
            run_all(runs, 3, &logs, &mut out, &mut started),
            Ok(vec![
                "exit".to_owned(),
                "signal".to_owned(),
                "later".to_owned()
            ])
        );
        assert_eq!(started, ["ok", "exit", "signal", "later"]);
        let read = |n: &str| std::fs::read_to_string(logs.join(format!("{n}.log")));
        assert_eq!(read("ok").ok().as_deref(), Some("out-ok\nerr-ok\n"));
        assert_eq!(read("exit").ok().as_deref(), Some("out-exit\n"));
        // Each log is copied to `out` after its header, in the given order.
        assert_eq!(
            String::from_utf8_lossy(&out),
            "xtask: fuzz: ---- ok (ok) ----\nout-ok\nerr-ok\n\
             xtask: fuzz: ---- exit (FAILED) ----\nout-exit\n\
             xtask: fuzz: ---- signal (FAILED) ----\n\
             xtask: fuzz: ---- later (FAILED) ----\n"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_unreadable_log_is_reported_in_its_place() {
        let dir = scratch("unreadable");
        let logs = dir.join("logs");
        let log = logs.join("gone.log");
        let runs = vec![(
            "gone".to_owned(),
            sh(&format!("echo lost; rm '{}'", log.display())),
        )];
        let mut out = Vec::new();
        assert_eq!(
            run_all(runs, 1, &logs, &mut out, &mut Vec::new()),
            Ok(vec![])
        );
        let text = String::from_utf8_lossy(&out);
        assert!(
            text.starts_with(&format!(
                "xtask: fuzz: ---- gone (ok) ----\nxtask: fuzz: cannot read {}: ",
                log.display()
            )),
            "{text}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_command_that_cannot_start_is_an_error_after_the_others_end() {
        let dir = scratch("spawn");
        let marker = dir.join("first-ran");
        let runs = vec![
            (
                "first".to_owned(),
                sh(&format!("sleep 0.2; touch '{}'; exit 1", marker.display())),
            ),
            (
                "missing".to_owned(),
                Command::new(dir.join("no-such-program")),
            ),
            ("never".to_owned(), sh("exit 0")),
        ];
        let mut started = Vec::new();
        let got = run_all(runs, 3, &dir.join("logs"), &mut Vec::new(), &mut started);
        // The spawn error keeps the failure found before it.
        let Err((e, failed)) = &got else {
            panic!("{got:?}")
        };
        assert!(e.contains("cannot run missing"), "{e}");
        assert_eq!(failed, &["first".to_owned()]);
        // `first` was waited for, not left running; `never` was not started.
        assert!(marker.is_file());
        assert!(!dir.join("logs/never.log").exists());
        assert_eq!(started, ["first"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn run_result_fails_on_any_failed_target_or_start_error() {
        // Audit 0006 AC.7: the mutant `Ok(failed) => Ok(())` passed every test.
        let crash = format!(
            "crash, hang or out-of-memory in [\"a\", \"b\"]; artifacts are in fuzz/artifacts/ \
             (minimize with `cargo +{NIGHTLY} fuzz tmin`, then add the repro to tests/regress/)"
        );
        assert_eq!(run_result(Ok(vec![])), Ok(()));
        assert_eq!(
            run_result(Ok(vec!["a".into(), "b".into()])),
            Err(crash.clone())
        );
        assert_eq!(
            run_result(Err(("cannot run c: x".into(), vec![]))),
            Err("cannot run c: x".into())
        );
        assert_eq!(
            run_result(Err((
                "cannot run c: x".into(),
                vec!["a".into(), "b".into()]
            ))),
            Err(format!("cannot run c: x; {crash}"))
        );
    }

    #[test]
    fn ci_result_keeps_both_problems() {
        let fmt = "fuzz/ is not formatted (run `cargo fmt --manifest-path fuzz/Cargo.toml`)";
        assert_eq!(ci_result(Ok(()), true), Ok(()));
        assert_eq!(ci_result(Ok(()), false), Err(fmt.to_owned()));
        assert_eq!(ci_result(Err("crash".into()), true), Err("crash".into()));
        assert_eq!(
            ci_result(Err("crash".into()), false),
            Err(format!("crash; {fmt}"))
        );
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_options() {
        assert_eq!(
            parse(&args(&[])),
            Ok(Options {
                seconds: NIGHTLY_SECONDS,
                target: None
            })
        );
        assert_eq!(parse(&args(&["--short"])).map(|o| o.seconds), Ok(30));
        assert_eq!(
            parse(&args(&["--seconds", "5", "--target", "engine_open"])),
            Ok(Options {
                seconds: 5,
                target: Some("engine_open".to_owned())
            })
        );
        for bad in [
            &["--seconds"][..],
            &["--seconds", "0"],
            &["--seconds", "3601"],
            &["--seconds", "x"],
            &["--target"],
            &["--long"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn every_listed_target_has_seeds() {
        // fuzz/Cargo.toml's [[bin]] names. A new target without seeds fails here on purpose:
        // add its seeds to `seeds_for` (or an explicit empty arm with a reason).
        let manifest = include_str!("../../fuzz/Cargo.toml");
        let names: Vec<&str> = manifest
            .lines()
            .filter_map(|l| l.strip_prefix("name = \""))
            .filter_map(|l| l.strip_suffix('"'))
            .filter(|n| *n != "jpdf-fuzz")
            .collect();
        assert_eq!(names, ["engine_open", "engine_objects", "syntax_resolve"]);
        for n in names {
            assert!(!seeds_for(n).is_empty(), "{n} has no seeds");
        }
    }

    #[test]
    fn writes_seed_files_and_keeps_other_corpus_files() {
        let dir = std::env::temp_dir().join(format!("xtask-fuzz-{}", std::process::id()));
        let corpus = dir.join("corpus/engine_open");
        std::fs::create_dir_all(&corpus).ok();
        std::fs::write(corpus.join("found-by-fuzzer"), b"x").ok();
        assert_eq!(write_seeds(&dir, "engine_open"), Ok(6));
        assert!(corpus.join("seed-one-page.pdf").is_file());
        assert!(corpus.join("found-by-fuzzer").is_file());
        assert_eq!(write_seeds(&dir, "no_such_target"), Ok(0));
        std::fs::remove_dir_all(&dir).ok();
    }
}
