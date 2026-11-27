//! `cargo xtask admit <crate>@<version> --work DIR`: a local pre-check of the dependency
//! admission rules (CONSTITUTION 8 and 10, PLAN §3, AUTOMATION.md §6.4) before a crate is
//! added. Spec: specs/tooling/admit.md. Part 1 (#303); the `cargo vet` and build-time steps
//! are #413.
//!
//! It resolves the candidate in a probe package of its own, outside any git checkout, and runs
//! the judge's own checks on it: `cargo deny check` with the repo's `deny.toml`, and the
//! judge's dependency scan (`check_deps` in `ops/judge/policy.py`, imported, never copied). It
//! only reports: it never records audits or exemptions, never writes inside the repo and never
//! builds the probe (no build script or proc-macro of the candidate runs). Tool output is data:
//! every line printed from it goes through [`clean`].

use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use jpdf_testgen::json::{self, Value};

use crate::steps::{self, Log, Outcome};

/// Longest crate name crates.io accepts.
pub const MAX_NAME: usize = 64;
/// Longest version text accepted (`MAJOR.MINOR.PATCH-pre+build`).
pub const MAX_VERSION: usize = 64;
/// Detail lines printed per step, and bytes per line, at most.
pub const MAX_DETAILS: usize = 40;
pub const MAX_LINE: usize = 300;
/// The probe package's name; it is never published.
pub const PROBE: &str = "jpdf-admit-probe";
/// Copied into the probe so rustup picks the pinned toolchain there.
const TOOLCHAIN: &str = "rust-toolchain.toml";
/// Runs the judge's `check_deps` on the probe and prints its violations as one JSON list.
pub const UNSAFE_SNIPPET: &str = "import json, sys\nfrom pathlib import Path\n\
sys.path.insert(0, sys.argv[1])\nimport policy\nr = policy.Report()\n\
policy.check_deps(Path(sys.argv[2]), r)\nprint(json.dumps(r.v))\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub name: String,
    pub version: String,
    pub work: PathBuf,
    pub python: String,
}

/// `<crate>@<version> --work DIR [--python PATH]`, each at most once.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let (mut target, mut work, mut python) = (None, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |flag: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match a.as_str() {
            "--work" if work.is_none() => work = Some(PathBuf::from(value("--work")?)),
            "--python" if python.is_none() => python = Some(value("--python")?),
            s if !s.starts_with('-') && target.is_none() => target = Some(s.to_owned()),
            s => return Err(format!("unexpected or repeated argument {s:?}")),
        }
    }
    let target = target.ok_or("missing <crate>@<version>")?;
    let (name, version) = target
        .split_once('@')
        .ok_or_else(|| format!("{target:?} is not <crate>@<version>"))?;
    check_name(name)?;
    check_version(version)?;
    let python = python.unwrap_or_else(|| "python3".to_owned());
    if python.is_empty() {
        return Err("--python is empty".to_owned());
    }
    let work = work.ok_or("missing --work DIR")?;
    Ok(Options {
        name: name.to_owned(),
        version: version.to_owned(),
        work,
        python,
    })
}

/// A crates.io name: an ASCII letter, then ASCII letters, digits, `-` or `_`.
fn check_name(name: &str) -> Result<(), String> {
    let ok = name.len() <= MAX_NAME
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then_some(()).ok_or_else(|| {
        format!("crate name {name:?} is not 1-{MAX_NAME} ASCII letters, digits, - or _ starting with a letter")
    })
}

/// An exact semver version (PLAN §3 rule 5, pinned): three decimal parts without leading
/// zeros, then an optional `-pre` and `+build`, each a non-empty run of ASCII letters, digits,
/// `.` and `-`.
fn check_version(v: &str) -> Result<(), String> {
    let (rest, build) = v.split_once('+').map_or((v, None), |(r, b)| (r, Some(b)));
    let (core, pre) = rest
        .split_once('-')
        .map_or((rest, None), |(c, p)| (c, Some(p)));
    let ident = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    };
    let number = |p: &str| {
        !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) && (p == "0" || !p.starts_with('0'))
    };
    let parts: Vec<&str> = core.split('.').collect();
    let ok = v.len() <= MAX_VERSION
        && parts.len() == 3
        && parts.iter().all(|p| number(p))
        && pre.is_none_or(ident)
        && build.is_none_or(ident);
    ok.then_some(()).ok_or_else(|| {
        format!("version {v:?} is not an exact MAJOR.MINOR.PATCH[-pre][+build] of at most {MAX_VERSION} bytes")
    })
}

