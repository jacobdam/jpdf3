//! `cargo xtask next-session`: the session scheduler, precedence steps 1–8, 10 and 11 (ADR 0011
//! §2, tasks 1a, 1b, 2 and 3; specs/tooling/next-session.md), with the work-mix budget of §3. Read-only: it runs read commands and writes
//! nothing.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use jpdf_testgen::json::{self, Value};

mod status;

const OWNER: &str = "jacobdam";
const TAG: &str = "🤖 jpdf-agent";
const ISSUE_URL: &str = "https://github.com/jacobdam/jpdf3/issues/";
const FEED_SCRIPT: &str = "ops/driver/owner-issues.sh";
const ALARM_FILE: &str = "ops/state/judge-alarm-since";
const SKIP_LABELS: [&str; 3] = ["owner-todo", "blocked", "deferred"];
/// The head-branch prefix of the owner's own protected changes (AUTOMATION.md §6.1, #298).
const OWNER_BRANCH: &str = "owner/";
/// The label of a PR escalated to a Design: technical session (ops/TICK.md §4 step 6, §8).
const NEEDS_DESIGN: &str = "needs-design";
/// The PR label of a design ADR PR and of a later `docs/PRODUCT.md` change (TICK §8 step 5,
/// §10 step 6): it merges after a 24-hour window unless the owner holds it (ADR 0011 §7).
const DESIGN_LABEL: &str = "design";
/// The 24-hour window (ADR 0011 §7), in seconds.
const DESIGN_WINDOW: i64 = 86_400;
/// The timeline event of a draft PR marked ready.
const READY_EVENT: &str = "ready_for_review";
/// How a review triage comment starts (ops/TICK.md §4 step 4). An `ai-review` success whose
/// `targetUrl` names such a comment on the same PR starts the design window at that comment's
/// time, so a success re-posted after a record-only update keeps the window (#373).
const TRIAGE_HEAD: &str = "🤖 jpdf-agent **AI review triage";
/// The owner's command that ends a design PR's 24-hour window early (#372, ADR 0011 §7). The
/// owner-commands workflow ignores it (it acts on `/hold` and `/release` only).
const APPROVE: &str = "/approve";
/// What a Design: technical session loads besides the PR or issue (ops/TICK.md §8 step 2).
const DESIGN_LOAD: [&str; 6] = [
    "ops/TICK.md §8",
    "specs/tooling/design-technical.md",
    "docs/PRINCIPLES.md",
    "docs/adr/_TEMPLATE-design.md",
    "docs/adr/0010-review-rounds-converge.md",
    "docs/adr/0011-session-types-and-scheduler.md §5",
];

pub struct Options {
    pub from: Option<PathBuf>,
}

