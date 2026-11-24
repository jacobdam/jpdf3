//! `cargo xtask nightly [--out DIR] [--cache DIR] [--limit N] [--previous FILE] [--max-minutes M]`,
//! part 1: the regress tier through `jpdf-cli` under the judge's limits, per-file results and a
//! diff against the previous night. Spec: specs/tooling/nightly.md (ops/judge/CONTRACT.md §2).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use jpdf_bench::engine::{self, Outcome, Status};
use jpdf_corpus::{Tier, fetch, manifest};
use jpdf_testgen::json::{self, Value};

/// Most `--limit` rows and most files in a previous results file (the corpus manifest's cap).
const MAX_ROWS: u64 = 200_000;
/// Most `--max-minutes`.
const MAX_MINUTES: u64 = 1440;
const DEFAULT_MINUTES: u64 = 120;
/// The judge's address-space limit, in KiB (`ulimit -v`): 2 GiB (ops/judge/lib/common.py).
const MEM_KIB: u64 = 2_097_152;
/// Checks that the shell can set the limit (spec item 6).
const MEM_PROBE: &str = "ulimit -v 2097152 && ulimit -v";
/// Sets the limit, then becomes `jpdf-cli` (`$0`) with its arguments (`$@`), never
/// interpolated. A failed `ulimit` exits 125: a crash, never a run without the limit.
const MEM_WRAPPER: &str = "ulimit -v 2097152 || exit 125; exec \"$0\" \"$@\"";
/// stderr bytes kept per run.
const STDERR_CAP: u64 = 4096;
/// Largest previous results file read.
const MAX_PREVIOUS: u64 = 64 * 1024 * 1024;

/// The five runs per file (spec item 4), in their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Op {
    Info,
    Render1,
    Render2,
    Text1,
    Text2,
}

const OPS: [Op; 5] = [Op::Info, Op::Render1, Op::Render2, Op::Text1, Op::Text2];

impl Op {
    fn id(self) -> &'static str {
        match self {
            Op::Info => "info",
            Op::Render1 => "render1",
            Op::Render2 => "render2",
            Op::Text1 => "text1",
            Op::Text2 => "text2",
        }
    }

    fn from_id(s: &str) -> Option<Op> {
        OPS.into_iter().find(|o| o.id() == s)
    }

    /// CONTRACT §1: info 20 s, render 30 s, other commands 60 s.
    fn limit(self) -> Duration {
        Duration::from_secs(match self {
            Op::Info => 20,
            Op::Render1 | Op::Render2 => 30,
            Op::Text1 | Op::Text2 => 60,
        })
    }

    fn args(self, file: &Path, png: &Path) -> Vec<OsString> {
        let mut a: Vec<OsString> = Vec::new();
        let (cmd, page) = match self {
            Op::Info => ("info", None),
            Op::Render1 => ("render", Some("1")),
            Op::Render2 => ("render", Some("2")),
            Op::Text1 => ("text", Some("1")),
            Op::Text2 => ("text", Some("2")),
        };
        a.push(cmd.into());
        a.push(file.into());
        if let Some(p) = page {
            a.extend(["--page".into(), p.into()]);
        }
        if cmd == "render" {
            a.extend(["--dpi".into(), "72".into(), "--out".into(), png.into()]);
        }
        a
    }
}

/// Every status a run can have in a results file (spec items 5 and 7).
const STATUSES: [&str; 6] = [
    "ok",
    "error",
    "not_implemented",
    "timeout",
    "crash",
    "not_run",
];

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub out: PathBuf,
    pub cache: Option<PathBuf>,
    pub limit: Option<usize>,
    pub previous: Option<PathBuf>,
    pub max_minutes: u64,
}

/// A plain decimal number in `lo..=hi` (no sign, no leading `+`).
fn number(flag: &str, v: &str, lo: u64, hi: u64) -> Result<u64, String> {
    v.bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| v.parse::<u64>().ok())
        .flatten()
        .filter(|n| (lo..=hi).contains(n))
        .ok_or_else(|| format!("bad {flag} {v:?} ({lo}-{hi})"))
}

