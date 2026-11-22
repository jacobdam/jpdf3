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
    pub fn as_str(self) -> &'static str {
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

/// Turns every `pass` into `fail`. Used when the run itself failed: a test file that didn't
/// load, a throwing hook or a failed release check can drop tests while another test with the
/// same ID passes, so no ID from such a run counts as passing (#76).
pub fn downgrade_passes(results: &mut BTreeMap<String, Status>) {
    for s in results.values_mut() {
        if *s == Status::Pass {
            *s = Status::Fail;
        }
    }
}

/// What `e2e.json` gets from a web run: the reporter's results, with every pass downgraded
/// when the Playwright run (or an earlier step) failed; damaged results become an empty set
/// and an error.
pub fn final_results(
    run: &Result<(), String>,
    read: Result<BTreeMap<String, Status>, String>,
) -> (BTreeMap<String, Status>, Option<String>) {
    let (mut results, parse_err) = match read {
        Ok(r) => (r, None),
        Err(e) => (BTreeMap::new(), Some(e)),
    };
    if run.is_err() {
        downgrade_passes(&mut results);
    }
    (results, parse_err)
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

/// `npm ci` with dependency install scripts off. `.npmrc` sets `ignore-scripts` too, but an
/// inherited `npm_config_ignore_scripts=false` overrides the file; the flag overrides both.
const NPM_INSTALL: &[&str] = &["ci", "--ignore-scripts"];

fn tool_version(tool: &str) -> Option<String> {
    let out = Command::new(tool).arg("--version").output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Runs one step in `dir`, echoing it; Err names the failed step.
pub fn step(
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
/// results when a step fails, with no `pass` if any step failed); writes `out/site/` only
/// after a successful build.
/// Returns Err with the first failure.
pub fn ci(repo: &Path, out: &Path) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let out = fs::canonicalize(out).map_err(|e| format!("resolve {}: {e}", out.display()))?;
    let e2e_json = out.join("e2e.json");
    let tsv = out.join("web-e2e.tsv");
    let site = out.join("site");
    // Never leave output of an earlier run: results, reporter file and site all go first.
    let mut stale_err = None;
    for stale in [&e2e_json, &tsv] {
        if let Err(e) = fs::remove_file(stale)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            stale_err = Some(format!("remove {}: {e}", stale.display()));
        }
    }
    if let Err(e) = fs::remove_dir_all(&site)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        stale_err = Some(format!("remove {}: {e}", site.display()));
    }
    if let Some(e) = stale_err {
        return Err(e);
    }
    let run = run_web(repo, &out, &tsv);
    let (results, parse_err) = final_results(&run, read_results(&tsv));
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

/// Reads at most `MAX_RESULTS_BYTES` (+1 to detect overflow) in one pass; a missing file
/// means no tests reported anything.
fn read_results(tsv: &Path) -> Result<BTreeMap<String, Status>, String> {
    use std::io::Read as _;
    let file = match fs::File::open(tsv) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(format!("open {}: {e}", tsv.display())),
    };
    let mut bytes = Vec::new();
    file.take(MAX_RESULTS_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read {}: {e}", tsv.display()))?;
    if u64::try_from(bytes.len()).map_or(true, |n| n > MAX_RESULTS_BYTES) {
        return Err(format!(
            "{}: larger than {MAX_RESULTS_BYTES} bytes",
            tsv.display()
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| format!("{}: not UTF-8", tsv.display()))?;
    parse_results(&text).map_err(|e| format!("{}: {e}", tsv.display()))
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
    step(&web, "install", "npm", NPM_INSTALL, no_env)?;
    step(&web, "licenses", "npm", &["run", "licenses"], no_env)?;
    step(&web, "typecheck", "npm", &["run", "typecheck"], no_env)?;
    step(&web, "lint", "npm", &["run", "lint"], no_env)?;
    step(&web, "build", "npm", &["run", "build"], no_env)?;

    copy_tree(&web.join("dist"), &out.join("site"))?;

    // Browsers are not downloaded by `npm ci` (scripts are off). On CI (Linux) also install
    // the system libraries Chromium needs.
    // `npm exec --no` never downloads a package that isn't installed from the lockfile.
    let mut install = vec!["exec", "--no", "--", "playwright", "install", "chromium"];
    if cfg!(target_os = "linux") && std::env::var_os("CI").is_some() {
        install.push("--with-deps");
    }
    step(&web, "browsers", "npm", &install, no_env)?;
    step(
        &web,
        "e2e",
        "npm",
        &["exec", "--no", "--", "playwright", "test"],
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

    fn all(pairs: &[(&str, Status)]) -> BTreeMap<String, Status> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    #[test]
    fn a_failed_run_reports_no_pass() {
        let read = || {
            Ok(all(&[
                ("e2e.web.a", Status::Pass),
                ("e2e.web.b", Status::Skip),
                ("e2e.web.c", Status::Fail),
            ]))
        };
        // Playwright exited 0: the reporter's results stand.
        assert_eq!(
            final_results(&Ok(()), read()),
            (
                all(&[
                    ("e2e.web.a", Status::Pass),
                    ("e2e.web.b", Status::Skip),
                    ("e2e.web.c", Status::Fail),
                ]),
                None
            )
        );
        // Playwright exited non-zero (an unloadable spec file, a throwing hook): no pass.
        assert_eq!(
            final_results(&Err("step `e2e` failed".to_owned()), read()),
            (
                all(&[
                    ("e2e.web.a", Status::Fail),
                    ("e2e.web.b", Status::Skip),
                    ("e2e.web.c", Status::Fail),
                ]),
                None
            )
        );
        // A damaged file is an error and an empty set, whatever the run did.
        for run in [Ok(()), Err("x".to_owned())] {
            assert_eq!(
                final_results(&run, Err("bad".to_owned())),
                (BTreeMap::new(), Some("bad".to_owned()))
            );
        }
    }

    #[test]
    fn downgrades_passes() {
        let mut r = all(&[("e2e.web.a", Status::Pass), ("e2e.web.b", Status::Skip)]);
        downgrade_passes(&mut r);
        assert_eq!(
            r,
            all(&[("e2e.web.a", Status::Fail), ("e2e.web.b", Status::Skip)])
        );
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
    fn reads_results_files() {
        let base = std::env::temp_dir().join(format!("xtask-web-read-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        assert!(fs::create_dir_all(&base).is_ok());
        let f = base.join("r.tsv");
        assert_eq!(
            read_results(&f),
            Ok(BTreeMap::new()),
            "missing file = no results"
        );
        assert!(fs::write(&f, "e2e.web.a\tpass\n").is_ok());
        assert_eq!(read_results(&f).map(|r| r.len()), Ok(1));
        assert!(fs::write(&f, [0xff_u8, b'\t']).is_ok());
        assert!(read_results(&f).is_err(), "non-UTF-8");
        let big = "e2e.web.a\tpass\n".repeat(80_000); // > 1 MiB
        assert!(fs::write(&f, big).is_ok());
        assert!(read_results(&f).is_err(), "too large");
        let _ = fs::remove_dir_all(&base);
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
