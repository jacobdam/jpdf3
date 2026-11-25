//! The bench and the extra fuzz time after the regress runs, and `nightly.json`
//! (specs/tooling/nightly.md items 11–14).

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jpdf_bench::engine::{Outcome, Status};

use super::Job;
use crate::{bench, fuzz};

/// `--bench-minutes` range and default (spec item 1); 0 skips the bench.
pub const MAX_BENCH_MINUTES: u64 = 120;
pub const DEFAULT_BENCH_MINUTES: u64 = 20;
/// `--fuzz-seconds` range (the cap of `cargo xtask fuzz --seconds`) and default; 0 skips it.
pub const MAX_FUZZ_SECONDS: u64 = 3600;
pub const DEFAULT_FUZZ_SECONDS: u64 = 1200;
/// The bench sample: the first verified regress files in blob order (spec item 11).
pub const BENCH_FILES: usize = 20;
/// Artifacts copied per fuzz target (spec item 12).
pub const MAX_ARTIFACTS: usize = 100;

/// How the bench ended (spec item 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchResult {
    pub status: &'static str,
    pub files: usize,
    pub exit: Option<i32>,
    pub signal: Option<i32>,
    pub ms: Option<u128>,
    /// `bench.json` was written by this night's bench.
    pub report: bool,
    pub error: Option<String>,
}

impl BenchResult {
    fn skipped() -> Self {
        BenchResult {
            status: "skipped",
            files: 0,
            exit: None,
            signal: None,
            ms: None,
            report: false,
            error: None,
        }
    }

    fn failed(files: usize, e: String) -> Self {
        BenchResult {
            status: "failed",
            files,
            error: Some(e),
            ..Self::skipped()
        }
    }

    /// A tool failure (spec item 10): anything but `ok`, `timeout` and `skipped`.
    pub fn tool_failure(&self) -> bool {
        self.status == "failed"
    }
}

/// One fuzz target's outcome: its name, whether it ran clean, the artifacts copied to `DIR`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetResult {
    pub name: String,
    pub ok: bool,
    pub artifacts: Vec<String>,
}

/// How the fuzz pass ended (spec item 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzResult {
    pub status: &'static str,
    pub seconds: u64,
    pub targets: Vec<TargetResult>,
    pub error: Option<String>,
}

impl FuzzResult {
    pub fn tool_failure(&self) -> bool {
        self.status == "failed"
    }
}

/// The first `n` verified files, in the jobs' (blob-id) order (spec item 11).
pub fn bench_inputs(jobs: &[Job], n: usize) -> Vec<&Path> {
    jobs.iter()
        .filter_map(|j| j.path.as_deref())
        .take(n)
        .collect()
}

/// The bench's status from how jpdf-bench ended (`None`: it could not start). Only exit 0 is
/// `ok`; a kill at the bench's time limit is `timeout`; anything else is `failed`.
pub fn bench_status(o: Option<&Outcome>) -> &'static str {
    match o.map(|o| o.status) {
        Some(Status::Ok) => "ok",
        Some(Status::Timeout) => "timeout",
        _ => "failed",
    }
}

/// Removes `path` if it is there; a missing file is fine.
fn remove_file(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("cannot remove {}: {e}", path.display()))
        }
        _ => Ok(()),
    }
}

/// Makes `dir` an empty directory.
fn fresh_dir(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("cannot remove {}: {e}", dir.display()));
        }
        _ => {}
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))
}

/// Copies the bench sample into `dir` (emptied first) under the files' own names.
pub fn stage_bench_corpus(inputs: &[&Path], dir: &Path) -> Result<(), String> {
    fresh_dir(dir)?;
    for p in inputs {
        let name = p
            .file_name()
            .ok_or_else(|| format!("no file name in {}", p.display()))?;
        let to = dir.join(name);
        std::fs::copy(p, &to).map_err(|e| format!("cannot copy {}: {e}", p.display()))?;
    }
    Ok(())
}

/// jpdf-bench's arguments for the staged sample (spec item 11).
pub fn bench_argv(cli: &Path, report: &Path, corpus: &Path, files: usize) -> Vec<OsString> {
    let mut a = bench::bench_args(cli, report, &[]);
    a.extend([
        "--corpus".into(),
        corpus.into(),
        "--sample".into(),
        files.to_string().into(),
    ]);
    a
}

/// Builds `jpdf-cli` and `jpdf-bench`; returns their release directory ([`bench::build`]).
pub type Build<'a> = &'a mut dyn FnMut() -> Result<PathBuf, String>;
/// Runs a program with arguments under a time limit ([`super::exec`], with the memory wrapper).
pub type Exec<'a> = &'a mut dyn FnMut(&Path, &[OsString], Duration) -> Option<Outcome>;

