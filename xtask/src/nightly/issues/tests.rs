//! The `issues` step (specs/tooling/nightly.md item 16): findings, bodies, deduplication over a
//! fake `gh` that acts on the arguments it gets, the caps, and the real `gh` runner with fake
//! programs.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::super::{FileResult, Op, Previous, Run, Statuses};
use super::*;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccccccccccc";

fn run(op: Op, status: &'static str) -> Run {
    Run {
        op,
        status,
        exit: (status == "crash").then_some(101),
        signal: None,
        ms: Some(7),
        stderr: matches!(status, "crash" | "timeout").then(|| "SECRET-STDERR @someone".into()),
    }
}

/// A file whose five runs have these statuses (in the item 4 order).
fn night_file(blob: &str, st: [&'static str; 5]) -> FileResult {
    let ops = [Op::Info, Op::Render1, Op::Render2, Op::Text1, Op::Text2];
    FileResult {
        blob: blob.into(),
        url: format!("https://example.invalid/{blob}.pdf"),
        runs: Some(ops.iter().zip(st).map(|(&op, s)| run(op, s)).collect()),
    }
}

fn missing(blob: &str) -> FileResult {
    FileResult {
        blob: blob.into(),
        url: String::new(),
        runs: None,
    }
}

fn case(id: &'static str, status: &'static str) -> CaseResult {
    CaseResult {
        id,
        bytes: 524_288_000,
        status,
        exit: Some(1),
        signal: None,
        ms: Some(3),
        stderr: Some("SECRET-STDERR".into()),
    }
}

const OK5: [&str; 5] = [
    "ok",
    "not_implemented",
    "not_implemented",
    "error",
    "not_run",
];

fn sigs(f: &[Finding]) -> Vec<&str> {
    f.iter().map(|f| f.sig.as_str()).collect()
}

#[test]
fn only_failed_cases_crashes_timeouts_and_regressions_are_findings() {
    let limits = [
        case("deep-nesting", "pass"),
        case("big-xref", "crash"),
        case("many-objects", "timeout"),
        case("long-string", "wrong_error"),
        case("huge-stream", "wrong_exit"),
    ];
    let files = [
        night_file(A, ["ok", "crash", "ok", "ok", "timeout"]),
        night_file(B, OK5),
        missing(C),
    ];
    let found = findings(&limits, None, Some(&files));
    assert_eq!(
        sigs(&found),
        [
            "limits:big-xref",
            "limits:many-objects",
            "limits:long-string",
            "limits:huge-stream",
            &format!("regress:{A}"),
        ]
    );
    let labels: Vec<&str> = found.iter().map(|f| f.label).collect();
    assert_eq!(labels, ["crash", "crash", "bug", "bug", "crash"]);
    // One finding per file, naming both failing runs and nothing else.
    let d = &found[4].detail;
    assert!(d.contains("`render1`: `crash`, exit 101"), "{d}");
    assert!(d.contains("`text2`: `timeout`"), "{d}");
    assert!(!d.contains("`info`"), "{d}");
    assert!(
        d.contains(&format!("`https://example.invalid/{A}.pdf`")),
        "{d}"
    );
    // Without limits results or files: nothing.
    assert!(findings(&[], None, None).is_empty());
    assert_eq!(
        sigs(&findings(&limits[..2], None, None)),
        ["limits:big-xref"]
    );
    // A blob that isn't a checked id is never filed (its text would reach a title).
    let odd = night_file("x`y", ["crash", "ok", "ok", "ok", "ok"]);
    assert!(findings(&[], None, Some(&[odd])).is_empty());
}

fn statuses(rows: &[(&str, Op, &str)]) -> Statuses {
    rows.iter()
        .map(|(b, op, st)| ((b.to_string(), *op), (st.to_string(), "u".into())))
        .collect()
}

#[test]
fn a_regression_is_ok_the_night_before_and_error_or_not_implemented_now() {
    let before = statuses(&[
        (A, Op::Info, "ok"),
        (A, Op::Render1, "ok"),
        (A, Op::Render2, "ok"),
        (A, Op::Text1, "error"),
        (A, Op::Text2, "ok"),
        (B, Op::Info, "ok"),
    ]);
    // A: info ok→ok, render1 ok→error, render2 ok→not_implemented, text1 error→not_implemented,
    // text2 ok→crash (a crash finding, not a regression); B missing now; C new.
    let files = [
        night_file(
            A,
            ["ok", "error", "not_implemented", "not_implemented", "crash"],
        ),
        missing(B),
        night_file(C, ["error", "error", "error", "error", "error"]),
    ];
    let prev = Previous::Loaded("p.json".into(), before);
    let found = findings(&[], Some(&prev), Some(&files));
    assert_eq!(
        sigs(&found),
        [
            format!("regress:{A}"),
            format!("regression:{A}:render1"),
            format!("regression:{A}:render2"),
        ]
    );
    assert_eq!(found[1].label, "bug");
    assert!(
        found[2]
            .title
            .contains("render2 on aaaaaaaaaaaa went from ok to not_implemented")
    );
    // No previous night: no regressions.
    let none = Previous::Nothing("no --previous results given".into());
    assert_eq!(findings(&[], Some(&none), Some(&files)).len(), 1);
}

#[test]
fn bodies_start_with_the_tag_carry_the_signature_line_and_no_stderr() {
    let found = findings(
        &[case("big-xref", "crash")],
        None,
        Some(&[night_file(A, ["crash", "ok", "ok", "ok", "ok"])]),
    );
    for f in &found {
        let body = issue_body(f, "2026-11-27 UTC");
        assert!(body.starts_with("🤖 jpdf-agent "), "{body}");
        let lines: Vec<&str> = body.lines().filter(|l| l.starts_with(SIGNATURE)).collect();
        assert_eq!(lines, [format!("{SIGNATURE}{}", f.sig)]);
        assert!(body.contains("First seen: 2026-11-27 UTC."));
        let comment = comment_body(f, "2026-11-28 UTC");
        assert!(comment.starts_with("🤖 jpdf-agent Seen again: 2026-11-28 UTC."));
        for text in [&body, &comment, &f.title] {
            assert!(!text.contains("SECRET-STDERR"), "{text}");
            assert!(!text.contains('@'), "{text}");
        }
        assert!(
            f.sig
                .bytes()
                .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b':' | b'-')),
            "{}",
            f.sig
        );
    }
}