pub fn parse(args: &[String]) -> Result<Options, String> {
    match args {
        [] => Ok(Options { from: None }),
        [flag, dir] if flag == "--from" => Ok(Options {
            from: Some(PathBuf::from(dir)),
        }),
        _ => Err("it takes only --from DIR".to_owned()),
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
    /// `<path>\t<line>` for each `- Session: ` line of the journal files of the last 7 UTC days
    /// (steps 8 and 11).
    records: String,
    /// The comments of each open PR (`number`, `comments`), for the owner's `/hold` (step 3).
    holds: String,
    /// The `ready_for_review` events of each open agent PR labeled `design` (`number`,
    /// `events`), for its 24-hour window (step 3, #314).
    design_ready: String,
    /// `ops/QUEUE.md` on origin/main, if it is there and readable (step 8).
    queue: Option<String>,
    /// The changed files of each agent-account PR (`number`, `files`), for the work mix (#288).
    pr_files: String,
    /// The milestone of every issue, open or closed (`number`, `milestone`), for the work mix.
    all_issues: String,
    /// `ops/areas.toml` in the current directory: which `area:` labels are product (step 10).
    areas: String,
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
/// Labels step 10 skips besides `SKIP_LABELS`. Since #275, `design` is defensive only: a
/// `design` and `ready` issue step 10 could take (no skip label, not in flight) is step 6's first.
const NOT_READY_LABELS: [&str; 4] = ["in-progress", "design", "design-pending", PRODUCT_LABEL];
/// The label of a Design: product session's issues (step 8, ops/TICK.md §10).
const PRODUCT_LABEL: &str = "design-product";
/// What a Design: product session writes first (ADR 0011 §1: its v1 trigger).
const PRODUCT_DOC: &str = "docs/PRODUCT.md";
const WEEK: i64 = 7 * 86_400;
const AUDIT_MERGES: usize = 10;
const BLOCKED_INCIDENT: usize = 3;
const JOURNAL: &str = "ops/journal/";
const SESSION_LINE: &str = "- Session: ";
const QUEUE: &str = "ops/QUEUE.md";
const MILESTONE_LINE: &str = "Milestone: ";
/// Days of journal directories read for records: today and the 6 days before it (step 8's
/// "7 days since the last plan").
const RECORD_DAYS: i64 = 7;
/// The work-mix window: the newest 20 counted session records (ADR 0011 §3, #288).
const MIX_WINDOW: usize = 20;
/// The work-mix budget: at least 70% of the window on milestone work.
const MIX_PERCENT: usize = 70;
const AREAS: &str = "ops/areas.toml";

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

/// Live `design-ready.json`: for each open agent PR labeled `design` in `prs`, the
/// `ready_for_review` events of its timeline (every page), as `[{"number":N,"events":[…]}]`.
/// Only `created_at` is kept, checked to be GitHub's fixed form, so the text is plain JSON.
fn design_ready_live(prs: &str) -> Result<String, Error> {
    let what = "gh api …/timeline";
    let mut parts = Vec::new();
    for p in parse_prs(prs).map_err(Error::Broken)? {
        if !p.design_label() {
            continue;
        }
        let n = p.number;
        let pages = gh(&format!(
            "api --paginate --slurp repos/jacobdam/jpdf3/issues/{n}/timeline?per_page=100"
        ))?;
        let v = parse_list(&pages, what).map_err(Error::Broken)?;
        let mut events = Vec::new();
        for page in list_of(&v) {
            for e in list(page, what).map_err(Error::Broken)? {
                if e.get("event").and_then(Value::as_str) != Some(READY_EVENT) {
                    continue;
                }
                let at = text(e, "created_at", what).map_err(Error::Broken)?;
                if !valid_floor(at) {
                    return Err(Error::Broken(format!(
                        "{what}: created_at {at:?} is not YYYY-MM-DDTHH:MM:SSZ"
                    )));
                }
                events.push(format!(
                    r#"{{"event":"{READY_EVENT}","created_at":"{at}"}}"#
                ));
            }
        }
        parts.push(format!(
            r#"{{"number":{n},"events":[{}]}}"#,
            events.join(",")
        ));
    }
    Ok(format!("[{}]", parts.join(",")))
}

/// Live `records.txt`: the `- Session: ` lines of the journal files dated `day` and the
/// `RECORD_DAYS - 1` days before it, on origin/main and on the head branch of each open agent
/// PR, as `<path>\t<line>`. The refs come from the last `git fetch` (TICK §1); a branch that
/// isn't fetched is exit 2.
fn records_live(prs: &str, day: i64) -> Result<String, Error> {
    let mut refs = vec!["refs/remotes/origin/main".to_owned()];
    for p in parse_prs(prs).map_err(Error::Broken)? {
        if p.open {
            // `refs/…` never reads as an option, and a ref name holds no `:` (git check-ref-format).
            refs.push(format!("refs/remotes/origin/{}", p.branch));
        }
    }
    let dirs =
        record_dirs(day).ok_or_else(|| Error::Broken("system clock out of range".to_owned()))?;
    let mut seen = std::collections::BTreeSet::new();
    let mut out = String::new();
    for r in &refs {
        // `--full-tree`: the path is from the repository root whatever the current directory
        // (R1.3). `core.quotePath`: any unusual byte in a name is C-quoted, so the listing is
        // ASCII and such a name is skipped below.
        let mut args = vec![
            "-c",
            "core.quotePath=true",
            "ls-tree",
            "-r",
            "--full-tree",
            r.as_str(),
            "--",
        ];
        args.extend(dirs.iter().map(String::as_str));
        let tree = cmd("git", &args).map_err(|e| match e {
            Error::Unknown(m) => Error::Unknown(format!("{m} (is {r} fetched? run git fetch)")),
            b @ Error::Broken(_) => b,
        })?;
        session_lines(&tree, &dirs, &mut seen, &mut out, |oid| {
            cmd("git", &["cat-file", "blob", oid]).ok()
        });
    }
    Ok(out)
}

/// The journal directories `records.txt` reads: `day` and the `RECORD_DAYS - 1` days before
/// it, oldest first, as `ops/journal/YYYY-MM-DD/`. `None` when `day` is too small.
fn record_dirs(day: i64) -> Option<Vec<String>> {
    let first = day.checked_sub(RECORD_DAYS - 1)?;
    Some(
        (first..=day)
            .map(|d| format!("{JOURNAL}{}/", crate::metrics::fmt_day(d)))
            .collect(),
    )
}

/// Appends `<path>\t<line>` to `out` for each `- Session: ` line of each journal file in a
/// `git ls-tree -r` listing (`<mode> blob <oid>\t<path>` lines) whose path is `<dir><name>.md`
/// for one of `dirs`
/// (`<name>` plain: no `/`, not C-quoted) and whose (oid, path) is not in `seen`. A blob that
/// `read` can't return (not UTF-8, over the output cap, a git error) is skipped: a missed record
/// costs one more Planner run that day, never a stopped scheduler (R1.1).
fn session_lines(
    tree: &str,
    dirs: &[String],
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
        let plain = dirs.iter().any(|dir| {
            path.strip_prefix(dir.as_str())
                .and_then(|n| n.strip_suffix(".md"))
                .is_some_and(|n| !n.is_empty() && !n.contains('/'))
        });
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

/// Live `ops/QUEUE.md` on origin/main: `None` when the tree has no regular file at that path,
/// or its blob can't be read as text (the Planner rewrites it; a stopped scheduler would stop
/// the loop). A failed `ls-tree` is exit 2, as for `records.txt`.
fn queue_live() -> Result<Option<String>, Error> {
    let tree = cmd(
        "git",
        &[
            "ls-tree",
            "--full-tree",
            "refs/remotes/origin/main",
            "--",
            QUEUE,
        ],
    )?;
    Ok(queue_blob(&tree).and_then(|oid| cmd("git", &["cat-file", "blob", oid]).ok()))
}

/// The blob id of `ops/QUEUE.md` in a `git ls-tree` listing, if it is a regular file there.
fn queue_blob(tree: &str) -> Option<&str> {
    tree.lines().find_map(|l| {
        let (meta, path) = l.split_once('\t')?;
        (path == QUEUE).then_some(())?;
        meta.strip_prefix("100644 blob ")
    })
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
            holds: read(dir, "holds.json")?,
            design_ready: read(dir, "design-ready.json")?,
            queue: optional(&dir.join("queue.md"))?,
            pr_files: read(dir, "pr-files.json")?,
            all_issues: read(dir, "all-issues.json")?,
            areas: read(dir, "areas.toml")?,
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
        "pr list --author @me --state all --limit 200 --json number,state,isDraft,autoMergeRequest,createdAt,body,headRefName,labels,statusCheckRollup,mergeable",
    )?;
    // The same day as `now.txt`, so a run at midnight reads the day step 11 compares with.
    let day = i64::try_from(now / 86_400)
        .map_err(|_| Error::Broken("system clock out of range".to_owned()))?;
    let records = records_live(&prs, day)?;
    let design_ready = design_ready_live(&prs)?;
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
        holds: gh("pr list --author @me --state open --limit 200 --json number,comments")?,
        design_ready,
        queue: queue_live()?,
        pr_files: gh("pr list --author @me --state all --limit 200 --json number,files")?,
        all_issues: gh("issue list --state all --limit 1000 --json number,milestone")?,
        areas: read(root, AREAS)?,
    })
}

/// Runs the scheduler; returns the pick and brief (stdout).
pub fn run(root: &Path, opts: &Options) -> Result<String, Error> {
    let inp = load(root, opts.from.as_deref())?;
    decide(&inp).map_err(Error::Broken)
}

