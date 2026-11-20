//! End-to-end tests of `cargo xtask check-specs` on fixture trees (specs/tooling/check-specs.md).
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .output()
}

fn fixture(name: &str) -> String {
    let p: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    p.to_string_lossy().into_owned()
}

#[test]
fn good_tree_passes() {
    let o = run(&["check-specs", "--root", &fixture("good")]).unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "{err}");
    assert_eq!(err, "check-specs: ok\n");
    assert!(o.stdout.is_empty());
}

#[test]
fn bad_tree_lists_every_problem_and_exits_1() {
    let o = run(&["check-specs", "--root", &fixture("bad")]).unwrap();
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr);
    let want = [
        "specs/TRACE.md: feature spec specs/area/feat.md is not traced",
        "specs/TRACE.md: journey J-RD-03 is not traced",
        "specs/TRACE.md: journey J-RD-04 is not traced",
        "specs/TRACE.md: mentions missing file specs/area/missing.md",
        "specs/TRACE.md: mentions unknown journey J-RD-77",
        "specs/area/feat.md: missing checklist section `## Undo` (see specs/_TEMPLATE.md)",
        "specs/area/feat.md: section `## Limits` is empty",
        // The malformed heading does not start a journey, so its fields land in J-RD-01's
        // body, exactly as the judge would read them.
        "specs/journeys/a.md: J-RD-01: field `milestone` given more than once (the judge reads only the first)",
        "specs/journeys/a.md: J-RD-03: missing `- milestone: M<N>`",
        "specs/journeys/a.md: J-RD-03: missing `- platform: web, macos`",
        "specs/journeys/a.md: J-RD-03: missing `- test: e2e.<platform>.<id>`",
        "specs/journeys/a.md: line 6: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)",
        "specs/journeys/b.md: J-RD-01: duplicate journey ID (also in specs/journeys/a.md)",
        "specs/journeys/b.md: J-RD-04: superseded_by unknown journey J-RD-99",
        "check-specs: 14 problem(s)",
    ];
    let got: Vec<&str> = err.lines().collect();
    assert_eq!(got, want);
}

#[test]
fn ci_linux_runs_the_check_after_writing_e2e() {
    let dir = std::env::temp_dir().join(format!("xtask-ci-{}", std::process::id()));
    let out = dir.to_string_lossy().into_owned();
    let o = match Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["ci", "linux", "--out", &out])
        .current_dir(fixture("bad"))
        .output()
    {
        Ok(o) => o,
        Err(e) => panic!("cannot run xtask: {e}"),
    };
    assert_eq!(o.status.code(), Some(1));
    assert!(dir.join("e2e.json").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn usage_errors_exit_2() {
    assert_eq!(
        run(&["check-specs", "--bogus"]).unwrap().status.code(),
        Some(2)
    );
    assert_eq!(
        run(&["check-specs", "--root"]).unwrap().status.code(),
        Some(2)
    );
    assert_eq!(run(&["frobnicate"]).unwrap().status.code(), Some(2));
}