#[test]
fn a_url_that_could_leave_its_backticks_is_not_shown() {
    assert_eq!(
        shown_url("https://x.invalid/a.pdf"),
        "`https://x.invalid/a.pdf`"
    );
    for bad in [
        "",
        "https://x/`a",
        "https://x/a b",
        "https://x/\na",
        "https://x/é",
    ] {
        assert_eq!(
            shown_url(bad),
            "(URL not shown: unexpected characters)",
            "{bad:?}"
        );
    }
}

/// (number, open, labels, body, comments)
type Issue = (u64, bool, Vec<String>, String, Vec<String>);

/// A fake `gh` that acts on its arguments like the real one (lists newest first, as GitHub does) for the three commands the step
/// uses, and refuses anything else.
#[derive(Default)]
struct FakeGh {
    issues: Vec<Issue>,
    calls: Vec<Vec<String>>,
    /// Fail the call with this index (0-based).
    fail_at: Option<usize>,
}

impl FakeGh {
    fn add(&mut self, open: bool, label: &str, body: &str) -> u64 {
        let n = 100 + u64::try_from(self.issues.len()).unwrap();
        self.issues
            .push((n, open, vec![label.into()], body.into(), Vec::new()));
        n
    }

    fn writes(&self) -> usize {
        self.calls.iter().filter(|c| c[1] != "list").count()
    }
}

