//! `cargo xtask admit <crate>@<version> --work DIR`: a local pre-check of the dependency
//! admission rules (CONSTITUTION 8-10, PLAN §3, AUTOMATION.md §6.4) before a crate is added.
//! Spec: specs/tooling/admit.md (#303, #413).
//!
//! It resolves the candidate in a probe package of its own, outside any git checkout, and runs
//! the judge's own checks on it: `cargo deny check` with the repo's `deny.toml`, the judge's
//! dependency scan (`check_deps` in `ops/judge/policy.py`, imported, never copied), and
//! `cargo vet` with a copy of the repo's store; then it checks that every build script and
//! proc-macro in the graph has an imported audit. It only reports: it never records audits or
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
/// Copied into the probe so rustup picks the pinned toolchain there.
const TOOLCHAIN: &str = "rust-toolchain.toml";
/// cargo vet's store, copied into the probe's own `supply-chain/` so `cargo vet` there uses the
/// repo's imports (and its audits and exemptions, if it ever has any) and fetches into the copy.
const STORE: [&str; 3] = [
    "supply-chain/config.toml",
    "supply-chain/audits.toml",
    "supply-chain/imports.lock",
];
/// Runs the judge's `check_deps` on the probe and prints its violations as one JSON list.
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

/// `<crate>@<version> --work DIR [--features A,B] [--no-default-features] [--python PATH]`,
/// each at most once.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let (mut target, mut work, mut python, mut features) = (None, None, None, None);
    let mut no_default_features = false;
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
            "--features" if features.is_none() => features = Some(value("--features")?),
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
    let python = python.unwrap_or_else(|| "python3".to_owned());
    if python.is_empty() {
        return Err("--python is empty".to_owned());
    }
    let work = work.ok_or("missing --work DIR")?;
    let features = match features {
        None => Vec::new(),
        Some(list) => check_features(&list)?,
    };
    Ok(Options {
        name: name.to_owned(),
        version: version.to_owned(),
        work,
        features,
        no_default_features,
        python,
    })
}

/// A `--features` list: 1 to [`MAX_FEATURES`] comma-separated names, each 1 to [`MAX_FEATURE`]
/// ASCII letters, digits, `_`, `-`, `+` or `.`, so none can leave its TOML string (no `"`, `\`,
/// space or `dep:`/`crate/feature` syntax).
fn check_features(list: &str) -> Result<Vec<String>, String> {
    let names: Vec<&str> = list.split(',').collect();
    if names.len() > MAX_FEATURES {
        return Err(format!(
            "--features lists more than {MAX_FEATURES} features"
        ));
    }
    let ok = |f: &str| {
        !f.is_empty()
            && f.len() <= MAX_FEATURE
            && f.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '+' | '.'))
    };
    match names.iter().find(|f| !ok(f)) {
        Some(bad) => Err(format!(
            "feature {bad:?} is not 1-{MAX_FEATURE} ASCII letters, digits, _, -, + or ."
        )),
        None => Ok(names.into_iter().map(str::to_owned).collect()),
    }
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

/// The probe's `Cargo.toml`: one dependency, pinned with `=`, with the asked features, and an
/// empty `[workspace]` so no parent manifest claims it.
pub fn manifest(o: &Options) -> String {
    let features: Vec<String> = o.features.iter().map(|f| format!("\"{f}\"")).collect();
    format!(
        "[package]\nname = \"{PROBE}\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies.{}]\nversion = \"={}\"\ndefault-features = {}\nfeatures = [{}]\n\n[workspace]\n",
        o.name,
        o.version,
        !o.no_default_features,
        features.join(", ")
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

/// The resolved graph, read for the build-time step and the OFL-1.1 note.
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

/// The judge's scan; `-I` keeps the working directory and `PYTHON*` variables out of the
/// import path.
pub fn unsafe_cmd(python: &str, probe: &Path, repo: &Path) -> Cmd {
    Cmd::new(python, "-I -c")
        .arg(UNSAFE_SNIPPET)
        .path(&repo.join("ops/judge"))
        .path(probe)
}

/// `cargo vet` in the probe, with its copy of the store. Not `--locked`: the repo's
/// `imports.lock` is pruned to the audits the repo's own graph needs, so the candidate's audits
/// must be fetched (into the copy only).
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
        let code = self
            .code
            .map_or_else(|| "signal".to_owned(), |c| c.to_string());
        format!(
            "exit {code}: {}",
            clean(first_error.or(last).unwrap_or("no output"))
        )
    }
}

