//! End-to-end tests of `cargo xtask metrics --from` on saved inputs (specs/tooling/metrics.md).
#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .output()
}

fn fixture() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "metrics"]
        .iter()
        .collect()
}

/// A fresh, empty directory under cargo's per-target scratch space.
fn scratch(name: &str) -> std::io::Result<PathBuf> {
    let p = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("metrics-{name}"));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p)?;
    Ok(p)
}

fn s(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

#[test]
fn report_from_saved_inputs() {
    let out = scratch("ok").unwrap();
    let o = run(&[
        "metrics",
        "--from",
        &s(&fixture()),
        "--since",
        "2026-11-15",
        "--until",
        "2026-11-22",
        "--out",
        &s(&out),
    ])
    .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "{err}");
    let md = String::from_utf8(o.stdout).unwrap();
    assert_eq!(fs::read_to_string(out.join("metrics.md")).unwrap(), md);
    assert!(!out.join("raw").exists(), "--from must not write raw/");
    for want in [
        "# Metrics 2026-11-15 to 2026-11-22\n",
        "| Merges on main | 2 |\n",
        "| Lines added / deleted (no lockfiles, no generated data) | 13 / 3 |\n",
        "| Merged PRs | 2 |\n",
        "| Closed issues | 2 |\n",
        "| Sessions (non-zero exit) | 2 (1) |\n",
        "| Session time, total | 30 min |\n",
        "| Session time, median | 20 min |\n",
        "| Session time per merged PR | 15.0 min |\n",
        "| Malformed index.tsv rows (skipped) | 1 |\n",
        "PRs reviewed: 1; median rounds: 2; at 4+ rounds: 0/1 (0%); with a final check: 0/1 (0%).\n",
        "subagent 0/1/1/0, second reviewer 0/0/0/0; regrades: 1.\n",
        "| #7 | 2 | no | 0/1/1/0 | 0/0/0/0 | 1 |\n",
        "`blocked`: 1\n- #3 (12 days): stuck / here\n",
        "`flaky`: 0\n",
        "- docs/audits/0001.md: 2026-11-15 (first)\n",
        "Merges since the last audit record: 2.\n",
        "- docs/retros/0002.md: 2026-11-22 (7 days after the previous one)\n",
        "Days from the last retro record to 2026-11-22: 0.\n",
    ] {
        assert!(md.contains(want), "missing {want:?} in\n{md}");
    }
}

#[test]
fn default_since_is_the_last_retro_and_index_can_be_given() {
    let out = scratch("default").unwrap();
    let index = scratch("index").unwrap().join("index.tsv");
    fs::write(&index, "20261122T000000Z\t0\tx\t120s\n").unwrap();
    let o = run(&[
        "metrics",
        "--from",
        &s(&fixture()),
        "--until",
        "2026-11-22",
        "--index",
        &s(&index),
        "--out",
        &s(&out),
    ])
    .unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let md = String::from_utf8(o.stdout).unwrap();
    assert!(
        md.starts_with("# Metrics 2026-11-22 to 2026-11-22\n"),
        "{md}"
    );
    assert!(md.contains("| Merges on main | 1 |\n"), "{md}");
    assert!(
        md.contains("| Sessions (non-zero exit) | 1 (0) |\n"),
        "{md}"
    );
}

#[test]
fn bad_input_exits_1_and_bad_usage_exits_2() {
    let bad = scratch("bad-in").unwrap();
    for f in ["git-log.txt", "issues-closed.json", "issues-open.json"] {
        fs::copy(fixture().join(f), bad.join(f)).unwrap();
    }
    fs::write(bad.join("prs.json"), "[{\"number\":1,").unwrap();
    let out = s(&scratch("bad-out").unwrap());
    let o = run(&[
        "metrics",
        "--from",
        &s(&bad),
        "--until",
        "2026-11-22",
        "--out",
        &out,
    ])
    .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("prs.json"));
    fs::remove_file(bad.join("prs.json")).unwrap();
    let o = run(&[
        "metrics",
        "--from",
        &s(&bad),
        "--until",
        "2026-11-22",
        "--out",
        &out,
    ])
    .unwrap();
    assert_eq!(o.status.code(), Some(1), "a missing prs.json is an error");

    for args in [
        &["metrics", "--since", "2026-11-31"][..],
        &["metrics", "--since", "2026-11-23", "--until", "2026-11-22"],
        &["metrics", "--since", "9999-12-31"], // after the default --until (PR #220 R1.2)
        &["metrics", "--out"],
        &["metrics", "--frm", "x"],
    ] {
        let o = run(args).unwrap();
        assert_eq!(o.status.code(), Some(2), "{args:?}");
        assert!(
            String::from_utf8_lossy(&o.stderr).contains("usage:"),
            "{args:?}"
        );
    }
}