impl Gh for FakeGh {
    fn run(&mut self, args: &[String]) -> Result<String, String> {
        self.calls.push(args.to_vec());
        if self.fail_at == Some(self.calls.len() - 1) {
            return Err("gh: HTTP 502".into());
        }
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        match a.as_slice() {
            [
                "issue",
                "list",
                "--state",
                "open",
                "--label",
                label,
                "--limit",
                n,
                "--json",
                "number,body",
            ] => {
                let n: usize = n.parse().unwrap();
                let mut s = String::from("[");
                let shown = self
                    .issues
                    .iter()
                    .rev()
                    .filter(|i| i.1 && i.2.iter().any(|l| l == label))
                    .take(n);
                for (k, i) in shown.enumerate() {
                    if k > 0 {
                        s.push(',');
                    }
                    s.push_str(&format!("{{\"number\":{},\"body\":", i.0));
                    jpdf_bench::json::string(&mut s, &i.3);
                    s.push('}');
                }
                s.push(']');
                Ok(s)
            }
            [
                "issue",
                "create",
                "--title",
                _,
                "--label",
                label,
                "--body",
                body,
            ] => {
                let n = self.add(true, label, body);
                Ok(format!("https://github.com/o/r/issues/{n}\n"))
            }
            ["issue", "comment", n, "--body", body] => {
                let n: u64 = n.parse().map_err(|_| "bad number".to_string())?;
                let i = self
                    .issues
                    .iter_mut()
                    .find(|i| i.0 == n)
                    .ok_or("no such issue")?;
                i.4.push((*body).to_string());
                Ok(String::new())
            }
            _ => Err(format!("fake gh: unexpected arguments {a:?}")),
        }
    }
}

fn night1() -> Vec<Finding> {
    findings(
        &[case("big-xref", "wrong_error")],
        None,
        Some(&[night_file(A, ["crash", "ok", "ok", "ok", "ok"])]),
    )
}

#[test]
fn a_repeat_night_comments_on_the_open_issue_instead_of_filing() {
    let mut gh = FakeGh::default();
    let found = night1();
    let d = file(&mut gh, &found, "night 1", MAX_WRITES, LIST_LIMIT).unwrap();
    assert_eq!(
        d,
        Filed {
            created: 2,
            commented: 0,
            skipped: 0
        }
    );
    // One list per label used (sorted), then the creates in finding order.
    // Both labels are always listed (R1.2).
    let heads: Vec<String> = gh
        .calls
        .iter()
        .map(|c| c[..2].join(" ") + " " + &c[5])
        .collect();
    assert_eq!(
        heads,
        [
            "issue list bug",
            "issue list crash",
            "issue create bug",
            "issue create crash"
        ]
    );
    assert_eq!(gh.issues.len(), 2);
    assert!(
        gh.issues[0]
            .3
            .contains(&format!("{SIGNATURE}limits:big-xref\n"))
    );
    // Night 2: the same findings update the two issues; nothing new is filed.
    gh.calls.clear();
    let d = file(&mut gh, &found, "night 2", MAX_WRITES, LIST_LIMIT).unwrap();
    assert_eq!((d.created, d.commented), (0, 2));
    assert_eq!(gh.issues.len(), 2);
    assert!(gh.issues.iter().all(|i| i.4.len() == 1));
    assert!(gh.issues[1].4[0].starts_with("🤖 jpdf-agent Seen again: night 2."));
    assert_eq!(gh.calls[2][..3], ["issue", "comment", "100"]);
}

