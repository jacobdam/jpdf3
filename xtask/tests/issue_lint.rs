//! `cargo xtask issue-lint --from` on built fixtures (specs/tooling/issue-lint.md). The base
//! fixture is clean; each case adds one issue with one problem, next to a near miss that
//! passes, with every other problem absent.
#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const AREAS: &str = r#"
[[area]]
name = "engine"
kind = "product"
paths = ["crates/jpdf-engine/", "tools/jpdf-cli/main.rs"]

[[area]]
name = "process"
kind = "process"
paths = ["ops/"]

[[area]]
name = "records"
kind = "process"
paths = ["ops/journal/"]
"#;
const TREE: &str =
    "crates/jpdf-engine/src/lib.rs\ntools/jpdf-cli/main.rs\nops/TICK.md\nops/journal/a.md\n";

fn issue(n: u64, labels: &[&str], body: &str) -> String {
    let l: Vec<String> = labels
        .iter()
        .map(|name| format!(r#"{{"id":"LA_x","name":{name:?},"description":"","color":"0E8A16"}}"#))
        .collect();
    format!(
        r#"{{"body":{body:?},"labels":[{}],"number":{n}}}"#,
        l.join(",")
    )
}

fn clean(n: u64) -> String {
    issue(n, &["ready", "area:engine", "origin:owner"], "Scope: x")
}

fn dir_with(areas: &str, tree: &str, issues: &str) -> io::Result<PathBuf> {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "issue-lint-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d)?;
    fs::write(d.join("areas.toml"), areas)?;
    fs::write(d.join("tree.txt"), tree)?;
    fs::write(d.join("issues.json"), issues)?;
    Ok(d)
}

fn run_dir(d: &PathBuf) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["issue-lint", "--from"])
        .arg(d)
        .output()
}

