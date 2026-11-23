//! The #146 phase gate, enforced on the dependency graph Cargo resolves (#154).
//! Spec: specs/tooling/cli-dependency-gate.md; the gate itself: specs/core/memory-budget.md
//! ("Phases run one at a time", Gate).
//!
//! No decode or raster phase may run on the jpdf-cli path until #146 is closed. This test
//! makes that structural: every package Cargo resolves for jpdf-cli (normal and build
//! edges, every feature, every target) must be one of the parse-only workspace crates, at its
//! workspace path. It reads only what `cargo tree` resolves, never Rust sources or manifests
//! as text (PR #151 showed any text scan can be bypassed).

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use jpdf_testgen::json::{self, Value};

/// The parse-only crates (package name, workspace-relative directory). Today this is exactly
/// jpdf-cli's resolved graph; `cli_graph_is_exactly_the_allowed_set` keeps it that way, so a
/// crate that leaves the graph also leaves the list.
const ALLOWED: &[(&str, &str)] = &[
    ("jpdf-cli", "tools/jpdf-cli"),
    ("jpdf-engine", "crates/jpdf-engine"),
    ("jpdf-cos", "crates/jpdf-cos"),
    ("jpdf-syntax", "crates/jpdf-syntax"),
    ("jpdf-traits", "crates/jpdf-traits"),
    ("jpdf-fsio", "tools/fsio"),
];

/// One `cargo tree --format {p}` line: `<name> v<version>` plus ` (<source>)` for path and
/// git packages; registry packages have no parenthesized part.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Pkg {
    name: String,
    /// The text inside the parentheses, if any (a path, or a git URL).
    source: Option<String>,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Whether the text inside a line's outer parentheses holds a Cargo marker at its token
/// boundary: `(proc-macro)` comes before the source (`proc-macro) (<path>`), `(*)` and any
/// `(proc-macro)` after it (`<path>) (*`, `<path>) (proc-macro`), or a marker alone where a
/// registry package has no source (`proc-macro`, `*`; S4.2).
fn has_marker(inner: &str) -> bool {
    inner == "proc-macro"
        || inner == "*"
        || inner.starts_with("proc-macro) (")
        || inner.ends_with(") (*")
        || inner.ends_with(") (proc-macro")
}

/// Parses `cargo tree --prefix none --format {p}` output. Blank lines (between roots) are
/// skipped; any other line that is not `<name> v<version>[ (<source>)]` is an error, so an
/// output format change fails the gate instead of passing it. The version starts with a digit
/// and holds only ASCII alphanumerics, `.`, `-` and `+`. The source is everything between the
/// ` (` after the version and the line's final `)`, so a path may contain parentheses (also
/// `) (`, S3.1). A marker at its token boundary (`has_marker`) makes the line an error, never a
/// path to resolve (S2.2: a directory named `x) (*` could alias an allowed one).
fn parse_tree(out: &str) -> Result<Vec<Pkg>, String> {
    let mut pkgs = Vec::new();
    for line in out.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let (name, rest) = line
            .split_once(" v")
            .ok_or_else(|| format!("unparsable cargo tree line: {line:?}"))?;
        if name.is_empty() || name.contains(char::is_whitespace) {
            return Err(format!("unparsable cargo tree line: {line:?}"));
        }
        let (version, tail) = match rest.split_once(' ') {
            None => (rest, None),
            Some((v, t)) => (v, Some(t)),
        };
        let version_ok = version.starts_with(|c: char| c.is_ascii_digit())
            && version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
        if !version_ok {
            return Err(format!("unparsable cargo tree line: {line:?}"));
        }
        let source = match tail {
            None => None,
            Some(t) => {
                let inner = t
                    .strip_prefix('(')
                    .and_then(|t| t.strip_suffix(')'))
                    .filter(|t| !t.is_empty() && !has_marker(t))
                    .ok_or_else(|| format!("unparsable cargo tree line: {line:?}"))?;
                Some(inner.to_owned())
            }
        };
        pkgs.push(Pkg {
            name: name.to_owned(),
            source,
        });
    }
    if pkgs.is_empty() {
        return Err("cargo tree listed no packages".to_owned());
    }
    Ok(pkgs)
}

