//! specs/tooling/nightly.md "Tests" (items 12–15).

use super::*;

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xtask-extra-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn outcome(status: Status, code: Option<i32>) -> Outcome {
    Outcome {
        status,
        code,
        signal: None,
        took: Duration::from_millis(1500),
        stderr: b"bench says <\"x\">".to_vec(),
    }
}

/// `n` jobs whose files exist under `dir` (`<blob>.pdf`), except the indexes in `missing`.
fn jobs(dir: &Path, n: u8, missing: &[u8]) -> Vec<Job> {
    (0..n)
        .map(|i| {
            let blob = format!("{:040x}", i);
            let p = dir.join(format!("{blob}.pdf"));
            std::fs::write(&p, [b'%', i]).unwrap();
            Job {
                blob,
                url: format!("https://example.test/{i}"),
                path: (!missing.contains(&i)).then_some(p),
            }
        })
        .collect()
}

#[test]
fn the_default_budgets_fit_the_ci_job() {
    // .github/workflows/ci.yml project-nightly: `timeout-minutes: 240`. Builds, the fetch and
    // the fuzz build get 30 of them; the rest is the worst case on one core, where the fuzz
    // targets run one after another (fuzz::jobs), plus one run's limit past the regress budget.
    let targets =
        std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../fuzz/fuzz_targets"))
            .unwrap()
            .count();
    let targets = u64::try_from(targets).unwrap();
    assert!(targets >= 1);
    let worst = super::super::DEFAULT_MINUTES
        + 1
        + DEFAULT_BENCH_MINUTES
        + (targets * DEFAULT_FUZZ_SECONDS).div_ceil(60);
    assert!(worst <= 240 - 30, "{worst} minutes for {targets} targets");
    assert_eq!(
        MAX_FUZZ_SECONDS, 3600,
        "cargo xtask fuzz --seconds caps at 3600"
    );
    assert_eq!((DEFAULT_BENCH_MINUTES, DEFAULT_FUZZ_SECONDS), (20, 1200));
}

#[test]
fn the_bench_sample_is_the_first_verified_files() {
    let d = scratch("inputs");
    let js = jobs(&d, 23, &[0, 5]);
    let got = bench_inputs(&js, BENCH_FILES);
    assert_eq!(got.len(), 20);
    // Missing files are skipped, the order is the jobs' order.
    let want: Vec<&Path> = js
        .iter()
        .filter_map(|j| j.path.as_deref())
        .take(20)
        .collect();
    assert_eq!(got, want);
    assert_eq!(
        got.first(),
        js.get(1).and_then(|j| j.path.as_deref()).as_ref()
    );
    // Exactly the cap: 20 verified give 20; fewer give all of them.
    assert_eq!(bench_inputs(&js[..22], 20).len(), 20);
    assert_eq!(bench_inputs(&js[..21], 20).len(), 19);
    assert_eq!(BENCH_FILES, 20);
}

#[test]
fn bench_statuses() {
    let st = |s, c| bench_status(Some(&outcome(s, c)));
    assert_eq!(st(Status::Ok, Some(0)), "ok");
    assert_eq!(st(Status::Timeout, None), "timeout");
    assert_eq!(st(Status::Error, Some(2)), "failed");
    assert_eq!(st(Status::NotImplemented, Some(3)), "failed");
    assert_eq!(st(Status::Crash, Some(1)), "failed");
    assert_eq!(bench_status(None), "failed");
    let r = |status| BenchResult {
        status,
        ..BenchResult::skipped()
    };
    assert!(r("failed").tool_failure());
    for s in ["ok", "timeout", "skipped"] {
        assert!(!r(s).tool_failure(), "{s}");
    }
}