fn lint(issues: &[String]) -> io::Result<Output> {
    run_dir(&dir_with(AREAS, TREE, &format!("[{}]", issues.join(",")))?)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// The base issues plus `extra`: exit 1 and exactly `want` as the problem lines.
fn reports(extra: String, want: &[&str]) -> io::Result<()> {
    let o = lint(&[clean(1), extra, clean(3)])?;
    let out = stdout(&o);
    let problems: Vec<&str> = out
        .lines()
        .filter(|l| !l.starts_with("issue-lint: "))
        .collect();
    assert_eq!(problems, want, "{out}");
    assert_eq!(o.status.code(), Some(1), "{out}");
    let summary = format!("issue-lint: {} problems in 1 of 3 open issues", want.len());
    assert!(out.ends_with(&format!("{summary}\n")), "{out}");
    Ok(())
}

/// The base issues plus `extra`: exit 0, no problem.
fn passes(extra: String) -> io::Result<()> {
    let o = lint(&[clean(1), extra, clean(3)])?;
    let out = stdout(&o);
    assert_eq!(out, "issue-lint: 3 open issues, no problems\n", "{out}");
    assert_eq!(o.status.code(), Some(0));
    Ok(())
}

#[test]
fn base_is_clean() -> io::Result<()> {
    passes(clean(2))?;
    Ok(())
}

#[test]
fn area_label_count() -> io::Result<()> {
    reports(issue(2, &["origin:owner"], ""), &["#2: no area: label"])?;
    reports(
        issue(2, &["area:engine", "area:process", "origin:owner"], ""),
        &["#2: 2 area: labels"],
    )?;
    passes(issue(
        2,
        &["area:records", "origin:owner", "areas", "arena:x"],
        "",
    ))?;
    Ok(())
}

#[test]
fn unknown_area_labels() -> io::Result<()> {
    for bad in [
        "Area:engine",
        "AREA:engine",
        "area: engine",
        "area:",
        "area:Engine",
        "area:ui",
    ] {
        reports(
            issue(2, &[bad, "origin:owner"], ""),
            &[&format!("#2: unknown area label {bad:?}")],
        )?;
    }
    reports(
        issue(2, &["area:engine", "area:x", "origin:owner"], ""),
        &["#2: 2 area: labels", "#2: unknown area label \"area:x\""],
    )?;
    Ok(())
}

#[test]
fn origin_label_count_and_values() -> io::Result<()> {
    reports(issue(2, &["area:engine"], ""), &["#2: no origin: label"])?;
    reports(
        issue(2, &["area:engine", "origin:owner", "origin:audit"], ""),
        &["#2: 2 origin: labels"],
    )?;
    for bad in [
        "Origin:owner",
        "origin:Owner",
        "origin: owner",
        "origin:",
        "origin:bot",
    ] {
        reports(
            issue(2, &["area:engine", bad], ""),
            &[&format!("#2: unknown origin label {bad:?}")],
        )?;
    }
    for good in ["owner", "review", "audit", "qc", "discovery"] {
        passes(issue(2, &["area:engine", &format!("origin:{good}")], ""))?;
    }
    Ok(())
}

#[test]
fn follow_up_needs_its_line() -> io::Result<()> {
    reports(
        issue(2, &["area:engine", "origin:follow-up"], "Scope: x"),
        &["#2: origin:follow-up without a Follow-up of: #N line"],
    )?;
    passes(issue(
        2,
        &["area:engine", "origin:follow-up"],
        "Scope: x\r\n  Follow-up of: #7 \r\nmore",
    ))?;
    Ok(())
}

#[test]
fn malformed_follow_up_lines() -> io::Result<()> {
    for bad in [
        "Follow-up of: #0",
        "Follow-up of: #07",
        "Follow-up of: 7",
        "Follow-up of: #7, #8",
        "follow-up of: #7",
        "FOLLOW-UP OF: #7",
        "Follow-up of: #18446744073709551616",
        "Follow-up of:",
    ] {
        let body = format!("x\n{bad}\n");
        reports(
            issue(2, &["area:engine", "origin:follow-up"], &body),
            &[
                &format!("#2: malformed Follow-up of: line {bad:?}"),
                "#2: origin:follow-up without a Follow-up of: #N line",
            ],
        )?;
    }
    // A line that only mentions it isn't one.
    passes(issue(
        2,
        &["area:engine", "origin:owner"],
        "See the Follow-up of: #7 line\n- Follow-up of: #7",
    ))?;
    Ok(())
}

#[test]
fn control_characters_are_escaped() -> io::Result<()> {
    let o = lint(&[
        clean(1),
        issue(2, &["area:engine\n#9: forged", "origin:owner"], ""),
    ])?;
    let out = stdout(&o);
    assert!(
        out.starts_with("#2: unknown area label \"area:engine\\n#9: forged\"\nissue-lint: "),
        "{out}"
    );
    Ok(())
}

#[test]
fn follow_up_line_on_another_origin() -> io::Result<()> {
    reports(
        issue(2, &["area:engine", "origin:review"], "Follow-up of: #7"),
        &["#2: Follow-up of: line but origin is not follow-up"],
    )?;
    // No known origin: only the origin problem is reported.
    reports(
        issue(2, &["area:engine"], "Follow-up of: #7"),
        &["#2: no origin: label"],
    )?;
    Ok(())
}

#[test]
fn one_parent_and_not_itself() -> io::Result<()> {
    reports(
        issue(
            2,
            &["area:engine", "origin:follow-up"],
            "Follow-up of: #7\nFollow-up of: #8",
        ),
        &["#2: 2 Follow-up of: lines"],
    )?;
    reports(
        issue(2, &["area:engine", "origin:follow-up"], "Follow-up of: #2"),
        &["#2: Follow-up of: itself"],
    )?;
    passes(issue(
        2,
        &["area:engine", "origin:follow-up"],
        "Follow-up of: #1",
    ))?;
    Ok(())
}

#[test]
fn issues_are_reported_in_number_order() -> io::Result<()> {
    let o = lint(&[
        issue(9, &["area:engine"], ""),
        clean(5),
        issue(4, &["origin:qc"], ""),
    ])?;
    assert_eq!(
        stdout(&o),
        "#4: no area: label\n#9: no origin: label\nissue-lint: 2 problems in 2 of 3 open issues\n"
    );
    assert_eq!(o.status.code(), Some(1));
    Ok(())
}

#[test]
fn area_paths_must_match_tracked_files() -> io::Result<()> {
    let tree = "crates/jpdf-engine/src/lib.rs\nops/TICK.md\nops/journal/a.md\n";
    let o = run_dir(&dir_with(AREAS, tree, &format!("[{}]", clean(1)))?)?;
    assert_eq!(
        stdout(&o),
        "ops/areas.toml: path tools/jpdf-cli/main.rs of area engine matches no tracked file\n\
         issue-lint: 1 problems in 0 of 1 open issues\n"
    );
    assert_eq!(o.status.code(), Some(1));
    // A directory path needs a file under it, not a sibling with the same prefix.
    let tree = "crates/jpdf-engine-x/a.rs\ntools/jpdf-cli/main.rs\nops/TICK.md\nops/journal/a.md\n";
    let o = run_dir(&dir_with(AREAS, tree, &format!("[{}]", clean(1)))?)?;
    assert!(stdout(&o).starts_with("ops/areas.toml: path crates/jpdf-engine/ of area engine"));
    assert_eq!(o.status.code(), Some(1));
    Ok(())
}

fn fails_to_run(d: &PathBuf, want: &str) -> io::Result<()> {
    let o = run_dir(d)?;
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(2), "{err}");
    assert!(err.contains(want), "{err} lacks {want}");
    assert!(o.stdout.is_empty());
    Ok(())
}