/// Returns one violation line per package that is not an allowed name at its allowed path.
/// `root` is the workspace root the allowed directories are relative to.
fn violations(pkgs: &[Pkg], root: &Path) -> Vec<String> {
    let mut bad = BTreeSet::new();
    for p in pkgs {
        let Some(&(_, dir)) = ALLOWED.iter().find(|(n, _)| *n == p.name) else {
            bad.insert(format!(
                "{} ({:?}): not a parse-only crate",
                p.name, p.source
            ));
            continue;
        };
        let want = root.join(dir).canonicalize().ok();
        // Only an absolute path is resolved: a relative one (a git URL is one on Unix) would be
        // read against the working directory.
        let got = p
            .source
            .as_deref()
            .map(Path::new)
            .filter(|s| s.is_absolute())
            .and_then(|s| s.canonicalize().ok());
        if want.is_none() || got != want {
            bad.insert(format!(
                "{} ({:?}): not at its workspace path {dir}",
                p.name, p.source
            ));
        }
    }
    bad.into_iter().collect()
}

/// The `cargo tree` arguments after the roots. `tree_args_are_exactly_these` pins the list, so
/// no flag can be dropped or narrowed (`--target all` to a set of triples, R3.1) without a test
/// failing; the fixture control shows what each one is for.
const TREE_ARGS: &[&str] = &[
    "-e",
    "normal,build",
    "--all-features",
    "--target",
    "all",
    "--prefix",
    "none",
    "--no-dedupe",
    "--format",
    "{p}",
    "--charset",
    "ascii",
    "--color",
    "never",
    "--locked",
    "--offline",
];

/// Runs `cargo tree` for `roots` with every feature of every root, on every target, normal and
/// build edges, no deduplication. Fails on a non-zero exit.
fn resolved(roots: &[&str]) -> Result<Vec<Pkg>, String> {
    resolved_in(&workspace_root().join("Cargo.toml"), roots)
}

/// The one `cargo tree` command every graph test runs; `tree_args_are_exactly_these` reads its
/// arguments back with `Command::get_args` (S4.1).
fn tree_command(manifest: &Path, roots: &[&str]) -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.arg("tree").arg("--manifest-path").arg(manifest);
    for r in roots {
        cmd.args(["-p", r]);
    }
    cmd.args(TREE_ARGS);
    cmd
}

