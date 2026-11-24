//! specs/tooling/nightly.md "Tests".

use super::*;

fn p(args: &[&str]) -> Result<Options, String> {
    parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xtask-nightly-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn blob(n: u8) -> String {
    format!("{:040x}", n)
}

#[test]
fn parses_flags_and_defaults() {
    assert_eq!(
        p(&[]).unwrap(),
        Options {
            out: PathBuf::from("target/xtask-out"),
            cache: None,
            limit: None,
            previous: None,
            max_minutes: 120,
        }
    );
    let o = p(&[
        "--out",
        "o",
        "--cache",
        "c",
        "--limit",
        "200000",
        "--previous",
        "prev.json",
        "--max-minutes",
        "1440",
    ])
    .unwrap();
    assert_eq!(
        o,
        Options {
            out: PathBuf::from("o"),
            cache: Some(PathBuf::from("c")),
            limit: Some(200_000),
            previous: Some(PathBuf::from("prev.json")),
            max_minutes: 1440,
        }
    );
    assert_eq!(
        p(&["--limit", "1", "--max-minutes", "1"]).unwrap().limit,
        Some(1)
    );
}

#[test]
fn usage_errors() {
    for (args, want) in [
        (&["--limit", "0"][..], "bad --limit"),
        (&["--limit", "200001"][..], "bad --limit"),
        (&["--limit", "+5"][..], "bad --limit"),
        (&["--limit", "-1"][..], "bad --limit"),
        (&["--limit", " 5"][..], "bad --limit"),
        (&["--limit", ""][..], "bad --limit"),
        (&["--max-minutes", "0"][..], "bad --max-minutes"),
        (&["--max-minutes", "1441"][..], "bad --max-minutes"),
        (
            &["--max-minutes", "99999999999999999999999"][..],
            "bad --max-minutes",
        ),
        (&["--limit"][..], "needs a value"),
        (&["--out", "a", "--out", "b"][..], "given twice"),
        (&["--cache", "a", "--cache", "b"][..], "given twice"),
        (&["--previous", "a", "--previous", "b"][..], "given twice"),
        (&["--limit", "2", "--limit", "2"][..], "given twice"),
        (
            &["--max-minutes", "2", "--max-minutes", "2"][..],
            "given twice",
        ),
        (&["--manifest", "x"][..], "unknown argument"),
        (&["--tier", "smoke"][..], "unknown argument"),
        (&["regress", "x"][..], "unknown argument"),
    ] {
        let e = p(args).unwrap_err();
        assert!(e.contains(want), "{args:?}: {e}");
    }
}

#[test]
fn op_arguments_and_limits_follow_the_contract() {
    let (f, w) = (
        Path::new("/c/regress/x.pdf"),
        Path::new("/o/work/render.png"),
    );
    let s = |op: Op| -> Vec<String> {
        op.args(f, w)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    };
    assert_eq!(s(Op::Info), ["info", "/c/regress/x.pdf"]);
    assert_eq!(
        s(Op::Render1),
        [
            "render",
            "/c/regress/x.pdf",
            "--page",
            "1",
            "--dpi",
            "72",
            "--out",
            "/o/work/render.png"
        ]
    );
    assert_eq!(
        s(Op::Render2),
        [
            "render",
            "/c/regress/x.pdf",
            "--page",
            "2",
            "--dpi",
            "72",
            "--out",
            "/o/work/render.png"
        ]
    );
    assert_eq!(s(Op::Text1), ["text", "/c/regress/x.pdf", "--page", "1"]);
    assert_eq!(s(Op::Text2), ["text", "/c/regress/x.pdf", "--page", "2"]);
    let limits: Vec<u64> = OPS.iter().map(|o| o.limit().as_secs()).collect();
    assert_eq!(limits, [20, 30, 30, 60, 60]);
    let ids: Vec<&str> = OPS.iter().map(|o| o.id()).collect();
    assert_eq!(ids, ["info", "render1", "render2", "text1", "text2"]);
    for op in OPS {
        assert_eq!(Op::from_id(op.id()), Some(op));
    }
    assert_eq!(Op::from_id("render"), None);
    assert_eq!(Op::from_id("INFO"), None);
}

#[test]
fn constants_match_the_judge() {
    assert_eq!(MEM_KIB * 1024, 2 * 1024 * 1024 * 1024);
    assert_eq!(MEM_PROBE, format!("ulimit -v {MEM_KIB} && ulimit -v"));
    assert!(MEM_WRAPPER.starts_with(&format!("ulimit -v {MEM_KIB} || exit 125; ")));
    assert_eq!(
        (MAX_ROWS, MAX_MINUTES, DEFAULT_MINUTES),
        (200_000, 1440, 120)
    );
    assert_eq!((STDERR_CAP, MAX_PREVIOUS), (4096, 64 * 1024 * 1024));
}

fn outcome(status: Status, code: Option<i32>) -> Option<Outcome> {
    Some(Outcome {
        status,
        code,
        signal: None,
        took: Duration::from_millis(5),
        stderr: b"e".to_vec(),
    })
}

fn jobs(n: u8, missing: &[u8]) -> Vec<Job> {
    (0..n)
        .map(|i| Job {
            blob: blob(i),
            url: format!("https://raw.githubusercontent.com/o/r/c/{i}.pdf"),
            path: (!missing.contains(&i)).then(|| PathBuf::from(format!("/c/{i}.pdf"))),
        })
        .collect()
}

#[test]
fn every_file_gets_five_runs_and_missing_files_none() {
    let mut calls = Vec::new();
    let (files, complete) = run_files(
        &jobs(3, &[1]),
        Path::new("/nonexistent/render.png"),
        Duration::from_secs(60),
        &mut || Duration::ZERO,
        &mut |op, args| {
            calls.push((op, args.get(1).cloned()));
            outcome(Status::NotImplemented, Some(3))
        },
    );
    assert!(complete);
    assert_eq!(calls.len(), 10, "two present files × five ops");
    assert_eq!(calls[0], (Op::Info, Some("/c/0.pdf".into())));
    assert_eq!(calls[5], (Op::Info, Some("/c/2.pdf".into())));
    assert_eq!(files.len(), 3);
    assert_eq!(files[1].runs, None);
    let runs = files[0].runs.as_ref().unwrap();
    let ops: Vec<Op> = runs.iter().map(|r| r.op).collect();
    assert_eq!(ops, OPS);
    assert!(runs.iter().all(|r| r.status == "not_implemented"
        && r.exit == Some(3)
        && r.ms == Some(5)
        && r.stderr.is_none()));
}

#[test]
fn the_budget_stops_runs_at_exactly_the_limit() {
    // elapsed: below the budget for the first 7 checks, then budget − 1 ns, then the budget.
    let budget = Duration::from_secs(60);
    let mut ticks = 0u32;
    let mut ran = 0u32;
    let (files, complete) = run_files(
        &jobs(2, &[]),
        Path::new("/nonexistent/render.png"),
        budget,
        &mut || {
            ticks += 1;
            match ticks {
                1..=7 => Duration::ZERO,
                8 => budget - Duration::from_nanos(1),
                _ => budget,
            }
        },
        &mut |_, _| {
            ran += 1;
            outcome(Status::Ok, Some(0))
        },
    );
    assert!(!complete);
    assert_eq!(ran, 8, "the run at budget − 1 ns ran");
    assert_eq!(ticks, 9, "no clock read after the budget was reached");
    let st: Vec<&str> = files
        .iter()
        .flat_map(|f| f.runs.as_ref().unwrap())
        .map(|r| r.status)
        .collect();
    assert_eq!(
        st,
        [
            "ok", "ok", "ok", "ok", "ok", "ok", "ok", "ok", "not_run", "not_run"
        ]
    );
    let last = files[1].runs.as_ref().unwrap().last().unwrap().clone();
    assert_eq!((last.exit, last.signal, last.ms), (None, None, None));
    // A zero-length budget runs nothing.
    let (files, complete) = run_files(
        &jobs(1, &[]),
        Path::new("/nonexistent/render.png"),
        Duration::ZERO,
        &mut || Duration::ZERO,
        &mut |_, _| panic!("ran"),
    );
    assert!(!complete);
    assert!(
        files[0]
            .runs
            .as_ref()
            .unwrap()
            .iter()
            .all(|r| r.status == "not_run")
    );
}

#[test]
fn renders_start_without_an_old_png() {
    let d = scratch("png");
    let png = d.join("render.png");
    let mut seen = Vec::new();
    run_files(
        &jobs(1, &[]),
        &png,
        Duration::from_secs(60),
        &mut || Duration::ZERO,
        &mut |op, _| {
            seen.push((op, png.exists()));
            std::fs::write(&png, b"old").unwrap();
            outcome(Status::Ok, Some(0))
        },
    );
    assert_eq!(
        seen,
        [
            (Op::Info, false),
            (Op::Render1, false),
            (Op::Render2, false),
            (Op::Text1, true),
            (Op::Text2, true)
        ]
    );
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn records_keep_stderr_only_for_crashes_and_timeouts() {
    for (st, keep) in [
        (Status::Ok, false),
        (Status::Error, false),
        (Status::NotImplemented, false),
        (Status::Crash, true),
        (Status::Timeout, true),
    ] {
        let r = record(Op::Info, outcome(st, None));
        assert_eq!(r.status, st.id());
        assert_eq!(r.stderr.is_some(), keep, "{st:?}");
    }
    let r = record(
        Op::Text1,
        Some(Outcome {
            status: Status::Crash,
            code: None,
            signal: Some(6),
            took: Duration::from_micros(2999),
            stderr: vec![b'a', 0xff, b'b'],
        }),
    );
    assert_eq!(
        (r.exit, r.signal, r.ms, r.stderr.as_deref()),
        (None, Some(6), Some(2), Some("a\u{fffd}b"))
    );
    let r = record(Op::Info, None);
    assert_eq!(
        (r.status, r.exit, r.signal, r.ms),
        ("crash", None, None, None)
    );
    // Only a run that never started counts as unstarted (a tool failure, spec item 10).
    let mut files = sample_files();
    assert_eq!(unstarted(&files), 0, "crash with an exit, timeout, not_run");
    if let Some(runs) = files[0].runs.as_mut() {
        runs.push(r);
    }
    assert_eq!(unstarted(&files), 1);
}

/// A fake `jpdf-cli` that acts on its first argument (the command) and page.
#[cfg(unix)]
fn fake_cli(dir: &Path) -> PathBuf {
    let script = dir.join("jpdf-cli");
    std::fs::write(
        &script,
        "#!/bin/sh\n\
         case \"$1:$4\" in\n\
         info:) echo '{\"pages\": 1}'; exit 0;;\n\
         render:1) exit 3;;\n\
         render:2) echo 'page 2 of 1' >&2; exit 2;;\n\
         text:1) head -c 5000 /dev/zero | tr '\\0' x >&2; exit 101;;\n\
         text:2) kill -s ABRT $$;;\n\
         ulimit:) ulimit -v; exit 0;;\n\
         esac\n\
         exit 9\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

/// status, exit, signal, stderr length.
type Seen<'a> = (&'a str, Option<i32>, Option<i32>, Option<usize>);

#[cfg(unix)]
#[test]
fn real_runs_are_classified_with_exit_signal_and_stderr() {
    engine::start_watchdog().unwrap();
    let d = scratch("real");
    let cli = fake_cli(&d);
    let mem = memory_limit_applies();
    let file = d.join("x.pdf");
    std::fs::write(&file, b"%PDF").unwrap();
    let job = vec![Job {
        blob: blob(1),
        url: "u".into(),
        path: Some(file),
    }];
    let (files, complete) = run_files(
        &job,
        &d.join("render.png"),
        Duration::from_secs(600),
        &mut || Duration::ZERO,
        &mut |op, args| exec(&cli, mem, args, op.limit()),
    );
    assert!(complete);
    let got: Vec<Seen> = files[0]
        .runs
        .as_ref()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r.status,
                r.exit,
                r.signal,
                r.stderr.as_ref().map(String::len),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("ok", Some(0), None, None),
            ("not_implemented", Some(3), None, None),
            ("error", Some(2), None, None),
            ("crash", Some(101), None, Some(4096)),
            ("crash", None, Some(6), Some(0)),
        ]
    );
    // A hang past the limit is a timeout, with no exit code or signal.
    let o = exec(
        Path::new("/bin/sh"),
        mem,
        &["-c".into(), "exec sleep 10".into()],
        Duration::from_millis(200),
    )
    .unwrap();
    assert_eq!((o.status, o.code, o.signal), (Status::Timeout, None, None));
    std::fs::remove_dir_all(&d).unwrap();
}

#[cfg(unix)]
#[test]
fn the_wrapper_applies_the_judges_memory_limit() {
    // Linux (the judge's OS) must enforce it; elsewhere the probe may refuse.
    let mem = memory_limit_applies();
    if cfg!(target_os = "linux") {
        assert!(mem, "ulimit -v must work on Linux");
    }
    if !mem {
        return;
    }
    engine::start_watchdog().unwrap();
    let d = scratch("mem");
    let cli = fake_cli(&d);
    // The fake prints its own `ulimit -v`; through the wrapper the child must see 2 GiB.
    let out = d.join("seen");
    let script = format!("{} ulimit > {}", cli.display(), out.display());
    let o = exec(
        Path::new("/bin/sh"),
        true,
        &["-c".into(), script.into()],
        Duration::from_secs(20),
    )
    .unwrap();
    assert_eq!(o.status, Status::Ok);
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        format!("{MEM_KIB}\n")
    );
    std::fs::remove_dir_all(&d).unwrap();
}