#[test]
fn nothing_checked_fails() -> io::Result<()> {
    fails_to_run(&dir_with(AREAS, TREE, "[]")?, "open issue list is empty")?;
    fails_to_run(
        &dir_with(AREAS, "\n", &format!("[{}]", clean(1)))?,
        "tracked-file list is empty",
    )?;
    Ok(())
}

#[test]
fn bad_inputs_fail() -> io::Result<()> {
    let one = format!("[{}]", clean(1));
    fails_to_run(&dir_with("", TREE, &one)?, "ops/areas.toml: no [[area]]")?;
    fails_to_run(
        &dir_with(
            &AREAS.replace("kind = \"product\"", "kind = \"x\""),
            TREE,
            &one,
        )?,
        "not product or process",
    )?;
    for (json, want) in [
        ("{}", "not a JSON list"),
        ("[", "gh issue list"),
        (r#"[{"labels":[],"body":""}]"#, "without a number"),
        (
            r#"[{"number":"1","labels":[],"body":""}]"#,
            "without a number",
        ),
        (
            r#"[{"number":-1,"labels":[],"body":""}]"#,
            "bad issue number",
        ),
        (r#"[{"number":1,"body":""}]"#, "no labels list"),
        (
            r#"[{"number":1,"labels":[{"id":"x"}],"body":""}]"#,
            "label without a name",
        ),
        (r#"[{"number":1,"labels":[]}]"#, "no body string"),
        (
            r#"[{"number":1,"labels":[],"body":null}]"#,
            "no body string",
        ),
    ] {
        fails_to_run(&dir_with(AREAS, TREE, json)?, want)?;
    }
    let d = dir_with(AREAS, TREE, &one)?;
    fs::remove_file(d.join("tree.txt")).expect("rm");
    fails_to_run(&d, "tree.txt")?;
    Ok(())
}

#[test]
fn bad_arguments_fail() -> io::Result<()> {
    for args in [&["issue-lint", "--from"][..], &["issue-lint", "--json"][..]] {
        let o = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(args)
            .output()
            .expect("run xtask");
        assert_eq!(o.status.code(), Some(2));
    }
    Ok(())
}

fn many(count: u64) -> String {
    let all: Vec<String> = (1..=count).map(clean).collect();
    format!("[{}]", all.join(","))
}

#[test]
fn list_below_limit_runs() -> io::Result<()> {
    let o = run_dir(&dir_with(AREAS, TREE, &many(999))?)?;
    assert_eq!(stdout(&o), "issue-lint: 999 open issues, no problems\n");
    assert_eq!(o.status.code(), Some(0));
    Ok(())
}

#[test]
fn list_at_limit_fails() -> io::Result<()> {
    fails_to_run(&dir_with(AREAS, TREE, &many(1000))?, "may be cut off")?;
    Ok(())
}
