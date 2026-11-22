//! `cargo xtask ci macos`: build the macOS app (apps/macos), run its XCUITest E2E suite and
//! check the shipped build's sandbox and hardened runtime.
//! Spec: specs/ui-macos/app-skeleton.md. Project and test-ID mapping: docs/adr/0003.
//! Contract: ops/judge/CONTRACT.md §2.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::json::{self, Value};
use crate::web::{Status, check_known_ids, downgrade_passes, remove_stale, render_e2e, step};

/// Largest tool output we read (xcresulttool JSON, entitlements, codesign text).
const MAX_TOOL_OUTPUT: usize = 16 << 20;

const PROJECT: &str = "apps/macos/JPDF.xcodeproj";
const SCHEME: &str = "JPDF";

/// Entitlements of the shipped (Release) app: sandboxed; files only through the open and save
/// panels (PLAN §2).
const APP_ENTITLEMENTS: &[&str] = &[
    "com.apple.security.app-sandbox",
    "com.apple.security.files.user-selected.read-write",
];
/// The XPC helper is sandboxed with no file entitlements at all (PLAN §2).
const HELPER_ENTITLEMENTS: &[&str] = &["com.apple.security.app-sandbox"];

/// `e2e.macos.<id>` with `id` = `[a-z0-9_]+` (CONTRACT §4).
pub fn valid_macos_id(id: &str) -> bool {
    id.strip_prefix("e2e.macos.").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    })
}

/// Maps an XCTest method name to its journey test ID (ADR 0003): `test_<id>()` or
/// `test_<id>__<detail>()` → `e2e.macos.<id>`. `<id>` has no `__` and doesn't end in `_`;
/// `<detail>` is non-empty and doesn't start with `_`.
pub fn test_id(name: &str) -> Option<String> {
    let base = name.strip_suffix("()").unwrap_or(name);
    let rest = base.strip_prefix("test_")?;
    let id = match rest.split_once("__") {
        Some((id, detail)) if !detail.is_empty() && !detail.starts_with('_') => id,
        Some(_) => return None,
        None => rest,
    };
    if id.ends_with('_') {
        return None;
    }
    let full = format!("e2e.macos.{id}");
    valid_macos_id(&full).then_some(full)
}

fn status_of(result: &str) -> Option<Status> {
    // "Expected Failure" counts as fail, like Playwright's `test.fail` (ADR 0002 item 1).
    match result {
        "Passed" => Some(Status::Pass),
        "Failed" | "Expected Failure" => Some(Status::Fail),
        "Skipped" => Some(Status::Skip),
        _ => None,
    }
}

fn worse(a: Status, b: Status) -> Status {
    let rank = |s| match s {
        Status::Fail => 2,
        Status::Skip => 1,
        Status::Pass => 0,
    };
    if rank(b) > rank(a) { b } else { a }
}

/// Collects test results from `xcresulttool get test-results tests` JSON. Every "Test Case"
/// node must map to an ID (a "System Failures" case or a misnamed test is an error); an ID
/// takes the worst result of its tests. Returns the results and any errors found.
pub fn collect_results(doc: &Value) -> (BTreeMap<String, Status>, Vec<String>) {
    let mut results = BTreeMap::new();
    let mut errors = Vec::new();
    let mut stack: Vec<&Value> = match doc.get("testNodes").and_then(Value::as_array) {
        Some(nodes) => nodes.iter().collect(),
        None => {
            errors.push("xcresulttool output has no `testNodes` array".to_owned());
            Vec::new()
        }
    };
    while let Some(node) = stack.pop() {
        let name = node.get("name").and_then(Value::as_str).unwrap_or("");
        if node.get("nodeType").and_then(Value::as_str) == Some("Test Case") {
            let result = node.get("result").and_then(Value::as_str).unwrap_or("");
            match (test_id(name), status_of(result)) {
                (Some(id), Some(s)) => {
                    let s = results.get(&id).map_or(s, |old| worse(*old, s));
                    results.insert(id, s);
                }
                (None, _) => errors.push(format!(
                    "test case {name:?} ({result}) does not name a test ID (test_<id>[__<detail>])"
                )),
                (Some(_), None) => {
                    errors.push(format!("test case {name:?}: unknown result {result:?}"));
                }
            }
            // A test case's children are failure messages and repetitions, not tests.
            continue;
        }
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            stack.extend(children);
        }
    }
    (results, errors)
}

