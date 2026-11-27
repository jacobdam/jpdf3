//! `cargo xtask admit <crate>@<version> --work DIR`: a local pre-check of the dependency
//! admission rules (CONSTITUTION 8-10, PLAN §3, AUTOMATION.md §6.4) before a crate is added.
//! Spec: specs/tooling/admit.md.
//!
//! It resolves the candidate in a probe package of its own, outside any git checkout, and runs
//! the judge's own checks on it: `cargo deny check` with the repo's `deny.toml`, the judge's
//! dependency scan (`check_deps` in `ops/judge/policy.py`, imported, never copied), and
//! `cargo vet` with the repo's imports. It only reports: it never records audits or
//! exemptions, never writes inside the repo and never builds the probe (no build script or
//! proc-macro of the candidate runs). Tool output and crate metadata are data: every line
//! printed from them goes through [`clean`].

use std::collections::BTreeSet;
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
/// Most `--features` entries, and the longest one.
pub const MAX_FEATURES: usize = 32;
pub const MAX_FEATURE: usize = 64;
/// Detail lines printed per step, and bytes per line, at most.
pub const MAX_DETAILS: usize = 40;
pub const MAX_LINE: usize = 300;
/// The probe package's name; it is never published.
pub const PROBE: &str = "jpdf-admit-probe";
/// Repository files copied into the probe (cargo vet's store and the pinned toolchain).
const COPIED: [&str; 4] = [
    "supply-chain/config.toml",
    "supply-chain/audits.toml",
    "supply-chain/imports.lock",
    "rust-toolchain.toml",
];
/// Runs the judge's `check_deps` on the probe and prints its violations as one JSON list.
/// `-I` keeps the working directory and `PYTHON*` variables out of the import path.
pub const UNSAFE_SNIPPET: &str = "import json, sys\nfrom pathlib import Path\n\
sys.path.insert(0, sys.argv[1])\nimport policy\nr = policy.Report()\n\
policy.check_deps(Path(sys.argv[2]), r)\nprint(json.dumps(r.v))\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub name: String,
    pub version: String,
    pub work: PathBuf,
    pub features: Vec<String>,
    pub no_default_features: bool,
    pub python: String,
}

/// `<crate>@<version> --work DIR [--features A,B] [--no-default-features] [--python PATH]`.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut target = None;
    let mut work = None;
    let mut features = None;
    let mut no_default_features = false;
    let mut python = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |flag: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match a.as_str() {
            "--work" if work.is_none() => work = Some(PathBuf::from(value("--work")?)),
            "--features" if features.is_none() => features = Some(value("--features")?),
            "--python" if python.is_none() => python = Some(value("--python")?),
            "--no-default-features" if !no_default_features => no_default_features = true,
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
    let features = match features {
        None => Vec::new(),
        Some(list) => list
            .split(',')
            .map(|f| check_feature(f).map(|()| f.to_owned()))
            .collect::<Result<Vec<_>, _>>()?,
    };
    if features.len() > MAX_FEATURES {
        return Err(format!("more than {MAX_FEATURES} features"));
    }
    let python = python.unwrap_or_else(|| "python3".to_owned());
    if python.is_empty() {
        return Err("--python is empty".to_owned());
    }
    Ok(Options {
        name: name.to_owned(),
        version: version.to_owned(),
        work: work.ok_or("missing --work DIR")?,
        features,
        no_default_features,
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
    if ok {
        Ok(())
    } else {
        Err(format!(
            "crate name {name:?} is not 1-{MAX_NAME} ASCII letters, digits, - or _ starting with a letter"
        ))
    }
}

/// An exact semver version (rule 5, pinned): three decimal parts without leading zeros, then an
/// optional `-pre` and `+build`, each a non-empty run of ASCII letters, digits, `.` and `-`.
fn check_version(v: &str) -> Result<(), String> {
    let (rest, build) = match v.split_once('+') {
        Some((r, b)) => (r, Some(b)),
        None => (v, None),
    };
    let (core, pre) = match rest.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (rest, None),
    };
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
    if ok {
        Ok(())
    } else {
        Err(format!(
            "version {v:?} is not an exact MAJOR.MINOR.PATCH[-pre][+build] of at most {MAX_VERSION} bytes"
        ))
    }
}

/// A feature name: 1-64 ASCII letters, digits, `_`, `-`, `+` or `.` (so it can't leave its
/// TOML string).
fn check_feature(f: &str) -> Result<(), String> {
    let ok = !f.is_empty()
        && f.len() <= MAX_FEATURE
        && f.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '.'));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "feature {f:?} is not 1-{MAX_FEATURE} ASCII letters, digits, _, -, + or ."
        ))
    }
}