/// Parses the arguments after `nightly` (spec item 1).
pub fn parse(args: &[String]) -> Result<Options, String> {
    let (mut out, mut cache, mut limit, mut previous, mut minutes) = (None, None, None, None, None);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let v = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        let twice = format!("{flag} given twice");
        let dup = match flag.as_str() {
            "--out" => out.replace(PathBuf::from(v)).is_some(),
            "--cache" => cache.replace(PathBuf::from(v)).is_some(),
            "--previous" => previous.replace(PathBuf::from(v)).is_some(),
            "--limit" => {
                let n = number(flag, v, 1, MAX_ROWS)?;
                let n = usize::try_from(n).map_err(|_| format!("bad {flag} {v:?}"))?;
                limit.replace(n).is_some()
            }
            "--max-minutes" => minutes.replace(number(flag, v, 1, MAX_MINUTES)?).is_some(),
            other => return Err(format!("unknown argument {other:?}")),
        };
        if dup {
            return Err(twice);
        }
    }
    Ok(Options {
        out: out.unwrap_or_else(|| PathBuf::from("target/xtask-out")),
        cache,
        limit,
        previous,
        max_minutes: minutes.unwrap_or(DEFAULT_MINUTES),
    })
}

/// One manifest row: `path` is `None` when the cached file failed the blob check (missing).
struct Job {
    blob: String,
    url: String,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Run {
    op: Op,
    status: &'static str,
    exit: Option<i32>,
    signal: Option<i32>,
    ms: Option<u128>,
    stderr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileResult {
    blob: String,
    url: String,
    /// `None`: the file is missing (never run).
    runs: Option<Vec<Run>>,
}

/// A run's record from how it ended (`None`: it could not be started, a crash).
fn record(op: Op, o: Option<Outcome>) -> Run {
    let Some(o) = o else {
        return Run {
            op,
            status: Status::Crash.id(),
            exit: None,
            signal: None,
            ms: None,
            stderr: Some("cannot start jpdf-cli".into()),
        };
    };
    let keep = matches!(o.status, Status::Crash | Status::Timeout);
    Run {
        op,
        status: o.status.id(),
        exit: o.code,
        signal: o.signal,
        ms: Some(o.took.as_millis()),
        stderr: keep.then(|| String::from_utf8_lossy(&o.stderr).into_owned()),
    }
}

/// Runs every file's five ops in order (spec items 4–7). `elapsed` is the time since the
/// runs started; from the first check where it reaches `budget`, every run is `not_run`.
fn run_files(
    jobs: &[Job],
    png: &Path,
    budget: Duration,
    elapsed: &mut dyn FnMut() -> Duration,
    exec: &mut dyn FnMut(Op, &[OsString]) -> Option<Outcome>,
) -> (Vec<FileResult>, bool) {
    let mut over = false;
    let mut files = Vec::with_capacity(jobs.len());
    for j in jobs {
        let runs = j.path.as_ref().map(|path| {
            OPS.iter()
                .map(|&op| {
                    if !over && elapsed() >= budget {
                        over = true;
                    }
                    if over {
                        return Run {
                            op,
                            status: "not_run",
                            exit: None,
                            signal: None,
                            ms: None,
                            stderr: None,
                        };
                    }
                    if matches!(op, Op::Render1 | Op::Render2) {
                        let _ = std::fs::remove_file(png);
                    }
                    record(op, exec(op, &op.args(path, png)))
                })
                .collect()
        });
        files.push(FileResult {
            blob: j.blob.clone(),
            url: j.url.clone(),
            runs,
        });
    }
    (files, !over)
}

/// Whether `/bin/sh` can set the judge's address-space limit here (spec item 6).
fn memory_limit_applies() -> bool {
    Command::new("/bin/sh")
        .args(["-c", MEM_PROBE])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success() && o.stdout == format!("{MEM_KIB}\n").as_bytes())
}

/// Runs `cli args` once under `limit`, through the memory wrapper when `mem` (spec item 6).
fn exec(cli: &Path, mem: bool, args: &[OsString], limit: Duration) -> Option<Outcome> {
    if mem {
        let mut a: Vec<OsString> = vec!["-c".into(), MEM_WRAPPER.into(), cli.into()];
        a.extend_from_slice(args);
        engine::run_child(Path::new("/bin/sh"), &a, limit, STDERR_CAP)
    } else {
        engine::run_child(cli, args, limit, STDERR_CAP)
    }
}

fn json_opt<T: std::fmt::Display>(out: &mut String, v: Option<T>) {
    match v {
        Some(x) => out.push_str(&x.to_string()),
        None => out.push_str("null"),
    }
}

/// `regress.json` (spec item 8).
fn results_json(files: &[FileResult], mem: bool, max_minutes: u64, complete: bool) -> String {
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    let mut missing: u64 = 0;
    for f in files {
        match &f.runs {
            None => missing = missing.saturating_add(1),
            Some(runs) => {
                for r in runs {
                    let c = counts.entry(r.status).or_default();
                    *c = c.saturating_add(1);
                }
            }
        }
    }
    let mut s = format!(
        "{{\"schema\":1,\"tier\":\"regress\",\"rows\":{},",
        files.len()
    );
    s.push_str("\"memory_limit_kib\":");
    json_opt(&mut s, mem.then_some(MEM_KIB));
    s.push_str(&format!(
        ",\"max_minutes\":{max_minutes},\"complete\":{complete},\"counts\":{{"
    ));
    for st in STATUSES {
        s.push_str(&format!(
            "\"{st}\":{},",
            counts.get(st).copied().unwrap_or(0)
        ));
    }
    s.push_str(&format!("\"missing\":{missing}}},\"files\":["));
    for (i, f) in files.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str("{\"blob\":");
        jpdf_bench::json::string(&mut s, &f.blob);
        s.push_str(",\"url\":");
        jpdf_bench::json::string(&mut s, &f.url);
        s.push_str(&format!(",\"missing\":{},\"runs\":[", f.runs.is_none()));
        for (k, r) in f.runs.iter().flatten().enumerate() {
            if k > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "{{\"op\":\"{}\",\"status\":\"{}\",\"exit\":",
                r.op.id(),
                r.status
            ));
            json_opt(&mut s, r.exit);
            s.push_str(",\"signal\":");
            json_opt(&mut s, r.signal);
            s.push_str(",\"ms\":");
            json_opt(&mut s, r.ms);
            if let Some(e) = &r.stderr {
                s.push_str(",\"stderr\":");
                jpdf_bench::json::string(&mut s, e);
            }
            s.push('}');
        }
        s.push_str("]}");
    }
    s.push_str("]}\n");
    s
}

