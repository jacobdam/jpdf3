//! `cargo xtask issue-lint`: every open issue carries exactly one `area:` label from
//! `ops/areas.toml` and one `origin:` label, and a follow-up names its parent (ADR 0011 §4;
//! specs/tooling/issue-lint.md). Read-only: it runs `git ls-files` and one `gh issue list`.
//!
//! GitHub text (bodies, label names) is data: it is only compared, and anything printed from it
//! goes through `{:?}`, so it can't forge a report line.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use jpdf_testgen::json::{self, Value};

/// `--limit` of the issue list; a list of exactly this many may be cut off (exit 2).
pub const LIST_LIMIT: usize = 1000;
/// Longest area name: a GitHub label holds 50 characters and `area:` takes 5.
pub const MAX_NAME: usize = 45;
/// The `origin:` values (ADR 0011 §4).
pub const ORIGINS: [&str; 6] = ["owner", "review", "audit", "qc", "discovery", "follow-up"];
/// Areas ADR 0011 §3 names; both must exist with `kind = "process"`.
const REQUIRED_PROCESS: [&str; 2] = ["process", "records"];
const AREAS_FILE: &str = "ops/areas.toml";
const FOLLOW_UP: &str = "Follow-up of: #";

/// The live issue list command (test `live_command_flags` pins it).
pub fn gh_args() -> Vec<String> {
    let limit = LIST_LIMIT.to_string();
    [
        "issue",
        "list",
        "--state",
        "open",
        "--limit",
        &limit,
        "--json",
        "number,labels,body",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

pub struct Options {
    pub from: Option<PathBuf>,
}

pub fn parse(args: &[String]) -> Result<Options, String> {
    match args {
        [] => Ok(Options { from: None }),
        [flag, dir] if flag == "--from" => Ok(Options {
            from: Some(PathBuf::from(dir)),
        }),
        _ => Err("issue-lint takes only --from DIR".to_owned()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    pub name: String,
    pub kind: Kind,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Product,
    Process,
}

/// The report: stdout text and whether any problem was found.
pub struct Report {
    pub text: String,
    pub problems: usize,
}

/// Lints the inputs: `Ok` with the report (exit 0 or 1), `Err` when it can't run (exit 2).
pub fn run(root: &Path, opts: &Options) -> Result<Report, String> {
    let (areas_text, tree, issues) = match &opts.from {
        Some(dir) => (
            read(&dir.join("areas.toml"))?,
            read(&dir.join("tree.txt"))?,
            read(&dir.join("issues.json"))?,
        ),
        None => {
            let areas = read(&root.join(AREAS_FILE))?;
            let root_arg = root.to_string_lossy();
            // `-z`: paths unquoted (no `core.quotePath` C-quoting), NUL-separated.
            let tree = crate::macos::capture("git", &["-C", &root_arg, "ls-files", "-z"], None)
                .map(|(out, _)| out.split('\0').collect::<Vec<_>>().join("\n"))
                .map_err(|e| format!("git ls-files: {e}"))?;
            let args = gh_args();
            let argv: Vec<&str> = args.iter().map(String::as_str).collect();
            let issues = crate::macos::capture("gh", &argv, None)
                .map(|(out, _)| out)
                .map_err(|e| format!("gh issue list: {e}"))?;
            (areas, tree, issues)
        }
    };
    lint(&areas_text, &tree, &issues)
}

fn read(p: &Path) -> Result<String, String> {
    fs::read_to_string(p).map_err(|e| format!("read {}: {e}", p.display()))
}

/// The pure lint over the three input texts.
pub fn lint(areas_text: &str, tree: &str, issues: &str) -> Result<Report, String> {
    let areas = parse_areas(areas_text).map_err(|e| format!("{AREAS_FILE}: {e}"))?;
    let files: BTreeSet<&str> = tree.lines().filter(|l| !l.is_empty()).collect();
    if files.is_empty() {
        return Err("the tracked-file list is empty: nothing to check paths against".to_owned());
    }
    let list = parse_issues(issues)?;
    if list.is_empty() {
        return Err("the open issue list is empty: nothing was checked".to_owned());
    }
    if list.len() >= LIST_LIMIT {
        return Err(format!(
            "gh returned {} issues, the --limit: the list may be cut off",
            list.len()
        ));
    }
    let names: BTreeSet<&str> = areas.iter().map(|a| a.name.as_str()).collect();
    let mut text = String::new();
    let mut problems = 0usize;
    let mut bad_issues = 0usize;
    for issue in &list {
        let found = issue_problems(issue, &names);
        if !found.is_empty() {
            bad_issues += 1;
        }
        for p in found {
            problems += 1;
            let _ = writeln!(text, "#{}: {p}", issue.number);
        }
    }
    for a in &areas {
        for p in &a.paths {
            if !covers(p, &files) {
                problems += 1;
                let _ = writeln!(
                    text,
                    "{AREAS_FILE}: path {p} of area {} matches no tracked file",
                    a.name
                );
            }
        }
    }
    if problems == 0 {
        let _ = writeln!(text, "issue-lint: {} open issues, no problems", list.len());
    } else {
        let _ = writeln!(
            text,
            "issue-lint: {problems} problem(s) in {bad_issues} of {} open issues",
            list.len()
        );
    }
    Ok(Report { text, problems })
}

/// A listed path covers a tracked file: the file itself, or, for a directory path (ending in
/// `/`), any file under it.
fn covers(path: &str, files: &BTreeSet<&str>) -> bool {
    if path.ends_with('/') {
        files
            .range::<str, _>((std::ops::Bound::Included(path), std::ops::Bound::Unbounded))
            .next()
            .is_some_and(|f| f.starts_with(path))
    } else {
        files.contains(path)
    }
}

struct Issue {
    number: u64,
    labels: Vec<String>,
    body: String,
}

fn parse_issues(text: &str) -> Result<Vec<Issue>, String> {
    let what = "gh issue list";
    let v = json::parse(text).map_err(|e| format!("{what}: {e}"))?;
    let items = v
        .as_array()
        .ok_or_else(|| format!("{what}: not a JSON list"))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let number = match item.get("number") {
            Some(Value::Number(n)) => n
                .parse::<u64>()
                .map_err(|_| format!("{what}: bad issue number {n:?}"))?,
            _ => return Err(format!("{what}: an issue without a number")),
        };
        let labels = item
            .get("labels")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{what}: #{number} has no labels list"))?
            .iter()
            .map(|l| {
                l.get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{what}: #{number} has a label without a name"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let body = item
            .get("body")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{what}: #{number} has no body string"))?
            .to_owned();
        out.push(Issue {
            number,
            labels,
            body,
        });
    }
    out.sort_by_key(|i| i.number);
    Ok(out)
}

/// `prefix` matched without ASCII case at the start of `s`.
fn has_prefix(s: &str, prefix: &str) -> bool {
    s.get(..prefix.len())
        .is_some_and(|h| h.eq_ignore_ascii_case(prefix))
}

/// The problems of one issue, in the spec's order.
fn issue_problems(issue: &Issue, areas: &BTreeSet<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let mut known_origin = Vec::new();
    for (prefix, what) in [("area:", "area"), ("origin:", "origin")] {
        let labels: Vec<&str> = issue
            .labels
            .iter()
            .map(String::as_str)
            .filter(|l| has_prefix(l, prefix))
            .collect();
        match labels.len() {
            0 => out.push(format!("no {prefix} label")),
            1 => {}
            k => out.push(format!("{k} {prefix} labels")),
        }
        for l in &labels {
            let value = l.strip_prefix(prefix);
            let known = match (what, value) {
                ("area", Some(v)) => areas.contains(v),
                ("origin", Some(v)) => ORIGINS.contains(&v),
                _ => false,
            };
            if !known {
                out.push(format!("unknown {what} label {l:?}"));
            } else if what == "origin" {
                known_origin.extend(value);
            }
        }
    }
    let mut parents = Vec::new();
    for line in issue.body.lines() {
        let line = line.trim_start();
        if !has_prefix(line, "follow-up of:") {
            continue;
        }
        match follow_up_parent(line) {
            Some(n) => parents.push(n),
            None => out.push(format!("malformed Follow-up of: line {line:?}")),
        }
    }
    let is_follow_up = known_origin.contains(&"follow-up");
    if is_follow_up && parents.is_empty() {
        out.push("origin:follow-up without a Follow-up of: #N line".to_owned());
    }
    if !parents.is_empty() && !is_follow_up && known_origin.len() == 1 {
        out.push("Follow-up of: line but origin is not follow-up".to_owned());
    }
    if parents.len() > 1 {
        out.push(format!("{} Follow-up of: lines", parents.len()));
    }
    if parents.contains(&issue.number) {
        out.push("Follow-up of: itself".to_owned());
    }
    out
}

/// `N` of an exact `Follow-up of: #N` line (trailing whitespace allowed), else `None`.
fn follow_up_parent(line: &str) -> Option<u64> {
    let digits = line.strip_prefix(FOLLOW_UP)?.trim_end();
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || digits.starts_with('0') {
        return None;
    }
    digits.parse().ok()
}

// ---------------------------------------------------------------------------------------------
// ops/areas.toml: a strict TOML subset (specs/tooling/issue-lint.md)

#[derive(Default)]
struct Partial {
    line: usize,
    name: Option<String>,
    kind: Option<String>,
    paths: Option<Vec<String>>,
}

pub fn parse_areas(text: &str) -> Result<Vec<Area>, String> {
    let mut partials: Vec<Partial> = Vec::new();
    let mut lines = text.lines().enumerate().map(|(i, l)| (i + 1, l));
    while let Some((n, raw)) = lines.next() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[area]]" {
            partials.push(Partial {
                line: n,
                ..Partial::default()
            });
            continue;
        }
        let err = |m: &str| format!("line {n}: {m}");
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| err("expected [[area]], a comment or key = value"))?;
        let (key, value) = (key.trim(), value.trim());
        let cur = partials
            .last_mut()
            .ok_or_else(|| err("a key before the first [[area]]"))?;
        match key {
            "name" | "kind" => {
                let s = string(value).ok_or_else(|| err("expected one \"string\""))?;
                let slot = if key == "name" {
                    &mut cur.name
                } else {
                    &mut cur.kind
                };
                if slot.replace(s).is_some() {
                    return Err(err(&format!("{key} repeated")));
                }
            }
            "paths" => {
                // An array may span lines: gather text up to its closing `]`.
                let mut acc = value.to_owned();
                while !acc.contains(']') {
                    match lines.next() {
                        Some((_, more)) => {
                            acc.push(' ');
                            acc.push_str(more.trim());
                        }
                        None => return Err(err("paths array not closed")),
                    }
                }
                let items = array(&acc).ok_or_else(|| err("expected [\"path\", …]"))?;
                if cur.paths.replace(items).is_some() {
                    return Err(err("paths repeated"));
                }
            }
            _ => return Err(err(&format!("unknown key {key:?}"))),
        }
    }
    if partials.is_empty() {
        return Err("no [[area]]".to_owned());
    }
    let mut areas = Vec::with_capacity(partials.len());
    let mut names = BTreeSet::new();
    let mut seen_paths = BTreeSet::new();
    for p in partials {
        let at = |m: String| format!("area at line {}: {m}", p.line);
        let name = p.name.ok_or_else(|| at("no name".to_owned()))?;
        let kind = match p.kind.as_deref() {
            Some("product") => Kind::Product,
            Some("process") => Kind::Process,
            Some(k) => return Err(at(format!("kind {k:?} is not product or process"))),
            None => return Err(at("no kind".to_owned())),
        };
        let paths = p.paths.ok_or_else(|| at("no paths".to_owned()))?;
        if !valid_name(&name) {
            return Err(at(format!(
                "name {name:?} is not [a-z][a-z0-9-]* of at most {MAX_NAME} bytes"
            )));
        }
        if !names.insert(name.clone()) {
            return Err(at(format!("name {name:?} repeated")));
        }
        if paths.is_empty() {
            return Err(at("paths is empty".to_owned()));
        }
        for path in &paths {
            if !valid_path(path) {
                return Err(at(format!("path {path:?} is not a clean relative path")));
            }
            if !seen_paths.insert(path.clone()) {
                return Err(at(format!("path {path:?} listed twice")));
            }
        }
        areas.push(Area { name, kind, paths });
    }
    for req in REQUIRED_PROCESS {
        if !areas
            .iter()
            .any(|a| a.name == req && a.kind == Kind::Process)
        {
            return Err(format!("no area {req:?} with kind = \"process\""));
        }
    }
    Ok(areas)
}

/// The content of `"…"`: no `\`, `"` or control characters inside.
fn string(s: &str) -> Option<String> {
    let inner = s.strip_prefix('"')?.strip_suffix('"')?;
    inner
        .chars()
        .all(|c| c != '\\' && c != '"' && !c.is_control())
        .then(|| inner.to_owned())
}

/// The strings of `[ "a", "b", ]` (a trailing comma allowed, nothing after `]`).
fn array(s: &str) -> Option<Vec<String>> {
    let inner = s.strip_prefix('[')?.strip_suffix(']')?.trim();
    let inner = inner.strip_suffix(',').unwrap_or(inner);
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    inner.split(',').map(|item| string(item.trim())).collect()
}

fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= MAX_NAME
        && bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn valid_path(path: &str) -> bool {
    let trimmed = path.strip_suffix('/').unwrap_or(path);
    !trimmed.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\\', '*'])
        && !path.chars().any(char::is_whitespace)
        && trimmed
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(name: &str, kind: &str, paths: &str) -> String {
        format!("[[area]]\nname = \"{name}\"\nkind = \"{kind}\"\npaths = {paths}\n")
    }

    fn base() -> String {
        format!(
            "{}{}",
            area("process", "process", r#"["ops/"]"#),
            area("records", "process", r#"["ops/journal/"]"#)
        )
    }

    #[test]
    fn live_command_flags() {
        assert_eq!(
            gh_args().join(" "),
            "issue list --state open --limit 1000 --json number,labels,body"
        );
    }

    #[test]
    fn repository_areas_parse_and_name_existing_paths() {
        // The checkout's files, not `git ls-files`: the test may run without `.git`. The live
        // lint checks the tracked files.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let text = fs::read_to_string(root.join(AREAS_FILE)).expect("read ops/areas.toml");
        let areas = parse_areas(&text).expect("ops/areas.toml parses");
        for a in &areas {
            for p in &a.paths {
                let full = root.join(p);
                let ok = if p.ends_with('/') {
                    fs::read_dir(&full).is_ok_and(|mut d| d.next().is_some())
                } else {
                    full.is_file()
                };
                assert!(ok, "{p} of {} names no file", a.name);
            }
        }
    }

    #[test]
    fn arguments() {
        let s = |v: &[&str]| v.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();
        assert!(parse(&[]).is_ok_and(|o| o.from.is_none()));
        assert!(parse(&s(&["--from", "d"])).is_ok_and(|o| o.from == Some(PathBuf::from("d"))));
        for bad in [
            &["--json", "d"][..],
            &["--from"],
            &["d"],
            &["--from", "d", "x"],
        ] {
            assert!(parse(&s(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn errors_name_their_line() {
        let e = parse_areas("# c\n\n[[area]]\ncolor = \"red\"\n").expect_err("unknown key");
        assert!(e.starts_with("line 4: "), "{e}");
        let e = parse_areas("# c\n[[area]]\nname = \"x\"\n").expect_err("no kind");
        assert!(e.starts_with("area at line 2: "), "{e}");
    }

    #[test]
    fn base_parses() {
        let a = parse_areas(&base()).expect("base parses");
        assert_eq!(a.len(), 2);
        assert_eq!(a[1].paths, vec!["ops/journal/".to_owned()]);
    }

    #[test]
    fn multi_line_array_with_trailing_comma_and_comments() {
        let text = format!(
            "# head\n\n{}[[area]]\n# c\nkind = \"product\"\npaths = [\n  \"a/\",\n  \"b.md\",\n]\nname = \"x-1\"\n",
            base()
        );
        let a = parse_areas(&text).expect("parses");
        assert_eq!(a[2].name, "x-1");
        assert_eq!(a[2].kind, Kind::Product);
        assert_eq!(a[2].paths, vec!["a/".to_owned(), "b.md".to_owned()]);
    }

    #[test]
    fn name_length_boundary() {
        let ok = "a".repeat(MAX_NAME);
        let long = "a".repeat(MAX_NAME + 1);
        assert!(parse_areas(&format!("{}{}", base(), area(&ok, "product", r#"["x/"]"#))).is_ok());
        let e = parse_areas(&format!(
            "{}{}",
            base(),
            area(&long, "product", r#"["x/"]"#)
        ))
        .expect_err("46 bytes");
        assert!(e.contains("at most 45"), "{e}");
    }

    #[test]
    fn rejected_area_files() {
        let b = base();
        let cases: Vec<(String, &str)> = vec![
            (String::new(), "no [[area]]"),
            ("name = \"x\"\n".to_owned() + &b, "before the first"),
            (b.clone() + "[area]\n", "expected [[area]]"),
            (
                b.clone() + &area("x", "product", r#"["x/"]"#) + "color = \"red\"\n",
                "unknown key",
            ),
            (
                b.clone() + &area("x", "product", r#"["x/"]"#) + "name = \"y\"\n",
                "name repeated",
            ),
            (
                b.clone() + &area("x", "product", r#"["x/"]"#) + "kind = \"product\"\n",
                "kind repeated",
            ),
            (
                b.clone() + &area("x", "product", r#"["x/"]"#) + "paths = [\"y/\"]\n",
                "paths repeated",
            ),
            (
                b.clone() + "[[area]]\nkind = \"product\"\npaths = [\"x/\"]\n",
                "no name",
            ),
            (
                b.clone() + "[[area]]\nname = \"x\"\npaths = [\"x/\"]\n",
                "no kind",
            ),
            (
                b.clone() + "[[area]]\nname = \"x\"\nkind = \"product\"\n",
                "no paths",
            ),
            (
                b.clone() + &area("x", "both", r#"["x/"]"#),
                "not product or process",
            ),
            (b.clone() + &area("x", "product", "[]"), "paths is empty"),
            (b.clone() + &area("x", "product", "[\"x/\""), "not closed"),
            (
                b.clone() + &area("x", "product", r#"["x/"] # c"#),
                "expected [\"path\"",
            ),
            (
                b.clone() + &area("x", "product", r#"["x/" "y/"]"#),
                "expected [\"path\"",
            ),
            (
                b.clone() + &area("x", "product", "[\n  # c\n  \"x/\",\n]"),
                "expected [\"path\"",
            ),
            (
                b.clone() + &area("x\ty", "product", r#"["x/"]"#),
                "expected one",
            ),
            (
                b.clone() + &area("x", "product", "[\"a\tb\"]"),
                "expected [\"path\"",
            ),
            (b.clone() + "[[area]]\nname = \"x\" # c\n", "expected one"),
            (b.clone() + "[[area]]\nname = 'x'\n", "expected one"),
            (
                b.clone() + "[[area]]\nname = \"a\\u0041\"\n",
                "expected one",
            ),
            (
                b.clone() + &area("X", "product", r#"["x/"]"#),
                "is not [a-z]",
            ),
            (
                b.clone() + &area("1x", "product", r#"["x/"]"#),
                "is not [a-z]",
            ),
            (
                b.clone() + &area("x_y", "product", r#"["x/"]"#),
                "is not [a-z]",
            ),
            (
                b.clone() + &area("", "product", r#"["x/"]"#),
                "is not [a-z]",
            ),
            (
                b.clone() + &area("process", "product", r#"["x/"]"#),
                "repeated",
            ),
            (
                b.clone() + &area("x", "product", r#"["ops/"]"#),
                "listed twice",
            ),
            (
                b.clone() + &area("x", "product", r#"["/x"]"#),
                "clean relative",
            ),
            (
                b.clone() + &area("x", "product", r#"["a/../b"]"#),
                "clean relative",
            ),
            (
                b.clone() + &area("x", "product", r#"["./a"]"#),
                "clean relative",
            ),
            (
                b.clone() + &area("x", "product", r#"["a//b"]"#),
                "clean relative",
            ),
            (
                b.clone() + &area("x", "product", r#"["a\b"]"#),
                "expected [\"path\"",
            ),
            (
                b.clone() + &area("x", "product", r#"["a b"]"#),
                "clean relative",
            ),
            (
                b.clone() + &area("x", "product", r#"["a/*"]"#),
                "clean relative",
            ),
            (
                b.clone() + &area("x", "product", r#"["/"]"#),
                "clean relative",
            ),
            (
                b.clone() + &area("x", "product", r#"[""]"#),
                "clean relative",
            ),
            (
                area("process", "process", r#"["ops/"]"#),
                "no area \"records\"",
            ),
            (
                area("process", "process", r#"["ops/"]"#)
                    + &area("records", "product", r#"["r/"]"#),
                "no area \"records\"",
            ),
            (
                area("records", "process", r#"["r/"]"#)
                    + &area("process", "product", r#"["ops/"]"#),
                "no area \"process\"",
            ),
        ];
        for (text, want) in cases {
            let e = parse_areas(&text).expect_err(&text);
            assert!(e.contains(want), "{text:?}: {e:?} lacks {want:?}");
        }
    }

    #[test]
    fn cover_checks() {
        let files: BTreeSet<&str> = ["a/b/c.rs", "ab.md", "z"].into_iter().collect();
        assert!(covers("a/", &files));
        assert!(covers("a/b/", &files));
        assert!(covers("ab.md", &files));
        assert!(!covers("a/b/c/", &files));
        assert!(!covers("a", &files), "a file path needs an exact file");
        assert!(!covers("ab/", &files), "ab.md is not under ab/");
        assert!(!covers("b/", &files));
    }

    #[test]
    fn follow_up_line_shapes() {
        assert_eq!(follow_up_parent("Follow-up of: #7"), Some(7));
        assert_eq!(follow_up_parent("Follow-up of: #7 \r"), Some(7));
        assert_eq!(
            follow_up_parent("Follow-up of: #18446744073709551615"),
            Some(u64::MAX)
        );
        for bad in [
            "Follow-up of: #",
            "Follow-up of: #0",
            "Follow-up of: #07",
            "Follow-up of: 7",
            "Follow-up of: #7 and #8",
            "Follow-up of: #7.",
            "Follow-up of: #+7",
            "Follow-up of:  #7",
            "follow-up of: #7",
            "Follow-up of: #18446744073709551616",
        ] {
            assert_eq!(follow_up_parent(bad), None, "{bad}");
        }
    }
}