fn sample_files() -> Vec<FileResult> {
    let run = |op, status, exit, stderr: Option<&str>| Run {
        op,
        status,
        exit,
        signal: None,
        ms: Some(7),
        stderr: stderr.map(str::to_string),
    };
    vec![
        FileResult {
            blob: blob(1),
            url: "https://raw.githubusercontent.com/o/r/c/a \"b\".pdf".into(),
            runs: Some(vec![
                run(Op::Info, "ok", Some(0), None),
                run(Op::Render1, "crash", Some(101), Some("panic\n\u{1}")),
                run(Op::Render2, "error", Some(2), None),
                run(Op::Text1, "timeout", None, Some("")),
                Run {
                    ms: None,
                    ..run(Op::Text2, "not_run", None, None)
                },
            ]),
        },
        FileResult {
            blob: blob(2),
            url: "https://raw.githubusercontent.com/o/r/c/m.pdf".into(),
            runs: None,
        },
    ]
}

#[test]
fn results_json_has_the_spec_shape_and_escapes_text() {
    let text = results_json(&sample_files(), true, 90, false);
    let v = json::parse(&text).unwrap();
    assert_eq!(v.get("schema"), Some(&Value::Number("1".into())));
    assert_eq!(v.get("tier").and_then(Value::as_str), Some("regress"));
    assert_eq!(v.get("rows"), Some(&Value::Number("2".into())));
    assert_eq!(
        v.get("memory_limit_kib"),
        Some(&Value::Number("2097152".into()))
    );
    assert_eq!(v.get("max_minutes"), Some(&Value::Number("90".into())));
    assert_eq!(v.get("complete"), Some(&Value::Bool(false)));
    let counts = v.get("counts").unwrap();
    for (k, n) in [
        ("ok", "1"),
        ("error", "1"),
        ("not_implemented", "0"),
        ("timeout", "1"),
        ("crash", "1"),
        ("not_run", "1"),
        ("missing", "1"),
    ] {
        assert_eq!(counts.get(k), Some(&Value::Number(n.into())), "{k}");
    }
    let files = v.get("files").and_then(Value::as_array).unwrap();
    assert_eq!(
        files[0].get("url").and_then(Value::as_str),
        Some("https://raw.githubusercontent.com/o/r/c/a \"b\".pdf")
    );
    let runs = files[0].get("runs").and_then(Value::as_array).unwrap();
    assert_eq!(
        runs[1].get("stderr").and_then(Value::as_str),
        Some("panic\n\u{1}")
    );
    assert_eq!(runs[1].get("exit"), Some(&Value::Number("101".into())));
    assert_eq!(runs[3].get("exit"), Some(&Value::Null));
    assert_eq!(runs[4].get("ms"), Some(&Value::Null));
    assert_eq!(runs[0].get("stderr"), None);
    assert_eq!(files[1].get("missing"), Some(&Value::Bool(true)));
    assert_eq!(
        files[1]
            .get("runs")
            .and_then(Value::as_array)
            .map(<[Value]>::len),
        Some(0)
    );
    let no_mem = json::parse(&results_json(&[], false, 120, true)).unwrap();
    assert_eq!(no_mem.get("memory_limit_kib"), Some(&Value::Null));
    assert_eq!(no_mem.get("complete"), Some(&Value::Bool(true)));
    // The writer's output is a valid previous file, and reads back the same statuses.
    assert_eq!(
        parse_previous(&text, MAX_ROWS).unwrap(),
        current_statuses(&sample_files())
    );
}