/// The work directory: absolute, not existing yet, its parent an existing directory, and no
/// ancestor (after resolving symlinks) a git checkout, so neither the repo's workspace, its
/// `.cargo/` nor the judge's tree walk sees the probe. Returns the resolved path.
pub fn check_work(work: &Path, is_checkout: &dyn Fn(&Path) -> bool) -> Result<PathBuf, String> {
    if !work.is_absolute() {
        return Err(format!("--work {} is not an absolute path", work.display()));
    }
    let leaf = match work.components().next_back() {
        Some(Component::Normal(leaf)) => leaf,
        _ => return Err(format!("--work {} has no final name", work.display())),
    };
    if fs::symlink_metadata(work).is_ok() {
        return Err(format!(
            "--work {} exists; give a new directory",
            work.display()
        ));
    }
    let parent = work
        .parent()
        .ok_or_else(|| format!("--work {} has no parent", work.display()))?;
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
            "--work {} is inside the git checkout {}",
            work.display(),
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
    let features = o
        .features
        .iter()
        .map(|f| format!("{f:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "[package]\nname = \"{PROBE}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies.{name}]\nversion = \"={version}\"\ndefault-features = {default}\nfeatures = [{features}]\n\n\
         [workspace]\n",
        name = o.name,
        version = o.version,
        default = !o.no_default_features,
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

pub fn metadata_cmd() -> Cmd {
    Cmd::new(
        "cargo",
        "metadata --format-version 1 --locked --color never",
    )
}

/// `cargo deny check` as the judge runs it, with the repo's `deny.toml`.
pub fn deny_cmd(probe: &Path, repo: &Path) -> Cmd {
    Cmd::new("cargo", "deny --color never --manifest-path")
        .path(&probe.join("Cargo.toml"))
        .words("--config")
        .path(&repo.join("deny.toml"))
        .words("check")
}

pub fn unsafe_cmd(python: &str, probe: &Path, repo: &Path) -> Cmd {
    Cmd::new(python, "-I -c")
        .arg(UNSAFE_SNIPPET)
        .path(&repo.join("ops/judge"))
        .path(probe)
}

/// `cargo vet` without `--locked`: the probe's crates are not in the repo's pruned
/// `imports.lock`, so the imports are fetched (into the probe's copy only).
pub fn vet_cmd(probe: &Path) -> Cmd {
    Cmd::new("cargo", "vet --manifest-path")
        .path(&probe.join("Cargo.toml"))
        .words("--output-format json")
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
        let line = first_error.or(last).unwrap_or("no output");
        format!("exit {}: {}", code_text(self.code), clean(line))
    }
}

fn code_text(code: Option<i32>) -> String {
    code.map_or_else(|| "signal".to_owned(), |c| c.to_string())
}

/// A line of tool output as printed: control characters become `?` and it is cut to
/// [`MAX_LINE`] bytes, so crate data can't forge or hide report lines.
pub fn clean(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let c = if c.is_control() { '?' } else { c };
        if out.len() + c.len_utf8() > MAX_LINE {
            out.push('…');
            break;
        }
        out.push(c);
    }
    out
}

/// One registry package of the probe's graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub license: Option<String>,
    /// Has a build script (`custom-build` target) or is a proc-macro.
    pub build_time: bool,
}

