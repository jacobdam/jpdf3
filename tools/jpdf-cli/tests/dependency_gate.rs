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

/// Parses `cargo tree --prefix none --format {p}` output. Blank lines (between roots) are
/// skipped; any other line that is not `<name> v<version>[ (<source>)]` is an error, so an
/// output format change fails the gate instead of passing it. The version starts with a digit
/// and holds only ASCII alphanumerics, `.`, `-` and `+`. The source is everything between the
/// ` (` after the version and the line's final `)`, so a path may contain parentheses; a
/// marker before or after the source (`(proc-macro)` comes before it, `(*)` after it) stays in
/// the source text, which is then not an absolute allowed path and fails the path check.
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
                    .filter(|t| !t.is_empty())
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

/// Runs `cargo tree` for `roots` with every feature of every root, on every target, normal and
/// build edges, no deduplication. Fails on a non-zero exit.
fn resolved(roots: &[&str]) -> Result<Vec<Pkg>, String> {
    resolved_in(&workspace_root().join("Cargo.toml"), roots)
}

/// `resolved` for the workspace at `manifest` (the fixture control uses its own).
fn resolved_in(manifest: &Path, roots: &[&str]) -> Result<Vec<Pkg>, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.arg("tree").arg("--manifest-path").arg(manifest);
    for r in roots {
        cmd.args(["-p", r]);
    }
    cmd.args([
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
    ]);
    let out = cmd
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
fn control_cargo_tree_failure_is_an_error() {
    let err = resolved(&["no-such-package-jpdf"]).unwrap_err();
    assert!(err.starts_with("cargo tree failed"), "{err}");
}

/// Writes a fixture workspace under `CARGO_TARGET_TMPDIR`: `root` depends on `mid`, which has
/// an optional `opt` behind feature `f`; `root` has a `cfg(windows)` dependency `win` and a
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
    for leaf in ["opt", "win", "bld"] {
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
/// (`--all-features` with `mid` as a root), the `cfg(windows)` dependency (`--target all`) and
/// the build-dependency (`-e build`) are each listed. Dropping any of those flags fails here.
#[test]
fn control_flags_reach_features_other_targets_and_build_edges() {
    let manifest = fixture_workspace().unwrap();
    let pkgs = resolved_in(&manifest, &["root", "mid"]).unwrap();
    let got = names(&pkgs);
    for want in ["root", "mid", "opt", "win", "bld"] {
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
    }

    /// A marker before the source (Cargo prints `<name> v<ver> (proc-macro) (<path>)`) or after it
    /// (`(*)`) becomes part of the source text, which is then not the allowed path: still one
    /// violation, never a pass.
    #[test]
    fn checker_rejects_marker_suffixes() {
        let r = root();
        let path = r.join("crates/jpdf-syntax").display().to_string();
        for c in [
            format!("jpdf-syntax v0.0.0 ({path}) (*)"),
            format!("jpdf-syntax v0.0.0 ({path}) (proc-macro)"),
            format!("jpdf-syntax v0.0.0 (proc-macro) ({path})"),
        ] {
            let pkgs = parse_tree(&c).unwrap();
            assert_eq!(violations(&pkgs, &r).len(), 1, "accepted {c:?}");
        }
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

    #[test]
    fn checker_reports_one_outsider_among_allowed_crates() {
        let mut out: String = ALLOWED.iter().map(|(n, d)| line(n, d) + "\n").collect();
        out.push_str(&line("jpdf-render", "crates/jpdf-render"));
        let bad = violations(&parse_tree(&out).unwrap(), &root());
        assert_eq!(bad.len(), 1, "{bad:?}");
        assert!(bad[0].starts_with("jpdf-render ("), "{bad:?}");
    }
}