/// Runs the bench on the staged sample under `minutes` (spec item 11).
pub fn run_bench(out: &Path, jobs: &[Job], minutes: u64, build: Build, exec: Exec) -> BenchResult {
    if minutes == 0 {
        return BenchResult::skipped();
    }
    let inputs = bench_inputs(jobs, BENCH_FILES);
    let files = inputs.len();
    let abs = |p: PathBuf| std::path::absolute(&p).map_err(|e| format!("{}: {e}", p.display()));
    let staged = (|| {
        let report = abs(out.join("bench.json"))?;
        let corpus = abs(out.join("bench-corpus"))?;
        // A report from an earlier night must never stand for this one.
        remove_file(&report)?;
        stage_bench_corpus(&inputs, &corpus)?;
        let release = abs(build()?)?;
        Ok::<_, String>((release, report, corpus))
    })();
    let (release, report, corpus) = match staged {
        Ok(s) => s,
        Err(e) => return BenchResult::failed(files, e),
    };
    let argv = bench_argv(&release.join("jpdf-cli"), &report, &corpus, files);
    let limit = Duration::from_secs(minutes.saturating_mul(60));
    let o = exec(&release.join("jpdf-bench"), &argv, limit);
    let status = bench_status(o.as_ref());
    if status != "ok"
        && let Some(o) = &o
        && !o.stderr.is_empty()
    {
        eprintln!(
            "xtask: nightly: bench stderr (first 4 KiB):\n{}",
            String::from_utf8_lossy(&o.stderr)
        );
    }
    BenchResult {
        status,
        files,
        exit: o.as_ref().and_then(|o| o.code),
        signal: o.as_ref().and_then(|o| o.signal),
        ms: o.as_ref().map(|o| o.took.as_millis()),
        report: status == "ok" && report.is_file(),
        error: match (&o, status) {
            (None, _) => Some(format!(
                "cannot start {}",
                release.join("jpdf-bench").display()
            )),
            (Some(o), "failed") => Some(format!("jpdf-bench ended with {:?}", o.status.id())),
            _ => None,
        },
    }
}

/// The regular files directly in `dir` (no symlinks), by name; an absent dir has none.
pub fn files_in(dir: &Path) -> BTreeSet<OsString> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return BTreeSet::new();
    };
    rd.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name())
        .collect()
}

/// Copies the artifacts in `from` that are not in `before` into `to`, at most
/// [`MAX_ARTIFACTS`] by name order; returns their names relative to `DIR` (`fuzz/<target>/…`).
pub fn copy_new_artifacts(
    from: &Path,
    before: &BTreeSet<OsString>,
    to: &Path,
    target: &str,
) -> Result<Vec<String>, String> {
    let new: Vec<OsString> = files_in(from)
        .into_iter()
        .filter(|n| !before.contains(n))
        .collect();
    if new.len() > MAX_ARTIFACTS {
        eprintln!(
            "xtask: nightly: fuzz: {target}: {} new artifacts, copying the first {MAX_ARTIFACTS}",
            new.len()
        );
    }
    let mut names = Vec::new();
    for n in new.into_iter().take(MAX_ARTIFACTS) {
        std::fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
        let src = from.join(&n);
        std::fs::copy(&src, to.join(&n))
            .map_err(|e| format!("cannot copy {}: {e}", src.display()))?;
        names.push(format!("fuzz/{target}/{}", n.to_string_lossy()));
    }
    Ok(names)
}

/// The fuzz pass's status (spec item 12): a tool error is `failed`, else any failed target
/// is `crash`, else `ok`.
pub fn fuzz_status(error: bool, any_failed: bool) -> &'static str {
    match (error, any_failed) {
        (true, _) => "failed",
        (false, true) => "crash",
        (false, false) => "ok",
    }
}