/// (blob, op) → (status, url), the shape both sides of the diff are compared in.
type Statuses = BTreeMap<(String, Op), (String, String)>;

fn current_statuses(files: &[FileResult]) -> Statuses {
    let mut m = Statuses::new();
    for f in files {
        for op in OPS {
            let st = match &f.runs {
                None => Some("missing"),
                Some(runs) => runs.iter().find(|r| r.op == op).map(|r| r.status),
            };
            if let Some(st) = st {
                m.insert((f.blob.clone(), op), (st.to_string(), f.url.clone()));
            }
        }
    }
    m
}

fn is_blob(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Checks a previous `regress.json` (spec item 9) and returns its statuses. `max_files` is
/// [`MAX_ROWS`] outside tests.
fn parse_previous(text: &str, max_files: u64) -> Result<Statuses, String> {
    let v = json::parse(text)?;
    if v.get("schema") != Some(&Value::Number("1".into())) {
        return Err("schema is not 1".into());
    }
    let files = v
        .get("files")
        .and_then(Value::as_array)
        .ok_or("no files array")?;
    if u64::try_from(files.len()).map_or(true, |n| n > max_files) {
        return Err(format!("more than {max_files} files"));
    }
    let mut m = Statuses::new();
    let mut blobs = std::collections::BTreeSet::new();
    for (i, f) in files.iter().enumerate() {
        let at = |what: &str| format!("files[{i}]: {what}");
        let blob = f
            .get("blob")
            .and_then(Value::as_str)
            .filter(|b| is_blob(b))
            .ok_or_else(|| at("bad blob"))?;
        if !blobs.insert(blob) {
            return Err(at("blob listed twice"));
        }
        let url = f
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| at("no url"))?;
        let missing = match f.get("missing") {
            Some(Value::Bool(b)) => *b,
            _ => return Err(at("missing is not a boolean")),
        };
        let runs = f
            .get("runs")
            .and_then(Value::as_array)
            .ok_or_else(|| at("no runs array"))?;
        if missing {
            if !runs.is_empty() {
                return Err(at("a missing file has runs"));
            }
            for op in OPS {
                m.insert((blob.to_string(), op), ("missing".into(), url.to_string()));
            }
            continue;
        }
        for r in runs {
            let op = r
                .get("op")
                .and_then(Value::as_str)
                .and_then(Op::from_id)
                .ok_or_else(|| at("unknown op"))?;
            let st = r
                .get("status")
                .and_then(Value::as_str)
                .filter(|s| STATUSES.contains(s))
                .ok_or_else(|| at("unknown status"))?;
            if m.insert((blob.to_string(), op), (st.to_string(), url.to_string()))
                .is_some()
            {
                return Err(at("op listed twice"));
            }
        }
    }
    Ok(m)
}