#[test]
fn only_an_open_tagged_issue_with_the_exact_signature_line_matches() {
    let sig = format!("regress:{A}");
    let mut gh = FakeGh::default();
    // Near misses: closed; untagged; the signature as part of a longer line or a longer sig;
    // a label that is neither `bug` nor `crash`.
    gh.add(false, "crash", &format!("{TAG} x\n{SIGNATURE}{sig}\n"));
    gh.add(true, "crash", &format!("someone\n{SIGNATURE}{sig}\n"));
    gh.add(true, "crash", &format!("{TAG}X\n{SIGNATURE}{sig}\n"));
    gh.add(true, "crash", &format!("{TAG} x\nsee {SIGNATURE}{sig}\n"));
    gh.add(true, "crash", &format!("{TAG} x\n{SIGNATURE}{sig}0\n"));
    gh.add(true, "docs", &format!("{TAG} x\n{SIGNATURE}{sig}\n"));
    let found = findings(
        &[],
        None,
        Some(&[night_file(A, ["crash", "ok", "ok", "ok", "ok"])]),
    );
    let d = file(&mut gh, &found, "n", MAX_WRITES, LIST_LIMIT).unwrap();
    assert_eq!((d.created, d.commented), (1, 0));
    // The match: the lowest open tagged issue with the line wins.
    let mut gh = FakeGh::default();
    gh.add(true, "crash", &format!("{TAG} x\n{SIGNATURE}{sig}\n"));
    gh.add(true, "crash", &format!("{TAG} y\n{SIGNATURE}{sig}\n"));
    let d = file(&mut gh, &found, "n", MAX_WRITES, LIST_LIMIT).unwrap();
    assert_eq!((d.created, d.commented), (0, 1));
    assert_eq!(gh.issues[0].4.len(), 1);
    assert!(gh.issues[1].4.is_empty());
}

#[test]
fn nothing_to_file_calls_nothing() {
    let mut gh = FakeGh::default();
    assert_eq!(
        file(&mut gh, &[], "n", MAX_WRITES, LIST_LIMIT).unwrap(),
        Filed::default()
    );
    assert!(gh.calls.is_empty());
    // Without `--file-issues`, findings or not, `gh` is never called.
    assert_eq!(step(false, &mut gh, &night1(), "n"), Step::Ok);
    assert!(gh.calls.is_empty());
}

fn many(n: usize) -> Vec<FileResult> {
    (0..n)
        .map(|i| night_file(&format!("{i:040x}"), ["crash", "ok", "ok", "ok", "ok"]))
        .collect()
}

#[test]
fn the_write_cap_is_exact() {
    for (n, created, skipped) in [(MAX_WRITES, MAX_WRITES, 0), (MAX_WRITES + 1, MAX_WRITES, 1)] {
        let mut gh = FakeGh::default();
        let found = findings(&[], None, Some(&many(n)));
        let d = file(&mut gh, &found, "n", MAX_WRITES, LIST_LIMIT).unwrap();
        assert_eq!((d.created, d.skipped), (created, skipped), "{n}");
        assert_eq!(gh.writes(), created, "{n}");
    }
    // Comments count against the cap too, but the creates go first (R1.1): 2 known + 1 new
    // with a cap of 2 files the new one and updates one known.
    let mut gh = FakeGh::default();
    let found = findings(&[], None, Some(&many(3)));
    file(&mut gh, &found[..2], "n", MAX_WRITES, LIST_LIMIT).unwrap();
    gh.calls.clear();
    let d = file(&mut gh, &found, "n", 2, LIST_LIMIT).unwrap();
    assert_eq!(
        d,
        Filed {
            created: 1,
            commented: 1,
            skipped: 1
        }
    );
    assert_eq!(gh.issues.len(), 3);
    assert_eq!(MAX_WRITES, 20);
}

/// R1.1: with more findings than the cap every night, each one is filed by the second night.
#[test]
fn findings_over_the_cap_are_filed_on_a_later_night() {
    let mut gh = FakeGh::default();
    let found = findings(&[], None, Some(&many(MAX_WRITES + 1)));
    let d = file(&mut gh, &found, "night 1", MAX_WRITES, LIST_LIMIT).unwrap();
    assert_eq!((d.created, d.commented, d.skipped), (MAX_WRITES, 0, 1));
    let d = file(&mut gh, &found, "night 2", MAX_WRITES, LIST_LIMIT).unwrap();
    assert_eq!((d.created, d.commented, d.skipped), (1, MAX_WRITES - 1, 1));
    assert_eq!(gh.issues.len(), MAX_WRITES + 1);
    let last = &found[MAX_WRITES].sig;
    assert!(
        gh.issues
            .iter()
            .any(|i| i.3.contains(&format!("{SIGNATURE}{last}\n")))
    );
}