/// What `e2e.json` gets from a macOS run, and every error in order (the first is the
/// summary). Any error (the `xcodebuild test` step, an unmapped or unreadable result, the
/// Release signature check) downgrades every `pass` to `fail`: such a run may not have run
/// every test, or its tests ran against a build that doesn't ship as checked (#76).
pub fn final_results(
    tests: Result<(), String>,
    read: (BTreeMap<String, Status>, Vec<String>),
    shipped: Result<(), String>,
) -> (BTreeMap<String, Status>, Vec<String>) {
    let (mut results, mut errors) = read;
    if let Err(e) = tests {
        errors.insert(0, e);
    }
    if let Err(e) = shipped {
        errors.push(e);
    }
    if !errors.is_empty() {
        downgrade_passes(&mut results);
    }
    (results, errors)
}

/// Checks a signed bundle's entitlements (`plutil -convert json` of `codesign -d
/// --entitlements`): exactly `expected`, each `true`.
pub fn check_entitlements(doc: &Value, expected: &[&str]) -> Result<(), String> {
    let Value::Object(members) = doc else {
        return Err("entitlements are not a dictionary".to_owned());
    };
    for (k, v) in members {
        if !expected.contains(&k.as_str()) {
            return Err(format!("unexpected entitlement {k}"));
        }
        if *v != Value::Bool(true) {
            return Err(format!("entitlement {k} is not `true`"));
        }
    }
    for k in expected {
        if !members.iter().any(|(m, _)| m == k) {
            return Err(format!("missing entitlement {k}"));
        }
    }
    Ok(())
}

/// True if `codesign -dv` output shows the hardened runtime flag on the code directory.
pub fn has_hardened_runtime(codesign_dv: &str) -> bool {
    codesign_dv.lines().any(|l| {
        l.starts_with("CodeDirectory ")
            && l.split_whitespace()
                .find_map(|w| w.strip_prefix("flags="))
                .and_then(|f| f.split_once('('))
                .and_then(|(_, rest)| rest.strip_suffix(')'))
                .is_some_and(|names| names.split(',').any(|n| n == "runtime"))
    })
}

/// Runs a tool and returns its stdout and stderr; a failure exit or too much output is an error.
fn capture(program: &str, args: &[&str], stdin: Option<&[u8]>) -> Result<(String, String), String> {
    use std::io::Write as _;
    use std::process::Stdio;
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    // The input goes in on its own thread while the pipes are read below: a tool that
    // writes a pipe's worth of output before it reads all its input would otherwise block
    // on its stdout while we block on its stdin. The pipe closes when the thread ends.
    let writer = match (stdin, child.stdin.take()) {
        (Some(input), Some(mut pipe)) => {
            let input = input.to_vec();
            Some(std::thread::spawn(move || pipe.write_all(&input)))
        }
        _ => None,
    };
    // Read at most MAX_TOOL_OUTPUT + 1 bytes from each pipe (stderr on a thread so neither
    // pipe can fill up and block the tool), so a runaway tool never fills memory.
    let limit = u64::try_from(MAX_TOOL_OUTPUT)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let read_capped = move |pipe: Option<Box<dyn std::io::Read + Send>>| -> Vec<u8> {
        use std::io::Read as _;
        let mut buf = Vec::new();
        if let Some(p) = pipe {
            // A read error leaves what was read; the exit status still decides success.
            let _ = p.take(limit).read_to_end(&mut buf);
        }
        buf
    };
    let err_pipe = child
        .stderr
        .take()
        .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>);
    let err_thread = std::thread::spawn(move || read_capped(err_pipe));
    let stdout = read_capped(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
    );
    if stdout.len() > MAX_TOOL_OUTPUT {
        // Stop the tool before waiting for stderr, so a tool that ignores SIGPIPE can't stall.
        let _ = child.kill();
    }
    let stderr = err_thread.join().unwrap_or_default();
    if stdout.len() > MAX_TOOL_OUTPUT || stderr.len() > MAX_TOOL_OUTPUT {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "{program}: output larger than {MAX_TOOL_OUTPUT} bytes"
        ));
    }
    let status = child.wait().map_err(|e| format!("{program}: {e}"))?;
    // The tool has exited, so its stdin is closed (unless it left a child holding it) and a
    // blocked write has failed. A write error means the tool didn't get all its input; a
    // failed exit is reported first, because its stderr says more.
    let written = match writer.map(std::thread::JoinHandle::join) {
        None | Some(Ok(Ok(()))) => Ok(()),
        Some(Ok(Err(e))) => Err(format!("{program}: write stdin: {e}")),
        Some(Err(_)) => Err(format!("{program}: write stdin: writer thread panicked")),
    };
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|_| format!("{program}: not UTF-8"));
    let (stdout, stderr) = (text(stdout)?, text(stderr)?);
    if !status.success() {
        return Err(format!(
            "{program} {} failed ({}): {}",
            args.join(" "),
            status,
            stderr.trim()
        ));
    }
    written?;
    Ok((stdout, stderr))
}