/// The third-party packages of `cargo metadata` output (first-party ones have no `source`).
pub fn parse_metadata(text: &str) -> Result<Vec<Package>, String> {
    let v = json::parse(text).map_err(|e| format!("cargo metadata: {e}"))?;
    let shape = |what: &str| format!("cargo metadata: unexpected shape ({what})");
    let packages = v
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| shape("no packages array"))?;
    let mut out = Vec::new();
    let mut probe = 0usize;
    for p in packages {
        let s = |k: &str| p.get(k).and_then(Value::as_str);
        let (Some(name), Some(version)) = (s("name"), s("version")) else {
            return Err(shape("a package without a name or version"));
        };
        let targets = p
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| shape("a package without targets"))?;
        let mut build_time = false;
        for t in targets {
            let kinds = t
                .get("kind")
                .and_then(Value::as_array)
                .ok_or_else(|| shape("a target without a kind array"))?;
            for k in kinds {
                let k = k
                    .as_str()
                    .ok_or_else(|| shape("a kind that is not a string"))?;
                build_time |= k == "custom-build" || k == "proc-macro";
            }
        }
        match p.get("source") {
            Some(Value::Null) | None if name == PROBE => {
                probe = probe.saturating_add(1);
                continue;
            }
            Some(Value::Null) | None => return Err(shape("a path package that is not the probe")),
            Some(Value::String(_)) => {}
            Some(_) => return Err(shape("a source that is not a string")),
        }
        let license = match p.get("license") {
            Some(Value::String(l)) => Some(l.clone()),
            Some(Value::Null) | None => None,
            Some(_) => return Err(shape("a license that is not a string")),
        };
        out.push(Package {
            name: name.to_owned(),
            version: version.to_owned(),
            license,
            build_time,
        });
    }
    if probe != 1 {
        return Err(shape("the probe package is not listed once"));
    }
    if out.is_empty() {
        return Err(shape("no third-party package"));
    }
    Ok(out)
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

/// `cargo deny check`: exit 0 is a pass; a non-zero exit with its summary line on stdout
/// (`advisories ok, bans ok, licenses FAILED, sources ok`) is a rejection naming the failed
/// checks and each `error[...]` diagnostic with its location line; anything else is a failure.
/// An OFL-1.1 license is noted either way (CONSTITUTION 10 allows it for fonts only).
pub fn deny_result(c: &Captured, packages: &[Package]) -> Checked {
    let mut details: Vec<String> = packages
        .iter()
        .filter(|p| p.license.as_deref().is_some_and(|l| l.contains("OFL-1.1")))
        .map(|p| {
            format!(
                "note: {}@{} is licensed {}: OFL-1.1 is allowed for fonts only (CONSTITUTION 10), check it by hand",
                clean(&p.name),
                clean(&p.version),
                clean(p.license.as_deref().unwrap_or_default())
            )
        })
        .collect();
    let summary = c.stdout.lines().find_map(deny_summary);
    match (c.ok(), summary) {
        (true, Some(failed)) if failed.is_empty() => checked(Outcome::Ok, details),
        (false, Some(failed)) if !failed.is_empty() => {
            details.insert(0, format!("failed: {}", failed.join(", ")));
            let lines: Vec<&str> = c.stderr.lines().collect();
            for (i, l) in lines.iter().enumerate() {
                if l.starts_with("error[") {
                    details.push(clean(l));
                    if let Some(at) = lines.get(i.saturating_add(1)).map(|n| n.trim_start())
                        && at.starts_with("┌─")
                    {
                        details.push(format!("  {}", clean(at)));
                    }
                }
            }
            checked(Outcome::Finding, details)
        }
        _ => checked(
            Outcome::Failed(format!("cargo deny: {}", c.tail())),
            details,
        ),
    }
}

/// The failed checks of a deny summary line, or `None` if the line isn't one: four
/// `<check> <ok|FAILED>` pairs joined by `, `.
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
    let fail = |why: String| checked(Outcome::Failed(why), Vec::new());
    if !c.ok() {
        return fail(format!(
            "judge dependency scan (needs Python 3.11+ for tomllib; see --python): {}",
            c.tail()
        ));
    }
    let last = c
        .stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let list = match json::parse(last) {
        Ok(Value::Array(items)) => items,
        _ => return fail("judge dependency scan: last line is not a JSON list".to_owned()),
    };
    let mut lines = Vec::new();
    for item in &list {
        match item.as_str() {
            Some(s) if s.starts_with("cargo metadata failed") => {
                return fail(format!("judge dependency scan: {}", clean(s)));
            }
            Some(s) => lines.push(clean(s)),
            None => return fail("judge dependency scan: a violation is not a string".to_owned()),
        }
    }
    if lines.is_empty() {
        checked(Outcome::Ok, lines)
    } else {
        checked(Outcome::Finding, lines)
    }
}

