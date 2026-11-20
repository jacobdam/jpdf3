//! `cargo xtask ci linux`: build and test the web app (apps/web) with real Playwright E2E.
//! Spec: specs/ui-web/app-skeleton.md. Test-ID mapping and site layout: docs/adr/0002.
//! Contract: ops/judge/CONTRACT.md §2.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Largest results file we accept from the Playwright reporter (it is a few lines per test).
const MAX_RESULTS_BYTES: u64 = 1 << 20;

/// A journey test result as CONTRACT §2 spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    Skip,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Fail => "fail",
            Status::Skip => "skip",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "pass" => Some(Status::Pass),
            "fail" => Some(Status::Fail),
            "skip" => Some(Status::Skip),
            _ => None,
        }
    }
}

/// `e2e.web.<id>` with `id` = `[a-z0-9_]+` (CONTRACT §4, specs/tooling/check-specs.md).
pub fn valid_web_id(id: &str) -> bool {
    id.strip_prefix("e2e.web.").is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    })
}

/// Parses the reporter's `<id>\t<status>` lines. Any malformed line, unknown status, invalid
/// or duplicate ID is an error: the file is ours, so damage means something went wrong.
pub fn parse_results(text: &str) -> Result<BTreeMap<String, Status>, String> {
    let mut out = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let lineno = n.saturating_add(1);
        let Some((id, status)) = line.split_once('\t') else {
            return Err(format!("line {lineno}: expected `<id>\\t<status>`"));
        };
        if !valid_web_id(id) {
            return Err(format!("line {lineno}: invalid test id {id:?}"));
        }
        let Some(status) = Status::parse(status) else {
            return Err(format!("line {lineno}: invalid status {status:?}"));
        };
        if out.insert(id.to_owned(), status).is_some() {
            return Err(format!("line {lineno}: duplicate test id {id}"));
        }
    }
    Ok(out)
}

/// Renders CONTRACT §2's `{"results": {...}}`. IDs are validated, so they need no escaping.
pub fn render_e2e(results: &BTreeMap<String, Status>) -> String {
    let mut s = String::from("{\"results\": {");
    for (i, (id, status)) in results.iter().enumerate() {
        let sep = if i == 0 { "" } else { "," };
        let _ = write!(s, "{sep}\n  \"{id}\": \"{}\"", status.as_str());
    }
    if !results.is_empty() {
        s.push('\n');
    }
    s.push_str("}}\n");
    s
}

/// Copies a directory tree. Symlinks are refused (the build output never contains any).
pub fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    let entries = fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", from.display()))?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let ty = entry
            .file_type()
            .map_err(|e| format!("stat {}: {e}", src.display()))?;
        if ty.is_dir() {
            copy_tree(&src, &dst)?;
        } else if ty.is_file() {
            fs::copy(&src, &dst).map_err(|e| format!("copy {}: {e}", src.display()))?;
        } else {
            return Err(format!(
                "{}: not a regular file or directory",
                src.display()
            ));
        }
    }
    Ok(())
}

fn tool_version(tool: &str) -> Option<String> {
    let out = Command::new(tool).arg("--version").output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Runs one step in `dir`, echoing it; Err names the failed step.
fn step(
    dir: &Path,
    name: &str,
    program: &str,
    args: &[&str],
    env: &[(&str, &Path)],
) -> Result<(), String> {
    eprintln!("xtask: [{name}] {program} {}", args.join(" "));
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(dir);
    for (k, v) in env {
        cmd.env(k, v);
    }
    match cmd.status() {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("step `{name}` failed ({s})")),
        Err(e) => Err(format!("step `{name}`: cannot run {program}: {e}")),
    }
}

/// The web part of `cargo xtask ci linux`. Always writes `out/e2e.json` (empty or partial
/// results when a step fails); writes `out/site/` only after a successful build.
/// Returns Err with the first failure.
pub fn ci(repo: &Path, out: &Path) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let out = fs::canonicalize(out).map_err(|e| format!("resolve {}: {e}", out.display()))?;
    let e2e_json = out.join("e2e.json");
    let tsv = out.join("web-e2e.tsv");
    // Never report results of an earlier run.
    for stale in [&e2e_json, &tsv] {
        if stale.exists() {
            fs::remove_file(stale).map_err(|e| format!("remove {}: {e}", stale.display()))?;
        }
    }
    let run = run_web(repo, &out, &tsv);
    let results = match fs::metadata(&tsv) {
        Err(_) => Ok(BTreeMap::new()),
        Ok(m) if m.len() > MAX_RESULTS_BYTES => Err(format!("{}: too large", tsv.display())),
        Ok(_) => fs::read_to_string(&tsv)
            .map_err(|e| format!("read {}: {e}", tsv.display()))
            .and_then(|t| parse_results(&t).map_err(|e| format!("{}: {e}", tsv.display()))),
    };
    let (results, parse_err) = match results {
        Ok(r) => (r, None),
        Err(e) => (BTreeMap::new(), Some(e)),
    };
    fs::write(&e2e_json, render_e2e(&results))
        .map_err(|e| format!("write {}: {e}", e2e_json.display()))?;
    eprintln!(
        "xtask: wrote {} ({} web test ids)",
        e2e_json.display(),
        results.len()
    );
    run?;
    if let Some(e) = parse_err {
        return Err(e);
    }
    if results.is_empty() {
        return Err("Playwright reported no journey test ids".to_owned());
    }
    if let Some((id, s)) = results.iter().find(|(_, s)| **s != Status::Pass) {
        return Err(format!("{id}: {}", s.as_str()));
    }
    Ok(())
}