/// `cargo xtask ci macos`. Always writes `out/e2e.json` once `out` exists (empty or partial
/// results when a step fails, with no `pass` if any step or check failed or a reported ID is
/// unknown). Returns Err with the first failure.
pub fn ci(repo: &Path, out: &Path) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let out = fs::canonicalize(out).map_err(|e| format!("resolve {}: {e}", out.display()))?;
    let e2e_json = out.join("e2e.json");
    let bundle = out.join("macos.xcresult");
    // If an earlier run's output can't be removed (or the repo can't be resolved), nothing
    // runs and e2e.json still gets (empty) results (#76).
    let ready = remove_stale(&[&e2e_json, &bundle]).and_then(|()| {
        fs::canonicalize(repo).map_err(|e| format!("resolve {}: {e}", repo.display()))
    });
    let (results, mut errors) = match ready {
        Ok(repo) => {
            // Derived data stays out of DIR (CI uploads DIR as an artifact).
            let derived = repo.join("target").join("xtask-macos");
            let tests = run_tests(&repo, &derived, &bundle);
            let read = read_results(&bundle);
            // The shipped configuration is checked even when tests fail, so one run shows
            // both, and before e2e.json is written, so its failure also turns every pass
            // into a fail.
            let shipped = check_release(&repo, &derived);
            let (mut results, mut errors) = final_results(tests, read, shipped);
            errors.extend(check_known_ids(
                &mut results,
                crate::specs::journey_test_ids(&repo),
            ));
            (results, errors)
        }
        Err(e) => (BTreeMap::new(), vec![e]),
    };
    fs::write(&e2e_json, render_e2e(&results))
        .map_err(|e| format!("write {}: {e}", e2e_json.display()))?;
    eprintln!(
        "xtask: wrote {} ({} macos test ids)",
        e2e_json.display(),
        results.len()
    );
    if results.is_empty() {
        errors.push("XCUITest reported no journey test ids".to_owned());
    }
    if let Some((id, s)) = results.iter().find(|(_, s)| **s != Status::Pass) {
        errors.push(format!("{id}: {}", s.as_str()));
    }
    for e in errors.iter().skip(1) {
        eprintln!("xtask: macos: {e}");
    }
    errors.into_iter().next().map_or(Ok(()), Err)
}

/// The architecture xtask was built for (the host's, with the usual toolchain), so `-destination` matches exactly one entry (not also the
/// Rosetta "My Mac").
fn destination() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "platform=macOS,arch=x86_64"
    } else {
        "platform=macOS,arch=arm64"
    }
}

fn xcodebuild_args<'a>(derived: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "-project",
        PROJECT,
        "-scheme",
        SCHEME,
        "-destination",
        destination(),
        "-derivedDataPath",
        derived,
    ];
    args.extend_from_slice(extra);
    args
}

fn run_tests(repo: &Path, derived: &Path, bundle: &Path) -> Result<(), String> {
    if !repo.join(PROJECT).is_dir() {
        return Err(format!(
            "{PROJECT} not found (run from the repository root)"
        ));
    }
    match xcodebuild_version() {
        Some(v) => eprintln!("xtask: {}", v.replace('\n', ", ")),
        None => return Err("missing tool: xcodebuild (install Xcode 16 or later)".to_owned()),
    }
    let derived = path_str(derived)?;
    let bundle = path_str(bundle)?;
    let args = xcodebuild_args(derived, &["-resultBundlePath", bundle, "test"]);
    step(repo, "xcuitest", "xcodebuild", &args, &[])
}