/// `resolved` for the workspace at `manifest` (the fixture control uses its own).
fn resolved_in(manifest: &Path, roots: &[&str]) -> Result<Vec<Pkg>, String> {
    let out = tree_command(manifest, roots)
        .output()
        .map_err(|e| format!("running cargo tree: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo tree failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let text = String::from_utf8(out.stdout).map_err(|e| format!("cargo tree output: {e}"))?;
    parse_tree(&text)
}

fn names(pkgs: &[Pkg]) -> BTreeSet<&str> {
    pkgs.iter().map(|p| p.name.as_str()).collect()
}

/// The gate: jpdf-cli and every allowed crate as roots, so `--all-features` turns on every
/// feature of each of them (a feature of jpdf-engine that pulls in a decoder is caught even if
/// only another workspace member would enable it).
#[test]
fn cli_graph_is_parse_only() {
    let roots: Vec<&str> = ALLOWED.iter().map(|(n, _)| *n).collect();
    let pkgs = resolved(&roots).unwrap();
    let bad = violations(&pkgs, &workspace_root());
    assert!(
        bad.is_empty(),
        "jpdf-cli's graph reaches crates outside the #146 gate (specs/core/memory-budget.md, \
         Gate):\n{}",
        bad.join("\n")
    );
}

/// From jpdf-cli alone the graph is exactly the allowed set: this proves the listing is
/// transitive (jpdf-traits sits three edges below jpdf-cli) and keeps `ALLOWED` from holding
/// crates the CLI no longer uses.
#[test]
fn cli_graph_is_exactly_the_allowed_set() {
    let pkgs = resolved(&["jpdf-cli"]).unwrap();
    let want: BTreeSet<&str> = ALLOWED.iter().map(|(n, _)| *n).collect();
    assert_eq!(names(&pkgs), want);
    assert!(violations(&pkgs, &workspace_root()).is_empty());
}

/// Control: the same command and check on a real graph that has crates outside the list
/// (xtask depends on jpdf-arlington and jpdf-testgen, and on jpdf-fsio, which is allowed).
#[test]
fn control_a_real_graph_outside_the_list_is_reported() {
    let pkgs = resolved(&["xtask"]).unwrap();
    let bad = violations(&pkgs, &workspace_root());
    for name in ["xtask", "jpdf-arlington", "jpdf-testgen"] {
        assert!(
            bad.iter()
                .any(|b| b.starts_with(&format!("{name} ("))
                    && b.ends_with("not a parse-only crate")),
            "{name} not reported: {bad:?}"
        );
    }
    assert!(
        !bad.iter().any(|b| b.starts_with("jpdf-fsio (")),
        "allowed crate reported: {bad:?}"
    );
}

#[test]
fn tree_args_are_exactly_these() {
    let cmd = tree_command(Path::new("/m/Cargo.toml"), &["a", "b"]);
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args.join(" "),
        "tree --manifest-path /m/Cargo.toml -p a -p b -e normal,build --all-features --target all \
         --prefix none --no-dedupe --format {p} --charset ascii --color never --locked --offline"
    );
}

#[test]
fn control_cargo_tree_failure_is_an_error() {
    let err = resolved(&["no-such-package-jpdf"]).unwrap_err();
    assert!(err.starts_with("cargo tree failed"), "{err}");
}

/// Writes a fixture workspace under `CARGO_TARGET_TMPDIR`: `root` depends on `mid`, which has
/// an optional `opt` behind feature `f`; `root` has a `cfg(windows)` dependency `win`, a
/// `cfg(target_os = "linux")` dependency `lin` (no single target has both) and a
/// build-dependency `bld`. Returns the root manifest path after generating its lockfile.
fn fixture_workspace() -> std::io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("dependency-gate-fixture-{}", std::process::id()));
    let pkg = |name: &str, extra: &str| -> std::io::Result<()> {
        let d = if name == "root" {
            dir.clone()
        } else {
            dir.join(name)
        };
        std::fs::create_dir_all(d.join("src"))?;
        std::fs::write(d.join("src/lib.rs"), "")?;
        std::fs::write(
            d.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
                 publish = false\n{extra}"
            ),
        )
    };
    for leaf in ["opt", "win", "lin", "bld"] {
        pkg(leaf, "")?;
    }
    pkg(
        "mid",
        "\n[dependencies]\nopt = { path = \"../opt\", optional = true }\n\n\
         [features]\nf = [\"dep:opt\"]\n",
    )?;
    pkg(
        "root",
        "build = \"build.rs\"\n\n[workspace]\n\n[dependencies]\nmid = { path = \"mid\" }\n\n\
         [target.'cfg(windows)'.dependencies]\nwin = { path = \"win\" }\n\n\
         [target.'cfg(target_os = \"linux\")'.dependencies]\nlin = { path = \"lin\" }\n\n\
         [build-dependencies]\nbld = { path = \"bld\" }\n",
    )?;
    std::fs::write(dir.join("build.rs"), "fn main() {}\n")?;
    let manifest = dir.join("Cargo.toml");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let st = Command::new(cargo)
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(&manifest)
        .status()?;
    if !st.success() {
        return Err(std::io::Error::other(format!("generate-lockfile: {st}")));
    }
    Ok(manifest)
}

/// Control for the command's flags: on the fixture, the feature-gated optional dependency
/// (`--all-features` with `mid` as a root), the `cfg(windows)` and `cfg(target_os = "linux")`
/// dependencies in one run (`--target all`: no single target or host has both) and
/// the build-dependency (`-e build`) are each listed. Dropping any of those flags fails here.
#[test]
fn control_flags_reach_features_other_targets_and_build_edges() {
    let manifest = fixture_workspace().unwrap();
    let pkgs = resolved_in(&manifest, &["root", "mid"]).unwrap();
    let got = names(&pkgs);
    for want in ["root", "mid", "opt", "win", "lin", "bld"] {
        assert!(got.contains(want), "{want} not listed: {got:?}");
    }
    // Near miss: from `root` alone, `f` is off and `opt` is not listed (so the gate needs every
    // allowed crate as a root).
    let pkgs = resolved_in(&manifest, &["root"]).unwrap();
    assert!(!names(&pkgs).contains("opt"), "{pkgs:?}");
    // `--locked`: a manifest edit the lockfile doesn't record (a new edge `mid` → `bld`) fails.
    let mid = manifest.with_file_name("mid").join("Cargo.toml");
    let text = std::fs::read_to_string(&mid).unwrap();
    std::fs::write(
        &mid,
        text.replace(
            "[dependencies]\n",
            "[dependencies]\nbld = { path = \"../bld\" }\n",
        ),
    )
    .unwrap();
    let err = resolved_in(&manifest, &["root", "mid"]).unwrap_err();
    assert!(err.starts_with("cargo tree failed"), "{err}");
    // The fixture is removed only after a pass, so a failing run leaves it to inspect.
    if let Some(dir) = manifest.parent() {
        std::fs::remove_dir_all(dir).unwrap();
    }
}

