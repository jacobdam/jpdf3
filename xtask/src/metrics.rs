//! `cargo xtask metrics`: the retro input of AUTOMATION §11, from git, the driver's session
//! index and read-only `gh` queries. Spec: specs/tooling/metrics.md.
//!
//! GitHub text (comments, titles) is data: this module only counts lines in it and escapes
//! titles before they reach the markdown table.

use jpdf_testgen::json::{self, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// `--limit` for every `gh` list; a list that returns exactly this many may be cut off.
pub const LIST_LIMIT: usize = 1000;
/// Failed CI runs whose jobs are read (`gh run view`), newest first.
pub const MAX_JOB_READS: usize = 100;
/// The conversation comments `gh pr list --json comments` returns per PR (its GraphQL query
/// asks for `comments(first: 100)` and doesn't paginate them).
pub const COMMENT_LIMIT: usize = 100;
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
    /// The driver's session index (default `ops/logs/index.tsv`, or `index.tsv` in `--from`).
    pub index: Option<PathBuf>,
}

/// Parses the arguments after `metrics`.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        since: None,
        until: None,
        out: PathBuf::from("target/metrics"),
        from: None,
        index: None,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--since" => opts.since = Some(parse_date_arg(value()?)?),
            "--until" => opts.until = Some(parse_date_arg(value()?)?),
            "--out" => opts.out = PathBuf::from(value()?),
            "--from" => opts.from = Some(PathBuf::from(value()?)),
            "--index" => opts.index = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    // The check runs against the default `--until` (today) too (PR #220 R1.2).
    let until = match opts.until {
        Some(u) => u,
        None => today()?,
    };
    opts.until = Some(until);
    if let Some(s) = opts.since
        && s > until
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

/// The UTC day of a Unix time in seconds (`%ct`), years 1970..=9999 only.
pub(crate) fn utc_day(secs: &str) -> Option<i64> {
    // Digits only: `i64::from_str` would also take a leading `+` (PR #220 R2.2).
    if secs.is_empty() || !secs.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let secs: i64 = secs.parse().ok()?;
    let max = days_from_civil(9999, 12, 31)?
        .checked_mul(86_400)?
        .checked_add(86_399)?;
    (0..=max).contains(&secs).then(|| secs.div_euclid(86_400))
}

/// The day of a GitHub timestamp, which has exactly the form `2026-11-22T11:48:38Z` (a real
/// date, a time up to 23:59:59, nothing after the `Z`; audit 0006 AC.8).
fn iso_day(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let shape = b.len() == 20
        && b.get(10) == Some(&b'T')
        && b.get(13) == Some(&b':')
        && b.get(16) == Some(&b':')
        && b.get(19) == Some(&b'Z');
    if !shape {
        return None;
    }
    let part = |r: std::ops::Range<usize>| {
        s.get(r)
            .filter(|p| p.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|p| p.parse::<u8>().ok())
    };
    if part(11..13)? > 23 || part(14..16)? > 59 || part(17..19)? > 59 {
        return None;
    }
    parse_date(s.get(..10)?)
}