/// Runs every fuzz target for `seconds` each and copies what they found (spec item 12).
pub fn run_fuzz(root: &Path, out: &Path, seconds: u64) -> FuzzResult {
    let mut r = FuzzResult {
        status: "skipped",
        seconds,
        targets: Vec::new(),
        error: None,
    };
    if seconds == 0 {
        return r;
    }
    let artifacts = root.join("fuzz").join("artifacts");
    let dest = out.join("fuzz");
    // Every target directory's files before the pass, so only this night's are copied.
    let snapshot: Vec<(OsString, BTreeSet<OsString>)> = std::fs::read_dir(&artifacts)
        .map(|rd| {
            rd.flatten()
                .map(|e| (e.file_name(), files_in(&e.path())))
                .collect()
        })
        .unwrap_or_default();
    let pass = (|| {
        fresh_dir(&dest)?;
        // Checked before anything is installed (a tree without fuzz/ installs nothing).
        fuzz::require_project(root)?;
        if std::env::var_os("CI").is_some() {
            fuzz::install_missing_tools(root)?;
        }
        let secs = u32::try_from(seconds).map_err(|_| format!("bad fuzz time {seconds}"))?;
        fuzz::run_report(
            root,
            &fuzz::Options {
                seconds: secs,
                target: None,
            },
        )
    })();
    let report = match pass {
        Ok(rep) => rep,
        Err(e) => {
            r.status = "failed";
            r.error = Some(e);
            return r;
        }
    };
    let mut error = report.error.clone();
    for t in &report.targets {
        let old = snapshot
            .iter()
            .find(|(n, _)| n.to_string_lossy() == t.as_str())
            .map(|(_, s)| s.clone())
            .unwrap_or_default();
        let copied = copy_new_artifacts(&artifacts.join(t), &old, &dest.join(t), t);
        let names = copied.unwrap_or_else(|e| {
            error.get_or_insert(e);
            Vec::new()
        });
        r.targets.push(TargetResult {
            name: t.clone(),
            ok: !report.failed.contains(t),
            artifacts: names,
        });
    }
    r.status = fuzz_status(error.is_some(), !report.failed.is_empty());
    r.error = error;
    r
}

fn opt<T: std::fmt::Display>(s: &mut String, v: Option<T>) {
    super::json_opt(s, v);
}

fn text(s: &mut String, v: Option<&str>) {
    match v {
        Some(t) => jpdf_bench::json::string(s, t),
        None => s.push_str("null"),
    }
}

/// `DIR/nightly.json` (spec item 13).
pub fn nightly_json(b: &BenchResult, f: &FuzzResult, bench_minutes: u64) -> String {
    let mut s = String::from("{\"schema\":1,\"regress\":\"regress.json\",\"bench\":{");
    s.push_str(&format!(
        "\"status\":\"{}\",\"minutes\":{bench_minutes},\"files\":{},\"exit\":",
        b.status, b.files
    ));
    opt(&mut s, b.exit);
    s.push_str(",\"signal\":");
    opt(&mut s, b.signal);
    s.push_str(",\"ms\":");
    opt(&mut s, b.ms);
    s.push_str(",\"report\":");
    text(&mut s, b.report.then_some("bench.json"));
    s.push_str(",\"error\":");
    text(&mut s, b.error.as_deref());
    s.push_str(&format!(
        "}},\"fuzz\":{{\"status\":\"{}\",\"seconds_per_target\":{},\"targets\":[",
        f.status, f.seconds
    ));
    for (i, t) in f.targets.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"name\":");
        jpdf_bench::json::string(&mut s, &t.name);
        s.push_str(&format!(",\"ok\":{},\"artifacts\":[", t.ok));
        for (k, a) in t.artifacts.iter().enumerate() {
            if k > 0 {
                s.push(',');
            }
            jpdf_bench::json::string(&mut s, a);
        }
        s.push_str("]}");
    }
    s.push_str("],\"error\":");
    text(&mut s, f.error.as_deref());
    s.push_str("}}\n");
    s
}

/// The bench, the fuzz pass and `nightly.json` (spec items 11–13). Returns whether any of
/// them was a tool failure (spec item 10).
pub fn run(
    root: &Path,
    out: &Path,
    jobs: &[Job],
    bench_minutes: u64,
    fuzz_seconds: u64,
    build: Build,
    exec: Exec,
) -> bool {
    let path = out.join("nightly.json");
    // An earlier night's summary must not stand next to this night's regress results.
    let mut failed = remove_file(&path)
        .map_err(|e| eprintln!("xtask: nightly: {e}"))
        .is_err();
    let b = run_bench(out, jobs, bench_minutes, build, exec);
    eprintln!(
        "xtask: nightly: bench: {} ({} files){}",
        b.status,
        b.files,
        b.error
            .as_deref()
            .map(|e| format!(": {e}"))
            .unwrap_or_default()
    );
    let f = run_fuzz(root, out, fuzz_seconds);
    let found: usize = f.targets.iter().map(|t| t.artifacts.len()).sum();
    eprintln!(
        "xtask: nightly: fuzz: {} ({} targets, {fuzz_seconds} s each, {found} artifacts){}",
        f.status,
        f.targets.len(),
        f.error
            .as_deref()
            .map(|e| format!(": {e}"))
            .unwrap_or_default()
    );
    failed |= b.tool_failure() || f.tool_failure();
    if let Err(e) = super::write(&path, &nightly_json(&b, &f, bench_minutes)) {
        eprintln!("xtask: nightly: {e}");
        failed = true;
    }
    failed
}

#[cfg(test)]
mod tests;
