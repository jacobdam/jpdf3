//! `cargo xtask next-session`: the session scheduler, precedence steps 1–5 (ADR 0011 §2, task 1a;
//! specs/tooling/next-session.md). Read-only: it runs read commands and writes nothing.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use jpdf_testgen::json::{self, Value};

const OWNER: &str = "jacobdam";
const TAG: &str = "🤖 jpdf-agent";
const ISSUE_URL: &str = "https://github.com/jacobdam/jpdf3/issues/";
const FEED_SCRIPT: &str = "ops/driver/owner-issues.sh";
const ALARM_FILE: &str = "ops/state/judge-alarm-since";
const SKIP_LABELS: [&str; 3] = ["owner-todo", "blocked", "deferred"];

pub struct Options {
    pub from: Option<PathBuf>,
}

pub fn parse(args: &[String]) -> Result<Options, String> {
    match args {
        [] => Ok(Options { from: None }),
        [flag, dir] if flag == "--from" => Ok(Options {
            from: Some(PathBuf::from(dir)),
        }),
        _ => Err("next-session takes only --from DIR".to_owned()),
    }
}

/// Exit 2 (`Unknown`: a command failed, the session does no work) or exit 1 (`Broken`).
pub enum Error {
    Unknown(String),
    Broken(String),
}

/// The raw inputs, one text per file name of the spec.
struct Inputs {
    feed: String,
    floor: String,
    owner_open: String,
    owner_closed: String,
    prs: String,
    issues: String,
    runs: String,
    worktrees: String,
    alarm: bool,
}

/// The value of the script's `FLOOR="…"` line.
fn floor_of(script: &str) -> Option<&str> {
    script.lines().find_map(|l| {
        l.strip_prefix("FLOOR=\"")
            .and_then(|r| r.split_once('"'))
            .map(|(v, _)| v)
    })
}

fn read(dir: &Path, name: &str) -> Result<String, Error> {
    let p = dir.join(name);
    fs::read_to_string(&p).map_err(|e| Error::Broken(format!("read {}: {e}", p.display())))
}

fn exists(p: &Path) -> Result<bool, Error> {
    match fs::metadata(p) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Error::Broken(format!("read {}: {e}", p.display()))),
    }
}

fn cmd(program: &str, args: &[&str]) -> Result<String, Error> {
    crate::macos::capture(program, args, None)
        .map(|(out, _)| out)
        .map_err(Error::Unknown)
}

fn gh(args: &str) -> Result<String, Error> {
    // The arguments hold no spaces, so split_whitespace is exact.
    cmd("gh", &args.split_whitespace().collect::<Vec<_>>())
}

fn load(root: &Path, from: Option<&Path>) -> Result<Inputs, Error> {
    if let Some(dir) = from {
        return Ok(Inputs {
            feed: read(dir, "feed.jsonl")?,
            floor: read(dir, "floor.txt")?.trim().to_owned(),
            owner_open: read(dir, "owner-open.json")?,
            owner_closed: read(dir, "owner-closed.json")?,
            prs: read(dir, "prs.json")?,
            issues: read(dir, "issues.json")?,
            runs: read(dir, "runs.json")?,
            worktrees: read(dir, "worktrees.txt")?,
            alarm: exists(&dir.join("judge-alarm-since"))?,
        });
    }
    let script = root.join(FEED_SCRIPT);
    let feed = cmd(&script.to_string_lossy(), &[])?;
    let text = read(root, FEED_SCRIPT)?;
    let floor = floor_of(&text)
        .ok_or_else(|| Error::Broken(format!("no FLOOR=\"…\" line in {FEED_SCRIPT}")))?
        .to_owned();
    let today = crate::metrics::today().map_err(Error::Broken)?;
    let since = crate::metrics::fmt_day(
        today
            .checked_sub(14)
            .ok_or_else(|| Error::Broken("system clock out of range".to_owned()))?,
    );
    Ok(Inputs {
        feed,
        floor,
        owner_open: gh(&format!(
            "issue list --state open --author {OWNER} --limit 1000 --json number,labels,comments"
        ))?,
        owner_closed: gh(&format!(
            "issue list --state closed --author {OWNER} --search closed:>={since} --limit 1000 --json number,body,comments"
        ))?,
        prs: gh(
            "pr list --author @me --state all --limit 200 --json number,state,isDraft,body,headRefName,labels,statusCheckRollup,mergeable",
        )?,
        issues: gh("issue list --state open --limit 1000 --json number,title,labels")?,
        runs: gh(
            "run list --workflow ci.yml --branch main --limit 20 --json databaseId,status,conclusion,event",
        )?,
        worktrees: cmd("git", &["worktree", "list", "--porcelain"])?,
        alarm: exists(&root.join(ALARM_FILE))?,
    })
}

