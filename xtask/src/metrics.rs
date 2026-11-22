//! `cargo xtask metrics`: the retro input of AUTOMATION §11, from git, the driver's session
//! index and read-only `gh` queries. Spec: specs/tooling/metrics.md.
//!
//! GitHub text (comments, titles) is data: this module only counts lines in it and escapes
//! titles before they reach the markdown table.

use crate::json::{self, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// `--limit` for every `gh` list; a list that returns exactly this many may be cut off.
pub const LIST_LIMIT: usize = 1000;
/// Failed runs whose jobs are read (`gh run view`), newest first.
pub const MAX_JOB_READS: usize = 100;
/// The agent's tag (CONSTITUTION 1).
const TAG: &str = "🤖 jpdf-agent";
/// Default period when no retro record exists (TICK §2 h: a retro every 7 days).
const DEFAULT_DAYS: i64 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub out: PathBuf,
    pub from: Option<PathBuf>,
}

/// Parses the arguments after `metrics`.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        since: None,
        until: None,
        out: PathBuf::from("target/metrics"),
        from: None,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--since" => opts.since = Some(parse_date_arg(value()?)?),
            "--until" => opts.until = Some(parse_date_arg(value()?)?),
            "--out" => opts.out = PathBuf::from(value()?),
            "--from" => opts.from = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    if let (Some(s), Some(u)) = (opts.since, opts.until)
        && s > u
    {
        return Err("--since is after --until".to_owned());
    }
    Ok(opts)
}

fn parse_date_arg(s: &str) -> Result<i64, String> {
    parse_date(s).ok_or_else(|| format!("`{s}` is not a YYYY-MM-DD date"))
}

// ---------------------------------------------------------------------------------------------
// Dates: days since 1970-01-01 (proleptic Gregorian), years 1..=9999 only.

/// Days since 1970-01-01 of a civil date; None outside years 1..=9999 or for a day that
/// doesn't exist.
fn days_from_civil(y: i64, m: i64, d: i64) -> Option<i64> {
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d < 1 || d > month_len(y, m) {
        return None;
    }
    // Bounded inputs: every intermediate stays far below i64::MAX.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9).rem_euclid(12);
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn month_len(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The civil date of a day number from `days_from_civil`.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

pub fn fmt_day(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `YYYY-MM-DD` exactly (ten ASCII characters).
fn parse_date(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let part = s.get(r)?;
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    days_from_civil(num(0..4)?, num(5..7)?, num(8..10)?)
}

/// The date part of an ISO 8601 timestamp (`2026-11-22T11:48:38Z`).
fn iso_day(s: &str) -> Option<i64> {
    parse_date(s.get(..10)?)
}

fn today() -> Result<i64, String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("system clock: {e}"))?
        .as_secs();
    i64::try_from(secs / 86_400).map_err(|_| "system clock out of range".to_owned())
}

// ---------------------------------------------------------------------------------------------
// git log

#[derive(Debug, Clone, PartialEq, Eq)]
struct Commit {
    day: i64,
    added: u64,
    deleted: u64,
    /// Paths this commit created (`--summary` "create mode" lines).
    creates: Vec<String>,
}

/// Lockfiles and generated data, left out of line counts (TICK §0 sizing rule).
fn excluded(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == "Cargo.lock" || name == "package-lock.json" || path.starts_with("tools/arlington/data/")
}

/// Parses `git log --first-parent --no-renames --format=@@%H%x09%cs%x09%s --numstat --summary`.
/// Returns commits oldest first.
fn parse_git_log(text: &str) -> Result<Vec<Commit>, String> {
    let mut commits: Vec<Commit> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let bad = || format!("git log line {}: unexpected `{line}`", i.saturating_add(1));
        if let Some(rest) = line.strip_prefix("@@") {
            let mut f = rest.splitn(3, '\t');
            let (_hash, date) = (f.next(), f.next());
            let day = date.and_then(parse_date).ok_or_else(bad)?;
            commits.push(Commit {
                day,
                added: 0,
                deleted: 0,
                creates: Vec::new(),
            });
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let commit = commits.last_mut().ok_or_else(bad)?;
        if let Some(summary) = line.strip_prefix(' ') {
            // " create mode 100644 <path>"; other summary lines (delete, mode change) are ignored.
            if let Some(rest) = summary.strip_prefix("create mode ") {
                let path = rest.split_once(' ').map(|(_, p)| p).ok_or_else(bad)?;
                commit.creates.push(path.to_owned());
            }
            continue;
        }
        let mut f = line.splitn(3, '\t');
        let (a, d, path) = match (f.next(), f.next(), f.next()) {
            (Some(a), Some(d), Some(p)) => (a, d, p),
            _ => return Err(bad()),
        };
        if a == "-" && d == "-" {
            continue; // binary file: no line counts
        }
        let (a, d): (u64, u64) = match (a.parse(), d.parse()) {
            (Ok(a), Ok(d)) => (a, d),
            _ => return Err(bad()),
        };
        if !excluded(path) {
            commit.added = commit.added.saturating_add(a);
            commit.deleted = commit.deleted.saturating_add(d);
        }
    }
    commits.reverse();
    Ok(commits)
}

/// `docs/<dir>/NNNN.md` with exactly four ASCII digits.
fn is_record(path: &str, dir: &str) -> bool {
    path.strip_prefix("docs/")
        .and_then(|p| p.strip_prefix(dir))
        .and_then(|p| p.strip_prefix('/'))
        .and_then(|p| p.strip_suffix(".md"))
        .is_some_and(|n| n.len() == 4 && n.bytes().all(|c| c.is_ascii_digit()))
}

/// (index in `commits`, day, record path) for every commit that created a record in `dir`.
fn records(commits: &[Commit], dir: &str) -> Vec<(usize, i64, String)> {
    let mut out = Vec::new();
    for (i, c) in commits.iter().enumerate() {
        for p in c.creates.iter().filter(|p| is_record(p, dir)) {
            out.push((i, c.day, p.clone()));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Sessions (ops/logs/index.tsv: stamp, exit code, session id, duration)

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Sessions {
    count: u64,
    nonzero: u64,
    total_s: u64,
    median_s: Option<u64>,
    malformed: u64,
}

fn parse_sessions(text: &str, since: i64, until: i64) -> Sessions {
    let mut s = Sessions::default();
    let mut durations = Vec::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        let row = match f.as_slice() {
            [stamp, code, _id, dur] => stamp_day(stamp).and_then(|day| {
                let code: i64 = code.parse().ok()?;
                let secs: u64 = dur.strip_suffix('s')?.parse().ok()?;
                Some((day, code, secs))
            }),
            _ => None,
        };
        let Some((day, code, secs)) = row else {
            s.malformed = s.malformed.saturating_add(1);
            continue;
        };
        if day < since || day > until {
            continue;
        }
        s.count = s.count.saturating_add(1);
        if code != 0 {
            s.nonzero = s.nonzero.saturating_add(1);
        }
        s.total_s = s.total_s.saturating_add(secs);
        durations.push(secs);
    }
    s.median_s = median(&mut durations);
    s
}

/// The day of a `YYYYMMDDTHHMMSSZ` stamp.
fn stamp_day(stamp: &str) -> Option<i64> {
    let b = stamp.as_bytes();
    if b.len() != 16 || b.get(8) != Some(&b'T') || b.get(15) != Some(&b'Z') {
        return None;
    }
    let digits = |r: std::ops::Range<usize>| {
        stamp
            .get(r)
            .filter(|p| p.bytes().all(|c| c.is_ascii_digit()))
    };
    digits(9..15)?;
    let d = digits(0..8)?;
    parse_date(&format!(
        "{}-{}-{}",
        d.get(0..4)?,
        d.get(4..6)?,
        d.get(6..8)?
    ))
}

/// The middle value (the upper one of the two middles for an even count).
fn median(v: &mut [u64]) -> Option<u64> {
    v.sort_unstable();
    v.get(v.len() / 2).copied()
}

// ---------------------------------------------------------------------------------------------
// JSON helpers

fn array<'a>(v: &'a Value, what: &str) -> Result<&'a [Value], String> {
    v.as_array()
        .ok_or_else(|| format!("{what}: not a JSON array"))
}

fn num(v: &Value, key: &str, what: &str) -> Result<u64, String> {
    match v.get(key) {
        Some(Value::Number(n)) => n
            .parse()
            .map_err(|_| format!("{what}: `{key}` is not a count")),
        _ => Err(format!("{what}: missing number `{key}`")),
    }
}

fn string<'a>(v: &'a Value, key: &str, what: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{what}: missing string `{key}`"))
}

