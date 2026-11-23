//! `cargo xtask next-session`: the session scheduler, precedence steps 1–5, 7, 10 and 11 (ADR 0011
//! §2, tasks 1a and 1b; specs/tooling/next-session.md). Read-only: it runs read commands and writes
//! nothing.

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
    /// The judge-alarm file's text, if the file exists.
    alarm: Option<String>,
    now: String,
    main_log: String,
    tags: String,
    milestones: String,
    blocked: String,
    /// `<path>\t<line>` for each `- Session: ` line of the recent journal files (step 11).
    records: String,
}

/// Paths of the interim product-merge rule (ADR 0011 §3): code that ships or is graded.
const PRODUCT_PATHS: [&str; 6] = [
    "crates/",
    "bindings/",
    "apps/",
    "fuzz/",
    "tools/jpdf-cli/",
    "tools/fsio/",
];
/// Labels step 10 skips besides `SKIP_LABELS`.
const NOT_READY_LABELS: [&str; 3] = ["in-progress", "design", "design-pending"];
const WEEK: i64 = 7 * 86_400;
const AUDIT_MERGES: usize = 10;
const BLOCKED_INCIDENT: usize = 3;
const JOURNAL: &str = "ops/journal/";
const SESSION_LINE: &str = "- Session: ";

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

/// The file's text, or `None` when it doesn't exist.
fn optional(p: &Path) -> Result<Option<String>, Error> {
    match fs::read_to_string(p) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Broken(format!("read {}: {e}", p.display()))),
    }
}

/// Live `blocked.json`: each issue labeled `blocked` and updated since `since` (a day), with
/// its events (one page of 100), as `[{"number":N,"events":[…]}]`.
fn blocked_live(since: &str) -> Result<String, Error> {
    let list = gh(&format!(
        "issue list --label blocked --state all --search updated:>={since} --limit 100 --json number"
    ))?;
    let v = parse_list(&list, "gh issue list --label blocked").map_err(Error::Broken)?;
    let mut parts = Vec::new();
    for i in list_of(&v) {
        let n = number(i, "number", "gh issue list --label blocked").map_err(Error::Broken)?;
        let events = gh(&format!(
            "api repos/jacobdam/jpdf3/issues/{n}/events?per_page=100"
        ))?;
        parts.push(format!("{{\"number\":{n},\"events\":{events}}}"));
    }
    Ok(format!("[{}]", parts.join(",")))
}

/// Live `records.txt`: the `- Session: ` lines of the journal files dated `day`, on origin/main and on the head branch of each open agent PR, as `<path>\t<line>`. The
/// refs come from the last `git fetch` (TICK §1); a branch that isn't fetched is exit 2.
fn records_live(prs: &str, day: i64) -> Result<String, Error> {
    let mut refs = vec!["refs/remotes/origin/main".to_owned()];
    for p in parse_prs(prs).map_err(Error::Broken)? {
        if p.open {
            // `refs/…` never reads as an option, and a ref name holds no `:` (git check-ref-format).
            refs.push(format!("refs/remotes/origin/{}", p.branch));
        }
    }
    let dir = format!("{JOURNAL}{}/", crate::metrics::fmt_day(day));
    let mut seen = std::collections::BTreeSet::new();
    let mut out = String::new();
    for r in &refs {
        // `--full-tree`: the path is from the repository root whatever the current directory
        // (R1.3). `core.quotePath`: any unusual byte in a name is C-quoted, so the listing is
        // ASCII and such a name is skipped below.
        let tree = cmd(
            "git",
            &[
                "-c",
                "core.quotePath=true",
                "ls-tree",
                "-r",
                "--full-tree",
                r.as_str(),
                "--",
                dir.as_str(),
            ],
        )
        .map_err(|e| match e {
            Error::Unknown(m) => Error::Unknown(format!("{m} (is {r} fetched? run git fetch)")),
            b @ Error::Broken(_) => b,
        })?;
        session_lines(&tree, &dir, &mut seen, &mut out, |oid| {
            cmd("git", &["cat-file", "blob", oid]).ok()
        });
    }
    Ok(out)
}

