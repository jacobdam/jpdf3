//! `cargo xtask fuzz [--short] [--seconds N] [--target NAME]`. Spec: specs/tooling/fuzz.md.
//!
//! Writes the jpdf-testgen seeds into `fuzz/corpus/<target>/`, builds the targets, then runs
//! every target from `cargo fuzz list` (or just `--target`) with the judge's memory limit.
//! Uses the dated nightly [`NIGHTLY`], so a broken nightly release cannot turn PRs red.

use std::path::Path;
use std::process::Command;

/// Seconds per target: `--short` for local and PR use, the default as in the nightly judge.
pub const SHORT_SECONDS: u32 = 30;
pub const NIGHTLY_SECONDS: u32 = 300;

/// The judge's address-space budget (CONTRACT §3 runs with `-rss_limit_mb=2048`).
const RSS_LIMIT_MB: u32 = 2048;
/// One input taking longer than this is a hang, which counts as a crash.
const INPUT_TIMEOUT_SECONDS: u32 = 10;

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub seconds: u32,
    pub target: Option<String>,
}

/// Parses the arguments after `fuzz`.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts = Options {
        seconds: NIGHTLY_SECONDS,
        target: None,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--short" => opts.seconds = SHORT_SECONDS,
            "--seconds" => {
                opts.seconds = iter
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|n| (1..=3600).contains(n))
                    .ok_or("--seconds needs an integer in 1..=3600")?;
            }
            "--target" => {
                let name = iter.next().ok_or("--target needs a name")?;
                opts.target = Some(name.clone());
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(opts)
}

/// Seed sets per target. A target missing here starts from an empty corpus.
pub fn seeds_for(target: &str) -> Vec<(&'static str, Vec<u8>)> {
    match target {
        "engine_open" => jpdf_testgen::engine_open_seeds(),
        // Both template targets read the chunk format.
        "engine_objects" => jpdf_testgen::engine_objects_seeds(),
        // Reads its input both raw and through the template: give it both seed sets.
        "syntax_resolve" => {
            let mut seeds = jpdf_testgen::engine_open_seeds();
            seeds.extend(jpdf_testgen::engine_objects_seeds());
            seeds
        }
        _ => Vec::new(),
    }
}

/// Writes seeds as `seed-<name>` files, overwriting earlier seed files and keeping whatever
/// the fuzzer added to the corpus.
pub fn write_seeds(fuzz_dir: &Path, target: &str) -> Result<usize, String> {
    let seeds = seeds_for(target);
    if seeds.is_empty() {
        return Ok(0);
    }
    let dir = fuzz_dir.join("corpus").join(target);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    for (name, bytes) in &seeds {
        let path = dir.join(format!("seed-{name}"));
        std::fs::write(&path, bytes)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(seeds.len())
}

/// The nightly toolchain for our fuzz runs (the judge's own nightly step is not pinned).
/// Bump it in an audit, like `rust-toolchain.toml`.
pub const NIGHTLY: &str = "nightly-2026-11-20";

fn plus_nightly() -> String {
    format!("+{NIGHTLY}")
}

/// cargo-fuzz version CI installs (`--locked`), so a new release cannot change PR results.
const CARGO_FUZZ_VERSION: &str = "0.13.2";

/// The short run inside `cargo xtask ci linux`. On CI (`CI` set) missing tools are installed
/// first; locally a missing tool is an error with the install hint.
pub fn ci(root: &Path) -> Result<(), String> {
    // Check before installing anything (the xtask tests run `ci linux` in fixture trees).
    require_project(root)?;
    if std::env::var_os("CI").is_some() && !tools_present(root) {
        install_tools()?;
    }
    // fuzz/ is outside the workspace, so `cargo fmt --all` does not see it.
    let fmt = Command::new("cargo")
        .args(["fmt", "--manifest-path", "fuzz/Cargo.toml", "--check"])
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run cargo fmt: {e}"))?;
    if !fmt.success() {
        return Err(
            "fuzz/ is not formatted (run `cargo fmt --manifest-path fuzz/Cargo.toml`)".into(),
        );
    }
    run(
        root,
        &Options {
            seconds: SHORT_SECONDS,
            target: None,
        },
    )
}

fn tools_present(root: &Path) -> bool {
    Command::new("cargo")
        .arg(plus_nightly())
        .args(["fuzz", "--version"])
        .current_dir(root)
        .output()
        .is_ok_and(|o| o.status.success())
}

fn install_tools() -> Result<(), String> {
    let steps: [&[&str]; 2] = [
        &[
            "rustup",
            "toolchain",
            "install",
            NIGHTLY,
            "--profile",
            "minimal",
        ],
        &[
            "cargo",
            "install",
            "cargo-fuzz",
            "--locked",
            "--version",
            CARGO_FUZZ_VERSION,
        ],
    ];
    for step in steps {
        let (program, rest) = step.split_first().ok_or("empty install step")?;
        let ok = Command::new(program)
            .args(rest)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return Err(format!("installing fuzz tools failed: {}", step.join(" ")));
        }
    }
    Ok(())
}