fn day_of(v: &Value, key: &str, what: &str) -> Result<i64, String> {
    let s = string(v, key, what)?;
    iso_day(s).ok_or_else(|| format!("{what}: `{key}` is not a timestamp"))
}

fn label_names(v: &Value) -> Vec<&str> {
    v.get("labels")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|l| l.get("name").and_then(Value::as_str))
        .collect()
}

fn parse_list(text: &str, what: &str) -> Result<Value, String> {
    json::parse(text).map_err(|e| format!("{what}: {e}"))
}

// ---------------------------------------------------------------------------------------------
// Reviews (TICK §4, ADR 0010)

const SEVERITIES: [&str; 4] = ["blocker", "major", "minor", "nit"];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Review {
    pr: u64,
    rounds: u64,
    final_check: bool,
    /// Findings by severity (index into SEVERITIES) for the subagent (R) and second reviewer (S).
    r: [u64; 4],
    s: [u64; 4],
    regrades: u64,
}

enum Header {
    Review { k: u64, final_check: bool },
    Second,
    Triage,
}

/// The review header on the first line of a tagged comment.
fn header(body: &str) -> Option<Header> {
    let first = body.trim_start().strip_prefix(TAG)?.lines().next()?;
    if first.contains("**AI review triage, round ") {
        return Some(Header::Triage);
    }
    if first.contains("**AI review (second reviewer), round ") {
        return Some(Header::Second);
    }
    let rest = first.split_once("**AI review, round ")?.1;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let k = digits.parse().ok()?;
    let final_check = rest
        .get(digits.len()..)
        .is_some_and(|r| r.starts_with(" (final check)"));
    Some(Header::Review { k, final_check })
}

/// `<R|S><K>.<n>` at the start of `line`; returns (ID, rest of the line).
fn finding_id(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix('R').or_else(|| line.strip_prefix('S'))?;
    let k_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    let after_k = rest.get(k_len..)?.strip_prefix('.')?;
    let n_len = after_k.bytes().take_while(u8::is_ascii_digit).count();
    if k_len == 0 || n_len == 0 {
        return None;
    }
    let id_len = 1usize
        .saturating_add(k_len)
        .saturating_add(1)
        .saturating_add(n_len);
    Some((line.get(..id_len)?, line.get(id_len..)?))
}

/// `R<K>.<n> [<severity>]` at the start of a line: (ID, severity index).
fn finding(line: &str) -> Option<(&str, usize)> {
    let (id, rest) = finding_id(line)?;
    let sev = rest.strip_prefix(" [")?.split_once(']')?.0;
    Some((id, SEVERITIES.iter().position(|s| *s == sev)?))
}

fn review_of(pr: u64, comments: &[Value]) -> Option<Review> {
    let mut rev = Review {
        pr,
        ..Review::default()
    };
    let mut seen_review = false;
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut regraded: BTreeSet<String> = BTreeSet::new();
    for body in comments
        .iter()
        .filter_map(|c| c.get("body").and_then(Value::as_str))
    {
        let scan_findings = match header(body) {
            Some(Header::Review { k, final_check }) => {
                seen_review = true;
                rev.rounds = rev.rounds.max(k);
                rev.final_check |= final_check;
                true
            }
            Some(Header::Second) => {
                seen_review = true;
                true
            }
            Some(Header::Triage) => {
                for line in body.lines() {
                    if let Some((id, rest)) = finding_id(line)
                        && rest.starts_with(": regraded")
                    {
                        regraded.insert(id.to_owned());
                    }
                }
                false
            }
            None => false,
        };
        if scan_findings {
            for (id, sev) in body.lines().filter_map(finding) {
                ids.entry(id.to_owned()).or_insert(sev);
            }
        }
    }
    if !seen_review {
        return None;
    }
    for (id, sev) in &ids {
        let slot = if id.starts_with('S') {
            &mut rev.s
        } else {
            &mut rev.r
        };
        if let Some(n) = slot.get_mut(*sev) {
            *n = n.saturating_add(1);
        }
    }
    rev.regrades = u64::try_from(regraded.len()).unwrap_or(u64::MAX);
    Some(rev)
}