/// A previous results file with the given files array text.
fn prev(files: &str) -> String {
    format!("{{\"schema\":1,\"extra\":true,\"files\":[{files}]}}")
}

fn file(b: &str, missing: bool, runs: &str) -> String {
    format!("{{\"blob\":\"{b}\",\"url\":\"u\",\"missing\":{missing},\"runs\":[{runs}],\"x\":1}}")
}

#[test]
fn the_previous_file_is_checked_strictly() {
    // Letters in the id, so the uppercase case really differs.
    let b1 = blob(0xab);
    let ok_run = "{\"op\":\"info\",\"status\":\"ok\"}";
    // Near misses that pass: extra members, missing:false with runs, a missing file.
    let good = prev(&[file(&b1, false, ok_run), file(&blob(2), true, "")].join(","));
    let m = parse_previous(&good, MAX_ROWS).unwrap();
    assert_eq!(m.len(), 1 + 5);
    assert_eq!(
        m.get(&(b1.clone(), Op::Info)).map(|x| x.0.as_str()),
        Some("ok")
    );
    assert_eq!(
        m.get(&(blob(2), Op::Text2)).map(|x| x.0.as_str()),
        Some("missing")
    );
    for st in STATUSES {
        let r = format!("{{\"op\":\"text2\",\"status\":\"{st}\"}}");
        assert!(
            parse_previous(&prev(&file(&b1, false, &r)), MAX_ROWS).is_ok(),
            "{st}"
        );
    }
    let bad = [
        ("{\"schema\":2,\"files\":[]}".to_string(), "schema"),
        ("{\"schema\":\"1\",\"files\":[]}".to_string(), "schema"),
        ("{\"files\":[]}".to_string(), "schema"),
        ("{\"schema\":1}".to_string(), "no files"),
        ("{\"schema\":1,\"files\":{}}".to_string(), "no files"),
        ("[]".to_string(), "schema"),
        (prev(&file(&b1.to_uppercase(), false, ok_run)), "bad blob"),
        (prev(&file(&b1[1..], false, ok_run)), "bad blob"),
        (prev(&file(&format!("{b1}0"), false, ok_run)), "bad blob"),
        (
            prev(&[file(&b1, false, ok_run), file(&b1, true, "")].join(",")),
            "listed twice",
        ),
        (
            prev(&file(&b1, false, "{\"op\":\"render\",\"status\":\"ok\"}")),
            "unknown op",
        ),
        (prev(&file(&b1, false, "{\"status\":\"ok\"}")), "unknown op"),
        (
            prev(&file(&b1, false, &[ok_run, ok_run].join(","))),
            "op listed twice",
        ),
        (
            prev(&file(
                &b1,
                false,
                "{\"op\":\"info\",\"status\":\"missing\"}",
            )),
            "unknown status",
        ),
        (
            prev(&file(&b1, false, "{\"op\":\"info\",\"status\":\"OK\"}")),
            "unknown status",
        ),
        (
            prev(&file(&b1, false, "{\"op\":\"info\"}")),
            "unknown status",
        ),
        (prev(&file(&b1, true, ok_run)), "missing file has runs"),
        (
            prev(&file(&b1, false, ok_run).replace("\"missing\":false", "\"missing\":0")),
            "not a boolean",
        ),
        (
            prev(&file(&b1, false, ok_run).replace("\"url\":\"u\"", "\"url\":1")),
            "no url",
        ),
        (
            prev(&file(&b1, false, ok_run).replace("\"runs\":[", "\"runz\":[")),
            "no runs",
        ),
        ("{\"schema\":1,\"files\":[]".to_string(), ""),
    ];
    for (text, want) in bad {
        let e = parse_previous(&text, MAX_ROWS).unwrap_err();
        assert!(e.contains(want), "{text}: {e}");
    }
}