fn require_project(root: &Path) -> Result<(), String> {
    let manifest = root.join("fuzz").join("Cargo.toml");
    if manifest.is_file() {
        Ok(())
    } else {
        Err(format!(
            "{} not found (run from the repository root)",
            manifest.display()
        ))
    }
}

pub fn run(root: &Path, opts: &Options) -> Result<(), String> {
    require_project(root)?;
    let fuzz_dir = root.join("fuzz");
    let list = Command::new("cargo")
        .arg(plus_nightly())
        .args(["fuzz", "list"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !list.status.success() {
        return Err(format!(
            "`cargo +{NIGHTLY} fuzz list` failed (install: `rustup toolchain install {NIGHTLY} \
             --profile minimal` and `cargo install cargo-fuzz --locked --version \
             {CARGO_FUZZ_VERSION}`): {}",
            String::from_utf8_lossy(&list.stderr).trim()
        ));
    }
    let all: Vec<String> = String::from_utf8_lossy(&list.stdout)
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let targets: Vec<String> = match &opts.target {
        Some(t) if all.contains(t) => vec![t.clone()],
        Some(t) => return Err(format!("unknown fuzz target {t:?}; targets: {all:?}")),
        None => all,
    };
    // Build first, so a toolchain or compile failure is not reported as a crash.
    let build = Command::new("cargo")
        .arg(plus_nightly())
        .args(["fuzz", "build", "--debug-assertions"])
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run cargo fuzz: {e}"))?;
    if !build.success() {
        return Err(format!(
            "fuzz targets do not build with {NIGHTLY} (a build failure, not a crash)"
        ));
    }
    let mut failed = Vec::new();
    for t in &targets {
        let n = write_seeds(&fuzz_dir, t)?;
        eprintln!("xtask: fuzz: {t}: {n} seed(s), {} s", opts.seconds);
        let status = Command::new("cargo")
            .arg(plus_nightly())
            .args(["fuzz", "run", "--debug-assertions", t, "--"])
            .arg(format!("-max_total_time={}", opts.seconds))
            .arg(format!("-rss_limit_mb={RSS_LIMIT_MB}"))
            .arg(format!("-timeout={INPUT_TIMEOUT_SECONDS}"))
            .current_dir(root)
            .status()
            .map_err(|e| format!("cannot run cargo fuzz: {e}"))?;
        if !status.success() {
            failed.push(t.clone());
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "crash, hang or out-of-memory in {failed:?}; artifacts are in fuzz/artifacts/ \
             (minimize with `cargo +{NIGHTLY} fuzz tmin`, then add the repro to tests/regress/)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_options() {
        assert_eq!(
            parse(&args(&[])),
            Ok(Options {
                seconds: NIGHTLY_SECONDS,
                target: None
            })
        );
        assert_eq!(parse(&args(&["--short"])).map(|o| o.seconds), Ok(30));
        assert_eq!(
            parse(&args(&["--seconds", "5", "--target", "engine_open"])),
            Ok(Options {
                seconds: 5,
                target: Some("engine_open".to_owned())
            })
        );
        for bad in [
            &["--seconds"][..],
            &["--seconds", "0"],
            &["--seconds", "3601"],
            &["--seconds", "x"],
            &["--target"],
            &["--long"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn every_listed_target_has_seeds() {
        // fuzz/Cargo.toml's [[bin]] names. A new target without seeds fails here on purpose:
        // add its seeds to `seeds_for` (or an explicit empty arm with a reason).
        let manifest = include_str!("../../fuzz/Cargo.toml");
        let names: Vec<&str> = manifest
            .lines()
            .filter_map(|l| l.strip_prefix("name = \""))
            .filter_map(|l| l.strip_suffix('"'))
            .filter(|n| *n != "jpdf-fuzz")
            .collect();
        assert_eq!(names, ["engine_open", "engine_objects", "syntax_resolve"]);
        for n in names {
            assert!(!seeds_for(n).is_empty(), "{n} has no seeds");
        }
    }

    #[test]
    fn writes_seed_files_and_keeps_other_corpus_files() {
        let dir = std::env::temp_dir().join(format!("xtask-fuzz-{}", std::process::id()));
        let corpus = dir.join("corpus/engine_open");
        std::fs::create_dir_all(&corpus).ok();
        std::fs::write(corpus.join("found-by-fuzzer"), b"x").ok();
        assert_eq!(write_seeds(&dir, "engine_open"), Ok(6));
        assert!(corpus.join("seed-one-page.pdf").is_file());
        assert!(corpus.join("found-by-fuzzer").is_file());
        assert_eq!(write_seeds(&dir, "no_such_target"), Ok(0));
        std::fs::remove_dir_all(&dir).ok();
    }
}
