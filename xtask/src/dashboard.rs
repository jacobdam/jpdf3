//! `cargo xtask dashboard`: the metrics snapshot the Pages dashboard renders, and the
//! `ci linux` step that copies it next to the page. Spec: specs/tooling/dashboard.md.
//!
//! GitHub text (comments, titles) is data: this module only counts review headers in it, as
//! `cargo xtask metrics` does, and escapes issue titles with the shared `json_str`.

use crate::metrics::{
    LIST_LIMIT, array, day_of, fmt_day, gh, json_str, label_names, median, num, parse_date,
    parse_list, parse_sessions, read, read_optional, review_of, string, today,
};
use jpdf_testgen::json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Weeks on the page (about one quarter).
pub const WEEKS: i64 = 12;
/// The forecast's trailing window (ops/TICK.md §7 step 5: the trailing 4-week rate).
pub const RATE_DAYS: i64 = 28;
/// Largest snapshot `ci linux` copies into the site.
pub const MAX_SNAPSHOT_BYTES: u64 = 1 << 20;
/// The committed snapshot, relative to the repo root.
pub const SNAPSHOT: &str = "ops/metrics/dashboard.json";
/// Nightly runs listed (about 28 weeks of daily nightlies, newest first).
pub const NIGHTLY_LIMIT: usize = 200;
/// Nightly runs tried per week before the week counts as missing.
pub const MAX_NIGHTLY_TRIES: usize = 7;
/// Largest `gates.json` read from a downloaded artifact.
pub const MAX_GATES_BYTES: u64 = 1 << 20;
/// Where `gh run download` writes, relative to the repo root (removed before and after).
pub const GATES_DIR: &str = "target/dashboard-gates";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub until: Option<i64>,
    pub out: PathBuf,
    pub from: Option<PathBuf>,
    pub index: Option<PathBuf>,
}

/// Parses the arguments after `dashboard`.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        until: None,
        out: PathBuf::from(SNAPSHOT),
        from: None,
        index: None,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--until" => {
                let v = value()?;
                opts.until =
                    Some(parse_date(v).ok_or_else(|| format!("--until: `{v}` is not YYYY-MM-DD"))?);
            }
            "--out" => opts.out = PathBuf::from(value()?),
            "--from" => opts.from = Some(PathBuf::from(value()?)),
            "--index" => opts.index = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(opts)
}

/// The Monday of the ISO week holding `day` (day 0, 1970-01-01, is a Thursday).
pub fn monday(day: i64) -> i64 {
    day.saturating_sub(day.saturating_add(3).rem_euclid(7))
}

/// The twelve weeks ending with the one that holds `until`: (Monday, last day ≤ `until`).
pub fn weeks(until: i64) -> Vec<(i64, i64)> {
    let last = monday(until);
    (0..WEEKS)
        .rev()
        .map(|i| {
            let start = last.saturating_sub(i.saturating_mul(7));
            (start, start.saturating_add(6).min(until))
        })
        .collect()
}

/// The gh arguments for one week's merged PRs (no spaces inside an argument).
pub fn prs_args(start: i64, end: i64) -> String {
    format!(
        "pr list --state merged --search merged:{}..{} --limit {LIST_LIMIT} --json number,mergedAt,comments",
        fmt_day(start),
        fmt_day(end)
    )
}

/// The gh arguments for every issue (no spaces inside an argument).
pub fn issues_args() -> String {
    format!(
        "issue list --state all --limit {LIST_LIMIT} --json number,title,state,createdAt,closedAt,labels,milestone"
    )
}

pub const JOURNAL_LOG_ARGS: &[&str] = &[
    "log",
    "--first-parent",
    "--diff-filter=A",
    "--name-only",
    "--format=@@%ct%x09%s",
    "origin/main",
    "--",
    "ops/journal/",
];

/// The events whose `ci` runs are nightlies (`.github/workflows/ci.yml`'s `NIGHTLY`).
pub const NIGHTLY_EVENTS: [&str; 2] = ["schedule", "workflow_dispatch"];

/// The gh arguments for one event's nightly runs (no spaces inside an argument).
pub fn nightly_runs_args(event: &str) -> String {
    format!(
        "run list --workflow ci.yml --event {event} --branch main --status completed --limit {NIGHTLY_LIMIT} --json databaseId,createdAt"
    )
}

/// Every listed nightly run, from the `(name, text)` lists.
fn nightly_runs(lists: &[(String, String)]) -> Result<Vec<Value>, String> {
    let mut all = Vec::new();
    for (name, text) in lists {
        let list = parse_list(text, name)?;
        all.extend(array(&list, name)?.iter().cloned());
    }
    Ok(all)
}

/// The gh arguments that download one run's `gates-report` artifact into `dir`.
pub fn download_args(run: u64, dir: &str) -> Vec<String> {
    [
        "run",
        "download",
        &run.to_string(),
        "-n",
        "gates-report",
        "-D",
        dir,
    ]
    .map(str::to_owned)
    .to_vec()
}

/// Every input, as fetched or read with `--from`.
#[derive(Debug, Default, Clone)]
pub struct Inputs {
    /// One `gh pr list` output per week, oldest first.
    pub prs: Vec<String>,
    pub issues: String,
    pub journal_log: String,
    pub index_tsv: Option<String>,
    /// `(file name, text)` of each `runs-<event>.json`; `None` only for a `--from` directory
    /// with neither file.
    pub nightly_runs: Option<Vec<(String, String)>>,
    /// `gates.json` text by run id, for the runs whose download succeeded.
    pub gates: BTreeMap<u64, String>,
}

/// Each week's nightly runs to try, newest first (ties: the larger id), at most
/// [`MAX_NIGHTLY_TRIES`]; one list per entry of `weeks`.
pub fn candidates(runs: &[Value], weeks: &[(i64, i64)]) -> Result<Vec<Vec<u64>>, String> {
    let what = "nightly-runs.json";
    let mut dated = Vec::new();
    for r in runs {
        dated.push((
            day_of(r, "createdAt", what)?,
            string(r, "createdAt", what)?,
            num(r, "databaseId", what)?,
        ));
    }
    // Newest first: the timestamp text sorts as time (one fixed shape, checked by day_of).
    dated.sort_by(|a, b| b.1.cmp(a.1).then(b.2.cmp(&a.2)));
    Ok(weeks
        .iter()
        .map(|&(start, end)| {
            dated
                .iter()
                .filter(|(day, _, _)| (start..=end).contains(day))
                .map(|&(_, _, id)| id)
                .take(MAX_NIGHTLY_TRIES)
                .collect()
        })
        .collect())
}

/// The judge's smoke metrics of one week (all `None` without a report).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JudgeWeek {
    pub start: i64,
    pub run: Option<u64>,
    pub day: Option<i64>,
    pub open_rate: Option<f64>,
    pub render_match: Option<f64>,
    pub text_f1: Option<f64>,
    pub crashes: Option<u64>,
    pub journeys_passing: Option<u64>,
    pub journeys: Option<u64>,
}

/// A rate from 0 to 1; absent or `null` is `None`.
fn rate(m: &Value, key: &str, what: &str) -> Result<Option<f64>, String> {
    match m.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .parse::<f64>()
            .ok()
            .filter(|x| x.is_finite() && (0.0..=1.0).contains(x))
            .map(Some)
            .ok_or_else(|| format!("{what}: `{key}` is not a rate from 0 to 1")),
        Some(_) => Err(format!("{what}: `{key}` is not a number")),
    }
}

/// A count; absent or `null` is `None`.
fn count(m: &Value, key: &str, what: &str) -> Result<Option<u64>, String> {
    match m.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => num(m, key, what).map(Some),
    }
}

/// Reads the six smoke metrics of one `gates.json`.
pub fn gate_metrics(text: &str, what: &str) -> Result<JudgeWeek, String> {
    let v = parse_list(text, what)?;
    let m = match v.get("metrics") {
        Some(m @ Value::Object(_)) => m,
        _ => return Err(format!("{what}: no `metrics` object")),
    };
    Ok(JudgeWeek {
        open_rate: rate(m, "smoke_open_rate", what)?,
        render_match: rate(m, "smoke_render_match", what)?,
        text_f1: rate(m, "smoke_text_f1", what)?,
        crashes: count(m, "smoke_crashes", what)?,
        journeys_passing: count(m, "smoke_journeys_passing", what)?,
        journeys: count(m, "smoke_journeys", what)?,
        ..JudgeWeek::default()
    })
}