#[test]
fn the_previous_file_count_cap_is_exact() {
    let files: Vec<String> = (0..3).map(|i| file(&blob(i), true, "")).collect();
    let text = prev(&files.join(","));
    assert!(parse_previous(&text, 3).is_ok());
    let e = parse_previous(&text, 2).unwrap_err();
    assert!(e.contains("more than 2 files"), "{e}");
}

#[test]
fn reading_the_previous_file_is_bounded() {
    let d = scratch("read");
    let f = d.join("prev.json");
    assert_eq!(read_bounded(&f, 10), Ok(None), "nothing there: no diff");
    std::fs::write(&f, "0123456789").unwrap();
    assert_eq!(
        read_bounded(&f, 10),
        Ok(Some("0123456789".into())),
        "exactly the cap"
    );
    let e = read_bounded(&f, 9).unwrap_err();
    assert!(e.contains("larger than 9"), "{e}");
    std::fs::write(&f, [0xffu8, 0xfe]).unwrap();
    assert!(read_bounded(&f, 10).unwrap_err().contains("UTF-8"));
    assert!(read_bounded(&d, 10).unwrap_err().contains("regular file"));
    std::fs::remove_file(&f).unwrap();
    assert!(
        Command::new("mkfifo").arg(&f).status().unwrap().success(),
        "mkfifo"
    );
    let (tx, rx) = std::sync::mpsc::channel();
    let f2 = f.clone();
    std::thread::spawn(move || {
        let _ = tx.send(read_bounded(&f2, 10));
    });
    let r = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("read_bounded blocked on a FIFO");
    assert!(r.unwrap_err().contains("regular file"));
    std::fs::remove_dir_all(&d).unwrap();
}

