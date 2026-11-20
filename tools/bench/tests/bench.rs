//! End-to-end tests of jpdf-bench with fake engines (shell scripts). Spec: specs/tooling/bench.md.
#![cfg(unix)]

use std::ffi::OsString;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use jpdf_bench::bench::corpus_inputs;
use jpdf_bench::engine::{Status, run_once};

/// The `try_*` helpers, unwrapped at the call site (inside the test function).
macro_rules! temp_dir {
    () => {
        try_temp_dir().unwrap()
    };
}
macro_rules! script {
    ($($a:tt)*) => {
        try_script($($a)*).unwrap()
    };
}
macro_rules! bench {
    ($($a:tt)*) => {
        try_bench($($a)*).unwrap()
    };
}
macro_rules! py {
    ($($a:tt)*) => {
        try_py($($a)*).unwrap()
    };
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn try_temp_dir() -> io::Result<PathBuf> {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("bench-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

fn try_script(dir: &Path, name: &str, body: &str) -> io::Result<PathBuf> {
    let p = dir.join(name);
    std::fs::write(&p, format!("#!/bin/sh\n{body}\n"))?;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755))?;
    Ok(p)
}

/// Behaves like the M0 `jpdf-cli` stub: `info` succeeds, everything else is not implemented.
const STUB_CLI: &str = r#"case "$1" in info) exit 0 ;; *) exit 3 ;; esac"#;

fn try_bench(args: &[&OsString]) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_jpdf-bench"))
        .args(args)
        .output()
}

fn os(s: impl Into<OsString>) -> OsString {
    s.into()
}