/// A non-empty run of ASCII digits as a `u64` (`u64::from_str` would also take a leading `+`).
fn digits_u64(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

pub(crate) fn today() -> Result<i64, String> {
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

/// Parses `git log --first-parent --no-renames --format=@@%H%x09%ct%x09%s --numstat --summary`.
/// `%ct` (Unix seconds) gives the UTC day; `%cs` would be the committer's own time zone.
/// Returns commits oldest first.
fn parse_git_log(text: &str) -> Result<Vec<Commit>, String> {
    let mut commits: Vec<Commit> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let bad = || format!("git log line {}: unexpected `{line}`", i.saturating_add(1));
        if let Some(rest) = line.strip_prefix("@@") {
            let mut f = rest.splitn(3, '\t');
            let (_hash, time) = (f.next(), f.next());
            let day = time.and_then(utc_day).ok_or_else(bad)?;
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
                // Digits only, as the driver writes them (audit 0006 AC.8: no `+`).
                let code = digits_u64(code)?;
                let secs = digits_u64(dur.strip_suffix('s')?)?;
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
    let t = digits(9..15)?;
    let part = |r: std::ops::Range<usize>| t.get(r).and_then(|x| x.parse::<u8>().ok());
    if part(0..2)? > 23 || part(2..4)? > 59 || part(4..6)? > 59 {
        return None;
    }
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
    // TICK §4 step 6: the final check is round 5.
    let final_check = k == 5
        && rest
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
    // Only IDs a review comment reported: a triage line can't add a finding (audit 0006 AC.4).
    let regrades = regraded.iter().filter(|id| ids.contains_key(*id)).count();
    rev.regrades = u64::try_from(regrades).unwrap_or(u64::MAX);
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
    /// Failed runs (of those read) per failing job name.
    failing_jobs: BTreeMap<String, u64>,
    jobs_read: u64,
    jobs_unknown: u64,
    /// More than `MAX_JOB_READS` failed runs: the older ones were left out.
    jobs_capped: bool,
    reruns: u64,
    reruns_success: u64,
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
    /// Merged PRs in the period whose comment list hit `COMMENT_LIMIT` (audit 0006 CX.10).
    comments_truncated: Vec<u64>,
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
    /// Failing job names per run id read (`failing_jobs`); a run missing here has no job data.
    jobs: BTreeMap<u64, Vec<String>>,
}

/// The default `--since`: the day the last retro record dated on or before `until` was added,
/// else `until` − 7 days. Records after `until` are skipped, as in the lag section (#261 R1.1).
fn default_since(commits: &[Commit], until: i64) -> i64 {
    records(commits, "retros")
        .into_iter()
        .map(|(_, day, _)| day)
        .rfind(|d| *d <= until)
        .unwrap_or(until.saturating_sub(DEFAULT_DAYS))
}

/// A run that `runs.json` lists.
struct Run<'a> {
    id: u64,
    created: &'a str,
    day: i64,
    conclusion: &'a str,
    event: &'a str,
    branch: &'a str,
    attempt: u64,
}

/// The runs in `runs.json` created in the period, each `databaseId` once (its first entry),
/// and whether the list hit `LIST_LIMIT`.
fn period_runs(runs: &Value, since: i64, until: i64) -> Result<(Vec<Run<'_>>, bool), String> {
    let what = "runs.json";
    let runs = array(runs, what)?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for r in runs {
        let run = Run {
            id: num(r, "databaseId", what)?,
            created: string(r, "createdAt", what)?,
            day: day_of(r, "createdAt", what)?,
            conclusion: string(r, "conclusion", what)?,
            event: string(r, "event", what)?,
            branch: string(r, "headBranch", what)?,
            attempt: num(r, "attempt", what)?,
        };
        if run.day >= since && run.day <= until && seen.insert(run.id) {
            out.push(run);
        }
    }
    Ok((out, runs.len() >= LIST_LIMIT))
}

/// Ids of the failed runs whose jobs are read: the newest `MAX_JOB_READS` (by `createdAt`
/// text, then the larger id), newest first; and whether older failed runs were left out.
fn job_run_ids(runs: &[Run<'_>]) -> (Vec<u64>, bool) {
    let mut failed: Vec<(&str, u64)> = runs
        .iter()
        .filter(|r| r.conclusion == "failure")
        .map(|r| (r.created, r.id))
        .collect();
    failed.sort_unstable_by(|a, b| b.cmp(a));
    let capped = failed.len() > MAX_JOB_READS;
    let ids = failed
        .into_iter()
        .take(MAX_JOB_READS)
        .map(|(_, id)| id)
        .collect();
    (ids, capped)
}

/// The names of the failed jobs in one `gh run view <id> --json jobs` output, each once.
fn failing_jobs(text: &str, id: u64) -> Result<Vec<String>, String> {
    let what = format!("jobs-{id}.json");
    let v = parse_list(text, &what)?;
    let jobs = v
        .get("jobs")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{what}: missing array `jobs`"))?;
    let mut names = BTreeSet::new();
    for j in jobs {
        if j.get("conclusion").and_then(Value::as_str) == Some("failure") {
            names.insert(string(j, "name", &what)?.to_owned());
        }
    }
    Ok(names.into_iter().collect())
}

fn compute_ci(inp: &Inputs, since: i64, until: i64, rep: &mut Report) -> Result<(), String> {
    let runs = parse_list(&inp.runs, "runs.json")?;
    let (runs, truncated) = period_runs(&runs, since, until)?;
    if truncated {
        rep.truncated.push("CI runs");
    }
    let ci = &mut rep.ci;
    for r in &runs {
        ci.runs = ci.runs.saturating_add(1);
        let key = if r.conclusion.is_empty() {
            "in progress"
        } else {
            r.conclusion
        };
        let n = ci.by_conclusion.entry(key.to_owned()).or_insert(0);
        *n = n.saturating_add(1);
        if r.conclusion == "failure" {
            if r.branch == "main" && r.event != "pull_request" {
                ci.failed_main = ci.failed_main.saturating_add(1);
            } else {
                ci.failed_other = ci.failed_other.saturating_add(1);
            }
        }
        if r.attempt > 1 {
            ci.reruns = ci.reruns.saturating_add(1);
            if r.conclusion == "success" {
                ci.reruns_success = ci.reruns_success.saturating_add(1);
            }
        }
    }
    let (ids, capped) = job_run_ids(&runs);
    ci.jobs_capped = capped;
    for id in ids {
        let Some(names) = inp.jobs.get(&id) else {
            ci.jobs_unknown = ci.jobs_unknown.saturating_add(1);
            continue;
        };
        ci.jobs_read = ci.jobs_read.saturating_add(1);
        for name in names {
            let n = ci.failing_jobs.entry(name.clone()).or_insert(0);
            *n = n.saturating_add(1);
        }
    }
    Ok(())
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
        if comments.len() >= COMMENT_LIMIT {
            rep.comments_truncated.push(number);
        }
        rep.reviews.extend(review_of(number, comments));
    }
    rep.reviews.sort_unstable_by_key(|r| r.pr);
    rep.comments_truncated.sort_unstable();

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

    compute_ci(inp, since, until, &mut rep)?;

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
    // Retro records dated after --until are left out, so the lag is never negative (audit
    // 0006 AC.3); default_since uses the same records.
    let mut prev_day: Option<i64> = None;
    for (_, day, path) in records(commits, "retros")
        .into_iter()
        .filter(|(_, day, _)| *day <= until)
    {
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

/// The review totals shown in the report and in `metrics.json`.
struct ReviewSummary {
    prs: u64,
    median_rounds: Option<u64>,
    at_cap: u64,
    final_checks: u64,
    r: [u64; 4],
    s: [u64; 4],
    regrades: u64,
}

fn review_summary(reviews: &[Review]) -> ReviewSummary {
    let count = |c: usize| u64::try_from(c).unwrap_or(u64::MAX);
    let mut rounds: Vec<u64> = reviews.iter().map(|r| r.rounds).collect();
    let mut sum = ReviewSummary {
        prs: count(reviews.len()),
        median_rounds: median(&mut rounds),
        at_cap: count(reviews.iter().filter(|r| r.rounds >= 4).count()),
        final_checks: count(reviews.iter().filter(|r| r.final_check).count()),
        r: [0; 4],
        s: [0; 4],
        regrades: 0,
    };
    for rev in reviews {
        for (t, v) in sum.r.iter_mut().zip(rev.r) {
            *t = t.saturating_add(v);
        }
        for (t, v) in sum.s.iter_mut().zip(rev.s) {
            *t = t.saturating_add(v);
        }
        sum.regrades = sum.regrades.saturating_add(rev.regrades);
    }
    sum
}

fn render_ci_md(o: &mut String, ci: &Ci) {
    o.push_str("\n## CI (workflow ci.yml)\n\n");
    let by: Vec<String> = ci
        .by_conclusion
        .iter()
        .map(|(k, v)| format!("{} {v}", cell(k)))
        .collect();
    let by = if by.is_empty() {
        "none".to_owned()
    } else {
        by.join(", ")
    };
    let _ = writeln!(o, "Runs: {} ({by}).\n", ci.runs);
    let _ = writeln!(
        o,
        "Failed runs (conclusion `failure`): main {}, other branches {}.\n",
        ci.failed_main, ci.failed_other
    );
    let _ = writeln!(
        o,
        "Re-runs (attempt > 1): {}, of which ended in success: {}. Infra and real failures are not \
         told apart (`gh run list` shows a run's latest attempt only).\n",
        ci.reruns, ci.reruns_success
    );
    let _ = write!(
        o,
        "Failing jobs over {} failed run{} read",
        ci.jobs_read,
        if ci.jobs_read == 1 { "" } else { "s" }
    );
    if ci.jobs_unknown > 0 {
        let _ = write!(o, " ({} more without job data)", ci.jobs_unknown);
    }
    if ci.jobs_capped {
        let _ = write!(
            o,
            " (only the {MAX_JOB_READS} newest failed runs; older ones left out)"
        );
    }
    o.push_str(":\n\n");
    let mut jobs: Vec<(&String, &u64)> = ci.failing_jobs.iter().collect();
    jobs.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    if jobs.is_empty() {
        o.push_str("none\n");
    } else {
        o.push_str("| Job | Failed runs |\n|---|---|\n");
        for (name, n) in jobs {
            let _ = writeln!(o, "| {} | {n} |", cell(name));
        }
    }
}

/// A JSON string literal: `"` and `\` escaped with a backslash, control characters and
/// U+2028/U+2029 as `\uXXXX`.
fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len().saturating_add(2));
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if c.is_control() || c == '\u{2028}' || c == '\u{2029}' => {
                // Control characters above U+FFFF don't exist, so one escape suffices.
                let _ = write!(o, "\\u{:04x}", u32::from(c));
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn json_opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or_else(|| "null".to_owned(), |v| v.to_string())
}

fn json_sev(v: &[u64; 4]) -> String {
    let parts: Vec<String> = SEVERITIES
        .iter()
        .zip(v)
        .map(|(name, n)| format!("\"{name}\":{n}"))
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn json_counts(m: &BTreeMap<String, u64>) -> String {
    let parts: Vec<String> = m
        .iter()
        .map(|(k, v)| format!("{}:{v}", json_str(k)))
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn json_issues(list: &[Issue]) -> String {
    let parts: Vec<String> = list
        .iter()
        .map(|i| {
            format!(
                "{{\"number\":{},\"title\":{},\"age_days\":{}}}",
                i.number,
                json_str(&i.title),
                i.age_days
            )
        })
        .collect();
    format!("[{}]", parts.join(","))
}

fn json_records<T: std::fmt::Display + Copy>(list: &[(String, i64, Option<T>)]) -> String {
    let parts: Vec<String> = list
        .iter()
        .map(|(path, day, prev)| {
            format!(
                "{{\"record\":{},\"date\":\"{}\",\"since_previous\":{}}}",
                json_str(path),
                fmt_day(*day),
                json_opt(*prev)
            )
        })
        .collect();
    format!("[{}]", parts.join(","))
}

/// `metrics.json`: the report's numbers (specs/tooling/metrics.md, Behavior).
fn render_json(rep: &Report) -> String {
    let sessions = rep.sessions.as_ref().map_or_else(
        || "null".to_owned(),
        |s| {
            format!(
                "{{\"count\":{},\"nonzero\":{},\"total_seconds\":{},\"median_seconds\":{},\"malformed\":{}}}",
                s.count,
                s.nonzero,
                s.total_s,
                json_opt(s.median_s),
                s.malformed
            )
        },
    );
    let sum = review_summary(&rep.reviews);
    let per_pr: Vec<String> = rep
        .reviews
        .iter()
        .map(|r| {
            format!(
                "{{\"pr\":{},\"rounds\":{},\"final_check\":{},\"subagent\":{},\"second\":{},\"regrades\":{}}}",
                r.pr,
                r.rounds,
                r.final_check,
                json_sev(&r.r),
                json_sev(&r.s),
                r.regrades
            )
        })
        .collect();
    let truncated: Vec<String> = rep.truncated.iter().map(|t| json_str(t)).collect();
    let comments_truncated: Vec<String> =
        rep.comments_truncated.iter().map(u64::to_string).collect();
    let ci = &rep.ci;
    let mut o = String::new();
    let _ = write!(
        o,
        "{{\"since\":\"{}\",\"until\":\"{}\",\"truncated\":[{}],\"comments_truncated\":[{}],",
        fmt_day(rep.since),
        fmt_day(rep.until),
        truncated.join(","),
        comments_truncated.join(",")
    );
    let _ = write!(
        o,
        "\"throughput\":{{\"merges\":{},\"lines_added\":{},\"lines_deleted\":{},\"merged_prs\":{},\"closed_issues\":{},\"sessions\":{sessions}}},",
        rep.merges, rep.added, rep.deleted, rep.merged_prs, rep.closed_issues
    );
    let _ = write!(
        o,
        "\"review\":{{\"prs\":{},\"median_rounds\":{},\"at_cap\":{},\"final_checks\":{},\"regrades\":{},\"subagent\":{},\"second\":{},\"per_pr\":[{}]}},",
        sum.prs,
        json_opt(sum.median_rounds),
        sum.at_cap,
        sum.final_checks,
        sum.regrades,
        json_sev(&sum.r),
        json_sev(&sum.s),
        per_pr.join(",")
    );
    let _ = write!(
        o,
        "\"ci\":{{\"runs\":{},\"by_conclusion\":{},\"failed_main\":{},\"failed_other\":{},\"reruns\":{},\"reruns_success\":{},\"jobs_read\":{},\"jobs_unknown\":{},\"jobs_capped\":{},\"failing_jobs\":{}}},",
        ci.runs,
        json_counts(&ci.by_conclusion),
        ci.failed_main,
        ci.failed_other,
        ci.reruns,
        ci.reruns_success,
        ci.jobs_read,
        ci.jobs_unknown,
        ci.jobs_capped,
        json_counts(&ci.failing_jobs)
    );
    let _ = write!(
        o,
        "\"stuck\":{{\"blocked\":{},\"flaky\":{}}},",
        json_issues(&rep.blocked),
        json_issues(&rep.flaky)
    );
    let _ = writeln!(
        o,
        "\"lag\":{{\"audits\":{},\"merges_since_audit\":{},\"retros\":{},\"days_since_retro\":{}}}}}",
        json_records(&rep.audits),
        json_opt(rep.merges_since_audit),
        json_records(&rep.retros),
        json_opt(rep.days_since_retro)
    );
    o
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
    if !rep.comments_truncated.is_empty() {
        let prs: Vec<String> = rep
            .comments_truncated
            .iter()
            .map(|n| format!("#{n}"))
            .collect();
        let _ = writeln!(
            o,
            "- **Warning:** the comment lists of these merged PRs returned {COMMENT_LIMIT} entries \
             and may be cut off, so their review figures may be incomplete: {}.",
            prs.join(", ")
        );
    }
    if !rep.truncated.is_empty() || !rep.comments_truncated.is_empty() {
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
    let sum = review_summary(&rep.reviews);
    let median = sum
        .median_rounds
        .map_or_else(|| "n/a".to_owned(), |m| m.to_string());
    let _ = writeln!(
        o,
        "PRs reviewed: {}; median rounds: {median}; at 4+ rounds: {}; with a final check: {}.",
        sum.prs,
        share(sum.at_cap, sum.prs),
        share(sum.final_checks, sum.prs)
    );
    let _ = writeln!(
        o,
        "Findings blocker/major/minor/nit: subagent {}, second reviewer {}; regrades: {}.\n",
        sev_counts(&sum.r),
        sev_counts(&sum.s),
        sum.regrades
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

    render_ci_md(&mut o, &rep.ci);

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
            |m| {
                format!(
                    "{m} merge{} after the previous one",
                    if m == 1 { "" } else { "s" }
                )
            },
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
            |d| {
                format!(
                    "{d} day{} after the previous one",
                    if d == 1 { "" } else { "s" }
                )
            },
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
    "--format=@@%H%x09%ct%x09%s",
    "--numstat",
    "--summary",
    "origin/main",
];

fn read(dir: &Path, name: &str) -> Result<String, String> {
    let p = dir.join(name);
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}

fn read_optional(p: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(p) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", p.display())),
    }
}

/// Writes the fetched inputs (not the job outputs) to `out/raw/`, which must exist.
fn save_raw(out: &Path, inp: &Inputs) -> Result<(), String> {
    let raw = out.join("raw");
    let write = |name: &str, text: &str| {
        let p = raw.join(name);
        fs::write(&p, text).map_err(|e| format!("write {}: {e}", p.display()))
    };
    write("git-log.txt", &inp.git_log)?;
    match &inp.index_tsv {
        Some(t) => write("index.tsv", t)?,
        // A copy left by an earlier run would be read back by `--from` as this run's
        // sessions (audit 0006 CX.11).
        None => remove_stale(&raw.join("index.tsv"))?,
    }
    write("prs.json", &inp.prs)?;
    write("issues-closed.json", &inp.issues_closed)?;
    write("issues-open.json", &inp.issues_open)?;
    write("runs.json", &inp.runs)
}

/// Removes `p` if it exists; a missing file is fine.
fn remove_stale(p: &Path) -> Result<(), String> {
    match fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("remove {}: {e}", p.display())),
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
    let out = &opts.out;
    let write = |name: &str, text: &str| {
        let p = out.join(name);
        fs::write(&p, text).map_err(|e| format!("write {}: {e}", p.display()))
    };
    let out_dir = if from.is_none() {
        out.join("raw")
    } else {
        out.clone()
    };
    fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;
    let mut texts = Vec::new();
    for (file, args) in &lists {
        texts.push(match from {
            Some(dir) => read(dir, file)?,
            None => gh(args)?,
        });
    }
    let mut it = texts.into_iter();
    let index_tsv = match (from, &opts.index) {
        (_, Some(file)) => read_optional(file)?,
        (Some(dir), None) => read_optional(&dir.join("index.tsv"))?,
        (None, None) => read_optional(&repo.join("ops/logs/index.tsv"))?,
    };
    let mut inp = Inputs {
        git_log,
        index_tsv,
        prs: it.next().unwrap_or_default(),
        issues_closed: it.next().unwrap_or_default(),
        issues_open: it.next().unwrap_or_default(),
        runs: it.next().unwrap_or_default(),
        jobs: BTreeMap::new(),
    };
    if from.is_none() {
        save_raw(out, &inp)?;
    }
    // One job output at a time: saved, reduced to its failing job names, then dropped.
    let runs = parse_list(&inp.runs, "runs.json")?;
    let (ids, _) = job_run_ids(&period_runs(&runs, since, until)?.0);
    for id in ids {
        let name = format!("jobs-{id}.json");
        let text = match from {
            Some(dir) => read_optional(&dir.join(&name))?,
            None => {
                let t = gh(&format!("run view {id} --json jobs"))?;
                write(&format!("raw/{name}"), &t)?;
                Some(t)
            }
        };
        if let Some(t) = text {
            inp.jobs.insert(id, failing_jobs(&t, id)?);
        }
    }
    let rep = compute(&inp, &commits, since, until)?;
    let md = render_md(&rep);
    write("metrics.md", &md)?;
    write("metrics.json", &render_json(&rep))?;
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
        assert_eq!(iso_day("2026-11-22T00:00:00Z"), Some(d("2026-11-22")));
        assert_eq!(iso_day("2026-11-22T23:59:59Z"), Some(d("2026-11-22")));
        // Audit 0006 AC.8: nothing after the `Z`, and the time part is checked too.
        for bad in [
            "2026-11",
            "2026-11-22",
            "2026-11-22T11:48:38Zx",
            "2026-11-22T11:48:38Z ",
            "2026-11-22T11:48:38",
            "2026-11-22T11:48:38.000Z",
            "2026-11-22T11:48:38x",
            "2026-11-22x11:48:38Z",
            "2026-11-22T11x48:38Z",
            "2026-11-22T11:48x38Z",
            "2026-11-22 11:48:38Z",
            "2026-11-22T11-48:38Z",
            "2026-11-22T11:48-38Z",
            "2026-11-22T24:00:00Z",
            "2026-11-22T23:60:00Z",
            "2026-11-22T23:59:60Z",
            "2026-11-22T+1:48:38Z",
            "2026-11-22T11:+8:38Z",
            "2026-11-22T11:48:+8Z",
            "2026-02-30T11:48:38Z",
        ] {
            assert_eq!(iso_day(bad), None, "{bad}");
        }
        assert_eq!(digits_u64("0"), Some(0));
        assert_eq!(digits_u64("143"), Some(143));
        for bad in ["", "+1", "-1", " 1", "1 ", "1x", "18446744073709551616"] {
            assert_eq!(digits_u64(bad), None, "{bad}");
        }
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
            "--index",
            "z",
        ])
        .unwrap();
        assert_eq!(o.since, Some(d("2026-11-15")));
        assert_eq!(o.until, Some(d("2026-11-22")));
        assert_eq!(
            (o.out, o.from, o.index),
            (
                PathBuf::from("x"),
                Some(PathBuf::from("y")),
                Some(PathBuf::from("z"))
            )
        );
        // `today` before and after the call, so a run across midnight UTC can't fail (R2.1).
        let before = today().unwrap();
        let o = a(&[]).unwrap();
        let after = today().unwrap();
        assert!(o.until.is_some_and(|u| (before..=after).contains(&u)));
        assert_eq!(
            (o.since, o.out, o.from, o.index),
            (None, PathBuf::from("target/metrics"), None, None)
        );
        // An explicit --since after the default --until (today) is refused too (PR #220 R1.2).
        assert!(a(&["--since", "9999-12-31"]).unwrap_err().contains("after"));
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

    // %ct times: c4 2026-11-23 00:00:00 UTC (one second past --until), c3 2026-11-22 23:59:59
    // (2026-11-23 in UTC+7, PR #220 R1.1), c1b 2026-11-15 00:00:00, c1 2026-11-14 23:59:59.
    const LOG: &str = "@@c4\t1795392000\tafter\n\
        50\t0\tsrc/b.rs\n\
        @@c3\t1795391999\tthird\n\
        5\t1\tsrc/a.rs\n\
        100\t100\tCargo.lock\n\
        7\t0\tapps/web/package-lock.json\n\
        9\t9\ttools/arlington/data/x.tsv\n\
        -\t-\tlogo.png\n\
        \x20create mode 100644 docs/retros/0002.md\n\
        \n\
        @@c2\t1795176000\tsecond\twith tab\n\
        2\t0\tdocs/audits/0002.md\n\
        \x20create mode 100644 docs/audits/0002.md\n\
        \x20create mode 100644 docs/audits/0002-extra.md\n\
        \x20mode change 100644 => 100755 x.sh\n\
        @@c1b\t1794700800\ton since\n\
        1\t0\tsrc/c.rs\n\
        @@c1\t1794700799\tfirst\n\
        \x20create mode 100644 docs/audits/0001.md\n\
        \x20create mode 100644 docs/retros/0001.md\n\
        \x20create mode 100644 docs/retros/0000-pilot.md\n\
        @@c0\t1794571200\tzero\n";

    #[test]
    fn git_log_counts_lines_without_lockfiles_and_finds_records() {
        let c = parse_git_log(LOG).unwrap();
        assert_eq!(c.len(), 6);
        let days: Vec<i64> = c.iter().map(|c| c.day).collect();
        let want = [
            "2026-11-13",
            "2026-11-14",
            "2026-11-15",
            "2026-11-20",
            "2026-11-22",
            "2026-11-23",
        ];
        assert_eq!(days, want.map(d));
        assert_eq!((c[4].added, c[4].deleted), (5, 1));
        assert_eq!((c[3].added, c[3].deleted), (2, 0));
        let audits: Vec<_> = records(&c, "audits")
            .into_iter()
            .map(|(i, _, p)| (i, p))
            .collect();
        assert_eq!(
            audits,
            [
                (1, "docs/audits/0001.md".to_owned()),
                (3, "docs/audits/0002.md".to_owned())
            ]
        );
        assert_eq!(records(&c, "retros").len(), 2); // 0000-pilot.md is not a record
        assert_eq!(default_since(&c, d("2026-11-22")), d("2026-11-22"));
        // The newest retro (2026-11-22) is after until: the one before it is used (#261 R1.1).
        // 2026-11-21 − 7 is also 2026-11-14, so 2026-11-20 tells the two rules apart.
        assert_eq!(default_since(&c, d("2026-11-21")), d("2026-11-14"));
        assert_eq!(default_since(&c, d("2026-11-20")), d("2026-11-14"));
        // No retro record on or before until.
        assert_eq!(default_since(&c, d("2026-11-13")), d("2026-11-06"));
        assert_eq!(default_since(&[], d("2026-11-22")), d("2026-11-15"));
        for bad in [
            "x\t1\tsrc/a.rs\n",
            "@@c\tnot-a-date\ts\n",
            "@@c\t2026-11-22\ts\n",
            "@@c\t-1\ts\n",
            "@@c\t+1795391999\ts\n",
            "@@c\t\ts\n",
            "@@c\t253402300800\ts\n",
            "@@c\t0\ts\n1\tsrc\n",
            "@@c\t0\ts\nx\t1\tp\n",
        ] {
            assert!(parse_git_log(bad).is_err(), "{bad:?}");
        }
        assert_eq!(utc_day("253402300799"), Some(d("9999-12-31")));
        assert_eq!(utc_day("0"), Some(0));
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
            20261116T000000Z\t0\te\t60s\textra\n\
            20261116T240000Z\t0\te\t5s\n\
            20261116T006000Z\t0\te\t5s\n\
            20261116T000060Z\t0\te\t5s\n\
            20261116T000000Z\t+0\te\t5s\n\
            20261116T000000Z\t-1\te\t5s\n\
            20261116T000000Z\t0\te\t+5s\n\
            20261116T000000Z\t0\te\ts\n";
        let s = parse_sessions(tsv, d("2026-11-15"), d("2026-11-22"));
        assert_eq!(
            s,
            Sessions {
                count: 2,
                nonzero: 1,
                total_s: 720,
                median_s: Some(600),
                malformed: 13
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
                "🤖 jpdf-agent **AI review triage, round 7**\n\nR1.1: fixed in def\nS1.1: regraded minor: no input\nS1.1: regraded minor: again\nR1.2: deferred: #9\nR3.9: regraded minor: never reported\nR1.3: regraded nit: its severity didn't parse",
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
        let r4 = review_of(
            1,
            &[comment(
                "🤖 jpdf-agent **AI review, round 4 (final check)** x",
            )],
        );
        assert!(!r4.unwrap().final_check); // the final check is round 5 only (PR #220 R1.5)
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

    fn run_json(
        id: u64,
        concl: &str,
        event: &str,
        branch: &str,
        attempt: u64,
        created: &str,
    ) -> String {
        format!(
            "{{\"databaseId\":{id},\"conclusion\":\"{concl}\",\"event\":\"{event}\",\"headBranch\":\"{branch}\",\"attempt\":{attempt},\"createdAt\":\"{created}\"}}"
        )
    }

    fn jobs(id: u64, text: &str) -> (u64, Vec<String>) {
        (id, failing_jobs(text, id).unwrap())
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
            issues_closed: "[{\"number\":1,\"closedAt\":\"2026-11-15T00:00:00Z\"},{\"number\":2,\"closedAt\":\"2026-11-14T23:59:59Z\"},{\"number\":3,\"closedAt\":\"2026-11-22T23:59:59Z\"},{\"number\":4,\"closedAt\":\"2026-11-23T00:00:00Z\"}]".to_owned(),
            issues_open: "[{\"number\":5,\"title\":\"a|b\\nc\",\"createdAt\":\"2026-11-12T00:00:00Z\",\"labels\":[{\"name\":\"blocked\"},{\"name\":\"flaky\"}]},{\"number\":6,\"title\":\"x\",\"createdAt\":\"2026-11-20T00:00:00Z\",\"labels\":[{\"name\":\"ready\"}]}]".to_owned(),
            runs: format!("[{}]", [
                run_json(10, "failure", "push", "main", 1, "2026-11-15T00:00:00Z"),
                run_json(11, "failure", "pull_request", "t/x", 1, "2026-11-22T23:59:59Z"),
                run_json(12, "success", "pull_request", "t/x", 2, "2026-11-21T10:00:00Z"),
                run_json(13, "failure", "pull_request", "t/y", 3, "2026-11-20T10:00:00Z"),
                run_json(14, "", "pull_request", "t/y", 1, "2026-11-20T10:00:00Z"),
                run_json(15, "failure", "push", "main", 1, "2026-11-14T23:59:59Z"),
                run_json(16, "failure", "push", "main", 1, "2026-11-23T00:00:00Z"),
                // A pull request from a (fork) branch named main is not a main failure.
                run_json(17, "failure", "pull_request", "main", 1, "2026-11-19T10:00:00Z"),
                run_json(18, "failure", "schedule", "main", 1, "2026-11-19T03:17:00Z"),
                run_json(19, "cancelled", "push", "main", 2, "2026-11-19T04:00:00Z"),
                // Not a pull request, but not on main either: another branch's failure.
                run_json(20, "failure", "workflow_dispatch", "t/z", 1, "2026-11-18T10:00:00Z"),
                // A repeated id counts once, at its first entry.
                run_json(12, "failure", "push", "main", 1, "2026-11-21T10:00:00Z"),
            ]
            .join(",")),
            jobs: BTreeMap::from([
                jobs(10, "{\"jobs\":[{\"name\":\"policy\",\"conclusion\":\"failure\"},{\"name\":\"judge-verdict\",\"conclusion\":\"failure\"},{\"name\":\"pages\",\"conclusion\":\"skipped\"},{\"name\":\"policy\",\"conclusion\":\"failure\"}]}"),
                jobs(11, "{\"jobs\":[{\"name\":\"judge-verdict\",\"conclusion\":\"failure\"},{\"conclusion\":\"success\"}]}"),
                jobs(13, "{\"jobs\":[{\"name\":\"a|b\",\"conclusion\":\"failure\"}]}"),
                jobs(15, "{\"jobs\":[{\"name\":\"outside\",\"conclusion\":\"failure\"}]}"),
                jobs(18, "{\"jobs\":[]}"),
            ]),
        }
    }

    #[test]
    fn compute_filters_the_period_on_both_edges() {
        let commits = parse_git_log(LOG).unwrap();
        let rep = compute(&inputs(), &commits, d("2026-11-15"), d("2026-11-22")).unwrap();
        assert_eq!((rep.merges, rep.added, rep.deleted), (3, 8, 1));
        assert_eq!((rep.merged_prs, rep.closed_issues), (2, 2));
        assert_eq!(rep.reviews.len(), 1);
        assert_eq!(
            (rep.reviews[0].pr, rep.reviews[0].rounds, rep.reviews[0].r),
            (1, 3, [0, 1, 0, 0])
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
        let ci = &rep.ci;
        assert_eq!(ci.runs, 9); // 15 and 16 outside the period, the second 12 a repeat
        assert_eq!(
            ci.by_conclusion,
            BTreeMap::from([
                ("cancelled".to_owned(), 1),
                ("failure".to_owned(), 6),
                ("in progress".to_owned(), 1),
                ("success".to_owned(), 1)
            ])
        );
        assert_eq!((ci.failed_main, ci.failed_other), (2, 4)); // 10, 18; 11, 13, 17, 20
        assert_eq!((ci.reruns, ci.reruns_success), (3, 1)); // 12, 13, 19; 12
        // Failed runs read: 11, 18, 10, 13 (17 and 20 without job data); `policy` once per run.
        assert_eq!(
            (ci.jobs_read, ci.jobs_unknown, ci.jobs_capped),
            (4, 2, false)
        );
        assert_eq!(
            ci.failing_jobs,
            BTreeMap::from([
                ("a|b".to_owned(), 1),
                ("judge-verdict".to_owned(), 2),
                ("policy".to_owned(), 1)
            ])
        );
        assert_eq!(
            rep.audits,
            [
                ("docs/audits/0001.md".to_owned(), d("2026-11-14"), None),
                ("docs/audits/0002.md".to_owned(), d("2026-11-20"), Some(2))
            ]
        );
        assert_eq!(rep.merges_since_audit, Some(2));
        assert_eq!(
            rep.retros.iter().map(|r| r.2).collect::<Vec<_>>(),
            [None, Some(8)]
        );
        assert_eq!(rep.days_since_retro, Some(0));
        assert!(rep.truncated.is_empty());
        let md = render_md(&rep);
        assert!(md.contains("- #5 (10 days): a/b c\n"), "{md}");
        assert!(md.contains("| Sessions | no ops/logs/index.tsv |"), "{md}");
        for want in [
            "Runs: 9 (cancelled 1, failure 6, in progress 1, success 1).\n",
            "Failed runs (conclusion `failure`): main 2, other branches 4.\n",
            "Re-runs (attempt > 1): 3, of which ended in success: 1.",
            "Failing jobs over 4 failed runs read (2 more without job data):\n\n",
            "| judge-verdict | 2 |\n| a/b | 1 |\n| policy | 1 |\n",
        ] {
            assert!(md.contains(want), "missing {want:?} in\n{md}");
        }
        assert!(!md.contains("older ones left out"), "{md}");
    }

    #[test]
    fn retro_lag_leaves_out_records_after_until() {
        // Audit 0006 AC.3: retro 0002 is dated 2026-11-22, after this --until.
        let commits = parse_git_log(LOG).unwrap();
        let lag = |until: &str| {
            let rep = compute(&inputs(), &commits, d("2026-11-15"), d(until)).unwrap();
            let retros: Vec<String> = rep.retros.iter().map(|r| r.0.clone()).collect();
            (retros, rep.days_since_retro, render_md(&rep))
        };
        let (retros, days, md) = lag("2026-11-21");
        assert_eq!(
            (retros, days),
            (vec!["docs/retros/0001.md".to_owned()], Some(7))
        );
        assert!(
            md.contains("Days from the last retro record to 2026-11-21: 7."),
            "{md}"
        );
        assert!(!md.contains("docs/retros/0002.md"), "{md}");
        let (retros, days, _) = lag("2026-11-22");
        assert_eq!((retros.len(), days), (2, Some(0)));
        let (retros, days, _) = lag("2026-11-13");
        assert_eq!((retros.len(), days), (0, None));
    }

    #[test]
    fn a_full_comment_list_is_flagged() {
        // Audit 0006 CX.10: gh returns at most COMMENT_LIMIT comments per PR.
        let commits = Vec::new();
        let c = "{\"body\":\"x\"}";
        let list = |n: usize| vec![c; n].join(",");
        let mut i = inputs();
        i.prs = format!(
            "[{},{},{}]",
            pr(8, "2026-11-15T00:00:00Z", &list(COMMENT_LIMIT)),
            pr(7, "2026-11-15T00:00:00Z", &list(COMMENT_LIMIT - 1)),
            // Out of the period: not flagged.
            pr(9, "2026-11-14T00:00:00Z", &list(COMMENT_LIMIT))
        );
        let rep = compute(&i, &commits, d("2026-11-15"), d("2026-11-22")).unwrap();
        assert_eq!(rep.comments_truncated, [8]);
        assert!(rep.truncated.is_empty());
        assert!(
            render_md(&rep).contains(
                "returned 100 entries and may be cut off, so their review figures may be \
                 incomplete: #8.\n"
            ),
            "{}",
            render_md(&rep)
        );
        assert!(render_json(&rep).contains("\"comments_truncated\":[8],"));
        i.prs = format!(
            "[{}]",
            pr(8, "2026-11-15T00:00:00Z", &list(COMMENT_LIMIT - 1))
        );
        let rep = compute(&i, &commits, d("2026-11-15"), d("2026-11-22")).unwrap();
        assert!(rep.comments_truncated.is_empty());
        assert!(!render_md(&rep).contains("may be cut off"));
        assert!(render_json(&rep).contains("\"comments_truncated\":[],"));
    }

    #[test]
    fn save_raw_removes_a_stale_index() {
        // Audit 0006 CX.11: a rerun without an index must not leave the old one behind.
        let out = std::env::temp_dir().join(format!("xtask-metrics-raw-{}", std::process::id()));
        let _ = fs::remove_dir_all(&out);
        fs::create_dir_all(out.join("raw")).unwrap();
        let mut i = inputs();
        i.index_tsv = Some("20261115T000000Z\t0\ta\t5s\n".to_owned());
        save_raw(&out, &i).unwrap();
        assert_eq!(
            fs::read_to_string(out.join("raw/index.tsv")).unwrap(),
            "20261115T000000Z\t0\ta\t5s\n"
        );
        assert_eq!(fs::read_to_string(out.join("raw/prs.json")).unwrap(), i.prs);
        i.index_tsv = None;
        save_raw(&out, &i).unwrap();
        assert!(!out.join("raw/index.tsv").exists());
        // Already gone: still fine.
        save_raw(&out, &i).unwrap();
        assert_eq!(
            fs::read_to_string(out.join("raw/runs.json")).unwrap(),
            i.runs
        );
        fs::remove_dir_all(&out).unwrap();
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
        for (runs, err) in [
            ("{}", "runs.json: not a JSON array"),
            (
                "[{\"databaseId\":1,\"conclusion\":\"\",\"headBranch\":\"m\",\"attempt\":1,\"createdAt\":\"2026-11-15T00:00:00Z\"}]",
                "runs.json: missing string `event`",
            ),
            (
                "[{\"databaseId\":-1,\"conclusion\":\"\",\"event\":\"push\",\"headBranch\":\"m\",\"attempt\":1,\"createdAt\":\"2026-11-15T00:00:00Z\"}]",
                "runs.json: `databaseId` is not a count",
            ),
        ] {
            let mut i = inputs();
            i.runs = runs.to_owned();
            let got = compute(&i, &commits, 0, 1).unwrap_err();
            assert!(got.contains(err), "{got}");
        }
        assert!(
            failing_jobs("{\"nojobs\":1}", 10)
                .unwrap_err()
                .contains("jobs-10.json: missing array `jobs`")
        );
        assert!(
            failing_jobs("{\"jobs\":[{\"conclusion\":\"failure\"}]}", 7)
                .unwrap_err()
                .contains("jobs-7.json: missing string `name`")
        );
        assert!(failing_jobs("[", 7).unwrap_err().contains("jobs-7.json"));
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
        let pr = "{\"number\":1,\"mergedAt\":\"2026-11-15T01:00:00Z\"}";
        let closed = "{\"number\":1,\"closedAt\":\"2026-11-15T01:00:00Z\"}";
        let open = "{\"number\":1,\"createdAt\":\"2026-11-15T01:00:00Z\"}";
        let run = run_json(1, "success", "push", "main", 1, "2026-11-15T01:00:00Z");
        type Field = fn(&mut Inputs) -> &mut String;
        let lists: [(Field, &str, &str); 4] = [
            (|i| &mut i.prs, pr, "merged PRs"),
            (|i| &mut i.issues_closed, closed, "closed issues"),
            (|i| &mut i.issues_open, open, "open issues"),
            (|i| &mut i.runs, &run, "CI runs"),
        ];
        for (field, item, name) in lists {
            let mut i = inputs();
            *field(&mut i) = list_of(LIST_LIMIT - 1, item);
            assert!(
                compute(&i, &commits, 0, 1).unwrap().truncated.is_empty(),
                "{name}"
            );
            *field(&mut i) = list_of(LIST_LIMIT, item);
            let rep = compute(&i, &commits, 0, 1).unwrap();
            assert_eq!(rep.truncated, [name]);
            assert!(render_md(&rep).contains(&format!("the {name} list returned 1000 entries")));
        }
    }

    #[test]
    fn job_reads_stop_at_the_newest_hundred() {
        // Run k fails at second k of 2026-11-15 with id 1000 − k, so an older run has a larger
        // id; the list is rotated to start at k = 50, so the oldest run sits in the middle and
        // neither list order nor its reverse is time order; a later success is mixed in. Only
        // the order by `createdAt` keeps the newest 100 (PR #267 R1.1, R2.1).
        let runs = |n: u64| {
            let mut items: Vec<String> = (0..n)
                .map(|k| {
                    run_json(
                        1000 - k,
                        "failure",
                        "push",
                        "main",
                        1,
                        &format!("2026-11-15T00:{:02}:{:02}Z", k / 60, k % 60),
                    )
                })
                .collect();
            items.push(run_json(
                5000,
                "success",
                "push",
                "main",
                1,
                "2026-11-15T23:00:00Z",
            ));
            items.rotate_left(50);
            format!("[{}]", items.join(","))
        };
        let ids = |n: u64| {
            let v = json::parse(&runs(n)).unwrap();
            job_run_ids(&period_runs(&v, 0, d("2026-12-01")).unwrap().0)
        };
        let (got, capped) = ids(100);
        assert_eq!((got.len(), capped), (100, false));
        assert!(got.contains(&1000));
        let (got, capped) = ids(101);
        assert_eq!((got.len(), capped), (100, true));
        assert_eq!((got.first(), got.last()), (Some(&900), Some(&999))); // newest first
        // The oldest run (k = 0, the largest id) is the one left out.
        assert!(!got.contains(&1000) && !got.contains(&5000));

        let mut i = inputs();
        i.runs = runs(101);
        i.jobs = BTreeMap::new();
        let rep = compute(&i, &[], 0, d("2026-12-01")).unwrap();
        assert_eq!(
            (rep.ci.jobs_read, rep.ci.jobs_unknown, rep.ci.jobs_capped),
            (0, 100, true)
        );
        assert!(render_md(&rep).contains("(only the 100 newest failed runs; older ones left out)"));
        // Same second: the larger id is newer.
        let tie = format!(
            "[{},{}]",
            run_json(3, "failure", "push", "main", 1, "2026-11-15T00:00:00Z"),
            run_json(4, "failure", "push", "main", 1, "2026-11-15T00:00:00Z")
        );
        let v = json::parse(&tie).unwrap();
        assert_eq!(
            job_run_ids(&period_runs(&v, 0, d("2026-12-01")).unwrap().0).0,
            [4, 3]
        );
    }

    #[test]
    fn json_matches_the_report_and_escapes_strings() {
        let commits = parse_git_log(LOG).unwrap();
        let mut i = inputs();
        i.index_tsv = Some("20261115T000000Z\t1\ta\t120s\n".to_owned());
        let nasty = "q\"b\\n\nc\u{1}d\u{2028}e\u{7f}";
        i.issues_open = format!(
            "[{{\"number\":5,\"title\":{},\"createdAt\":\"2026-11-12T00:00:00Z\",\"labels\":[{{\"name\":\"blocked\"}}]}}]",
            json_str(nasty)
        );
        let rep = compute(&i, &commits, d("2026-11-15"), d("2026-11-22")).unwrap();
        let text = render_json(&rep);
        let v = json::parse(&text).unwrap();
        let at = |path: &[&str]| -> Value {
            let mut cur = &v;
            for k in path {
                cur = cur.get(k).unwrap_or_else(|| panic!("{path:?} in {text}"));
            }
            cur.clone()
        };
        let n = |path: &[&str]| match at(path) {
            Value::Number(s) => s,
            other => panic!("{path:?}: {other:?}"),
        };
        assert_eq!(at(&["since"]).as_str(), Some("2026-11-15"));
        assert_eq!(at(&["until"]).as_str(), Some("2026-11-22"));
        assert_eq!(n(&["throughput", "merges"]), "3");
        assert_eq!(n(&["throughput", "lines_added"]), "8");
        assert_eq!(n(&["throughput", "merged_prs"]), "2");
        assert_eq!(n(&["throughput", "sessions", "nonzero"]), "1");
        assert_eq!(n(&["throughput", "sessions", "median_seconds"]), "120");
        assert_eq!(n(&["review", "prs"]), "1");
        assert_eq!(n(&["review", "median_rounds"]), "3");
        assert_eq!(n(&["review", "subagent", "major"]), "1");
        assert_eq!(n(&["review", "second", "blocker"]), "0");
        assert_eq!(
            at(&["review", "per_pr"]).as_array().map(<[Value]>::len),
            Some(1)
        );
        assert_eq!(n(&["ci", "runs"]), "9");
        assert_eq!(n(&["ci", "by_conclusion", "in progress"]), "1");
        assert_eq!(n(&["ci", "failed_main"]), "2");
        assert_eq!(n(&["ci", "failed_other"]), "4");
        assert_eq!(n(&["ci", "reruns_success"]), "1");
        assert_eq!(n(&["ci", "jobs_unknown"]), "2");
        assert_eq!(at(&["ci", "jobs_capped"]), Value::Bool(false));
        assert_eq!(n(&["ci", "failing_jobs", "a|b"]), "1");
        assert_eq!(n(&["ci", "failing_jobs", "judge-verdict"]), "2");
        let blocked = at(&["stuck", "blocked"]);
        let first = blocked.as_array().and_then(<[Value]>::first).unwrap();
        assert_eq!(first.get("title").and_then(Value::as_str), Some(nasty));
        assert_eq!(n(&["lag", "merges_since_audit"]), "2");
        let audits = at(&["lag", "audits"]);
        let first = audits.as_array().and_then(<[Value]>::first).unwrap();
        assert_eq!(first.get("since_previous"), Some(&Value::Null));
        assert_eq!(
            first.get("date").and_then(Value::as_str),
            Some("2026-11-14")
        );
        assert_eq!(at(&["truncated"]).as_array().map(<[Value]>::len), Some(0));
        assert!(
            !text.contains('\u{2028}') && !text.contains('\u{1}'),
            "{text}"
        );
        assert_eq!(json_str("a\"\\\n\u{2029}"), "\"a\\\"\\\\\\u000a\\u2029\"");
        // No index, no reviews: nulls.
        let empty = render_json(
            &compute(
                &Inputs {
                    runs: "[]".to_owned(),
                    ..inputs()
                },
                &[],
                0,
                1,
            )
            .unwrap(),
        );
        let v = json::parse(&empty).unwrap();
        assert_eq!(
            v.get("throughput").and_then(|t| t.get("sessions")),
            Some(&Value::Null)
        );
        assert_eq!(
            v.get("review").and_then(|t| t.get("median_rounds")),
            Some(&Value::Null)
        );
    }

    #[test]
    fn cells_and_shares() {
        assert_eq!(cell("x|y\r\n\u{2028}z"), "x/y   z");
        assert_eq!(share(1, 3), "1/3 (33%)");
        assert_eq!(share(0, 0), "0/0");
    }
}