fn statuses(rows: &[(u8, Op, &str)]) -> Statuses {
    rows.iter()
        .map(|&(b, op, st)| ((blob(b), op), (st.to_string(), format!("url{b}"))))
        .collect()
}

#[test]
fn the_diff_lists_changes_and_counts_failures_and_fixes() {
    let before = statuses(&[
        (1, Op::Info, "ok"),              // → crash: new failure
        (1, Op::Text1, "crash"),          // → ok: fixed
        (1, Op::Text2, "timeout"),        // → error: fixed
        (2, Op::Info, "not_implemented"), // unchanged: not listed
        (2, Op::Render1, "crash"),        // → timeout: change, neither count
        (2, Op::Render2, "not_run"),      // → crash: new failure
        (3, Op::Info, "crash"),           // → absent (blob gone): change only
        (4, Op::Info, "missing"),         // → ok: change only
        (5, Op::Info, "crash"),           // → not_run: change only (unknown now)
    ]);
    let after = statuses(&[
        (1, Op::Info, "crash"),
        (1, Op::Text1, "ok"),
        (1, Op::Text2, "error"),
        (2, Op::Info, "not_implemented"),
        (2, Op::Render1, "timeout"),
        (2, Op::Render2, "crash"),
        (4, Op::Info, "ok"),
        (5, Op::Info, "not_run"),
        (6, Op::Text2, "timeout"), // absent → timeout: new failure
        (0, Op::Info, "ok"),       // absent → ok: change only, sorts first
    ]);
    let (text, line) = diff_json("prev.json", &before, &after);
    assert_eq!(
        line,
        "nightly: regress diff: 10 changes, 3 new crash/timeout, 2 fixed"
    );
    let v = json::parse(&text).unwrap();
    assert_eq!(v.get("previous").and_then(Value::as_str), Some("prev.json"));
    assert_eq!(v.get("new_failures"), Some(&Value::Number("3".into())));
    assert_eq!(v.get("fixed"), Some(&Value::Number("2".into())));
    let got: Vec<String> = v
        .get("changes")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|c| {
            let s = |k| c.get(k).and_then(Value::as_str).unwrap().to_string();
            format!(
                "{}:{}:{}:{}>{}",
                &s("blob")[38..],
                s("url"),
                s("op"),
                s("before"),
                s("after")
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            "00:url0:info:absent>ok",
            "01:url1:info:ok>crash",
            "01:url1:text1:crash>ok",
            "01:url1:text2:timeout>error",
            "02:url2:render1:crash>timeout",
            "02:url2:render2:not_run>crash",
            "03:url3:info:crash>absent",
            "04:url4:info:missing>ok",
            "05:url5:info:crash>not_run",
            "06:url6:text2:absent>timeout",
        ]
    );
    let (same, line) = diff_json("p", &before, &before);
    assert!(same.contains("\"changes\":[]"), "{same}");
    assert_eq!(
        line,
        "nightly: regress diff: 0 changes, 0 new crash/timeout, 0 fixed"
    );
}