/// `cargo xtask status` (specs/tooling/status.md): the generated status, from the same inputs.
pub fn status(root: &Path, opts: &Options) -> Result<String, Error> {
    let inp = load(root, opts.from.as_deref())?;
    status::render(&inp).map_err(Error::Broken)
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

/// The linked issue's labels stop an open design PR that isn't a draft: `blocked` or `deferred`
/// do (a session or the owner parked the work), `owner-todo` doesn't, since the issue carries it
/// while the PR's 24-hour window runs (TICK §8 step 5). Shared by step 3's failing-PR term (so a
/// design PR that conflicts in its window is picked, #372) and its window term (PR #333 R1.3).
fn window_parked(labels: &[String]) -> bool {
    has_label(labels, "blocked") || has_label(labels, "deferred")
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
    /// `isDraft`.
    draft: bool,
    /// `autoMergeRequest` is not null: auto-merge is on.
    auto_merge: bool,
    /// `createdAt`, GitHub's `YYYY-MM-DDTHH:MM:SSZ`.
    created: String,
    /// The newest `ai-review` status context in state `SUCCESS` on the head (read for an open
    /// PR labeled `design` only).
    review: Option<Review>,
    /// Open and on the owner's `/hold` (`held_prs`), set by `decide`.
    held: bool,
    /// Open and `mergeable` is `CONFLICTING` (part of `failing`).
    conflict: bool,
    /// Open and its rollup has a failing check or no `ai-review` success (part of `failing`).
    rollup_bad: bool,
}

impl Pr {
    /// The owner's own change: its head branch starts with `owner/` (AUTOMATION.md §6.1). The
    /// owner signs its commits, posts its `ai-review` and merges it; a session can't push to it
    /// (#298).
    fn owner_branch(&self) -> bool {
        self.branch.starts_with(OWNER_BRANCH)
    }

    /// Open, and not a session's to move: held (#295) or the owner's own change (#298).
    fn parked(&self) -> bool {
        self.open && (self.held || self.owner_branch())
    }

    /// Open and labeled `needs-design`: escalated to a design session (ops/TICK.md §4 step 6).
    /// It is a draft with `ai-review` failure on purpose, so it reads as failing; step 6 picks
    /// it, never step 3, and step 1 counts its issue in flight (#275).
    fn needs_design(&self) -> bool {
        self.open && has_label(&self.labels, NEEDS_DESIGN)
    }

    /// Open and labeled `design` (exact): a PR whose merge waits out the owner's 24-hour
    /// window (ADR 0011 §7, #314).
    fn design_label(&self) -> bool {
        self.open && has_label(&self.labels, DESIGN_LABEL)
    }
}

/// An `ai-review` success on a PR's head: when the status was posted, and the comment it
/// links (`targetUrl`, `None` when null or absent).
#[derive(Clone)]
struct Review {
    started: String,
    target: Option<String>,
}

/// The newest `ai-review` status context in state `SUCCESS` (#314), by `startedAt`.
fn review_time(rollup: &Value, what: &str) -> Result<Option<Review>, String> {
    let entries = match rollup {
        Value::Null => &[][..],
        v => list(v, what)?,
    };
    let mut newest: Option<(&str, Option<&str>)> = None;
    for e in entries {
        if e.get("__typename").and_then(Value::as_str) != Some("StatusContext")
            || text(e, "context", what)? != "ai-review"
            || text(e, "state", what)? != "SUCCESS"
        {
            continue;
        }
        let at = text(e, "startedAt", what)?;
        if !valid_floor(at) {
            return Err(format!(
                "{what}: startedAt {at:?} is not YYYY-MM-DDTHH:MM:SSZ"
            ));
        }
        let target = match e.get("targetUrl") {
            None | Some(Value::Null) => None,
            Some(Value::String(u)) => Some(u.as_str()),
            Some(_) => {
                return Err(format!(
                    "{what}: field \"targetUrl\" is not a string or null"
                ));
            }
        };
        if newest.is_none_or(|(t, _)| at > t) {
            newest = Some((at, target));
        }
    }
    Ok(newest.map(|(at, target)| Review {
        started: at.to_owned(),
        target: target.map(str::to_owned),
    }))
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
        let draft = match field(p, "isDraft", what)? {
            Value::Bool(b) => *b,
            _ => return Err(format!("{what}: field \"isDraft\" is not a boolean")),
        };
        let auto_merge = match field(p, "autoMergeRequest", what)? {
            Value::Null => false,
            Value::Object(_) => true,
            _ => {
                return Err(format!(
                    "{what}: field \"autoMergeRequest\" is not null or an object"
                ));
            }
        };
        let created = text(p, "createdAt", what)?;
        if !valid_floor(created) {
            return Err(format!(
                "{what}: createdAt {created:?} is not YYYY-MM-DDTHH:MM:SSZ"
            ));
        }
        let labels = labels(p, what)?;
        let review = if open && has_label(&labels, DESIGN_LABEL) {
            review_time(field(p, "statusCheckRollup", what)?, what)?
        } else {
            None
        };
        let pr = Pr {
            number: number(p, "number", what)?,
            open,
            body: body.to_owned(),
            branch: text(p, "headRefName", what)?.to_owned(),
            labels,
            failing: open && (conflict || rollup_bad),
            draft,
            auto_merge,
            created: created.to_owned(),
            review,
            held: false,
            conflict: open && conflict,
            rollup_bad,
        };
        // The account is shared with the owner: only tagged PRs are the agent's own.
        if tagged(body) {
            out.push(pr);
        }
    }
    out.sort_by_key(|p| p.number);
    Ok(out)
}

/// The numbers of the PRs in `holds.json` whose newest owner command is a `/hold` (#295). A
/// command is what `.github/workflows/owner-commands.yml` acts on: an owner-account comment
/// whose body starts with `/hold` or `/release` and doesn't contain the tag, both ignoring case
/// (Actions' `startsWith` and `contains` do; ASCII case here, R1.1). The workflow then holds when
/// the body starts with `/hold` exactly and releases otherwise, so `/Hold` is a release. Newest is
/// the largest `createdAt` (GitHub's `YYYY-MM-DDTHH:MM:SSZ`, so text order is time order); on a
/// tie, the later in the list.
fn held_prs(text_: &str) -> Result<Vec<u64>, String> {
    Ok(newest_commands(text_)?
        .into_iter()
        .filter(|(_, _, hold)| *hold)
        .map(|(n, _, _)| n)
        .collect())
}

/// For each PR in `holds.json` with an owner command (`held_prs`' rule), its number, the
/// newest command's `createdAt` and whether it is a hold. A release's time starts the
/// 24-hour window again (ADR 0011 §7, #314).
fn newest_commands(text_: &str) -> Result<Vec<(u64, String, bool)>, String> {
    let what = "holds.json";
    let v = parse_list(text_, what)?;
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for p in list(&v, what)? {
        let n = number(p, "number", what)?;
        let mut newest: Option<(&str, bool)> = None;
        for c in list(field(p, "comments", what)?, what)? {
            let body = text(c, "body", what)?;
            let login = match field(c, "author", what)? {
                Value::Null => None,
                a => Some(text(a, "login", what)?),
            };
            let lower = body.to_ascii_lowercase();
            let command = lower.starts_with("/hold") || lower.starts_with("/release");
            if login != Some(OWNER) || lower.contains(&TAG.to_ascii_lowercase()) || !command {
                continue;
            }
            let hold = body.starts_with("/hold");
            let at = text(c, "createdAt", what)?;
            // A release's time starts the design window (#314), so it must compare as time.
            if !valid_floor(at) {
                return Err(format!(
                    "{what}: createdAt {at:?} is not YYYY-MM-DDTHH:MM:SSZ"
                ));
            }
            if newest.is_none_or(|(t, _)| at >= t) {
                newest = Some((at, hold));
            }
        }
        if !seen.insert(n) {
            return Err(format!("{what}: PR {n} listed twice"));
        }
        if let Some((at, hold)) = newest {
            out.push((n, at.to_owned(), hold));
        }
    }
    Ok(out)
}

