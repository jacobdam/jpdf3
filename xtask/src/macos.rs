//! `cargo xtask ci macos`: build the macOS app (apps/macos), run its XCUITest E2E suite and
//! check the shipped build's sandbox and hardened runtime.
//! Spec: specs/ui-macos/app-skeleton.md. Project and test-ID mapping: docs/adr/0003.
//! Contract: ops/judge/CONTRACT.md §2.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::json::{self, Value};
use crate::web::{Status, render_e2e, step};

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
    if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(input)
            .map_err(|e| format!("{program}: write stdin: {e}"))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("{program}: {e}"))?;
    if out.stdout.len() > MAX_TOOL_OUTPUT || out.stderr.len() > MAX_TOOL_OUTPUT {
        return Err(format!(
            "{program}: output larger than {MAX_TOOL_OUTPUT} bytes"
        ));
    }
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|_| format!("{program}: not UTF-8"));
    let (stdout, stderr) = (text(out.stdout)?, text(out.stderr)?);
    if !out.status.success() {
        return Err(format!(
            "{program} {} failed ({}): {}",
            args.join(" "),
            out.status,
            stderr.trim()
        ));
    }
    Ok((stdout, stderr))
}

fn remove_stale(path: &Path) -> Result<(), String> {
    let r = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match r {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("remove {}: {e}", path.display()))
        }
        _ => Ok(()),
    }
}

/// `cargo xtask ci macos`. Always writes `out/e2e.json` (empty or partial results when a step
/// fails). Returns Err with the first failure.
pub fn ci(repo: &Path, out: &Path) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let out = fs::canonicalize(out).map_err(|e| format!("resolve {}: {e}", out.display()))?;
    let repo = fs::canonicalize(repo).map_err(|e| format!("resolve {}: {e}", repo.display()))?;
    let e2e_json = out.join("e2e.json");
    let bundle = out.join("macos.xcresult");
    // Derived data stays out of DIR (CI uploads DIR as an artifact).
    let derived = repo.join("target").join("xtask-macos");
    for stale in [&e2e_json, &bundle] {
        remove_stale(stale)?;
    }

    let tests = run_tests(&repo, &derived, &bundle);
    let (results, mut errors) = read_results(&bundle);
    fs::write(&e2e_json, render_e2e(&results))
        .map_err(|e| format!("write {}: {e}", e2e_json.display()))?;
    eprintln!(
        "xtask: wrote {} ({} macos test ids)",
        e2e_json.display(),
        results.len()
    );
    // The shipped configuration is checked even when tests fail, so one run shows both.
    let shipped = check_release(&repo, &derived);
    if let Err(e) = tests {
        errors.insert(0, e);
    }
    if let Err(e) = shipped {
        errors.push(e);
    }
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

fn xcodebuild_args<'a>(derived: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "-project",
        PROJECT,
        "-scheme",
        SCHEME,
        "-destination",
        "platform=macOS",
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
        let (_, dv) = capture("codesign", &["-dv", "--verbose=2", b], None)?;
        if !has_hardened_runtime(&dv) {
            return Err(format!("{name}: hardened runtime is off"));
        }
        let (xml, _) = capture("codesign", &["-d", "--entitlements", ":-", b], None)?;
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
