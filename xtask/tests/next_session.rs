//! `cargo xtask next-session --from` on built fixtures (specs/tooling/next-session.md). The base
//! fixture selects nothing but step 11 (`planner`); each test changes one input so one step applies, and
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
        commit(
            101,
            ROOT_T,
            "Add plan",
            &[("A", "PLAN.md"), ("A", "docs/PRODUCT.md")],
        ),
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
        ("records.txt", base_records()),
        ("queue.md", BASE_QUEUE.to_owned()),
        // The work mix (#288): no PR changes files, no issue has a milestone, and the base's 5
        // records are too few to measure.
        ("pr-files.json", "[]".to_owned()),
        ("all-issues.json", "[]".to_owned()),
        ("areas.toml", AREAS.to_owned()),
        // PR 300 was held and released: not held.
        (
            "holds.json",
            format!(
                r#"[{{"number":300,"comments":[{},{}]}}]"#,
                cmd("jacobdam", "/hold", "2026-11-23T01:00:00Z"),
                cmd("jacobdam", "/release", "2026-11-23T02:00:00Z")
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

/// `ops/areas.toml`: one product area and the two process areas issue-lint requires.
const AREAS: &str = "[[area]]\nname = \"engine\"\nkind = \"product\"\npaths = [\"crates/\"]\n\n\
                     [[area]]\nname = \"process\"\nkind = \"process\"\npaths = [\"ops/\"]\n\n\
                     [[area]]\nname = \"records\"\nkind = \"process\"\npaths = [\"ops/journal/\"]\n";

/// `ops/QUEUE.md` on origin/main: it plans the current milestone, M0 (step 8 near miss).
const BASE_QUEUE: &str = "# QUEUE\n\nMilestone: M0\nPlanned: 2026-11-23\n";

/// A PR comment of `holds.json`.
fn cmd(login: &str, body: &str, at: &str) -> String {
    format!(r#"{{"author":{{"login":{login:?}}},"body":{body:?},"createdAt":{at:?}}}"#)
}

/// `holds.json` with one entry per (PR, comments).
fn holds(entries: &[(u64, &[String])]) -> (&'static str, Option<String>) {
    let e: Vec<String> = entries
        .iter()
        .map(|(n, c)| format!(r#"{{"number":{n},"comments":[{}]}}"#, c.join(",")))
        .collect();
    ("holds.json", Some(format!("[{}]", e.join(","))))
}

#[test]
fn step3_skips_a_pr_on_the_owners_hold() {
    // PR 300 fails (no ai-review); its issue 301 is the base's ready issue.
    let failing = set(
        "prs.json",
        format!(
            "[{},{}]",
            pr(300, "OPEN", "Closes #301", "t/301-y", &[], "[]"),
            pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]")
        ),
    );
    let on = |c: &[String]| pick(&[failing.clone(), holds(&[(300, c)])]);
    let h = |at: &str| cmd("jacobdam", "/hold", at);
    let r = |at: &str| cmd("jacobdam", "/release", at);
    const T1: &str = "2026-11-24T01:00:00Z";
    const T2: &str = "2026-11-24T02:00:00Z";
    // Unheld: picked.
    assert_eq!(on(&[]), "worker own-pr #300");
    // Held: not picked; the linked issue stays in flight, so step 10 doesn't take #301.
    assert_eq!(on(&[h(T1)]), "planner");
    assert_eq!(on(&[r(T1), h(T2)]), "planner", "hold after release");
    assert_eq!(on(&[h(T2), r(T1)]), "planner", "list order doesn't matter");
    assert_eq!(on(&[r(T1), h(T1)]), "planner", "a tie goes by list order");
    assert_eq!(
        on(&[cmd("jacobdam", "/holdx", T1)]),
        "planner",
        "the workflow's prefix match"
    );
    // Near misses: picked.
    let picked = "worker own-pr #300";
    assert_eq!(on(&[h(T1), r(T2)]), picked, "released");
    assert_eq!(on(&[r(T2), h(T1)]), picked, "released, list order");
    assert_eq!(on(&[h(T1), r(T1)]), picked, "a tie goes by list order");
    assert_eq!(
        on(&[cmd("jacobdam", &format!("/hold {TAG}"), T1)]),
        picked,
        "tagged"
    );
    assert_eq!(on(&[cmd("someone", "/hold", T1)]), picked, "not the owner");
    assert_eq!(
        on(&[cmd("jacobdam", " /hold", T1)]),
        picked,
        "not at the start"
    );
    assert_eq!(
        on(&[cmd("jacobdam", "please /hold", T1)]),
        picked,
        "later text"
    );
    assert_eq!(on(&[cmd("jacobdam", "/Hold", T1)]), picked, "case");
    // A tagged or stranger's /release doesn't lift the owner's hold.
    assert_eq!(on(&[h(T1), cmd("someone", "/release", T2)]), "planner");
    assert_eq!(
        on(&[h(T1), cmd("jacobdam", &format!("{TAG} /release"), T2)]),
        "planner"
    );
    assert_eq!(
        on(&[
            h(T1),
            cmd("jacobdam", "/release x", T2),
            cmd("jacobdam", "ok", "2026-11-25T00:00:00Z")
        ]),
        picked,
        "an ordinary comment after the release changes nothing"
    );
    // R1.1: the workflow matches the command ignoring case, then holds only on `/hold` exactly.
    for later in ["/Release", "/RELEASE", "/Hold", "/HOLD now"] {
        assert_eq!(
            on(&[h(T1), cmd("jacobdam", later, T2)]),
            picked,
            "{later} releases"
        );
    }
    assert_eq!(
        on(&[h(T1), cmd("jacobdam", "/release 🤖 JPDF-Agent", T2)]),
        "planner",
        "the tag in other case: no command"
    );
    assert_eq!(
        on(&[cmd("jacobdam", "/hold 🤖 JPDF-AGENT", T1)]),
        picked,
        "the tag in other case: no command"
    );
    // R1.2: step 1 treats a held failing PR like a green one: its owner issue and an owner
    // comment its body names wait for the /release.
    let url = format!("{ISSUES}777#issuecomment-1");
    let linking = set(
        "prs.json",
        format!(
            "[{},{}]",
            pr(
                300,
                "OPEN",
                &format!("Closes #260\nOwner instruction: {url}"),
                "t/301-y",
                &[],
                "[]"
            ),
            pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]")
        ),
    );
    let feed = feed_plus(&[feed_issue(260)]);
    assert_eq!(
        pick(&[linking.clone(), feed.clone(), holds(&[(300, &[])])]),
        "worker owner-issue #260"
    );
    assert_eq!(
        pick(&[linking.clone(), feed, holds(&[(300, &[h(T1)])])]),
        "worker ready #301",
        "the base's ready issue #301 is no longer linked"
    );
    let comment = feed_plus(&[feed_comment("comment", &url, FLOOR)]);
    assert_eq!(
        pick(&[linking.clone(), comment.clone(), holds(&[(300, &[])])]),
        format!("worker owner-comment {url}")
    );
    assert_eq!(
        pick(&[linking, comment, holds(&[(300, &[h(T1)])])]),
        "worker ready #301"
    );
    // R1.3: two held PRs are noted lowest first, whatever the list order.
    let two = set(
        "prs.json",
        format!(
            "[{},{},{}]",
            pr(305, "OPEN", "", "t/9-b", &[], "[]"),
            pr(300, "OPEN", "Closes #301", "t/301-y", &[], "[]"),
            pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]")
        ),
    );
    let o = run_with(&[two, holds(&[(305, &[h(T1)]), (300, &[h(T1)])])]).unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.starts_with("planner\n"), "{out}");
    let (a, b) = (
        out.find("PR #300 is").unwrap(),
        out.find("PR #305 is").unwrap(),
    );
    assert!(a < b, "{out}");
    // A hold on another PR number doesn't hold this one.
    assert_eq!(
        pick(&[failing.clone(), holds(&[(301, &[h(T1)]), (300, &[])])]),
        picked
    );
    // A comment with a null author (a deleted account) is not the owner's.
    let ghost = r#"{"author":null,"body":"/hold","createdAt":"2026-11-24T01:00:00Z"}"#;
    assert_eq!(on(&[ghost.to_owned()]), picked);
    // The brief notes the held PR.
    let o = run_with(&[failing.clone(), holds(&[(300, &[h(T1)])])]).unwrap();
    assert_eq!(o.status.code(), Some(0));
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.contains(
            "\n- PR #300 is on the owner's /hold (no /release after it): step 3 doesn't pick it\n"
        ),
        "{out}"
    );
    // A held green PR is noted too; a held merged PR is not.
    let o = run_with(&[holds(&[(300, &[h(T1)]), (299, &[h(T1)])])]).unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.contains("- PR #300 is on the owner's /hold"), "{out}");
    assert!(!out.contains("PR #299"), "{out}");
    // Malformed: exit 1.
    for bad in [
        r#"[{"number":300}]"#.to_owned(),
        r#"{}"#.to_owned(),
        r#"[{"number":300,"comments":[{"author":{"login":"jacobdam"},"body":"/hold"}]}]"#
            .to_owned(),
        r#"[{"number":300,"comments":[{"author":{"login":"jacobdam"}}]}]"#.to_owned(),
    ] {
        let o = run_with(&[set("holds.json", bad.clone())]).unwrap();
        assert_eq!(o.status.code(), Some(1), "{bad}");
        assert!(o.stdout.is_empty(), "{bad}");
    }
    // A non-command comment needs no createdAt.
    let plain = r#"[{"number":300,"comments":[{"author":{"login":"jacobdam"},"body":"hi"}]}]"#;
    assert_eq!(
        pick(&[failing, set("holds.json", plain.to_owned())]),
        picked
    );
}