/// R1.2: a limits case that changes label keeps its one issue.
#[test]
fn a_finding_that_changes_label_keeps_its_issue() {
    let mut gh = FakeGh::default();
    let wrong = findings(&[case("xref-entries", "wrong_exit")], None, None);
    let crash = findings(&[case("xref-entries", "crash")], None, None);
    assert_eq!((wrong[0].label, crash[0].label), ("bug", "crash"));
    file(&mut gh, &wrong, "night 1", MAX_WRITES, LIST_LIMIT).unwrap();
    let d = file(&mut gh, &crash, "night 2", MAX_WRITES, LIST_LIMIT).unwrap();
    assert_eq!((d.created, d.commented), (0, 1));
    assert_eq!(gh.issues.len(), 1);
}

#[test]
fn a_list_that_reaches_its_limit_is_refused() {
    let found = findings(&[], None, Some(&many(1)));
    for (open, ok) in [(2, true), (3, false)] {
        let mut gh = FakeGh::default();
        for _ in 0..open {
            gh.add(true, "crash", &format!("{TAG} other"));
        }
        let r = file(&mut gh, &found, "n", MAX_WRITES, 3);
        assert_eq!(r.is_ok(), ok, "{open} open");
        if !ok {
            let (e, d) = r.unwrap_err();
            assert!(e.contains("cannot deduplicate"), "{e}");
            assert_eq!(d, Filed::default());
            assert_eq!(gh.writes(), 0);
        }
    }
    assert_eq!(LIST_LIMIT, 1000);
}

#[test]
fn a_gh_failure_stops_the_step_and_says_what_was_done() {
    // The list fails: no writes.
    let mut gh = FakeGh {
        fail_at: Some(0),
        ..FakeGh::default()
    };
    let o = step(true, &mut gh, &night1(), "n");
    assert_eq!(
        o,
        Step::Failed("issues: gh: HTTP 502 (0 filed, 0 updated before it)".into())
    );
    assert_eq!(gh.calls.len(), 1);
    // The second create fails: one filed, nothing after it.
    let mut gh = FakeGh {
        fail_at: Some(3),
        ..FakeGh::default()
    };
    let o = step(true, &mut gh, &night1(), "n");
    assert_eq!(
        o,
        Step::Failed("issues: gh: HTTP 502 (1 filed, 0 updated before it)".into())
    );
    assert_eq!(gh.calls.len(), 4);
    // Bad list output is a failure too.
    struct Junk;
    impl Gh for Junk {
        fn run(&mut self, _: &[String]) -> Result<String, String> {
            Ok("{\"not\":\"a list\"}".into())
        }
    }
    let o = step(true, &mut Junk, &night1(), "n");
    assert!(
        matches!(&o, Step::Failed(e) if e.contains("not a JSON array")),
        "{o:?}"
    );
    // The success case files and is Ok.
    let mut gh = FakeGh::default();
    assert_eq!(step(true, &mut gh, &night1(), "n"), Step::Ok);
    assert_eq!(gh.writes(), 2);
}

#[test]
fn the_night_label_is_a_utc_date_and_a_checked_run_id() {
    // 2026-11-27 23:59:59 UTC and one second later.
    let t = 1_795_823_999;
    assert_eq!(night_label(t, None), "2026-11-27 UTC");
    assert_eq!(night_label(t + 1, None), "2026-11-28 UTC");
    assert_eq!(night_label(t, Some("123")), "2026-11-27 UTC (CI run 123)");
    for bad in ["", "12a", "1 2", "@x", "123456789012345678901"] {
        assert_eq!(night_label(t, Some(bad)), "2026-11-27 UTC", "{bad:?}");
    }
    assert_eq!(
        night_label(t, Some("12345678901234567890")),
        "2026-11-27 UTC (CI run 12345678901234567890)"
    );
}

