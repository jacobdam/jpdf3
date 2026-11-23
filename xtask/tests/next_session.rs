//! `cargo xtask next-session --from` on built fixtures (specs/tooling/next-session.md). The base
//! fixture selects nothing (`legacy`); each test changes one input so one step applies, and
//! its near misses change one filter so it doesn't.
#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const TAG: &str = "🤖 jpdf-agent";
const ISSUES: &str = "https://github.com/jacobdam/jpdf3/issues/";
const FLOOR: &str = "2026-11-23T00:00:00Z";

fn owner_reply(body: &str) -> String {
    format!(r#"{{"author":{{"login":"jacobdam"}},"body":{body:?}}}"#)
}

fn done(n: u64) -> String {
    owner_reply(&format!("{TAG} done: {ISSUES}{n}\n- merged"))
}

fn labels(names: &[&str]) -> String {
    let l: Vec<String> = names
        .iter()
        .map(|n| format!(r#"{{"name":{n:?}}}"#))
        .collect();
    format!("[{}]", l.join(","))
}

fn issue(n: u64, title: &str, l: &[&str]) -> String {
    format!(
        r#"{{"number":{n},"title":{title:?},"labels":{}}}"#,
        labels(l)
    )
}

const GREEN: &str = r#"[{"__typename":"CheckRun","name":"policy","conclusion":"SUCCESS"},
 {"__typename":"CheckRun","name":"project-linux","conclusion":""},
 {"__typename":"StatusContext","context":"ai-review","state":"SUCCESS"}]"#;

fn pr(n: u64, state: &str, body: &str, branch: &str, l: &[&str], rollup: &str) -> String {
    format!(
        r#"{{"number":{n},"state":{state:?},"isDraft":false,"body":{body:?},"headRefName":{branch:?},"labels":{},"statusCheckRollup":{rollup}}}"#,
        labels(l)
    )
}

fn feed_issue(n: u64) -> String {
    format!(r#"{{"kind":"issue","number":{n},"title":"t","labels":[],"is_pr":false,"body":"b"}}"#)
}

fn feed_comment(kind: &str, url: &str, at: &str) -> String {
    format!(r#"{{"kind":{kind:?},"url":{url:?},"created_at":{at:?},"stale":false,"body":"b"}}"#)
}

fn run_entry(id: u64, status: &str, conclusion: &str, event: &str) -> String {
    format!(
        r#"{{"databaseId":{id},"status":{status:?},"conclusion":{conclusion:?},"event":{event:?}}}"#
    )
}

/// The base inputs: an owner issue with its done: reply, a pre-floor owner comment, a green
/// open PR and a merged one, a deferred sev:high issue, a red PR run and a green main run,
/// and worktrees on both PR branches plus a non-`t/` branch.
fn base() -> Vec<(&'static str, String)> {
    vec![
        (
            "feed.jsonl",
            format!(
                "{}\n\n{}\n",
                feed_issue(238),
                feed_comment(
                    "comment",
                    &format!("{ISSUES}31#issuecomment-70"),
                    "2026-11-20T16:51:14Z"
                )
            ),
        ),
        ("floor.txt", format!("{FLOOR}\n")),
        (
            "owner-open.json",
            format!(
                r#"[{{"number":238,"labels":{},"comments":[{},{{"author":null,"body":"x"}}]}}]"#,
                labels(&["human"]),
                done(238)
            ),
        ),
        ("owner-closed.json", "[]".to_owned()),
        (
            "prs.json",
            format!(
                "[{},{}]",
                pr(300, "OPEN", "Closes #301", "t/301-y", &[], GREEN),
                pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]")
            ),
        ),
        (
            "issues.json",
            format!(
                "[{},{},{}]",
                issue(301, "feature", &["ready"]),
                issue(146, "memory", &["security", "sev:high", "deferred"]),
                issue(150, "low", &["security", "sev:low"])
            ),
        ),
        (
            "runs.json",
            format!(
                "[{},{}]",
                run_entry(11, "completed", "failure", "pull_request"),
                run_entry(10, "completed", "success", "push")
            ),
        ),
        (
            "worktrees.txt",
            "worktree /w/main\nHEAD a\nbranch refs/heads/main\n\nworktree /w/a\nHEAD b\nbranch refs/heads/t/298-x\n\n\
             worktree /w/b\nHEAD c\nbranch refs/heads/t/301-y\n\nworktree /w/c\nHEAD d\nbranch refs/heads/owner/x\n\n\
             worktree /w/d\nHEAD e\ndetached\n"
                .to_owned(),
        ),
    ]
}

static N: AtomicUsize = AtomicUsize::new(0);

/// One file of `base()` (empty if there is none).
fn base_of(name: &str) -> String {
    base()
        .into_iter()
        .find(|(n, _)| *n == name)
        .map(|(_, t)| t)
        .unwrap_or_default()
}

/// Writes `base()` with `changes` applied (`None` removes the file) and runs the scheduler.
fn run_with(changes: &[(&str, Option<String>)]) -> std::io::Result<Output> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "next-session-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    let mut files: Vec<(String, String)> =
        base().into_iter().map(|(n, t)| (n.to_owned(), t)).collect();
    for (name, text) in changes {
        files.retain(|(n, _)| n != name);
        if let Some(t) = text {
            files.push(((*name).to_owned(), t.clone()));
        }
    }
    for (name, text) in &files {
        fs::write(dir.join(name), text)?;
    }
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["next-session", "--from", &dir.to_string_lossy()])
        .output()
}

/// The first line printed, or a description of the failure (so the caller's assertion shows it).
fn pick(changes: &[(&str, Option<String>)]) -> String {
    match run_with(changes) {
        Err(e) => format!("io error: {e}"),
        Ok(o) if o.status.code() != Some(0) => format!(
            "exit {:?}: {}",
            o.status.code(),
            String::from_utf8_lossy(&o.stderr)
        ),
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned(),
    }
}

fn set(name: &str, text: String) -> (&str, Option<String>) {
    (name, Some(text))
}

/// base() with `extra` objects appended to the JSON array in `name`.
fn plus(name: &'static str, extra: &[String]) -> (&'static str, Option<String>) {
    let text = base_of(name);
    let t = text.trim();
    let inner = t
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .unwrap_or_default();
    let mut items: Vec<String> = Vec::new();
    if !inner.trim().is_empty() {
        items.push(inner.to_owned());
    }
    items.extend(extra.iter().cloned());
    (name, Some(format!("[{}]", items.join(","))))
}

fn feed_plus(lines: &[String]) -> (&'static str, Option<String>) {
    let text = base_of("feed.jsonl");
    ("feed.jsonl", Some(format!("{text}{}\n", lines.join("\n"))))
}

#[test]
fn base_is_legacy_with_notes() {
    let o = run_with(&[]).unwrap();
    assert_eq!(o.status.code(), Some(0));
    let out = String::from_utf8(o.stdout).unwrap();
    assert_eq!(
        out,
        "legacy\n\nRule: none of steps 1–5 applies; use TICK §2 rules g–j\nLoad:\n- ops/TICK.md §2 rules g–j\n\
         Notes:\n- worktree on t/298-x: its PRs (#299) are merged or closed; remove it\n"
    );
}

#[test]
fn step1_owner_issue() {
    // An owner issue missing from owner-open.json counts as pending.
    assert_eq!(
        pick(&[feed_plus(&[feed_issue(251), feed_issue(250)])]),
        "worker owner-issue #250"
    );
    let open = |extra: &str| {
        plus(
            "owner-open.json",
            &[format!(
                r#"{{"number":250,"labels":{},"comments":[{extra}]}}"#,
                labels(&[])
            )],
        )
    };
    let with = |extra: &str| pick(&[feed_plus(&[feed_issue(250)]), open(extra)]);
    assert_eq!(with(&done(250)), "legacy");
    assert_eq!(with(&done(2500)), "worker owner-issue #250", "a longer URL");
    assert_eq!(
        with(&format!(
            r#"{{"author":{{"login":"stranger"}},"body":"{TAG} done: {ISSUES}250"}}"#
        )),
        "worker owner-issue #250",
        "a stranger's done: reply"
    );
    assert_eq!(
        with(&owner_reply(&format!("{TAG} picked up {ISSUES}250"))),
        "worker owner-issue #250"
    );
    for skip in ["owner-todo", "blocked", "deferred"] {
        let l = plus(
            "owner-open.json",
            &[format!(
                r#"{{"number":250,"labels":{},"comments":[]}}"#,
                labels(&[skip])
            )],
        );
        assert_eq!(
            pick(&[feed_plus(&[feed_issue(250)]), l]),
            "legacy",
            "{skip}"
        );
    }
    let l = plus(
        "owner-open.json",
        &[format!(
            r#"{{"number":250,"labels":{},"comments":[]}}"#,
            labels(&["human"])
        )],
    );
    assert_eq!(
        pick(&[feed_plus(&[feed_issue(250)]), l]),
        "worker owner-issue #250"
    );
    // Linked by an open green PR: in flight. A failing or merged one doesn't count.
    let linked = |state: &str, rollup: &str| {
        pick(&[
            feed_plus(&[feed_issue(250)]),
            plus(
                "prs.json",
                &[pr(310, state, "x\nCloses #250", "t/250-z", &[], rollup)],
            ),
        ])
    };
    assert_eq!(linked("OPEN", GREEN), "legacy");
    assert_eq!(linked("OPEN", "[]"), "worker owner-issue #250");
    assert_eq!(linked("MERGED", GREEN), "worker owner-issue #250");
}

#[test]
fn step1_owner_comment() {
    let url = format!("{ISSUES}40#issuecomment-123");
    let review = "https://github.com/jacobdam/jpdf3/pull/41#pullrequestreview-9";
    let c = |at: &str| feed_comment("comment", &url, at);
    assert_eq!(
        pick(&[feed_plus(&[c(FLOOR)])]),
        format!("worker owner-comment {url}")
    );
    assert_eq!(
        pick(&[feed_plus(&[c("2026-11-22T23:59:59Z")])]),
        "legacy",
        "pre-floor"
    );
    // Oldest first, whatever the kind and feed order.
    assert_eq!(
        pick(&[feed_plus(&[
            c("2026-11-24T00:00:00Z"),
            feed_comment("review", review, "2026-11-23T10:00:00Z"),
        ])]),
        format!("worker owner-comment {review}")
    );
    let in_pr = |state: &str, body: String, rollup: &str| {
        pick(&[
            feed_plus(&[c(FLOOR)]),
            plus("prs.json", &[pr(320, state, &body, "t/9-q", &[], rollup)]),
        ])
    };
    assert_eq!(
        in_pr("OPEN", format!("Owner instruction: {url}"), GREEN),
        "legacy"
    );
    assert_eq!(
        in_pr("OPEN", format!("Owner instruction: {url}4"), GREEN),
        format!("worker owner-comment {url}")
    );
    assert_eq!(
        in_pr("OPEN", format!("Owner instruction: {url}"), "[]"),
        format!("worker owner-comment {url}")
    );
    assert_eq!(
        in_pr("CLOSED", format!("Owner instruction: {url}"), GREEN),
        format!("worker owner-comment {url}")
    );
    // Owner issues come before owner comments.
    assert_eq!(
        pick(&[feed_plus(&[c(FLOOR), feed_issue(260)])]),
        "worker owner-issue #260"
    );
}

#[test]
fn step1_owner_done() {
    let closed = |body: &str, comments: &[String]| {
        pick(&[set(
            "owner-closed.json",
            format!(
                r#"[{{"number":173,"body":{body:?},"comments":[{}]}},{{"number":172,"body":"b","comments":[]}}]"#,
                comments.join(",")
            ),
        )])
    };
    let picked = owner_reply(&format!("{TAG} picked up {ISSUES}173 (session x)"));
    assert_eq!(
        closed("Owner text", std::slice::from_ref(&picked)),
        "worker owner-done #173"
    );
    assert_eq!(closed("Owner text", &[picked.clone(), done(173)]), "legacy");
    assert_eq!(
        closed("Owner text", &[picked.clone(), done(1730)]),
        "worker owner-done #173"
    );
    assert_eq!(
        closed(&format!(" {TAG} filed for the owner"), &[picked]),
        "legacy",
        "tagged"
    );
    assert_eq!(closed("Owner text", &[]), "legacy");
    let stranger = format!(r#"{{"author":{{"login":"x"}},"body":"{TAG} picked up"}}"#);
    assert_eq!(closed("Owner text", &[stranger]), "legacy");
}

#[test]
fn step2_main_red() {
    let runs = |r: &[String]| set("runs.json", format!("[{}]", r.join(",")));
    let red = |c: &str, e: &str| {
        pick(&[runs(&[
            run_entry(20, "completed", c, e),
            run_entry(10, "completed", "success", "push"),
        ])])
    };
    assert_eq!(red("failure", "push"), "worker main-red run 20");
    assert_eq!(red("timed_out", "schedule"), "worker main-red run 20");
    assert_eq!(
        red("startup_failure", "workflow_dispatch"),
        "worker main-red run 20"
    );
    assert_eq!(red("failure", "pull_request"), "legacy");
    assert_eq!(red("cancelled", "push"), "legacy");
    // The newest completed run decides, by id, not list order.
    let newer_green = runs(&[
        run_entry(10, "completed", "failure", "push"),
        run_entry(20, "completed", "success", "push"),
    ]);
    assert_eq!(pick(&[newer_green]), "legacy");
    let running = runs(&[
        run_entry(30, "in_progress", "", "push"),
        run_entry(20, "completed", "failure", "push"),
    ]);
    assert_eq!(
        pick(std::slice::from_ref(&running)),
        "worker main-red run 20"
    );
    // A judge alarm skips the step and says so.
    let o = run_with(&[running, set("judge-alarm-since", "1795324800\n".to_owned())]).unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.starts_with("legacy\n"), "{out}");
    assert!(out.contains("- judge alarm active (ops/state/judge-alarm-since exists): step 2 skipped, main is red\n"), "{out}");
    // Rule b outranks rule c.
    let failing = plus(
        "prs.json",
        &[pr(330, "OPEN", "Closes #301", "t/1-a", &[], "[]")],
    );
    assert_eq!(
        pick(&[
            failing,
            runs(&[run_entry(20, "completed", "failure", "push")])
        ]),
        "worker main-red run 20"
    );
}

#[test]
fn step3_own_pr() {
    // Every list keeps base()'s merged PR #299, so its worktree is no leftover.
    const PRS: &str = "prs.json";
    let merged = pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]");
    let one = |rollup: &str, l: &[&str]| {
        pick(&[set(
            PRS,
            format!(
                "[{},{merged}]",
                pr(340, "OPEN", "Closes #301", "t/301-y", l, rollup)
            ),
        )])
    };
    let ok = r#"{"__typename":"StatusContext","context":"ai-review","state":"SUCCESS"}"#;
    assert_eq!(one(&format!("[{ok}]"), &[]), "legacy");
    assert_eq!(
        one("null", &[]),
        "worker own-pr #340",
        "no checks, no ai-review"
    );
    for c in [
        "FAILURE",
        "TIMED_OUT",
        "STARTUP_FAILURE",
        "ACTION_REQUIRED",
        "CANCELLED",
    ] {
        let r = format!(r#"[{ok},{{"__typename":"CheckRun","name":"j","conclusion":"{c}"}}]"#);
        assert_eq!(one(&r, &[]), "worker own-pr #340", "{c}");
    }
    for c in ["SUCCESS", "SKIPPED", "NEUTRAL", ""] {
        let r = format!(r#"[{ok},{{"__typename":"CheckRun","name":"j","conclusion":"{c}"}}]"#);
        assert_eq!(one(&r, &[]), "legacy", "{c}");
    }
    for s in ["FAILURE", "ERROR"] {
        let r =
            format!(r#"[{ok},{{"__typename":"StatusContext","context":"judge","state":"{s}"}}]"#);
        assert_eq!(one(&r, &[]), "worker own-pr #340", "{s}");
    }
    for s in ["PENDING", "FAILURE"] {
        let r =
            format!(r#"[{{"__typename":"StatusContext","context":"ai-review","state":"{s}"}}]"#);
        assert_eq!(one(&r, &[]), "worker own-pr #340", "ai-review {s}");
    }
    let other = r#"[{"__typename":"StatusContext","context":"ai-review-x","state":"SUCCESS"}]"#;
    assert_eq!(
        one(other, &[]),
        "worker own-pr #340",
        "another context's success"
    );
    assert_eq!(one("[]", &["owner-todo"]), "legacy");
    // A skip label on the linked issue parks the PR too.
    let parked = set(
        "issues.json",
        format!("[{}]", issue(301, "f", &["blocked"])),
    );
    let failing = set(
        PRS,
        format!(
            "[{},{merged}]",
            pr(340, "OPEN", "Closes #301", "t/301-y", &[], "[]")
        ),
    );
    assert_eq!(pick(&[failing.clone(), parked]), "legacy");
    // Lowest number first; merged PRs never count.
    let two = set(
        PRS,
        format!(
            "[{},{},{}]",
            pr(342, "OPEN", "", "t/2-b", &[], "[]"),
            pr(341, "OPEN", "", "t/1-a", &[], "[]"),
            pr(339, "MERGED", "", "t/0-a", &[], "[]")
        ),
    );
    assert_eq!(pick(&[two]), "worker own-pr #341");
}

#[test]
fn step3_scheduler_bug() {
    let bug = |title: &str, l: &[&str]| pick(&[plus("issues.json", &[issue(400, title, l)])]);
    assert_eq!(
        bug("next-session: parse error", &["bug"]),
        "worker scheduler-bug #400"
    );
    assert_eq!(bug("next-session: parse error", &[]), "legacy");
    assert_eq!(bug("next-session parse error", &["bug"]), "legacy");
    assert_eq!(bug("next-session: x", &["bug", "owner-todo"]), "legacy");
    let flight = plus(
        "prs.json",
        &[pr(401, "OPEN", "Closes #400", "t/400-a", &[], GREEN)],
    );
    assert_eq!(
        pick(&[
            plus("issues.json", &[issue(400, "next-session: x", &["bug"])]),
            flight
        ]),
        "legacy"
    );
    // Rule c's own PR comes first, then the bug issue, then rule d.
    let both = plus(
        "issues.json",
        &[
            issue(400, "next-session: x", &["bug"]),
            issue(5, "c", &["crash"]),
        ],
    );
    assert_eq!(
        pick(std::slice::from_ref(&both)),
        "worker scheduler-bug #400"
    );
    let failing = plus("prs.json", &[pr(402, "OPEN", "", "t/1-a", &[], "[]")]);
    assert_eq!(pick(&[both, failing]), "worker own-pr #402");
}

#[test]
fn step4_crash_security() {
    let one = |l: &[&str]| pick(&[plus("issues.json", &[issue(500, "x", l)])]);
    assert_eq!(one(&["crash"]), "worker crash-security #500");
    assert_eq!(one(&["security", "sev:high"]), "worker crash-security #500");
    assert_eq!(
        one(&["security", "sev:critical"]),
        "worker crash-security #500"
    );
    assert_eq!(
        one(&["security"]),
        "worker crash-security #500",
        "unlabeled: label it first"
    );
    assert_eq!(one(&["security", "sev:medium"]), "legacy");
    assert_eq!(one(&["sev:high"]), "legacy", "not a security issue");
    assert_eq!(one(&["crash", "owner-todo"]), "legacy");
    assert_eq!(one(&["crash", "blocked"]), "legacy");
    let flight = plus(
        "prs.json",
        &[pr(501, "OPEN", "Closes #500", "t/500-a", &[], GREEN)],
    );
    assert_eq!(
        pick(&[plus("issues.json", &[issue(500, "x", &["crash"])]), flight]),
        "legacy"
    );
    // Order: sev:critical, then security without sev:*, then crash and sev:high, each lowest first.
    let mix = |extra: &[String]| {
        let mut v = vec![
            issue(510, "c", &["crash"]),
            issue(509, "h", &["security", "sev:high"]),
        ];
        v.extend(extra.iter().cloned());
        pick(&[plus("issues.json", &v)])
    };
    assert_eq!(mix(&[]), "worker crash-security #509");
    assert_eq!(
        mix(&[issue(530, "u", &["security"])]),
        "worker crash-security #530"
    );
    assert_eq!(
        mix(&[
            issue(530, "u", &["security"]),
            issue(540, "k", &["security", "sev:critical"])
        ]),
        "worker crash-security #540"
    );
}

#[test]
fn step5_flaky_and_leftover() {
    assert_eq!(
        pick(&[plus("issues.json", &[issue(600, "f", &["flaky"])])]),
        "worker flaky #600"
    );
    assert_eq!(
        pick(&[plus(
            "issues.json",
            &[issue(600, "f", &["flaky", "deferred"])]
        )]),
        "legacy"
    );
    let flight = plus(
        "prs.json",
        &[pr(601, "OPEN", "Closes #600", "t/600-a", &[], GREEN)],
    );
    assert_eq!(
        pick(&[plus("issues.json", &[issue(600, "f", &["flaky"])]), flight]),
        "legacy"
    );
    // Rule d outranks rule e.
    assert_eq!(
        pick(&[plus(
            "issues.json",
            &[issue(600, "f", &["flaky"]), issue(700, "c", &["crash"])]
        )]),
        "worker crash-security #700"
    );
    let (_, wt) = base()
        .into_iter()
        .find(|(n, _)| *n == "worktrees.txt")
        .unwrap();
    let extra = |b: &str| {
        set(
            "worktrees.txt",
            format!("{wt}\nworktree /w/e\nHEAD f\nbranch refs/heads/{b}\n"),
        )
    };
    assert_eq!(pick(&[extra("t/9-z")]), "worker leftover-worktree t/9-z");
    assert_eq!(pick(&[extra("u/9-z")]), "legacy");
    assert_eq!(pick(&[extra("x-t/9-z")]), "legacy");
    // The first such branch by name; flaky comes before a leftover.
    let two = set(
        "worktrees.txt",
        format!(
            "{wt}\nworktree /w/e\nHEAD f\nbranch refs/heads/t/9-z\n\nworktree /w/f\nHEAD g\nbranch refs/heads/t/10-a\n"
        ),
    );
    assert_eq!(
        pick(std::slice::from_ref(&two)),
        "worker leftover-worktree t/10-a"
    );
    assert_eq!(
        pick(&[two, plus("issues.json", &[issue(600, "f", &["flaky"])])]),
        "worker flaky #600"
    );
    // A closed PR for the branch: not picked, noted.
    let o = run_with(&[
        extra("t/9-z"),
        plus("prs.json", &[pr(602, "CLOSED", "", "t/9-z", &[], "[]")]),
    ])
    .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.starts_with("legacy\n"), "{out}");
    assert!(
        out.contains("- worktree on t/9-z: its PRs (#602) are merged or closed; remove it\n"),
        "{out}"
    );
}

#[test]
fn precedence_removes_each_step_in_turn() {
    let url = format!("{ISSUES}40#issuecomment-123");
    // (expected pick prefix, extra feed lines, extra issues, whole-file changes)
    type Step = (
        &'static str,
        Vec<String>,
        Vec<String>,
        Vec<(&'static str, String)>,
    );
    let steps: Vec<Step> = vec![
        (
            "worker owner-issue #250",
            vec![feed_issue(250)],
            vec![],
            vec![],
        ),
        (
            "worker owner-comment",
            vec![feed_comment("comment", &url, FLOOR)],
            vec![],
            vec![],
        ),
        (
            "worker owner-done #173",
            vec![],
            vec![],
            vec![(
                "owner-closed.json",
                format!(
                    r#"[{{"number":173,"body":"b","comments":[{}]}}]"#,
                    owner_reply(&format!("{TAG} picked up"))
                ),
            )],
        ),
        (
            "worker main-red run 20",
            vec![],
            vec![],
            vec![(
                "runs.json",
                format!("[{}]", run_entry(20, "completed", "failure", "push")),
            )],
        ),
        (
            "worker own-pr #341",
            vec![],
            vec![],
            vec![(
                "prs.json",
                format!("[{}]", pr(341, "OPEN", "", "t/1-a", &[], "[]")),
            )],
        ),
        (
            "worker scheduler-bug #400",
            vec![],
            vec![issue(400, "next-session: x", &["bug"])],
            vec![],
        ),
        (
            "worker crash-security #500",
            vec![],
            vec![issue(500, "x", &["crash"])],
            vec![],
        ),
        (
            "worker flaky #600",
            vec![],
            vec![issue(600, "f", &["flaky"])],
            vec![],
        ),
        (
            "worker leftover-worktree t/9-z",
            vec![],
            vec![],
            vec![(
                "worktrees.txt",
                "worktree /w/e\nHEAD f\nbranch refs/heads/t/9-z\n".to_owned(),
            )],
        ),
    ];
    for start in 0..steps.len() {
        let mut feed = Vec::new();
        let mut issues = Vec::new();
        let mut changes: Vec<(&str, Option<String>)> = Vec::new();
        for (_, f, i, c) in &steps[start..] {
            feed.extend(f.iter().cloned());
            issues.extend(i.iter().cloned());
            changes.extend(c.iter().map(|(n, t)| (*n, Some(t.clone()))));
        }
        changes.push(feed_plus(&feed));
        changes.push(plus("issues.json", &issues));
        let got = pick(&changes);
        assert!(got.starts_with(steps[start].0), "from step {start}: {got}");
    }
}

#[test]
fn malformed_inputs_exit_1() {
    let bad: Vec<(&str, &str)> = vec![
        ("feed.jsonl", "{\"kind\":\"issue\"}\n"),
        (
            "feed.jsonl",
            "{\"kind\":\"other\",\"url\":\"u\",\"created_at\":\"t\"}\n",
        ),
        ("feed.jsonl", "not json\n"),
        ("feed.jsonl", "{\"kind\":\"comment\",\"url\":\"u\"}\n"),
        ("owner-open.json", "{}"),
        (
            "owner-open.json",
            r#"[{"number":238,"labels":[],"comments":[{"author":{"login":1},"body":"x"}]}]"#,
        ),
        ("owner-closed.json", r#"[{"number":1,"comments":[]}]"#),
        ("prs.json", r#"[{"number":-1}]"#),
        (
            "prs.json",
            r#"[{"number":1,"state":"OPEN","isDraft":false,"body":"","headRefName":"b","labels":[],"statusCheckRollup":{}}]"#,
        ),
        (
            "prs.json",
            r#"[{"number":1,"state":"OPEN","isDraft":false,"body":"","headRefName":"b","labels":[],"statusCheckRollup":[{"__typename":"StatusContext","context":"ai-review"}]}]"#,
        ),
        ("issues.json", r#"[{"number":1.5,"title":"t","labels":[]}]"#),
        (
            "issues.json",
            r#"[{"number":1,"title":"t","labels":[{"name":2}]}]"#,
        ),
        (
            "runs.json",
            r#"[{"databaseId":1,"status":"completed","event":"push"}]"#,
        ),
        ("runs.json", "[1]"),
    ];
    for (name, text) in bad {
        let o = run_with(&[set(name, text.to_owned())]).unwrap();
        assert_eq!(o.status.code(), Some(1), "{name}: {text}");
        assert!(o.stdout.is_empty(), "{name}: {text}");
    }
    for name in [
        "feed.jsonl",
        "floor.txt",
        "owner-open.json",
        "owner-closed.json",
        "prs.json",
        "issues.json",
        "runs.json",
        "worktrees.txt",
    ] {
        let o = run_with(&[(name, None)]).unwrap();
        assert_eq!(o.status.code(), Some(1), "missing {name}");
    }
}

#[test]
fn usage_and_live_failure() {
    let xtask = || Command::new(env!("CARGO_BIN_EXE_xtask"));
    for args in [
        &["next-session", "--from"][..],
        &["next-session", "--x", "d"],
        &["next-session", "--from", "a", "b"],
    ] {
        let o = xtask().args(args).output().unwrap();
        assert_eq!(o.status.code(), Some(1), "{args:?}");
    }
    // Live mode in a directory without the feed script: the command can't run, exit 2.
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("next-session-live");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let o = xtask()
        .arg("next-session")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(o.stdout.is_empty());
}
