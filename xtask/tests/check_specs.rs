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
    // The fixture has no apps/web: the web step fails, reports no results, and the spec
    // check still runs.
    assert_eq!(
        std::fs::read_to_string(dir.join("e2e.json"))
            .ok()
            .as_deref(),
        Some("{\"results\": {}}\n")
    );
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("xtask: web: ./apps/web/package.json not found"),
        "{err}"
    );
    // The short fuzz run and then the Arlington check come last; the fixture has neither
    // fuzz/ nor tools/arlington, so both fail fast (before installing or fetching anything)
    // and the exit code stays 1.
    assert!(
        err.ends_with(
            "check-specs: 14 problem(s)\n\
             xtask: fuzz: ./fuzz/Cargo.toml not found (run from the repository root)\n\
             xtask: arlington: ./tools/arlington/Cargo.toml not found (run from the repository root)\n"
        ),
        "{err}"
    );
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
    // `fuzz` usage errors are 2, not the 1 of a crash (specs/tooling/fuzz.md).
    for bad in [
        &["fuzz", "--bogus"][..],
        &["fuzz", "--seconds", "0"],
        &["fuzz", "--target"],
        &["arlington", "--bogus"],
    ] {
        assert_eq!(run(bad).unwrap().status.code(), Some(2), "{bad:?}");
    }
}

#[test]
fn filesystem_and_supersede_edge_cases() {
    let root = std::env::temp_dir().join(format!("xtask-edge-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let specs = root.join("specs");
    std::fs::create_dir_all(specs.join("journeys/nested")).unwrap();
    std::fs::create_dir_all(specs.join("area")).unwrap();
    let good = std::fs::read(format!("{}/specs/area/feat.md", fixture("good"))).unwrap();
    std::fs::write(specs.join("_TEMPLATE.md"), &good).unwrap();
    std::fs::write(specs.join("area/bin.md"), b"## Persistence\n\xff\n").unwrap();
    std::fs::write(specs.join("area/big.md"), vec![b'x'; (1 << 20) + 1]).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(specs.join("_TEMPLATE.md"), specs.join("area/link.md")).unwrap();
    std::fs::write(specs.join("journeys/nested/n.md"), "").unwrap();
    std::fs::create_dir_all(specs.join("area/dir.md")).unwrap();
    std::fs::write(specs.join("area/.md"), "nothing\n").unwrap();
    std::fs::write(
        specs.join("journeys/s.md"),
        "## J-S-1: self\n- milestone: M1\n- platform: web\n- superseded_by: J-S-1\n\n\
         ## J-S-2: a\n- milestone: M1\n- platform: web\n- superseded_by: J-S-3\n\n\
         ## J-S-3: b\n- milestone: M1\n- platform: web\n- superseded_by: J-S-2\n",
    )
    .unwrap();
    std::fs::write(
        specs.join("TRACE.md"),
        "J-S-1 J-S-2 J-S-3 specs/area/bin.md specs/area/big.md\n",
    )
    .unwrap();
    let o = run(&["check-specs", "--root", &root.to_string_lossy()]).unwrap();
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(o.status.code(), Some(1), "{err}");
    let mut want = vec![
        "specs/TRACE.md: feature spec specs/area/.md is not traced",
        "specs/area/.md: missing checklist section `## Behavior` (see specs/_TEMPLATE.md)",
        "specs/area/.md: missing checklist section `## Persistence` (see specs/_TEMPLATE.md)",
        "specs/area/.md: missing checklist section `## Undo` (see specs/_TEMPLATE.md)",
        "specs/area/.md: missing checklist section `## Errors` (see specs/_TEMPLATE.md)",
        "specs/area/.md: missing checklist section `## Limits` (see specs/_TEMPLATE.md)",
        "specs/area/.md: missing checklist section `## Security` (see specs/_TEMPLATE.md)",
        "specs/area/.md: missing checklist section `## Accessibility` (see specs/_TEMPLATE.md)",
        "specs/area/.md: missing checklist section `## Platforms` (see specs/_TEMPLATE.md)",
        "specs/area/.md: missing checklist section `## Performance` (see specs/_TEMPLATE.md)",
        "specs/area/.md: missing checklist section `## Tests` (see specs/_TEMPLATE.md)",
        "specs/area/big.md: file is 1048577 bytes, above the 1048576-byte limit",
        "specs/area/bin.md: not valid UTF-8",
        "specs/area/dir.md: directory named like a Markdown file (the judge would try to read it)",
        "specs/journeys/nested/n.md: journey files must be directly in specs/journeys/ (the judge ignores subdirectories)",
        "specs/journeys/s.md: J-S-1: superseded_by names itself",
        "specs/journeys/s.md: J-S-2: superseded_by chain is a cycle (it must end in a journey with tests)",
        "specs/journeys/s.md: J-S-3: superseded_by chain is a cycle (it must end in a journey with tests)",
    ];
    if cfg!(unix) {
        want.push("specs/area/link.md: symlink not followed; specs must be regular files");
    }
    // The checker sorts by (path, message); no path here contains ": ", so string order matches.
    want.sort_unstable();
    let n = format!("check-specs: {} problem(s)", want.len());
    want.push(&n);
    assert_eq!(err.lines().collect::<Vec<_>>(), want);
}