#[test]
fn step3_skips_the_owners_own_pr_on_an_owner_branch() {
    // #298: PR 300 fails (no ai-review) and is tagged (the owner's interactive session wrote
    // it); its issue 301 is the base's ready issue.
    // Only the main worktree, so a PR branch other than t/301-y leaves no leftover (rule f).
    let wt = set(
        "worktrees.txt",
        "worktree /w/main\nHEAD a\nbranch refs/heads/main\n".to_owned(),
    );
    let on = |branch: &str| {
        set(
            "prs.json",
            format!(
                "[{},{}]",
                pr(300, "OPEN", "Closes #301", branch, &[], "[]"),
                pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]")
            ),
        )
    };
    // The owner's branch: not picked; the linked issue stays in flight, so step 10 doesn't
    // take #301 either.
    assert_eq!(pick(&[wt.clone(), on("owner/hold-automerge")]), "planner");
    assert_eq!(
        pick(&[wt.clone(), on("owner/")]),
        "planner",
        "the bare prefix"
    );
    // Near misses: picked.
    for branch in [
        "t/301-y",
        "owner",
        "owners/x",
        "Owner/x",
        "t/owner/x",
        " owner/x",
        "x-owner/y",
    ] {
        assert_eq!(
            pick(&[wt.clone(), on(branch)]),
            "worker own-pr #300",
            "{branch}"
        );
    }
    // The brief notes it, once, without the branch name.
    let o = run_with(&[wt.clone(), on("owner/hold-automerge")]).unwrap();
    assert_eq!(o.status.code(), Some(0));
    let out = String::from_utf8(o.stdout).unwrap();
    let note = "\n- PR #300 is the owner's own change (an owner/ branch, AUTOMATION.md §6.1): step 3 doesn't pick it\n";
    assert_eq!(out.matches(note).count(), 1, "{out}");
    assert!(!out.contains("hold-automerge"), "{out}");
    // A merged or closed owner PR is not noted, and doesn't hold the issue.
    for state in ["MERGED", "CLOSED"] {
        let gone = set(
            "prs.json",
            format!("[{}]", pr(300, state, "Closes #301", "owner/x", &[], "[]")),
        );
        let o = run_with(&[wt.clone(), gone]).unwrap();
        let out = String::from_utf8(o.stdout).unwrap();
        assert!(out.starts_with("worker ready #301\n"), "{state}: {out}");
        assert!(!out.contains("PR #300"), "{state}: {out}");
    }
    // A green owner PR is noted too (it waits on the owner either way).
    let green = set(
        "prs.json",
        format!(
            "[{}]",
            pr(300, "OPEN", "Closes #301", "owner/x", &[], GREEN)
        ),
    );
    let o = run_with(&[wt.clone(), green]).unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.starts_with("planner\n"), "{out}");
    assert!(out.contains(note), "{out}");
    // Held and on the owner's branch: both notes.
    let o = run_with(&[
        wt.clone(),
        on("owner/x"),
        holds(&[(300, &[cmd("jacobdam", "/hold", "2026-11-24T01:00:00Z")])]),
    ])
    .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.starts_with("planner\n"), "{out}");
    assert!(out.contains(note), "{out}");
    assert!(out.contains("- PR #300 is on the owner's /hold"), "{out}");
    // R1.1: owner-branch notes come lowest number first, after the held ones, whatever the
    // list order.
    let mixed = set(
        "prs.json",
        format!(
            "[{},{},{}]",
            pr(302, "OPEN", "", "owner/b", &[], "[]"),
            pr(300, "OPEN", "", "t/301-y", &[], "[]"),
            pr(301, "OPEN", "", "owner/a", &[], "[]")
        ),
    );
    let o = run_with(&[
        wt.clone(),
        mixed,
        holds(&[(300, &[cmd("jacobdam", "/hold", "2026-11-24T01:00:00Z")])]),
    ])
    .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.starts_with("worker ready #301\n"),
        "none links #301: {out}"
    );
    let at = |s: &str| out.find(s).unwrap_or_else(|| panic!("{s}: {out}"));
    let (h, a, b) = (
        at("- PR #300 is on the owner's /hold"),
        at("- PR #301 is the owner's own change"),
        at("- PR #302 is the owner's own change"),
    );
    assert!(h < a && a < b, "{out}");
    // Step 1: an owner issue the owner's PR closes, and an owner comment its body names, wait
    // for that PR, as for a held one.
    let url = format!("{ISSUES}777#issuecomment-1");
    let linking = |branch: &str| {
        set(
            "prs.json",
            format!(
                "[{}]",
                pr(
                    300,
                    "OPEN",
                    &format!("Closes #260\nOwner instruction: {url}"),
                    branch,
                    &[],
                    "[]"
                )
            ),
        )
    };
    let feed = feed_plus(&[feed_issue(260)]);
    assert_eq!(
        pick(&[wt.clone(), linking("t/260-x"), feed.clone()]),
        "worker owner-issue #260"
    );
    assert_eq!(
        pick(&[wt.clone(), linking("owner/x"), feed]),
        "worker ready #301",
        "the base's ready issue #301 is no longer linked"
    );
    let comment = feed_plus(&[feed_comment("comment", &url, FLOOR)]);
    assert_eq!(
        pick(&[wt.clone(), linking("t/260-x"), comment.clone()]),
        format!("worker owner-comment {url}")
    );
    assert_eq!(
        pick(&[wt.clone(), linking("owner/x"), comment]),
        "worker ready #301"
    );
}

/// `records.txt` lines (`<path>\t<line>`).
fn records(lines: &[(&str, &str)]) -> String {
    lines.iter().map(|(p, l)| format!("{p}\t{l}\n")).collect()
}

/// Step 11's near misses: a Planner record dated yesterday (NOW is 2026-11-24T06:00:00Z), and
/// today a worker record, a record whose first word only starts with `planner`, and `planner`
/// as a later word.
fn base_records() -> String {
    records(&[
        ("ops/journal/2026-11-23/2350-a.md", "- Session: planner"),
        (
            "ops/journal/2026-11-24/0100-b.md",
            "- Session: worker ready #301",
        ),
        ("ops/journal/2026-11-24/0200-c.md", "- Session: planners"),
        (
            "ops/journal/2026-11-24/0300-d.md",
            "- Session: worker planner",
        ),
        ("ops/journal/2026-11-24/0400-e.md", "- Session: Planner"),
    ])
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
fn base_is_planner_with_notes() {
    let o = run_with(&[]).unwrap();
    assert_eq!(o.status.code(), Some(0));
    let out = String::from_utf8(o.stdout).unwrap();
    assert_eq!(
        out,
        "planner\n\nRule: j (nothing applies and no Session: planner record is dated today): the Planner\n\
         Load:\n- ops/TICK.md §9\n- ops/QUEUE.md\n- PLAN.md §8\n\
         Notes:\n- worktree on t/298-x: its PRs (#299) are merged or closed; remove it\n\
         - work mix: not measured (5 of the 20 session records it needs)\n"
    );
}

/// Step 11: a `Session: planner` record dated today (UTC) makes it `idle`.
fn with_records(extra: &[(&str, &str)], now: i64) -> String {
    pick(&[
        set(
            "records.txt",
            format!("{}{}", base_records(), records(extra)),
        ),
        set("now.txt", format!("{now}\n")),
    ])
}

#[test]
fn step11_idle_when_the_planner_ran_today() {
    let today = ("ops/journal/2026-11-24/0500-p.md", "- Session: planner");
    let o = run_with(&[set(
        "records.txt",
        format!("{}{}", base_records(), records(&[today])),
    )])
    .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(o.stdout).unwrap(),
        "idle\n\nRule: j (nothing applies and the Planner ran today): exit now, write no record\nLoad:\n\
         Notes:\n- worktree on t/298-x: its PRs (#299) are merged or closed; remove it\n"
    );
    // Words after the type don't matter; nor does the order of the lines.
    let later = (
        "ops/journal/2026-11-24/0000-p.md",
        "- Session: planner  queue #5",
    );
    assert_eq!(with_records(&[later], NOW), "idle");
    let first = set(
        "records.txt",
        format!("{}{}", records(&[today]), base_records()),
    );
    assert_eq!(pick(&[first]), "idle");
    // Day boundary: 2026-11-24T00:00:00Z is 1_795_478_400. Only today's record, without the
    // base's yesterday record: a record dated tomorrow doesn't count. A Planner record of
    // 2026-11-20 keeps step 8's 7-day trigger off on both days.
    let midnight = 1_795_478_400;
    let older = ("ops/journal/2026-11-20/0000-p.md", "- Session: planner");
    let only = |now: i64| {
        pick(&[
            set("records.txt", records(&[today, older])),
            set("now.txt", format!("{now}\n")),
        ])
    };
    assert_eq!(only(midnight), "idle");
    assert_eq!(only(midnight - 1), "planner", "a record dated tomorrow");
    // One second before midnight, yesterday's base record is today's.
    assert_eq!(with_records(&[], midnight - 1), "idle");
    assert_eq!(with_records(&[], midnight), "planner");
    // Near misses: another day's directory, a record in a file not under a day directory is
    // malformed (below), and an empty records file.
    let other = ("ops/journal/2026-11-25/0000-p.md", "- Session: planner");
    assert_eq!(with_records(&[other], NOW), "planner");
    // An empty records file has no Planner record in the last 7 days either: step 8 (#244).
    assert_eq!(pick(&[set("records.txt", String::new())]), "planner due");
    // Step 11 comes last: an earlier step still wins over idle.
    let ran = set(
        "records.txt",
        format!("{}{}", base_records(), records(&[today])),
    );
    assert_eq!(
        pick(&[
            ran.clone(),
            plus("issues.json", &[ready(700, "M0", &[], "")])
        ]),
        "worker ready #700"
    );
    assert_eq!(
        pick(&[ran, feed_plus(&[feed_issue(250)])]),
        "worker owner-issue #250"
    );
}