/// The `cargo metadata` arguments after the manifest path; `metadata_args_are_exactly_these`
/// pins them. `--no-deps` lists the workspace members only, which is where every allowed crate
/// must be (the graph tests check that); no `--filter-platform`, so every target is listed.
const METADATA_ARGS: &[&str] = &[
    "--format-version",
    "1",
    "--no-deps",
    "--locked",
    "--offline",
];

/// The one `cargo metadata` command the target-path tests run (#235).
fn metadata_command(manifest: &Path) -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.arg("metadata").arg("--manifest-path").arg(manifest);
    cmd.args(METADATA_ARGS);
    cmd
}

/// Runs `cargo metadata` on the workspace at `manifest` and parses its JSON. Fails on a
/// non-zero exit.
fn metadata_in(manifest: &Path) -> Result<Value, String> {
    let out = metadata_command(manifest)
        .output()
        .map_err(|e| format!("running cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let text = String::from_utf8(out.stdout).map_err(|e| format!("cargo metadata output: {e}"))?;
    json::parse(&text).map_err(|e| format!("cargo metadata output: {e}"))
}

/// An absolute path, canonicalized; `None` for a relative path (never resolved against the
/// working directory) or one that doesn't resolve.
fn canonical_absolute(p: &str) -> Option<PathBuf> {
    let p = Path::new(p);
    if p.is_absolute() {
        p.canonicalize().ok()
    } else {
        None
    }
}

/// Returns one violation line per allowed crate that `cargo metadata` doesn't list exactly once
/// at its directory (by its manifest path), and per target (lib, bin, custom-build, test, …) of
/// an allowed crate whose `src_path`, canonicalized, is not inside that crate's directory,
/// canonicalized (compared by path components, so `crates/a-x` is not inside `crates/a`). A
/// manifest `[lib] path`, `[[bin]] path` or `build = …` into another crate's sources, or a target
/// root that is a symlink out of the directory, is a violation (#235). A metadata shape it
/// doesn't know (no `packages` array, a target without a string `src_path` or a `kind` array of
/// strings) is an error, so a format change fails the gate instead of passing it.
fn target_violations(
    meta: &Value,
    root: &Path,
    allowed: &[(&str, &str)],
) -> Result<Vec<String>, String> {
    let shape = |what: &str| format!("unexpected cargo metadata shape: {what}");
    let pkgs = meta
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| shape("no packages array"))?;
    let mut bad = BTreeSet::new();
    for &(name, dir) in allowed {
        let Some(want) = root.join(dir).canonicalize().ok() else {
            bad.insert(format!("{name}: its workspace path {dir} doesn't resolve"));
            continue;
        };
        let mut found = Vec::new();
        for p in pkgs {
            let pname = p
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| shape("package without a name"))?;
            let manifest = p
                .get("manifest_path")
                .and_then(Value::as_str)
                .ok_or_else(|| shape("package without a manifest_path"))?;
            let at = canonical_absolute(manifest);
            if pname == name && at.as_deref().and_then(Path::parent) == Some(want.as_path()) {
                found.push(p);
            }
        }
        let [pkg] = found.as_slice() else {
            bad.insert(format!(
                "{name}: listed {} times at its workspace path {dir}",
                found.len()
            ));
            continue;
        };
        let targets = pkg
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| shape("package without a targets array"))?;
        if targets.is_empty() {
            bad.insert(format!("{name}: no targets"));
        }
        for t in targets {
            let tname = t
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| shape("target without a name"))?;
            let kinds = t
                .get("kind")
                .and_then(Value::as_array)
                .ok_or_else(|| shape("target without a kind array"))?
                .iter()
                .map(|k| k.as_str().ok_or_else(|| shape("non-string target kind")))
                .collect::<Result<Vec<_>, _>>()?
                .join(",");
            let src = t
                .get("src_path")
                .and_then(Value::as_str)
                .ok_or_else(|| shape("target without a src_path"))?;
            if !canonical_absolute(src).is_some_and(|s| s.starts_with(&want)) {
                bad.insert(format!(
                    "{name}: target {tname} ({kinds}) compiles {src}, outside {dir}"
                ));
            }
        }
    }
    Ok(bad.into_iter().collect())
}