fn xcodebuild_version() -> Option<String> {
    capture("xcodebuild", &["-version"], None)
        .ok()
        .map(|(o, _)| o.trim().to_owned())
}

fn path_str(p: &Path) -> Result<&str, String> {
    p.to_str()
        .ok_or_else(|| format!("{}: path is not UTF-8", p.display()))
}

/// Reads the result bundle; a missing bundle (build failed) means no results.
fn read_results(bundle: &Path) -> (BTreeMap<String, Status>, Vec<String>) {
    if !bundle.exists() {
        return (BTreeMap::new(), Vec::new());
    }
    let parsed = path_str(bundle)
        .and_then(|b| {
            capture(
                "xcrun",
                &["xcresulttool", "get", "test-results", "tests", "--path", b],
                None,
            )
        })
        .and_then(|(stdout, _)| json::parse(&stdout));
    match parsed {
        Ok(doc) => collect_results(&doc),
        Err(e) => (BTreeMap::new(), vec![format!("read results: {e}")]),
    }
}

/// Builds Release (the shipped configuration) and checks that the app and its XPC helper are
/// signed with the hardened runtime and exactly the expected entitlements.
fn check_release(repo: &Path, derived: &Path) -> Result<(), String> {
    let derived_s = path_str(derived)?;
    let args = xcodebuild_args(derived_s, &["-configuration", "Release", "build"]);
    step(repo, "release-build", "xcodebuild", &args, &[])?;
    let app: PathBuf = derived.join("Build/Products/Release/jpdf.app");
    let helper = app.join("Contents/XPCServices/JPDFHelper.xpc");
    for (bundle, expected) in [(&app, APP_ENTITLEMENTS), (&helper, HELPER_ENTITLEMENTS)] {
        let name = bundle
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if !bundle.is_dir() {
            return Err(format!("{}: not built", bundle.display()));
        }
        let b = path_str(bundle)?;
        // The seal must hold (nothing changed after signing; nested code verified too).
        capture("codesign", &["--verify", "--deep", "--strict", b], None)?;
        let (_, dv) = capture("codesign", &["-dv", "--verbose=2", b], None)?;
        if !has_hardened_runtime(&dv) {
            return Err(format!("{name}: hardened runtime is off"));
        }
        let (xml, _) = capture("codesign", &["-d", "--entitlements", "-", "--xml", b], None)?;
        let (js, _) = capture(
            "plutil",
            &["-convert", "json", "-o", "-", "--", "-"],
            Some(xml.as_bytes()),
        )?;
        let doc = json::parse(&js).map_err(|e| format!("{name}: entitlements: {e}"))?;
        check_entitlements(&doc, expected).map_err(|e| format!("{name}: {e}"))?;
        eprintln!("xtask: macos: {name}: hardened runtime, entitlements {expected:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `capture` on a thread with a deadline, so a pipe deadlock fails the test.
    fn capture_within(
        args: &'static [&'static str],
        input: Vec<u8>,
    ) -> Result<(String, String), String> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(capture("/bin/sh", args, Some(&input)));
        });
        rx.recv_timeout(std::time::Duration::from_secs(30))
            .expect("capture deadlocked")
    }

    #[test]
    fn capture_feeds_stdin_while_reading_output() {
        // specs/ui-macos/app-skeleton.md Security (#83 item 6).
        // 300 000 bytes each way is well over a pipe buffer (64 KiB on macOS and Linux). The
        // tool writes all its output before it reads any input: writing stdin first, then
        // reading, deadlocks here.
        let script: &'static [&'static str] = &[
            "-c",
            "head -c 300000 /dev/zero | tr '\\0' a; wc -c | tr -d ' '",
        ];
        let (out, _) = capture_within(script, vec![b'b'; 300_000]).unwrap();
        assert_eq!(out.len(), 300_000 + "300000\n".len());
        assert!(
            out.ends_with("a300000\n"),
            "{:?}",
            out.get(out.len() - 20..)
        );
    }

    #[test]
    fn capture_reports_unread_input_and_failures() {
        // specs/ui-macos/app-skeleton.md Security (#83 item 6).
        // A tool that exits 0 without reading 300 000 bytes of input didn't get its input.
        let e = capture_within(&["-c", "exit 0"], vec![b'b'; 300_000]).unwrap_err();
        assert!(e.starts_with("/bin/sh: write stdin: "), "{e}");
        // A failing tool is reported by its exit, not by the unread input.
        let e = capture_within(&["-c", "echo no >&2; exit 4"], vec![b'b'; 300_000]).unwrap_err();
        assert!(e.contains("failed (") && e.ends_with(": no"), "{e}");
    }

    #[test]
    fn maps_test_names() {
        let id = |n: &str| test_id(n);
        assert_eq!(
            id("test_app_start_empty()").as_deref(),
            Some("e2e.macos.app_start_empty")
        );
        assert_eq!(
            id("test_app_start_empty__open_is_inert()").as_deref(),
            Some("e2e.macos.app_start_empty")
        );
        assert_eq!(id("test_a1").as_deref(), Some("e2e.macos.a1"));
        for bad in [
            "testAppStart()",
            "test_()",
            "test___x()",
            "test_a__()",
            "test_a___b()",
            "test_App()",
            "test_a-b()",
            "JPDFUITests-Runner (57103) encountered an error",
            "",
        ] {
            assert_eq!(id(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn macos_ids() {
        assert!(valid_macos_id("e2e.macos.rd_open_dialog"));
        for bad in ["e2e.macos.", "e2e.web.a", "e2e.macos.A", "e2e.macos.a.b"] {
            assert!(!valid_macos_id(bad), "{bad:?}");
        }
    }

    fn case(name: &str, result: &str) -> String {
        format!(
            r#"{{"name": "{name}", "nodeType": "Test Case", "result": "{result}",
               "children": [{{"name": "x", "nodeType": "Failure Message"}}]}}"#
        )
    }

    fn doc(cases: &[String]) -> Value {
        let text = format!(
            r#"{{"devices": [], "testNodes": [{{"name": "JPDF", "nodeType": "Test Plan",
               "children": [{{"name": "JPDFUITests", "nodeType": "UI test bundle",
               "children": [{{"name": "S", "nodeType": "Test Suite", "children": [{}]}}]}}]}}]}}"#,
            cases.join(",")
        );
        json::parse(&text).unwrap()
    }

    #[test]
    fn collects_worst_result_per_id() {
        let (r, errors) = collect_results(&doc(&[
            case("test_a__one()", "Passed"),
            case("test_a__two()", "Skipped"),
            case("test_b()", "Passed"),
            case("test_c__x()", "Expected Failure"),
            case("test_d__x()", "Failed"),
            case("test_d__y()", "Skipped"),
        ]));
        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(r.get("e2e.macos.a"), Some(&Status::Skip));
        assert_eq!(r.get("e2e.macos.b"), Some(&Status::Pass));
        assert_eq!(r.get("e2e.macos.c"), Some(&Status::Fail));
        assert_eq!(r.get("e2e.macos.d"), Some(&Status::Fail));
        assert_eq!(r.len(), 4);
    }

    #[test]
    fn unmapped_cases_and_unknown_results_are_errors() {
        let (r, errors) = collect_results(&doc(&[
            case("JPDFUITests-Runner (1) encountered an error", "Failed"),
            case("test_a()", "Mixed"),
            case("test_b()", "Passed"),
        ]));
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert_eq!(r.len(), 1);
        let (r, errors) = collect_results(&json::parse(r#"{"devices": []}"#).unwrap());
        assert!(r.is_empty());
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn any_error_reports_no_pass() {
        let read = |errors: &[&str]| {
            let mut r = BTreeMap::new();
            r.insert("e2e.macos.a".to_owned(), Status::Pass);
            r.insert("e2e.macos.b".to_owned(), Status::Skip);
            (
                r,
                errors.iter().map(|e| (*e).to_owned()).collect::<Vec<_>>(),
            )
        };
        let get = |r: &BTreeMap<String, Status>| {
            (r.get("e2e.macos.a").copied(), r.get("e2e.macos.b").copied())
        };
        let err = |e: &str| Err(e.to_owned());
        // A clean run keeps its passes.
        let (r, e) = final_results(Ok(()), read(&[]), Ok(()));
        assert_eq!(get(&r), (Some(Status::Pass), Some(Status::Skip)));
        assert!(e.is_empty());
        // xcodebuild test failed (#76 item 2): no pass, and its error comes first.
        let (r, e) = final_results(err("test"), read(&["read"]), Ok(()));
        assert_eq!(get(&r), (Some(Status::Fail), Some(Status::Skip)));
        assert_eq!(e, ["test", "read"]);
        // xcodebuild test failed although every case mapped cleanly: still no pass.
        let (r, e) = final_results(err("test"), read(&[]), Ok(()));
        assert_eq!(get(&r), (Some(Status::Fail), Some(Status::Skip)));
        assert_eq!(e, ["test"]);
        // Only the Release check failed (#76 item 2): no pass, its error comes last.
        let (r, e) = final_results(Ok(()), read(&["read"]), err("release"));
        assert_eq!(get(&r), (Some(Status::Fail), Some(Status::Skip)));
        assert_eq!(e, ["read", "release"]);
        let (r, e) = final_results(Ok(()), read(&[]), err("release"));
        assert_eq!(get(&r), (Some(Status::Fail), Some(Status::Skip)));
        assert_eq!(e, ["release"]);
        // Only an unmapped result (the "System Failures" case): no pass.
        let (r, e) = final_results(Ok(()), read(&["unmapped"]), Ok(()));
        assert_eq!(get(&r), (Some(Status::Fail), Some(Status::Skip)));
        assert_eq!(e, ["unmapped"]);
    }

    #[test]
    fn stale_output_that_cannot_be_removed_still_writes_e2e_json() {
        use crate::web::testutil::{undeletable, unlock};
        let base = std::env::temp_dir().join(format!("xtask-macos-ci-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let out = base.join("out");
        assert!(fs::create_dir_all(&out).is_ok());
        let Some(locked) = undeletable(&out.join("macos.xcresult")) else {
            let _ = fs::remove_dir_all(&base);
            return; // root can remove anything
        };
        assert!(
            fs::write(
                out.join("e2e.json"),
                "{\"results\": {\"e2e.macos.a\": \"pass\"}}\n"
            )
            .is_ok()
        );
        // The repo doesn't exist either: nothing may run, and the stale error comes first.
        let r = ci(&base.join("repo"), &out);
        assert!(r.as_ref().is_err_and(|e| e.starts_with("remove ")), "{r:?}");
        assert_eq!(
            fs::read_to_string(out.join("e2e.json")).ok().as_deref(),
            Some("{\"results\": {}}\n")
        );
        unlock(&locked);
        // Stale output gone, but the repo can't be resolved: e2e.json is still written.
        let r = ci(&base.join("repo"), &out);
        assert!(
            r.as_ref().is_err_and(|e| e.starts_with("resolve ")),
            "{r:?}"
        );
        assert_eq!(
            fs::read_to_string(out.join("e2e.json")).ok().as_deref(),
            Some("{\"results\": {}}\n")
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn checks_entitlements() {
        let e = |t: &str| json::parse(t).unwrap();
        let ok = e(r#"{"com.apple.security.app-sandbox": true}"#);
        assert_eq!(check_entitlements(&ok, HELPER_ENTITLEMENTS), Ok(()));
        assert!(
            check_entitlements(&ok, APP_ENTITLEMENTS).is_err(),
            "missing"
        );
        for bad in [
            r#"{"com.apple.security.app-sandbox": false}"#,
            r#"{"com.apple.security.app-sandbox": true, "com.apple.security.get-task-allow": true}"#,
            r#"{"com.apple.security.app-sandbox": true,
                "com.apple.security.temporary-exception.files.absolute-path.read-only": ["/"]}"#,
            r#"{}"#,
            r#"[]"#,
        ] {
            assert!(
                check_entitlements(&e(bad), HELPER_ENTITLEMENTS).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn reads_hardened_runtime_flag() {
        let dv = "Executable=/x\nCodeDirectory v=20500 size=632 flags=0x10002(adhoc,runtime) hashes=9+7 location=embedded\n";
        assert!(has_hardened_runtime(dv));
        for bad in [
            "CodeDirectory v=20400 size=432 flags=0x2(adhoc) hashes=3+7 location=embedded",
            "Identifier=runtime\nCodeDirectory v=1 flags=0x2(adhoc) x",
            "CodeDirectory v=1 flags=0x10002(adhoc,runtimex) x",
            "",
        ] {
            assert!(!has_hardened_runtime(bad), "{bad:?}");
        }
    }
}