/// What `cargo vet` decided, for the build-time step: `None` if it didn't decide.
pub type Unvetted = Option<BTreeSet<(String, String)>>;

/// `cargo vet --output-format json`: exit 0 with conclusion `success` is a pass (nothing
/// unvetted); a non-zero exit with conclusion `fail (vetting)` and a `failures` list is a
/// rejection naming each unvetted `name@version`; another `fail (...)` conclusion (a
/// violation) is a rejection that leaves the unvetted set unknown; anything else is a failure.
pub fn vet_result(c: &Captured) -> (Checked, Unvetted) {
    let fail = |why: String| (checked(Outcome::Failed(why), Vec::new()), None);
    let v = match json::parse(c.stdout.trim()) {
        Ok(v) => v,
        Err(_) => return fail(format!("cargo vet: no JSON report: {}", c.tail())),
    };
    let conclusion = v.get("conclusion").and_then(Value::as_str);
    match (c.ok(), conclusion) {
        (true, Some("success")) => (checked(Outcome::Ok, Vec::new()), Some(BTreeSet::new())),
        (false, Some("fail (vetting)")) => {
            let Some(failures) = v.get("failures").and_then(Value::as_array) else {
                return fail("cargo vet: fail (vetting) without a failures list".to_owned());
            };
            let mut set = BTreeSet::new();
            let mut lines = Vec::new();
            for f in failures {
                let s = |k: &str| f.get(k).and_then(Value::as_str);
                let (Some(name), Some(version)) = (s("name"), s("version")) else {
                    return fail("cargo vet: a failure without a name or version".to_owned());
                };
                let missing: Vec<&str> = f
                    .get("missing_criteria")
                    .and_then(Value::as_array)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(Value::as_str)
                    .collect();
                lines.push(clean(&format!(
                    "{name}@{version}: no imported audit for {}",
                    missing.join(", ")
                )));
                set.insert((name.to_owned(), version.to_owned()));
            }
            if set.is_empty() {
                return fail("cargo vet: fail (vetting) with no failures".to_owned());
            }
            (checked(Outcome::Finding, lines), Some(set))
        }
        (false, Some(other)) if other.starts_with("fail (") => (
            checked(
                Outcome::Finding,
                vec![clean(&format!("conclusion: {other}"))],
            ),
            None,
        ),
        _ => {
            let why = v
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .map_or_else(|| c.tail(), clean);
            fail(format!("cargo vet: {why}"))
        }
    }
}

/// AUTOMATION.md §6.4 "Build-time code": a dependency with a build script or a proc-macro
/// needs an imported audit. Each one is listed; one vet left unvetted is a rejection, and if
/// vet didn't say which crates are unvetted the step fails.
pub fn build_time_result(packages: &[Package], unvetted: &Unvetted) -> Checked {
    let build: Vec<&Package> = packages.iter().filter(|p| p.build_time).collect();
    let Some(unvetted) = unvetted else {
        if build.is_empty() {
            return checked(Outcome::Ok, Vec::new());
        }
        return checked(
            Outcome::Failed("cargo vet did not say which build-time crates are audited".to_owned()),
            build
                .iter()
                .map(|p| clean(&format!("{}@{}", p.name, p.version)))
                .collect(),
        );
    };
    let mut bad = false;
    let details = build
        .iter()
        .map(|p| {
            let audited = !unvetted.contains(&(p.name.clone(), p.version.clone()));
            bad |= !audited;
            clean(&format!(
                "{}@{}: {}",
                p.name,
                p.version,
                if audited { "audited" } else { "not audited" }
            ))
        })
        .collect();
    checked(if bad { Outcome::Finding } else { Outcome::Ok }, details)
}