#[test]
fn step11_records_malformed_exit_1() {
    for text in [
        "ops/journal/2026-11-24/a.md - Session: planner\n",
        "ops/journal/2026-11-24/a.md\t-  Session: planner\n",
        "ops/journal/2026-11-24/a.md\tSession: planner\n",
        "ops/journal/2026-11-24.md\t- Session: planner\n",
        "ops/journal/2026-11-24/x/a.md\t- Session: planner\n",
        "ops/journal/2026-11-24/a.txt\t- Session: planner\n",
        "ops/journal/2026-13-24/a.md\t- Session: planner\n",
        "ops/journal/26-11-24/a.md\t- Session: planner\n",
        "docs/2026-11-24/a.md\t- Session: planner\n",
        "ops/journal/2026-11-24/a.md\t- Session: worker\nbad line\n",
    ] {
        let o = run_with(&[set("records.txt", text.to_owned())]).unwrap();
        assert_eq!(o.status.code(), Some(1), "{text:?}");
        assert!(o.stdout.is_empty(), "{text:?}");
    }
    // R1.5: a malformed line after a Planner record dated today still fails (no early return).
    let o = run_with(&[set(
        "records.txt",
        "ops/journal/2026-11-24/a.md\t- Session: planner\nbad line\n".to_owned(),
    )])
    .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    // Parsed whatever is picked.
    let o = run_with(&[
        set("records.txt", "bad\n".to_owned()),
        feed_plus(&[feed_issue(250)]),
    ])
    .unwrap();
    assert_eq!(o.status.code(), Some(1));
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
    assert_eq!(with(&done(250)), "planner");
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
            "planner",
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
    assert_eq!(linked("OPEN", GREEN), "planner");
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
    assert_eq!(conflicted("UNKNOWN"), "planner");
    assert_eq!(conflicted("MERGEABLE"), "planner");
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
        "planner",
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
        "planner"
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
        "planner"
    );
    assert_eq!(
        pick(&[plus("issues.json", &[issue(250, "c", &["crash"])]), quoted]),
        "planner"
    );
    // R1.2: an owner PR in the feed is not an owner issue; is_pr must be a boolean.
    let owner_pr =
        r#"{"kind":"issue","number":270,"title":"t","labels":[],"is_pr":true,"body":"b"}"#;
    assert_eq!(pick(&[feed_plus(&[owner_pr.to_owned()])]), "planner");
    let no_flag = r#"{"kind":"issue","number":270,"title":"t","labels":[],"body":"b"}"#;
    let o = run_with(&[feed_plus(&[no_flag.to_owned()])]).unwrap();
    assert_eq!(o.status.code(), Some(1));
    // R1.3: an untagged (owner) PR is never the agent's own, failing or in flight.
    let owner = |body: &str| plus("prs.json", &[raw_pr(350, "OPEN", body, "o/x", &[], "[]")]);
    assert_eq!(pick(&[owner("Closes #301")]), "planner");
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
    assert_eq!(pick(&[closed]), "planner");
    let longer = owner_reply(&format!("{TAG} picked up {ISSUES}1730"));
    let closed = set(
        "owner-closed.json",
        format!(r#"[{{"number":173,"body":"b","comments":[{longer}]}}]"#),
    );
    assert_eq!(pick(&[closed]), "planner");
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
    assert_eq!(
        closed("Owner text", &[picked.clone(), done(173)]),
        "planner"
    );
    assert_eq!(
        closed("Owner text", &[picked.clone(), done(1730)]),
        "worker owner-done #173"
    );
    assert_eq!(
        closed(&format!(" {TAG} filed for the owner"), &[picked]),
        "planner",
        "tagged"
    );
    assert_eq!(closed("Owner text", &[]), "planner");
    let stranger = format!(r#"{{"author":{{"login":"x"}},"body":"{TAG} picked up"}}"#);
    assert_eq!(closed("Owner text", &[stranger]), "planner");
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
    assert_eq!(red("failure", "pull_request"), "planner");
    assert_eq!(red("cancelled", "push"), "planner");
    // The newest completed run decides, by id, not list order.
    let newer_green = runs(&[
        run_entry(10, "completed", "failure", "push"),
        run_entry(20, "completed", "success", "push"),
    ]);
    assert_eq!(pick(&[newer_green]), "planner");
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
    assert!(out.starts_with("planner\n"), "{out}");
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
    assert_eq!(one(&format!("[{ok}]"), &[]), "planner");
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
        assert_eq!(one(&r, &[]), "planner", "{c}");
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
    assert_eq!(one("[]", &["owner-todo"]), "planner");
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
    assert_eq!(pick(&[failing.clone(), parked]), "planner");
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
        assert_eq!(with(m(green("OPEN", &[]), v)), "planner", "{v}");
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
            "planner",
            "{skip} on the PR"
        );
        let parked = set("issues.json", format!("[{}]", issue(301, "f", &[skip])));
        let prs = set(
            "prs.json",
            format!("[{},{merged}]", m(green("OPEN", &[]), "CONFLICTING")),
        );
        assert_eq!(pick(&[prs, parked]), "planner", "{skip} on the issue");
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
    assert_eq!(bug("next-session: parse error", &[]), "planner");
    assert_eq!(bug("next-session parse error", &["bug"]), "planner");
    assert_eq!(bug("next-session: x", &["bug", "owner-todo"]), "planner");
    let flight = plus(
        "prs.json",
        &[pr(401, "OPEN", "Closes #400", "t/400-a", &[], GREEN)],
    );
    assert_eq!(
        pick(&[
            plus("issues.json", &[issue(400, "next-session: x", &["bug"])]),
            flight
        ]),
        "planner"
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
    assert_eq!(one(&["security", "sev:medium"]), "planner");
    assert_eq!(one(&["sev:high"]), "planner", "not a security issue");
    assert_eq!(one(&["crash", "owner-todo"]), "planner");
    assert_eq!(one(&["crash", "blocked"]), "planner");
    let flight = plus(
        "prs.json",
        &[pr(501, "OPEN", "Closes #500", "t/500-a", &[], GREEN)],
    );
    assert_eq!(
        pick(&[plus("issues.json", &[issue(500, "x", &["crash"])]), flight]),
        "planner"
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
        "planner"
    );
    let flight = plus(
        "prs.json",
        &[pr(601, "OPEN", "Closes #600", "t/600-a", &[], GREEN)],
    );
    assert_eq!(
        pick(&[plus("issues.json", &[issue(600, "f", &["flaky"])]), flight]),
        "planner"
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
    assert_eq!(pick(&[extra("u/9-z")]), "planner");
    assert_eq!(pick(&[extra("x-t/9-z")]), "planner");
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
    assert!(out.starts_with("planner\n"), "{out}");
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
            "design-technical design-issue #650",
            vec![],
            vec![issue(650, "d", &["design", "ready"])],
            vec![],
        ),
        (
            "audit due",
            vec![],
            vec![],
            vec![(
                "main-log.txt",
                [
                    commit(103, RETRO_T, "Retro", &[("A", "docs/retros/0001.md")]),
                    commit(
                        101,
                        ROOT_T,
                        "Add plan",
                        &[("A", "PLAN.md"), ("A", "docs/PRODUCT.md")],
                    ),
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
            "design product #800",
            vec![],
            vec![ready(800, "M3", &["design-product"], "")],
            vec![],
        ),
        (
            "worker ready #700",
            vec![],
            vec![ready(700, "M0", &[], "")],
            vec![],
        ),
        (
            "idle",
            vec![],
            vec![],
            vec![(
                "records.txt",
                records(&[("ops/journal/2026-11-24/0500-p.md", "- Session: planner")]),
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
    // Step 8's Planner term (#244) can't sit in the list above: the `idle` entry's Planner record of today turns
    // it off. Without that entry and with no ops/QUEUE.md, every step from 1 to 7 still wins
    // over `planner due`, and `planner due` wins over the ready issue of step 10.
    let ready_at = steps
        .iter()
        .position(|s| s.0 == "worker ready #700")
        .unwrap();
    // `planner due` also wins over Design: product, its step-8 neighbour (#249).
    let product_at = steps
        .iter()
        .position(|s| s.0 == "design product #800")
        .unwrap();
    for start in 0..=ready_at {
        let mut feed = Vec::new();
        let mut issues = Vec::new();
        let mut changes: Vec<(&str, Option<String>)> = vec![("queue.md", None)];
        for (_, f, i, c) in &steps[start..=ready_at] {
            feed.extend(f.iter().cloned());
            issues.extend(i.iter().cloned());
            changes.extend(c.iter().map(|(n, t)| (*n, Some(t.clone()))));
        }
        changes.push(feed_plus(&feed));
        changes.push(plus("issues.json", &issues));
        let want = if start >= product_at {
            "planner due"
        } else {
            steps[start].0
        };
        let got = pick(&changes);
        assert!(got.starts_with(want), "step 8, from step {start}: {got}");
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
    // merged PR (every PR is parsed in full). The unchanged list picks `planner`.
    let open = pr(300, "OPEN", "Closes #301", "t/301-y", &[], GREEN);
    let merged = pr(299, "MERGED", "Closes #298", "t/298-x", &[], "[]");
    assert_eq!(
        pick(&[set("prs.json", format!("[{open},{merged}]"))]),
        "planner"
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
        "records.txt",
        "pr-files.json",
        "all-issues.json",
        "areas.toml",
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
    assert_eq!(pick(&[log_with(&merges(8, "crates/c.rs"))]), "planner");
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
        assert_eq!(pick(&[log_with(&merges(9, path))]), "planner", "{path}");
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

/// #282: a due audit or retro is not picked while an open agent PR links an open issue with
/// that label (the slot is already being done), whatever the PR's checks say.
#[test]
fn step7_slot_in_flight() {
    let audit_due = log_with(&merges(9, "crates/c.rs"));
    let slot = |issue_labels: &[&str], pr_json: String| {
        vec![
            audit_due.clone(),
            plus("issues.json", &[issue(400, "Audit 0007", issue_labels)]),
            plus("prs.json", &[pr_json]),
        ]
    };
    let open_pr = |body: &str| pr(401, "OPEN", body, "t/400-audit", &[], GREEN);
    assert_eq!(pick(std::slice::from_ref(&audit_due)), "audit due");
    let doing = slot(&["audit", "in-progress"], open_pr("Closes #400"));
    assert_eq!(pick(&doing), "planner");
    let out = String::from_utf8(run_with(&doing).unwrap().stdout).unwrap();
    assert!(
        out.contains(&format!(
            "- audit due (10 product merges since {:07x}) but PR #401 is open for it: not picked\n",
            104
        )),
        "{out}"
    );
    // The label alone is enough, and a draft or failing PR counts too: a failing one is step 3's.
    assert_eq!(pick(&slot(&["audit"], open_pr("Closes #400"))), "planner");
    let failing = pr(401, "OPEN", "Closes #400", "t/400-audit", &[], "null");
    assert_eq!(
        pick(&slot(&["audit"], failing.clone())),
        "worker own-pr #401"
    );
    // Parked for the owner: step 3 skips it, and the audit is still not redone.
    assert_eq!(pick(&slot(&["audit", "owner-todo"], failing)), "planner");
    // Near misses: the audit is still due.
    for (l, p) in [
        (vec!["retro"], open_pr("Closes #400")),
        (vec!["audit-x"], open_pr("Closes #400")),
        (vec!["in-progress"], open_pr("Closes #400")),
        (vec!["audit"], open_pr("Closes #402")),
        (vec!["audit"], open_pr("See #400")),
        (
            vec!["audit"],
            pr(401, "MERGED", "Closes #400", "t/400-audit", &[], "[]"),
        ),
        (
            vec!["audit"],
            pr(401, "CLOSED", "Closes #400", "t/400-audit", &[], "[]"),
        ),
        (
            vec!["audit"],
            raw_pr(401, "OPEN", "Closes #400", "t/400-audit", &[], GREEN),
        ),
        // The PR's own label is not the linked issue's.
        (
            vec![],
            pr(401, "OPEN", "Closes #400", "t/400-audit", &["audit"], GREEN),
        ),
    ] {
        assert_eq!(pick(&slot(&l, p.clone())), "audit due", "{l:?} {p}");
    }
    // A linked issue that is closed (not in issues.json) doesn't hold the slot.
    assert_eq!(
        pick(&[
            audit_due.clone(),
            plus("prs.json", &[open_pr("Closes #400")])
        ]),
        "audit due"
    );

    // The retro: due 7 days after its record (the audit record stays newer).
    let now = set("now.txt", format!("{}\n", RETRO_T + WEEK));
    let log = set(
        "main-log.txt",
        [
            commit(
                104,
                RETRO_T + WEEK - 10,
                "Audit",
                &[("A", "docs/audits/0006.md")],
            ),
            commit(103, RETRO_T, "Retro", &[("A", "docs/retros/0001.md")]),
            commit(
                101,
                ROOT_T,
                "Add plan",
                &[("A", "PLAN.md"), ("A", "docs/PRODUCT.md")],
            ),
        ]
        .concat(),
    );
    let retro = |issue_labels: &[&str]| {
        pick(&[
            now.clone(),
            log.clone(),
            plus("issues.json", &[issue(410, "Retro 0003", issue_labels)]),
            plus(
                "prs.json",
                &[pr(411, "OPEN", "Closes #410", "t/410-retro", &[], GREEN)],
            ),
        ])
    };
    assert_eq!(retro(&[]), "retro due");
    assert_eq!(retro(&["audit"]), "retro due");
    assert_eq!(retro(&["retro", "in-progress"]), "planner");
    // An audit in flight while a retro is due falls through to the retro.
    let mut both = slot(&["audit"], open_pr("Closes #400"));
    both.push(set("now.txt", format!("{}\n", NOW + 6 * DAY)));
    // NOW + 6 days is 2026-11-30: a Planner record of 2026-11-29 keeps step 8 off.
    both.push(set(
        "records.txt",
        records(&[("ops/journal/2026-11-29/0000-p.md", "- Session: planner")]),
    ));
    assert_eq!(pick(&both), "retro due");
    // Both in flight: neither is picked, and the brief notes both.
    both.extend([
        plus(
            "issues.json",
            &[
                issue(400, "Audit 0007", &["audit"]),
                issue(410, "Retro 0003", &["retro"]),
            ],
        ),
        plus(
            "prs.json",
            &[
                open_pr("Closes #400"),
                pr(411, "OPEN", "Closes #410", "t/410-retro", &[], GREEN),
            ],
        ),
    ]);
    assert_eq!(pick(&both), "planner");
    let out = String::from_utf8(run_with(&both).unwrap().stdout).unwrap();
    assert!(
        out.contains(&format!(
            "- audit due (10 product merges since {:07x}) but PR #401 is open for it: not picked\n",
            104
        )),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            "- retro due (8 days since {:07x}) but PR #411 is open for it: not picked\n",
            103
        )),
        "{out}"
    );
    // Two open PRs hold the audit: the note names the lowest number, whatever the list order.
    let two = vec![
        audit_due.clone(),
        plus(
            "issues.json",
            &[
                issue(400, "Audit 0007", &["audit"]),
                issue(402, "Audit 0007", &["audit"]),
            ],
        ),
        plus(
            "prs.json",
            &[
                pr(403, "OPEN", "Closes #402", "t/402-audit", &[], GREEN),
                open_pr("Closes #400"),
            ],
        ),
    ];
    let lowest = String::from_utf8(run_with(&two).unwrap().stdout).unwrap();
    assert!(lowest.contains("but PR #401 is open for it"), "{lowest}");
    assert!(!lowest.contains("PR #403"), "{lowest}");
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
            commit(
                101,
                ROOT_T,
                "Add plan",
                &[("A", "PLAN.md"), ("A", "docs/PRODUCT.md")],
            ),
        ]
        .concat(),
    );
    // AUDIT_T + WEEK - 1 is 2026-11-30T05:59:59Z: a Planner record of 2026-11-29 keeps step 8's
    // 7-day trigger off, so nothing applies.
    let planned = set(
        "records.txt",
        records(&[("ops/journal/2026-11-29/0000-p.md", "- Session: planner")]),
    );
    assert_eq!(
        pick(&[at(AUDIT_T + WEEK - 1), log.clone(), planned]),
        "planner"
    );
    assert_eq!(pick(&[at(AUDIT_T + WEEK), log]), "audit due");
    // No audit record at all.
    let no_audit = |files: &[(&str, &str)]| {
        let log = [
            commit(110, NOW - 60, "x", files),
            commit(103, RETRO_T, "Retro", &[("A", "docs/retros/0001.md")]),
            commit(
                101,
                ROOT_T,
                "Add plan",
                &[("A", "PLAN.md"), ("A", "docs/PRODUCT.md")],
            ),
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
    assert_eq!(no_audit(&[("A", "docs/advocate/0007.md")]), "planner");
    assert_eq!(no_audit(&[("A", "docs/audits/0007.md")]), "planner");
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
            commit(
                101,
                ROOT_T,
                "Add plan",
                &[("A", "PLAN.md"), ("A", "docs/PRODUCT.md")],
            ),
        ]
        .concat();
        set("main-log.txt", log)
    };
    assert_eq!(
        pick(&[at(RETRO_T + WEEK - 1), log_at(RETRO_T + WEEK - 10)]),
        "planner"
    );
    assert_eq!(
        pick(&[at(RETRO_T + WEEK), log_at(RETRO_T + WEEK - 10)]),
        "retro due"
    );
    // No retro record: seven days after the first commit.
    let no_retro = |now: i64| {
        let log = [
            commit(104, now - 10, "Audit", &[("A", "docs/audits/0006.md")]),
            commit(
                101,
                ROOT_T,
                "Add plan",
                &[("A", "PLAN.md"), ("A", "docs/PRODUCT.md")],
            ),
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
    assert_eq!(no_retro(ROOT_T + WEEK - 1), "planner");
    assert_eq!(no_retro(ROOT_T + WEEK), "retro due");
    // An audit due comes first.
    assert_eq!(pick(&[at(AUDIT_T + WEEK)]), "audit due");
}

#[test]
fn step7_retro_incidents() {
    // A revert after the retro record (the base has one before it).
    let rev = |subject: &str| pick(&[log_with(&[commit(120, NOW - 60, subject, &[])])]);
    assert_eq!(rev("Revert \"feature\" (#10) (#11)"), "retro due");
    assert_eq!(rev("Reverts feature"), "planner");
    assert_eq!(rev("Revert feature"), "planner");
    assert_eq!(rev("x Revert \"feature\""), "planner");
    // A judge alarm raised after the retro record.
    let alarm = |t: i64| pick(&[set("judge-alarm-since", format!("{t}\n"))]);
    assert_eq!(alarm(RETRO_T + 1), "retro due");
    assert_eq!(alarm(RETRO_T), "planner");
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
        assert_eq!(tag(&near), "planner", "{near}");
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
        "planner"
    );
    assert_eq!(
        third(&[labeled("blocked-x", "2026-11-24T05:00:00Z")]),
        "planner"
    );
    assert_eq!(
        third(&[
            r#"{"event":"unlabeled","created_at":"2026-11-24T05:00:00Z","label":{"name":"blocked"}}"#
                .to_owned()
        ]),
        "planner"
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
        "planner"
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
    assert_eq!(with(&[ready(701, "M1", &[], "")]), "planner");
    let ms = |list: &str| set("milestones.json", list.to_owned());
    assert_eq!(
        pick(&[
            plus(
                "issues.json",
                &[ready(701, "M2", &[], ""), ready(702, "M10", &[], "")]
            ),
            ms(r#"[{"title":"M10"},{"title":"M2"},{"title":"Mx"},{"title":"M-1"}]"#),
            // M2 is planned, so step 8 doesn't pick the Planner first.
            set("queue.md", "Milestone: M2\n".to_owned())
        ]),
        "worker ready #701"
    );
    assert_eq!(
        pick(&[plus("issues.json", &[ready(701, "M0", &[], "")]), ms("[]")]),
        "planner"
    );
    // Near misses: no milestone, not ready, each skip label, in flight.
    assert_eq!(
        with(&[full_issue(701, "r", &["ready"], "", "null")]),
        "planner"
    );
    assert_eq!(
        with(&[full_issue(701, "r", &[], "", r#"{"title":"M0"}"#)]),
        "planner"
    );
    for l in [
        "owner-todo",
        "blocked",
        "deferred",
        "in-progress",
        "design",
        "design-pending",
    ] {
        // Since #275 step 6 takes a `design` and `ready` issue before step 10 sees it, so step
        // 10's own `design` skip is defensive only and no fixture reaches it (R1.2).
        let want = if l == "design" {
            "design-technical design-issue #701"
        } else {
            "planner"
        };
        assert_eq!(with(&[ready(701, "M0", &[l], "")]), want, "{l}");
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
    assert_eq!(one("x\nDepends on: #150 (task 1a)."), "planner");
    assert_eq!(one("**Depends on:** #149, #150."), "planner");
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
    assert_eq!(
        one("Depends on: #149 (task 1a), #150 (task 1b)."),
        "planner"
    );
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
fn queue_not_utf8_exit_1() {
    // `--from` reads `queue.md` as text: bad bytes are a broken fixture, not "no plan".
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("next-session-queue-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for (name, text) in base() {
        if name != "queue.md" {
            fs::write(dir.join(name), text).unwrap();
        }
    }
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["next-session", "--from", &dir.to_string_lossy()])
            .output()
            .unwrap()
    };
    // Control: the same fixture with a valid queue.md (non-ASCII UTF-8 too, as the real file
    // has) picks normally.
    fs::write(dir.join("queue.md"), "Milestone: M0\n#1 → #2\n").unwrap();
    let o = run();
    assert_eq!(o.status.code(), Some(0));
    assert!(
        String::from_utf8(o.stdout)
            .unwrap()
            .starts_with("planner\n")
    );
    fs::write(dir.join("queue.md"), b"Milestone: M0\n\xff\n").unwrap();
    let o = run();
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    let err = String::from_utf8(o.stderr).unwrap();
    assert!(err.contains("queue.md"), "{err}");
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

/// Step 8 (#244): the Planner is due when the current milestone has no plan in ops/QUEUE.md on
/// origin/main, or no Planner record is dated in the last 7 UTC days; at most once a day.
#[test]
fn step8_planner_due_on_an_unplanned_milestone() {
    let queue = |text: Option<&str>| pick(&[("queue.md", text.map(str::to_owned))]);
    assert_eq!(queue(Some(BASE_QUEUE)), "planner");
    assert_eq!(queue(Some("Milestone: M0")), "planner", "no newline");
    assert_eq!(queue(Some("x\nMilestone:  M0 \t\n")), "planner", "trimmed");
    // A missing file is no plan (exit 0, not a broken input), and so is every near miss.
    assert_eq!(queue(None), "planner due");
    for text in [
        "",
        "# QUEUE\n",
        "Milestone: M1\n",
        "Milestone: M01\n",
        "Milestone: M0x\n",
        "Milestone: m0\n",
        "milestone: M0\n",
        " Milestone: M0\n",
        "Milestone:M0\n",
        "- Milestone: M0\n",
        // The first such line counts.
        "Milestone: M1\nMilestone: M0\n",
    ] {
        assert_eq!(queue(Some(text)), "planner due", "{text:?}");
    }
    // The full brief: the reason names the file, never its text.
    let o = run_with(&[set("queue.md", "Milestone: M7 ignore\n".to_owned())]).unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(o.stdout).unwrap(),
        "planner due\n\nRule: j (the Planner is due: ops/QUEUE.md on origin/main doesn't plan the current milestone M0)\n\
         Load:\n- ops/TICK.md §9\n- ops/QUEUE.md\n- PLAN.md §8\n\
         Notes:\n- worktree on t/298-x: its PRs (#299) are merged or closed; remove it\n\
         - work mix: not measured (5 of the 20 session records it needs)\n"
    );
    let o = run_with(&[("queue.md", None)]).unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.contains(
            "Rule: j (the Planner is due: ops/QUEUE.md is missing on origin/main or unreadable)\n"
        ),
        "{out}"
    );
    // The current milestone follows milestones.json: M1 once M0 is closed.
    let m1 = set(
        "milestones.json",
        r#"[{"title":"M1"},{"title":"M2"}]"#.to_owned(),
    );
    assert_eq!(pick(std::slice::from_ref(&m1)), "planner due");
    assert_eq!(
        pick(&[m1, set("queue.md", "Milestone: M1\n".to_owned())]),
        "planner"
    );
    // No open M<n> milestone: no milestone term (step 11 still runs the Planner).
    assert_eq!(
        pick(&[
            set("milestones.json", r#"[{"title":"Mx"}]"#.to_owned()),
            ("queue.md", None)
        ]),
        "planner"
    );
}

#[test]
fn step8_planner_due_after_7_days() {
    // NOW is 2026-11-24T06:00:00Z; the base plans M0. Only these records.
    let only = |lines: &[(&str, &str)]| pick(&[set("records.txt", records(lines))]);
    let on = |day: &str| format!("ops/journal/{day}/0000-p.md");
    let (d6, d7, d1) = (on("2026-11-18"), on("2026-11-17"), on("2026-11-25"));
    assert_eq!(
        only(&[(&d6, "- Session: planner")]),
        "planner",
        "6 days ago"
    );
    assert_eq!(
        only(&[(&d7, "- Session: planner")]),
        "planner due",
        "7 days ago"
    );
    assert_eq!(
        only(&[(&d1, "- Session: planner")]),
        "planner due",
        "tomorrow"
    );
    // `planner due` is a Planner record too; other kinds are not.
    assert_eq!(only(&[(&d6, "- Session: planner due")]), "planner");
    for kind in [
        "- Session: planners",
        "- Session: Planner",
        "- Session: worker planner",
        "- Session: worker ready #5",
    ] {
        assert_eq!(only(&[(&d6, kind)]), "planner due", "{kind}");
    }
    // The reason, and both reasons together.
    let o = run_with(&[set("records.txt", String::new()), ("queue.md", None)]).unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.contains(
            "Rule: j (the Planner is due: ops/QUEUE.md is missing on origin/main or unreadable; \
             no Session: planner record is dated in the last 7 UTC days)\n"
        ),
        "{out}"
    );
    // The window moves with the clock: at the last second of 2026-11-24 the 2026-11-18 record
    // still counts; at the next midnight it is 7 days old.
    let midnight = 1_795_564_800; // 2026-11-25T00:00:00Z
    let at = |now: i64| {
        pick(&[
            set("records.txt", records(&[(&d6, "- Session: planner")])),
            set("now.txt", format!("{now}\n")),
        ])
    };
    assert_eq!(at(midnight - 1), "planner");
    assert_eq!(at(midnight), "planner due");
}

#[test]
fn step8_once_a_day_and_precedence() {
    let today = ("ops/journal/2026-11-24/0500-p.md", "- Session: planner due");
    let ran = set("records.txt", records(&[today]));
    // The Planner ran today (a `planner due` record): no step 8, and step 11 is idle.
    assert_eq!(pick(&[ran.clone(), ("queue.md", None)]), "idle");
    // Step 8 comes before step 10 and after step 7.
    let ready = plus("issues.json", &[ready(700, "M0", &[], "")]);
    assert_eq!(pick(std::slice::from_ref(&ready)), "worker ready #700");
    assert_eq!(pick(&[ready.clone(), ("queue.md", None)]), "planner due");
    assert_eq!(
        pick(&[ready.clone(), set("records.txt", String::new())]),
        "planner due"
    );
    // Once it ran today, step 10 picks again.
    assert_eq!(pick(&[ready, ("queue.md", None), ran]), "worker ready #700");
    let audit = set(
        "main-log.txt",
        commit(
            101,
            ROOT_T,
            "Add plan",
            &[("A", "PLAN.md"), ("A", "docs/PRODUCT.md")],
        ),
    );
    assert_eq!(pick(&[audit, ("queue.md", None)]), "audit due");
    assert_eq!(
        pick(&[feed_plus(&[feed_issue(250)]), ("queue.md", None)]),
        "worker owner-issue #250"
    );
}

/// `main-log.txt` without `docs/PRODUCT.md`: the base log minus its root commit's line.
fn log_without_product() -> (&'static str, Option<String>) {
    let log = base_log().concat();
    let line = "A\tdocs/PRODUCT.md\n";
    assert_eq!(log.matches(line).count(), 1);
    ("main-log.txt", Some(log.replace(line, "")))
}

/// Step 8, after the Planner (#249, ops/TICK.md §10): Design: product. `docs/PRODUCT.md`
/// missing on origin/main → `design product v1`; else the lowest workable `ready`
/// `design-product` issue; neither while an open agent PR links an open `design-product` issue.
#[test]
fn step8_design_product() {
    let missing = log_without_product();
    assert_eq!(pick(std::slice::from_ref(&missing)), "design product v1");
    let o = run_with(std::slice::from_ref(&missing)).unwrap();
    assert_eq!(
        String::from_utf8(o.stdout).unwrap(),
        "design product v1\n\nRule: k (Design: product is due: docs/PRODUCT.md is not on origin/main)\n\
         Load:\n- ops/TICK.md §10\n- PLAN.md §1, §4\n- specs/journeys/\n\
         Notes:\n- worktree on t/298-x: its PRs (#299) are merged or closed; remove it\n"
    );
    // The newest commit touching the path decides: deleted → missing, re-added or changed →
    // present. A rename (`--no-renames`: D plus A) away from the path is a delete.
    let top = |st: &str, path: &str| commit(110, NOW - 60, "x", &[(st, path)]);
    assert_eq!(
        pick(&[log_with(&[top("D", "docs/PRODUCT.md")])]),
        "design product v1"
    );
    for st in ["M", "T", "A"] {
        assert_eq!(
            pick(&[log_with(&[top(st, "docs/PRODUCT.md")])]),
            "planner",
            "{st}"
        );
    }
    assert_eq!(
        pick(&[log_with(&[
            top("A", "docs/PRODUCT.md"),
            commit(109, NOW - 120, "y", &[("D", "docs/PRODUCT.md")]),
        ])]),
        "planner"
    );
    assert_eq!(
        pick(&[log_with(&[
            top("D", "docs/PRODUCT.md"),
            commit(109, NOW - 120, "y", &[("A", "docs/PRODUCT.md")]),
        ])]),
        "design product v1"
    );
    // Only the exact path counts.
    for path in [
        "docs/PRODUCT.md.bak",
        "docs/product.md",
        "PRODUCT.md",
        "x/docs/PRODUCT.md",
        "\"docs/PRODUCT.md\"",
    ] {
        let only = commit(1, ROOT_T, "root", &[("A", path)]);
        let log = log_without_product()
            .1
            .unwrap()
            .replace(&commit(101, ROOT_T, "Add plan", &[("A", "PLAN.md")]), &only);
        assert!(log.contains(path), "{path}");
        assert_eq!(
            pick(&[set("main-log.txt", log)]),
            "design product v1",
            "{path}"
        );
    }

    // With the file on main: a ready design-product issue, lowest number first, in any
    // milestone (or none).
    let product = |n: u64, l: &[&str]| {
        let mut all = vec!["design-product"];
        all.extend_from_slice(l);
        ready(n, "M5", &all, "")
    };
    assert_eq!(
        pick(&[plus("issues.json", &[product(902, &[]), product(901, &[])])]),
        "design product #901"
    );
    assert_eq!(
        pick(&[plus(
            "issues.json",
            &[full_issue(
                903,
                "p",
                &["design-product", "ready"],
                "",
                "null"
            )]
        )]),
        "design product #903"
    );
    let o = run_with(&[plus("issues.json", &[product(901, &[])])]).unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.starts_with(
            "design product #901\n\nRule: k (Design: product is due: a ready design-product issue)\n\
             Load:\n- gh issue view 901 --comments\n- ops/TICK.md §10\n- docs/PRODUCT.md\n- PLAN.md §1, §4\n- specs/journeys/\n"
        ),
        "{out}"
    );
    // It wins over a ready Worker issue (step 10), and step 10 never takes it.
    assert_eq!(
        pick(&[plus(
            "issues.json",
            &[product(901, &[]), ready(10, "M0", &[], "")]
        )]),
        "design product #901"
    );
    // Its dependencies must be closed, as in step 10: a `Depends on:` issue not in issues.json
    // is closed.
    assert_eq!(
        pick(&[plus(
            "issues.json",
            &[ready(901, "M5", &["design-product"], "Depends on: #5")]
        )]),
        "design product #901"
    );
    // Near misses: not ready, a skip label, an open dependency, the wrong label (an in-flight
    // issue is the slot rule's, below); and while the file
    // is missing only v1 is picked.
    for (issues, prs) in [
        (
            vec![full_issue(901, "p", &["design-product"], "", "null")],
            vec![],
        ),
        (vec![product(901, &["owner-todo"])], vec![]),
        (vec![product(901, &["blocked"])], vec![]),
        (vec![product(901, &["deferred"])], vec![]),
        (vec![ready(901, "M5", &["design"], "")], vec![]),
        (vec![ready(901, "M5", &["design-products"], "")], vec![]),
        (
            vec![
                ready(901, "M5", &["design-product"], "Depends on: #5"),
                issue(5, "dep", &[]),
            ],
            vec![],
        ),
    ] {
        let mut changes = vec![plus("issues.json", &issues)];
        if !prs.is_empty() {
            changes.push(plus("prs.json", &prs));
        }
        let got = pick(&changes);
        // A `design` and `ready` issue is step 6's since #275.
        assert!(
            got == "planner"
                || got == "worker ready #901"
                || got == "design-technical design-issue #901",
            "{issues:?}: {got}"
        );
        assert_ne!(got, "design product #901");
    }
    // While the file is missing, v1 reads only the design-product issues without `ready`
    // (v1's; a ready one is a gap issue for later, R4.2), and a ready one is never `#N`.
    assert_eq!(
        pick(&[missing.clone(), plus("issues.json", &[product(901, &[])])]),
        "design product v1"
    );
    let v1_issue = |n: u64, l: &[&str], body: &str| {
        let mut all = vec!["design-product"];
        all.extend_from_slice(l);
        full_issue(n, "v1", &all, body, "null")
    };
    let o = run_with(&[
        missing.clone(),
        plus(
            "issues.json",
            &[v1_issue(922, &[], ""), v1_issue(921, &[], "")],
        ),
    ])
    .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.starts_with(
            "design product v1 #921\n\nRule: k (Design: product is due: docs/PRODUCT.md is not on origin/main)\n\
             Load:\n- gh issue view 921 --comments\n- ops/TICK.md §10\n- PLAN.md §1, §4\n- specs/journeys/\n"
        ),
        "{out}"
    );
    // While any v1 issue waits, nothing is picked (R3.1, R4.2: an escalated v1 issue waiting
    // on its design ADR), whatever other v1 or gap issues are open.
    assert_eq!(
        pick(&[
            missing.clone(),
            plus(
                "issues.json",
                &[
                    product(320, &[]),
                    v1_issue(330, &[], "Depends on: #5"),
                    issue(5, "design", &["design"]),
                ],
            ),
        ]),
        "planner"
    );
    // A waiting gap issue (`ready`, with a skip label or an open dependency) is not v1's, so it
    // never stops a workable v1 issue (#313: `waiting` over v1's issues only, not all of them).
    for gap in [
        product(320, &["owner-todo"]),
        ready(320, "M5", &["design-product"], "Depends on: #5"),
    ] {
        assert_eq!(
            pick(&[
                missing.clone(),
                plus(
                    "issues.json",
                    &[
                        gap.clone(),
                        v1_issue(330, &[], ""),
                        issue(5, "design", &["design"])
                    ],
                ),
            ]),
            "design product v1 #330",
            "{gap}"
        );
    }
    assert_eq!(
        pick(&[
            missing.clone(),
            plus(
                "issues.json",
                &[
                    v1_issue(921, &["owner-todo"], ""),
                    v1_issue(922, &[], "Depends on: #5"),
                    v1_issue(923, &[], ""),
                    issue(5, "design", &["design"]),
                ],
            ),
        ]),
        "planner"
    );
    for waiting in [
        v1_issue(921, &["owner-todo"], ""),
        v1_issue(921, &["blocked"], ""),
        v1_issue(921, &["deferred"], ""),
        v1_issue(921, &[], "Depends on: #5"),
    ] {
        let changes = vec![
            missing.clone(),
            plus(
                "issues.json",
                &[waiting.clone(), issue(5, "design", &["design"])],
            ),
        ];
        assert_eq!(pick(&changes), "planner", "{waiting}");
        let out = String::from_utf8(run_with(&changes).unwrap().stdout).unwrap();
        assert!(
            out.contains(
                "- Design: product due (docs/PRODUCT.md is not on origin/main) but an open design-product issue without ready has a skip label or an open dependency: not picked\n"
            ),
            "{out}"
        );
    }
    // Only the label counts: other issues don't stop the plain v1 pick.
    assert_eq!(
        pick(&[
            missing.clone(),
            plus(
                "issues.json",
                &[issue(921, "PRODUCT.md v1", &["design", "owner-todo"])]
            ),
        ]),
        "design product v1"
    );

    // One product PR at a time: an open agent PR linking an open design-product issue (any
    // labels, any checks) turns both terms off, with a note.
    let slot = |issue_labels: &[&str], pr_json: String| {
        vec![
            missing.clone(),
            plus("issues.json", &[issue(911, "PRODUCT.md v1", issue_labels)]),
            plus("prs.json", &[pr_json]),
        ]
    };
    let open_pr = |body: &str| pr(912, "OPEN", body, "t/911-product", &[], GREEN);
    let doing = slot(&["design-product", "owner-todo"], open_pr("Closes #911"));
    assert_eq!(pick(&doing), "planner");
    let out = String::from_utf8(run_with(&doing).unwrap().stdout).unwrap();
    assert!(
        out.contains(
            "- Design: product due (docs/PRODUCT.md is not on origin/main) but PR #912 is open for it: not picked\n"
        ),
        "{out}"
    );
    let busy = vec![
        plus(
            "issues.json",
            &[issue(911, "p", &["design-product"]), product(913, &[])],
        ),
        plus("prs.json", &[open_pr("Closes #911")]),
    ];
    assert_eq!(pick(&busy), "planner");
    // A ready design-product issue in the current milestone is not a Worker's (step 10), even
    // while the product terms are off.
    let busy_m0 = vec![
        plus(
            "issues.json",
            &[
                issue(911, "p", &["design-product"]),
                ready(913, "M0", &["design-product"], ""),
            ],
        ),
        plus("prs.json", &[open_pr("Closes #911")]),
    ];
    assert_eq!(pick(&busy_m0), "planner");
    let failing = pr(912, "OPEN", "Closes #911", "t/911-product", &[], "null");
    assert_eq!(
        pick(&slot(&["design-product"], failing)),
        "worker own-pr #912"
    );
    for (l, p) in [
        (vec!["design"], open_pr("Closes #911")),
        (vec!["design-product"], open_pr("Closes #914")),
        (vec!["design-product"], open_pr("See #911")),
        (
            vec!["design-product"],
            pr(912, "MERGED", "Closes #911", "t/911-product", &[], "[]"),
        ),
        (
            vec!["design-product"],
            pr(912, "CLOSED", "Closes #911", "t/911-product", &[], "[]"),
        ),
        (
            vec!["design-product"],
            raw_pr(912, "OPEN", "Closes #911", "t/911-product", &[], GREEN),
        ),
        (
            vec![],
            pr(
                912,
                "OPEN",
                "Closes #911",
                "t/911-product",
                &["design-product"],
                GREEN,
            ),
        ),
    ] {
        let want = if l.contains(&"design-product") {
            "design product v1 #911"
        } else {
            "design product v1"
        };
        assert_eq!(pick(&slot(&l, p.clone())), want, "{l:?} {p}");
    }
    // Steps 7 and the Planner come first; the product note still shows when they win.
    let planner_due = ("queue.md", None);
    assert_eq!(pick(&[missing.clone(), planner_due.clone()]), "planner due");
    let mut held = doing.clone();
    held.push(planner_due);
    let out = String::from_utf8(run_with(&held).unwrap().stdout).unwrap();
    assert!(out.starts_with("planner due\n"), "{out}");
    assert!(
        out.contains("but PR #912 is open for it: not picked"),
        "{out}"
    );
}

/// Step 6 (#275): a Design: technical session on an open agent PR labeled `needs-design`, then
/// on an open issue labeled `design` and `ready`.
#[test]
fn step6_needs_design_pr() {
    // PR 320 is a draft with no ai-review success (failing, as an escalated PR is on purpose);
    // it closes issue 321.
    let nd = |n: u64, state: &str, branch: &str, l: &[&str]| {
        pr(n, state, &format!("Closes #{}", n + 1), branch, l, "[]")
    };
    let issue321 = |l: &[&str]| plus("issues.json", &[issue(321, "x", l)]);
    let with = |prs: &[String], l: &[&str]| pick(&[plus("prs.json", prs), issue321(l)]);
    let picked = "design-technical needs-design #320";
    assert_eq!(
        with(&[nd(320, "OPEN", "t/321-a", &["needs-design"])], &[]),
        picked,
        "not step 3's own-pr"
    );
    // Lowest number first, whatever the list order.
    assert_eq!(
        with(
            &[
                nd(330, "OPEN", "t/331-a", &["needs-design"]),
                nd(320, "OPEN", "t/321-a", &["needs-design"])
            ],
            &[]
        ),
        picked
    );
    // A failing PR without the label is step 3's, which comes first.
    assert_eq!(
        with(
            &[
                nd(320, "OPEN", "t/321-a", &["needs-design"]),
                nd(330, "OPEN", "t/331-a", &[])
            ],
            &[]
        ),
        "worker own-pr #330"
    );
    // Near misses that a failing PR without the label would hit: step 3 picks them.
    for label in ["needs-designx", "design", "Needs-design"] {
        assert_eq!(
            with(&[nd(320, "OPEN", "t/321-a", &[label])], &[]),
            "worker own-pr #320",
            "{label}"
        );
    }
    // Near misses: not picked by step 6 (nor by step 3).
    for s in ["owner-todo", "blocked", "deferred"] {
        assert_eq!(
            with(&[nd(320, "OPEN", "t/321-a", &["needs-design", s])], &[]),
            "planner",
            "{s} on the PR"
        );
        assert_eq!(
            with(&[nd(320, "OPEN", "t/321-a", &["needs-design"])], &[s]),
            "planner",
            "{s} on its issue"
        );
    }
    for state in ["MERGED", "CLOSED"] {
        assert_eq!(
            with(&[nd(320, state, "t/321-a", &["needs-design"])], &[]),
            "planner",
            "{state}"
        );
    }
    assert_eq!(
        with(&[nd(320, "OPEN", "owner/x", &["needs-design"])], &[]),
        "planner",
        "the owner's own change"
    );
    assert_eq!(
        pick(&[
            plus(
                "prs.json",
                &[raw_pr(
                    320,
                    "OPEN",
                    "Closes #321",
                    "t/321-a",
                    &["needs-design"],
                    "[]"
                )]
            ),
            issue321(&[]),
        ]),
        "planner",
        "an untagged PR is the owner's"
    );
    assert_eq!(
        pick(&[
            plus("prs.json", &[nd(320, "OPEN", "t/321-a", &["needs-design"])]),
            issue321(&[]),
            holds(&[(320, &[cmd("jacobdam", "/hold", "2026-11-24T01:00:00Z")])]),
        ]),
        "planner",
        "held"
    );
    // Without a Closes link: picked, and the brief has no issue lines.
    let o = run_with(&[plus(
        "prs.json",
        &[pr(
            320,
            "OPEN",
            "Refs #321",
            "t/321-a",
            &["needs-design"],
            "[]",
        )],
    )])
    .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(out.starts_with(&format!("{picked}\n")), "{out}");
    assert!(!out.contains("#321"), "{out}");
    // The brief.
    let o = run_with(&[
        plus("prs.json", &[nd(320, "OPEN", "t/321-a", &["needs-design"])]),
        issue321(&[]),
    ])
    .unwrap();
    assert_eq!(o.status.code(), Some(0));
    let out = String::from_utf8(o.stdout).unwrap();
    let brief = "design-technical needs-design #320\n\n\
        Rule: step 6 (a PR escalated to design, ops/TICK.md §4 step 6: one design output, §8)\n\
        Load:\n- gh pr view 320 --comments\n- gh issue view 321 --comments\n\
        - gh pr list --state all --search '\"Closes #321\" in:body'\n\
        - ops/TICK.md §8\n- specs/tooling/design-technical.md\n- docs/PRINCIPLES.md\n\
        - docs/adr/_TEMPLATE-design.md\n- docs/adr/0010-review-rounds-converge.md\n\
        - docs/adr/0011-session-types-and-scheduler.md §5\nNotes:\n";
    assert!(out.starts_with(brief), "{out}");
}

#[test]
fn step6_needs_design_pr_holds_its_owner_instruction() {
    // Step 1 counts an owner issue closed by, or an owner comment named in, a needs-design PR
    // as in flight (the design session is doing it), as it does for a held PR.
    let url = format!("{ISSUES}40#issuecomment-123");
    let nd =
        |body: &str, l: &[&str]| plus("prs.json", &[pr(320, "OPEN", body, "t/250-a", l, "[]")]);
    let issue = feed_plus(&[feed_issue(250)]);
    let comment = feed_plus(&[feed_comment("comment", &url, FLOOR)]);
    assert_eq!(
        pick(&[issue.clone(), nd("Closes #250", &["needs-design"])]),
        "design-technical needs-design #320"
    );
    assert_eq!(
        pick(&[
            comment.clone(),
            nd(&format!("Owner instruction: {url}"), &["needs-design"])
        ]),
        "design-technical needs-design #320"
    );
    // Near misses: without the label the failing PR doesn't hold them.
    assert_eq!(
        pick(&[issue, nd("Closes #250", &[])]),
        "worker owner-issue #250"
    );
    assert_eq!(
        pick(&[comment, nd(&format!("Owner instruction: {url}"), &[])]),
        format!("worker owner-comment {url}")
    );
}

#[test]
fn step6_ready_design_issue() {
    let d = |n: u64, l: &[&str], m: &str| {
        full_issue(n, "d", l, "", &format!(r#"{{"title":{m:?},"number":5}}"#))
    };
    let one = |l: &[&str]| pick(&[plus("issues.json", &[d(650, l, "M5")])]);
    let picked = "design-technical design-issue #650";
    // Any milestone, and a claimed one (a design session that died) too.
    assert_eq!(one(&["design", "ready"]), picked);
    assert_eq!(one(&["ready", "design", "in-progress"]), picked);
    assert_eq!(
        pick(&[plus(
            "issues.json",
            &[full_issue(650, "d", &["design", "ready"], "", "null")]
        )]),
        picked,
        "no milestone"
    );
    // Lowest number first.
    assert_eq!(
        pick(&[plus(
            "issues.json",
            &[
                d(660, &["design", "ready"], "M0"),
                d(650, &["design", "ready"], "M5")
            ]
        )]),
        picked
    );
    // Near misses.
    assert_eq!(one(&["design"]), "planner", "not ready");
    assert_eq!(one(&["ready"]), "planner", "not design (and not M0)");
    assert_eq!(
        one(&["design-pending", "ready"]),
        "planner",
        "another label"
    );
    for s in ["owner-todo", "blocked", "deferred"] {
        assert_eq!(one(&["design", "ready", s]), "planner", "{s}");
    }
    // In flight: an open agent PR links it, whatever its checks; a merged one doesn't.
    let linked = |state: &str, rollup: &str| {
        pick(&[
            plus("issues.json", &[d(650, &["design", "ready"], "M5")]),
            plus(
                "prs.json",
                &[pr(
                    651,
                    state,
                    "Closes #650",
                    "t/650-a",
                    &["design"],
                    rollup,
                )],
            ),
        ])
    };
    assert_eq!(linked("OPEN", GREEN), "planner", "an open PR links it");
    assert_eq!(linked("MERGED", "[]"), picked, "a merged PR");
    assert_eq!(linked("CLOSED", "[]"), picked, "a closed PR");
    // A needs-design PR comes first; the brief.
    let o = run_with(&[
        plus("issues.json", &[d(650, &["design", "ready"], "M5")]),
        plus(
            "prs.json",
            &[pr(
                670,
                "OPEN",
                "Closes #671",
                "t/671-a",
                &["needs-design"],
                "[]",
            )],
        ),
    ])
    .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(
        out.starts_with("design-technical needs-design #670\n"),
        "{out}"
    );
    let o = run_with(&[plus("issues.json", &[d(650, &["design", "ready"], "M5")])]).unwrap();
    assert_eq!(o.status.code(), Some(0));
    let out = String::from_utf8(o.stdout).unwrap();
    let brief = "design-technical design-issue #650\n\n\
        Rule: step 6 (a ready design issue: an ADR or principle, or one owner question, ops/TICK.md §8)\n\
        Load:\n- gh issue view 650 --comments\n\
        - ops/TICK.md §8\n- specs/tooling/design-technical.md\n- docs/PRINCIPLES.md\n\
        - docs/adr/_TEMPLATE-design.md\n- docs/adr/0010-review-rounds-converge.md\n\
        - docs/adr/0011-session-types-and-scheduler.md §5\nNotes:\n";
    assert!(out.starts_with(brief), "{out}");
}

#[test]
fn step6_needs_design_pr_comes_after_steps_2_to_5() {
    // R1.1: a needs-design PR (320, closing 321) loses to each of steps 2 to 5, and wins over
    // step 7 once they are gone.
    let nd = plus(
        "prs.json",
        &[pr(
            320,
            "OPEN",
            "Closes #321",
            "t/321-a",
            &["needs-design"],
            "[]",
        )],
    );
    let wins = "design-technical needs-design #320";
    assert_eq!(pick(std::slice::from_ref(&nd)), wins);
    let cases: Vec<(&str, (&'static str, Option<String>))> = vec![
        (
            "worker main-red run 20",
            set(
                "runs.json",
                format!("[{}]", run_entry(20, "completed", "failure", "push")),
            ),
        ),
        (
            "worker scheduler-bug #400",
            plus("issues.json", &[issue(400, "next-session: x", &["bug"])]),
        ),
        (
            "worker crash-security #500",
            plus("issues.json", &[issue(500, "x", &["crash"])]),
        ),
        (
            "worker crash-security #510",
            plus("issues.json", &[issue(510, "x", &["security", "sev:high"])]),
        ),
        (
            "worker flaky #600",
            plus("issues.json", &[issue(600, "f", &["flaky"])]),
        ),
        (
            "worker leftover-worktree t/9-z",
            set(
                "worktrees.txt",
                "worktree /w/e\nHEAD f\nbranch refs/heads/t/9-z\n".to_owned(),
            ),
        ),
    ];
    for (want, change) in cases {
        assert_eq!(pick(&[nd.clone(), change]), want);
    }
    // Step 7 comes after it.
    let no_audit = set(
        "main-log.txt",
        [
            commit(103, RETRO_T, "Retro", &[("A", "docs/retros/0001.md")]),
            commit(101, ROOT_T, "Add plan", &[("A", "PLAN.md")]),
        ]
        .concat(),
    );
    assert_eq!(pick(std::slice::from_ref(&no_audit)), "audit due");
    assert_eq!(pick(&[nd, no_audit]), wins);
}

/// Work-mix fixtures (#288): issue #801 is in M0 (`all-issues.json`), and the merged agent PR
/// #802 closes it and changes a product path (`pr-files.json`), so `worker ready #801` and
/// `worker own-pr #802` are milestone work.
fn mix_inputs() -> Vec<(&'static str, Option<String>)> {
    vec![
        plus(
            "prs.json",
            &[pr(802, "MERGED", "Closes #801", "t/801-m", &[], "[]")],
        ),
        set(
            "pr-files.json",
            r#"[{"number":802,"files":[{"path":"ops/TICK.md"},{"path":"crates/jpdf-cos/src/lib.rs"}]}]"#
                .to_owned(),
        ),
        set(
            "all-issues.json",
            r#"[{"number":801,"milestone":{"title":"M0","number":1}},{"number":9,"milestone":null}]"#
                .to_owned(),
        ),
    ]
}

/// Records, oldest first by path: a Planner record on `planner_day` (in the window, never
/// milestone work), then `other` audit records and `milestone` Worker records on #801, all on
/// 2026-11-24. They are written newest first, so the scheduler must order them by path.
fn mix_records(milestone: usize, other: usize, planner_day: &str) -> String {
    let mut lines = vec![(
        format!("ops/journal/{planner_day}/0000-p.md"),
        "- Session: planner".to_owned(),
    )];
    for i in 0..other {
        lines.push((
            format!("ops/journal/2026-11-24/01{i:02}-o.md"),
            "- Session: audit due".to_owned(),
        ));
    }
    for i in 0..milestone {
        lines.push((
            format!("ops/journal/2026-11-24/02{i:02}-m.md"),
            "- Session: worker ready #801".to_owned(),
        ));
    }
    lines.reverse();
    lines.iter().map(|(p, l)| format!("{p}\t{l}\n")).collect()
}

/// `mix_inputs()` plus `changes` (later entries replace earlier ones of the same name).
fn mix_pick(changes: &[(&'static str, Option<String>)]) -> String {
    let mut all = mix_inputs();
    all.extend_from_slice(changes);
    pick(&all)
}

/// The whole output, or a description of the failure (so the caller's assertion shows it).
fn mix_brief(changes: &[(&'static str, Option<String>)]) -> String {
    let mut all = mix_inputs();
    all.extend_from_slice(changes);
    match run_with(&all) {
        Err(e) => format!("io error: {e}"),
        Ok(o) if o.status.code() != Some(0) => format!(
            "exit {:?}: {}",
            o.status.code(),
            String::from_utf8_lossy(&o.stderr)
        ),
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
    }
}

const BREACH_NOTE: &str =
    "- work mix: 13 of the last 20 sessions on milestone work (65%; budget 70%)\n";

#[test]
fn work_mix_70_percent_of_the_last_20() {
    let recs = |m: usize, o: usize| set("records.txt", mix_records(m, o, "2026-11-20"));
    // 14 of 20 (70%) meets the budget; 13 of 20 (65%) is below it, and the Planner is due. With
    // a window of 20, 13 is the nearest count under 70% (69% can't occur).
    assert_eq!(mix_pick(&[recs(14, 5)]), "planner");
    assert_eq!(mix_pick(&[recs(13, 6)]), "planner due");
    assert_eq!(mix_pick(&[recs(20, 0)]), "planner");
    assert_eq!(mix_pick(&[recs(0, 19)]), "planner due");
    // Fewer than 20 records: not measured, never a breach.
    assert_eq!(mix_pick(&[recs(0, 18)]), "planner");
    assert_eq!(mix_pick(&[recs(13, 5)]), "planner");
    // The briefs: the reading is a Planner pick's note, and the breach is a step 8 reason.
    let out = mix_brief(&[recs(13, 6)]);
    assert_eq!(
        out,
        format!(
            "planner due\n\nRule: j (the Planner is due: the work mix is below 70%: 13 of the last 20 sessions on milestone work)\n\
             Load:\n- ops/TICK.md §9\n- ops/QUEUE.md\n- PLAN.md §8\n\
             Notes:\n- worktree on t/298-x: its PRs (#299) are merged or closed; remove it\n{BREACH_NOTE}"
        )
    );
    assert!(
        mix_brief(&[recs(14, 5)]).ends_with(
            "- work mix: 14 of the last 20 sessions on milestone work (70%; budget 70%)\n"
        )
    );
    assert!(
        mix_brief(&[recs(13, 5)])
            .ends_with("- work mix: not measured (19 of the 20 session records it needs)\n")
    );
    // Every term together, in order.
    let out = mix_brief(&[
        set("records.txt", mix_records(13, 7, "2026-11-10")),
        ("queue.md", None),
    ]);
    assert!(
        out.starts_with(
            "planner due\n\nRule: j (the Planner is due: ops/QUEUE.md is missing on origin/main or unreadable; \
             no Session: planner record is dated in the last 7 UTC days; \
             the work mix is below 70%: 13 of the last 20 sessions on milestone work)\n"
        ),
        "{out}"
    );
    // The reading is noted on Planner picks only.
    let ready = plus("issues.json", &[ready(700, "M0", &[], "")]);
    let out = mix_brief(&[recs(14, 5), ready]);
    assert!(out.starts_with("worker ready #700\n"), "{out}");
    assert!(!out.contains("work mix"), "{out}");
    // No current milestone: not measured (and no milestone term).
    let out = mix_brief(&[
        recs(13, 6),
        set("milestones.json", r#"[{"title":"Mx"}]"#.to_owned()),
    ]);
    assert!(
        out.starts_with("planner\n")
            && out.ends_with("- work mix: not measured (no current milestone)\n"),
        "{out}"
    );
    // Once a day: a Planner record dated today turns the term off (idle, no note).
    let out = mix_brief(&[set("records.txt", mix_records(13, 6, "2026-11-24"))]);
    assert!(
        out.starts_with("idle\n") && !out.contains("work mix"),
        "{out}"
    );
}

#[test]
fn work_mix_window_is_the_newest_20_by_path() {
    // The Planner record (2026-11-20), six older milestone records (2026-11-21), then 7 audits
    // and 13 milestone records: the newest 20 hold 13 (below). The 27 together hold 19 milestone
    // records (70%, not below); the oldest 20 would hold 12.
    let older: String = (0..6)
        .map(|i| format!("ops/journal/2026-11-21/03{i:02}-m.md\t- Session: worker ready #801\n"))
        .collect();
    let text = format!("{}{older}", mix_records(13, 7, "2026-11-20"));
    assert_eq!(mix_pick(&[set("records.txt", text.clone())]), "planner due");
    // Line order doesn't matter: the same records oldest first.
    let mut lines: Vec<&str> = text.lines().collect();
    lines.sort_unstable();
    let sorted: String = lines.iter().map(|l| format!("{l}\n")).collect();
    assert_eq!(mix_pick(&[set("records.txt", sorted)]), "planner due");
    // A record on two refs (the same path and line twice) counts once. The Planner record,
    // 14 milestone records, then 5 audits, each twice: counted once, 14 of 20; without it the
    // newest 20 lines would be the last 10 records (5 milestone).
    let mut once: Vec<String> =
        vec!["ops/journal/2026-11-20/0000-p.md\t- Session: planner".to_owned()];
    once.extend(
        (0..14)
            .map(|i| format!("ops/journal/2026-11-24/00{i:02}-m.md\t- Session: worker ready #801")),
    );
    once.extend(
        (0..5).map(|i| format!("ops/journal/2026-11-24/01{i:02}-o.md\t- Session: audit due")),
    );
    let text = |n: usize| -> String {
        once.iter()
            .flat_map(|l| std::iter::repeat_n(l, n))
            .map(|l| format!("{l}\n"))
            .collect()
    };
    assert_eq!(mix_pick(&[set("records.txt", text(1))]), "planner");
    assert_eq!(mix_pick(&[set("records.txt", text(2))]), "planner");
    // A different line in the same file is another record.
    let extra: String = (0..5)
        .map(|i| format!("ops/journal/2026-11-24/01{i:02}-o.md\t- Session: retro due\n"))
        .collect();
    assert_eq!(
        mix_pick(&[set("records.txt", format!("{}{extra}", text(1)))]),
        "planner due"
    );
}

#[test]
fn work_mix_leaves_out_owner_instructions_and_design_sessions() {
    // The Planner record (oldest by path), 14 milestone records, 5 audits, then 5 newer ones of
    // an excluded kind: left out, the window is 14 of 20; in the window, 10 of 20. The owner
    // kinds name #9 (no milestone), so they'd not be milestone work if they were counted (R1.1).
    for kind in [
        "worker owner-issue #9",
        "worker owner-comment https://github.com/jacobdam/jpdf3/issues/1#issuecomment-2",
        "worker owner-done #9",
        "design-technical needs-design #802",
        "design-technical design-issue #801",
        "design-product x #801",
    ] {
        let mut text = String::new();
        for i in 0..14 {
            text.push_str(&format!(
                "ops/journal/2026-11-24/00{i:02}-m.md\t- Session: worker ready #801\n"
            ));
        }
        text.push_str("ops/journal/2026-11-20/0000-p.md\t- Session: planner\n");
        for i in 0..5 {
            text.push_str(&format!(
                "ops/journal/2026-11-24/01{i:02}-o.md\t- Session: audit due\n"
            ));
        }
        let base = text.clone();
        for i in 0..5 {
            text.push_str(&format!(
                "ops/journal/2026-11-24/02{i:02}-x.md\t- Session: {kind}\n"
            ));
        }
        assert_eq!(mix_pick(&[set("records.txt", base)]), "planner", "{kind}");
        assert_eq!(mix_pick(&[set("records.txt", text)]), "planner", "{kind}");
    }
    // Near misses: these kinds are in the window and not milestone work, so 5 of them push 4
    // milestone records and the Planner record out (10 of 20).
    for kind in [
        "worker main-red run 801",
        "worker leftover-worktree t/801-m",
        "Worker ready #801",
        "worker ready #801 x",
        "worker ready 801",
        "worker ready #801x",
        "workers ready #801",
        "worker",
        "design technical #801",
        "audit due",
        "retro due",
        "fallback exit 1",
        "worker owner",
    ] {
        let mut text = String::new();
        for i in 0..14 {
            text.push_str(&format!(
                "ops/journal/2026-11-24/00{i:02}-m.md\t- Session: worker ready #801\n"
            ));
        }
        text.push_str("ops/journal/2026-11-20/0000-p.md\t- Session: planner\n");
        for i in 0..5 {
            text.push_str(&format!(
                "ops/journal/2026-11-24/01{i:02}-o.md\t- Session: audit due\n"
            ));
        }
        for i in 0..5 {
            text.push_str(&format!(
                "ops/journal/2026-11-24/02{i:02}-x.md\t- Session: {kind}\n"
            ));
        }
        assert_eq!(
            mix_pick(&[set("records.txt", text)]),
            "planner due",
            "{kind}"
        );
    }
}

#[test]
fn work_mix_counts_milestone_work_only() {
    // 14 of 20 records name the same work; a change that makes it not milestone work drops the
    // count to 0 (planner due).
    let with = |work: &str, extra: &[(&'static str, Option<String>)]| {
        let text = mix_records(14, 5, "2026-11-20").replace("worker ready #801", work);
        let mut changes = vec![set("records.txt", text)];
        changes.extend_from_slice(extra);
        mix_pick(&changes)
    };
    let prs = |p: String| plus("prs.json", &[p]);
    let files = |list: &str| set("pr-files.json", list.to_owned());
    let issues = |list: &str| set("all-issues.json", list.to_owned());
    assert_eq!(with("worker ready #801", &[]), "planner");
    // Every Worker step on an issue counts, and a QC session on one.
    for w in [
        "worker crash-security #801",
        "worker flaky #801",
        "worker scheduler-bug #801",
        "qc any #801",
    ] {
        assert_eq!(with(w, &[]), "planner", "{w}");
    }
    // `own-pr #802`: the PR changes a product path and links #801, which is in M0.
    assert_eq!(with("worker own-pr #802", &[]), "planner");
    // The PR may be open, merged or closed.
    for state in ["OPEN", "CLOSED"] {
        let p = set(
            "prs.json",
            format!(
                "[{},{}]",
                base_of("prs.json")
                    .trim()
                    .trim_start_matches('[')
                    .trim_end_matches(']'),
                pr(802, state, "Closes #801", "t/801-m", &[], GREEN)
            ),
        );
        assert_eq!(
            with("worker ready #801", std::slice::from_ref(&p)),
            "planner",
            "{state}"
        );
        assert_eq!(with("worker own-pr #802", &[p]), "planner", "{state}");
    }
    // Near misses on the issue: another milestone, no milestone, not listed.
    for list in [
        r#"[{"number":801,"milestone":{"title":"M1"}}]"#,
        r#"[{"number":801,"milestone":{"title":"m0"}}]"#,
        r#"[{"number":801,"milestone":null}]"#,
        r#"[{"number":8010,"milestone":{"title":"M0"}}]"#,
        "[]",
    ] {
        assert_eq!(
            with("worker ready #801", &[issues(list)]),
            "planner due",
            "{list}"
        );
        assert_eq!(
            with("worker own-pr #802", &[issues(list)]),
            "planner due",
            "{list}"
        );
    }
    // Near misses on the PR's files: no product path (paths that only start like one), no
    // entry, an entry for another PR.
    for list in [
        r#"[{"number":802,"files":[{"path":"ops/TICK.md"},{"path":"cratesx/a.rs"},{"path":"tools/jpdf-clix/a"},{"path":"xtask/crates/a.rs"},{"path":"/crates/a.rs"}]}]"#,
        r#"[{"number":802,"files":[]}]"#,
        r#"[{"number":803,"files":[{"path":"crates/a.rs"}]}]"#,
        "[]",
    ] {
        assert_eq!(
            with("worker ready #801", &[files(list)]),
            "planner due",
            "{list}"
        );
        assert_eq!(
            with("worker own-pr #802", &[files(list)]),
            "planner due",
            "{list}"
        );
    }
    // Every product path counts.
    for path in [
        "crates/a.rs",
        "bindings/b",
        "apps/web/c.ts",
        "fuzz/d.rs",
        "tools/jpdf-cli/e.rs",
        "tools/fsio/f.rs",
    ] {
        let list = format!(r#"[{{"number":802,"files":[{{"path":{path:?}}}]}}]"#);
        assert_eq!(
            with("worker ready #801", &[files(&list)]),
            "planner",
            "{path}"
        );
    }
    // Near misses on the link: an untagged PR (the owner's), a PR closing another issue, no PR.
    let base_prs = base_of("prs.json");
    let base_inner = base_prs
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let only = |p: String| set("prs.json", format!("[{base_inner},{p}]"));
    assert_eq!(
        with(
            "worker ready #801",
            &[only(raw_pr(
                802,
                "MERGED",
                "Closes #801",
                "t/801-m",
                &[],
                "[]"
            ))]
        ),
        "planner due"
    );
    assert_eq!(
        with(
            "worker own-pr #802",
            &[only(raw_pr(
                802,
                "MERGED",
                "Closes #801",
                "t/801-m",
                &[],
                "[]"
            ))]
        ),
        "planner due"
    );
    let other = only(pr(802, "MERGED", "Closes #9", "t/9-m", &[], "[]"));
    assert_eq!(
        with("worker ready #801", std::slice::from_ref(&other)),
        "planner due"
    );
    assert_eq!(with("worker own-pr #802", &[other]), "planner due");
    assert_eq!(
        with("worker ready #801", &[set("prs.json", base_of("prs.json"))]),
        "planner due"
    );
    // A second PR for the issue that changes a product path is enough.
    assert_eq!(
        with(
            "worker ready #801",
            &[
                prs(pr(805, "CLOSED", "Closes #801", "t/801-n", &[], "[]")),
                files(r#"[{"number":802,"files":[]},{"number":805,"files":[{"path":"apps/x"}]}]"#)
            ]
        ),
        "planner"
    );
    // `own-pr` names a PR, not an issue: #801 as a PR and #802 as an issue are not work.
    assert_eq!(with("worker own-pr #801", &[]), "planner due");
    assert_eq!(with("worker ready #802", &[]), "planner due");
    // The current milestone moves: M1 work is milestone work once M0 is closed (the plan
    // follows, so step 8's milestone term stays off).
    let m1 = [
        set("milestones.json", r#"[{"title":"M1"}]"#.to_owned()),
        set("queue.md", "Milestone: M1\n".to_owned()),
    ];
    assert_eq!(with("worker ready #801", &m1), "planner due");
    let mut moved = m1.to_vec();
    moved.push(issues(r#"[{"number":801,"milestone":{"title":"M1"}}]"#));
    assert_eq!(with("worker ready #801", &moved), "planner");
}

#[test]
fn step10_product_issues_first_below_the_budget() {
    // A Planner record dated today: step 8 is off, so step 10 picks.
    let below = set("records.txt", mix_records(13, 6, "2026-11-24"));
    let meets = set("records.txt", mix_records(14, 5, "2026-11-24"));
    let ready_list = |extra: &[String]| plus("issues.json", extra);
    let both = ready_list(&[
        ready(701, "M0", &[], ""),
        ready(702, "M0", &["area:process"], ""),
        ready(703, "M0", &["area:engine"], ""),
    ]);
    assert_eq!(
        mix_pick(&[meets.clone(), both.clone()]),
        "worker ready #701"
    );
    let out = mix_brief(&[below.clone(), both]);
    assert_eq!(
        out,
        "worker ready #703\n\nRule: i (the work mix is below 70%: the lowest-numbered ready product issue in the \
         current milestone with its dependencies closed, this one)\nLoad:\n- gh issue view 703 --comments\n\
         Notes:\n- worktree on t/298-x: its PRs (#299) are merged or closed; remove it\n"
    );
    // The lowest product issue among several.
    assert_eq!(
        mix_pick(&[
            below.clone(),
            ready_list(&[
                ready(705, "M0", &["area:engine"], ""),
                ready(701, "M0", &[], ""),
                ready(704, "M0", &["area:engine"], "")
            ])
        ]),
        "worker ready #704"
    );
    // No product issue ready: the lowest ready issue, and the rule says so.
    let out = mix_brief(&[
        below.clone(),
        ready_list(&[
            ready(702, "M0", &[], ""),
            ready(701, "M0", &["area:process"], ""),
        ]),
    ]);
    assert!(
        out.starts_with(
            "worker ready #701\n\nRule: i (the work mix is below 70%: the lowest-numbered ready product issue in the \
             current milestone with its dependencies closed, none, so the lowest-numbered ready issue)\n"
        ),
        "{out}"
    );
    // Near misses: labels that aren't a product area's, and product issues step 10 can't take
    // (each filter applies first).
    for l in [
        "area:records",
        "area:enginex",
        "area:Engine",
        "engine",
        "area: engine",
        "area:",
    ] {
        assert_eq!(
            mix_pick(&[
                below.clone(),
                ready_list(&[ready(701, "M0", &[], ""), ready(702, "M0", &[l], "")])
            ]),
            "worker ready #701",
            "{l}"
        );
    }
    for product in [
        ready(702, "M1", &["area:engine"], ""),
        full_issue(702, "r", &["area:engine"], "", r#"{"title":"M0"}"#),
        ready(702, "M0", &["area:engine", "in-progress"], ""),
        ready(702, "M0", &["area:engine", "deferred"], ""),
        ready(702, "M0", &["area:engine"], "Depends on: #150"),
    ] {
        assert_eq!(
            mix_pick(&[
                below.clone(),
                ready_list(&[ready(701, "M0", &[], ""), product.clone()])
            ]),
            "worker ready #701",
            "{product}"
        );
    }
    // Product areas come from areas.toml: with `engine` a process area and `web` a product
    // one, `area:web` comes first.
    let swapped = AREAS.replace("kind = \"product\"", "kind = \"process\"")
        + "\n[[area]]\nname = \"web\"\nkind = \"product\"\npaths = [\"apps/\"]\n";
    assert_eq!(
        mix_pick(&[
            below,
            set("areas.toml", swapped),
            ready_list(&[
                ready(701, "M0", &["area:engine"], ""),
                ready(702, "M0", &["area:web"], "")
            ])
        ]),
        "worker ready #702"
    );
}

#[test]
fn work_mix_inputs_malformed_exit_1() {
    for (name, text) in [
        ("pr-files.json", "{}"),
        ("pr-files.json", r#"[{"number":1}]"#),
        ("pr-files.json", r#"[{"files":[]}]"#),
        ("pr-files.json", r#"[{"number":1,"files":[{"name":"a"}]}]"#),
        ("pr-files.json", r#"[{"number":1,"files":[{"path":1}]}]"#),
        ("all-issues.json", r#"[{"number":1}]"#),
        ("all-issues.json", r#"[{"milestone":null}]"#),
        (
            "all-issues.json",
            r#"[{"number":1,"milestone":{"title":1}}]"#,
        ),
        ("all-issues.json", r#"[{"number":1,"milestone":"M0"}]"#),
        ("areas.toml", ""),
        ("areas.toml", "[[area]]\nname = \"engine\"\n"),
        // issue-lint's rules: both process areas are required.
        (
            "areas.toml",
            "[[area]]\nname = \"engine\"\nkind = \"product\"\npaths = [\"crates/\"]\n",
        ),
    ] {
        // Parsed whatever is picked: here an owner issue (step 1) comes first.
        for first in [None, Some(feed_plus(&[feed_issue(250)]))] {
            let mut changes = vec![set(name, text.to_owned())];
            changes.extend(first);
            let o = run_with(&changes).unwrap();
            assert_eq!(o.status.code(), Some(1), "{name}: {text}");
            assert!(o.stdout.is_empty(), "{name}: {text}");
        }
    }
}
