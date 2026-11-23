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
    full_issue(n, title, l, "", "null")
}

/// An issue with a body and a milestone (`milestone`: JSON, `null` or `{"title":"M0"}`).
fn full_issue(n: u64, title: &str, l: &[&str], body: &str, milestone: &str) -> String {
    format!(
        r#"{{"number":{n},"title":{title:?},"labels":{},"body":{body:?},"milestone":{milestone}}}"#,
        labels(l)
    )
}

/// A `ready` issue in milestone `m` with this body.
fn ready(n: u64, m: &str, l: &[&str], body: &str) -> String {
    let mut all = vec!["ready"];
    all.extend_from_slice(l);
    full_issue(
        n,
        "r",
        &all,
        body,
        &format!(r#"{{"title":{m:?},"number":1}}"#),
    )
}

// Times (Unix seconds): now is 2026-11-24T06:00:00Z; the last audit record is a day old, the
// last retro record two days old.
const NOW: i64 = 1_795_500_000;
const DAY: i64 = 86_400;
const WEEK: i64 = 7 * DAY;
const AUDIT_T: i64 = NOW - DAY;
const RETRO_T: i64 = NOW - 2 * DAY;
const ROOT_T: i64 = 1_795_000_000;

/// One commit of `main-log.txt` (`--format=commit %H %ct %s --name-status`).
fn commit(id: u64, time: i64, subject: &str, files: &[(&str, &str)]) -> String {
    // The id leads the hash, so the 7-character short form is `{id:07x}`.
    let mut out = format!("commit {:0<40} {time} {subject}\n\n", format!("{id:07x}"));
    for (st, path) in files {
        out.push_str(&format!("{st}\t{path}\n"));
    }
    out
}

/// The base log's commits, oldest last: a product merge after the audit record, the audit
/// record (which also changes product code), the retro record, a revert before it, the root.
fn base_log() -> Vec<String> {
    vec![
        commit(
            105,
            NOW - 3600,
            "feature (#10)",
            &[("M", "crates/a/src/lib.rs"), ("M", "ops/journal/x.md")],
        ),
        commit(
            104,
            AUDIT_T,
            "Audit 0006 (#9)",
            &[("A", "docs/audits/0006.md"), ("M", "crates/b.rs")],
        ),
        commit(
            103,
            RETRO_T,
            "Retro 0001 (#8)",
            &[("A", "docs/retros/0001.md")],
        ),
        commit(
            102,
            RETRO_T - 100,
            "Revert \"x\" (#7)",
            &[("M", "crates/z.rs")],
        ),
        commit(101, ROOT_T, "Add plan", &[("A", "PLAN.md")]),
    ]
}

/// `main-log.txt` with `newer` commits on top of the base log.
fn log_with(newer: &[String]) -> (&'static str, Option<String>) {
    let mut all = newer.to_vec();
    all.extend(base_log());
    ("main-log.txt", Some(all.concat()))
}

fn labeled(label: &str, at: &str) -> String {
    format!(r#"{{"event":"labeled","created_at":{at:?},"label":{{"name":{label:?}}}}}"#)
}

fn blocked_issue(n: u64, events: &[String]) -> String {
    format!(r#"{{"number":{n},"events":[{}]}}"#, events.join(","))
}

const GREEN: &str = r#"[{"__typename":"CheckRun","name":"policy","conclusion":"SUCCESS"},
 {"__typename":"CheckRun","name":"project-linux","conclusion":""},
 {"__typename":"StatusContext","context":"ai-review","state":"SUCCESS"}]"#;

/// An agent PR: its body starts with the tag.
fn pr(n: u64, state: &str, body: &str, branch: &str, l: &[&str], rollup: &str) -> String {
    raw_pr(n, state, &format!("{TAG} {body}"), branch, l, rollup)
}

/// A PR with exactly this body (untagged: the owner's). `mergeable` is what `gh` shows live:
/// `MERGEABLE` for an open PR, `UNKNOWN` for a merged or closed one.
fn raw_pr(n: u64, state: &str, body: &str, branch: &str, l: &[&str], rollup: &str) -> String {
    let m = if state == "OPEN" {
        "MERGEABLE"
    } else {
        "UNKNOWN"
    };
    format!(
        r#"{{"number":{n},"state":{state:?},"isDraft":false,"body":{body:?},"headRefName":{branch:?},"labels":{},"statusCheckRollup":{rollup},"mergeable":"{m}"}}"#,
        labels(l)
    )
}

/// The same PR object with its `mergeable` field (always last) replaced by the JSON `value`,
/// or removed when `value` is empty.
fn set_mergeable(pr_json: &str, value: &str) -> String {
    let key = r#","mergeable":"#;
    assert!(
        pr_json.contains(key),
        "pr object without mergeable: {pr_json}"
    );
    let head = pr_json.rsplit_once(key).map_or(pr_json, |(h, _)| h);
    if value.is_empty() {
        format!("{head}}}")
    } else {
        format!(r#"{head},"mergeable":{value}}}"#)
    }
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
                ready(301, "M0", &[], ""),
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
        ("now.txt", format!("{NOW}\n")),
        ("main-log.txt", base_log().concat()),
        ("tags.txt", format!("m0 {}\nv1 {}\n", RETRO_T - 1, NOW - 10)),
        (
            "milestones.json",
            r#"[{"title":"M10","number":11},{"title":"M1","number":2},{"title":"M0","number":1},{"title":"Mx","number":12}]"#
                .to_owned(),
        ),
        (
            "blocked.json",
            format!(
                "[{},{},{}]",
                blocked_issue(
                    187,
                    &[
                        labeled("ready", "2026-11-20T00:00:00Z"),
                        labeled("blocked", "2026-11-23T10:00:00Z")
                    ]
                ),
                blocked_issue(188, &[labeled("blocked", "2026-11-22T06:00:01Z")]),
                blocked_issue(
                    189,
                    &[
                        labeled("blocked", "2026-11-22T06:00:00Z"),
                        r#"{"event":"commented","created_at":"2026-11-23T00:00:00Z"}"#.to_owned()
                    ]
                )
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
        "legacy\n\nRule: none of steps 1–5, 7 or 10 applies; use TICK §2 rule j\nLoad:\n- ops/TICK.md §2 rule j\n\
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
    // A green PR in conflict with its base can't land: not in flight (#270).
    let conflicted = |m: &str| {
        pick(&[
            feed_plus(&[feed_issue(250)]),
            plus(
                "prs.json",
                &[set_mergeable(
                    &pr(310, "OPEN", "x\nCloses #250", "t/250-z", &[], GREEN),
                    &format!("{m:?}"),
                )],
            ),
        ])
    };
    assert_eq!(conflicted("CONFLICTING"), "worker owner-issue #250");
    assert_eq!(conflicted("UNKNOWN"), "legacy");
    assert_eq!(conflicted("MERGEABLE"), "legacy");
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
    // A green PR in conflict with its base doesn't hold the comment either (#270).
    let conflicted = set_mergeable(
        &pr(
            320,
            "OPEN",
            &format!("Owner instruction: {url}"),
            "t/9-q",
            &[],
            GREEN,
        ),
        r#""CONFLICTING""#,
    );
    assert_eq!(
        pick(&[feed_plus(&[c(FLOOR)]), plus("prs.json", &[conflicted])]),
        format!("worker owner-comment {url}")
    );
    // Owner issues come before owner comments.
    assert_eq!(
        pick(&[feed_plus(&[c(FLOOR), feed_issue(260)])]),
        "worker owner-issue #260"
    );
}

#[test]
fn review_round_1_near_misses() {
    // R1.1: a quoted `Closes #<issue>` before the real link.
    let quoted = plus(
        "prs.json",
        &[pr(
            310,
            "OPEN",
            "says `Closes #<issue>` here\nCloses #250",
            "t/250-z",
            &[],
            GREEN,
        )],
    );
    assert_eq!(
        pick(&[feed_plus(&[feed_issue(250)]), quoted.clone()]),
        "legacy"
    );
    assert_eq!(
        pick(&[plus("issues.json", &[issue(250, "c", &["crash"])]), quoted]),
        "legacy"
    );
    // R1.2: an owner PR in the feed is not an owner issue; is_pr must be a boolean.
    let owner_pr =
        r#"{"kind":"issue","number":270,"title":"t","labels":[],"is_pr":true,"body":"b"}"#;
    assert_eq!(pick(&[feed_plus(&[owner_pr.to_owned()])]), "legacy");
    let no_flag = r#"{"kind":"issue","number":270,"title":"t","labels":[],"body":"b"}"#;
    let o = run_with(&[feed_plus(&[no_flag.to_owned()])]).unwrap();
    assert_eq!(o.status.code(), Some(1));
    // R1.3: an untagged (owner) PR is never the agent's own, failing or in flight.
    let owner = |body: &str| plus("prs.json", &[raw_pr(350, "OPEN", body, "o/x", &[], "[]")]);
    assert_eq!(pick(&[owner("Closes #301")]), "legacy");
    let flight = plus(
        "prs.json",
        &[raw_pr(351, "OPEN", "Closes #500", "o/y", &[], GREEN)],
    );
    assert_eq!(
        pick(&[plus("issues.json", &[issue(500, "x", &["crash"])]), flight]),
        "worker crash-security #500"
    );
    assert_eq!(
        pick(&[owner(&format!(" {TAG} x"))]),
        "worker own-pr #350",
        "leading space"
    );
    // R1.4: merged and closed PRs don't put an issue in flight.
    for state in ["MERGED", "CLOSED"] {
        let done = plus(
            "prs.json",
            &[pr(352, state, "Closes #500", "t/500-a", &[], GREEN)],
        );
        let wt = set("worktrees.txt", String::new());
        for (l, want) in [
            ("crash", "worker crash-security #500"),
            ("flaky", "worker flaky #500"),
        ] {
            let i = plus("issues.json", &[issue(500, "x", &[l])]);
            assert_eq!(pick(&[i, done.clone(), wt.clone()]), want, "{state}");
        }
        let i = plus("issues.json", &[issue(500, "next-session: x", &["bug"])]);
        assert_eq!(pick(&[i, done, wt]), "worker scheduler-bug #500", "{state}");
    }
    // R1.5: owner lists are parsed even when an earlier owner item is picked.
    for (name, text) in [
        ("owner-closed.json", r#"[{"number":1,"comments":[]}]"#),
        ("owner-open.json", r#"[{"number":1,"comments":[]}]"#),
    ] {
        let o = run_with(&[feed_plus(&[feed_issue(250)]), set(name, text.to_owned())]).unwrap();
        assert_eq!(o.status.code(), Some(1), "{name}");
    }
    // R1.6: the floor must be YYYY-MM-DDTHH:MM:SSZ.
    for f in [
        "",
        "2026-11-23",
        "2026-11-23T00:00:00",
        "2026-11-23 00:00:00Z",
        "2026-11-23T00:00:0xZ",
    ] {
        let o = run_with(&[set("floor.txt", f.to_owned())]).unwrap();
        assert_eq!(o.status.code(), Some(1), "{f:?}");
    }
    // R1.7: a picked up reply for a comment on the closed issue doesn't count.
    let comment_only = owner_reply(&format!("{TAG} picked up {ISSUES}173#issuecomment-5"));
    let closed = set(
        "owner-closed.json",
        format!(r#"[{{"number":173,"body":"b","comments":[{comment_only}]}}]"#),
    );
    assert_eq!(pick(&[closed]), "legacy");
    let longer = owner_reply(&format!("{TAG} picked up {ISSUES}1730"));
    let closed = set(
        "owner-closed.json",
        format!(r#"[{{"number":173,"body":"b","comments":[{longer}]}}]"#),
    );
    assert_eq!(pick(&[closed]), "legacy");
    // R2.1: the first numeric `Closes #N` is the link, not a later one.
    let two = plus(
        "prs.json",
        &[pr(
            311,
            "OPEN",
            "Closes #250\nCloses #251",
            "t/250-z",
            &[],
            GREEN,
        )],
    );
    assert_eq!(
        pick(&[feed_plus(&[feed_issue(251), feed_issue(250)]), two]),
        "worker owner-issue #251"
    );
    // R2.2: a non-boolean is_pr is exit 1.
    for v in ["\"false\"", "null", "0"] {
        let line = format!(r#"{{"kind":"issue","number":270,"is_pr":{v}}}"#);
        let o = run_with(&[feed_plus(&[line])]).unwrap();
        assert_eq!(o.status.code(), Some(1), "is_pr {v}");
    }
    // R2.3: malformed owner replies in either owner list are exit 1.
    for name in ["owner-open.json", "owner-closed.json"] {
        let bad = r#"[{"number":1,"body":"b","labels":[],"comments":"x"}]"#;
        let o = run_with(&[set(name, bad.to_owned())]).unwrap();
        assert_eq!(o.status.code(), Some(1), "{name}");
    }
    // R1.8: a crash issue labeled sev:critical comes first.
    let mix = plus(
        "issues.json",
        &[
            issue(530, "u", &["security"]),
            issue(560, "c", &["crash", "sev:critical"]),
        ],
    );
    assert_eq!(pick(&[mix]), "worker crash-security #560");
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
fn step3_conflicting_pr() {
    // #270: an agent PR whose checks and ai-review are green but whose mergeable is
    // CONFLICTING (auto-merge can't land it) is a rule c pick. Every list keeps base()'s
    // merged PR #299, so its worktree is no leftover.
    let merged = pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]");
    let with = |p: String| pick(&[set("prs.json", format!("[{p},{merged}]"))]);
    let green = |state: &str, l: &[&str]| pr(340, state, "Closes #301", "t/301-y", l, GREEN);
    let m = |p: String, v: &str| set_mergeable(&p, &format!("{v:?}"));
    assert_eq!(
        with(m(green("OPEN", &[]), "CONFLICTING")),
        "worker own-pr #340"
    );
    // Near misses: lazily computed or mergeable; not open; skip labels; the owner's PR.
    for v in ["UNKNOWN", "MERGEABLE"] {
        assert_eq!(with(m(green("OPEN", &[]), v)), "legacy", "{v}");
    }
    // A merged or closed PR doesn't keep #301 (ready, M0) in flight: step 10 picks it.
    for state in ["MERGED", "CLOSED"] {
        assert_eq!(
            with(m(green(state, &[]), "CONFLICTING")),
            "worker ready #301",
            "{state}"
        );
    }
    for skip in ["owner-todo", "blocked", "deferred"] {
        assert_eq!(
            with(m(green("OPEN", &[skip]), "CONFLICTING")),
            "legacy",
            "{skip} on the PR"
        );
        let parked = set("issues.json", format!("[{}]", issue(301, "f", &[skip])));
        let prs = set(
            "prs.json",
            format!("[{},{merged}]", m(green("OPEN", &[]), "CONFLICTING")),
        );
        assert_eq!(pick(&[prs, parked]), "legacy", "{skip} on the issue");
    }
    let owners = raw_pr(340, "OPEN", "Closes #301", "t/301-y", &[], GREEN);
    // The owner's PR is no agent PR, so its branch's worktree (base()) is a leftover instead.
    assert_eq!(
        with(m(owners, "CONFLICTING")),
        "worker leftover-worktree t/301-y",
        "untagged"
    );
    // Rule c's order is by number, whatever the reason: a conflicting PR before a failing one.
    let failing = pr(341, "OPEN", "", "t/1-a", &[], "[]");
    assert_eq!(
        with(format!(
            "{failing},{}",
            m(green("OPEN", &[]), "CONFLICTING")
        )),
        "worker own-pr #340"
    );
    // And a failing PR before a conflicting one with a higher number.
    let failing = pr(339, "OPEN", "", "t/1-a", &[], "[]");
    assert_eq!(
        with(format!(
            "{},{failing}",
            m(green("OPEN", &[]), "CONFLICTING")
        )),
        "worker own-pr #339"
    );
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
                    owner_reply(&format!("{TAG} picked up {ISSUES}173"))
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
        (
            "audit due",
            vec![],
            vec![],
            vec![(
                "main-log.txt",
                [
                    commit(103, RETRO_T, "Retro", &[("A", "docs/retros/0001.md")]),
                    commit(101, ROOT_T, "Add plan", &[("A", "PLAN.md")]),
                ]
                .concat(),
            )],
        ),
        (
            "retro due",
            vec![],
            vec![],
            vec![("tags.txt", format!("m1 {}\n", RETRO_T + 1))],
        ),
        (
            "worker ready #700",
            vec![],
            vec![ready(700, "M0", &[], "")],
            vec![],
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
            r#"[{"number":1,"state":"OPEN","isDraft":false,"body":"","headRefName":"b","labels":[],"statusCheckRollup":{},"mergeable":"MERGEABLE"}]"#,
        ),
        (
            "prs.json",
            r#"[{"number":1,"state":"OPEN","isDraft":false,"body":"","headRefName":"b","labels":[],"statusCheckRollup":[{"__typename":"StatusContext","context":"ai-review"}],"mergeable":"MERGEABLE"}]"#,
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
    // `mergeable` (#270): missing, not a string or an unknown value is exit 1, also on a
    // merged PR (every PR is parsed in full). The unchanged list picks `legacy`.
    let open = pr(300, "OPEN", "Closes #301", "t/301-y", &[], GREEN);
    let merged = pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]");
    assert_eq!(
        pick(&[set("prs.json", format!("[{open},{merged}]"))]),
        "legacy"
    );
    // A conflicting open PR's statusCheckRollup is still parsed (R1.1).
    let bad_rollup = set_mergeable(
        &pr(300, "OPEN", "Closes #301", "t/301-y", &[], "{}"),
        r#""CONFLICTING""#,
    );
    let out = run_with(&[set("prs.json", format!("[{bad_rollup},{merged}]"))]).unwrap();
    assert_eq!(out.status.code(), Some(1), "{bad_rollup}");
    assert!(out.stdout.is_empty(), "{bad_rollup}");
    for v in ["", "1", "null", r#""DIRTY""#, r#""conflicting""#] {
        for (o, m) in [
            (set_mergeable(&open, v), merged.clone()),
            (open.clone(), set_mergeable(&merged, v)),
        ] {
            let text = format!("[{o},{m}]");
            let out = run_with(&[set("prs.json", text.clone())]).unwrap();
            assert_eq!(out.status.code(), Some(1), "{text}");
            assert!(out.stdout.is_empty(), "{text}");
        }
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
        "now.txt",
        "main-log.txt",
        "tags.txt",
        "milestones.json",
        "blocked.json",
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

/// `n` commits after the audit record, each changing `path`.
fn merges(n: u64, path: &str) -> Vec<String> {
    (0..n)
        .map(|i| commit(200 + i, NOW - 60, "m", &[("M", path)]))
        .collect()
}

#[test]
fn step7_audit_product_merges() {
    // The base has 1 product merge after the record (the record's own commit isn't counted).
    assert_eq!(pick(&[log_with(&merges(8, "crates/c.rs"))]), "legacy");
    assert_eq!(pick(&[log_with(&merges(9, "crates/c.rs"))]), "audit due");
    // Each product path counts; files outside them don't.
    for path in [
        "crates/x/src/lib.rs",
        "bindings/web/src/lib.rs",
        "apps/web/src/App.tsx",
        "fuzz/fuzz_targets/a.rs",
        "tools/jpdf-cli/src/main.rs",
        "tools/fsio/src/lib.rs",
        "\"crates/\\303\\251.rs\"",
    ] {
        assert_eq!(pick(&[log_with(&merges(9, path))]), "audit due", "{path}");
    }
    for path in [
        "xtask/src/main.rs",
        "tools/jpdf-cli-extra/a.rs",
        "tools/fsiox/a.rs",
        "ops/TICK.md",
        "specs/crates/a.md",
        "cratesx/a.rs",
    ] {
        assert_eq!(pick(&[log_with(&merges(9, path))]), "legacy", "{path}");
    }
    // A deletion counts as a change.
    let del = (0..9)
        .map(|i| commit(300 + i, NOW - 60, "d", &[("D", "crates/old.rs")]))
        .collect::<Vec<_>>();
    assert_eq!(pick(&[log_with(&del)]), "audit due");
    let o = run_with(&[log_with(&merges(9, "crates/c.rs"))]).unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.contains(&format!(
            "Rule: g (audit due: 10 product merges since {:07x})\n",
            104
        )),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            "- git log --first-parent --oneline {:07x}..origin/main\n",
            104
        )),
        "{out}"
    );
}

#[test]
fn step7_audit_records_and_age() {
    let at = |t: i64| set("now.txt", format!("{}\n", t));
    // Seven days after the audit record, with a retro record 2 days newer (so no retro).
    let log = set(
        "main-log.txt",
        [
            commit(
                103,
                AUDIT_T + 2 * DAY,
                "Retro",
                &[("A", "docs/retros/0001.md")],
            ),
            commit(104, AUDIT_T, "Audit", &[("A", "docs/audits/0006.md")]),
            commit(101, ROOT_T, "Add plan", &[("A", "PLAN.md")]),
        ]
        .concat(),
    );
    assert_eq!(pick(&[at(AUDIT_T + WEEK - 1), log.clone()]), "legacy");
    assert_eq!(pick(&[at(AUDIT_T + WEEK), log]), "audit due");
    // No audit record at all.
    let no_audit = |files: &[(&str, &str)]| {
        let log = [
            commit(110, NOW - 60, "x", files),
            commit(103, RETRO_T, "Retro", &[("A", "docs/retros/0001.md")]),
            commit(101, ROOT_T, "Add plan", &[("A", "PLAN.md")]),
        ]
        .concat();
        pick(&[set("main-log.txt", log)])
    };
    assert_eq!(no_audit(&[]), "audit due");
    // Only an added NNNN.md record counts: a changed one, other names, other folders don't.
    for f in [
        ("M", "docs/audits/0006.md"),
        ("A", "docs/audits/0007-x.md"),
        ("A", "docs/audits/007.md"),
        ("A", "docs/audits/00071.md"),
        ("A", "docs/audits/sub/0007.md"),
        ("A", "docs/retros/0007.md"),
    ] {
        assert_eq!(no_audit(&[f]), "audit due", "{f:?}");
    }
    // A user-advocate record is an audit slot too.
    assert_eq!(no_audit(&[("A", "docs/advocate/0007.md")]), "legacy");
    assert_eq!(no_audit(&[("A", "docs/audits/0007.md")]), "legacy");
}

#[test]
fn step7_retro() {
    // No incident and 2 days: nothing. Seven days after the retro record (the audit record
    // stays newer, so the audit isn't due).
    let at = |t: i64| set("now.txt", format!("{}\n", t));
    let log_at = |audit: i64| {
        let log = [
            commit(104, audit, "Audit", &[("A", "docs/audits/0006.md")]),
            commit(103, RETRO_T, "Retro", &[("A", "docs/retros/0001.md")]),
            commit(101, ROOT_T, "Add plan", &[("A", "PLAN.md")]),
        ]
        .concat();
        set("main-log.txt", log)
    };
    assert_eq!(
        pick(&[at(RETRO_T + WEEK - 1), log_at(RETRO_T + WEEK - 10)]),
        "legacy"
    );
    assert_eq!(
        pick(&[at(RETRO_T + WEEK), log_at(RETRO_T + WEEK - 10)]),
        "retro due"
    );
    // No retro record: seven days after the first commit.
    let no_retro = |now: i64| {
        let log = [
            commit(104, now - 10, "Audit", &[("A", "docs/audits/0006.md")]),
            commit(101, ROOT_T, "Add plan", &[("A", "PLAN.md")]),
        ]
        .concat();
        // The base's tag and blocked events would be incidents since the first commit.
        pick(&[
            at(now),
            set("main-log.txt", log),
            set("tags.txt", String::new()),
            set("blocked.json", "[]".to_owned()),
        ])
    };
    assert_eq!(no_retro(ROOT_T + WEEK - 1), "legacy");
    assert_eq!(no_retro(ROOT_T + WEEK), "retro due");
    // An audit due comes first.
    assert_eq!(pick(&[at(AUDIT_T + WEEK)]), "audit due");
}

#[test]
fn step7_retro_incidents() {
    // A revert after the retro record (the base has one before it).
    let rev = |subject: &str| pick(&[log_with(&[commit(120, NOW - 60, subject, &[])])]);
    assert_eq!(rev("Revert \"feature\" (#10) (#11)"), "retro due");
    assert_eq!(rev("Reverts feature"), "legacy");
    assert_eq!(rev("Revert feature"), "legacy");
    assert_eq!(rev("x Revert \"feature\""), "legacy");
    // A judge alarm raised after the retro record.
    let alarm = |t: i64| pick(&[set("judge-alarm-since", format!("{t}\n"))]);
    assert_eq!(alarm(RETRO_T + 1), "retro due");
    assert_eq!(alarm(RETRO_T), "legacy");
    // A milestone tag created after the retro record.
    let tag = |line: &str| pick(&[set("tags.txt", format!("{line}\n"))]);
    assert_eq!(tag(&format!("m1 {}", RETRO_T + 1)), "retro due");
    assert_eq!(tag(&format!("m12 {}", NOW)), "retro due");
    for near in [
        format!("m1 {RETRO_T}"),
        format!("v1 {NOW}"),
        format!("m1a {NOW}"),
        format!("m {NOW}"),
        format!("M1 {NOW}"),
    ] {
        assert_eq!(tag(&near), "legacy", "{near}");
    }
    let o = run_with(&[set("tags.txt", format!("m1 {}\n", RETRO_T + 1))]).unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.contains(&format!(
            "Rule: h (retro due: incident: milestone tag m1 since {:07x})\n",
            103
        )),
        "{out}"
    );
}

#[test]
fn step7_retro_blocked_incident() {
    let third = |events: &[String]| pick(&[plus("blocked.json", &[blocked_issue(190, events)])]);
    // The base has 2 (#187, #188; #189's event is at the retro record's second, not after).
    assert_eq!(
        third(&[labeled("blocked", "2026-11-24T05:00:00Z")]),
        "retro due"
    );
    assert_eq!(
        third(&[labeled("blocked", "2026-11-22T06:00:00Z")]),
        "legacy"
    );
    assert_eq!(
        third(&[labeled("blocked-x", "2026-11-24T05:00:00Z")]),
        "legacy"
    );
    assert_eq!(
        third(&[
            r#"{"event":"unlabeled","created_at":"2026-11-24T05:00:00Z","label":{"name":"blocked"}}"#
                .to_owned()
        ]),
        "legacy"
    );
    // The same issue twice counts once.
    assert_eq!(
        pick(&[plus(
            "blocked.json",
            &[blocked_issue(
                187,
                &[labeled("blocked", "2026-11-24T05:00:00Z")]
            )]
        )]),
        "legacy"
    );
    let o = run_with(&[plus(
        "blocked.json",
        &[blocked_issue(
            190,
            &[labeled("blocked", "2026-11-24T05:00:00Z")],
        )],
    )])
    .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.contains(&format!(
            "Rule: h (retro due: incident: 3 issues labeled blocked since {:07x} (#187, #188, #190))\n",
            103
        )),
        "{out}"
    );
}

#[test]
fn step10_ready_issue() {
    let with = |extra: &[String]| pick(&[plus("issues.json", extra)]);
    assert_eq!(with(&[ready(701, "M0", &[], "")]), "worker ready #701");
    assert_eq!(
        with(&[ready(702, "M0", &[], ""), ready(701, "M0", &[], "")]),
        "worker ready #701"
    );
    // Lowest open M<n> by number: M1 is not current while M0 is open.
    assert_eq!(with(&[ready(701, "M1", &[], "")]), "legacy");
    let ms = |list: &str| set("milestones.json", list.to_owned());
    assert_eq!(
        pick(&[
            plus(
                "issues.json",
                &[ready(701, "M2", &[], ""), ready(702, "M10", &[], "")]
            ),
            ms(r#"[{"title":"M10"},{"title":"M2"},{"title":"Mx"},{"title":"M-1"}]"#)
        ]),
        "worker ready #701"
    );
    assert_eq!(
        pick(&[plus("issues.json", &[ready(701, "M0", &[], "")]), ms("[]")]),
        "legacy"
    );
    // Near misses: no milestone, not ready, each skip label, in flight.
    assert_eq!(
        with(&[full_issue(701, "r", &["ready"], "", "null")]),
        "legacy"
    );
    assert_eq!(
        with(&[full_issue(701, "r", &[], "", r#"{"title":"M0"}"#)]),
        "legacy"
    );
    for l in [
        "owner-todo",
        "blocked",
        "deferred",
        "in-progress",
        "design",
        "design-pending",
    ] {
        assert_eq!(with(&[ready(701, "M0", &[l], "")]), "legacy", "{l}");
    }
    // #301 (ready, M0) is linked by the open PR #300 in the base; without the PR it's picked.
    let merged = pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]");
    let closed = pr(300, "CLOSED", "Closes #301", "t/301-y", &[], "[]");
    assert_eq!(
        pick(&[set("prs.json", format!("[{closed},{merged}]"))]),
        "worker ready #301"
    );
}

#[test]
fn step10_dependencies() {
    let one = |body: &str| pick(&[plus("issues.json", &[ready(701, "M0", &[], body)])]);
    // #150 is open in the base, #149 is not.
    assert_eq!(one("x\nDepends on: #150 (task 1a)."), "legacy");
    assert_eq!(one("**Depends on:** #149, #150."), "legacy");
    assert_eq!(one("Depends on: #149."), "worker ready #701");
    assert_eq!(one("Depends on: none."), "worker ready #701");
    // Only the text after `Depends on:` on its own line counts.
    assert_eq!(one("See #150.\nDepends on: #149"), "worker ready #701");
    assert_eq!(one("Depends on:\n#150"), "worker ready #701");
    assert_eq!(one("depends on: #150"), "worker ready #701");
    // R1.2: a number in prose after the phrase isn't a dependency.
    assert_eq!(
        one("with its Depends on: issues closed (#150)."),
        "worker ready #701"
    );
    assert_eq!(one("Depends on: #149 (task 1a), #150 (task 1b)."), "legacy");
    // A lower-numbered blocked one doesn't hide a later free one.
    assert_eq!(
        pick(&[plus(
            "issues.json",
            &[
                ready(701, "M0", &[], "Depends on: #150"),
                ready(702, "M0", &[], "")
            ]
        )]),
        "worker ready #702"
    );
}

#[test]
fn new_inputs_malformed_exit_1() {
    let bad: Vec<(&str, String)> =
        vec![
        ("now.txt", "x\n".to_owned()),
        ("now.txt", "+1795500000\n".to_owned()),
        ("now.txt", "".to_owned()),
        ("main-log.txt", "".to_owned()),
        ("main-log.txt", "M\tcrates/a.rs\n".to_owned()),
        ("main-log.txt", format!("commit abc {NOW} x\n")),
        ("main-log.txt", format!("commit {:040x} x y\n", 1)),
        ("main-log.txt", format!("{}R100\ta\tb\n", base_log().concat())),
        ("main-log.txt", format!("{}A crates/a.rs\n", base_log().concat())),
        ("tags.txt", "m1\n".to_owned()),
        ("tags.txt", "m1 x\n".to_owned()),
        ("milestones.json", "{}".to_owned()),
        ("milestones.json", r#"[{"title":1}]"#.to_owned()),
        ("blocked.json", "{}".to_owned()),
        ("blocked.json", r#"[{"number":1}]"#.to_owned()),
        (
            "blocked.json",
            format!("[{}]", blocked_issue(1, &[labeled("blocked", "2026-11-24")])),
        ),
        (
            "blocked.json",
            r#"[{"number":1,"events":[{"event":"labeled","created_at":"2026-11-24T00:00:00Z"}]}]"#
                .to_owned(),
        ),
        ("judge-alarm-since", "\n".to_owned()),
        ("judge-alarm-since", "soon\n".to_owned()),
        (
            "issues.json",
            r#"[{"number":1,"title":"t","labels":[],"milestone":null}]"#.to_owned(),
        ),
        (
            "issues.json",
            r#"[{"number":1,"title":"t","labels":[],"body":""}]"#.to_owned(),
        ),
        (
            "issues.json",
            r#"[{"number":1,"title":"t","labels":[],"body":"","milestone":{"title":2}}]"#
                .to_owned(),
        ),
    ];
    for (name, text) in bad {
        let o = run_with(&[set(name, text.clone())]).unwrap();
        assert_eq!(o.status.code(), Some(1), "{name}: {text}");
        assert!(o.stdout.is_empty(), "{name}: {text}");
    }
    // R1.1: every step-7 input is parsed when the pick is `audit due` or an earlier step too.
    let audit_now = set("now.txt", format!("{}\n", AUDIT_T + WEEK));
    assert_eq!(pick(std::slice::from_ref(&audit_now)), "audit due");
    let owner = feed_plus(&[feed_issue(250)]);
    assert_eq!(
        pick(std::slice::from_ref(&owner)),
        "worker owner-issue #250"
    );
    for (name, text) in [
        ("blocked.json", r#"{"not":"a list"}"#),
        ("blocked.json", r#"[{"number":1}]"#),
        ("tags.txt", "m1\n"),
        ("main-log.txt", "x\n"),
    ] {
        for first in [&audit_now, &owner] {
            let o = run_with(&[first.clone(), set(name, text.to_owned())]).unwrap();
            assert_eq!(o.status.code(), Some(1), "{name}: {text}");
            assert!(o.stdout.is_empty(), "{name}: {text}");
        }
    }
    // A commit with an empty subject and no files parses.
    let o = run_with(&[log_with(&[format!("commit {:040x} {}\n", 9, NOW - 5)])]).unwrap();
    assert_eq!(o.status.code(), Some(0));
}