fn is_failure(st: &str) -> bool {
    st == "crash" || st == "timeout"
}

/// `regress-diff.json` for a previous file (spec item 9), and the stderr summary line.
fn diff_json(previous: &str, before: &Statuses, after: &Statuses) -> (String, String) {
    let keys: std::collections::BTreeSet<&(String, Op)> =
        before.keys().chain(after.keys()).collect();
    let (mut changes, mut new_failures, mut fixed) = (0u64, 0u64, 0u64);
    let mut s = String::from("{\"schema\":1,\"previous\":");
    jpdf_bench::json::string(&mut s, previous);
    s.push_str(",\"changes\":[");
    for key in keys {
        let (b, a) = (before.get(key), after.get(key));
        let bs = b.map_or("absent", |x| x.0.as_str());
        let as_ = a.map_or("absent", |x| x.0.as_str());
        if bs == as_ {
            continue;
        }
        if is_failure(as_) && !is_failure(bs) {
            new_failures = new_failures.saturating_add(1);
        }
        if is_failure(bs) && matches!(as_, "ok" | "error" | "not_implemented") {
            fixed = fixed.saturating_add(1);
        }
        if changes > 0 {
            s.push(',');
        }
        changes = changes.saturating_add(1);
        let url = a.or(b).map_or("", |x| x.1.as_str());
        s.push_str("{\"blob\":");
        jpdf_bench::json::string(&mut s, &key.0);
        s.push_str(",\"url\":");
        jpdf_bench::json::string(&mut s, url);
        s.push_str(&format!(
            ",\"op\":\"{}\",\"before\":\"{bs}\",\"after\":\"{as_}\"}}",
            key.1.id()
        ));
    }
    s.push_str(&format!(
        "],\"new_failures\":{new_failures},\"fixed\":{fixed}}}\n"
    ));
    let line = format!(
        "nightly: regress diff: {changes} changes, {new_failures} new crash/timeout, {fixed} fixed"
    );
    (s, line)
}

/// Reads a regular file of at most `cap` bytes; `Ok(None)` when nothing is at `path`.
fn read_bounded(path: &Path, cap: u64) -> Result<Option<String>, String> {
    match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(m) if !m.is_file() => return Err("not a regular file".into()),
        Ok(_) => {}
    }
    let f = jpdf_fsio::open_nonblocking(path).map_err(|e| e.to_string())?;
    if !f.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("not a regular file".into());
    }
    let mut buf = Vec::new();
    f.take(cap.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    if u64::try_from(buf.len()).map_or(true, |n| n > cap) {
        return Err(format!("larger than {cap} bytes"));
    }
    String::from_utf8(buf)
        .map(Some)
        .map_err(|_| "not UTF-8".to_string())
}