/// The work directory: absolute, not existing yet, its parent an existing directory, and no
/// ancestor (after resolving symlinks) a git checkout, so neither the repo's workspace, its
/// `.cargo/` nor the judge's tree walk sees the probe. Returns the resolved path.
pub fn check_work(work: &Path, is_checkout: &dyn Fn(&Path) -> bool) -> Result<PathBuf, String> {
    let show = work.display();
    if !work.is_absolute() {
        return Err(format!("--work {show} is not an absolute path"));
    }
    let Some(Component::Normal(leaf)) = work.components().next_back() else {
        return Err(format!("--work {show} has no final name"));
    };
    if fs::symlink_metadata(work).is_ok() {
        return Err(format!("--work {show} exists; give a new directory"));
    }
    let parent = work
        .parent()
        .ok_or_else(|| format!("--work {show} has no parent"))?;
    let parent =
        fs::canonicalize(parent).map_err(|e| format!("--work parent {}: {e}", parent.display()))?;
    if !parent.is_dir() {
        return Err(format!(
            "--work parent {} is not a directory",
            parent.display()
        ));
    }
    if let Some(repo) = parent.ancestors().find(|a| is_checkout(a)) {
        return Err(format!(
            "--work {show} is inside the git checkout {}",
            repo.display()
        ));
    }
    Ok(parent.join(leaf))
}

/// The live checkout test: the directory holds a `.git` entry (a worktree's is a file).
pub fn has_git(dir: &Path) -> bool {
    fs::symlink_metadata(dir.join(".git")).is_ok()
}

/// The probe's `Cargo.toml`: one dependency, pinned with `=`, and an empty `[workspace]` so no
/// parent manifest claims it.
pub fn manifest(o: &Options) -> String {
    format!(
        "[package]\nname = \"{PROBE}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\n{} = \"={}\"\n\n[workspace]\n",
        o.name, o.version
    )
}

/// One command a step runs, in the probe directory (tests pin these).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
}

impl Cmd {
    /// A command from its program and space-separated literal arguments.
    fn new(program: &str, words: &str) -> Cmd {
        Cmd {
            program: program.to_owned(),
            args: Vec::new(),
        }
        .words(words)
    }

    fn words(mut self, words: &str) -> Cmd {
        self.args.extend(words.split(' ').map(str::to_owned));
        self
    }

    /// One argument, kept whole (a path, the snippet).
    fn arg(mut self, a: &str) -> Cmd {
        self.args.push(a.to_owned());
        self
    }

    fn path(self, p: &Path) -> Cmd {
        self.arg(&p.to_string_lossy())
    }
}

pub fn lock_cmd() -> Cmd {
    Cmd::new("cargo", "generate-lockfile --color never")
}

/// `cargo deny check` as the judge runs it, with the repo's `deny.toml`.
pub fn deny_cmd(probe: &Path, repo: &Path) -> Cmd {
    Cmd::new("cargo", "deny --color never --manifest-path")
        .path(&probe.join("Cargo.toml"))
        .words("--config")
        .path(&repo.join("deny.toml"))
        .words("check")
}

/// The judge's scan; `-I` keeps the working directory and `PYTHON*` variables out of the
/// import path.
pub fn unsafe_cmd(python: &str, probe: &Path, repo: &Path) -> Cmd {
    Cmd::new(python, "-I -c")
        .arg(UNSAFE_SNIPPET)
        .path(&repo.join("ops/judge"))
        .path(probe)
}

/// What a command printed and how it ended (`None`: killed by a signal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Captured {
    fn ok(&self) -> bool {
        self.code == Some(0)
    }

    /// The exit code and one stderr line: the first `error:` or `error[` line (cargo's own
    /// message), else the last non-blank one (a Python traceback's exception).
    fn tail(&self) -> String {
        let first_error = self
            .stderr
            .lines()
            .find(|l| l.starts_with("error:") || l.starts_with("error["));
        let last = self.stderr.lines().rev().find(|l| !l.trim().is_empty());
        let code = self
            .code
            .map_or_else(|| "signal".to_owned(), |c| c.to_string());
        format!(
            "exit {code}: {}",
            clean(first_error.or(last).unwrap_or("no output"))
        )
    }
}