/// Runs the scheduler; returns the pick and brief (stdout).
pub fn run(root: &Path, opts: &Options) -> Result<String, Error> {
    let inp = load(root, opts.from.as_deref())?;
    decide(&inp).map_err(Error::Broken)
}

// ---------------------------------------------------------------------------------------------
// Parsing

fn field<'a>(v: &'a Value, key: &str, what: &str) -> Result<&'a Value, String> {
    v.get(key)
        .ok_or_else(|| format!("{what}: missing field {key:?}"))
}

fn text<'a>(v: &'a Value, key: &str, what: &str) -> Result<&'a str, String> {
    field(v, key, what)?
        .as_str()
        .ok_or_else(|| format!("{what}: field {key:?} is not a string"))
}

fn number(v: &Value, key: &str, what: &str) -> Result<u64, String> {
    match field(v, key, what)? {
        Value::Number(n) if !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()) => n
            .parse()
            .map_err(|_| format!("{what}: field {key:?} out of range")),
        _ => Err(format!("{what}: field {key:?} is not a whole number")),
    }
}

fn list<'a>(v: &'a Value, what: &str) -> Result<&'a [Value], String> {
    v.as_array()
        .ok_or_else(|| format!("{what}: not a JSON array"))
}

fn parse_list(text: &str, what: &str) -> Result<Value, String> {
    let v = json::parse(text).map_err(|e| format!("{what}: {e}"))?;
    list(&v, what)?;
    Ok(v)
}

fn labels(v: &Value, what: &str) -> Result<Vec<String>, String> {
    list(field(v, "labels", what)?, what)?
        .iter()
        .map(|l| text(l, "name", what).map(str::to_owned))
        .collect()
}

/// Bodies of the comments the owner account wrote (a null author is not the owner).
fn owner_replies(v: &Value, what: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for c in list(field(v, "comments", what)?, what)? {
        let body = text(c, "body", what)?;
        let login = match field(c, "author", what)? {
            Value::Null => None,
            a => Some(text(a, "login", what)?),
        };
        if login == Some(OWNER) {
            out.push(body.to_owned());
        }
    }
    Ok(out)
}

fn has_label(labels: &[String], name: &str) -> bool {
    labels.iter().any(|l| l == name)
}

fn skipped(labels: &[String]) -> bool {
    SKIP_LABELS.iter().any(|s| has_label(labels, s))
}

/// `body` names `url` exactly: at a match, the next character isn't a digit (so a link to
/// `…-1234` never counts for `…-123`).
fn names_url(body: &str, url: &str) -> bool {
    body.match_indices(url).any(|(i, _)| {
        !body
            .get(i + url.len()..)
            .and_then(|r| r.chars().next())
            .is_some_and(|c| c.is_ascii_digit())
    })
}

/// An owner reply `<TAG> done: <url>` with the URL followed by whitespace or the end.
fn done_reply(replies: &[String], url: &str) -> bool {
    reply(replies, "done:", url)
}

/// An owner reply `<TAG> <word> <url>` with the URL followed by whitespace or the end.
fn reply(replies: &[String], word: &str, url: &str) -> bool {
    let prefix = format!("{TAG} {word} {url}");
    replies.iter().any(|b| {
        b.strip_prefix(&prefix)
            .is_some_and(|r| r.chars().next().is_none_or(char::is_whitespace))
    })
}

fn tagged(body: &str) -> bool {
    body.trim_start().starts_with(TAG)
}