/// Runs one command; `Err` when it couldn't start.
pub type Runner<'a> = dyn FnMut(&Cmd) -> Result<Captured, String> + 'a;

/// The report: each step's outcome and details, in order.
pub type Report = Vec<(&'static str, Checked)>;

/// The step order (specs/tooling/admit.md, docs/PRINCIPLES.md P4): `probe` yields the package
/// list every later step needs, so it stops the tool; `deny`, `unsafe`, `vet` and `build-time`
/// all run whatever the others found, and `build-time` reads `vet`'s result.
pub fn drive(
    o: &Options,
    probe: &Path,
    repo: &Path,
    write: &mut dyn FnMut() -> Result<(), String>,
    run: &mut Runner<'_>,
) -> (Log, Report) {
    let mut log = Log::printing("xtask: admit: ");
    let mut report = Report::new();
    let packages = probe_step(write, run);
    let record = |log: &mut Log, report: &mut Report, step, c: Checked| {
        log.push(step, c.outcome.clone());
        report.push((step, c));
    };
    let packages = match packages {
        Ok(p) => {
            record(
                &mut log,
                &mut report,
                "probe",
                checked(Outcome::Ok, Vec::new()),
            );
            p
        }
        Err(why) => {
            record(
                &mut log,
                &mut report,
                "probe",
                checked(Outcome::Failed(why), Vec::new()),
            );
            return (log, report);
        }
    };
    let deny = run(&deny_cmd(probe, repo)).map_or_else(
        |e| checked(Outcome::Failed(e), Vec::new()),
        |c| deny_result(&c, &packages),
    );
    record(&mut log, &mut report, "deny", deny);
    let scan = run(&unsafe_cmd(&o.python, probe, repo)).map_or_else(
        |e| checked(Outcome::Failed(e), Vec::new()),
        |c| unsafe_result(&c),
    );
    record(&mut log, &mut report, "unsafe", scan);
    let (vet, unvetted) = run(&vet_cmd(probe)).map_or_else(
        |e| (checked(Outcome::Failed(e), Vec::new()), None),
        |c| vet_result(&c),
    );
    record(&mut log, &mut report, "vet", vet);
    record(
        &mut log,
        &mut report,
        "build-time",
        build_time_result(&packages, &unvetted),
    );
    (log, report)
}

/// Writes the probe, resolves it and reads its package list.
fn probe_step(
    write: &mut dyn FnMut() -> Result<(), String>,
    run: &mut Runner<'_>,
) -> Result<Vec<Package>, String> {
    write()?;
    let lock = run(&lock_cmd())?;
    if !lock.ok() {
        return Err(format!("cargo generate-lockfile: {}", lock.tail()));
    }
    let meta = run(&metadata_cmd())?;
    if !meta.ok() {
        return Err(format!("cargo metadata: {}", meta.tail()));
    }
    parse_metadata(&meta.stdout)
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
        0 => "admitted".to_owned(),
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
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(p)
            .and_then(|mut f| f.write_all(bytes))
            .map_err(|e| format!("write {}: {e}", p.display()))
    };
    let dir = |p: &Path| fs::create_dir(p).map_err(|e| format!("create {}: {e}", p.display()));
    dir(probe)?;
    dir(&probe.join("src"))?;
    dir(&probe.join("supply-chain"))?;
    new(&probe.join("Cargo.toml"), manifest(o).as_bytes())?;
    new(&probe.join("src/lib.rs"), b"")?;
    for rel in COPIED {
        let bytes = fs::read(repo.join(rel)).map_err(|e| format!("read {rel}: {e}"))?;
        new(&probe.join(rel), &bytes)?;
    }
    Ok(())
}

fn live(c: &Cmd, probe: &Path) -> Result<Captured, String> {
    let mut command = Command::new(&c.program);
    command.args(&c.args).current_dir(probe);
    let out = command
        .output()
        .map_err(|e| format!("cannot run {}: {e}", c.program))?;
    Ok(Captured {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests;