// ---------------------------------------------------------------------------------------------
// The report

#[derive(Debug, Clone, PartialEq, Eq)]
struct Issue {
    number: u64,
    title: String,
    age_days: i64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Ci {
    runs: u64,
    by_conclusion: BTreeMap<String, u64>,
    failed_main: u64,
    failed_other: u64,
    failing_jobs: BTreeMap<String, u64>,
    jobs_read: u64,
    jobs_unknown: u64,
    jobs_capped: bool,
    reruns: u64,
    reruns_green: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Report {
    since: i64,
    until: i64,
    merges: u64,
    added: u64,
    deleted: u64,
    merged_prs: u64,
    closed_issues: u64,
    sessions: Option<Sessions>,
    reviews: Vec<Review>,
    ci: Ci,
    blocked: Vec<Issue>,
    flaky: Vec<Issue>,
    /// (record, day, merges since the previous record; None for the first)
    audits: Vec<(String, i64, Option<u64>)>,
    merges_since_audit: Option<u64>,
    /// (record, day, days since the previous record)
    retros: Vec<(String, i64, Option<i64>)>,
    days_since_retro: Option<i64>,
    truncated: Vec<&'static str>,
}

/// Every input, as fetched (or read with `--from`).
#[derive(Debug, Default, Clone)]
pub struct Inputs {
    git_log: String,
    index_tsv: Option<String>,
    prs: String,
    issues_closed: String,
    issues_open: String,
    runs: String,
    jobs: BTreeMap<u64, String>,
}

/// The default `--since`: the day the last retro record was added, else `until` − 7 days; a
/// default after `until` also falls back to `until` − 7 days.
fn default_since(commits: &[Commit], until: i64) -> i64 {
    records(commits, "retros")
        .last()
        .map(|(_, day, _)| *day)
        .filter(|d| *d <= until)
        .unwrap_or(until.saturating_sub(DEFAULT_DAYS))
}

/// Ids of the failed runs whose jobs are read: the newest `MAX_JOB_READS`.
fn job_run_ids(runs_json: &str, since: i64, until: i64) -> Result<(Vec<u64>, bool), String> {
    let runs = parse_list(runs_json, "runs.json")?;
    let mut failed = Vec::new();
    for r in array(&runs, "runs.json")? {
        let day = day_of(r, "createdAt", "runs.json")?;
        if day < since || day > until || string(r, "conclusion", "runs.json")? != "failure" {
            continue;
        }
        let created = string(r, "createdAt", "runs.json")?.to_owned();
        failed.push((created, num(r, "databaseId", "runs.json")?));
    }
    failed.sort_unstable_by(|a, b| b.cmp(a));
    let capped = failed.len() > MAX_JOB_READS;
    Ok((
        failed
            .into_iter()
            .take(MAX_JOB_READS)
            .map(|(_, id)| id)
            .collect(),
        capped,
    ))
}

fn compute(inp: &Inputs, commits: &[Commit], since: i64, until: i64) -> Result<Report, String> {
    let mut rep = Report {
        since,
        until,
        ..Report::default()
    };
    let in_period = |d: i64| d >= since && d <= until;

    for c in commits.iter().filter(|c| in_period(c.day)) {
        rep.merges = rep.merges.saturating_add(1);
        rep.added = rep.added.saturating_add(c.added);
        rep.deleted = rep.deleted.saturating_add(c.deleted);
    }
    rep.sessions = inp
        .index_tsv
        .as_deref()
        .map(|t| parse_sessions(t, since, until));

    // Merged PRs and their reviews.
    let prs = parse_list(&inp.prs, "prs.json")?;
    let prs = array(&prs, "prs.json")?;
    if prs.len() >= LIST_LIMIT {
        rep.truncated.push("merged PRs");
    }
    for pr in prs {
        let number = num(pr, "number", "prs.json")?;
        if !in_period(day_of(pr, "mergedAt", "prs.json")?) {
            continue;
        }
        rep.merged_prs = rep.merged_prs.saturating_add(1);
        let comments = pr
            .get("comments")
            .and_then(Value::as_array)
            .unwrap_or_default();
        rep.reviews.extend(review_of(number, comments));
    }
    rep.reviews.sort_unstable_by_key(|r| r.pr);

    // Issues.
    let closed = parse_list(&inp.issues_closed, "issues-closed.json")?;
    let closed = array(&closed, "issues-closed.json")?;
    if closed.len() >= LIST_LIMIT {
        rep.truncated.push("closed issues");
    }
    for i in closed {
        num(i, "number", "issues-closed.json")?;
        if in_period(day_of(i, "closedAt", "issues-closed.json")?) {
            rep.closed_issues = rep.closed_issues.saturating_add(1);
        }
    }
    let open = parse_list(&inp.issues_open, "issues-open.json")?;
    let open = array(&open, "issues-open.json")?;
    if open.len() >= LIST_LIMIT {
        rep.truncated.push("open issues");
    }
    for i in open {
        let issue = Issue {
            number: num(i, "number", "issues-open.json")?,
            title: i
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            age_days: until.saturating_sub(day_of(i, "createdAt", "issues-open.json")?),
        };
        let labels = label_names(i);
        if labels.contains(&"flaky") {
            rep.flaky.push(issue.clone());
        }
        if labels.contains(&"blocked") {
            rep.blocked.push(issue);
        }
    }

    // CI runs.
    let runs = parse_list(&inp.runs, "runs.json")?;
    let runs = array(&runs, "runs.json")?;
    if runs.len() >= LIST_LIMIT {
        rep.truncated.push("CI runs");
    }
    for r in runs {
        let what = "runs.json";
        num(r, "databaseId", what)?;
        if !in_period(day_of(r, "createdAt", what)?) {
            continue;
        }
        let conclusion = string(r, "conclusion", what)?;
        let attempt = num(r, "attempt", what)?;
        let branch = string(r, "headBranch", what)?;
        let ci = &mut rep.ci;
        ci.runs = ci.runs.saturating_add(1);
        let key = if conclusion.is_empty() {
            "in progress"
        } else {
            conclusion
        };
        let n = ci.by_conclusion.entry(key.to_owned()).or_insert(0);
        *n = n.saturating_add(1);
        if conclusion == "failure" {
            if branch == "main" {
                ci.failed_main = ci.failed_main.saturating_add(1);
            } else {
                ci.failed_other = ci.failed_other.saturating_add(1);
            }
        }
        if attempt > 1 {
            ci.reruns = ci.reruns.saturating_add(1);
            if conclusion == "success" {
                ci.reruns_green = ci.reruns_green.saturating_add(1);
            }
        }
    }
    let (ids, capped) = job_run_ids(&inp.runs, since, until)?;
    rep.ci.jobs_capped = capped;
    for id in ids {
        let Some(text) = inp.jobs.get(&id) else {
            rep.ci.jobs_unknown = rep.ci.jobs_unknown.saturating_add(1);
            continue;
        };
        let what = format!("jobs-{id}.json");
        let v = parse_list(text, &what)?;
        let jobs = v
            .get("jobs")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{what}: missing array `jobs`"))?;
        rep.ci.jobs_read = rep.ci.jobs_read.saturating_add(1);
        for j in jobs {
            if j.get("conclusion").and_then(Value::as_str) == Some("failure") {
                let name = string(j, "name", &what)?;
                let n = rep.ci.failing_jobs.entry(name.to_owned()).or_insert(0);
                *n = n.saturating_add(1);
            }
        }
    }

    // Audit and retro lag (all history).
    let audits = records(commits, "audits");
    let mut prev: Option<usize> = None;
    for (i, day, path) in &audits {
        let since_prev = prev.map(|p| u64::try_from(i.saturating_sub(p)).unwrap_or(u64::MAX));
        rep.audits.push((path.clone(), *day, since_prev));
        prev = Some(*i);
    }
    rep.merges_since_audit = prev.map(|p| {
        u64::try_from(commits.len().saturating_sub(p).saturating_sub(1)).unwrap_or(u64::MAX)
    });
    let mut prev_day: Option<i64> = None;
    for (_, day, path) in records(commits, "retros") {
        rep.retros
            .push((path, day, prev_day.map(|p| day.saturating_sub(p))));
        prev_day = Some(day);
    }
    rep.days_since_retro = prev_day.map(|d| until.saturating_sub(d));
    Ok(rep)
}

// ---------------------------------------------------------------------------------------------
// Rendering

/// Text from GitHub, made safe for one markdown table cell.
fn cell(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '|' => '/',
            c if c.is_control() || c == '\u{2028}' || c == '\u{2029}' => ' ',
            c => c,
        })
        .collect()
}