/// The number in the first `Closes #N` of a PR body.
/// `Closes #` without digits after it (a quoted `Closes #<issue>`) is skipped.
fn linked_issue(body: &str) -> Option<u64> {
    body.split("Closes #").skip(1).find_map(|rest| {
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    })
}

struct Pr {
    number: u64,
    open: bool,
    body: String,
    branch: String,
    labels: Vec<String>,
    failing: bool,
}

fn failing(rollup: &Value, what: &str) -> Result<bool, String> {
    let entries = match rollup {
        Value::Null => &[][..],
        v => list(v, what)?,
    };
    let mut reviewed = false;
    let mut bad = false;
    for e in entries {
        match e.get("__typename").and_then(Value::as_str) {
            Some("CheckRun") => {
                let c = e.get("conclusion").and_then(Value::as_str).unwrap_or("");
                bad |= matches!(
                    c,
                    "FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE" | "ACTION_REQUIRED" | "CANCELLED"
                );
            }
            Some("StatusContext") => {
                let state = text(e, "state", what)?;
                bad |= matches!(state, "FAILURE" | "ERROR");
                reviewed |= text(e, "context", what)? == "ai-review" && state == "SUCCESS";
            }
            _ => {}
        }
    }
    Ok(bad || !reviewed)
}

/// `mergeable` is `CONFLICTING`: auto-merge can never land the PR (#270). `UNKNOWN` (GitHub
/// computes it lazily) and `MERGEABLE` don't count; any other value is a parse error.
fn conflicting(p: &Value, what: &str) -> Result<bool, String> {
    match text(p, "mergeable", what)? {
        "CONFLICTING" => Ok(true),
        "MERGEABLE" | "UNKNOWN" => Ok(false),
        other => Err(format!("{what}: unexpected mergeable {other:?}")),
    }
}

fn parse_prs(text_: &str) -> Result<Vec<Pr>, String> {
    let what = "prs.json";
    let v = parse_list(text_, what)?;
    let mut out = Vec::new();
    for p in list(&v, what)? {
        let open = text(p, "state", what)? == "OPEN";
        let body = text(p, "body", what)?;
        // Parsed for every PR (not only open ones), so a renamed field always fails.
        let conflict = conflicting(p, what)?;
        let pr = Pr {
            number: number(p, "number", what)?,
            open,
            body: body.to_owned(),
            branch: text(p, "headRefName", what)?.to_owned(),
            labels: labels(p, what)?,
            failing: open && (conflict || failing(field(p, "statusCheckRollup", what)?, what)?),
        };
        // The account is shared with the owner: only tagged PRs are the agent's own.
        if tagged(body) {
            out.push(pr);
        }
    }
    out.sort_by_key(|p| p.number);
    Ok(out)
}

struct Issue {
    number: u64,
    title: String,
    labels: Vec<String>,
}

fn parse_issues(text_: &str) -> Result<Vec<Issue>, String> {
    let what = "issues.json";
    let v = parse_list(text_, what)?;
    let mut out = Vec::new();
    for i in list(&v, what)? {
        out.push(Issue {
            number: number(i, "number", what)?,
            title: text(i, "title", what)?.to_owned(),
            labels: labels(i, what)?,
        });
    }
    out.sort_by_key(|i| i.number);
    Ok(out)
}

enum Feed {
    Issue(u64),
    Comment { url: String, created_at: String },
}

fn parse_feed(text_: &str) -> Result<Vec<Feed>, String> {
    let what = "feed.jsonl";
    let mut out = Vec::new();
    for line in text_.lines().filter(|l| !l.trim().is_empty()) {
        let v = json::parse(line).map_err(|e| format!("{what}: {e}"))?;
        out.push(match text(&v, "kind", what)? {
            "issue" => match field(&v, "is_pr", what)? {
                // An owner PR is the owner's own work, not an instruction; owner comments on
                // it still come as comment lines.
                Value::Bool(true) => continue,
                Value::Bool(false) => Feed::Issue(number(&v, "number", what)?),
                _ => return Err(format!("{what}: field \"is_pr\" is not a boolean")),
            },
            "comment" | "review_comment" | "review" => Feed::Comment {
                url: text(&v, "url", what)?.to_owned(),
                created_at: text(&v, "created_at", what)?.to_owned(),
            },
            k => return Err(format!("{what}: unknown kind {k:?}")),
        });
    }
    Ok(out)
}