/// The judge history: per week, the newest run whose `gates.json` is in `inp.gates`.
fn judge_history(inp: &Inputs, weeks: &[(i64, i64)]) -> Result<Option<Vec<JudgeWeek>>, String> {
    let Some(lists) = &inp.nightly_runs else {
        return Ok(None);
    };
    let all = nightly_runs(lists)?;
    let runs = all.as_slice();
    let days: BTreeMap<u64, i64> = runs
        .iter()
        .map(|r| {
            Ok((
                num(r, "databaseId", "nightly-runs.json")?,
                day_of(r, "createdAt", "nightly-runs.json")?,
            ))
        })
        .collect::<Result<_, String>>()?;
    let mut out = Vec::new();
    for (&(start, _), ids) in weeks.iter().zip(candidates(runs, weeks)?) {
        let found = ids
            .into_iter()
            .find_map(|id| inp.gates.get(&id).map(|t| (id, t)));
        out.push(match found {
            Some((id, t)) => JudgeWeek {
                start,
                run: Some(id),
                day: days.get(&id).copied(),
                ..gate_metrics(t, &format!("gates-{id}.json"))?
            },
            None => JudgeWeek {
                start,
                ..JudgeWeek::default()
            },
        });
    }
    Ok(Some(out))
}

/// `ops/journal/YYYY-MM-DD/<name>.md`: one session's entry (ops/TICK.md §5).
fn is_session_entry(path: &str) -> bool {
    let Some(rest) = path.strip_prefix("ops/journal/") else {
        return false;
    };
    let Some((day, name)) = rest.split_once('/') else {
        return false;
    };
    parse_date(day).is_some() && name.len() > 3 && name.ends_with(".md") && !name.contains('/')
}