/// The gate's second half (#235): every target of every allowed crate compiles from inside that
/// crate's directory, which `cargo tree` doesn't show.
#[test]
fn cli_crates_compile_only_their_own_sources() {
    let meta = metadata_in(&workspace_root().join("Cargo.toml")).unwrap();
    let bad = target_violations(&meta, &workspace_root(), ALLOWED).unwrap();
    assert!(
        bad.is_empty(),
        "an allowed crate compiles sources from outside its directory (#146 gate, #235):\n{}",
        bad.join("\n")
    );
}

#[test]
fn metadata_args_are_exactly_these() {
    let cmd = metadata_command(Path::new("/m/Cargo.toml"));
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args.join(" "),
        "metadata --manifest-path /m/Cargo.toml --format-version 1 --no-deps --locked --offline"
    );
}

#[test]
fn control_cargo_metadata_failure_is_an_error() {
    let err = metadata_in(Path::new("/nonexistent-jpdf/Cargo.toml")).unwrap_err();
    assert!(err.starts_with("cargo metadata failed"), "{err}");
}

/// Writes a fixture workspace under `CARGO_TARGET_TMPDIR` whose members point targets at a
/// `victim` crate's sources: `libout` (`[lib] path`), `binout` (`[[bin]] path`), `buildout`
/// (`build = …`), `pre` (`[lib] path` into the sibling `pre-x`, a string-prefix near miss), and
/// on unix `linkout`, whose own `src/lib.rs` is a symlink to the victim's. `detour` points its
/// `[lib] path` back into itself through `..` (must pass). Returns the root manifest path.
fn target_fixture() -> std::io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("dependency-gate-targets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let pkg = |name: &str, extra: &str| -> std::io::Result<()> {
        let d = dir.join(name);
        std::fs::create_dir_all(d.join("src"))?;
        std::fs::write(d.join("src/lib.rs"), "")?;
        std::fs::write(
            d.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
                 publish = false\n{extra}"
            ),
        )
    };
    pkg("victim", "")?;
    std::fs::write(dir.join("victim/src/main.rs"), "fn main() {}\n")?;
    std::fs::write(dir.join("victim/build.rs"), "fn main() {}\n")?;
    pkg("libout", "\n[lib]\npath = \"../victim/src/lib.rs\"\n")?;
    pkg(
        "binout",
        "\n[[bin]]\nname = \"binout\"\npath = \"../victim/src/main.rs\"\n",
    )?;
    pkg("buildout", "build = \"../victim/build.rs\"\n")?;
    pkg("pre-x", "")?;
    pkg("pre", "\n[lib]\npath = \"../pre-x/src/lib.rs\"\n")?;
    pkg("detour", "\n[lib]\npath = \"../detour/src/lib.rs\"\n")?;
    let mut members = vec![
        "victim", "libout", "binout", "buildout", "pre-x", "pre", "detour",
    ];
    #[cfg(unix)]
    {
        pkg("linkout", "")?;
        std::fs::remove_file(dir.join("linkout/src/lib.rs"))?;
        std::os::unix::fs::symlink(
            dir.join("victim/src/lib.rs"),
            dir.join("linkout/src/lib.rs"),
        )?;
        members.push("linkout");
    }
    let list: Vec<String> = members.iter().map(|m| format!("\"{m}\"")).collect();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"3\"\nmembers = [{}]\n",
            list.join(", ")
        ),
    )?;
    let manifest = dir.join("Cargo.toml");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let st = Command::new(cargo)
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(&manifest)
        .status()?;
    if !st.success() {
        return Err(std::io::Error::other(format!("generate-lockfile: {st}")));
    }
    Ok(manifest)
}

/// Control on real `cargo metadata` output: each kind of target that compiles from outside its
/// crate is exactly one violation naming that crate and kind; the crate it reaches into, the
/// `..` detour back into itself and the prefix sibling's own target pass.
#[test]
fn control_target_paths_outside_the_crate_are_reported() {
    let manifest = target_fixture().unwrap();
    let root = manifest.parent().unwrap().to_path_buf();
    let meta = metadata_in(&manifest).unwrap();
    let mut cases = vec![
        ("libout", "(lib)"),
        ("binout", "(bin)"),
        ("buildout", "(custom-build)"),
        ("pre", "(lib)"),
    ];
    if cfg!(unix) {
        cases.push(("linkout", "(lib)"));
    }
    for (name, kind) in cases {
        let bad = target_violations(&meta, &root, &[(name, name)]).unwrap();
        assert_eq!(bad.len(), 1, "{name}: {bad:?}");
        assert!(
            bad[0].starts_with(&format!("{name}: target ")) && bad[0].contains(kind),
            "{name}: {bad:?}"
        );
    }
    for name in ["victim", "detour", "pre-x"] {
        let bad = target_violations(&meta, &root, &[(name, name)]).unwrap();
        assert!(bad.is_empty(), "{name}: {bad:?}");
    }
    std::fs::remove_dir_all(&root).unwrap();
}