/// A line of tool output as printed: each character [`hides`] matches becomes `?` and the line
/// is cut to [`MAX_LINE`] bytes, so crate data can't start or reorder a report line.
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
/// isolate (U+202A-U+202E, U+2066-U+2069) or mark (U+200E, U+200F, U+061C).
fn hides(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}')
}

/// One third-party package of the probe's resolved graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub license: Option<String>,
    /// Has a build script (a `custom-build` target) or is a proc-macro.
    pub build_time: bool,
}

/// The third-party packages of `cargo metadata` output: every package but the probe, which must
/// be listed once and be the only one without a `source`. Any other shape is an error.
pub fn parse_metadata(text: &str) -> Result<Vec<Package>, String> {
    let shape = |what: &str| format!("cargo metadata: unexpected shape ({what})");
    let v = json::parse(text).map_err(|_| shape("not JSON"))?;
    let packages = v
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| shape("no packages array"))?;
    let (mut out, mut probes) = (Vec::new(), 0usize);
    for p in packages {
        let text = |k: &str| p.get(k).and_then(Value::as_str);
        let (Some(name), Some(version)) = (text("name"), text("version")) else {
            return Err(shape("a package without a name or version"));
        };
        let mut build_time = false;
        let targets = p.get("targets").and_then(Value::as_array);
        for t in targets.ok_or_else(|| shape("a package without targets"))? {
            let kinds = t.get("kind").and_then(Value::as_array);
            for k in kinds.ok_or_else(|| shape("a target without a kind list"))? {
                let k = k
                    .as_str()
                    .ok_or_else(|| shape("a kind that is not a string"))?;
                build_time |= k == "custom-build" || k == "proc-macro";
            }
        }
        match p.get("source") {
            None | Some(Value::Null) if name == PROBE => {
                probes = probes.saturating_add(1);
                continue;
            }
            None | Some(Value::Null) => return Err(shape("a path package that is not the probe")),
            Some(Value::String(_)) => {}
            Some(_) => return Err(shape("a source that is not a string")),
        }
        let license = match p.get("license") {
            None | Some(Value::Null) => None,
            Some(Value::String(l)) => Some(l.clone()),
            Some(_) => return Err(shape("a license that is not a string")),
        };
        out.push(Package {
            name: name.to_owned(),
            version: version.to_owned(),
            license,
            build_time,
        });
    }
    if probes != 1 {
        return Err(shape("the probe is not listed once"));
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

fn failure(why: String) -> Checked {
    checked(Outcome::Failed(why), Vec::new())
}

/// `cargo deny check`: exit 0 with an all-`ok` summary line on stdout passes; a non-zero exit
/// whose summary line has a `FAILED` check is a rejection naming the failed checks and each
/// `error[...]` diagnostic with its location line; anything else is a failure. Whatever the
/// result, each package whose license expression names OFL-1.1 gets a note: CONSTITUTION 10
/// allows it for fonts only, which `deny.toml` can't tell.
pub fn deny_result(c: &Captured, packages: &[Package]) -> Checked {
    let mut result = deny_verdict(c);
    for p in packages {
        if let Some(license) = p.license.as_deref().filter(|l| l.contains("OFL-1.1")) {
            result.details.push(clean(&format!(
                "note: {}@{} is licensed {license}: OFL-1.1 is allowed for fonts only (CONSTITUTION 10); check it by hand",
                p.name, p.version
            )));
        }
    }
    result
}

fn deny_verdict(c: &Captured) -> Checked {
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

/// The `name@version`s `cargo vet` left unvetted, for the build-time step; `None` when vet
/// didn't say (it failed, or rejected for another reason).
pub type Unvetted = Option<BTreeSet<(String, String)>>;

/// `cargo vet --output-format json`, a JSON document on stdout. Exit 0 with conclusion
/// `success` passes (nothing unvetted). A non-zero exit with conclusion `fail (vetting)` and a
/// non-empty `failures` list is a rejection naming each `name@version` and its missing
/// criteria. A non-zero exit with another `fail (...)` conclusion (a violation) is a rejection
/// that leaves the unvetted set unknown. Anything else fails the step, quoting vet's
/// `error.message` when there is one.
pub fn vet_result(c: &Captured) -> (Checked, Unvetted) {
    let fail = |why: String| (failure(format!("cargo vet: {why}")), None);
    let Ok(v) = json::parse(c.stdout.trim()) else {
        return fail(format!("no JSON report: {}", c.tail()));
    };
    match (c.ok(), v.get("conclusion").and_then(Value::as_str)) {
        (true, Some("success")) => (checked(Outcome::Ok, Vec::new()), Some(BTreeSet::new())),
        (false, Some("fail (vetting)")) => {
            let Some(failures) = v.get("failures").and_then(Value::as_array) else {
                return fail("fail (vetting) without a failures list".to_owned());
            };
            let (mut set, mut lines) = (BTreeSet::new(), Vec::new());
            for f in failures {
                let text = |k: &str| f.get(k).and_then(Value::as_str);
                let (Some(name), Some(version)) = (text("name"), text("version")) else {
                    return fail("a failure without a name or version".to_owned());
                };
                let missing = f.get("missing_criteria").and_then(Value::as_array);
                let missing: Vec<&str> = missing
                    .map(|m| m.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                lines.push(clean(&format!(
                    "{name}@{version}: no imported audit for {}",
                    missing.join(", ")
                )));
                set.insert((name.to_owned(), version.to_owned()));
            }
            if set.is_empty() {
                return fail("fail (vetting) with no failures".to_owned());
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
            let message = v.get("error").and_then(|e| e.get("message"));
            fail(
                message
                    .and_then(Value::as_str)
                    .map_or_else(|| c.tail(), clean),
            )
        }
    }
}

/// AUTOMATION.md §6.4 "Build-time code": every package with a build script or a proc-macro
/// needs an imported audit, so it must not be one vet left unvetted (the judge allows no
/// exemptions or own audits, so vetted means imported). Each one is listed. One unvetted is a
/// rejection; if vet didn't say what is unvetted while there is one, the step fails.
pub fn build_time_result(packages: &[Package], unvetted: &Unvetted) -> Checked {
    let build: Vec<&Package> = packages.iter().filter(|p| p.build_time).collect();
    if build.is_empty() {
        return checked(Outcome::Ok, Vec::new());
    }
    let Some(unvetted) = unvetted else {
        let names = build
            .iter()
            .map(|p| clean(&format!("{}@{}", p.name, p.version)));
        let why = "cargo vet did not say which of these build-time crates are audited".to_owned();
        return checked(Outcome::Failed(why), names.collect());
    };
    let mut rejected = false;
    let mut details = Vec::new();
    for p in build {
        let audited = !unvetted.contains(&(p.name.clone(), p.version.clone()));
        rejected |= !audited;
        let state = if audited { "audited" } else { "not audited" };
        details.push(clean(&format!("{}@{}: {state}", p.name, p.version)));
    }
    let outcome = if rejected {
        Outcome::Finding
    } else {
        Outcome::Ok
    };
    checked(outcome, details)
}

/// Runs one command; `Err` when it couldn't start.
pub type Runner<'a> = dyn FnMut(&Cmd) -> Result<Captured, String> + 'a;

/// The report: each step's outcome and details, in order.
pub type Report = Vec<(&'static str, Checked)>;

/// The step order (specs/tooling/admit.md, docs/PRINCIPLES.md P4): `probe` writes and resolves
/// the probe and reads its package list, which every later step needs, so its failure stops
/// the tool; `deny`, `unsafe`, `vet` and `build-time` each run whatever the others found, and
/// `build-time` reads `vet`'s result.
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
    let packages = match probe_step(write, run) {
        Ok(packages) => packages,
        Err(why) => {
            record("probe", failure(why));
            return (log, report);
        }
    };
    record("probe", checked(Outcome::Ok, Vec::new()));
    let deny = run(&deny_cmd(probe, repo)).map_or_else(failure, |c| deny_result(&c, &packages));
    record("deny", deny);
    let scan = run(&unsafe_cmd(&o.python, probe, repo)).map_or_else(failure, |c| unsafe_result(&c));
    record("unsafe", scan);
    let (vet, unvetted) =
        run(&vet_cmd(probe)).map_or_else(|e| (failure(e), None), |c| vet_result(&c));
    record("vet", vet);
    record("build-time", build_time_result(&packages, &unvetted));
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
        let mut options = fs::OpenOptions::new();
        let file = options.write(true).create_new(true).open(p);
        file.and_then(|mut f| f.write_all(bytes))
            .map_err(|e| format!("write {}: {e}", p.display()))
    };
    for dir in [
        probe.to_path_buf(),
        probe.join("src"),
        probe.join("supply-chain"),
    ] {
        fs::create_dir(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    new(&probe.join("Cargo.toml"), manifest(o).as_bytes())?;
    new(&probe.join("src/lib.rs"), b"")?;
    for rel in STORE.into_iter().chain([TOOLCHAIN]) {
        let bytes = fs::read(repo.join(rel)).map_err(|e| format!("read {rel}: {e}"))?;
        new(&probe.join(rel), &bytes)?;
    }
    Ok(())
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