#[test]
fn the_bench_runs_on_a_fresh_copy_of_the_sample() {
    let d = scratch("bench");
    let src = d.join("cache");
    std::fs::create_dir_all(&src).unwrap();
    let js = jobs(&src, 3, &[1]);
    let out = d.join("out");
    let corpus = out.join("bench-corpus");
    std::fs::create_dir_all(&corpus).unwrap();
    std::fs::write(corpus.join("stale.pdf"), b"old").unwrap();
    std::fs::write(out.join("bench.json"), b"an earlier night").unwrap();
    let release = d.join("release");
    let mut calls = Vec::new();
    let r = run_bench(
        &out,
        &js,
        7,
        &mut || Ok(release.clone()),
        &mut |bin, args, limit| {
            // The old report is gone before the bench starts.
            assert!(!out.join("bench.json").exists());
            calls.push((bin.to_path_buf(), args.to_vec(), limit));
            std::fs::write(out.join("bench.json"), b"{}").unwrap();
            Some(outcome(Status::Ok, Some(0)))
        },
    );
    let mut staged: Vec<_> = files_in(&corpus).into_iter().collect();
    staged.sort();
    assert_eq!(
        staged,
        [
            OsString::from(format!("{:040x}.pdf", 0)),
            OsString::from(format!("{:040x}.pdf", 2))
        ]
    );
    assert_eq!(
        std::fs::read(corpus.join(format!("{:040x}.pdf", 2))).unwrap(),
        [b'%', 2]
    );
    let abs = |p: PathBuf| std::path::absolute(p).unwrap();
    assert_eq!(
        calls,
        [(
            release.join("jpdf-bench"),
            bench_argv(
                &release.join("jpdf-cli"),
                &abs(out.join("bench.json")),
                &abs(corpus.clone()),
                2
            ),
            Duration::from_secs(7 * 60)
        )]
    );
    assert_eq!(
        r,
        BenchResult {
            status: "ok",
            files: 2,
            exit: Some(0),
            signal: None,
            ms: Some(1500),
            report: true,
            error: None,
        }
    );
}

#[test]
fn a_bench_that_does_not_finish_leaves_no_report() {
    let d = scratch("bench-fail");
    let js = jobs(&d, 2, &[]);
    let out = d.join("out");
    std::fs::create_dir_all(&out).unwrap();
    for (st, code, want) in [
        (Status::Timeout, None, "timeout"),
        (Status::Crash, Some(1), "failed"),
        (Status::Error, Some(2), "failed"),
    ] {
        std::fs::write(out.join("bench.json"), b"an earlier night").unwrap();
        let r = run_bench(&out, &js, 1, &mut || Ok(d.clone()), &mut |_, _, _| {
            Some(outcome(st, code))
        });
        assert_eq!((r.status, r.report, r.exit), (want, false, code));
        assert!(!out.join("bench.json").exists(), "{want}");
        assert_eq!(r.error.is_some(), want == "failed");
    }
    // An ok exit that wrote no report says so (report false) without being a failure.
    let r = run_bench(&out, &js, 1, &mut || Ok(d.clone()), &mut |_, _, _| {
        Some(outcome(Status::Ok, Some(0)))
    });
    assert_eq!((r.status, r.report), ("ok", false));
    // A spawn failure and a build failure are failures; the build failure starts nothing.
    let r = run_bench(&out, &js, 1, &mut || Ok(d.clone()), &mut |_, _, _| None);
    assert_eq!(r.status, "failed");
    assert!(r.error.unwrap().contains("cannot start"));
    let r = run_bench(
        &out,
        &js,
        1,
        &mut || Err("release build failed (exit status: 101)".into()),
        &mut |_, _, _| panic!("no bench without a build"),
    );
    assert_eq!(
        (r.status, r.files, r.error.as_deref()),
        ("failed", 2, Some("release build failed (exit status: 101)"))
    );
}

#[test]
fn zero_minutes_skips_the_bench() {
    let d = scratch("bench-skip");
    let js = jobs(&d, 1, &[]);
    let out = d.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("bench.json"), b"kept").unwrap();
    let r = run_bench(&out, &js, 0, &mut || panic!("no build"), &mut |_, _, _| {
        panic!("no run")
    });
    assert_eq!(r, BenchResult::skipped());
    // At one minute it runs.
    let r = run_bench(&out, &js, 1, &mut || Ok(d.clone()), &mut |_, _, limit| {
        assert_eq!(limit, Duration::from_secs(60));
        Some(outcome(Status::Ok, Some(0)))
    });
    assert_eq!(r.status, "ok");
}

