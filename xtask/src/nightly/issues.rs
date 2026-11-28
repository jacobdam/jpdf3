//! The nightly's `issues` step (specs/tooling/nightly.md item 16): one GitHub issue per
//! failure signature, filed with `gh` when `--file-issues` is given, or a comment on the open
//! issue that already carries the signature: failed limits cases, regress crashes and
//! timeouts, and regress regressions (#302), then fuzz crashes and a bench timeout (#383).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use jpdf_testgen::json::{self, Value};

use super::extra::{BenchResult, FuzzResult};
use super::{FileResult, Previous, current_statuses, is_blob};
use crate::limits::CaseResult;
use crate::steps::Outcome as Step;

/// Every issue body and comment starts with it (CONSTITUTION 1).
pub const TAG: &str = "🤖 jpdf-agent";
/// The line that names a finding's signature in the body of the issue filed for it.
pub const SIGNATURE: &str = "nightly-signature: ";
/// Most issues created plus comments posted in one night (spec item 16).
pub const MAX_WRITES: usize = 20;
/// `gh issue list --limit`: a list that returns this many may be cut, so it is refused.
pub const LIST_LIMIT: usize = 1000;
/// Each `gh` call's time limit, and the most stdout read from it.
const GH_LIMIT: Duration = Duration::from_secs(120);
pub const GH_STDOUT_CAP: u64 = 64 * 1024 * 1024;

/// One failure to file (spec item 16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// `limits:<case>`, `regress:<blob>` or `regression:<blob>:<op>`: only ASCII lowercase
    /// letters, digits, `_`, `:` and `-`, all from xtask's own names and checked blob ids.
    pub sig: String,
    /// `crash` or `bug`.
    pub label: &'static str,
    pub title: String,
    /// Markdown list lines (no `jpdf-cli` output: stderr is never copied, spec item 16).
    pub detail: String,
}

fn num<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or("none".into(), |v| v.to_string())
}

/// The manifest's URL in backticks, or a note when it holds a character that could break
/// out of them (it is shown, never fetched).
fn shown_url(url: &str) -> String {
    if !url.is_empty() && url.bytes().all(|b| b.is_ascii_graphic() && b != b'`') {
        format!("`{url}`")
    } else {
        "(URL not shown: unexpected characters)".into()
    }
}

/// Every finding of the night: failed limits cases in case order, then regress files with a
/// crash or timeout in run order (blob-id order), then regressions by blob and op (spec item 16).
pub fn findings(
    limits: &[CaseResult],
    previous: Option<&Previous>,
    files: Option<&[FileResult]>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in limits.iter().filter(|r| r.status != "pass") {
        let label = if matches!(r.status, "crash" | "timeout") {
            "crash"
        } else {
            "bug"
        };
        out.push(Finding {
            sig: format!("limits:{}", r.id),
            label,
            title: format!("nightly: limits case {} ended {}", r.id, r.status),
            detail: format!(
                "- Limits case `{}` (specs/tooling/limits.md), a generated file of {} bytes: \
                 `{}`, exit {}, signal {}, {} ms\n- Reproduce: `cargo xtask limits`\n",
                r.id,
                r.bytes,
                r.status,
                num(r.exit),
                num(r.signal),
                num(r.ms)
            ),
        });
    }
    let Some(files) = files else { return out };
    for f in files.iter().filter(|f| is_blob(&f.blob)) {
        let bad: Vec<String> = f
            .runs
            .iter()
            .flatten()
            .filter(|r| matches!(r.status, "crash" | "timeout"))
            .map(|r| {
                format!(
                    "  - `{}`: `{}`, exit {}, signal {}, {} ms",
                    r.op.id(),
                    r.status,
                    num(r.exit),
                    num(r.signal),
                    num(r.ms)
                )
            })
            .collect();
        if bad.is_empty() {
            continue;
        }
        out.push(Finding {
            sig: format!("regress:{}", f.blob),
            label: "crash",
            title: format!(
                "nightly: jpdf-cli crash or timeout on regress file {}",
                f.blob.get(..12).unwrap_or(&f.blob)
            ),
            detail: format!(
                "- Regress file `{}` ({})\n- Runs (specs/tooling/nightly.md item 4):\n{}\n",
                f.blob,
                shown_url(&f.url),
                bad.join("\n")
            ),
        });
    }
    let Some(Previous::Loaded(_, before)) = previous else {
        return out;
    };
    for ((blob, op), (after, url)) in current_statuses(files) {
        let was_ok = before.get(&(blob.clone(), op)).is_some_and(|b| b.0 == "ok");
        if !was_ok || !matches!(after.as_str(), "error" | "not_implemented") || !is_blob(&blob) {
            continue;
        }
        out.push(Finding {
            sig: format!("regression:{blob}:{}", op.id()),
            label: "bug",
            title: format!(
                "nightly: regress regression: {} on {} went from ok to {after}",
                op.id(),
                blob.get(..12).unwrap_or(&blob)
            ),
            detail: format!(
                "- Regress file `{blob}` ({}): `{}` was `ok` the night before, now `{after}`\n",
                shown_url(&url),
                op.id()
            ),
        });
    }
    out
}