/// Picks the run with the largest id among completed runs not started by a pull request.
fn main_red(runs_text: &str) -> Result<Option<u64>, String> {
    let what = "runs.json";
    let v = parse_list(runs_text, what)?;
    let mut newest: Option<(u64, bool)> = None;
    for r in list(&v, what)? {
        let id = number(r, "databaseId", what)?;
        let status = text(r, "status", what)?;
        let event = text(r, "event", what)?;
        let conclusion = text(r, "conclusion", what)?;
        if status != "completed" || event == "pull_request" {
            continue;
        }
        if newest.is_none_or(|(n, _)| id > n) {
            let red = matches!(conclusion, "failure" | "timed_out" | "startup_failure");
            newest = Some((id, red));
        }
    }
    Ok(newest.and_then(|(id, red)| red.then_some(id)))
}

// ---------------------------------------------------------------------------------------------
// Decision

struct Pick {
    first: String,
    rule: String,
    load: Vec<String>,
}

fn pick(first: String, rule: &str, load: Vec<String>) -> Pick {
    Pick {
        first,
        rule: rule.to_owned(),
        load,
    }
}

fn view_issue(n: u64) -> String {
    format!("gh issue view {n} --comments")
}

/// (number, labels or none for owner-closed.json, tagged body, owner replies) of each entry.
type OwnerIssue = (u64, Vec<String>, bool, Vec<String>);

fn parse_owner(text_: &str, what: &str, closed: bool) -> Result<Vec<OwnerIssue>, String> {
    let v = parse_list(text_, what)?;
    let mut out = Vec::new();
    for i in list(&v, what)? {
        out.push((
            number(i, "number", what)?,
            if closed { Vec::new() } else { labels(i, what)? },
            closed && tagged(text(i, "body", what)?),
            owner_replies(i, what)?,
        ));
    }
    Ok(out)
}