/// A squash-merge subject ends in `(#N)`.
fn subject_pr(subject: &str) -> Option<u64> {
    let n = subject.strip_suffix(')')?.rsplit_once("(#")?.1;
    (!n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
        .then(|| n.parse().ok())
        .flatten()
}

/// Journal entries per merged PR, for commits dated `since..=until` (UTC days).
fn journal_entries(text: &str, since: i64, until: i64) -> Result<Vec<u64>, String> {
    let mut out = Vec::new();
    // (counted, entries) of the commit being read.
    let mut cur: Option<(bool, u64)> = None;
    for (i, line) in text.lines().enumerate() {
        if let Some(rest) = line.strip_prefix("@@") {
            if let Some((true, n)) = cur.take()
                && n > 0
            {
                out.push(n);
            }
            let bad = || {
                format!(
                    "journal-log.txt line {}: unexpected `{line}`",
                    i.saturating_add(1)
                )
            };
            let (secs, subject) = rest.split_once('\t').ok_or_else(bad)?;
            let day = crate::metrics::utc_day(secs).ok_or_else(bad)?;
            let counted = (since..=until).contains(&day) && subject_pr(subject).is_some();
            cur = Some((counted, 0));
            continue;
        }
        if line.is_empty() {
            continue;
        }
        match cur.as_mut() {
            Some((_, n)) if is_session_entry(line) => *n = n.saturating_add(1),
            Some(_) => {}
            None => {
                return Err(format!(
                    "journal-log.txt line {}: a path before any commit",
                    i.saturating_add(1)
                ));
            }
        }
    }
    if let Some((true, n)) = cur
        && n > 0
    {
        out.push(n);
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Week {
    pub start: i64,
    pub end: i64,
    pub merged_prs: u64,
    pub median_rounds: Option<u64>,
    pub sessions: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Milestone {
    pub title: String,
    pub open: u64,
    pub closed: u64,
    pub closed_28d: u64,
    pub forecast: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub generated: i64,
    pub until: i64,
    pub weeks: Vec<Week>,
    pub has_index: bool,
    /// Entries per merged PR that had at least one.
    pub entries: Vec<u64>,
    /// (number, title, age in days)
    pub blocked: Vec<(u64, String, i64)>,
    pub milestones: Vec<Milestone>,
    /// `None`: no nightly run list (`source` `missing`).
    pub judge: Option<Vec<JudgeWeek>>,
    pub truncated: Vec<String>,
    pub malformed_index_rows: u64,
}

/// `until` + ⌈open × 28 / closed⌉ days, or None (nothing open, no closures, past 9999).
pub fn forecast(until: i64, open: u64, closed_28d: u64) -> Option<i64> {
    if open == 0 || closed_28d == 0 {
        return None;
    }
    let days = open
        .checked_mul(RATE_DAYS.unsigned_abs())?
        .div_ceil(closed_28d);
    let day = until.checked_add(i64::try_from(days).ok()?)?;
    (day <= parse_date("9999-12-31")?).then_some(day)
}

/// `M<n>` by number first, then every other title.
fn milestone_key(title: &str) -> (u8, u64, String) {
    match title.strip_prefix('M').and_then(|n| {
        (!n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
            .then(|| n.parse::<u64>().ok())
            .flatten()
    }) {
        Some(n) => (0, n, String::new()),
        None => (1, 0, title.to_owned()),
    }
}

fn milestones(issues: &[Value], until: i64) -> Result<Vec<Milestone>, String> {
    let window = until.saturating_sub(RATE_DAYS.saturating_sub(1))..=until;
    let mut by: BTreeMap<(u8, u64, String), Milestone> = BTreeMap::new();
    for issue in issues {
        let what = "issues.json";
        let Some(title) = issue
            .get("milestone")
            .and_then(|m| m.get("title"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let m = by.entry(milestone_key(title)).or_insert_with(|| Milestone {
            title: title.to_owned(),
            open: 0,
            closed: 0,
            closed_28d: 0,
            forecast: None,
        });
        if string(issue, "state", what)? == "CLOSED" {
            m.closed = m.closed.saturating_add(1);
            if window.contains(&day_of(issue, "closedAt", what)?) {
                m.closed_28d = m.closed_28d.saturating_add(1);
            }
        } else {
            m.open = m.open.saturating_add(1);
        }
    }
    Ok(by
        .into_values()
        .map(|mut m| {
            m.forecast = forecast(until, m.open, m.closed_28d);
            m
        })
        .collect())
}

/// Builds the snapshot from the inputs (`inp.prs` holds one list per entry of `weeks(until)`).
pub fn compute(inp: &Inputs, until: i64, generated: i64) -> Result<Snapshot, String> {
    let weeks_list = weeks(until);
    let mut truncated = Vec::new();
    let mut out_weeks = Vec::new();
    for (i, &(start, end)) in weeks_list.iter().enumerate() {
        let name = format!("prs-{}.json", fmt_day(start));
        let text = inp.prs.get(i).ok_or_else(|| format!("{name}: missing"))?;
        let list = parse_list(text, &name)?;
        let prs = array(&list, &name)?;
        if prs.len() >= LIST_LIMIT {
            truncated.push(format!("prs-{}", fmt_day(start)));
        }
        let mut merged = 0u64;
        let mut rounds = Vec::new();
        for pr in prs {
            let number = num(pr, "number", &name)?;
            let day = day_of(pr, "mergedAt", &name)?;
            if day < start || day > end {
                continue;
            }
            merged = merged.saturating_add(1);
            let comments = pr
                .get("comments")
                .and_then(Value::as_array)
                .unwrap_or_default();
            if let Some(r) = review_of(number, comments) {
                rounds.push(r.rounds);
            }
        }
        let sessions = inp
            .index_tsv
            .as_deref()
            .map(|t| parse_sessions(t, start, end).count);
        out_weeks.push(Week {
            start,
            end,
            merged_prs: merged,
            median_rounds: median(&mut rounds),
            sessions,
        });
    }
    let first = weeks_list.first().map_or(until, |w| w.0);
    let entries = journal_entries(&inp.journal_log, first, until)?;

    let list = parse_list(&inp.issues, "issues.json")?;
    let issues = array(&list, "issues.json")?;
    if issues.len() >= LIST_LIMIT {
        truncated.push("issues".to_owned());
    }
    for (name, text) in inp.nightly_runs.iter().flatten() {
        if array(&parse_list(text, name)?, name)?.len() >= NIGHTLY_LIMIT {
            truncated.push(name.trim_end_matches(".json").to_owned());
        }
    }
    let mut blocked = Vec::new();
    for issue in issues {
        let what = "issues.json";
        let number = num(issue, "number", what)?;
        let title = string(issue, "title", what)?;
        let created = day_of(issue, "createdAt", what)?;
        let open = string(issue, "state", what)? != "CLOSED";
        if !open {
            // Required on every closed issue, milestone or not (the spec's Errors).
            day_of(issue, "closedAt", what)?;
        }
        if open && label_names(issue).contains(&"blocked") {
            blocked.push((
                number,
                title.to_owned(),
                until.saturating_sub(created).max(0),
            ));
        }
    }
    blocked.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    let malformed_index_rows = inp
        .index_tsv
        .as_deref()
        .map_or(0, |t| parse_sessions(t, 0, 0).malformed);
    Ok(Snapshot {
        generated,
        until,
        weeks: out_weeks,
        has_index: inp.index_tsv.is_some(),
        entries,
        blocked,
        milestones: milestones(issues, until)?,
        judge: judge_history(inp, &weeks_list)?,
        truncated,
        malformed_index_rows,
    })
}

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or_else(|| "null".to_owned(), |v| v.to_string())
}

fn opt_day(v: Option<i64>) -> String {
    v.map_or_else(|| "null".to_owned(), |d| json_str(&fmt_day(d)))
}

/// The snapshot as JSON (one field per line at the top level, so diffs stay readable).
pub fn render(s: &Snapshot) -> String {
    let mut o = String::from("{\n");
    let _ = writeln!(o, "  \"version\": 1,");
    let _ = writeln!(o, "  \"generated\": {},", json_str(&fmt_day(s.generated)));
    let _ = writeln!(o, "  \"until\": {},", json_str(&fmt_day(s.until)));
    let weeks: Vec<String> = s
        .weeks
        .iter()
        .map(|w| {
            format!(
                "    {{\"start\": {}, \"end\": {}, \"merged_prs\": {}, \"median_rounds\": {}, \"sessions\": {}}}",
                json_str(&fmt_day(w.start)),
                json_str(&fmt_day(w.end)),
                w.merged_prs,
                opt(w.median_rounds),
                opt(w.sessions)
            )
        })
        .collect();
    let _ = writeln!(o, "  \"weeks\": [\n{}\n  ],", weeks.join(",\n"));
    let source = if s.has_index { "index" } else { "missing" };
    let _ = writeln!(o, "  \"sessions\": \"{source}\",");
    let mut sorted = s.entries.clone();
    let max = sorted.iter().copied().max();
    let med = median(&mut sorted);
    let hist: Vec<String> = (1..=4u64)
        .map(|k| {
            let n = s
                .entries
                .iter()
                .filter(|&&e| if k == 4 { e >= 4 } else { e == k })
                .count();
            format!("{{\"entries\": {k}, \"prs\": {n}}}")
        })
        .collect();
    let _ = writeln!(
        o,
        "  \"journal_entries\": {{\"prs\": {}, \"median\": {}, \"max\": {}, \"histogram\": [{}]}},",
        s.entries.len(),
        opt(med),
        opt(max),
        hist.join(", ")
    );
    let blocked: Vec<String> = s
        .blocked
        .iter()
        .map(|(n, t, a)| {
            format!(
                "    {{\"number\": {n}, \"title\": {}, \"age_days\": {a}}}",
                json_str(t)
            )
        })
        .collect();
    let _ = writeln!(o, "  \"blocked\": [{}],", wrap(&blocked));
    let ms: Vec<String> = s
        .milestones
        .iter()
        .map(|m| {
            format!(
                "    {{\"title\": {}, \"open\": {}, \"closed\": {}, \"closed_28d\": {}, \"forecast\": {}}}",
                json_str(&m.title),
                m.open,
                m.closed,
                m.closed_28d,
                opt_day(m.forecast)
            )
        })
        .collect();
    let _ = writeln!(o, "  \"milestones\": [{}],", wrap(&ms));
    let empty: Vec<JudgeWeek> = s
        .weeks
        .iter()
        .map(|w| JudgeWeek {
            start: w.start,
            ..JudgeWeek::default()
        })
        .collect();
    let jw: Vec<String> = s
        .judge
        .as_ref()
        .unwrap_or(&empty)
        .iter()
        .map(|w| {
            format!(
                "    {{\"start\": {}, \"run\": {}, \"day\": {}, \"open_rate\": {}, \"render_match\": {}, \"text_f1\": {}, \"crashes\": {}, \"journeys_passing\": {}, \"journeys\": {}}}",
                json_str(&fmt_day(w.start)),
                opt(w.run),
                opt_day(w.day),
                opt(w.open_rate),
                opt(w.render_match),
                opt(w.text_f1),
                opt(w.crashes),
                opt(w.journeys_passing),
                opt(w.journeys)
            )
        })
        .collect();
    let source = if s.judge.is_some() {
        "nightly"
    } else {
        "missing"
    };
    let _ = writeln!(
        o,
        "  \"judge\": {{\"source\": \"{source}\", \"weeks\": [{}]}},",
        wrap(&jw)
    );
    let tr: Vec<String> = s.truncated.iter().map(|t| json_str(t)).collect();
    let _ = writeln!(o, "  \"truncated\": [{}],", tr.join(", "));
    let _ = writeln!(o, "  \"malformed_index_rows\": {}", s.malformed_index_rows);
    o.push_str("}\n");
    o
}

fn wrap(items: &[String]) -> String {
    if items.is_empty() {
        String::new()
    } else {
        format!("\n{}\n  ", items.join(",\n"))
    }
}

/// Reads or fetches every input.
pub fn inputs(repo: &Path, opts: &Options, until: i64) -> Result<Inputs, String> {
    let from = opts.from.as_deref();
    let mut prs = Vec::new();
    for (start, end) in weeks(until) {
        prs.push(match from {
            Some(dir) => read(dir, &format!("prs-{}.json", fmt_day(start)))?,
            None => gh(&prs_args(start, end))?,
        });
    }
    let issues = match from {
        Some(dir) => read(dir, "issues.json")?,
        None => gh(&issues_args())?,
    };
    let journal_log = match from {
        Some(dir) => read(dir, "journal-log.txt")?,
        None => {
            crate::macos::capture(
                "git",
                &[&["-C", &repo.to_string_lossy()], JOURNAL_LOG_ARGS].concat(),
                None,
            )?
            .0
        }
    };
    let index_tsv = match (from, &opts.index) {
        (_, Some(file)) => read_optional(file)?,
        (Some(dir), None) => read_optional(&dir.join("index.tsv"))?,
        (None, None) => read_optional(&repo.join("ops/logs/index.tsv"))?,
    };
    let mut lists = Vec::new();
    for event in NIGHTLY_EVENTS {
        let name = format!("runs-{event}.json");
        let text = match from {
            Some(dir) => read_optional(&dir.join(&name))?,
            None => Some(gh(&nightly_runs_args(event))?),
        };
        if let Some(t) = text {
            lists.push((name, t));
        }
    }
    let nightly_runs = (!lists.is_empty()).then_some(lists);
    let mut gates = BTreeMap::new();
    if let Some(lists) = &nightly_runs {
        let ids = candidates(&self::nightly_runs(lists)?, &weeks(until))?;
        let scratch = repo.join(GATES_DIR);
        if from.is_none() {
            remove_tree(&scratch)?;
        }
        // The tree is removed after an error too (a bad artifact), then the error returned.
        let fetched = fetch_gates(from, &scratch, ids, &mut gates);
        if from.is_none() {
            remove_tree(&scratch)?;
        }
        fetched?;
    }
    Ok(Inputs {
        prs,
        issues,
        journal_log,
        index_tsv,
        nightly_runs,
        gates,
    })
}

/// For each week, the first candidate whose `gates.json` can be had: read from `--from`
/// (`gates-<id>.json`, a missing file counting as a failed download) or downloaded.
fn fetch_gates(
    from: Option<&Path>,
    scratch: &Path,
    ids: Vec<Vec<u64>>,
    gates: &mut BTreeMap<u64, String>,
) -> Result<(), String> {
    for week in ids {
        for id in week {
            let got = match from {
                Some(dir) => match read_gates(&dir.join(format!("gates-{id}.json"))) {
                    Ok(t) => Some(t),
                    Err(GatesError::Missing) => None,
                    Err(GatesError::Bad(e)) => return Err(e),
                },
                None => download(scratch, id)?,
            };
            if let Some(t) = got {
                gates.insert(id, t);
                break;
            }
        }
    }
    Ok(())
}

fn remove_tree(p: &Path) -> Result<(), String> {
    match fs::remove_dir_all(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("remove {}: {e}", p.display())),
    }
}

/// Downloads run `id`'s `gates-report` into `scratch/<id>` and reads its `gates.json`; a
/// failed download is `None` (reported on stderr).
fn download(scratch: &Path, id: u64) -> Result<Option<String>, String> {
    let dir = scratch.join(id.to_string());
    let args = download_args(id, &dir.to_string_lossy());
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if let Err(e) = crate::macos::capture("gh", &args, None) {
        eprintln!(
            "dashboard: run {id}: no gates-report ({})",
            e.lines().next().unwrap_or("")
        );
        return Ok(None);
    }
    match read_gates(&dir.join("gates.json")) {
        Ok(t) => Ok(Some(t)),
        // The download succeeded, so a missing file is a bad artifact.
        Err(GatesError::Missing) => Err(format!("run {id}: the gates-report has no gates.json")),
        Err(GatesError::Bad(e)) => Err(e),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatesError {
    /// No file at the path.
    Missing,
    Bad(String),
}

/// Reads a `gates.json`: a regular file (not a symlink, and the opened file regular too) of
/// at most [`MAX_GATES_BYTES`].
pub fn read_gates(p: &Path) -> Result<String, GatesError> {
    use std::io::Read as _;
    let bad = |what: &str, e: &dyn std::fmt::Display| {
        GatesError::Bad(format!("{what} {}: {e}", p.display()))
    };
    let meta = match fs::symlink_metadata(p) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(GatesError::Missing),
        Err(e) => return Err(bad("read", &e)),
    };
    let not_regular = || GatesError::Bad(format!("{}: not a regular file", p.display()));
    if !meta.is_file() {
        return Err(not_regular());
    }
    let file = jpdf_fsio::open_nonblocking(p).map_err(|e| bad("open", &e))?;
    // The path could have changed between the two calls: check what was opened.
    if !file.metadata().map_err(|e| bad("read", &e))?.is_file() {
        return Err(not_regular());
    }
    let mut bytes = Vec::new();
    file.take(MAX_GATES_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| bad("read", &e))?;
    if u64::try_from(bytes.len()).map_or(true, |n| n > MAX_GATES_BYTES) {
        return Err(GatesError::Bad(format!(
            "{}: larger than {MAX_GATES_BYTES} bytes",
            p.display()
        )));
    }
    String::from_utf8(bytes).map_err(|_| GatesError::Bad(format!("{}: not UTF-8", p.display())))
}

/// Writes the snapshot to `opts.out`; returns its path.
pub fn run(repo: &Path, opts: &Options) -> Result<PathBuf, String> {
    let generated = today()?;
    let until = opts.until.unwrap_or(generated);
    let inp = inputs(repo, opts, until)?;
    let snap = compute(&inp, until, generated)?;
    if let Some(dir) = opts.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    fs::write(&opts.out, render(&snap))
        .map_err(|e| format!("write {}: {e}", opts.out.display()))?;
    Ok(opts.out.clone())
}

/// What [`copy_into_site`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Copied {
    Yes,
    /// No `site/dashboard/` (the web build failed or didn't run).
    NoPage,
    /// No committed snapshot.
    NoSnapshot,
}

/// The `ci linux` step: copies the committed snapshot to `site/dashboard/data.json` when the
/// web build wrote `site/dashboard/`.
pub fn copy_into_site(repo: &Path, site: &Path) -> Result<Copied, String> {
    use std::io::Read as _;
    let dir = site.join("dashboard");
    if !dir.is_dir() {
        return Ok(Copied::NoPage);
    }
    let src = repo.join(SNAPSHOT);
    let file = match jpdf_fsio::open_nonblocking(&src) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Copied::NoSnapshot),
        Err(e) => return Err(format!("open {}: {e}", src.display())),
    };
    let meta = file
        .metadata()
        .map_err(|e| format!("open {}: {e}", src.display()))?;
    if !meta.is_file() {
        return Err(format!("{}: not a regular file", src.display()));
    }
    let mut bytes = Vec::new();
    file.take(MAX_SNAPSHOT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read {}: {e}", src.display()))?;
    if u64::try_from(bytes.len()).map_or(true, |n| n > MAX_SNAPSHOT_BYTES) {
        return Err(format!(
            "{}: larger than {MAX_SNAPSHOT_BYTES} bytes",
            src.display()
        ));
    }
    let dst = dir.join("data.json");
    fs::write(&dst, bytes).map_err(|e| format!("write {}: {e}", dst.display()))?;
    Ok(Copied::Yes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> i64 {
        parse_date(s).unwrap_or(i64::MIN)
    }

    fn pr(n: u64, merged: &str, rounds: Option<u64>) -> String {
        let comments = rounds.map_or_else(String::new, |k| {
            format!(
                r#"{{"body": "🤖 jpdf-agent **AI review, round {k}** of abc\n\nR{k}.1 [minor] a:1: x"}}"#
            )
        });
        format!(r#"{{"number": {n}, "mergedAt": "{merged}T12:00:00Z", "comments": [{comments}]}}"#)
    }

    /// One PR list per week of `weeks(until)`, each holding the PRs given for its Monday.
    fn prs_for(until: i64, by_week: &[(&str, Vec<String>)]) -> Vec<String> {
        weeks(until)
            .iter()
            .map(|(start, _)| {
                let items: Vec<String> = by_week
                    .iter()
                    .filter(|(m, _)| d(m) == *start)
                    .flat_map(|(_, v)| v.clone())
                    .collect();
                format!("[{}]", items.join(","))
            })
            .collect()
    }

    fn issue(
        n: u64,
        state: &str,
        created: &str,
        closed: Option<&str>,
        labels: &[&str],
        ms: Option<&str>,
    ) -> String {
        let labels: Vec<String> = labels
            .iter()
            .map(|l| format!(r#"{{"name": "{l}"}}"#))
            .collect();
        let closed = closed.map_or_else(|| "null".to_owned(), |c| format!(r#""{c}T08:00:00Z""#));
        let ms = ms.map_or_else(|| "null".to_owned(), |t| format!(r#"{{"title": "{t}"}}"#));
        format!(
            r#"{{"number": {n}, "title": "issue \"{n}\"", "state": "{state}", "createdAt": "{created}T08:00:00Z", "closedAt": {closed}, "labels": [{}], "milestone": {ms}}}"#,
            labels.join(",")
        )
    }

    fn inputs_with(
        prs: Vec<String>,
        issues: &[String],
        journal: &str,
        index: Option<&str>,
    ) -> Inputs {
        Inputs {
            prs,
            issues: format!("[{}]", issues.join(",")),
            journal_log: journal.to_owned(),
            index_tsv: index.map(str::to_owned),
            ..Inputs::default()
        }
    }

    const UNTIL: &str = "2026-11-25"; // a Wednesday

    #[test]
    fn weeks_are_twelve_iso_weeks_ending_with_until() {
        assert_eq!(
            monday(d("2026-11-23")),
            d("2026-11-23"),
            "a Monday is its own week start"
        );
        assert_eq!(
            monday(d("2026-11-29")),
            d("2026-11-23"),
            "Sunday belongs to the week before"
        );
        assert_eq!(monday(d("2026-11-30")), d("2026-11-30"));
        assert_eq!(monday(0), d("1969-12-29"), "1970-01-01 was a Thursday");
        let w = weeks(d(UNTIL));
        assert_eq!(w.len(), 12);
        assert_eq!(w.first(), Some(&(d("2026-09-07"), d("2026-09-13"))));
        assert_eq!(w.get(10), Some(&(d("2026-11-16"), d("2026-11-22"))));
        assert_eq!(
            w.last(),
            Some(&(d("2026-11-23"), d(UNTIL))),
            "the last week ends at --until"
        );
        let sunday = weeks(d("2026-11-29"));
        assert_eq!(sunday.last(), Some(&(d("2026-11-23"), d("2026-11-29"))));
    }

    #[test]
    fn command_arguments_are_pinned() {
        assert_eq!(
            prs_args(d("2026-11-23"), d(UNTIL)),
            "pr list --state merged --search merged:2026-11-23..2026-11-25 --limit 1000 --json number,mergedAt,comments"
        );
        assert_eq!(
            issues_args(),
            "issue list --state all --limit 1000 --json number,title,state,createdAt,closedAt,labels,milestone"
        );
        assert_eq!(
            JOURNAL_LOG_ARGS.join(" "),
            "log --first-parent --diff-filter=A --name-only --format=@@%ct%x09%s origin/main -- ops/journal/"
        );
    }

    #[test]
    fn weekly_merges_rounds_and_sessions() {
        let until = d(UNTIL);
        let prs = prs_for(
            until,
            &[
                // Monday and Sunday are in their week; a list's stray entry from the next
                // Monday is not (gh's search is by day, this guards the week bounds).
                (
                    "2026-11-16",
                    vec![
                        pr(1, "2026-11-16", Some(1)),
                        pr(2, "2026-11-22", Some(4)),
                        pr(3, "2026-11-22", None),
                        pr(4, "2026-11-23", Some(9)),
                        pr(7, "2026-11-15", Some(9)), // the Sunday before: the week before's
                    ],
                ),
                // The last week: --until is in, the day after is out.
                (
                    "2026-11-23",
                    vec![pr(5, UNTIL, Some(2)), pr(6, "2026-11-26", Some(7))],
                ),
            ],
        );
        let index = "20261116T010000Z\t0\ts1\t60s\n20261122T235959Z\t1\ts2\t60s\n20261123T000000Z\t0\ts3\t60s\nbad row\n";
        let snap = compute(
            &inputs_with(prs.clone(), &[], "", Some(index)),
            until,
            until,
        );
        let snap = snap.unwrap_or_else(|e| panic!("{e}"));
        let w = |i: usize| snap.weeks.get(i).cloned();
        let prev = w(10).unwrap_or_else(|| panic!("week 10"));
        assert_eq!(prev.merged_prs, 3);
        assert_eq!(
            prev.median_rounds,
            Some(4),
            "median of rounds 1 and 4 (upper middle); PR 3 has no review"
        );
        assert_eq!(prev.sessions, Some(2));
        let last = w(11).unwrap_or_else(|| panic!("week 11"));
        assert_eq!(
            (last.merged_prs, last.median_rounds, last.sessions),
            (1, Some(2), Some(1))
        );
        let empty = w(0).unwrap_or_else(|| panic!("week 0"));
        assert_eq!(
            (empty.merged_prs, empty.median_rounds, empty.sessions),
            (0, None, Some(0))
        );
        assert!(snap.has_index);
        assert_eq!(snap.malformed_index_rows, 1);

        let none = compute(&inputs_with(prs, &[], "", None), until, until)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(!none.has_index);
        assert!(
            none.weeks.iter().all(|w| w.sessions.is_none()),
            "no index: missing, not 0"
        );
        let json = render(&none);
        assert!(json.contains("\"sessions\": \"missing\""));
        assert!(json.contains("\"sessions\": null}"));
    }

    #[test]
    fn journal_entries_per_merged_pr() {
        // git log prints newest first: header, blank line, the added paths.
        let log = "@@1795651200\tthree sessions (#30)\n\nops/journal/2026-11-25/0100-30-a.md\nops/journal/2026-11-25/0200-30-b.md\nops/journal/2026-11-26/0300-30-c.md\n\
@@1795564800\tday file only (#29)\n\nops/journal/2026-11-24.md\n\
@@1795564800\tnot a squash subject (#28) more\n\nops/journal/2026-11-24/x.md\n\
@@1795564800\tone (#27)\n\nops/journal/2026-11-24/x.md\nops/journal/2026-11-24/sub/y.md\nops/journal/notes/z.md\n\
@@1788739200\tbefore the first week (#26)\n\nops/journal/2026-09-07/x.md\n\
@@1788652800\ttoo old (#25)\n\nops/journal/2026-09-06/x.md\n";
        // 1795651200 = 2026-11-26 (after --until), 1795564800 = 2026-11-25, 1788739200 = 2026-09-07.
        let until = d(UNTIL);
        let first = d("2026-09-07");
        let got = journal_entries(log, first, until).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            got,
            vec![1, 1],
            "#27 (sub/ and a non-date directory are no entries) and #26 (first Monday) count"
        );
        let later = journal_entries(log, first, d("2026-11-26")).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(later, vec![3, 1, 1]);
        assert!(journal_entries("ops/journal/2026-11-24/x.md\n", first, until).is_err());
        assert!(journal_entries("@@+1\t(#1)\n", first, until).is_err());
        assert!(journal_entries("@@1795564800 (#1)\n", first, until).is_err());
        assert_eq!(subject_pr("x (#12)"), Some(12));
        assert_eq!(subject_pr("x (#)"), None);
        assert_eq!(subject_pr("x (#1a)"), None);

        let snap = Snapshot {
            generated: until,
            until,
            weeks: Vec::new(),
            has_index: false,
            entries: vec![1, 2, 2, 5, 4, 3],
            blocked: Vec::new(),
            milestones: Vec::new(),
            judge: None,
            truncated: Vec::new(),
            malformed_index_rows: 0,
        };
        let json = render(&snap);
        assert!(
            json.contains(r#""journal_entries": {"prs": 6, "median": 3, "max": 5, "histogram": [{"entries": 1, "prs": 1}, {"entries": 2, "prs": 2}, {"entries": 3, "prs": 1}, {"entries": 4, "prs": 2}]}"#),
            "{json}"
        );
        let empty = render(&Snapshot {
            entries: Vec::new(),
            ..snap
        });
        assert!(
            empty.contains(r#""journal_entries": {"prs": 0, "median": null, "max": null"#),
            "{empty}"
        );
    }

    #[test]
    fn blocked_issues_and_milestone_forecast() {
        let until = d(UNTIL);
        let issues = [
            issue(12, "OPEN", "2026-11-20", None, &["blocked"], None), // same age as #1, listed first (gh is newest first): sorted by number
            issue(1, "OPEN", "2026-11-20", None, &["blocked"], Some("M1")),
            issue(
                2,
                "CLOSED",
                "2026-11-01",
                Some("2026-10-29"),
                &["blocked"],
                Some("M1"),
            ), // day −27: in
            issue(
                3,
                "CLOSED",
                "2026-10-01",
                Some("2026-10-28"),
                &[],
                Some("M1"),
            ), // day −28: out
            issue(4, "OPEN", "2026-11-10", None, &["blocked-x"], Some("M10")),
            issue(5, "OPEN", "2026-11-10", None, &["blocked"], None),
            issue(6, "OPEN", "2026-11-26", None, &["blocked"], Some("Ops")), // created after --until
            issue(7, "CLOSED", "2026-11-01", Some(UNTIL), &[], Some("M2")),
            issue(
                8,
                "CLOSED",
                "2026-11-01",
                Some("2026-11-26"),
                &[],
                Some("M2"),
            ), // after --until
            issue(9, "OPEN", "2026-11-01", None, &[], Some("M2")),
            issue(10, "OPEN", "2026-11-01", None, &[], Some("M2")),
        ];
        let snap = compute(
            &inputs_with(prs_for(until, &[]), &issues, "", None),
            until,
            until,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let blocked: Vec<(u64, i64)> = snap.blocked.iter().map(|b| (b.0, b.2)).collect();
        assert_eq!(
            blocked,
            vec![(5, 15), (1, 5), (12, 5), (6, 0)],
            "oldest first; closed and blocked-x left out"
        );
        let ms: Vec<(&str, u64, u64, u64, Option<i64>)> = snap
            .milestones
            .iter()
            .map(|m| (m.title.as_str(), m.open, m.closed, m.closed_28d, m.forecast))
            .collect();
        assert_eq!(
            ms,
            vec![
                // M1: 1 open, 1 closure in 28 days → 28 days.
                ("M1", 1, 2, 1, Some(until + 28)),
                // M2: 2 open, 1 closure in the window (the one after --until is out) → 56 days.
                ("M2", 2, 2, 1, Some(until + 56)),
                ("M10", 1, 0, 0, None),
                ("Ops", 1, 0, 0, None),
            ]
        );
        let json = render(&snap);
        assert!(json.contains(r#"{"title": "M1", "open": 1, "closed": 2, "closed_28d": 1, "forecast": "2026-12-23"}"#), "{json}");
        assert!(
            json.contains(r#""title": "issue \"5\"", "age_days": 15"#),
            "titles are escaped: {json}"
        );
        assert!(json_parses(&json));
    }

    fn json_parses(s: &str) -> bool {
        jpdf_testgen::json::parse(s).is_ok()
    }

    #[test]
    fn forecast_rounds_up_and_has_no_value_without_work_or_rate() {
        assert_eq!(forecast(100, 1, 3), Some(110), "⌈28/3⌉ = 10");
        assert_eq!(forecast(100, 3, 3), Some(128));
        assert_eq!(forecast(100, 0, 3), None, "done");
        assert_eq!(forecast(100, 3, 0), None, "no closures");
        let end = d("9999-12-31");
        assert_eq!(forecast(end - 28, 1, 1), Some(end));
        assert_eq!(forecast(end - 27, 1, 1), None, "past 9999-12-31");
        assert_eq!(
            forecast(0, u64::MAX, 1),
            None,
            "overflow is no forecast, not a panic"
        );
    }

    #[test]
    fn lists_at_the_limit_are_flagged() {
        let until = d(UNTIL);
        let many = |n: usize, f: &dyn Fn(usize) -> String| {
            format!("[{}]", (0..n).map(f).collect::<Vec<_>>().join(","))
        };
        let issue_n = |i: usize| {
            issue(
                u64::try_from(i).unwrap_or(0),
                "OPEN",
                "2026-11-01",
                None,
                &[],
                None,
            )
        };
        let pr_n = |i: usize| pr(u64::try_from(i).unwrap_or(0), "2026-09-08", None);
        for (n, flagged) in [(LIST_LIMIT - 1, false), (LIST_LIMIT, true)] {
            let mut prs = prs_for(until, &[]);
            if let Some(first) = prs.first_mut() {
                *first = many(n, &pr_n);
            }
            let inp = Inputs {
                prs,
                issues: many(n, &issue_n),
                journal_log: String::new(),
                index_tsv: None,
                ..Inputs::default()
            };
            let snap = compute(&inp, until, until).unwrap_or_else(|e| panic!("{e}"));
            let want: Vec<String> = if flagged {
                vec!["prs-2026-09-07".to_owned(), "issues".to_owned()]
            } else {
                Vec::new()
            };
            assert_eq!(snap.truncated, want, "{n} entries");
        }
    }

    #[test]
    fn bad_inputs_are_errors() {
        let until = d(UNTIL);
        let ok = |prs: Vec<String>, issues: &str| {
            compute(
                &Inputs {
                    prs,
                    issues: issues.to_owned(),
                    journal_log: String::new(),
                    index_tsv: None,
                    ..Inputs::default()
                },
                until,
                until,
            )
        };
        assert!(ok(prs_for(until, &[]), "[]").is_ok());
        assert!(
            ok(prs_for(until, &[]).into_iter().take(11).collect(), "[]").is_err(),
            "a week missing"
        );
        let mut bad = prs_for(until, &[]);
        if let Some(f) = bad.first_mut() {
            *f = r#"[{"number": 1, "mergedAt": "2026-09-08"}]"#.to_owned();
        }
        assert!(ok(bad, "[]").is_err(), "not a timestamp");
        assert!(ok(prs_for(until, &[]), "{").is_err());
        assert!(ok(prs_for(until, &[]), r#"[{"number": 1, "title": "t", "state": "CLOSED", "createdAt": "2026-11-01T00:00:00Z", "closedAt": null}]"#).is_err(), "closed without closedAt");
        assert!(ok(prs_for(until, &[]), r#"[{"number": 1, "title": "t", "state": "OPEN", "createdAt": "2026-11-01T00:00:00Z", "closedAt": null, "milestone": "M1"}]"#).is_ok(), "a milestone that isn't an object counts as none");
    }

    #[test]
    fn parses_flags() {
        let a = |v: &[&str]| parse(&v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
        let o = a(&[]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            (o.until, o.out, o.from, o.index),
            (None, PathBuf::from(SNAPSHOT), None, None)
        );
        let o = a(&[
            "--until", UNTIL, "--out", "x.json", "--from", "raw", "--index", "i.tsv",
        ])
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(o.until, Some(d(UNTIL)));
        assert_eq!(o.from, Some(PathBuf::from("raw")));
        assert!(a(&["--until", "2026-02-30"]).is_err());
        assert!(a(&["--until"]).is_err());
        assert!(a(&["--since", UNTIL]).is_err());
    }

    #[test]
    fn from_dir_runs_end_to_end() {
        let base =
            std::env::temp_dir().join(format!("xtask-dashboard-from-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let raw = base.join("raw");
        assert!(fs::create_dir_all(&raw).is_ok());
        let until = d(UNTIL);
        for ((start, _), text) in weeks(until).iter().zip(prs_for(
            until,
            &[("2026-11-23", vec![pr(1, UNTIL, Some(3))])],
        )) {
            assert!(fs::write(raw.join(format!("prs-{}.json", fmt_day(*start))), text).is_ok());
        }
        let out = base.join("out").join("dashboard.json");
        let opts = Options {
            until: Some(until),
            out: out.clone(),
            from: Some(raw.clone()),
            index: None,
        };
        assert!(run(&base, &opts).is_err(), "issues.json missing");
        assert!(fs::write(raw.join("issues.json"), "[]").is_ok());
        assert!(run(&base, &opts).is_err(), "journal-log.txt missing");
        assert!(fs::write(raw.join("journal-log.txt"), "").is_ok());
        assert_eq!(
            run(&base, &opts),
            Ok(out.clone()),
            "index.tsv may be missing"
        );
        let text = fs::read_to_string(&out).unwrap_or_default();
        assert!(text.contains(r#"{"start": "2026-11-23", "end": "2026-11-25", "merged_prs": 1, "median_rounds": 3, "sessions": null}"#), "{text}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn copies_the_snapshot_up_to_one_mib() {
        let base =
            std::env::temp_dir().join(format!("xtask-dashboard-copy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (repo, site) = (base.join("repo"), base.join("site"));
        assert!(fs::create_dir_all(repo.join("ops/metrics")).is_ok());
        let snap = repo.join(SNAPSHOT);
        assert!(fs::write(&snap, "{}").is_ok());
        assert_eq!(
            copy_into_site(&repo, &site),
            Ok(Copied::NoPage),
            "no page: nothing copied"
        );
        assert!(fs::create_dir_all(site.join("dashboard")).is_ok());
        let data = site.join("dashboard/data.json");
        let limit = usize::try_from(MAX_SNAPSHOT_BYTES).unwrap_or(0);
        assert!(fs::write(&snap, vec![b' '; limit]).is_ok());
        assert_eq!(copy_into_site(&repo, &site), Ok(Copied::Yes));
        assert_eq!(
            fs::metadata(&data).map(|m| m.len()).ok(),
            Some(MAX_SNAPSHOT_BYTES)
        );
        assert!(fs::write(&snap, vec![b' '; limit + 1]).is_ok());
        let r = copy_into_site(&repo, &site);
        assert!(
            r.as_ref()
                .is_err_and(|e| e.contains("larger than 1048576 bytes")),
            "{r:?}"
        );
        assert!(fs::remove_file(&snap).is_ok() && fs::create_dir_all(&snap).is_ok());
        let r = copy_into_site(&repo, &site);
        assert!(
            r.as_ref().is_err_and(|e| e.contains("not a regular file")),
            "{r:?}"
        );
        assert!(fs::remove_dir_all(&snap).is_ok());
        assert_eq!(
            copy_into_site(&repo, &site),
            Ok(Copied::NoSnapshot),
            "no snapshot: nothing copied"
        );
        let _ = fs::remove_dir_all(&base);
    }

    // ---- judge history (#306)

    fn run_json(id: u64, created: &str) -> String {
        format!(r#"{{"databaseId": {id}, "createdAt": "{created}"}}"#)
    }

    /// A gates.json whose smoke metrics are `open` etc., with exam values that differ.
    fn gates_json(open: &str, passing: &str) -> String {
        format!(
            r#"{{"current_milestone": "M1", "gates": {{}}, "metrics": {{"exam_open_rate": 0.25, "exam_crashes": 9, "smoke_open_rate": {open}, "smoke_render_match": 0.0, "smoke_text_f1": 1, "smoke_crashes": 2, "smoke_journeys_passing": {passing}, "smoke_journeys": 129}}}}"#
        )
    }

    fn judge_inputs(runs: &[String], gates: &[(u64, String)]) -> Inputs {
        Inputs {
            prs: prs_for(d(UNTIL), &[]),
            issues: "[]".to_owned(),
            nightly_runs: Some(vec![(
                "runs-schedule.json".to_owned(),
                format!("[{}]", runs.join(",")),
            )]),
            gates: gates.iter().cloned().collect(),
            ..Inputs::default()
        }
    }

    #[test]
    fn nightly_candidates_are_newest_first_and_capped() {
        let until = d(UNTIL);
        let mut runs = vec![
            run_json(10, "2026-11-24T12:00:00Z"),
            run_json(11, "2026-11-25T01:00:00Z"),
            run_json(12, "2026-11-26T01:00:00Z"), // after --until
            run_json(20, "2026-11-17T05:00:00Z"),
            run_json(21, "2026-11-17T05:00:00Z"), // same time: the larger id first
            run_json(30, "2026-11-22T23:59:59Z"), // Sunday: week 2026-11-16
            run_json(40, "2026-08-01T00:00:00Z"), // before every week
        ];
        // Week 2026-11-09: eight runs, one a day plus one; only seven are tried.
        for i in 0..8u64 {
            runs.push(run_json(
                100 + i,
                &format!("2026-11-{:02}T0{}:00:00Z", 9 + i.min(6), i),
            ));
        }
        let list = parse_list(&format!("[{}]", runs.join(",")), "t").unwrap_or(Value::Null);
        let c = candidates(list.as_array().unwrap_or_default(), &weeks(until))
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(c.len(), 12);
        assert_eq!(c[11], vec![11, 10]);
        assert_eq!(c[10], vec![30, 21, 20]);
        assert_eq!(
            c[9],
            vec![107, 106, 105, 104, 103, 102, 101],
            "the 8th (oldest) isn't tried"
        );
        assert!(c[..9].iter().all(Vec::is_empty), "{c:?}");
        let bad = parse_list(r#"[{"databaseId": 1, "createdAt": "2026-11-24"}]"#, "t")
            .unwrap_or(Value::Null);
        assert!(candidates(bad.as_array().unwrap_or_default(), &weeks(until)).is_err());
        let bad =
            parse_list(r#"[{"createdAt": "2026-11-24T00:00:00Z"}]"#, "t").unwrap_or(Value::Null);
        assert!(candidates(bad.as_array().unwrap_or_default(), &weeks(until)).is_err());
    }

    #[test]
    fn judge_history_takes_each_weeks_newest_report() {
        let until = d(UNTIL);
        let mut runs = vec![
            run_json(10, "2026-11-24T12:00:00Z"),
            run_json(11, "2026-11-25T01:00:00Z"),
            run_json(12, "2026-11-26T01:00:00Z"),
            run_json(20, "2026-11-17T05:00:00Z"),
            run_json(21, "2026-11-17T05:00:00Z"),
        ];
        for i in 0..8u64 {
            runs.push(run_json(
                100 + i,
                &format!("2026-11-{:02}T0{}:00:00Z", 9 + i.min(6), i),
            ));
        }
        for i in 0..7u64 {
            runs.push(run_json(200 + i, &format!("2026-11-0{}T00:00:00Z", 2 + i)));
        }
        let gates = vec![
            (10, gates_json("0.5935", "3")),
            // 11 (newer) failed to download; 12 is after --until.
            (12, gates_json("0.9", "9")),
            (20, gates_json("0.1", "1")),
            (21, gates_json("0.2", "2")),
            // Only the 8th run of week 2026-11-09 has a report: past the cap.
            (100, gates_json("0.3", "3")),
            // Only the 7th (oldest) run of week 2026-11-02: tried.
            (200, gates_json("0.4", "null")),
        ];
        let snap =
            compute(&judge_inputs(&runs, &gates), until, until).unwrap_or_else(|e| panic!("{e}"));
        let j = snap.judge.clone().unwrap_or_default();
        assert_eq!(j.len(), 12);
        assert_eq!(
            j[11],
            JudgeWeek {
                start: d("2026-11-23"),
                run: Some(10),
                day: Some(d("2026-11-24")),
                open_rate: Some(0.5935),
                render_match: Some(0.0),
                text_f1: Some(1.0),
                crashes: Some(2),
                journeys_passing: Some(3),
                journeys: Some(129),
            }
        );
        assert_eq!((j[10].run, j[10].open_rate), (Some(21), Some(0.2)));
        assert_eq!(
            j[9],
            JudgeWeek {
                start: d("2026-11-09"),
                ..JudgeWeek::default()
            }
        );
        assert_eq!(
            (j[8].run, j[8].journeys_passing, j[8].open_rate),
            (Some(200), None, Some(0.4))
        );
        assert!(
            j[..8]
                .iter()
                .all(|w| w.run.is_none() && w.open_rate.is_none())
        );
        assert!(j.iter().zip(weeks(until)).all(|(w, (s, _))| w.start == s));

        let json = render(&snap);
        assert!(
            json.contains(r#""judge": {"source": "nightly", "weeks": ["#),
            "{json}"
        );
        assert!(json.contains(r#"{"start": "2026-11-23", "run": 10, "day": "2026-11-24", "open_rate": 0.5935, "render_match": 0, "text_f1": 1, "crashes": 2, "journeys_passing": 3, "journeys": 129}"#), "{json}");
        assert!(json.contains(r#"{"start": "2026-11-09", "run": null, "day": null, "open_rate": null, "render_match": null, "text_f1": null, "crashes": null, "journeys_passing": null, "journeys": null}"#), "{json}");
        assert!(json_parses(&json));

        // No run list: source missing, twelve null weeks.
        let none = compute(
            &Inputs {
                nightly_runs: None,
                ..judge_inputs(&[], &[])
            },
            until,
            until,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(none.judge, None);
        let json = render(&none);
        assert!(
            json.contains(r#""judge": {"source": "missing", "weeks": ["#),
            "{json}"
        );
        assert_eq!(json.matches(r#""run": null, "day": null"#).count(), 12);
        assert!(json_parses(&json));
        // An empty run list: nightly, every week null.
        let empty =
            compute(&judge_inputs(&[], &[]), until, until).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            empty.judge.map(|j| j.iter().all(|w| w.run.is_none())),
            Some(true)
        );
    }

    #[test]
    fn gate_metrics_reads_smoke_values_and_rejects_bad_shapes() {
        let m = |metrics: &str| gate_metrics(&format!(r#"{{"metrics": {{{metrics}}}}}"#), "g");
        assert_eq!(m(""), Ok(JudgeWeek::default()), "absent keys are null");
        let all_null = m(
            r#""smoke_open_rate": null, "smoke_render_match": null, "smoke_text_f1": null, "smoke_crashes": null, "smoke_journeys_passing": null, "smoke_journeys": null"#,
        );
        assert_eq!(all_null, Ok(JudgeWeek::default()));
        assert_eq!(
            m(r#""exam_open_rate": 0.5, "exam_crashes": 3"#),
            Ok(JudgeWeek::default()),
            "exam values are never read"
        );
        assert_eq!(
            m(r#""smoke_open_rate": 0"#).map(|w| w.open_rate),
            Ok(Some(0.0))
        );
        assert_eq!(
            m(r#""smoke_open_rate": 1"#).map(|w| w.open_rate),
            Ok(Some(1.0))
        );
        assert_eq!(m(r#""smoke_crashes": 0"#).map(|w| w.crashes), Ok(Some(0)));
        for bad in [
            r#""smoke_open_rate": 1.5"#,
            r#""smoke_render_match": -0.1"#,
            r#""smoke_text_f1": 1e400"#,
            r#""smoke_open_rate": "0.5""#,
            r#""smoke_crashes": -1"#,
            r#""smoke_journeys": 1.5"#,
            r#""smoke_journeys_passing": true"#,
        ] {
            assert!(m(bad).is_err(), "{bad}");
        }
        assert!(gate_metrics("{}", "g").is_err(), "no metrics");
        assert!(
            gate_metrics(r#"{"metrics": []}"#, "g").is_err(),
            "metrics not an object"
        );
        assert!(gate_metrics("{", "g").is_err());
        // A bad file fails the run only when it is the one chosen.
        let until = d(UNTIL);
        let runs = [
            run_json(10, "2026-11-24T12:00:00Z"),
            run_json(11, "2026-11-25T01:00:00Z"),
        ];
        let bad = (11, r#"{"metrics": {"smoke_open_rate": 2}}"#.to_owned());
        assert!(
            compute(
                &judge_inputs(&runs, std::slice::from_ref(&bad)),
                until,
                until
            )
            .is_err()
        );
        assert!(
            compute(
                &judge_inputs(&runs[..1], &[(10, gates_json("0.5", "1")), bad]),
                until,
                until
            )
            .is_ok(),
            "run 11 isn't listed, so its file is never read"
        );
    }

    #[test]
    fn nightly_runs_at_the_limit_are_flagged() {
        let until = d(UNTIL);
        for (n, flagged) in [(NIGHTLY_LIMIT - 1, false), (NIGHTLY_LIMIT, true)] {
            let runs: Vec<String> = (0..n)
                .map(|i| run_json(u64::try_from(i).unwrap_or(0), "2026-08-01T00:00:00Z"))
                .collect();
            let snap =
                compute(&judge_inputs(&runs, &[]), until, until).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(
                snap.truncated.contains(&"runs-schedule".to_owned()),
                flagged,
                "{n} runs"
            );
        }
    }

    #[test]
    fn judge_command_arguments_are_pinned() {
        assert_eq!(NIGHTLY_EVENTS, ["schedule", "workflow_dispatch"]);
        assert_eq!(
            nightly_runs_args("schedule"),
            "run list --workflow ci.yml --event schedule --branch main --status completed --limit 200 --json databaseId,createdAt"
        );
        assert_eq!(
            nightly_runs_args("workflow_dispatch"),
            "run list --workflow ci.yml --event workflow_dispatch --branch main --status completed --limit 200 --json databaseId,createdAt"
        );
        assert_eq!(
            download_args(45822452567, "/r/target/dashboard-gates/45822452567"),
            [
                "run",
                "download",
                "45822452567",
                "-n",
                "gates-report",
                "-D",
                "/r/target/dashboard-gates/45822452567"
            ]
        );
        assert_eq!(GATES_DIR, "target/dashboard-gates");
    }

    #[test]
    fn reads_downloaded_gates_up_to_one_mib() {
        let base =
            std::env::temp_dir().join(format!("xtask-dashboard-gates-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        assert!(fs::create_dir_all(&base).is_ok());
        let f = base.join("gates.json");
        assert_eq!(read_gates(&f), Err(GatesError::Missing));
        let limit = usize::try_from(MAX_GATES_BYTES).unwrap_or(0);
        assert!(fs::write(&f, vec![b' '; limit]).is_ok());
        assert_eq!(read_gates(&f).map(|t| t.len()), Ok(limit), "exactly 1 MiB");
        assert!(fs::write(&f, vec![b' '; limit + 1]).is_ok());
        assert!(
            matches!(read_gates(&f), Err(GatesError::Bad(_))),
            "1 MiB + 1"
        );
        assert!(fs::write(base.join("real.json"), "{}").is_ok());
        assert!(fs::remove_file(&f).is_ok());
        #[cfg(unix)]
        {
            assert!(std::os::unix::fs::symlink(base.join("real.json"), &f).is_ok());
            assert!(
                matches!(read_gates(&f), Err(GatesError::Bad(_))),
                "a symlink"
            );
            assert!(fs::remove_file(&f).is_ok());
        }
        assert!(fs::create_dir(&f).is_ok());
        assert!(
            matches!(read_gates(&f), Err(GatesError::Bad(_))),
            "a directory"
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn from_dir_judge_history_missing_then_partial() {
        let base =
            std::env::temp_dir().join(format!("xtask-dashboard-judge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let raw = base.join("raw");
        assert!(fs::create_dir_all(&raw).is_ok());
        let until = d(UNTIL);
        for ((start, _), text) in weeks(until).iter().zip(prs_for(until, &[])) {
            assert!(fs::write(raw.join(format!("prs-{}.json", fmt_day(*start))), text).is_ok());
        }
        assert!(fs::write(raw.join("issues.json"), "[]").is_ok());
        assert!(fs::write(raw.join("journal-log.txt"), "").is_ok());
        let out = base.join("dashboard.json");
        let opts = Options {
            until: Some(until),
            out: out.clone(),
            from: Some(raw.clone()),
            index: None,
        };
        assert_eq!(run(&base, &opts), Ok(out.clone()));
        let text = fs::read_to_string(&out).unwrap_or_default();
        assert!(text.contains(r#""judge": {"source": "missing""#), "{text}");
        let runs = [
            run_json(10, "2026-11-24T12:00:00Z"),
            run_json(11, "2026-11-25T01:00:00Z"),
            run_json(20, "2026-11-17T00:00:00Z"),
        ];
        assert!(
            fs::write(
                raw.join("runs-schedule.json"),
                format!("[{}]", runs.join(","))
            )
            .is_ok()
        );
        // Run 11 has no file (a failed download); run 20's week has none at all.
        assert!(fs::write(raw.join("gates-10.json"), gates_json("0.5935", "0")).is_ok());
        assert_eq!(run(&base, &opts), Ok(out.clone()));
        let text = fs::read_to_string(&out).unwrap_or_default();
        assert!(text.contains(r#""judge": {"source": "nightly""#), "{text}");
        assert!(
            text.contains(
                r#"{"start": "2026-11-23", "run": 10, "day": "2026-11-24", "open_rate": 0.5935"#
            ),
            "{text}"
        );
        assert!(
            text.contains(r#"{"start": "2026-11-16", "run": null"#),
            "{text}"
        );
        // A dispatched nightly (a manual rerun) counts too: the newest of week 2026-11-16.
        assert!(
            fs::write(
                raw.join("runs-workflow_dispatch.json"),
                format!("[{}]", run_json(30, "2026-11-18T09:00:00Z"))
            )
            .is_ok()
        );
        assert!(fs::write(raw.join("gates-30.json"), gates_json("0.25", "1")).is_ok());
        assert_eq!(run(&base, &opts), Ok(out.clone()));
        let text = fs::read_to_string(&out).unwrap_or_default();
        assert!(
            text.contains(
                r#"{"start": "2026-11-16", "run": 30, "day": "2026-11-18", "open_rate": 0.25"#
            ),
            "{text}"
        );
        // With --from, a chosen gates file still has the 1 MiB cap.
        let limit = usize::try_from(MAX_GATES_BYTES).unwrap_or(0);
        assert!(fs::write(raw.join("gates-10.json"), vec![b' '; limit + 1]).is_ok());
        assert!(run(&base, &opts).is_err(), "gates-10.json over 1 MiB");
        assert!(!base.join(GATES_DIR).exists(), "--from downloads nothing");
        let _ = fs::remove_dir_all(&base);
    }
}