#[test]
fn only_new_regular_artifacts_are_copied_up_to_the_cap() {
    let d = scratch("artifacts");
    let from = d.join("artifacts");
    std::fs::create_dir_all(&from).unwrap();
    std::fs::write(from.join("crash-old"), b"o").unwrap();
    let before = files_in(&from);
    std::fs::write(from.join("crash-new"), b"n").unwrap();
    std::fs::create_dir_all(from.join("subdir")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(from.join("crash-new"), from.join("link")).unwrap();
    let to = d.join("out/fuzz/t");
    let (got, err) = copy_new_artifacts(&from, &before, &to, "t");
    assert_eq!(err, None);
    assert_eq!(got, ["fuzz/t/crash-new"]);
    assert_eq!(std::fs::read(to.join("crash-new")).unwrap(), b"n");
    assert!(!to.join("crash-old").exists());
    // Nothing new: nothing copied, no directory made.
    let to2 = d.join("out/fuzz/u");
    let all = files_in(&from);
    assert_eq!(copy_new_artifacts(&from, &all, &to2, "u"), (vec![], None));
    assert!(!to2.exists());
    // An absent target directory has no artifacts.
    assert_eq!(
        copy_new_artifacts(&d.join("none"), &before, &to2, "u"),
        (vec![], None)
    );
    // A copy that fails keeps the ones made before it listed, with the error.
    let from3 = d.join("a3");
    std::fs::create_dir_all(&from3).unwrap();
    std::fs::write(from3.join("crash-a"), b"a").unwrap();
    std::fs::write(from3.join("crash-b"), b"b").unwrap();
    let to3 = d.join("out/fuzz/v");
    std::fs::create_dir_all(to3.join("crash-b")).unwrap();
    let (names, err) = copy_new_artifacts(&from3, &BTreeSet::new(), &to3, "v");
    assert_eq!(names, ["fuzz/v/crash-a"]);
    assert!(err.unwrap().contains("cannot copy"));
}

#[test]
fn the_artifact_cap_is_exact() {
    let d = scratch("cap");
    let from = d.join("a");
    std::fs::create_dir_all(&from).unwrap();
    for i in 0..MAX_ARTIFACTS {
        std::fs::write(from.join(format!("crash-{i:03}")), b"x").unwrap();
    }
    let none = BTreeSet::new();
    let (at, _) = copy_new_artifacts(&from, &none, &d.join("at"), "t");
    assert_eq!(at.len(), MAX_ARTIFACTS);
    std::fs::write(from.join("crash-999"), b"x").unwrap();
    let (over, _) = copy_new_artifacts(&from, &none, &d.join("over"), "t");
    assert_eq!(over.len(), MAX_ARTIFACTS);
    assert_eq!(over.last().map(String::as_str), Some("fuzz/t/crash-099"));
    assert!(!d.join("over/crash-999").exists());
    assert_eq!(MAX_ARTIFACTS, 100);
}

#[test]
fn fuzz_statuses() {
    assert_eq!(fuzz_status(false, false), "ok");
    assert_eq!(fuzz_status(false, true), "crash");
    assert_eq!(fuzz_status(true, false), "failed");
    assert_eq!(fuzz_status(true, true), "failed");
    let r = |status| FuzzResult {
        status,
        seconds: 1,
        targets: vec![],
        error: None,
    };
    assert!(r("failed").tool_failure());
    for s in ["ok", "crash", "skipped"] {
        assert!(!r(s).tool_failure(), "{s}");
    }
}

#[test]
fn the_fuzz_pass_needs_the_fuzz_project_and_zero_skips_it() {
    let d = scratch("fuzz");
    let out = d.join("out");
    std::fs::create_dir_all(out.join("fuzz/old")).unwrap();
    let r = run_fuzz(&d, &out, 0, &mut |_| panic!("no pass"));
    assert_eq!((r.status, r.seconds, r.error), ("skipped", 0, None));
    // Skipped: nothing touched.
    assert!(out.join("fuzz/old").exists());
    // No fuzz/Cargo.toml: a tool failure before anything is installed or run, with the
    // artifact directory emptied.
    let r = run_fuzz(&d, &out, 1, &mut |_| panic!("no pass without fuzz/"));
    assert_eq!(r.status, "failed");
    assert!(r.targets.is_empty());
    assert!(r.error.unwrap().contains("Cargo.toml not found"));
    assert!(out.join("fuzz").is_dir());
    assert!(!out.join("fuzz/old").exists());
}

#[test]
fn nightly_json_has_the_spec_shape() {
    let b = BenchResult {
        status: "ok",
        files: 20,
        exit: Some(0),
        signal: None,
        ms: Some(1234),
        report: true,
        error: None,
    };
    let f = FuzzResult {
        status: "crash",
        seconds: 1200,
        targets: vec![
            TargetResult {
                name: "engine_open".into(),
                ok: false,
                artifacts: vec!["fuzz/engine_open/crash-\"a\"".into()],
            },
            TargetResult {
                name: "syntax_resolve".into(),
                ok: true,
                artifacts: vec![],
            },
        ],
        error: None,
    };
    assert_eq!(
        nightly_json(&b, &f, 20),
        "{\"schema\":1,\"regress\":\"regress.json\",\
         \"bench\":{\"status\":\"ok\",\"minutes\":20,\"files\":20,\"exit\":0,\"signal\":null,\
         \"ms\":1234,\"report\":\"bench.json\",\"error\":null},\
         \"fuzz\":{\"status\":\"crash\",\"seconds_per_target\":1200,\"targets\":[\
         {\"name\":\"engine_open\",\"ok\":false,\"artifacts\":[\"fuzz/engine_open/crash-\\\"a\\\"\"]},\
         {\"name\":\"syntax_resolve\",\"ok\":true,\"artifacts\":[]}],\"error\":null}}\n"
    );
    let b = BenchResult::failed(3, "cannot \"copy\"".into());
    let f = FuzzResult {
        status: "skipped",
        seconds: 0,
        targets: vec![],
        error: None,
    };
    let s = nightly_json(&b, &f, 5);
    assert!(s.contains("\"status\":\"failed\",\"minutes\":5,\"files\":3,\"exit\":null"));
    assert!(s.contains("\"report\":null,\"error\":\"cannot \\\"copy\\\"\""));
    assert!(
        s.contains("\"fuzz\":{\"status\":\"skipped\",\"seconds_per_target\":0,\"targets\":[],")
    );
    assert!(jpdf_testgen::json::parse(&s).is_ok());
}

fn no_pass(_: u32) -> Result<fuzz::Report, String> {
    panic!("no fuzz pass")
}

fn no_build() -> Result<PathBuf, String> {
    panic!("no build")
}

#[test]
fn run_writes_the_summary_and_reports_tool_failures() {
    let d = scratch("run");
    let js = jobs(&d, 2, &[]);
    let out = d.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("nightly.json"), b"an earlier night").unwrap();
    let with = |bench: u64, fuzz_s: u64, st: Option<(Status, Option<i32>)>| {
        let mut exec = |_: &Path, _: &[OsString], _| st.map(|(s, c)| outcome(s, c));
        let mut build = || Ok(d.clone());
        let failed = run(
            &d,
            &out,
            &js,
            bench,
            fuzz_s,
            Hooks {
                build: &mut build,
                exec: &mut exec,
                pass: &mut no_pass,
            },
        );
        (
            failed,
            std::fs::read_to_string(out.join("nightly.json")).unwrap(),
        )
    };
    let (failed, s) = with(1, 0, Some((Status::Ok, Some(0))));
    assert!(!failed);
    assert!(s.contains("\"bench\":{\"status\":\"ok\",\"minutes\":1,\"files\":2,"));
    assert!(s.contains("\"fuzz\":{\"status\":\"skipped\""));
    // A bench that ends with exit 1 is a tool failure, recorded too.
    let (failed, s) = with(1, 0, Some((Status::Crash, Some(1))));
    assert!(failed);
    assert!(s.contains("\"bench\":{\"status\":\"failed\""));
    // A timeout is recorded and is not a tool failure.
    let (failed, s) = with(1, 0, Some((Status::Timeout, None)));
    assert!(!failed);
    assert!(s.contains("\"bench\":{\"status\":\"timeout\""));
    // Nothing to do: no failure.
    let mut never = |_: &Path, _: &[OsString], _| -> Option<Outcome> { panic!("no run") };
    let hooks = Hooks {
        build: &mut no_build,
        exec: &mut never,
        pass: &mut no_pass,
    };
    assert!(!run(&d, &out, &js, 0, 0, hooks));
    // A fuzz pass that can't run (no fuzz/ here) is a tool failure.
    let hooks = Hooks {
        build: &mut no_build,
        exec: &mut never,
        pass: &mut no_pass,
    };
    assert!(run(&d, &out, &js, 0, 1, hooks));
    let s = std::fs::read_to_string(out.join("nightly.json")).unwrap();
    assert!(s.contains("\"fuzz\":{\"status\":\"failed\",\"seconds_per_target\":1,"));
}