/// The `FLOOR` value must be GitHub's `YYYY-MM-DDTHH:MM:SSZ` form, so text comparison works.
fn valid_floor(f: &str) -> bool {
    let b = f.as_bytes();
    b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

fn step1(inp: &Inputs, prs: &[Pr]) -> Result<Option<Pick>, String> {
    let feed = parse_feed(&inp.feed)?;
    if !valid_floor(&inp.floor) {
        return Err(format!("floor {:?} is not YYYY-MM-DDTHH:MM:SSZ", inp.floor));
    }
    let open = parse_owner(&inp.owner_open, "owner-open.json", false)?;
    let closed = parse_owner(&inp.owner_closed, "owner-closed.json", true)?;
    let in_flight_issue = |n: u64| {
        prs.iter()
            .any(|p| p.open && !p.failing && linked_issue(&p.body) == Some(n))
    };
    let mut issues: Vec<u64> = Vec::new();
    for item in &feed {
        if let Feed::Issue(n) = item {
            let url = format!("{ISSUE_URL}{n}");
            let (labels, replies) = open
                .iter()
                .find(|(m, ..)| m == n)
                .map(|(_, l, _, r)| (l.clone(), r.clone()))
                .unwrap_or_default();
            if !done_reply(&replies, &url) && !skipped(&labels) && !in_flight_issue(*n) {
                issues.push(*n);
            }
        }
    }
    if let Some(n) = issues.into_iter().min() {
        return Ok(Some(pick(
            format!("worker owner-issue #{n}"),
            "a (an owner issue without a done: reply; TICK §0 replies)",
            vec![
                view_issue(n),
                format!("gh pr list --state all --search '\"Closes #{n}\" in:body'"),
            ],
        )));
    }
    let mut comments: Vec<(&str, &str)> = feed
        .iter()
        .filter_map(|f| match f {
            Feed::Comment { url, created_at } => Some((created_at.as_str(), url.as_str())),
            Feed::Issue(_) => None,
        })
        .filter(|(at, url)| {
            *at >= inp.floor.as_str()
                && !prs
                    .iter()
                    .any(|p| p.open && !p.failing && names_url(&p.body, url))
        })
        .collect();
    comments.sort_unstable();
    if let Some((_, url)) = comments.first() {
        let id = url.rsplit(['-', 'r']).next().unwrap_or(url);
        return Ok(Some(pick(
            format!("worker owner-comment {url}"),
            "a (an owner comment without a done: reply; TICK §0 replies)",
            vec![
                (*url).to_owned(),
                format!("gh pr list --state all --search \"{id} in:body\""),
            ],
        )));
    }
    let mut missing = Vec::new();
    for (n, _, is_tagged, replies) in &closed {
        let url = format!("{ISSUE_URL}{n}");
        if !is_tagged && reply(replies, "picked up", &url) && !done_reply(replies, &url) {
            missing.push(*n);
        }
    }
    Ok(missing.into_iter().min().map(|n| {
        pick(
            format!("worker owner-done #{n}"),
            "a (a closed owner issue with a picked up reply and no done: reply)",
            vec![
                view_issue(n),
                format!("gh pr list --state all --search '\"Closes #{n}\" in:body'"),
            ],
        )
    }))
}

fn decide(inp: &Inputs) -> Result<String, String> {
    let prs = parse_prs(&inp.prs)?;
    let issues = parse_issues(&inp.issues)?;
    let red = main_red(&inp.runs)?;
    let in_flight = |n: u64| {
        prs.iter()
            .any(|p| p.open && linked_issue(&p.body) == Some(n))
    };
    let workable = |i: &&Issue| !skipped(&i.labels) && !in_flight(i.number);
    let labels_of = |n: u64| {
        issues
            .iter()
            .find(|i| i.number == n)
            .map(|i| i.labels.as_slice())
            .unwrap_or_default()
    };

    let mut chosen = step1(inp, &prs)?;
    if chosen.is_none() && !inp.alarm {
        chosen = red.map(|id| {
            pick(
                format!("worker main-red run {id}"),
                "b (main is red; infra failure → rerun, real failure → fix)",
                vec![
                    format!("gh run view {id} --log-failed"),
                    format!("gh run download {id} -n judge-report"),
                ],
            )
        });
    }
    if chosen.is_none() {
        chosen = prs
            .iter()
            .filter(|p| p.open && p.failing && !skipped(&p.labels))
            .find(|p| linked_issue(&p.body).is_none_or(|n| !skipped(labels_of(n))))
            .map(|p| {
                let mut load = vec![
                    format!("gh pr view {} --comments", p.number),
                    format!("gh pr checks {}", p.number),
                ];
                load.extend(linked_issue(&p.body).map(view_issue));
                pick(
                    format!("worker own-pr #{}", p.number),
                    "c (own open PR failing CI, in conflict with its base, or without ai-review success on its head)",
                    load,
                )
            });
    }
    if chosen.is_none() {
        chosen = issues
            .iter()
            .filter(workable)
            .find(|i| has_label(&i.labels, "bug") && i.title.starts_with("next-session: "))
            .map(|i| {
                pick(
                    format!("worker scheduler-bug #{}", i.number),
                    "c (the scheduler's own bug issue, ADR 0011 §2 Failure)",
                    vec![
                        view_issue(i.number),
                        "specs/tooling/next-session.md".to_owned(),
                        "xtask/src/next_session.rs".to_owned(),
                    ],
                )
            });
    }
    if chosen.is_none() {
        let rank = |i: &Issue| {
            let security = has_label(&i.labels, "security");
            let sev = |s: &str| has_label(&i.labels, s);
            if (security || has_label(&i.labels, "crash")) && sev("sev:critical") {
                Some(0)
            } else if security && !i.labels.iter().any(|l| l.starts_with("sev:")) {
                Some(1)
            } else if has_label(&i.labels, "crash") || (security && sev("sev:high")) {
                Some(2)
            } else {
                None
            }
        };
        chosen = issues
            .iter()
            .filter(workable)
            .filter_map(|i| rank(i).map(|r| (r, i.number)))
            .min()
            .map(|(_, n)| {
                pick(
                    format!("worker crash-security #{n}"),
                    "d (crash, or security sev:critical/sev:high or without a sev:* label)",
                    vec![
                        view_issue(n),
                        "docs/adr/0009-security-severity-and-release-gates.md".to_owned(),
                    ],
                )
            });
    }
    if chosen.is_none() {
        chosen = issues
            .iter()
            .filter(workable)
            .find(|i| has_label(&i.labels, "flaky"))
            .map(|i| {
                pick(
                    format!("worker flaky #{}", i.number),
                    "e (flaky test; fix within 5 sessions)",
                    vec![view_issue(i.number)],
                )
            });
    }
    let mut branches: Vec<&str> = inp
        .worktrees
        .lines()
        .filter_map(|l| l.strip_prefix("branch refs/heads/"))
        .filter(|b| b.starts_with("t/"))
        .collect();
    branches.sort_unstable();
    let mut notes = Vec::new();
    if inp.alarm {
        notes.push(format!(
            "judge alarm active ({ALARM_FILE} exists): step 2 skipped{}",
            if red.is_some() { ", main is red" } else { "" }
        ));
    }
    for b in &branches {
        let done: Vec<String> = prs
            .iter()
            .filter(|p| p.branch == *b)
            .map(|p| format!("#{}", p.number))
            .collect();
        if !done.is_empty() && !prs.iter().any(|p| p.branch == *b && p.open) {
            notes.push(format!(
                "worktree on {b}: its PRs ({}) are merged or closed; remove it",
                done.join(", ")
            ));
        }
    }
    if chosen.is_none() {
        chosen = branches
            .iter()
            .find(|b| !prs.iter().any(|p| p.branch == **b))
            .map(|b| {
                pick(
                    format!("worker leftover-worktree {b}"),
                    "f (a worktree whose branch has no PR: resume it if valuable, else remove it)",
                    vec![
                        format!("git log --oneline origin/main..{b}"),
                        "git worktree list".to_owned(),
                    ],
                )
            });
    }
    let p = chosen.unwrap_or_else(|| {
        pick(
            "legacy".to_owned(),
            "none of steps 1–5 applies; use TICK §2 rules g–j",
            vec!["ops/TICK.md §2 rules g–j".to_owned()],
        )
    });
    let mut out = format!("{}\n\nRule: {}\nLoad:\n", p.first, p.rule);
    for l in &p.load {
        let _ = writeln!(out, "- {l}");
    }
    if !notes.is_empty() {
        out.push_str("Notes:\n");
        for n in &notes {
            let _ = writeln!(out, "- {n}");
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_line() {
        let s = "# FLOOR=\"x\"\nOWNER=1\nFLOOR=\"2026-11-23T00:00:00Z\"\nFLOOR=\"later\"\n";
        assert_eq!(floor_of(s), Some("2026-11-23T00:00:00Z"));
        assert_eq!(floor_of("FLOOR=2026\n"), None);
        assert_eq!(floor_of("FLOOR=\"open\n"), None);
    }

    #[test]
    fn done_url_is_exact() {
        let u = "https://github.com/jacobdam/jpdf3/issues/12";
        let r = |s: &str| vec![s.to_owned()];
        assert!(done_reply(&r(&format!("{TAG} done: {u}")), u));
        assert!(done_reply(&r(&format!("{TAG} done: {u}\n- x")), u));
        assert!(!done_reply(&r(&format!("{TAG} done: {u}3")), u));
        assert!(!done_reply(
            &r(&format!("{TAG} done: {u}#issuecomment-1")),
            u
        ));
        assert!(!done_reply(&r(&format!(" {TAG} done: {u}")), u));
        assert!(!done_reply(&r(&format!("{TAG} picked up {u}")), u));
    }

    #[test]
    fn url_in_body_is_exact() {
        let u = "https://github.com/jacobdam/jpdf3/issues/1#issuecomment-123";
        assert!(names_url(&format!("Owner instruction: {u}"), u));
        assert!(names_url(&format!("{u}4 and {u}."), u));
        assert!(!names_url(&format!("{u}4"), u));
    }

    #[test]
    fn closes_link() {
        assert_eq!(linked_issue("x\nCloses #42.\nCloses #7"), Some(42));
        assert_eq!(linked_issue("closes #42"), None);
        assert_eq!(linked_issue("Closes #x"), None);
    }
}