// Parser and checker cases on synthetic output: each one should-fail case has a near miss that
// must pass. `#[cfg(test)]` is what lets clippy's allow-unwrap-in-tests (clippy.toml) cover the
// helpers here, which are not `#[test]` functions.
#[cfg(test)]
mod synthetic {
    use super::*;

    fn root() -> PathBuf {
        workspace_root().canonicalize().unwrap()
    }

    fn line(name: &str, dir: &str) -> String {
        format!("{name} v0.0.0 ({})", root().join(dir).display())
    }

    #[test]
    fn parser_reads_path_registry_and_git_lines() {
        let out = format!(
            "{}\n\nserde v1.0.200\nfoo v0.1.0 (https://example.invalid/foo?rev=1#abcdef)\n",
            line("jpdf-cli", "tools/jpdf-cli")
        );
        let pkgs = parse_tree(&out).unwrap();
        assert_eq!(
            pkgs,
            vec![
                Pkg {
                    name: "jpdf-cli".into(),
                    source: Some(root().join("tools/jpdf-cli").display().to_string()),
                },
                Pkg {
                    name: "serde".into(),
                    source: None,
                },
                Pkg {
                    name: "foo".into(),
                    source: Some("https://example.invalid/foo?rev=1#abcdef".into()),
                },
            ]
        );
    }

    #[test]
    fn parser_rejects_unknown_shapes_and_empty_output() {
        for bad in [
            "",
            "\n\n",
            "jpdf-cli",
            " v0.0.0 (/x)",
            "jpdf cli v0.0.0 (/x)",
            "jpdf-cli v0.0.0 /x",
            "jpdf-cli v0.0.0 (/x",
            // A missing or malformed version (S1.3), with and without a source.
            "jpdf-cli v",
            "jpdf-cli v (/x)",
            "jpdf-cli v  (/x)",
            "jpdf-cli vx.0 (/x)",
            "jpdf-cli v0.0.0/x (/x)",
        ] {
            assert!(parse_tree(bad).is_err(), "accepted {bad:?}");
        }
        // Near misses: pre-release and build-metadata versions parse.
        for good in ["jpdf-cli v0.0.0-rc.1+b2 (/x)", "serde v1.0.200"] {
            assert!(parse_tree(good).is_ok(), "rejected {good:?}");
        }
    }

    /// A checkout directory with parentheses in its name is a valid source path (S1.2): the
    /// source is everything between the first ` (` after the version and the final `)`.
    #[test]
    fn parser_keeps_parentheses_inside_a_path() {
        let pkgs = parse_tree("jpdf-cli v0.0.0 (/work/jpdf (copy)/tools/jpdf-cli)").unwrap();
        assert_eq!(
            pkgs[0].source.as_deref(),
            Some("/work/jpdf (copy)/tools/jpdf-cli")
        );
        // S3.1: adjacent parenthesized directory names (`) (` inside the path) still parse.
        let pkgs = parse_tree("jpdf-cli v0.0.0 (/work/jpdf (old) (copy)/tools/jpdf-cli)").unwrap();
        assert_eq!(
            pkgs[0].source.as_deref(),
            Some("/work/jpdf (old) (copy)/tools/jpdf-cli")
        );
    }

    /// A marker before the source (Cargo prints `<name> v<ver> (proc-macro) (<path>)`) or after
    /// it (`(*)`) is a parse error, on a dummy path and on the allowed path itself.
    #[test]
    fn parser_rejects_marker_shapes() {
        let path = root().join("crates/jpdf-syntax").display().to_string();
        for p in ["/x", path.as_str()] {
            for c in [
                format!("jpdf-syntax v0.0.0 ({p}) (*)"),
                format!("jpdf-syntax v0.0.0 ({p}) (proc-macro)"),
                format!("jpdf-syntax v0.0.0 (proc-macro) ({p})"),
                // S4.2: a registry proc-macro or deduplicated line has a marker and no source.
                "serde_derive v1.0.217 (proc-macro)".to_owned(),
                "serde v1.0.217 (*)".to_owned(),
            ] {
                assert!(parse_tree(&c).is_err(), "accepted {c:?}");
            }
        }
    }