/// An owner `/approve` (#372): an owner-account comment without the tag (ASCII case, as the
/// command rule above) whose body starts with `/approve` in any ASCII case, followed by the end
/// or ASCII whitespace (so `/approved` is no approval).
fn approve_command(login: Option<&str>, body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    login == Some(OWNER)
        && !lower.contains(&TAG.to_ascii_lowercase())
        && lower
            .strip_prefix(APPROVE)
            .is_some_and(|rest| rest.chars().next().is_none_or(|c| c.is_ascii_whitespace()))
}

/// For each PR in `holds.json` whose newest owner command, `/approve` (`approve_command`)
/// counted with `/hold` and `/release` (`newest_commands`' rule), is an `/approve`: its
/// `createdAt` and `url` (#372; an approval needs a string `url`, which step 1 matches). Newest is the largest `createdAt`; on a tie, the later in the list. A
/// `/release` or `/hold` after it ends the approval; it never releases a hold (the hold rule
/// ignores it).
fn approvals(text_: &str) -> Result<std::collections::BTreeMap<u64, (String, String)>, String> {
    let what = "holds.json";
    let v = parse_list(text_, what)?;
    let mut out = std::collections::BTreeMap::new();
    for p in list(&v, what)? {
        let n = number(p, "number", what)?;
        let mut newest: Option<(&str, Option<&str>)> = None;
        for c in list(field(p, "comments", what)?, what)? {
            let body = text(c, "body", what)?;
            let login = match field(c, "author", what)? {
                Value::Null => None,
                a => Some(text(a, "login", what)?),
            };
            let lower = body.to_ascii_lowercase();
            let approve = approve_command(login, body);
            let url = if approve {
                Some(text(c, "url", what)?)
            } else {
                None
            };
            let other = login == Some(OWNER)
                && !lower.contains(&TAG.to_ascii_lowercase())
                && (lower.starts_with("/hold") || lower.starts_with("/release"));
            if !approve && !other {
                continue;
            }
            let at = text(c, "createdAt", what)?;
            if !valid_floor(at) {
                return Err(format!(
                    "{what}: createdAt {at:?} is not YYYY-MM-DDTHH:MM:SSZ"
                ));
            }
            if newest.is_none_or(|(t, _)| at >= t) {
                newest = Some((at, url));
            }
        }
        if let Some((at, Some(url))) = newest {
            out.insert(n, (at.to_owned(), url.to_owned()));
        }
    }
    Ok(out)
}

/// For each PR in `holds.json`, the `createdAt` of each of its triage comments by URL: an
/// owner-account comment (the account the agent shares) whose body starts with `TRIAGE_HEAD`
/// (#373). Such a comment needs a string `url`; other comments need none.
fn triage_times(text_: &str) -> Result<std::collections::BTreeMap<(u64, String), String>, String> {
    let what = "holds.json";
    let v = parse_list(text_, what)?;
    let mut out = std::collections::BTreeMap::new();
    for p in list(&v, what)? {
        let n = number(p, "number", what)?;
        for c in list(field(p, "comments", what)?, what)? {
            let login = match field(c, "author", what)? {
                Value::Null => None,
                a => Some(text(a, "login", what)?),
            };
            if login != Some(OWNER) || !text(c, "body", what)?.starts_with(TRIAGE_HEAD) {
                continue;
            }
            let at = text(c, "createdAt", what)?;
            if !valid_floor(at) {
                return Err(format!(
                    "{what}: createdAt {at:?} is not YYYY-MM-DDTHH:MM:SSZ"
                ));
            }
            out.insert((n, text(c, "url", what)?.to_owned()), at.to_owned());
        }
    }
    Ok(out)
}