#[test]
fn no_previous_file_means_no_diff_said_so() {
    let d = scratch("diff");
    let (text, line) = diff_for(&load_previous(None).unwrap(), &[]);
    let v = json::parse(&text).unwrap();
    assert_eq!(v.get("previous"), Some(&Value::Null));
    assert!(line.contains("no diff"), "{line}");
    let gone = d.join("gone.json");
    let (text, line) = diff_for(&load_previous(Some(&gone)).unwrap(), &[]);
    assert!(
        json::parse(&text)
            .unwrap()
            .get("note")
            .and_then(Value::as_str)
            .unwrap()
            .contains("no previous results at")
    );
    assert!(line.contains("no diff"), "{line}");
    // A previous file that exists but is invalid fails the diff.
    let bad = d.join("bad.json");
    std::fs::write(&bad, "{\"schema\":2}").unwrap();
    let e = load_previous(Some(&bad)).unwrap_err();
    assert!(
        e.starts_with("previous results ") && e.contains("schema"),
        "{e}"
    );
    // A valid one gives a diff against this night.
    let good = d.join("good.json");
    std::fs::write(&good, results_json(&sample_files(), true, 120, true)).unwrap();
    let (text, line) = diff_for(&load_previous(Some(&good)).unwrap(), &sample_files()[..1]);
    assert_eq!(
        line,
        "nightly: regress diff: 5 changes, 0 new crash/timeout, 0 fixed"
    );
    assert!(
        text.contains("\"before\":\"missing\",\"after\":\"absent\""),
        "{text}"
    );
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn only_verified_files_become_runnable_jobs() {
    let d = scratch("jobs");
    let entry = |bytes: &[u8]| manifest::Entry {
        blob: jpdf_corpus::sha1::git_blob_id(bytes),
        size: u64::try_from(bytes.len()).unwrap(),
        url: "https://raw.githubusercontent.com/o/r/c/x.pdf".into(),
    };
    let (good, stale, absent) = (
        entry(b"%PDF good"),
        entry(b"%PDF stale"),
        entry(b"%PDF gone"),
    );
    std::fs::write(d.join(format!("{}.pdf", good.blob)), b"%PDF good").unwrap();
    // Same size, other bytes: a stale or planted file under the right name.
    std::fs::write(d.join(format!("{}.pdf", stale.blob)), b"%PDF STALE").unwrap();
    let jobs = jobs_for(&[good.clone(), stale, absent], &d);
    let paths: Vec<bool> = jobs.iter().map(|j| j.path.is_some()).collect();
    assert_eq!(paths, [true, false, false]);
    assert_eq!(jobs[0].path, Some(d.join(format!("{}.pdf", good.blob))));
    assert_eq!(
        (jobs[0].blob.as_str(), jobs[0].url.as_str()),
        (good.blob.as_str(), good.url.as_str())
    );
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn exit_codes_follow_the_spec() {
    let none = jobs(2, &[0, 1]);
    let one = jobs(2, &[0]);
    assert_eq!(nothing_to_run(&one, 0), None);
    assert_eq!(
        nothing_to_run(&one, 75),
        None,
        "some files ran: 75 comes at the end"
    );
    assert_eq!(nothing_to_run(&none, 75), Some(Ok(75)));
    for code in [0, 1] {
        let e = nothing_to_run(&none, code).unwrap().unwrap_err();
        assert!(e.contains("no regress file to run (2 rows missing)"), "{e}");
    }
    assert!(nothing_to_run(&[], 0).unwrap().is_err(), "no rows at all");
    assert_eq!(memory_limit_allowed(true, true), Ok(true));
    assert_eq!(memory_limit_allowed(true, false), Ok(true));
    assert_eq!(memory_limit_allowed(false, false), Ok(false));
    assert!(
        memory_limit_allowed(false, true)
            .unwrap_err()
            .contains("not enforced")
    );
    assert_eq!(
        [(false, 0), (false, 1), (false, 75), (true, 0), (true, 75)].map(|(f, c)| exit_code(f, c)),
        [0, 1, 75, 1, 1]
    );
}

/// The wrapper's `exec "$0" "$@"` half on every OS (where `ulimit -v` fails, as on macOS,
/// the wrapper never runs, so this is tested with the limit step swapped for `true`).
#[cfg(unix)]
#[test]
fn the_wrapper_passes_the_program_and_its_arguments_unchanged() {
    let d = scratch("argv");
    let step = format!("ulimit -v {MEM_KIB}");
    assert_eq!(MEM_WRAPPER.matches(&step).count(), 1);
    let script = MEM_WRAPPER.replacen(&step, "true", 1);
    let out = d.join("argv");
    let st = Command::new("/bin/sh")
        .args(["-c", &script, "/bin/sh", "-c"])
        .arg(format!("printf '%s|' \"$0\" \"$@\" > '{}'", out.display()))
        .args(["zero", "a", "b c", "-d", "$HOME", "*"])
        .status()
        .unwrap();
    assert!(st.success());
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "zero|a|b c|-d|$HOME|*|"
    );
    std::fs::remove_dir_all(&d).unwrap();
}

/// R1.1: the previous night is read before this night's `regress.json` replaces it, so a
/// `--previous` naming that same file still diffs against the old night.
#[test]
fn the_previous_night_is_held_before_the_new_results_are_written() {
    let d = scratch("same");
    let f = d.join("regress.json");
    std::fs::write(&f, results_json(&sample_files()[1..], true, 120, true)).unwrap();
    let previous = load_previous(Some(&f)).unwrap();
    std::fs::write(&f, results_json(&sample_files(), true, 120, true)).unwrap();
    let (_, line) = diff_for(&previous, &sample_files());
    assert_eq!(
        line,
        "nightly: regress diff: 5 changes, 2 new crash/timeout, 0 fixed"
    );
    std::fs::remove_dir_all(&d).unwrap();
}