#[test]
fn the_fuzz_pass_copies_only_this_nights_artifacts_of_started_targets() {
    let d = scratch("fuzz-pass");
    std::fs::create_dir_all(d.join("fuzz")).unwrap();
    std::fs::write(d.join("fuzz/Cargo.toml"), b"").unwrap();
    let art = d.join("fuzz/artifacts");
    std::fs::create_dir_all(art.join("a")).unwrap();
    std::fs::write(art.join("a/crash-old"), b"old").unwrap();
    let out = d.join("out");
    let mut seen = None;
    let r = run_fuzz(&d, &out, 7, &mut |secs| {
        seen = Some(secs);
        // This night's finds, written during the pass (after the snapshot).
        std::fs::write(art.join("a/crash-new"), b"new").unwrap();
        std::fs::create_dir_all(art.join("b")).unwrap();
        std::fs::write(art.join("b/timeout-1"), b"t").unwrap();
        std::fs::create_dir_all(art.join("c")).unwrap();
        std::fs::write(art.join("c/crash-c"), b"c").unwrap();
        Ok(fuzz::Report {
            targets: vec!["a".into(), "b".into(), "c".into()],
            // `c` never started (run_all stopped): it is not listed.
            started: vec!["a".into(), "b".into()],
            failed: vec!["a".into()],
            error: Some("cannot run c: no such file".into()),
        })
    });
    assert_eq!(seen, Some(7));
    assert_eq!(
        r,
        FuzzResult {
            status: "failed",
            seconds: 7,
            targets: vec![
                TargetResult {
                    name: "a".into(),
                    ok: false,
                    artifacts: vec!["fuzz/a/crash-new".into()],
                },
                TargetResult {
                    name: "b".into(),
                    ok: true,
                    artifacts: vec!["fuzz/b/timeout-1".into()],
                },
            ],
            error: Some("cannot run c: no such file".into()),
        }
    );
    assert!(!out.join("fuzz/a/crash-old").exists());
    assert_eq!(std::fs::read(out.join("fuzz/a/crash-new")).unwrap(), b"new");
    assert!(!out.join("fuzz/c").exists());
    // No tool error: a failed target makes it `crash`, none makes it `ok`.
    for (failed, want) in [(vec!["b".to_owned()], "crash"), (vec![], "ok")] {
        let r = run_fuzz(&d, &out, 1, &mut |_| {
            Ok(fuzz::Report {
                targets: vec!["a".into(), "b".into()],
                started: vec!["a".into(), "b".into()],
                failed: failed.clone(),
                error: None,
            })
        });
        assert_eq!((r.status, r.error), (want, None));
        assert_eq!(
            r.targets.iter().map(|t| t.ok).collect::<Vec<_>>(),
            [true, want == "ok"]
        );
    }
    // A pass that fails before any target ran is `failed`, with its message.
    let r = run_fuzz(
        &d,
        &out,
        1,
        &mut |_| Err("fuzz targets do not build".into()),
    );
    assert_eq!(
        (r.status, r.targets.len(), r.error.as_deref()),
        ("failed", 0, Some("fuzz targets do not build"))
    );
}