/// Appends `<path>\t<line>` to `out` for each `- Session: ` line of each journal file in a
/// `git ls-tree -r` listing (`<mode> blob <oid>\t<path>` lines) whose path is `<dir><name>.md`
/// (`<name>` plain: no `/`, not C-quoted) and whose (oid, path) is not in `seen`. A blob that
/// `read` can't return (not UTF-8, over the output cap, a git error) is skipped: a missed record
/// costs one more Planner run that day, never a stopped scheduler (R1.1).
fn session_lines(
    tree: &str,
    dir: &str,
    seen: &mut std::collections::BTreeSet<(String, String)>,
    out: &mut String,
    mut read: impl FnMut(&str) -> Option<String>,
) {
    for line in tree.lines() {
        let Some((meta, path)) = line.split_once('\t') else {
            continue;
        };
        let Some(oid) = meta.strip_prefix("100644 blob ") else {
            continue;
        };
        let plain = path
            .strip_prefix(dir)
            .and_then(|n| n.strip_suffix(".md"))
            .is_some_and(|n| !n.is_empty() && !n.contains('/'));
        if !plain || path.starts_with('"') || !seen.insert((oid.to_owned(), path.to_owned())) {
            continue;
        }
        let Some(text) = read(oid) else {
            continue;
        };
        for l in text.lines().filter(|l| l.starts_with(SESSION_LINE)) {
            let _ = writeln!(out, "{path}\t{l}");
        }
    }
}

fn list_of(v: &Value) -> &[Value] {
    v.as_array().unwrap_or_default()
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
            alarm: optional(&dir.join("judge-alarm-since"))?,
            now: read(dir, "now.txt")?.trim().to_owned(),
            main_log: read(dir, "main-log.txt")?,
            tags: read(dir, "tags.txt")?,
            milestones: read(dir, "milestones.json")?,
            blocked: read(dir, "blocked.json")?,
            records: read(dir, "records.txt")?,
        });
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| Error::Broken(format!("system clock: {e}")))?
        .as_secs();
    let week_ago = crate::metrics::fmt_day(
        i64::try_from(now / 86_400)
            .ok()
            .and_then(|d| d.checked_sub(7))
            .ok_or_else(|| Error::Broken("system clock out of range".to_owned()))?,
    );
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
    let prs = gh(
        "pr list --author @me --state all --limit 200 --json number,state,isDraft,body,headRefName,labels,statusCheckRollup,mergeable",
    )?;
    // The same day as `now.txt`, so a run at midnight reads the day step 11 compares with.
    let day = i64::try_from(now / 86_400)
        .map_err(|_| Error::Broken("system clock out of range".to_owned()))?;
    let records = records_live(&prs, day)?;
    Ok(Inputs {
        feed,
        floor,
        owner_open: gh(&format!(
            "issue list --state open --author {OWNER} --limit 1000 --json number,labels,comments"
        ))?,
        owner_closed: gh(&format!(
            "issue list --state closed --author {OWNER} --search closed:>={since} --limit 1000 --json number,body,comments"
        ))?,
        prs,
        issues: gh(
            "issue list --state open --limit 1000 --json number,title,labels,body,milestone",
        )?,
        runs: gh(
            "run list --workflow ci.yml --branch main --limit 20 --json databaseId,status,conclusion,event",
        )?,
        worktrees: cmd("git", &["worktree", "list", "--porcelain"])?,
        alarm: optional(&root.join(ALARM_FILE))?,
        now: now.to_string(),
        main_log: cmd(
            "git",
            &[
                "log",
                "--first-parent",
                "--no-renames",
                "--name-status",
                "--format=commit %H %ct %s",
                "origin/main",
            ],
        )?,
        tags: cmd(
            "git",
            &[
                "for-each-ref",
                "--format=%(refname:short) %(creatordate:unix)",
                "refs/tags",
            ],
        )?,
        milestones: gh("api repos/jacobdam/jpdf3/milestones?state=open&per_page=100")?,
        blocked: blocked_live(&week_ago)?,
        records,
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
        // Read before combining with `conflict`, so a conflicting PR's rollup is checked too.
        let rollup_bad = open && failing(field(p, "statusCheckRollup", what)?, what)?;
        let pr = Pr {
            number: number(p, "number", what)?,
            open,
            body: body.to_owned(),
            branch: text(p, "headRefName", what)?.to_owned(),
            labels: labels(p, what)?,
            failing: open && (conflict || rollup_bad),
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
    body: String,
    /// The milestone's title, `None` without one.
    milestone: Option<String>,
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
            body: text(i, "body", what)?.to_owned(),
            milestone: match field(i, "milestone", what)? {
                Value::Null => None,
                m => Some(text(m, "title", what)?.to_owned()),
            },
        });
    }
    out.sort_by_key(|i| i.number);
    Ok(out)
}