/// Most bytes in a fuzz target or artifact name that goes into a signature.
pub const MAX_NAME: usize = 100;

/// A name a signature may hold (spec item 16): 1 to [`MAX_NAME`] bytes of ASCII lowercase
/// letters, digits, `_` and `-`.
pub fn safe_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= MAX_NAME
        && n.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// The fuzz pass's and the bench's findings, after [`findings`] (spec item 16): for each fuzz
/// target that failed, in run order, one `crash` finding per copied artifact whose name is in
/// the set of [`safe_name`], by name order, and one for the target itself when it has no such
/// artifact or any other; then a `perf` finding when the bench ended `timeout`. A target whose
/// own name is outside the set is never filed; a target that ran clean is not filed for its
/// artifacts.
pub fn extra_findings(bench: Option<&BenchResult>, fuzz: Option<&FuzzResult>) -> Vec<Finding> {
    let mut out = Vec::new();
    let targets = fuzz.map_or(&[][..], |f| f.targets.as_slice());
    let seconds = fuzz.map_or(0, |f| f.seconds);
    for t in targets.iter().filter(|t| !t.ok && safe_name(&t.name)) {
        let prefix = format!("fuzz/{}/", t.name);
        let (named, other): (Vec<&str>, Vec<&str>) = t
            .artifacts
            .iter()
            .map(|a| a.strip_prefix(&prefix).unwrap_or(""))
            .partition(|n| safe_name(n));
        for n in &named {
            out.push(Finding {
                sig: format!("fuzz:{}:{n}", t.name),
                label: "crash",
                title: format!("nightly: fuzz target {} crashed: {n}", t.name),
                detail: format!(
                    "- Fuzz target `{}`, {seconds} s (specs/tooling/nightly.md item 13): \
                     artifact `{prefix}{n}` in the nightly's output directory\n\
                     - Reproduce: `cargo fuzz run {} fuzz/artifacts/{}/{n}`\n",
                    t.name, t.name, t.name
                ),
            });
        }
        if named.is_empty() || !other.is_empty() {
            out.push(Finding {
                sig: format!("fuzz:{}", t.name),
                label: "crash",
                title: format!("nightly: fuzz target {} crashed", t.name),
                detail: format!(
                    "- Fuzz target `{}`, {seconds} s (specs/tooling/nightly.md item 13): \
                     crashed, hung or ran out of memory; {} artifacts copied, {} of them \
                     not named here (no artifact, or a name outside the allowed set)\n\
                     - Reproduce: `cargo xtask fuzz --target {} --seconds {seconds}`\n",
                    t.name,
                    t.artifacts.len(),
                    other.len(),
                    t.name
                ),
            });
        }
    }
    if let Some(b) = bench.filter(|b| b.status == "timeout") {
        out.push(Finding {
            sig: "bench:timeout".into(),
            label: "perf",
            title: "nightly: the bench ran out of time".into(),
            detail: format!(
                "- Bench on {} regress files (specs/tooling/nightly.md item 12): `timeout` at \
                 the `--bench-minutes` limit, exit {}, signal {}, {} ms\n\
                 - Reproduce: `cargo xtask bench`\n",
                b.files,
                num(b.exit),
                num(b.signal),
                num(b.ms)
            ),
        });
    }
    out
}