/// A line of tool output as printed: control characters become `?` and it is cut to
/// [`MAX_LINE`] bytes, so crate data can't forge or hide report lines.
pub fn clean(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().map(|c| if hides(c) { '?' } else { c }) {
        if out.len().saturating_add(c.len_utf8()) > MAX_LINE {
            out.push('…');
            break;
        }
        out.push(c);
    }
    out
}

/// A character that can break or reorder a line where the report is shown: a control
/// character, U+2028/U+2029 (line and paragraph separators) or a bidi embedding, override or
/// isolate (U+202A-U+202E, U+2066-U+2069).
fn hides(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// A step's result plus the lines printed under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub outcome: Outcome,
    pub details: Vec<String>,
}

fn checked(outcome: Outcome, details: Vec<String>) -> Checked {
    Checked { outcome, details }
}

fn failure(why: String) -> Checked {
    checked(Outcome::Failed(why), Vec::new())
}

/// `cargo deny check`: exit 0 with an all-`ok` summary line on stdout passes; a non-zero exit
/// whose summary line has a `FAILED` check is a rejection naming the failed checks and each
/// `error[...]` diagnostic with its location line; anything else is a failure.
pub fn deny_result(c: &Captured) -> Checked {
    match (c.ok(), c.stdout.lines().find_map(deny_summary)) {
        (true, Some(failed)) if failed.is_empty() => checked(Outcome::Ok, Vec::new()),
        (false, Some(failed)) if !failed.is_empty() => {
            let mut details = vec![format!("failed: {}", failed.join(", "))];
            let mut lines = c.stderr.lines().peekable();
            while let Some(l) = lines.next() {
                if l.starts_with("error[") {
                    details.push(clean(l));
                    if let Some(at) = lines
                        .peek()
                        .map(|n| n.trim_start())
                        .filter(|n| n.starts_with("┌─"))
                    {
                        details.push(format!("  {}", clean(at)));
                    }
                }
            }
            checked(Outcome::Finding, details)
        }
        _ => failure(format!("cargo deny: {}", c.tail())),
    }
}

/// The failed checks of a deny summary line (`advisories ok, bans ok, licenses FAILED,
/// sources ok`), or `None` if the line isn't exactly those four pairs in that order.
fn deny_summary(line: &str) -> Option<Vec<String>> {
    const CHECKS: [&str; 4] = ["advisories", "bans", "licenses", "sources"];
    let parts: Vec<&str> = line.trim_end().split(", ").collect();
    if parts.len() != CHECKS.len() {
        return None;
    }
    let mut failed = Vec::new();
    for (part, check) in parts.iter().zip(CHECKS) {
        match part.strip_prefix(check).and_then(|r| r.strip_prefix(' ')) {
            Some("ok") => {}
            Some("FAILED") => failed.push(check.to_owned()),
            _ => return None,
        }
    }
    Some(failed)
}

/// The judge's `check_deps`: its JSON list on stdout's last line. Empty is a pass; violations
/// are a rejection, except `cargo metadata failed`, which is the judge's own failure to run.
pub fn unsafe_result(c: &Captured) -> Checked {
    if !c.ok() {
        let tail = c.tail();
        let hint = if tail.contains("tomllib") {
            " (needs Python 3.11+ for tomllib; see --python)"
        } else {
            ""
        };
        return failure(format!("judge dependency scan{hint}: {tail}"));
    }
    let last = c
        .stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let Ok(Value::Array(items)) = json::parse(last) else {
        return failure("judge dependency scan: last line is not a JSON list".to_owned());
    };
    let mut lines = Vec::new();
    for item in &items {
        match item.as_str() {
            Some(s) if s.starts_with("cargo metadata failed") => {
                return failure(format!("judge dependency scan: {}", clean(s)));
            }
            Some(s) => lines.push(clean(s)),
            None => {
                return failure("judge dependency scan: a violation is not a string".to_owned());
            }
        }
    }
    checked(
        if lines.is_empty() {
            Outcome::Ok
        } else {
            Outcome::Finding
        },
        lines,
    )
}

/// Runs one command; `Err` when it couldn't start.
pub type Runner<'a> = dyn FnMut(&Cmd) -> Result<Captured, String> + 'a;

