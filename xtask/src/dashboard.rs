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

/// Every input, as fetched or read with `--from`.
#[derive(Debug, Default, Clone)]
pub struct Inputs {
    /// One `gh pr list` output per week, oldest first.
    pub prs: Vec<String>,
    pub issues: String,
    pub journal_log: String,
    pub index_tsv: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    Ok(Inputs {
        prs,
        issues,
        journal_log,
        index_tsv,
    })
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

/// The `ci linux` step: copies the committed snapshot to `site/dashboard/data.json` when the
/// web build wrote `site/dashboard/`. Ok(false): nothing copied (no snapshot, or no page).
pub fn copy_into_site(repo: &Path, site: &Path) -> Result<bool, String> {
    use std::io::Read as _;
    let dir = site.join("dashboard");
    if !dir.is_dir() {
        return Ok(false);
    }
    let src = repo.join(SNAPSHOT);
    let file = match jpdf_fsio::open_nonblocking(&src) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
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
    Ok(true)
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
            Ok(false),
            "no page: nothing copied"
        );
        assert!(fs::create_dir_all(site.join("dashboard")).is_ok());
        let data = site.join("dashboard/data.json");
        let limit = usize::try_from(MAX_SNAPSHOT_BYTES).unwrap_or(0);
        assert!(fs::write(&snap, vec![b' '; limit]).is_ok());
        assert_eq!(copy_into_site(&repo, &site), Ok(true));
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
            Ok(false),
            "no snapshot: nothing copied"
        );
        let _ = fs::remove_dir_all(&base);
    }
}