/// The night's findings in filing order (spec item 16): [`findings`], then
/// [`extra_findings`].
pub fn night_findings(
    limits: &[CaseResult],
    previous: Option<&Previous>,
    files: Option<&[FileResult]>,
    bench: Option<&BenchResult>,
    fuzz: Option<&FuzzResult>,
) -> Vec<Finding> {
    let mut out = findings(limits, previous, files);
    out.extend(extra_findings(bench, fuzz));
    out
}

/// The body of a new issue.
pub fn issue_body(f: &Finding, night: &str) -> String {
    format!(
        "{TAG} **Nightly finding**, filed by `cargo xtask nightly --file-issues` \
         (specs/tooling/nightly.md item 16).\n\n{SIGNATURE}{}\n\nFirst seen: {night}.\n\n{}",
        f.sig, f.detail
    )
}

/// The comment on an open issue that already has the signature.
pub fn comment_body(f: &Finding, night: &str) -> String {
    format!("{TAG} Seen again: {night}.\n\n{}", f.detail)
}

/// Runs `gh` with these arguments; `Ok` is its stdout after exit 0.
pub trait Gh {
    fn run(&mut self, args: &[String]) -> Result<String, String>;
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_string()).collect()
}

/// Signature → issue number over `gh issue list`'s JSON: only bodies that start with the tag,
/// only whole `nightly-signature:` lines; the lowest number wins.
fn index(
    text: &str,
    label: &str,
    cap: usize,
    map: &mut BTreeMap<String, u64>,
) -> Result<(), String> {
    let bad = |e: &str| format!("gh issue list --label {label}: {e}");
    let v = json::parse(text).map_err(|e| bad(&e))?;
    let items = v.as_array().ok_or_else(|| bad("not a JSON array"))?;
    if items.len() >= cap {
        return Err(bad(&format!(
            "{} open issues, at least the list limit: cannot deduplicate",
            items.len()
        )));
    }
    for it in items {
        let number = match it.get("number") {
            Some(Value::Number(n)) => n.parse::<u64>().ok(),
            _ => None,
        }
        .ok_or_else(|| bad("an issue without a number"))?;
        let body = it
            .get("body")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("an issue without a body"))?;
        if !body.starts_with(&format!("{TAG} ")) {
            continue;
        }
        for sig in body.lines().filter_map(|l| l.strip_prefix(SIGNATURE)) {
            let e = map.entry(sig.to_string()).or_insert(number);
            *e = (*e).min(number);
        }
    }
    Ok(())
}

/// What one night filed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Filed {
    pub created: usize,
    pub commented: usize,
    /// Findings left for a later night by the write cap.
    pub skipped: usize,
}

/// The labels listed for deduplication on every filing night (a limits case changes label
/// with its status, so its open issue may carry the other one).
pub const LABELS: [&str; 2] = ["bug", "crash"];
/// Listed too when a finding carries it (only `bench:timeout` does, and always with it).
pub const PERF: &str = "perf";

/// Files `found` (spec item 16): no `gh` call when there is nothing to file; else one list
/// per label of [`LABELS`] (and [`PERF`] when a finding has it), then at most `max_writes`
/// writes: the creates first (in finding
/// order), then the comments, so a new finding is never starved by known ones. `Err` at the
/// first failed call, with what was filed before it.
pub fn file(
    gh: &mut dyn Gh,
    found: &[Finding],
    night: &str,
    max_writes: usize,
    list_cap: usize,
) -> Result<Filed, (String, Filed)> {
    let mut done = Filed::default();
    if found.is_empty() {
        return Ok(done);
    }
    let mut map = BTreeMap::new();
    let perf = found.iter().any(|f| f.label == PERF).then_some(PERF);
    for label in LABELS.into_iter().chain(perf) {
        let limit = list_cap.to_string();
        let args = s(&[
            "issue",
            "list",
            "--state",
            "open",
            "--label",
            label,
            "--limit",
            &limit,
            "--json",
            "number,body",
        ]);
        let text = gh.run(&args).map_err(|e| (e, Filed::default()))?;
        index(&text, label, list_cap, &mut map).map_err(|e| (e, Filed::default()))?;
    }
    let (new, known): (Vec<&Finding>, Vec<&Finding>) =
        found.iter().partition(|f| !map.contains_key(&f.sig));
    for f in new.into_iter().chain(known) {
        if done.created.saturating_add(done.commented) >= max_writes {
            done.skipped = done.skipped.saturating_add(1);
            continue;
        }
        let r = match map.get(&f.sig) {
            Some(n) => gh
                .run(&s(&[
                    "issue",
                    "comment",
                    &n.to_string(),
                    "--body",
                    &comment_body(f, night),
                ]))
                .map(|_| done.commented = done.commented.saturating_add(1)),
            None => gh
                .run(&s(&[
                    "issue",
                    "create",
                    "--title",
                    &f.title,
                    "--label",
                    f.label,
                    "--body",
                    &issue_body(f, night),
                ]))
                .map(|_| done.created = done.created.saturating_add(1)),
        };
        if let Err(e) = r {
            return Err((e, done));
        }
    }
    Ok(done)
}