/// The report: each step's outcome and details, in order.
pub type Report = Vec<(&'static str, Checked)>;

/// The step order (specs/tooling/admit.md, docs/PRINCIPLES.md P4): `probe` writes and resolves
/// the probe, which every later step needs, so its failure stops the tool; `deny` and `unsafe`
/// both run whatever the other found.
pub fn drive(
    o: &Options,
    probe: &Path,
    repo: &Path,
    write: &mut dyn FnMut() -> Result<(), String>,
    run: &mut Runner<'_>,
) -> (Log, Report) {
    let mut log = Log::printing("xtask: admit: ");
    let mut report = Report::new();
    let mut record = |step, c: Checked| {
        log.push(step, c.outcome.clone());
        report.push((step, c));
    };
    let resolved = write().and_then(|()| match run(&lock_cmd())? {
        lock if lock.ok() => Ok(()),
        lock => Err(format!("cargo generate-lockfile: {}", lock.tail())),
    });
    if let Err(why) = resolved {
        record("probe", failure(why));
        return (log, report);
    }
    record("probe", checked(Outcome::Ok, Vec::new()));
    record(
        "deny",
        run(&deny_cmd(probe, repo)).map_or_else(failure, |c| deny_result(&c)),
    );
    let scan = run(&unsafe_cmd(&o.python, probe, repo)).map_or_else(failure, |c| unsafe_result(&c));
    record("unsafe", scan);
    (log, report)
}

/// The report text and the verdict line.
pub fn render(o: &Options, report: &Report, code: u8) -> String {
    let mut out = String::new();
    for (step, c) in report {
        let state = match &c.outcome {
            Outcome::Ok => "ok".to_owned(),
            Outcome::Finding => "rejected".to_owned(),
            Outcome::Infra(why) | Outcome::Failed(why) => format!("failed: {why}"),
        };
        out.push_str(&format!("admit: {step}: {state}\n"));
        for d in c.details.iter().take(MAX_DETAILS) {
            out.push_str(&format!("  {d}\n"));
        }
        if c.details.len() > MAX_DETAILS {
            out.push_str(&format!(
                "  … {} more\n",
                c.details.len().saturating_sub(MAX_DETAILS)
            ));
        }
    }
    let rejected: Vec<&str> = report
        .iter()
        .filter(|(_, c)| c.outcome == Outcome::Finding)
        .map(|(s, _)| *s)
        .collect();
    let verdict = match code {
        0 => "admitted by the license and unsafe checks (cargo vet, build-time code and features: #413)".to_owned(),
        4 => format!("rejected by {}", rejected.join(", ")),
        _ => "not decided: a step failed".to_owned(),
    };
    out.push_str(&format!("admit: {}@{}: {verdict}\n", o.name, o.version));
    out
}

/// The live run from the repo root `repo`: exit 0 admitted, 4 rejected, 1 a step failed.
pub fn run(repo: &Path, o: &Options) -> Result<u8, String> {
    let repo = fs::canonicalize(repo).map_err(|e| format!("repo root: {e}"))?;
    let probe = check_work(&o.work, &has_git)?;
    let mut write = || write_probe(&probe, &repo, o);
    let mut runner = |c: &Cmd| live(c, &probe);
    let (log, report) = drive(o, &probe, &repo, &mut write, &mut runner);
    let code = steps::exit_code(&log);
    print!("{}", render(o, &report, code));
    Ok(code)
}

/// Creates the probe directory (it must not exist) and its files, each created new.
fn write_probe(probe: &Path, repo: &Path, o: &Options) -> Result<(), String> {
    let new = |p: &Path, bytes: &[u8]| -> Result<(), String> {
        let mut options = fs::OpenOptions::new();
        let file = options.write(true).create_new(true).open(p);
        file.and_then(|mut f| f.write_all(bytes))
            .map_err(|e| format!("write {}: {e}", p.display()))
    };
    for dir in [probe.to_path_buf(), probe.join("src")] {
        fs::create_dir(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    new(&probe.join("Cargo.toml"), manifest(o).as_bytes())?;
    new(&probe.join("src/lib.rs"), b"")?;
    let toolchain = fs::read(repo.join(TOOLCHAIN)).map_err(|e| format!("read {TOOLCHAIN}: {e}"))?;
    new(&probe.join(TOOLCHAIN), &toolchain)
}

fn live(c: &Cmd, probe: &Path) -> Result<Captured, String> {
    let out = Command::new(&c.program)
        .args(&c.args)
        .current_dir(probe)
        .output()
        .map_err(|e| format!("cannot run {}: {e}", c.program))?;
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    Ok(Captured {
        code: out.status.code(),
        stdout: text(&out.stdout),
        stderr: text(&out.stderr),
    })
}

#[cfg(test)]
mod tests;