fn run_web(repo: &Path, out: &Path, tsv: &Path) -> Result<(), String> {
    let web = repo.join("apps").join("web");
    if !web.join("package.json").is_file() {
        return Err(format!(
            "{} not found (run from the repository root)",
            web.join("package.json").display()
        ));
    }
    for tool in ["node", "npm"] {
        match tool_version(tool) {
            Some(v) => eprintln!("xtask: {tool} {v}"),
            None => {
                return Err(format!(
                    "missing tool: {tool} (install Node.js 22.12 or later; CI uses Node 24)"
                ));
            }
        }
    }
    let no_env: &[(&str, &Path)] = &[];
    // .npmrc sets ignore-scripts: no dependency install script ever runs.
    step(&web, "install", "npm", &["ci"], no_env)?;
    step(&web, "licenses", "npm", &["run", "licenses"], no_env)?;
    step(&web, "typecheck", "npm", &["run", "typecheck"], no_env)?;
    step(&web, "lint", "npm", &["run", "lint"], no_env)?;
    step(&web, "build", "npm", &["run", "build"], no_env)?;

    let site = out.join("site");
    if site.exists() {
        fs::remove_dir_all(&site).map_err(|e| format!("remove {}: {e}", site.display()))?;
    }
    copy_tree(&web.join("dist"), &site)?;

    // Browsers are not downloaded by `npm ci` (scripts are off). On CI (Linux) also install
    // the system libraries Chromium needs.
    let mut install = vec!["exec", "--", "playwright", "install", "chromium"];
    if cfg!(target_os = "linux") && std::env::var_os("CI").is_some() {
        install.push("--with-deps");
    }
    step(&web, "browsers", "npm", &install, no_env)?;
    step(
        &web,
        "e2e",
        "npm",
        &["exec", "--", "playwright", "test"],
        &[("JPDF_E2E_RESULTS", tsv)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_ids() {
        assert!(valid_web_id("e2e.web.app_start_empty"));
        assert!(valid_web_id("e2e.web.a1"));
        for bad in [
            "e2e.web.",
            "e2e.macos.a",
            "e2e.web.A",
            "e2e.web.a.b",
            "e2e.web.a-b",
            "e2e.web.a\"",
            "e2e.web.a\\",
            " e2e.web.a",
            "e2e.web.é",
        ] {
            assert!(!valid_web_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn parses_reporter_lines() {
        let r = parse_results("e2e.web.b\tfail\ne2e.web.a\tpass\ne2e.web.c\tskip\n");
        let r = r.unwrap_or_default();
        assert_eq!(r.len(), 3);
        assert_eq!(r.get("e2e.web.a"), Some(&Status::Pass));
        assert_eq!(r.get("e2e.web.b"), Some(&Status::Fail));
        assert_eq!(r.get("e2e.web.c"), Some(&Status::Skip));
        assert_eq!(parse_results(""), Ok(BTreeMap::new()));
    }

    #[test]
    fn rejects_damaged_reporter_lines() {
        for bad in [
            "e2e.web.a pass",
            "e2e.web.a\tok",
            "e2e.web.a\tpass\t",
            "e2e.web.a\tPASS",
            "e2e.web.\"x\tpass",
            "e2e.web.a\tpass\ne2e.web.a\tfail",
            "\n",
        ] {
            assert!(parse_results(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn renders_contract_json() {
        assert_eq!(render_e2e(&BTreeMap::new()), "{\"results\": {}}\n");
        let mut m = BTreeMap::new();
        m.insert("e2e.web.b".to_owned(), Status::Fail);
        m.insert("e2e.web.a".to_owned(), Status::Pass);
        assert_eq!(
            render_e2e(&m),
            "{\"results\": {\n  \"e2e.web.a\": \"pass\",\n  \"e2e.web.b\": \"fail\"\n}}\n"
        );
    }

    #[test]
    fn copies_a_tree() {
        let base = std::env::temp_dir().join(format!("xtask-web-copy-{}", std::process::id()));
        let src = base.join("src");
        let dst = base.join("dst");
        let _ = fs::remove_dir_all(&base);
        assert!(fs::create_dir_all(src.join("assets")).is_ok());
        assert!(fs::write(src.join("index.html"), "i").is_ok());
        assert!(fs::write(src.join("assets").join("a.js"), "a").is_ok());
        assert_eq!(copy_tree(&src, &dst), Ok(()));
        assert_eq!(
            fs::read_to_string(dst.join("index.html")).ok().as_deref(),
            Some("i")
        );
        assert_eq!(
            fs::read_to_string(dst.join("assets").join("a.js"))
                .ok()
                .as_deref(),
            Some("a")
        );
        let _ = fs::remove_dir_all(&base);
    }
}