fn scratch(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/scratch/issues-tests")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn script(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let p = dir.join("gh");
    std::fs::write(
        &p,
        format!("#!/bin/sh\n[ -n \"$WARM\" ] && exit 0\n{body}\n"),
    )
    .unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    // A first exec of a fresh script can be slow on macOS (ops/LESSONS.md): warm it up.
    let _ = std::process::Command::new(&p).env("WARM", "1").output();
    p
}

fn real(program: PathBuf, secs: u64) -> RealGh {
    RealGh {
        program,
        limit: Duration::from_secs(secs),
        stdout_cap: GH_STDOUT_CAP,
    }
}

#[test]
fn the_real_runner_passes_arguments_and_reads_stdout() {
    let d = scratch("args");
    // Prints each argument on its own line, plus the no-prompt setting.
    let p = script(
        &d,
        r#"for a in "$@"; do printf '%s\n' "$a"; done; echo "P=$GH_PROMPT_DISABLED""#,
    );
    let args = s(&["issue", "create", "--body", "a b\n$(x) `y` 'z'"]);
    let out = real(p, 30).run(&args).unwrap();
    assert_eq!(out, "issue\ncreate\n--body\na b\n$(x) `y` 'z'\nP=1\n");
}

#[test]
fn the_real_runner_fails_on_exit_codes_hangs_and_missing_programs() {
    let d = scratch("fail");
    let p = script(&d, "echo partial; exit 1");
    let e = real(p, 30).run(&s(&["issue", "list"])).unwrap_err();
    assert!(e.starts_with("gh issue list: "), "{e}");
    let p = script(&d, "exec sleep 30");
    let start = Instant::now();
    let e = real(p, 1).run(&s(&["issue", "list"])).unwrap_err();
    assert!(e.contains("no answer in 1 s"), "{e}");
    assert!(start.elapsed() < Duration::from_secs(10));
    let e = real(d.join("absent"), 30).run(&[]).unwrap_err();
    assert!(e.starts_with("cannot start "), "{e}");
    let p = script(&d, "printf '\\377'");
    let e = real(p, 30).run(&[]).unwrap_err();
    assert!(e.contains("not UTF-8"), "{e}");
}

/// R1.3: stdout of exactly the cap is read; one byte more fails (the cap passed in small, the
/// real value pinned).
#[test]
fn the_stdout_cap_is_exact() {
    let d = scratch("cap");
    let p = script(&d, "head -c 1000 /dev/zero | tr '\\0' x");
    for (cap, ok) in [(1000, true), (999, false)] {
        let mut r = real(p.clone(), 30);
        r.stdout_cap = cap;
        let out = r.run(&[]);
        assert_eq!(out.is_ok(), ok, "cap {cap}: {out:?}");
        if ok {
            assert_eq!(out.unwrap().len(), 1000);
        } else {
            assert!(out.unwrap_err().contains("more than 999 bytes"));
        }
    }
    assert_eq!(GH_STDOUT_CAP, 64 * 1024 * 1024);
}

/// R1.5: a list whose items lack a number or a body, or whose number isn't a u64, fails.
#[test]
fn a_list_of_the_wrong_shape_fails() {
    let mut map = BTreeMap::new();
    for (text, want) in [
        ("[{\"body\":\"x\"}]", "an issue without a number"),
        (
            "[{\"number\":\"7\",\"body\":\"x\"}]",
            "an issue without a number",
        ),
        (
            "[{\"number\":-1,\"body\":\"x\"}]",
            "an issue without a number",
        ),
        (
            "[{\"number\":1.5,\"body\":\"x\"}]",
            "an issue without a number",
        ),
        ("[{\"number\":7}]", "an issue without a body"),
        ("[{\"number\":7,\"body\":null}]", "an issue without a body"),
        ("[1]", "an issue without a number"),
        ("not json", "gh issue list --label crash: "),
    ] {
        let e = index(text, "crash", LIST_LIMIT, &mut map).unwrap_err();
        assert!(e.contains(want), "{text}: {e}");
    }
    // The near miss: the right shape is read.
    index(
        "[{\"number\":7,\"body\":\"x\"}]",
        "crash",
        LIST_LIMIT,
        &mut map,
    )
    .unwrap();
}