/// The list right after each `Depends on:` (bold markers allowed): `#N` items separated by
/// commas, spaces or `and`, each optionally followed by a `(…)` note. The list ends at the first
/// other text, so prose after it (`Depends on: issues closed (#244)`) names nothing.
fn dependencies(body: &str) -> Vec<u64> {
    let mut out = Vec::new();
    for line in body.lines() {
        for rest in line.split("Depends on:").skip(1) {
            let mut r = rest;
            loop {
                r = r.trim_start_matches([' ', '*', ',']);
                if let Some(x) = r.strip_prefix("and ") {
                    r = x;
                    continue;
                }
                let Some(x) = r.strip_prefix('#') else { break };
                let len = x.bytes().take_while(u8::is_ascii_digit).count();
                let Some(n) = x.get(..len).and_then(|d| d.parse().ok()) else {
                    break;
                };
                out.push(n);
                r = x.get(len..).unwrap_or_default().trim_start();
                if r.starts_with('(') {
                    match r.find(')').and_then(|i| r.get(i + 1..)) {
                        Some(after) => r = after,
                        None => break,
                    }
                }
            }
        }
    }
    out
}

/// The lowest `M<n>` title among the open milestones (`n` digits only), as that title.
fn current_milestone(text_: &str) -> Result<Option<String>, String> {
    let what = "milestones.json";
    let v = parse_list(text_, what)?;
    let mut best: Option<(u64, String)> = None;
    for m in list(&v, what)? {
        let title = text(m, "title", what)?;
        let n = title
            .strip_prefix('M')
            .filter(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|d| d.parse::<u64>().ok());
        if let Some(n) = n
            && best.as_ref().is_none_or(|(b, _)| n < *b)
        {
            best = Some((n, title.to_owned()));
        }
    }
    Ok(best.map(|(_, t)| t))
}

/// Unix seconds: digits only, within the years 1970..=9999 (`metrics::utc_day`).
fn unix(s: &str, what: &str) -> Result<i64, String> {
    crate::metrics::utc_day(s)
        .and_then(|_| s.parse().ok())
        .ok_or_else(|| format!("{what}: {s:?} is not Unix seconds"))
}

/// Unix seconds as GitHub writes timestamps (`YYYY-MM-DDTHH:MM:SSZ`), so text comparison works.
fn iso(secs: i64) -> String {
    let t = secs.rem_euclid(86_400);
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        crate::metrics::fmt_day(secs.div_euclid(86_400)),
        t / 3600,
        t % 3600 / 60,
        t % 60
    )
}

/// One first-parent commit of origin/main, newest first.
struct Commit {
    sha: String,
    time: i64,
    subject: String,
    /// (status letter, path) per `--name-status` line.
    files: Vec<(String, String)>,
}

fn parse_log(text_: &str) -> Result<Vec<Commit>, String> {
    let what = "main-log.txt";
    let mut out: Vec<Commit> = Vec::new();
    for line in text_.lines().filter(|l| !l.is_empty()) {
        if let Some(rest) = line.strip_prefix("commit ") {
            let (sha, rest) = rest
                .split_once(' ')
                .ok_or_else(|| format!("{what}: bad commit line {line:?}"))?;
            let (time, subject) = rest.split_once(' ').unwrap_or((rest, ""));
            if sha.len() != 40 || !sha.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!("{what}: bad commit id in {line:?}"));
            }
            out.push(Commit {
                sha: sha.to_owned(),
                time: unix(time, what)?,
                subject: subject.to_owned(),
                files: Vec::new(),
            });
            continue;
        }
        let (status, path) = line
            .split_once('\t')
            .filter(|(s, p)| matches!(*s, "A" | "M" | "D" | "T") && !p.is_empty())
            .ok_or_else(|| format!("{what}: unexpected line {line:?}"))?;
        out.last_mut()
            .ok_or_else(|| format!("{what}: file line before the first commit"))?
            .files
            .push((status.to_owned(), path.to_owned()));
    }
    Ok(out)
}

/// git C-quotes unusual paths (`"crates/\303\251.rs"`); compare without the opening quote.
fn unquoted(path: &str) -> &str {
    path.strip_prefix('"').unwrap_or(path)
}

/// `<dir>/NNNN.md` with exactly four digits.
fn is_record(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .and_then(|r| r.strip_suffix(".md"))
        .is_some_and(|n| n.len() == 4 && n.bytes().all(|c| c.is_ascii_digit()))
}

