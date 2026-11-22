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
    // fuzz/ nor tools/arlington, so both fail fast (before installing or writing anything)
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

#[cfg(unix)]
#[test]
fn template_and_trace_must_be_regular_files() {
    // A FIFO would block the open forever and a symlink could point anywhere: both are
    // refused before opening (specs/tooling/check-specs.md, Limits). The run has a
    // deadline, so the old blocking behavior fails instead of hanging the suite.
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("xtask-special-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let specs = root.join("specs");
    std::fs::create_dir_all(specs.join("area")).unwrap();
    let good = format!("{}/specs", fixture("good"));
    std::fs::copy(format!("{good}/area/feat.md"), specs.join("area/feat.md")).unwrap();
    std::fs::copy(format!("{good}/TRACE.md"), root.join("trace-target.md")).unwrap();
    std::os::unix::fs::symlink(root.join("trace-target.md"), specs.join("TRACE.md")).unwrap();
    let made = Command::new("mkfifo")
        .arg(specs.join("_TEMPLATE.md"))
        .arg(specs.join("area/hidden.md"))
        .status()
        .unwrap();
    assert!(made.success(), "mkfifo failed");
    let mut child = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["check-specs", "--root"])
        .arg(&root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > std::time::Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&root);
            panic!("check-specs blocked on a FIFO");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let o = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(o.status.code(), Some(1), "{err}");
    // Exact list: the FIFO template is reported once (the walk and the by-name read agree),
    // the symlinked TRACE.md twice (the link itself, and the file that wasn't read).
    assert_eq!(
        err.lines().collect::<Vec<_>>(),
        [
            "specs/TRACE.md: cannot read: a symlink (not followed)",
            "specs/TRACE.md: symlink not followed; specs must be regular files",
            "specs/_TEMPLATE.md: cannot read: not a regular file",
            "specs/area/hidden.md: cannot read: not a regular file",
            "check-specs: 4 problem(s)",
        ],
        "{err}"
    );
}

#[cfg(unix)]
#[test]
fn ci_linux_installs_with_scripts_off_even_if_the_env_turns_them_on() {
    // specs/ui-web/app-skeleton.md Behavior 5 step 2 (#83): `npm_config_ignore_scripts=false`
    // overrides .npmrc, so the install must pass `--ignore-scripts` itself. A stub npm on
    // PATH records its arguments and fails the install, which ends the web steps there.
    use std::os::unix::fs::PermissionsExt;
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("xtask-npm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(root.join("apps/web")).unwrap();
    std::fs::write(root.join("apps/web/package.json"), "{}\n").unwrap();
    let log = root.join("npm.log");
    for (name, body) in [
        ("node", "echo v24.0.0".to_owned()),
        (
            "npm",
            format!(
                "[ \"$1\" = --version ] && {{ echo 11.0.0; exit 0; }}\necho \"$*\" >> '{}'\nexit 1",
                log.display()
            ),
        ),
    ] {
        let p = bin.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let o = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["ci", "linux", "--out"])
        .arg(root.join("out"))
        .current_dir(&root)
        .env("PATH", path)
        .env("npm_config_ignore_scripts", "false")
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    let logged = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert_eq!(logged, "ci --ignore-scripts\n", "{err}");
    assert!(err.contains("xtask: web: step `install` failed"), "{err}");
}

#[cfg(unix)]
#[test]
fn ci_linux_writes_no_pass_when_playwright_fails() {
    // specs/ui-web/app-skeleton.md Behavior 5 (#76): the reporter may have written `pass`
    // lines, but a Playwright step that exits non-zero must leave no `pass` in e2e.json.
    // A stub npm passes every step; its `playwright test` writes a pass and exits 1.
    use std::os::unix::fs::PermissionsExt;
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("xtask-e2e-fail-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(root.join("apps/web/dist")).unwrap();
    std::fs::write(root.join("apps/web/package.json"), "{}\n").unwrap();
    std::fs::write(root.join("apps/web/dist/index.html"), "i").unwrap();
    // The reported ID must be a journey's (#76 item 4).
    std::fs::create_dir_all(root.join("specs/journeys")).unwrap();
    std::fs::write(
        root.join("specs/journeys/a.md"),
        "## J-A-1: a\n- milestone: M1\n- platform: web\n- test: e2e.web.a\n",
    )
    .unwrap();
    for (name, body) in [
        ("node", "echo v24.0.0"),
        (
            "npm",
            "[ \"$1\" = --version ] && { echo 11.0.0; exit 0; }\n\
             case \"$*\" in *'playwright test'*) \
             printf \"$E2E_LINES\" > \"$JPDF_E2E_RESULTS\"; exit $E2E_EXIT;; esac\nexit 0",
        ),
    ] {
        let p = bin.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let run_lines = |exit: &str, lines: &str| {
        let o = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["ci", "linux", "--out"])
            .arg(root.join("out"))
            .current_dir(&root)
            .env("PATH", &path)
            .env("E2E_EXIT", exit)
            .env("E2E_LINES", lines)
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&o.stderr).into_owned();
        let json = std::fs::read_to_string(root.join("out/e2e.json")).unwrap_or_default();
        (err, json)
    };
    let run_with = |exit: &str| run_lines(exit, "e2e.web.a\\tpass\\n");
    // Control: Playwright exits 0, so the pass stands and the web part is ok.
    let (ok_err, ok_json) = run_with("0");
    // Playwright exits 1 after the reporter wrote a pass.
    let (err, json) = run_with("1");
    // Playwright exits 0, but one reported ID is in no journey (a typo in a tag): no pass.
    let (typo_err, typo_json) = run_lines("0", "e2e.web.a\\tpass\\ne2e.web.b\\tpass\\n");
    // Both at once: the failed step is the summary, the unknown ID is still named.
    let (both_err, _) = run_lines("1", "e2e.web.a\\tpass\\ne2e.web.b\\tpass\\n");
    let _ = std::fs::remove_dir_all(&root);
    let at = |needle: &str| both_err.find(needle);
    let unknown =
        at("xtask: web: test ids that no journey lists and that are not harness ids: e2e.web.b");
    let summary = at("xtask: web: step `e2e` failed");
    assert!(
        unknown.is_some() && summary.is_some() && unknown < summary,
        "{both_err}"
    );
    // The summary is the last web line.
    assert_eq!(both_err.rfind("xtask: web: "), summary, "{both_err}");
    assert!(
        typo_err.contains(
            "xtask: web: test ids that no journey lists and that are not harness ids: e2e.web.b"
        ),
        "{typo_err}"
    );
    assert_eq!(
        typo_json, "{\"results\": {\n  \"e2e.web.a\": \"fail\",\n  \"e2e.web.b\": \"fail\"\n}}\n",
        "{typo_err}"
    );
    assert!(ok_err.contains("xtask: web: ok"), "{ok_err}");
    assert_eq!(
        ok_json, "{\"results\": {\n  \"e2e.web.a\": \"pass\"\n}}\n",
        "{ok_err}"
    );
    assert!(err.contains("xtask: web: step `e2e` failed"), "{err}");
    assert_eq!(
        json, "{\"results\": {\n  \"e2e.web.a\": \"fail\"\n}}\n",
        "{err}"
    );
}