/// The latest `created_at` of a `ready_for_review` event of each PR in `design-ready.json`
/// (`None`: never marked ready, so it was created ready). Events of other kinds are ignored.
fn ready_times(text_: &str) -> Result<std::collections::BTreeMap<u64, Option<String>>, String> {
    let what = "design-ready.json";
    let v = parse_list(text_, what)?;
    let mut out = std::collections::BTreeMap::new();
    for i in list(&v, what)? {
        let n = number(i, "number", what)?;
        let mut newest: Option<String> = None;
        for e in list(field(i, "events", what)?, what)? {
            if text(e, "event", what)? != READY_EVENT {
                continue;
            }
            let at = text(e, "created_at", what)?;
            if !valid_floor(at) {
                return Err(format!(
                    "{what}: created_at {at:?} is not YYYY-MM-DDTHH:MM:SSZ"
                ));
            }
            if newest.as_deref().is_none_or(|t| at > t) {
                newest = Some(at.to_owned());
            }
        }
        if out.insert(n, newest).is_some() {
            return Err(format!("{what}: PR {n} listed twice"));
        }
    }
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

/// Whether `path` is on origin/main, from the newest first-parent commit that touches it: `D`
/// means deleted, any other status means present, no such commit means it was never added.
/// `--no-renames` turns a rename into `D` plus `A`, so both ends are seen.
fn on_main(log: &[Commit], path: &str) -> bool {
    log.iter()
        .find_map(|c| c.files.iter().find(|(_, p)| p == path))
        .is_some_and(|(s, _)| s != "D")
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

/// `approved`: the URLs of owner `/approve` comments that step 3's window term acts on (#372).
/// Step 1 leaves them to that term, whose session posts both replies (TICK §8 step 5); an
/// approval that doesn't count stays rule a's, so its session replies why.
fn step1(inp: &Inputs, prs: &[Pr], approved: &[String]) -> Result<Option<Pick>, String> {
    let feed = parse_feed(&inp.feed)?;
    if !valid_floor(&inp.floor) {
        return Err(format!("floor {:?} is not YYYY-MM-DDTHH:MM:SSZ", inp.floor));
    }
    let open = parse_owner(&inp.owner_open, "owner-open.json", false)?;
    let closed = parse_owner(&inp.owner_closed, "owner-closed.json", true)?;
    let in_flight_issue = |n: u64| {
        prs.iter().any(|p| {
            p.open
                && (!p.failing || p.parked() || p.needs_design())
                && linked_issue(&p.body) == Some(n)
        })
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
                && !approved.iter().any(|a| a == url)
                && !prs.iter().any(|p| {
                    p.open
                        && (!p.failing || p.parked() || p.needs_design())
                        && names_url(&p.body, url)
                })
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

/// The lines of `records.txt` as (path, UTC day of the path, text after `- Session: `). Every
/// line is checked, so a malformed one always fails, whatever is picked.
fn parse_records(text_: &str) -> Result<Vec<(&str, i64, &str)>, String> {
    let what = "records.txt";
    let mut out = Vec::new();
    for line in text_.lines().filter(|l| !l.is_empty()) {
        let (path, record) = line
            .split_once('\t')
            .ok_or_else(|| format!("{what}: no tab in {line:?}"))?;
        let day = path
            .strip_prefix(JOURNAL)
            .and_then(|r| r.split_once('/'))
            .filter(|(_, n)| n.ends_with(".md") && !n.contains('/'))
            .and_then(|(d, _)| crate::metrics::parse_date(d))
            .ok_or_else(|| format!("{what}: {path:?} is not {JOURNAL}YYYY-MM-DD/<name>.md"))?;
        let rest = record
            .strip_prefix(SESSION_LINE)
            .ok_or_else(|| format!("{what}: {record:?} is not a {SESSION_LINE:?} line"))?;
        out.push((path, day, rest));
    }
    Ok(out)
}

/// When the Planner last ran, from `records.txt`'s `- Session: planner` lines (the first word
/// `planner`, so `planner due` counts too): `(today, week)`, whether one is in a journal file
/// dated `today` (ADR 0011 §1, the Planner's bound) and whether one is dated from `today - 6`
/// to `today` (step 8's 7-day trigger).
fn planner_ran(records: &[(&str, i64, &str)], today: i64) -> (bool, bool) {
    let (mut ran, mut week) = (false, false);
    for (_, day, rest) in records {
        if rest.split_whitespace().next() == Some("planner") {
            ran |= *day == today;
            // `today - day` can't overflow: both are days of years 0000–9999 at most.
            week |= *day <= today && today - *day < RECORD_DAYS;
        }
    }
    (ran, week)
}

/// What a session record says it worked on, for the work mix (ADR 0011 §3).
#[derive(Debug, PartialEq, Eq)]
enum Work {
    /// Not in the window: an owner instruction (`worker owner-…`, rule a) or a Design session
    /// (`design-…`; left out until task 4 adds `area:` labels).
    Excluded,
    /// In the window and never milestone work: an audit, retro or Planner session, a Worker
    /// session on no issue (main red, a leftover worktree), anything else.
    Other,
    /// A Worker or QC session on this issue.
    Issue(u64),
    /// A Worker session on this PR (`worker own-pr #N` or `worker design-window #N`, rule c).
    Pr(u64),
}

/// The work of the text after `- Session: `, by its words: the session type, the step, and for
/// a Worker or QC session a last word `#<digits>`.
fn record_work(rest: &str) -> Work {
    let words: Vec<&str> = rest.split_whitespace().collect();
    let (kind, step) = (words.first().copied(), words.get(1).copied());
    if kind.is_some_and(|k| k.starts_with("design-"))
        || (kind == Some("worker") && step.is_some_and(|s| s.starts_with("owner-")))
    {
        return Work::Excluded;
    }
    let n = words
        .last()
        .and_then(|w| w.strip_prefix('#'))
        .filter(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|d| d.parse().ok());
    match (kind, step, n) {
        (Some("worker"), Some("own-pr" | "design-window"), Some(n)) if words.len() == 3 => {
            Work::Pr(n)
        }
        (Some("worker" | "qc"), Some(_), Some(n)) if words.len() == 3 => Work::Issue(n),
        _ => Work::Other,
    }
}

/// The numbers of the PRs in `pr-files.json` that change a path of the interim product-merge
/// rule (ADR 0011 §3). Every entry is parsed, so a renamed field always fails.
fn product_prs(text_: &str) -> Result<std::collections::BTreeSet<u64>, String> {
    let what = "pr-files.json";
    let v = parse_list(text_, what)?;
    let mut out = std::collections::BTreeSet::new();
    for p in list(&v, what)? {
        let n = number(p, "number", what)?;
        let mut product = false;
        for f in list(field(p, "files", what)?, what)? {
            let path = text(f, "path", what)?;
            product |= PRODUCT_PATHS.iter().any(|d| path.starts_with(d));
        }
        if product {
            out.insert(n);
        }
    }
    Ok(out)
}

/// The milestone title of every issue in `all-issues.json` (`None` without one).
fn issue_milestones(
    text_: &str,
) -> Result<std::collections::BTreeMap<u64, Option<String>>, String> {
    let what = "all-issues.json";
    let v = parse_list(text_, what)?;
    let mut out = std::collections::BTreeMap::new();
    for i in list(&v, what)? {
        let m = match field(i, "milestone", what)? {
            Value::Null => None,
            m => Some(text(m, "title", what)?.to_owned()),
        };
        out.insert(number(i, "number", what)?, m);
    }
    Ok(out)
}

/// The `area:<name>` labels of the product areas in `ops/areas.toml`.
fn product_labels(areas: &str) -> Result<Vec<String>, String> {
    Ok(crate::issue_lint::parse_areas(areas)
        .map_err(|e| format!("{AREAS}: {e}"))?
        .into_iter()
        .filter(|a| a.kind == crate::issue_lint::Kind::Product)
        .map(|a| format!("area:{}", a.name))
        .collect())
}

/// The work-mix reading (ADR 0011 §3): how many of the newest `MIX_WINDOW` session records,
/// not counting excluded ones, are milestone work.
#[derive(Debug, PartialEq, Eq)]
enum Mix {
    /// Fewer than `MIX_WINDOW` records in the window (this many).
    Few(usize),
    /// No current milestone, so no milestone work.
    NoMilestone,
    /// This many of the `MIX_WINDOW` records are milestone work.
    Measured(usize),
}

impl Mix {
    /// Below the budget: `counted / MIX_WINDOW < MIX_PERCENT / 100`, in whole numbers.
    fn below(&self) -> bool {
        matches!(self, Mix::Measured(c) if c * 100 < MIX_PERCENT * MIX_WINDOW)
    }

    fn note(&self) -> String {
        match self {
            Mix::Few(n) => {
                format!("work mix: not measured ({n} of the {MIX_WINDOW} session records it needs)")
            }
            Mix::NoMilestone => "work mix: not measured (no current milestone)".to_owned(),
            Mix::Measured(c) => format!(
                "work mix: {c} of the last {MIX_WINDOW} sessions on milestone work ({}%; budget {MIX_PERCENT}%)",
                c * 100 / MIX_WINDOW
            ),
        }
    }
}

/// The work mix of `records` (ADR 0011 §3, interim rule until task 4): the records are ordered
/// by path (a journal file's name starts with its UTC time), each (path, text) pair once; the
/// window is the newest `MIX_WINDOW` that aren't `Work::Excluded`. A record is milestone work
/// when its issue is in `current` and an agent PR (any state) that links it changes a product
/// path; for `own-pr #N`, when PR N changes a product path and links an issue in `current`.
fn work_mix(
    records: &[(&str, i64, &str)],
    current: Option<&str>,
    prs: &[Pr],
    product: &std::collections::BTreeSet<u64>,
    milestones: &std::collections::BTreeMap<u64, Option<String>>,
) -> Mix {
    let mut sorted: Vec<(&str, &str)> = records.iter().map(|(p, _, r)| (*p, *r)).collect();
    // Stable, so lines of one file keep their order.
    sorted.sort_by_key(|(p, _)| *p);
    let mut seen = std::collections::BTreeSet::new();
    let mut window: Vec<Work> = sorted
        .into_iter()
        .filter(|pair| seen.insert(*pair))
        .map(|(_, r)| record_work(r))
        .filter(|w| *w != Work::Excluded)
        .collect();
    let Some(current) = current else {
        return Mix::NoMilestone;
    };
    if window.len() < MIX_WINDOW {
        return Mix::Few(window.len());
    }
    let window = window.split_off(window.len() - MIX_WINDOW);
    let in_current = |n: u64| {
        milestones
            .get(&n)
            .is_some_and(|m| m.as_deref() == Some(current))
    };
    let counted = window
        .iter()
        .filter(|w| match w {
            Work::Issue(n) => {
                in_current(*n)
                    && prs
                        .iter()
                        .any(|p| linked_issue(&p.body) == Some(*n) && product.contains(&p.number))
            }
            Work::Pr(n) => {
                product.contains(n)
                    && prs
                        .iter()
                        .find(|p| p.number == *n)
                        .and_then(|p| linked_issue(&p.body))
                        .is_some_and(in_current)
            }
            Work::Excluded | Work::Other => false,
        })
        .count();
    Mix::Measured(counted)
}

/// The milestone that `ops/QUEUE.md` plans: the text after `Milestone: ` on its first line
/// starting that way, trimmed. `None` when the file or the line is missing (no plan).
fn plan_milestone(queue: Option<&str>) -> Option<&str> {
    queue?
        .lines()
        .find_map(|l| l.strip_prefix(MILESTONE_LINE))
        .map(str::trim)
}

/// The pick and the brief's notes.
fn choose(inp: &Inputs) -> Result<(Pick, Vec<String>), String> {
    let mut prs = parse_prs(&inp.prs)?;
    let held = held_prs(&inp.holds)?;
    for p in &mut prs {
        p.held = p.open && held.contains(&p.number);
    }
    let commands = newest_commands(&inp.holds)?;
    let ready_at = ready_times(&inp.design_ready)?;
    let triaged = triage_times(&inp.holds)?;
    let approved = approvals(&inp.holds)?;
    let issues = parse_issues(&inp.issues)?;
    let red = main_red(&inp.runs)?;
    let now = unix(&inp.now, "now.txt")?;
    let alarm = inp
        .alarm
        .as_deref()
        .map(|t| unix(t.trim(), "judge-alarm-since"))
        .transpose()?;
    let milestone = current_milestone(&inp.milestones)?;
    let records = parse_records(&inp.records)?;
    let (planned, planned_week) = planner_ran(&records, now.div_euclid(86_400));
    // The work mix (ADR 0011 §3, #288): parsed whatever is picked, so a broken input always fails.
    let mix = work_mix(
        &records,
        milestone.as_deref(),
        &prs,
        &product_prs(&inp.pr_files)?,
        &issue_milestones(&inp.all_issues)?,
    );
    let product_areas = product_labels(&inp.areas)?;
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

    // ADR 0011 §7: the window starts at the latest of the PR marked ready, its last
    // `ai-review` success and the owner's last `/release`; at exactly 24 hours it has passed.
    // The success counts from the triage comment it links when that is one of this PR's
    // (#373): a success re-posted on a record-only update links the same comment, so the
    // window keeps its start; else from the status itself (the safe side: a restart).
    // `now` is in the years 1970–9999 (`unix`), so this can't overflow; before 1970-01-02
    // the cutoff is the epoch, which no GitHub time precedes.
    let cutoff = iso(now.saturating_sub(DESIGN_WINDOW).max(0));
    let window: Vec<(&Pr, bool)> = prs
        .iter()
        .filter(|p| {
            p.design_label()
                && !p.draft
                && !p.auto_merge
                && !p.failing
                && !p.parked()
                && !skipped(&p.labels)
                // Its issue's `owner-todo` is the window's own; `blocked` or `deferred`
                // there stops the merge (the safe side, PR #333 R1.3).
                && !linked_issue(&p.body).is_some_and(|n| window_parked(labels_of(n)))
        })
        .filter_map(|p| {
            // No entry: its ready time is unknown, so it is never picked (the safe side;
            // live, every open agent design PR gets one).
            let ready = ready_at.get(&p.number)?;
            let ready = ready.clone().unwrap_or_else(|| p.created.clone());
            let release = commands
                .iter()
                .find(|(n, _, hold)| *n == p.number && !hold)
                .map(|(_, at, _)| at.clone());
            // Not failing implies an `ai-review` success on the head; `review` is checked
            // anyway, so a PR without its time is never picked.
            let review = p.review.as_ref()?;
            let reviewed = review
                .target
                .as_ref()
                .and_then(|u| triaged.get(&(p.number, u.clone())))
                .unwrap_or(&review.started)
                .clone();
            // #372: the owner's newest command is an `/approve` at or after the review it
            // saw, so it covers the reviewed content. A record-only re-post links the same
            // triage, so it keeps the approval; a new round's triage, later, ends it.
            if approved.get(&p.number).is_some_and(|(a, _)| *a >= reviewed) {
                return Some((p, true));
            }
            [Some(ready), Some(reviewed), release]
                .into_iter()
                .flatten()
                .all(|t| t <= cutoff)
                .then_some((p, false))
        })
        .collect();
    // Step 1 leaves an approval that counts to this term (#372, PR #376 R1.1).
    let approve_urls: Vec<String> = window
        .iter()
        .filter(|(_, by_approval)| *by_approval)
        .filter_map(|(p, _)| approved.get(&p.number).map(|(_, u)| u.clone()))
        .collect();

    let mut chosen = step1(inp, &prs, &approve_urls)?;
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
            .filter(|p| p.open && p.failing && !skipped(&p.labels) && !p.parked() && !p.needs_design())
            // A design PR in its window: its issue's `owner-todo` is the window's own, so it
            // doesn't hide a conflict or a missing review there (#372: else no step picks it).
            .find(|p| {
                linked_issue(&p.body).is_none_or(|n| {
                    if p.design_label() && !p.draft {
                        !window_parked(labels_of(n))
                    } else {
                        !skipped(labels_of(n))
                    }
                })
            })
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
        chosen = window
            .first()
            .copied()
            .map(|(p, approved)| {
                let mut load = vec![format!("gh pr view {} --comments", p.number)];
                load.extend(linked_issue(&p.body).map(view_issue));
                load.push("ops/TICK.md §8 step 5".to_owned());
                pick(
                    format!("worker design-window #{}", p.number),
                    if approved {
                        "c (own design PR approved by the owner's /approve, ADR 0011 §7: enable auto-merge, remove owner-todo from its issue)"
                    } else {
                        "c (own design PR past its 24-hour window, ADR 0011 §7: enable auto-merge, remove owner-todo from its issue)"
                    },
                    load,
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
    for p in prs.iter().filter(|p| p.held) {
        notes.push(format!(
            "PR #{} is on the owner's /hold (no /release after it): step 3 doesn't pick it",
            p.number
        ));
    }
    for p in prs.iter().filter(|p| p.open && p.owner_branch()) {
        notes.push(format!(
            "PR #{} is the owner's own change (an owner/ branch, AUTOMATION.md §6.1): step 3 doesn't pick it",
            p.number
        ));
    }
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
    // Step 6 (#275; ADR 0011 §2, ops/TICK.md §8): a Design: technical session on an open agent PR
    // labeled `needs-design`, then on an open issue labeled `design` and `ready`. Skip labels
    // count on the PR and its linked issue, as in step 3; a held or `owner/` PR waits for the
    // owner (TICK §5).
    if chosen.is_none() {
        chosen = prs
            .iter()
            .filter(|p| p.needs_design() && !skipped(&p.labels) && !p.parked())
            .find(|p| linked_issue(&p.body).is_none_or(|n| !skipped(labels_of(n))))
            .map(|p| {
                let mut load = vec![format!("gh pr view {} --comments", p.number)];
                if let Some(n) = linked_issue(&p.body) {
                    load.push(view_issue(n));
                    load.push(format!(
                        "gh pr list --state all --search '\"Closes #{n}\" in:body'"
                    ));
                }
                load.extend(DESIGN_LOAD.iter().map(|l| (*l).to_owned()));
                pick(
                    format!("design-technical needs-design #{}", p.number),
                    "step 6 (a PR escalated to design, ops/TICK.md §4 step 6: one design output, §8)",
                    load,
                )
            });
    }
    if chosen.is_none() {
        chosen = issues
            .iter()
            .filter(workable)
            .find(|i| has_label(&i.labels, "design") && has_label(&i.labels, "ready"))
            .map(|i| {
                let mut load = vec![view_issue(i.number)];
                load.extend(DESIGN_LOAD.iter().map(|l| (*l).to_owned()));
                pick(
                    format!("design-technical design-issue #{}", i.number),
                    "step 6 (a ready design issue: an ADR or principle, or one owner question, ops/TICK.md §8)",
                    load,
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
    // Step 8: the Planner is due (ADR 0011 §1) when the current milestone has no plan in
    // ops/QUEUE.md on origin/main, or no Planner record is dated in the last 7 UTC days; at most
    // once a day.
    if chosen.is_none() && !planned {
        let mut why = Vec::new();
        if let Some(m) = &milestone
            && plan_milestone(inp.queue.as_deref()) != Some(m.as_str())
        {
            // The file's text is not printed (Security: no file or issue text in the brief).
            why.push(match inp.queue {
                None => format!("{QUEUE} is missing on origin/main or unreadable"),
                Some(_) => format!("{QUEUE} on origin/main doesn't plan the current milestone {m}"),
            });
        }
        if !planned_week {
            why.push("no Session: planner record is dated in the last 7 UTC days".to_owned());
        }
        if let Mix::Measured(c) = mix
            && mix.below()
        {
            why.push(format!(
                "the work mix is below {MIX_PERCENT}%: {c} of the last {MIX_WINDOW} sessions on milestone work"
            ));
        }
        if !why.is_empty() {
            chosen = Some(pick(
                "planner due".to_owned(),
                &format!("j (the Planner is due: {})", why.join("; ")),
                vec![
                    "ops/TICK.md §9".to_owned(),
                    QUEUE.to_owned(),
                    "PLAN.md §8".to_owned(),
                ],
            ));
        }
    }
    // Step 8, after the Planner: Design: product (ADR 0011 §1, ops/TICK.md §10). Off while an
    // open agent PR links an open `design-product` issue (one product PR at a time, as #282
    // does for audits); `docs/PRODUCT.md` missing on origin/main → v1, else the lowest
    // `design-product` issue labeled `ready`, without a skip label, with its dependencies
    // closed (step 10's test). An in-flight one is already covered by the slot rule.
    let product_pr = slot_pr(PRODUCT_LABEL);
    let has_doc = on_main(&parse_log(&inp.main_log)?, PRODUCT_DOC);
    let open = |n: u64| issues.iter().any(|i| i.number == n);
    // An open `design-product` issue is workable without a skip label and with its
    // dependencies closed (step 10's test); in flight is the slot rule's.
    let product_issues: Vec<&Issue> = issues
        .iter()
        .filter(|i| has_label(&i.labels, PRODUCT_LABEL))
        .collect();
    let workable_product =
        |i: &&&Issue| !skipped(&i.labels) && !dependencies(&i.body).into_iter().any(open);
    let issue_load = |n: u64, doc: bool| {
        let mut l = vec![view_issue(n), "ops/TICK.md §10".to_owned()];
        if doc {
            l.push(PRODUCT_DOC.to_owned());
        }
        l.extend(["PLAN.md §1, §4".to_owned(), "specs/journeys/".to_owned()]);
        l
    };
    let product = if has_doc {
        product_issues
            .iter()
            .filter(workable_product)
            .find(|i| has_label(&i.labels, "ready"))
            .map(|i| {
                (
                    format!("#{}", i.number),
                    format!("a ready {PRODUCT_LABEL} issue"),
                    issue_load(i.number, true),
                )
            })
    } else {
        let why = format!("{PRODUCT_DOC} is not on origin/main");
        // v1's issues are the `design-product` ones without `ready` (ready ones are gap
        // issues for after v1, R4.2): the lowest is worked, one is filed when there is none,
        // and nothing is picked while any of them waits.
        let v1: Vec<&&Issue> = product_issues
            .iter()
            .filter(|i| !has_label(&i.labels, "ready"))
            .collect();
        let waiting = v1.iter().any(|i| !workable_product(i));
        match v1.first() {
            Some(i) if !waiting => Some((
                format!("v1 #{}", i.number),
                why,
                issue_load(i.number, false),
            )),
            // With a product PR open, the slot rule's note says why instead.
            _ if v1.is_empty() || product_pr.is_some() => Some((
                "v1".to_owned(),
                why,
                vec![
                    "ops/TICK.md §10".to_owned(),
                    "PLAN.md §1, §4".to_owned(),
                    "specs/journeys/".to_owned(),
                ],
            )),
            _ => {
                notes.push(format!(
                    "Design: product due ({why}) but an open {PRODUCT_LABEL} issue without ready has a skip label or an open dependency: not picked"
                ));
                None
            }
        }
    };
    if let Some((reference, why, load)) = product {
        match product_pr {
            Some(n) => notes.push(format!(
                "Design: product due ({why}) but PR #{n} is open for it: not picked"
            )),
            None if chosen.is_none() => {
                chosen = Some(pick(
                    format!("design product {reference}"),
                    &format!("k (Design: product is due: {why})"),
                    load,
                ));
            }
            None => {}
        }
    }
    if chosen.is_none() {
        let ready: Vec<&Issue> = issues
            .iter()
            .filter(workable)
            .filter(|i| milestone.is_some() && i.milestone == milestone)
            .filter(|i| has_label(&i.labels, "ready"))
            .filter(|i| !NOT_READY_LABELS.iter().any(|l| has_label(&i.labels, l)))
            .filter(|i| !dependencies(&i.body).into_iter().any(open))
            .collect();
        // Below the work-mix budget, a product issue (a product `area:` label) comes first; the
        // budget orders, it never blocks (ADR 0011 §3).
        let is_product = |i: &&Issue| product_areas.iter().any(|a| has_label(&i.labels, a));
        let product_first = mix
            .below()
            .then(|| ready.iter().copied().find(is_product))
            .flatten();
        chosen = product_first.or(ready.first().copied()).map(|i| {
            let rule = if mix.below() {
                format!(
                    "i (the work mix is below {MIX_PERCENT}%: the lowest-numbered ready product issue in the current milestone with its dependencies closed, {})",
                    if product_first.is_some() {
                        "this one"
                    } else {
                        "none, so the lowest-numbered ready issue"
                    }
                )
            } else {
                "i (the lowest-numbered ready issue in the current milestone with its dependencies closed)".to_owned()
            };
            pick(
                format!("worker ready #{}", i.number),
                &rule,
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
                vec![
                    "ops/TICK.md §9".to_owned(),
                    QUEUE.to_owned(),
                    "PLAN.md §8".to_owned(),
                ],
            )
        }
    });
    // The Planner writes the reading into ops/QUEUE.md (ops/TICK.md §9).
    if p.first.starts_with("planner") {
        notes.push(mix.note());
    }
    Ok((p, notes))
}

fn decide(inp: &Inputs) -> Result<String, String> {
    let (p, notes) = choose(inp)?;
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
        // Two of the window's days; a file of a day outside `dirs` is not read.
        let dir = &[
            "ops/journal/2026-11-24/".to_owned(),
            "ops/journal/2026-11-18/".to_owned(),
        ];
        let tree = "100644 blob aa\tops/journal/2026-11-24/0100-a.md\n\
                    100644 blob a3\tops/journal/2026-11-18/0900-w.md\n\
                    100644 blob a4\tops/journal/2026-11-17/0900-v.md\n\
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
                "a3" => Some("- Session: worker ready #5\n".to_owned()),
                // Unreadable (not UTF-8, too large): skipped, never an error.
                _ => None,
            }
        });
        assert_eq!(read_ids, ["aa", "a3", "ff"]);
        assert_eq!(
            out,
            "ops/journal/2026-11-24/0100-a.md\t- Session: planner\n\
             ops/journal/2026-11-24/0100-a.md\t- Session: idle\n\
             ops/journal/2026-11-18/0900-w.md\t- Session: worker ready #5\n"
        );
        // The same (oid, path) on a second ref is read once.
        session_lines(tree, dir, &mut seen, &mut out, |_| {
            Some("- Session: planner\n".to_owned())
        });
        assert_eq!(out.lines().count(), 3);
    }

    #[test]
    fn record_dirs_are_seven_days_to_today() {
        // 2026-11-24 is day 20 781 since 1970.
        assert_eq!(
            record_dirs(20_781).unwrap(),
            [
                "ops/journal/2026-11-18/",
                "ops/journal/2026-11-19/",
                "ops/journal/2026-11-20/",
                "ops/journal/2026-11-21/",
                "ops/journal/2026-11-22/",
                "ops/journal/2026-11-23/",
                "ops/journal/2026-11-24/",
            ]
        );
        assert!(record_dirs(i64::MIN).is_none());
    }

    #[test]
    fn queue_blob_from_a_listing() {
        assert_eq!(queue_blob("100644 blob ab12\tops/QUEUE.md\n"), Some("ab12"));
        // Not a regular file, another path, a path that only starts the same, no tab.
        assert_eq!(queue_blob("120000 blob ab12\tops/QUEUE.md\n"), None);
        assert_eq!(queue_blob("040000 tree ab12\tops/QUEUE.md\n"), None);
        assert_eq!(queue_blob("100644 blob ab12\tops/QUEUE.mdx\n"), None);
        assert_eq!(queue_blob("100644 blob ab12 ops/QUEUE.md\n"), None);
        assert_eq!(queue_blob(""), None);
    }

    #[test]
    fn plan_milestone_line() {
        assert_eq!(
            plan_milestone(Some("# Q\nMilestone: M3 \nMilestone: M4\n")),
            Some("M3")
        );
        assert_eq!(
            plan_milestone(Some("# Q\n Milestone: M3\nmilestone: M3\n")),
            None
        );
        assert_eq!(plan_milestone(None), None);
    }

    #[test]
    fn record_work_words() {
        assert_eq!(record_work("worker ready #801"), Work::Issue(801));
        assert_eq!(record_work("worker  flaky\t#7 "), Work::Issue(7));
        assert_eq!(record_work("qc any #8"), Work::Issue(8));
        assert_eq!(record_work("worker own-pr #9"), Work::Pr(9));
        // #314: a design-window session works a PR, as own-pr does.
        assert_eq!(record_work("worker design-window #9"), Work::Pr(9));
        assert_eq!(record_work("worker design-windows #9"), Work::Issue(9));
        for excluded in [
            "worker owner-issue #1",
            "worker owner-comment https://x/1#issuecomment-2",
            "worker owner-done #1",
            "worker owner-x",
            "design-technical needs-design #3",
            "design-product",
        ] {
            assert_eq!(record_work(excluded), Work::Excluded, "{excluded}");
        }
        for other in [
            "",
            "planner",
            "planner due",
            "audit due",
            "worker main-red run 5",
            "worker leftover-worktree t/5-x",
            "worker ready #5 x",
            "worker ready x #5",
            "worker ready 5",
            "worker ready #",
            "worker ready #5x",
            "worker ready #-5",
            "worker #5",
            "qc #5",
            "Worker ready #5",
            "design technical #5",
            "worker owner",
            "worker ready #99999999999999999999999",
        ] {
            assert_eq!(record_work(other), Work::Other, "{other}");
        }
    }

    #[test]
    fn closes_link() {
        assert_eq!(linked_issue("x\nCloses #42.\nCloses #7"), Some(42));
        assert_eq!(linked_issue("closes #42"), None);
        assert_eq!(linked_issue("Closes #x"), None);
    }
}