/// Checks that `text` is valid JSON and returns `expr` evaluated on it by Python's strict
/// parser (`r` is the parsed report).
fn try_py(text: &str, expr: &str) -> io::Result<String> {
    let f = try_temp_dir()?.join("r.json");
    std::fs::write(&f, text)?;
    let out = Command::new("python3")
        .args(["-I", "-c"])
        .arg(format!(
            "import json,sys\nr=json.load(open(sys.argv[1],encoding='utf-8'))\nprint(json.dumps({expr}))"
        ))
        .arg(&f)
        .output()?;
    if !out.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[test]
fn stub_cli_on_generated_sample() {
    let d = temp_dir!();
    let cli = script!(&d, "cli", STUB_CLI);
    let out = d.join("out/bench.json");
    let o = bench!(&[
        &os("--cli"),
        &os(&cli),
        &os("--out"),
        &os(&out),
        &os("--mutool"),
        &os("none"),
        &os("--pdfium"),
        &os("none"),
        &os("--runs"),
        &os("2"),
        &os("--warmup"),
        &os("0"),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = std::fs::read_to_string(&out).unwrap();
    assert_eq!(py!(&text, "r['schema']"), r#""jpdf-bench/1""#);
    assert_eq!(
        py!(&text, "r['config']"),
        r#"{"sample": "generated", "files": 4, "runs": 2, "warmup": 0}"#
    );
    assert_eq!(
        py!(&text, "[f['name'] for f in r['files']]"),
        r#"["gen-pages-0001.pdf", "gen-pages-0010.pdf", "gen-pages-0100.pdf", "gen-pages-1000.pdf"]"#
    );
    // The generated files are real PDFs in the default work dir.
    assert!(
        std::fs::read(d.join("out/bench.json.work/gen-pages-0010.pdf"))
            .unwrap()
            .starts_with(b"%PDF-")
    );
    assert_eq!(
        py!(&text, "r['engines']"),
        r#"{"jpdf": {"present": true}, "mutool": {"present": false, "version": null}, "pdfium": {"present": false, "version": null}}"#
    );
    // info ran: 4 ok files with real (positive) times.
    let info = py!(&text, "r['operations']['info']['engines']['jpdf']");
    assert!(
        info.starts_with(r#"{"ok": 4, "error": 0, "not_implemented": 0, "timeout": 0, "crash": 0"#),
        "{info}"
    );
    assert_eq!(
        py!(
            &text,
            "r['operations']['info']['engines']['jpdf']['median_ms'] > 0"
        ),
        "true"
    );
    assert_eq!(
        py!(
            &text,
            "all(f['results']['info']['jpdf']['ms'] > 0 for f in r['files'])"
        ),
        "true"
    );
    // Not implemented is never a time, never zero, and never in a ratio.
    for op in ["render72", "render150", "text"] {
        assert_eq!(
            py!(&text, &format!("r['operations']['{op}']")),
            r#"{"engines": {"jpdf": {"ok": 0, "error": 0, "not_implemented": 4, "timeout": 0, "crash": 0, "median_ms": null, "p95_ms": null}}, "ratio": {"mutool": {"value": null, "samples": 0}, "pdfium": {"value": null, "samples": 0}}}"#,
            "{op}"
        );
        assert_eq!(
            py!(
                &text,
                &format!("[f['results']['{op}'] for f in r['files']][0]")
            ),
            r#"{"jpdf": {"status": "not_implemented", "ms": null}}"#
        );
    }
    // Machine id: 16 hex digits; no absolute paths anywhere in the report.
    assert_eq!(py!(&text, "len(r['machine']['id'])"), "16");
    assert!(
        !text.contains(d.to_str().unwrap()),
        "report leaks a local path"
    );
}

#[test]
fn reference_ratio_only_where_both_ok() {
    let d = temp_dir!();
    let cli = script!(&d, "cli", STUB_CLI);
    // Fake mutool: prints a version for -v and succeeds at everything.
    let mutool = script!(
        &d,
        "mutool",
        r#"[ "$1" = "-v" ] && echo "mutool version 9.9.9" >&2; exit 0"#,
    );
    let out = d.join("r.json");
    let o = bench!(&[
        &os("--cli"),
        &os(&cli),
        &os("--out"),
        &os(&out),
        &os("--mutool"),
        &os(&mutool),
        &os("--pdfium"),
        &os("none"),
        &os("--runs"),
        &os("1"),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = std::fs::read_to_string(&out).unwrap();
    assert_eq!(
        py!(&text, "r['engines']['mutool']"),
        r#"{"present": true, "version": "mutool version 9.9.9"}"#
    );
    assert_eq!(
        py!(
            &text,
            "r['operations']['info']['ratio']['mutool']['samples']"
        ),
        "4"
    );
    assert_eq!(
        py!(
            &text,
            "r['operations']['info']['ratio']['mutool']['value'] > 0"
        ),
        "true"
    );
    assert_eq!(
        py!(
            &text,
            "r['operations']['render72']['engines']['mutool']['ok']"
        ),
        "4"
    );
    assert_eq!(
        py!(&text, "r['operations']['render72']['ratio']['mutool']"),
        r#"{"value": null, "samples": 0}"#
    );
}

#[test]
fn exit_codes_of_the_bench() {
    let d = temp_dir!();
    let cli = script!(&d, "cli", STUB_CLI);
    let out = d.join("r.json");
    // Usage errors → 2.
    assert_eq!(bench!(&[&os("--cli"), &os(&cli)]).status.code(), Some(2));
    assert_eq!(
        bench!(&[
            &os("--cli"),
            &os(&cli),
            &os("--out"),
            &os(&out),
            &os("--runs"),
            &os("0")
        ])
        .status
        .code(),
        Some(2)
    );
    assert_eq!(
        bench!(&[
            &os("--cli"),
            &os(&cli),
            &os("--out"),
            &os(&out),
            &os("--mutool"),
            &os(d.join("no-such-mutool"))
        ])
        .status
        .code(),
        Some(2)
    );
    // A missing --cli or an unreadable corpus → 1.
    assert_eq!(
        bench!(&[
            &os("--cli"),
            &os(d.join("missing")),
            &os("--out"),
            &os(&out)
        ])
        .status
        .code(),
        Some(1)
    );
    assert_eq!(
        bench!(&[
            &os("--cli"),
            &os(&cli),
            &os("--out"),
            &os(&out),
            &os("--mutool"),
            &os("none"),
            &os("--pdfium"),
            &os("none"),
            &os("--corpus"),
            &os(d.join("no-such-dir"))
        ])
        .status
        .code(),
        Some(1)
    );
    assert!(!out.exists());
}

#[test]
fn real_process_classification() {
    let d = temp_dir!();
    let limit = Duration::from_secs(20);
    for (body, want) in [
        ("exit 0", Status::Ok),
        ("exit 2", Status::Error),
        ("exit 3", Status::NotImplemented),
        ("exit 1", Status::Crash),
        ("exit 101", Status::Crash),
        ("kill -9 $$", Status::Crash),
        ("kill -SEGV $$", Status::Crash),
    ] {
        let s = script!(
            &d,
            &format!("e{}", NEXT.fetch_add(1, Ordering::Relaxed)),
            body,
        );
        assert_eq!(run_once(&s, &[], limit).0, want, "{body}");
    }
    assert_eq!(
        run_once(&d.join("does-not-exist"), &[], limit).0,
        Status::Crash
    );
    let slow = script!(&d, "slow", "exec sleep 10");
    let (status, took) = run_once(&slow, &[], Duration::from_millis(100));
    assert_eq!(status, Status::Timeout);
    assert!(
        took >= Duration::from_millis(100) && took < Duration::from_secs(5),
        "{took:?}"
    );
}

#[test]
fn corpus_selection_and_odd_names() {
    let d = temp_dir!();
    for name in ["b.pdf", "a.pdf", "c.PDF", "d.txt", "q\"\\\n\u{1}é.pdf"] {
        std::fs::write(d.join(name), b"%PDF-1.7\n").unwrap();
    }
    std::fs::create_dir(d.join("e.pdf")).unwrap();
    let names = |n| {
        corpus_inputs(&d, n)
            .unwrap()
            .into_iter()
            .map(|i| i.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(2), ["a.pdf", "b.pdf"]);
    assert_eq!(names(10), ["a.pdf", "b.pdf", "q\"\\\n\u{1}é.pdf"]);
    assert_eq!(corpus_inputs(&d, 1).unwrap()[0].bytes, 9);

    // The odd name survives the JSON round trip.
    let cli = script!(&temp_dir!(), "cli", STUB_CLI);
    let out = temp_dir!().join("r.json");
    let o = bench!(&[
        &os("--cli"),
        &os(&cli),
        &os("--out"),
        &os(&out),
        &os("--mutool"),
        &os("none"),
        &os("--pdfium"),
        &os("none"),
        &os("--corpus"),
        &os(&d),
        &os("--runs"),
        &os("1"),
    ]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = std::fs::read_to_string(&out).unwrap();
    assert_eq!(py!(&text, "r['config']['sample']"), r#""corpus""#);
    assert_eq!(
        py!(
            &text,
            "r['files'][2]['name'] == 'q\"\\\\\\n\\x01\\u00e9.pdf'"
        ),
        "true"
    );
}