/// The step's outcome (spec item 16): without `--file-issues` nothing is called; any `gh`
/// failure is a tool failure.
pub fn step(enabled: bool, gh: &mut dyn Gh, found: &[Finding], night: &str) -> Step {
    if !enabled {
        eprintln!(
            "xtask: nightly: issues: {} findings, not filed (no --file-issues)",
            found.len()
        );
        return Step::Ok;
    }
    match file(gh, found, night, MAX_WRITES, LIST_LIMIT) {
        Ok(d) => {
            eprintln!(
                "xtask: nightly: issues: {} findings: {} filed, {} updated, {} over the cap of {MAX_WRITES}",
                found.len(),
                d.created,
                d.commented,
                d.skipped
            );
            Step::Ok
        }
        Err((e, d)) => Step::Failed(format!(
            "issues: {e} ({} filed, {} updated before it)",
            d.created, d.commented
        )),
    }
}

/// `YYYY-MM-DD UTC`, plus the CI run id when `GITHUB_RUN_ID` is all digits.
pub fn night_label(now_secs: u64, run_id: Option<&str>) -> String {
    let day = i64::try_from(now_secs / 86_400).unwrap_or(i64::MAX / 2);
    let mut s = format!("{} UTC", crate::metrics::fmt_day(day));
    if let Some(id) = run_id
        .filter(|id| !id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit()))
    {
        s.push_str(&format!(" (CI run {id})"));
    }
    s
}

/// The real `gh`: no prompts, no pager, stdin on the null device, stderr to ours, each call
/// killed at `limit`, stdout read up to `stdout_cap` bytes (`GH_STDOUT_CAP`; more is an error).
pub struct RealGh {
    pub program: PathBuf,
    pub limit: Duration,
    pub stdout_cap: u64,
}

impl RealGh {
    pub fn new() -> Self {
        RealGh {
            program: PathBuf::from("gh"),
            limit: GH_LIMIT,
            stdout_cap: GH_STDOUT_CAP,
        }
    }
}

impl Gh for RealGh {
    fn run(&mut self, args: &[String]) -> Result<String, String> {
        let what = format!(
            "gh {}",
            args.iter().take(2).cloned().collect::<Vec<_>>().join(" ")
        );
        let mut child = Command::new(&self.program)
            .args(args.iter().map(OsString::from))
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .env("GH_PAGER", "")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", self.program.display()))?;
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{what}: no stdout pipe"));
        };
        let cap = self.stdout_cap;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let r = stdout.take(cap.saturating_add(1)).read_to_end(&mut buf);
            let _ = tx.send(r.map(|_| buf));
        });
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break st,
                Ok(None) if start.elapsed() >= self.limit => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{what}: no answer in {} s", self.limit.as_secs()));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("{what}: {e}"));
                }
            }
        };
        if !status.success() {
            return Err(format!("{what}: {status}"));
        }
        let out = rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| format!("{what}: stdout still open after exit"))?
            .map_err(|e| format!("{what}: {e}"))?;
        if u64::try_from(out.len()).map_or(true, |n| n > cap) {
            return Err(format!("{what}: more than {cap} bytes of output"));
        }
        String::from_utf8(out).map_err(|_| format!("{what}: output is not UTF-8"))
    }
}

#[cfg(test)]
mod tests;