    /// S2.2: a directory literally named `jpdf-syntax) (*` that is a symlink to the allowed
    /// directory must not make the `(*)` line pass. Control: the alias does resolve there.
    #[cfg(unix)]
    #[test]
    fn marker_line_through_a_symlink_alias_is_refused() {
        let r = root();
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("dependency-gate-alias-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let alias = dir.join("jpdf-syntax) (*");
        let _ = std::fs::remove_file(&alias);
        std::os::unix::fs::symlink(r.join("crates/jpdf-syntax"), &alias).unwrap();
        assert_eq!(
            alias.canonicalize().unwrap(),
            r.join("crates/jpdf-syntax").canonicalize().unwrap()
        );
        let c = format!("jpdf-syntax v0.0.0 ({}/jpdf-syntax) (*)", dir.display());
        let refused = match parse_tree(&c) {
            Err(_) => true,
            Ok(pkgs) => violations(&pkgs, &r).len() == 1,
        };
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(refused, "accepted {c:?}");
    }

    /// A relative source (a git URL is one on Unix) is never resolved against the working
    /// directory (S1.6). Tests run in the package directory, so `../../crates/jpdf-syntax` names
    /// the allowed directory from there (the control) and must still be refused.
    #[test]
    fn checker_rejects_relative_sources() {
        let r = root();
        let rel = "../../crates/jpdf-syntax";
        assert_eq!(
            Path::new(rel).canonicalize().unwrap(),
            r.join("crates/jpdf-syntax").canonicalize().unwrap()
        );
        let c = format!("jpdf-syntax v0.0.0 ({rel})");
        let pkgs = parse_tree(&c).unwrap();
        assert_eq!(violations(&pkgs, &r).len(), 1, "accepted {c:?}");
    }

    #[test]
    fn checker_accepts_every_allowed_crate_at_its_path() {
        let out: String = ALLOWED.iter().map(|(n, d)| line(n, d) + "\n").collect();
        let pkgs = parse_tree(&out).unwrap();
        assert_eq!(pkgs.len(), ALLOWED.len());
        assert_eq!(violations(&pkgs, &root()), Vec::<String>::new());
    }

    #[test]
    fn checker_rejects_each_kind_of_outsider() {
        let r = root();
        let cases = [
            // Not on the list (a decode crate in the workspace).
            line("jpdf-filters", "crates/jpdf-filters"),
            // Name on the list but from the registry, or from git.
            "jpdf-syntax v0.0.0".to_owned(),
            "jpdf-syntax v0.0.0 (https://example.invalid/x#1)".to_owned(),
            // Name on the list at another crate's workspace path.
            line("jpdf-syntax", "crates/jpdf-cos"),
            // Name on the list at a path that doesn't exist.
            line("jpdf-syntax", "crates/jpdf-syntax-x"),
        ];
        for c in cases {
            let pkgs = parse_tree(&c).unwrap();
            assert_eq!(violations(&pkgs, &r).len(), 1, "accepted {c:?}");
        }
        // Near miss: the same name at its own path, written through a `..` detour.
        let detour = format!(
            "jpdf-syntax v0.0.0 ({})",
            r.join("crates/jpdf-cos/../jpdf-syntax").display()
        );
        assert!(violations(&parse_tree(&detour).unwrap(), &r).is_empty());
    }

    /// A `cargo metadata` document with one package `name` whose manifest sits in `dir` and
    /// whose targets are the given `(kind, src_path)` pairs.
    fn meta(name: &str, dir: &Path, targets: &[(&str, &str)]) -> Value {
        let q = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
        let ts: Vec<String> = targets
            .iter()
            .map(|(k, s)| {
                format!(
                    "{{\"name\":\"t\",\"kind\":[{}],\"src_path\":{}}}",
                    q(k),
                    q(s)
                )
            })
            .collect();
        let manifest = dir.join("Cargo.toml").display().to_string();
        json::parse(&format!(
            "{{\"packages\":[{{\"name\":{},\"manifest_path\":{},\"targets\":[{}]}}]}}",
            q(name),
            q(&manifest),
            ts.join(",")
        ))
        .unwrap()
    }

    const SYNTAX: &[(&str, &str)] = &[("jpdf-syntax", "crates/jpdf-syntax")];

    #[test]
    fn target_checker_accepts_own_sources_and_rejects_each_outside_one() {
        let r = root();
        let d = r.join("crates/jpdf-syntax");
        let own = d.join("src/lib.rs").display().to_string();
        let ok = meta("jpdf-syntax", &d, &[("lib", &own)]);
        assert_eq!(
            target_violations(&ok, &r, SYNTAX).unwrap(),
            Vec::<String>::new()
        );
        let other = r
            .join("crates/jpdf-filters/src/lib.rs")
            .display()
            .to_string();
        let missing = d.join("src/no-such-file.rs").display().to_string();
        for src in [
            other.as_str(),
            missing.as_str(),
            "src/lib.rs",
            "../jpdf-syntax/src/lib.rs",
        ] {
            let m = meta("jpdf-syntax", &d, &[("lib", &own), ("custom-build", src)]);
            let bad = target_violations(&m, &r, SYNTAX).unwrap();
            assert_eq!(bad.len(), 1, "{src}: {bad:?}");
            assert!(bad[0].contains("(custom-build)"), "{bad:?}");
        }
    }

    #[test]
    fn target_checker_requires_the_crate_once_at_its_path() {
        let r = root();
        let d = r.join("crates/jpdf-syntax");
        let own = d.join("src/lib.rs").display().to_string();
        // Missing, at another crate's directory, under another name, and with no targets.
        for m in [
            json::parse("{\"packages\":[]}").unwrap(),
            meta("jpdf-syntax", &r.join("crates/jpdf-cos"), &[("lib", &own)]),
            meta("jpdf-syntax-x", &d, &[("lib", &own)]),
            meta("jpdf-syntax", &d, &[]),
        ] {
            assert_eq!(target_violations(&m, &r, SYNTAX).unwrap().len(), 1, "{m:?}");
        }
        // Listed twice.
        let one = meta("jpdf-syntax", &d, &[("lib", &own)]);
        let p = one.get("packages").and_then(Value::as_array).unwrap()[0].clone();
        let twice = Value::Object(vec![("packages".into(), Value::Array(vec![p.clone(), p]))]);
        let bad = target_violations(&twice, &r, SYNTAX).unwrap();
        assert_eq!(
            bad,
            vec!["jpdf-syntax: listed 2 times at its workspace path crates/jpdf-syntax"]
        );
    }

    #[test]
    fn target_checker_rejects_unknown_metadata_shapes() {
        let r = root();
        let m = r
            .join("crates/jpdf-syntax/Cargo.toml")
            .display()
            .to_string();
        let src = r
            .join("crates/jpdf-syntax/src/lib.rs")
            .display()
            .to_string();
        let pkg = |targets: &str| {
            format!(
                "{{\"packages\":[{{\"name\":\"jpdf-syntax\",\"manifest_path\":{m:?},{targets}}}]}}"
            )
        };
        for doc in [
            "{}".to_owned(),
            "{\"packages\":{}}".to_owned(),
            "{\"packages\":[{\"manifest_path\":\"/x\"}]}".to_owned(),
            "{\"packages\":[{\"name\":\"x\"}]}".to_owned(),
            pkg("\"targets\":{}"),
            pkg(&format!(
                "\"targets\":[{{\"kind\":[\"lib\"],\"src_path\":{src:?}}}]"
            )),
            pkg(&format!(
                "\"targets\":[{{\"name\":\"t\",\"kind\":\"lib\",\"src_path\":{src:?}}}]"
            )),
            pkg(&format!(
                "\"targets\":[{{\"name\":\"t\",\"kind\":[1],\"src_path\":{src:?}}}]"
            )),
            pkg("\"targets\":[{\"name\":\"t\",\"kind\":[\"lib\"]}]"),
        ] {
            let v = json::parse(&doc).unwrap();
            let e = target_violations(&v, &r, SYNTAX).unwrap_err();
            assert!(
                e.starts_with("unexpected cargo metadata shape"),
                "{doc}: {e}"
            );
        }
        // Near miss: the same shape with every field present passes.
        let good = pkg(&format!(
            "\"targets\":[{{\"name\":\"t\",\"kind\":[\"lib\"],\"src_path\":{src:?}}}]"
        ));
        let v = json::parse(&good).unwrap();
        assert_eq!(
            target_violations(&v, &r, SYNTAX).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn checker_reports_one_outsider_among_allowed_crates() {
        let mut out: String = ALLOWED.iter().map(|(n, d)| line(n, d) + "\n").collect();
        out.push_str(&line("jpdf-render", "crates/jpdf-render"));
        let bad = violations(&parse_tree(&out).unwrap(), &root());
        assert_eq!(bad.len(), 1, "{bad:?}");
        assert!(bad[0].starts_with("jpdf-render ("), "{bad:?}");
    }
}