#[test]
fn a_bench_sample_that_cannot_be_copied_is_a_failure() {
    let d = scratch("bench-copy");
    let out = d.join("out");
    std::fs::create_dir_all(&out).unwrap();
    let js = vec![Job {
        blob: "0".repeat(40),
        url: "https://example.test/0".into(),
        path: Some(d.join("vanished.pdf")),
    }];
    let r = run_bench(&out, &js, 1, &mut no_build, &mut |_, _, _| panic!("no run"));
    assert_eq!(r.status, "failed");
    assert!(r.error.unwrap().contains("cannot copy"));
}

#[test]
fn a_stale_summary_that_cannot_be_removed_is_a_tool_failure() {
    let d = scratch("remove");
    assert_eq!(remove_file(&d.join("absent.json")), Ok(()));
    std::fs::write(d.join("f.json"), b"x").unwrap();
    assert_eq!(remove_file(&d.join("f.json")), Ok(()));
    assert!(!d.join("f.json").exists());
    // A directory in the file's place can't be removed as a file.
    std::fs::create_dir_all(d.join("nightly.json/x")).unwrap();
    assert!(remove_file(&d.join("nightly.json")).is_err());
    let mut never = |_: &Path, _: &[OsString], _| -> Option<Outcome> { panic!("no run") };
    let hooks = Hooks {
        build: &mut no_build,
        exec: &mut never,
        pass: &mut no_pass,
    };
    assert!(run(&d, &d, &[], 0, 0, hooks));
}