/// The index of the newest commit that adds a record in one of `dirs`.
fn last_record(log: &[Commit], dirs: &[&str]) -> Option<usize> {
    log.iter().position(|c| {
        c.files
            .iter()
            .any(|(s, p)| s == "A" && dirs.iter().any(|d| is_record(p, d)))
    })
}

fn product_merge(c: &Commit) -> bool {
    c.files
        .iter()
        .any(|(_, p)| PRODUCT_PATHS.iter().any(|d| unquoted(p).starts_with(d)))
}

/// Creation times of the milestone tags `m<digits>` in `tags.txt`.
fn milestone_tags(text_: &str) -> Result<Vec<(String, i64)>, String> {
    let what = "tags.txt";
    let mut out = Vec::new();
    for line in text_.lines().filter(|l| !l.is_empty()) {
        let (name, time) = line
            .rsplit_once(' ')
            .ok_or_else(|| format!("{what}: bad line {line:?}"))?;
        let time = unix(time, what)?;
        let digits = name.strip_prefix('m').unwrap_or("");
        if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) {
            out.push((name.to_owned(), time));
        }
    }
    Ok(out)
}

/// Issues with a `labeled` `blocked` event created at or after `since` (ISO text).
fn blocked_since(text_: &str, since: &str) -> Result<Vec<u64>, String> {
    let what = "blocked.json";
    let v = parse_list(text_, what)?;
    let mut out = Vec::new();
    for i in list(&v, what)? {
        let n = number(i, "number", what)?;
        let mut hit = false;
        for e in list(field(i, "events", what)?, what)? {
            let at = text(e, "created_at", what)?;
            if !valid_floor(at) {
                return Err(format!(
                    "{what}: created_at {at:?} is not YYYY-MM-DDTHH:MM:SSZ"
                ));
            }
            if text(e, "event", what)? == "labeled" {
                hit |= text(field(e, "label", what)?, "name", what)? == "blocked" && at >= since;
            }
        }
        if hit && !out.contains(&n) {
            out.push(n);
        }
    }
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

/// Step 7 (TICK rules g, h): an audit slot due, then a retro due. `audit_pr` and `retro_pr` are
/// the open agent PRs already doing that slot (#282): a due term with one is not picked, and the
/// returned notes say so.
fn step7(
    inp: &Inputs,
    now: i64,
    alarm: Option<i64>,
    audit_pr: Option<u64>,
    retro_pr: Option<u64>,
) -> Result<(Option<Pick>, Vec<String>), String> {
    let mut notes = Vec::new();
    let mut in_flight = |kind: &str, why: &str, pr: Option<u64>| {
        pr.map(|n| {
            notes.push(format!(
                "{kind} due ({why}) but PR #{n} is open for it: not picked"
            ))
        })
        .is_some()
    };
    let log = parse_log(&inp.main_log)?;
    let tags = milestone_tags(&inp.tags)?;
    let short = |c: &Commit| c.sha.get(..7).unwrap_or(&c.sha).to_owned();
    let age = |t: i64| {
        now.checked_sub(t)
            .ok_or_else(|| "time difference out of range".to_owned())
    };
    let root = log
        .last()
        .ok_or_else(|| "main-log.txt: no commits".to_owned())?;

    // The retro's inputs are parsed before the audit can return, so a malformed `blocked.json`
    // fails every run (R1.1).
    let (since, newer, from) = match last_record(&log, &["docs/retros/"]) {
        Some(i) => {
            let c = log.get(i).ok_or("record index")?;
            (c.time, log.get(..i).unwrap_or_default(), short(c))
        }
        None => (root.time, &log[..], "the first commit".to_owned()),
    };
    // "This week" is "since the retro record": an older record is due by age already.
    let blocked = blocked_since(&inp.blocked, &iso(since.saturating_add(1)))?;

    let audit = match last_record(&log, &["docs/audits/", "docs/advocate/"]) {
        None => Some("no audit record on origin/main".to_owned()),
        Some(i) => {
            let c = log.get(i).ok_or("record index")?;
            let merges = log
                .get(..i)
                .unwrap_or_default()
                .iter()
                .filter(|c| product_merge(c))
                .count();
            let days = age(c.time)?;
            if merges >= AUDIT_MERGES {
                Some(format!("{merges} product merges since {}", short(c)))
            } else if days >= WEEK {
                Some(format!("{} days since {}", days / 86_400, short(c)))
            } else {
                None
            }
        }
    };
    let audit = audit.filter(|why| !in_flight("audit", why, audit_pr));
    if let Some(why) = audit {
        let range = last_record(&log, &["docs/audits/", "docs/advocate/"])
            .and_then(|i| log.get(i))
            .map_or_else(
                || "git log --first-parent --oneline origin/main".to_owned(),
                |c| format!("git log --first-parent --oneline {}..origin/main", short(c)),
            );
        let p = pick(
            "audit due".to_owned(),
            &format!("g (audit due: {why})"),
            vec!["ops/TICK.md §6".to_owned(), range],
        );
        return Ok((Some(p), notes));
    }

    let why = if age(since)? >= WEEK {
        Some(format!("{} days since {from}", age(since)? / 86_400))
    } else if let Some(c) = newer.iter().find(|c| c.subject.starts_with("Revert \"")) {
        Some(format!("incident: revert {} since {from}", short(c)))
    } else if alarm.is_some_and(|t| t > since) {
        Some(format!("incident: judge alarm since {from}"))
    } else if let Some((name, _)) = tags.iter().find(|(_, t)| *t > since) {
        Some(format!("incident: milestone tag {name} since {from}"))
    } else if blocked.len() >= BLOCKED_INCIDENT {
        let list: Vec<String> = blocked.iter().map(|n| format!("#{n}")).collect();
        Some(format!(
            "incident: {} issues labeled blocked since {from} ({})",
            blocked.len(),
            list.join(", ")
        ))
    } else {
        None
    };
    let why = why.filter(|why| !in_flight("retro", why, retro_pr));
    let p = why.map(|why| {
        pick(
            "retro due".to_owned(),
            &format!("h (retro due: {why})"),
            vec![
                "ops/TICK.md §7".to_owned(),
                "cargo xtask metrics --index <main checkout>/ops/logs/index.tsv".to_owned(),
            ],
        )
    });
    Ok((p, notes))
}

/// Whether `records.txt` holds a `- Session: planner` line in a journal file dated `today`
/// (ADR 0011 §1, the Planner's bound). Every line is checked, so a malformed one always fails.
fn planner_ran(text_: &str, today: &str) -> Result<bool, String> {
    let what = "records.txt";
    let mut ran = false;
    for line in text_.lines().filter(|l| !l.is_empty()) {
        let (path, record) = line
            .split_once('\t')
            .ok_or_else(|| format!("{what}: no tab in {line:?}"))?;
        let (day, _) = path
            .strip_prefix(JOURNAL)
            .and_then(|r| r.split_once('/'))
            .filter(|(d, n)| {
                crate::metrics::parse_date(d).is_some() && n.ends_with(".md") && !n.contains('/')
            })
            .ok_or_else(|| format!("{what}: {path:?} is not {JOURNAL}YYYY-MM-DD/<name>.md"))?;
        let kind = record
            .strip_prefix(SESSION_LINE)
            .ok_or_else(|| format!("{what}: {record:?} is not a {SESSION_LINE:?} line"))?
            .split_whitespace()
            .next();
        ran |= day == today && kind == Some("planner");
    }
    Ok(ran)
}

fn decide(inp: &Inputs) -> Result<String, String> {
    let prs = parse_prs(&inp.prs)?;
    let issues = parse_issues(&inp.issues)?;
    let red = main_red(&inp.runs)?;
    let now = unix(&inp.now, "now.txt")?;
    let alarm = inp
        .alarm
        .as_deref()
        .map(|t| unix(t.trim(), "judge-alarm-since"))
        .transpose()?;
    let milestone = current_milestone(&inp.milestones)?;
    let planned = planner_ran(
        &inp.records,
        &crate::metrics::fmt_day(now.div_euclid(86_400)),
    )?;
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
    if chosen.is_none() && alarm.is_none() {
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
    if alarm.is_some() {
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
    // Step 7 runs whatever is picked and parses all its inputs before it returns, so a broken
    // input always fails.
    // An open agent PR whose linked issue is open and labeled `audit` or `retro` is doing that
    // slot already (#282), whatever its checks say: a failing one is step 3's, or skipped as the
    // owner's turn. Lowest number first.
    let slot_pr = |label: &str| {
        prs.iter()
            .filter(|p| p.open)
            .find(|p| linked_issue(&p.body).is_some_and(|n| has_label(labels_of(n), label)))
            .map(|p| p.number)
    };
    let (seven, seven_notes) = step7(inp, now, alarm, slot_pr("audit"), slot_pr("retro"))?;
    notes.extend(seven_notes);
    if chosen.is_none() {
        chosen = seven;
    }
    if chosen.is_none() {
        let open = |n: u64| issues.iter().any(|i| i.number == n);
        chosen = issues
            .iter()
            .filter(workable)
            .filter(|i| milestone.is_some() && i.milestone == milestone)
            .filter(|i| has_label(&i.labels, "ready"))
            .filter(|i| !NOT_READY_LABELS.iter().any(|l| has_label(&i.labels, l)))
            .find(|i| !dependencies(&i.body).into_iter().any(open))
            .map(|i| {
                pick(
                    format!("worker ready #{}", i.number),
                    "i (the lowest-numbered ready issue in the current milestone with its dependencies closed)",
                    vec![view_issue(i.number)],
                )
            });
    }
    let p = chosen.unwrap_or_else(|| {
        if planned {
            pick(
                "idle".to_owned(),
                "j (nothing applies and the Planner ran today): exit now, write no record",
                Vec::new(),
            )
        } else {
            pick(
                "planner".to_owned(),
                "j (nothing applies and no Session: planner record is dated today): the Planner",
                vec!["ops/TICK.md §2 rule j".to_owned()],
            )
        }
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
    fn iso_time() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(86_399), "1970-01-01T23:59:59Z");
        assert_eq!(iso(1_795_500_000), "2026-11-24T06:00:00Z");
        assert_eq!(iso(1_795_327_201), "2026-11-22T06:00:01Z");
    }

    #[test]
    fn depends_on_lines() {
        assert_eq!(
            dependencies("**Depends on:** #4, #12, #13, #19."),
            [4, 12, 13, 19]
        );
        assert_eq!(
            dependencies("a #1\nDepends on: #242 (task 1a), #273"),
            [242, 273]
        );
        assert!(dependencies("Depends on: # 5, #6").is_empty());
        assert!(dependencies("Depends on: PR #7").is_empty());
        assert!(dependencies("depends on: #5\nDepends on:\n#6").is_empty());
        assert_eq!(
            dependencies("Depends on: #1 and #2 (x, #9), #3. See #4"),
            [1, 2, 3]
        );
        assert_eq!(dependencies("Depends on: #1 (open"), [1]);
        // R1.2: prose after the phrase names nothing.
        assert!(dependencies("with its Depends on: issues closed, task 2 (#244).").is_empty());
    }

    #[test]
    fn session_lines_from_a_listing() {
        let dir = "ops/journal/2026-11-24/";
        let tree = "100644 blob aa\tops/journal/2026-11-24/0100-a.md\n\
                    100644 blob bb\tops/journal/2026-11-24/sub/b.md\n\
                    100644 blob cc\t\"ops/journal/2026-11-24/\\303\\251.md\"\n\
                    120000 blob dd\tops/journal/2026-11-24/link.md\n\
                    100644 blob ee\tops/journal/2026-11-24/notes.txt\n\
                    100644 blob ff\tops/journal/2026-11-24/0200-bad.md\n\
                    100644 blob 11\tops/journal/2026-11-24/.md\n\
                    100644 blob aa\tops/journal/2026-11-24/0100-a.md\n\
                    100644 blob a2\tops/journal/2026-11-240/x.md\n\
                    no tab here\n";
        let mut read_ids = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut out = String::new();
        session_lines(tree, dir, &mut seen, &mut out, |oid| {
            read_ids.push(oid.to_owned());
            match oid {
                "aa" => Some(
                    "## x\n- Session: planner\n - Session: worker\n- Session: idle\n".to_owned(),
                ),
                // Unreadable (not UTF-8, too large): skipped, never an error.
                _ => None,
            }
        });
        assert_eq!(read_ids, ["aa", "ff"]);
        assert_eq!(
            out,
            "ops/journal/2026-11-24/0100-a.md\t- Session: planner\n\
             ops/journal/2026-11-24/0100-a.md\t- Session: idle\n"
        );
        // The same (oid, path) on a second ref is read once.
        session_lines(tree, dir, &mut seen, &mut out, |_| {
            Some("- Session: planner\n".to_owned())
        });
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn closes_link() {
        assert_eq!(linked_issue("x\nCloses #42.\nCloses #7"), Some(42));
        assert_eq!(linked_issue("closes #42"), None);
        assert_eq!(linked_issue("Closes #x"), None);
    }
}
