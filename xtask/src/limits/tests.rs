//! specs/tooling/limits.md "Tests".

use super::*;
use jpdf_engine::{Document, Limits};

fn p(args: &[&str]) -> Result<Options, String> {
    parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xtask-limits-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn outcome(status: Status, code: Option<i32>, stderr: &str) -> Outcome {
    Outcome {
        status,
        code,
        signal: None,
        took: Duration::from_millis(7),
        stderr: stderr.as_bytes().to_vec(),
    }
}

/// `jpdf-cli info FILE` in-process: the engine on the file, the CLI's exit and stderr shape.
fn engine_exec(f: &Path) -> Option<Outcome> {
    let bytes = std::fs::read(f).unwrap();
    Some(match Document::open(&bytes, &Limits::default()) {
        Ok(_) => outcome(Status::Ok, Some(0), ""),
        Err(e) => outcome(Status::Error, Some(2), &format!("jpdf-cli: {e}\n")),
    })
}

#[test]
fn parses_flags_and_defaults() {
    assert_eq!(
        p(&[]).unwrap(),
        Options {
            out: PathBuf::from("target/xtask-out"),
            mib: 500,
        }
    );
    assert_eq!(
        p(&["--out", "o", "--mib", "64"]).unwrap(),
        Options {
            out: PathBuf::from("o"),
            mib: 64,
        }
    );
    assert_eq!(p(&["--mib", "511"]).unwrap().mib, 511);
}

#[test]
fn usage_errors() {
    for bad in [
        &["--mib", "63"][..],
        &["--mib", "512"],
        &["--mib", "+100"],
        &["--mib", ""],
        &["--mib"],
        &["--out", "a", "--out", "b"],
        &["--mib", "100", "--mib", "100"],
        &["--limit", "3"],
        &["extra"],
    ] {
        assert!(p(bad).is_err(), "{bad:?}");
    }
}

/// At the smallest target every class's file is what it claims: the engine (the code `jpdf-cli
/// info` runs) refuses each hostile file with the case's own error and opens each valid one.
#[test]
fn every_case_ends_as_expected_on_the_engine() {
    let d = scratch("engine");
    let file = d.join("limits.pdf");
    let results = run_cases(MIN_MIB * MIB, &file, &mut engine_exec).unwrap();
    let got: Vec<(&str, &str)> = results.iter().map(|r| (r.id, r.status)).collect();
    let want: Vec<(&str, &str)> = CASES.iter().map(|c| (c.id, "pass")).collect();
    assert_eq!(got, want, "{results:?}");
    assert!(!file.exists(), "the last case's file is removed");
    std::fs::remove_dir_all(&d).unwrap();
}

/// The valid classes open with every page found, not just exit 0 by luck.
#[test]
fn the_valid_classes_have_their_page_counts() {
    let pages = |case: &str, target: u64| {
        let c = CASES.iter().find(|c| c.id == case).unwrap();
        let mut v = Vec::new();
        (c.generate)(&mut Out::new(&mut v), target).unwrap();
        Document::open(&v, &Limits::default()).map(|d| d.page_count())
    };
    assert_eq!(pages("long-comment", 4 * MIB), Ok(1));
    assert_eq!(pages("dense-kids", 4 * MIB), Ok(32_768));
    assert_eq!(pages("max-per-object", 4 * MIB), Ok(2));
}

/// The counts the classes are built from, at the default size: each stays under the limit
/// that would otherwise fire first, or (xref-entries) far over the one it targets.
#[test]
fn default_size_parameters_sit_where_the_spec_says() {
    let t = DEFAULT_MIB * MIB;
    let mut sink = io::sink();
    let mut o = Out::new(&mut sink);
    xref_entries(&mut o, t).unwrap();
    assert!(o.pos <= t && o.pos > t - 200, "{}", o.pos);
    let entries = (t - 200) / 20;
    // The issue's "10 M entries", and so far over the 2^21-entry limit.
    assert!(entries > 10_000_000, "{entries}");
    // dense-kids: exactly the 1 000 000-page limit, and under the 512 MiB file limit.
    let mut sink = io::sink();
    let mut o = Out::new(&mut sink);
    dense_kids(&mut o, t).unwrap();
    assert!(o.pos <= t && o.pos >= t / 2, "{}", o.pos);
    assert_eq!(dense_pages(t), 1_000_000);
    // nested-strings: the first kid's string (every later header and close) under 32 MiB.
    assert_eq!(nested_pages(t), 600_000);
    let header = len(format!("{:07} 0 obj << /Type /Page /S (", 3).as_bytes());
    assert!(nested_pages(t) * (header + len(b") >> endobj ")) < 32 * MIB);
    assert_eq!(shared_kids(t), 100_000);
    assert_eq!(STACKED_SECTIONS, 1000);
    // max-per-object: just under the string and per-parse object limits, 13 pairs.
    assert_eq!(
        (MAX_STRING, MAX_ENTRIES),
        (32 * MIB - 4096, (1 << 19) - 1024)
    );
    const { assert!(2 * MAX_ENTRIES + 8 < 1 << 20) };
    assert_eq!(max_pairs(t), 13);
}

/// R1.1 (PR #349): the max-per-object pair count reaches half the target and stays under
/// 512 MiB at every `--mib` (the payloads plus at most 4 KiB per pair of headers and xref).
#[test]
fn max_per_object_fits_at_every_size() {
    let pair = pair_bytes();
    for mib in MIN_MIB..=MAX_MIB {
        let t = mib * MIB;
        let pairs = max_pairs(t);
        assert!(pairs * pair >= t / 2, "{mib} MiB");
        assert!(pairs * (pair + 4096) <= 512 * MIB - 4096, "{mib} MiB");
    }
}

#[test]
fn every_case_reaches_its_size_at_the_bounds() {
    for mib in [MIN_MIB, 75, 76, 79, DEFAULT_MIB, MAX_MIB] {
        let t = mib * MIB;
        for c in CASES {
            let mut sink = io::sink();
            let mut o = Out::new(&mut sink);
            (c.generate)(&mut o, t).unwrap();
            assert!(size_ok(o.pos, t), "{} at {mib} MiB: {} bytes", c.id, o.pos);
        }
    }
}

#[test]
fn the_size_check_is_exact_at_both_ends() {
    let t = 100 * MIB;
    assert!(size_ok(t / 2, t));
    assert!(!size_ok(t / 2 - 1, t));
    assert!(size_ok(512 * MIB, t));
    assert!(!size_ok(512 * MIB + 1, t));
}

#[test]
fn verdicts_follow_the_spec() {
    let refused = Expect::Refused("limit exceeded: cross-reference entries");
    let msg = "jpdf-cli: limit exceeded: cross-reference entries\n";
    let cases = [
        (Expect::Opens, outcome(Status::Ok, Some(0), ""), "pass"),
        (refused, outcome(Status::Error, Some(2), msg), "pass"),
        (
            refused,
            outcome(Status::Error, Some(2), "jpdf-cli: limit exceeded: pages\n"),
            "wrong_error",
        ),
        // The text must follow the CLI's prefix, not just appear somewhere.
        (
            refused,
            outcome(
                Status::Error,
                Some(2),
                "limit exceeded: cross-reference entries\n",
            ),
            "wrong_error",
        ),
        (refused, outcome(Status::Ok, Some(0), ""), "wrong_exit"),
        (
            refused,
            outcome(Status::NotImplemented, Some(3), msg),
            "wrong_exit",
        ),
        (
            Expect::Opens,
            outcome(Status::Error, Some(2), msg),
            "wrong_exit",
        ),
        (refused, outcome(Status::Crash, Some(101), msg), "crash"),
        (Expect::Opens, outcome(Status::Crash, None, ""), "crash"),
        (refused, outcome(Status::Timeout, None, msg), "timeout"),
        (Expect::Opens, outcome(Status::Timeout, None, ""), "timeout"),
    ];
    for (expect, o, want) in cases {
        assert_eq!(verdict(expect, Some(&o)), want, "{expect:?} {o:?}");
    }
    assert_eq!(verdict(Expect::Opens, None), "crash");
}

#[test]
fn each_file_is_fresh_and_removed_after_its_run() {
    let d = scratch("fresh");
    let file = d.join("limits.pdf");
    let mut seen = Vec::new();
    let results = run_cases(MIN_MIB * MIB, &file, &mut |f| {
        seen.push(std::fs::metadata(f).unwrap().len());
        None
    })
    .unwrap();
    assert!(!file.exists());
    let sizes: Vec<u64> = results.iter().map(|r| r.bytes).collect();
    assert_eq!(seen, sizes);
    // A run that couldn't start is a crash, with the reason kept.
    for r in &results {
        assert_eq!(
            (r.status, r.exit, r.ms, r.stderr.as_deref()),
            ("crash", None, None, Some("cannot start jpdf-cli"))
        );
    }
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn an_unwritable_file_is_a_tool_failure() {
    let d = scratch("unwritable");
    let e = run_cases(MIN_MIB * MIB, &d.join("no/such/dir/x.pdf"), &mut |_| None).unwrap_err();
    assert!(e.starts_with("case nested-strings: cannot write"), "{e}");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn results_json_has_the_spec_shape() {
    let results = [
        CaseResult {
            id: "a",
            bytes: 10,
            status: "pass",
            exit: Some(2),
            signal: None,
            ms: Some(5),
            stderr: None,
        },
        CaseResult {
            id: "b",
            bytes: 11,
            status: "crash",
            exit: None,
            signal: Some(6),
            ms: Some(9),
            stderr: Some("boom \"x\"\n".into()),
        },
    ];
    assert_eq!(
        results_json(&results, 500, true),
        "{\"schema\":1,\"target_bytes\":500,\"memory_limit_kib\":2097152,\"time_limit_ms\":20000,\"failed\":1,\"cases\":[\
         {\"id\":\"a\",\"bytes\":10,\"status\":\"pass\",\"exit\":2,\"signal\":null,\"ms\":5},\
         {\"id\":\"b\",\"bytes\":11,\"status\":\"crash\",\"exit\":null,\"signal\":6,\"ms\":9,\"stderr\":\"boom \\\"x\\\"\\n\"}]}\n"
    );
    assert!(results_json(&[], 1, false).contains("\"memory_limit_kib\":null,"));
}

/// End to end through the memory wrapper with a fake `jpdf-cli` that refuses every file as
/// overlapping: the two overlap cases pass, the others fail, and `limits.json` says so.
#[cfg(unix)]
#[test]
fn run_with_a_fake_cli_writes_the_results() {
    jpdf_bench::engine::start_watchdog().unwrap();
    let d = scratch("fake");
    let cli = d.join("jpdf-cli");
    std::fs::write(
        &cli,
        "#!/bin/sh\n\
         [ \"$1\" = info ] && [ -f \"$2\" ] || exit 9\n\
         echo 'jpdf-cli: malformed PDF: indirect objects overlap at byte 1' >&2\n\
         exit 2\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mem = crate::nightly::memory_limit_applies();
    let out = d.join("out");
    assert_eq!(run_with(&cli, mem, &out, MIN_MIB * MIB), Ok(false));
    let text = std::fs::read_to_string(out.join("limits.json")).unwrap();
    let v = jpdf_testgen::json::parse(&text).unwrap();
    assert_eq!(
        v.get("failed"),
        Some(&jpdf_testgen::json::Value::Number("5".into()))
    );
    let statuses: Vec<&str> = v
        .get("cases")
        .and_then(|c| c.as_array())
        .unwrap()
        .iter()
        .map(|c| c.get("status").and_then(|s| s.as_str()).unwrap())
        .collect();
    assert_eq!(
        statuses,
        [
            "pass",
            "pass",
            "wrong_error",
            "wrong_error",
            "wrong_exit",
            "wrong_exit",
            "wrong_exit"
        ]
    );
    assert!(!out.join("work/limits.pdf").exists());
    std::fs::remove_dir_all(&d).unwrap();
}