fn share(x: u64, of: u64) -> String {
    match x.saturating_mul(100).checked_div(of) {
        Some(p) => format!("{x}/{of} ({p}%)"),
        None => format!("{x}/0"),
    }
}

fn sev_counts(v: &[u64; 4]) -> String {
    v.iter().map(u64::to_string).collect::<Vec<_>>().join("/")
}

fn render_md(rep: &Report) -> String {
    let mut o = String::new();
    let _ = writeln!(
        o,
        "# Metrics {} to {}\n",
        fmt_day(rep.since),
        fmt_day(rep.until)
    );
    let _ = writeln!(
        o,
        "Generated by `cargo xtask metrics` (specs/tooling/metrics.md).\n"
    );
    for t in &rep.truncated {
        let _ = writeln!(
            o,
            "- **Warning:** the {t} list returned {LIST_LIMIT} entries and may be cut off."
        );
    }
    if !rep.truncated.is_empty() {
        o.push('\n');
    }

    o.push_str("## Throughput\n\n| Metric | Value |\n|---|---|\n");
    let _ = writeln!(o, "| Merges on main | {} |", rep.merges);
    let _ = writeln!(
        o,
        "| Lines added / deleted (no lockfiles, no generated data) | {} / {} |",
        rep.added, rep.deleted
    );
    let _ = writeln!(o, "| Merged PRs | {} |", rep.merged_prs);
    let _ = writeln!(o, "| Closed issues | {} |", rep.closed_issues);
    match &rep.sessions {
        Some(s) => {
            let _ = writeln!(
                o,
                "| Sessions (non-zero exit) | {} ({}) |",
                s.count, s.nonzero
            );
            let _ = writeln!(o, "| Session time, total | {} min |", s.total_s / 60);
            let median = s
                .median_s
                .map_or_else(|| "n/a".to_owned(), |m| format!("{} min", m / 60));
            let _ = writeln!(o, "| Session time, median | {median} |");
            let per_pr = (s.total_s / 6).checked_div(rep.merged_prs).map_or_else(
                || "n/a".to_owned(),
                |t| format!("{}.{} min", t / 10, t % 10),
            );
            let _ = writeln!(o, "| Session time per merged PR | {per_pr} |");
            if s.malformed > 0 {
                let _ = writeln!(
                    o,
                    "| Malformed index.tsv rows (skipped) | {} |",
                    s.malformed
                );
            }
        }
        None => o.push_str("| Sessions | no ops/logs/index.tsv |\n"),
    }

    o.push_str("\n## Review (ADR 0010)\n\n");
    let n = u64::try_from(rep.reviews.len()).unwrap_or(u64::MAX);
    let mut rounds: Vec<u64> = rep.reviews.iter().map(|r| r.rounds).collect();
    let median = median(&mut rounds).map_or_else(|| "n/a".to_owned(), |m| m.to_string());
    let capped = rep.reviews.iter().filter(|r| r.rounds >= 4).count();
    let finals = rep.reviews.iter().filter(|r| r.final_check).count();
    let count = |c: usize| u64::try_from(c).unwrap_or(u64::MAX);
    let (mut r, mut s, mut regrades) = ([0u64; 4], [0u64; 4], 0u64);
    for rev in &rep.reviews {
        for i in 0..4 {
            if let (Some(t), Some(v)) = (r.get_mut(i), rev.r.get(i)) {
                *t = t.saturating_add(*v);
            }
            if let (Some(t), Some(v)) = (s.get_mut(i), rev.s.get(i)) {
                *t = t.saturating_add(*v);
            }
        }
        regrades = regrades.saturating_add(rev.regrades);
    }
    let _ = writeln!(
        o,
        "PRs reviewed: {n}; median rounds: {median}; at 4+ rounds: {}; with a final check: {}.",
        share(count(capped), n),
        share(count(finals), n)
    );
    let _ = writeln!(
        o,
        "Findings blocker/major/minor/nit: subagent {}, second reviewer {}; regrades: {regrades}.\n",
        sev_counts(&r),
        sev_counts(&s)
    );
    if !rep.reviews.is_empty() {
        o.push_str("| PR | Rounds | Final check | Subagent B/M/m/n | Second B/M/m/n | Regrades |\n|---|---|---|---|---|---|\n");
        for rev in &rep.reviews {
            let _ = writeln!(
                o,
                "| #{} | {} | {} | {} | {} | {} |",
                rev.pr,
                rev.rounds,
                if rev.final_check { "yes" } else { "no" },
                sev_counts(&rev.r),
                sev_counts(&rev.s),
                rev.regrades
            );
        }
    }

    let ci = &rep.ci;
    o.push_str("\n## CI (`ci` workflow)\n\n");
    let by: Vec<String> = ci
        .by_conclusion
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    let _ = writeln!(
        o,
        "Runs: {} ({}).",
        ci.runs,
        if by.is_empty() {
            "none".to_owned()
        } else {
            by.join(", ")
        }
    );
    let _ = writeln!(
        o,
        "Failed runs: main {}, other branches {}.",
        ci.failed_main, ci.failed_other
    );
    let _ = writeln!(
        o,
        "Re-run (attempt > 1): {}, of which success: {}. Infra vs real failures are not classified (`gh run list` shows the latest attempt only).",
        ci.reruns, ci.reruns_green
    );
    let mut jobs: Vec<(&String, &u64)> = ci.failing_jobs.iter().collect();
    jobs.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let _ = write!(o, "Failing jobs over {} failed runs read", ci.jobs_read);
    if ci.jobs_unknown > 0 {
        let _ = write!(o, " ({} without job data)", ci.jobs_unknown);
    }
    if ci.jobs_capped {
        let _ = write!(o, " (only the {MAX_JOB_READS} newest failed runs)");
    }
    o.push_str(":\n\n");
    if jobs.is_empty() {
        o.push_str("none\n");
    } else {
        o.push_str("| Job | Failed runs |\n|---|---|\n");
        for (name, n) in jobs {
            let _ = writeln!(o, "| {} | {n} |", cell(name));
        }
    }

    o.push_str("\n## Stuck (open issues; age from creation to the period's end)\n\n");
    for (label, list) in [("blocked", &rep.blocked), ("flaky", &rep.flaky)] {
        let _ = writeln!(o, "`{label}`: {}", list.len());
        for i in list {
            let _ = writeln!(
                o,
                "- #{} ({} days): {}",
                i.number,
                i.age_days,
                cell(&i.title)
            );
        }
        o.push('\n');
    }

    o.push_str("## Audit and retro lag (all history)\n\n");
    for (path, day, prev) in &rep.audits {
        let prev = prev.map_or_else(
            || "first".to_owned(),
            |m| format!("{m} merges after the previous one"),
        );
        let _ = writeln!(o, "- {path}: {} ({prev})", fmt_day(*day));
    }
    let since_audit = rep
        .merges_since_audit
        .map_or_else(|| "no audit record".to_owned(), |m| m.to_string());
    let _ = writeln!(o, "\nMerges since the last audit record: {since_audit}.\n");
    for (path, day, prev) in &rep.retros {
        let prev = prev.map_or_else(
            || "first".to_owned(),
            |d| format!("{d} days after the previous one"),
        );
        let _ = writeln!(o, "- {path}: {} ({prev})", fmt_day(*day));
    }
    let since_retro = rep
        .days_since_retro
        .map_or_else(|| "no retro record".to_owned(), |d| d.to_string());
    let _ = writeln!(
        o,
        "\nDays from the last retro record to {}: {since_retro}.",
        fmt_day(rep.until)
    );
    o
}