/// The previous night, loaded and checked before any run (spec item 9), so an invalid file
/// fails at once and a `--previous` that is this night's own `regress.json` is read before
/// it is replaced.
#[derive(Debug, PartialEq, Eq)]
enum Previous {
    /// No diff: why.
    Nothing(String),
    /// The path as given, and its statuses.
    Loaded(String, Statuses),
}

fn load_previous(previous: Option<&Path>) -> Result<Previous, String> {
    let Some(p) = previous else {
        return Ok(Previous::Nothing("no --previous results given".into()));
    };
    let shown = p.display().to_string();
    let bad = |e: String| format!("previous results {shown}: {e}");
    match read_bounded(p, MAX_PREVIOUS).map_err(bad)? {
        None => Ok(Previous::Nothing(format!("no previous results at {shown}"))),
        Some(text) => {
            let before = parse_previous(&text, MAX_ROWS).map_err(bad)?;
            Ok(Previous::Loaded(shown, before))
        }
    }
}

/// The diff file's text and stderr line.
fn diff_for(previous: &Previous, files: &[FileResult]) -> (String, String) {
    match previous {
        Previous::Nothing(note) => {
            let mut s = String::from("{\"schema\":1,\"previous\":null,\"note\":");
            jpdf_bench::json::string(&mut s, note);
            s.push_str("}\n");
            (s, format!("nightly: {note}: no diff"))
        }
        Previous::Loaded(shown, before) => diff_json(shown, before, &current_statuses(files)),
    }
}

/// Runs that never started (spec item 10: a tool failure, not a crash of `jpdf-cli`).
fn unstarted(files: &[FileResult]) -> usize {
    files
        .iter()
        .flat_map(|f| f.runs.iter().flatten())
        .filter(|r| r.status == "crash" && r.exit.is_none() && r.signal.is_none() && r.ms.is_none())
        .count()
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Builds `jpdf-cli` the judge's way; returns its path (spec item 2).
fn build_cli(root: &Path) -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(root)
        .args(["build", "--release", "--locked", "-p", "jpdf-cli"])
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("release build failed ({status})"));
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    Ok(root.join(target).join("release").join("jpdf-cli"))
}