// ---------------------------------------------------------------------------------------------
// Running

const GIT_LOG_ARGS: &[&str] = &[
    "log",
    "--first-parent",
    "--no-renames",
    "--format=@@%H%x09%cs%x09%s",
    "--numstat",
    "--summary",
    "origin/main",
];

fn read(dir: &Path, name: &str) -> Result<String, String> {
    let p = dir.join(name);
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}

fn read_optional(dir: &Path, name: &str) -> Result<Option<String>, String> {
    let p = dir.join(name);
    match fs::read_to_string(&p) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", p.display())),
    }
}

fn gh(args: &str) -> Result<String, String> {
    let args: Vec<&str> = args.split_whitespace().collect();
    crate::macos::capture("gh", &args, None).map(|(out, _)| out)
}

/// Runs the report; returns the markdown (also written to `out/metrics.md`).
pub fn run(repo: &Path, opts: &Options) -> Result<String, String> {
    let until = match opts.until {
        Some(u) => u,
        None => today()?,
    };
    let from = opts.from.as_deref();
    let git_log = match from {
        Some(dir) => read(dir, "git-log.txt")?,
        None => {
            crate::macos::capture(
                "git",
                &[&["-C", &repo.to_string_lossy()], GIT_LOG_ARGS].concat(),
                None,
            )?
            .0
        }
    };
    let commits = parse_git_log(&git_log)?;
    let since = opts.since.unwrap_or_else(|| default_since(&commits, until));
    let range = format!("{}..{}", fmt_day(since), fmt_day(until));
    // (raw file, gh arguments); the arguments hold no spaces, so split_whitespace is exact.
    let lists = [
        (
            "prs.json",
            format!(
                "pr list --state merged --search merged:{range} --limit {LIST_LIMIT} --json number,title,mergedAt,comments"
            ),
        ),
        (
            "issues-closed.json",
            format!(
                "issue list --state closed --search closed:{range} --limit {LIST_LIMIT} --json number,title,closedAt,labels"
            ),
        ),
        (
            "issues-open.json",
            format!(
                "issue list --state open --limit {LIST_LIMIT} --json number,title,createdAt,labels"
            ),
        ),
        (
            "runs.json",
            format!(
                "run list --workflow ci.yml --created {range} --limit {LIST_LIMIT} --json databaseId,conclusion,event,headBranch,attempt,createdAt"
            ),
        ),
    ];
    let mut texts = Vec::new();
    for (file, args) in &lists {
        texts.push(match from {
            Some(dir) => read(dir, file)?,
            None => gh(args)?,
        });
    }
    let mut it = texts.clone().into_iter();
    let mut inp = Inputs {
        git_log,
        index_tsv: match from {
            Some(dir) => read_optional(dir, "index.tsv")?,
            None => read_optional(&repo.join("ops/logs"), "index.tsv")?,
        },
        prs: it.next().unwrap_or_default(),
        issues_closed: it.next().unwrap_or_default(),
        issues_open: it.next().unwrap_or_default(),
        runs: it.next().unwrap_or_default(),
        jobs: BTreeMap::new(),
    };
    let (ids, _) = job_run_ids(&inp.runs, since, until)?;
    for id in ids {
        let name = format!("jobs-{id}.json");
        let text = match from {
            Some(dir) => read_optional(dir, &name)?,
            None => Some(gh(&format!("run view {id} --json jobs"))?),
        };
        if let Some(t) = text {
            inp.jobs.insert(id, t);
        }
    }
    let rep = compute(&inp, &commits, since, until)?;
    let md = render_md(&rep);
    let out = &opts.out;
    let write = |name: &str, text: &str| {
        let p = out.join(name);
        fs::write(&p, text).map_err(|e| format!("write {}: {e}", p.display()))
    };
    if from.is_none() {
        let raw = out.join("raw");
        fs::create_dir_all(&raw).map_err(|e| format!("create {}: {e}", raw.display()))?;
        write("raw/git-log.txt", &inp.git_log)?;
        if let Some(t) = &inp.index_tsv {
            write("raw/index.tsv", t)?;
        }
        for ((file, _), text) in lists.iter().zip(&texts) {
            write(&format!("raw/{file}"), text)?;
        }
        for (id, t) in &inp.jobs {
            write(&format!("raw/jobs-{id}.json"), t)?;
        }
    } else {
        fs::create_dir_all(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    }
    write("metrics.md", &md)?;
    Ok(md)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> i64 {
        parse_date(s).unwrap()
    }

    #[test]
    fn dates_round_trip_and_reject_impossible_days() {
        assert_eq!(d("1970-01-01"), 0);
        assert_eq!(d("2026-11-22") - d("2026-11-15"), 7);
        assert_eq!(d("2024-03-01") - d("2024-02-28"), 2); // leap year
        assert_eq!(d("2100-03-01") - d("2100-02-28"), 1); // century, not leap
        assert_eq!(d("2000-03-01") - d("2000-02-28"), 2); // 400-year leap
        for s in [
            "0001-01-01",
            "1969-12-31",
            "2026-11-22",
            "2024-02-29",
            "9999-12-31",
        ] {
            assert_eq!(fmt_day(d(s)), s);
        }
        for bad in [
            "2026-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "0000-01-01",
            "2026-1-01",
            "2026-11-2x",
            "2026/11/22",
            "+026-11-22",
            "2026-11-22 ",
            "",
            "2026-11-+2",
        ] {
            assert_eq!(parse_date(bad), None, "{bad}");
        }
        assert_eq!(iso_day("2026-11-22T11:48:38Z"), Some(d("2026-11-22")));
        assert_eq!(iso_day("2026-11"), None);
    }

    #[test]
    fn options_parse() {
        let a = |v: &[&str]| parse(&v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
        let o = a(&[
            "--since",
            "2026-11-15",
            "--until",
            "2026-11-22",
            "--out",
            "x",
            "--from",
            "y",
        ])
        .unwrap();
        assert_eq!(o.since, Some(d("2026-11-15")));
        assert_eq!(o.until, Some(d("2026-11-22")));
        assert_eq!(
            (o.out, o.from),
            (PathBuf::from("x"), Some(PathBuf::from("y")))
        );
        assert_eq!(a(&[]).unwrap().out, PathBuf::from("target/metrics"));
        assert!(a(&["--since", "2026-11-22", "--until", "2026-11-22"]).is_ok());
        assert!(
            a(&["--since", "2026-11-23", "--until", "2026-11-22"])
                .unwrap_err()
                .contains("after")
        );
        assert!(a(&["--since"]).unwrap_err().contains("needs a value"));
        assert!(a(&["--since", "2026-02-30"]).is_err());
        assert!(a(&["--bogus"]).unwrap_err().contains("unknown"));
    }

    const LOG: &str = "@@c3\t2026-11-22\tthird\n\
        5\t1\tsrc/a.rs\n\
        100\t100\tCargo.lock\n\
        7\t0\tapps/web/package-lock.json\n\
        9\t9\ttools/arlington/data/x.tsv\n\
        -\t-\tlogo.png\n\
        \x20create mode 100644 docs/retros/0002.md\n\
        \n\
        @@c2\t2026-11-20\tsecond\twith tab\n\
        2\t0\tdocs/audits/0002.md\n\
        \x20create mode 100644 docs/audits/0002.md\n\
        \x20create mode 100644 docs/audits/0002-extra.md\n\
        \x20mode change 100644 => 100755 x.sh\n\
        @@c1\t2026-11-14\tfirst\n\
        \x20create mode 100644 docs/audits/0001.md\n\
        \x20create mode 100644 docs/retros/0001.md\n\
        \x20create mode 100644 docs/retros/0000-pilot.md\n\
        @@c0\t2026-11-13\tzero\n";

    #[test]
    fn git_log_counts_lines_without_lockfiles_and_finds_records() {
        let c = parse_git_log(LOG).unwrap();
        assert_eq!(c.len(), 4);
        assert_eq!((c[0].day, c[3].day), (d("2026-11-13"), d("2026-11-22")));
        assert_eq!((c[3].added, c[3].deleted), (5, 1));
        assert_eq!((c[2].added, c[2].deleted), (2, 0));
        let audits: Vec<_> = records(&c, "audits")
            .into_iter()
            .map(|(i, _, p)| (i, p))
            .collect();
        assert_eq!(
            audits,
            [
                (1, "docs/audits/0001.md".to_owned()),
                (2, "docs/audits/0002.md".to_owned())
            ]
        );
        assert_eq!(records(&c, "retros").len(), 2); // 0000-pilot.md is not a record
        assert_eq!(default_since(&c, d("2026-11-22")), d("2026-11-22"));
        assert_eq!(default_since(&c, d("2026-11-21")), d("2026-11-14")); // newest retro after until: skipped
        assert_eq!(default_since(&[], d("2026-11-22")), d("2026-11-15"));
        for bad in [
            "x\t1\tsrc/a.rs\n",
            "@@c\tnot-a-date\ts\n",
            "@@c\t2026-11-22\ts\n1\tsrc\n",
            "@@c\t2026-11-22\ts\nx\t1\tp\n",
        ] {
            assert!(parse_git_log(bad).is_err(), "{bad:?}");
        }
        assert!(
            excluded("Cargo.lock") && excluded("fuzz/Cargo.lock") && !excluded("Cargo.lock.md")
        );
    }

    #[test]
    fn sessions_window_and_malformed_rows() {
        let tsv = "20261114T235959Z\t0\ta\t60s\n\
            20261115T000000Z\t0\tb\t120s\n\
            20261122T235959Z\t1\tc\t600s\n\
            20261123T000000Z\t0\td\t60s\n\
            20261116T000000Z\t0\te\n\
            20261116T000000Z\tx\te\t5s\n\
            20261116T000000Z\t0\te\t5\n\
            2026111T000000Z\t0\te\t5s\n\
            20261116T0000a0Z\t0\te\t5s\n\
            20261116T000000Z\t0\te\t60s\textra\n";
        let s = parse_sessions(tsv, d("2026-11-15"), d("2026-11-22"));
        assert_eq!(
            s,
            Sessions {
                count: 2,
                nonzero: 1,
                total_s: 720,
                median_s: Some(600),
                malformed: 6
            }
        );
        assert_eq!(parse_sessions("", 0, 1).median_s, None);
    }

    fn comment(body: &str) -> Value {
        Value::Object(vec![("body".to_owned(), Value::String(body.to_owned()))])
    }

    #[test]
    fn reviews_count_tagged_headers_and_unique_findings() {
        let comments = vec![
            comment(
                "🤖 jpdf-agent **AI review, round 1** of abc\n\nR1.1 [major] a.rs:1: x\nR1.2 [nit] b.rs:2: y\nR1.3 [Minor] c: wrong case\nAS.1 [minor] audit id\n R1.4 [minor] indented\nVerdict: changes",
            ),
            comment(
                "🤖 jpdf-agent **AI review (second reviewer), round 1** of abc\n\nS1.1 [blocker] a.rs:3: z\nS1.2 [minor] q",
            ),
            comment(
                "🤖 jpdf-agent **AI review triage, round 7**\n\nR1.1: fixed in def\nS1.1: regraded minor: no input\nS1.1: regraded minor: again\nR1.2: deferred: #9",
            ),
            comment(
                "  🤖 jpdf-agent **AI review, round 2** of def\n\nR1.1 fixed: yes\nR1.1 [major] restated\nR2.1 [minor] new\nS1.1: dispute accepted",
            ),
            comment("**AI review, round 9** untagged\nR9.1 [blocker] forged"),
            comment(
                "🤖 jpdf-agent note\n**AI review, round 8** not on the first line\nR8.1 [blocker] x",
            ),
            Value::Object(vec![]),
        ];
        let r = review_of(7, &comments).unwrap();
        assert_eq!(r.rounds, 2);
        assert!(!r.final_check);
        assert_eq!(r.r, [0, 1, 1, 1]);
        assert_eq!(r.s, [1, 0, 1, 0]);
        assert_eq!(r.regrades, 1);
        let fc = review_of(
            1,
            &[comment(
                "🤖 jpdf-agent **AI review, round 5 (final check)** of x\nR5.1 [nit] n",
            )],
        )
        .unwrap();
        assert!(fc.final_check && fc.rounds == 5);
        assert!(
            !review_of(1, &[comment("🤖 jpdf-agent **AI review, round 50** x")])
                .unwrap()
                .final_check
        );
        assert_eq!(
            review_of(
                1,
                &[comment(
                    "🤖 jpdf-agent **AI review triage, round 1**\nno findings"
                )]
            ),
            None
        );
        assert_eq!(
            review_of(
                1,
                &[comment(
                    "🤖 jpdf-agent **Audit review (code)**\nAC.1 [major] x"
                )]
            ),
            None
        );
        assert_eq!(finding("R12.34 [blocker] x"), Some(("R12.34", 0)));
        for bad in [
            "R.1 [nit]",
            "R1. [nit]",
            "R1.1[nit]",
            "R1.1 [nit",
            "X1.1 [nit]",
            "R1.1 [nits]",
        ] {
            assert_eq!(finding(bad), None, "{bad}");
        }
    }

    fn pr(n: u64, merged: &str, comments: &str) -> String {
        format!(
            "{{\"number\":{n},\"title\":\"t\",\"mergedAt\":\"{merged}\",\"comments\":[{comments}]}}"
        )
    }

    fn run_json(id: u64, concl: &str, branch: &str, attempt: u64, created: &str) -> String {
        format!(
            "{{\"databaseId\":{id},\"conclusion\":\"{concl}\",\"event\":\"push\",\"headBranch\":\"{branch}\",\"attempt\":{attempt},\"createdAt\":\"{created}\"}}"
        )
    }

    fn inputs() -> Inputs {
        let review = "{\"body\":\"🤖 jpdf-agent **AI review, round 3** of x\\nR3.1 [major] y\"}";
        Inputs {
            git_log: String::new(),
            index_tsv: None,
            prs: format!(
                "[{},{},{},{}]",
                pr(1, "2026-11-15T00:00:00Z", review),
                pr(2, "2026-11-22T23:59:59Z", ""),
                pr(3, "2026-11-14T23:59:59Z", review),
                pr(4, "2026-11-23T00:00:00Z", review)
            ),
            issues_closed: "[{\"number\":1,\"closedAt\":\"2026-11-15T01:00:00Z\"},{\"number\":2,\"closedAt\":\"2026-11-14T01:00:00Z\"}]".to_owned(),
            issues_open: "[{\"number\":5,\"title\":\"a|b\\nc\",\"createdAt\":\"2026-11-12T00:00:00Z\",\"labels\":[{\"name\":\"blocked\"},{\"name\":\"flaky\"}]},{\"number\":6,\"title\":\"x\",\"createdAt\":\"2026-11-20T00:00:00Z\",\"labels\":[{\"name\":\"ready\"}]}]".to_owned(),
            runs: format!(
                "[{},{},{},{},{},{},{}]",
                run_json(10, "failure", "main", 1, "2026-11-15T00:00:00Z"),
                run_json(11, "failure", "t/x", 1, "2026-11-22T10:00:00Z"),
                run_json(12, "success", "t/x", 2, "2026-11-21T10:00:00Z"),
                run_json(13, "failure", "t/y", 3, "2026-11-20T10:00:00Z"),
                run_json(14, "", "t/y", 1, "2026-11-20T10:00:00Z"),
                run_json(15, "failure", "main", 1, "2026-11-14T10:00:00Z"),
                run_json(16, "failure", "main", 1, "2026-11-23T00:00:00Z")
            ),
            jobs: BTreeMap::from([
                (10, "{\"jobs\":[{\"name\":\"policy\",\"conclusion\":\"failure\"},{\"name\":\"judge-verdict\",\"conclusion\":\"failure\"},{\"name\":\"pages\",\"conclusion\":\"skipped\"}]}".to_owned()),
                (11, "{\"jobs\":[{\"name\":\"judge-verdict\",\"conclusion\":\"failure\"}]}".to_owned()),
                (15, "{\"jobs\":[{\"name\":\"outside\",\"conclusion\":\"failure\"}]}".to_owned()),
            ]),
        }
    }

    #[test]
    fn compute_filters_the_period_on_both_edges() {
        let commits = parse_git_log(LOG).unwrap();
        let rep = compute(&inputs(), &commits, d("2026-11-15"), d("2026-11-22")).unwrap();
        assert_eq!((rep.merges, rep.added, rep.deleted), (2, 7, 1));
        assert_eq!((rep.merged_prs, rep.closed_issues), (2, 1));
        assert_eq!(rep.reviews.len(), 1);
        assert_eq!(
            (rep.reviews[0].pr, rep.reviews[0].rounds, rep.reviews[0].r),
            (1, 3, [0, 1, 0, 0])
        );
        let ci = &rep.ci;
        assert_eq!(ci.runs, 5);
        assert_eq!(
            ci.by_conclusion,
            BTreeMap::from([
                ("failure".to_owned(), 3),
                ("in progress".to_owned(), 1),
                ("success".to_owned(), 1)
            ])
        );
        assert_eq!((ci.failed_main, ci.failed_other), (1, 2));
        assert_eq!((ci.reruns, ci.reruns_green), (2, 1));
        assert_eq!(
            (ci.jobs_read, ci.jobs_unknown, ci.jobs_capped),
            (2, 1, false)
        );
        assert_eq!(
            ci.failing_jobs,
            BTreeMap::from([("judge-verdict".to_owned(), 2), ("policy".to_owned(), 1)])
        );
        assert_eq!(
            rep.blocked,
            [Issue {
                number: 5,
                title: "a|b\nc".to_owned(),
                age_days: 10
            }]
        );
        assert_eq!(rep.flaky.len(), 1);
        assert_eq!(
            rep.audits,
            [
                ("docs/audits/0001.md".to_owned(), d("2026-11-14"), None),
                ("docs/audits/0002.md".to_owned(), d("2026-11-20"), Some(1))
            ]
        );
        assert_eq!(rep.merges_since_audit, Some(1));
        assert_eq!(
            rep.retros.iter().map(|r| r.2).collect::<Vec<_>>(),
            [None, Some(8)]
        );
        assert_eq!(rep.days_since_retro, Some(0));
        assert!(rep.truncated.is_empty());
        let md = render_md(&rep);
        assert!(md.contains("- #5 (10 days): a/b c\n"), "{md}");
        assert!(md.contains("| judge-verdict | 2 |\n| policy | 1 |"), "{md}");
        assert!(md.contains("(1 without job data)"), "{md}");
        assert!(md.contains("| Sessions | no ops/logs/index.tsv |"), "{md}");
    }

    #[test]
    fn missing_fields_fail_with_the_file_name() {
        let commits = Vec::new();
        let mut i = inputs();
        i.prs = "[{\"number\":1}]".to_owned();
        assert!(
            compute(&i, &commits, 0, 1)
                .unwrap_err()
                .contains("prs.json: missing string `mergedAt`")
        );
        let mut i = inputs();
        i.runs = "{}".to_owned();
        assert!(
            compute(&i, &commits, 0, 1)
                .unwrap_err()
                .contains("runs.json: not a JSON array")
        );
        let mut i = inputs();
        i.jobs.insert(10, "{\"nojobs\":1}".to_owned());
        let err = compute(&i, &commits, d("2026-11-15"), d("2026-11-22")).unwrap_err();
        assert!(err.contains("jobs-10.json: missing array `jobs`"), "{err}");
        let mut i = inputs();
        i.issues_open = "[{\"number\":-1,\"createdAt\":\"2026-11-12T00:00:00Z\"}]".to_owned();
        assert!(
            compute(&i, &commits, 0, 1)
                .unwrap_err()
                .contains("not a count")
        );
    }

    fn list_of(n: usize, item: &str) -> String {
        format!("[{}]", vec![item; n].join(","))
    }

    #[test]
    fn truncation_flags_exactly_the_limit() {
        let commits = Vec::new();
        let issue = "{\"number\":1,\"closedAt\":\"2026-11-15T01:00:00Z\"}";
        let mut i = inputs();
        i.issues_closed = list_of(LIST_LIMIT - 1, issue);
        assert!(compute(&i, &commits, 0, 1).unwrap().truncated.is_empty());
        i.issues_closed = list_of(LIST_LIMIT, issue);
        let rep = compute(&i, &commits, 0, 1).unwrap();
        assert_eq!(rep.truncated, ["closed issues"]);
        assert!(render_md(&rep).contains("the closed issues list returned 1000 entries"));
    }

    #[test]
    fn job_reads_stop_at_the_newest_hundred() {
        let runs = |n: u64| {
            let items: Vec<String> = (0..n)
                .map(|k| {
                    run_json(
                        k,
                        "failure",
                        "main",
                        1,
                        &format!("2026-11-15T00:{:02}:{:02}Z", k / 60, k % 60),
                    )
                })
                .collect();
            format!("[{}]", items.join(","))
        };
        let (ids, capped) = job_run_ids(&runs(100), 0, d("2026-12-01")).unwrap();
        assert_eq!((ids.len(), capped), (100, false));
        let (ids, capped) = job_run_ids(&runs(101), 0, d("2026-12-01")).unwrap();
        assert_eq!((ids.len(), capped), (100, true));
        assert_eq!((ids.first(), ids.last()), (Some(&100), Some(&1))); // newest first; run 0 dropped
    }

    #[test]
    fn cells_and_shares() {
        assert_eq!(cell("x|y\r\n\u{2028}z"), "x/y   z");
        assert_eq!(share(1, 3), "1/3 (33%)");
        assert_eq!(share(0, 0), "0/0");
    }
}