/// The whole nightly (spec items 2–10). `Ok(code)` is the exit code; `Err` is a tool failure
/// before the runs (exit 1).
pub fn run(root: &Path, opts: &Options) -> Result<u8, String> {
    let previous = load_previous(opts.previous.as_deref())?;
    let cli = std::path::absolute(build_cli(root)?).map_err(|e| e.to_string())?;
    if !cli.is_file() {
        return Err(format!("no jpdf-cli at {} after the build", cli.display()));
    }
    let cache = jpdf_corpus::resolve_cache(
        opts.cache.clone(),
        std::env::var_os("JPDF_CORPUS_CACHE").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
        &jpdf_corpus::repo_root(),
    )
    .map_err(|e| e.to_string())?;
    let m = manifest::load(&Tier::Regress.manifest()).map_err(|e| e.to_string())?;
    let entries = fetch::select(m.entries, opts.limit);
    let dir = jpdf_corpus::sub_dir(&cache, Tier::Regress.name()).map_err(|e| e.to_string())?;
    let report = fetch::fetch(&entries, &dir, &fetch::Options::default());
    for line in report.integrity.iter().chain(&report.failed) {
        eprintln!("xtask: nightly: fetch: {line}");
    }
    eprintln!("xtask: nightly: fetch: {}", report.summary());
    let infra = || {
        eprintln!(
            "JUDGE-INFRA-FAILURE: {} regress files could not be fetched",
            report.failed.len()
        );
    };
    let jobs = jobs_for(&entries, &dir);
    if let Some(stop) = nothing_to_run(&jobs, report.exit_code()) {
        if stop == Ok(75) {
            infra();
        }
        return stop;
    }
    let mem = memory_limit_allowed(memory_limit_applies(), cfg!(target_os = "linux"))?;
    engine::start_watchdog()?;
    // Earlier results in DIR must not survive next to this night's (spec item 10).
    for name in ["regress.json", "regress-diff.json"] {
        match std::fs::remove_file(opts.out.join(name)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "cannot remove {}: {e}",
                    opts.out.join(name).display()
                ));
            }
            _ => {}
        }
    }
    let work = opts.out.join("work");
    std::fs::create_dir_all(&work).map_err(|e| format!("cannot write {}: {e}", work.display()))?;
    let png = std::path::absolute(work.join("render.png")).map_err(|e| e.to_string())?;
    let budget = Duration::from_secs(opts.max_minutes.saturating_mul(60));
    let start = Instant::now();
    let (files, complete) = run_files(
        &jobs,
        &png,
        budget,
        &mut || start.elapsed(),
        &mut |op, args| exec(&cli, mem, args, op.limit()),
    );
    let results = results_json(&files, mem, opts.max_minutes, complete);
    write(&opts.out.join("regress.json"), &results)?;
    eprintln!(
        "xtask: nightly: wrote {} ({} files{})",
        opts.out.join("regress.json").display(),
        files.len(),
        if complete {
            ""
        } else {
            ", time budget reached"
        }
    );
    let mut failed = false;
    let (text, line) = diff_for(&previous, &files);
    eprintln!("{line}");
    if let Err(e) = write(&opts.out.join("regress-diff.json"), &text) {
        eprintln!("xtask: nightly: {e}");
        failed = true;
    }
    let n = unstarted(&files);
    if n > 0 {
        eprintln!("xtask: nightly: {n} runs could not start {}", cli.display());
        failed = true;
    }
    let code = exit_code(failed, report.exit_code());
    if code == 75 {
        infra();
    }
    Ok(code)
}

/// The rows as jobs: a row whose cached file fails the blob check is missing (spec item 3).
fn jobs_for(entries: &[manifest::Entry], dir: &Path) -> Vec<Job> {
    entries
        .iter()
        .map(|e| {
            let p = dir.join(format!("{}.pdf", e.blob));
            Job {
                blob: e.blob.clone(),
                url: e.url.clone(),
                path: fetch::is_verified(&p, e).then_some(p),
            }
        })
        .collect()
}

/// When no row has a verified file, the nightly stops before any run (spec item 10): 75 when
/// the fetch was an infrastructure failure, else a tool failure. `None`: something to run.
fn nothing_to_run(jobs: &[Job], fetch_code: u8) -> Option<Result<u8, String>> {
    if jobs.iter().any(|j| j.path.is_some()) {
        return None;
    }
    Some(if fetch_code == 75 {
        Ok(75)
    } else {
        Err(format!(
            "no regress file to run ({} rows missing)",
            jobs.len()
        ))
    })
}

/// Whether runs get the memory wrapper (spec item 6): the probe result, except that on
/// Linux a failed probe is a tool failure.
fn memory_limit_allowed(applies: bool, linux: bool) -> Result<bool, String> {
    if applies {
        return Ok(true);
    }
    if linux {
        return Err(format!(
            "ulimit -v {MEM_KIB} is not enforced here (the judge's Linux limit)"
        ));
    }
    eprintln!("xtask: nightly: the 2 GiB address-space limit is not applied on this OS");
    Ok(false)
}

/// The exit code once the runs are written (spec item 10): a tool failure (`failed`, or the
/// fetch's integrity failure 1) beats the fetch's infrastructure failure 75.
fn exit_code(failed: bool, fetch_code: u8) -> u8 {
    if failed { 1 } else { fetch_code }
}

#[cfg(test)]
mod tests;
