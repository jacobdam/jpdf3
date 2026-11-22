//! `jpdf-arlington check|generate|vendor` (specs/tooling/arlington.md, "CLI").
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jpdf_arlington::{Source, generate, load, vendor};

const USAGE: &str = "usage: jpdf-arlington check [--vendored DIR | --tsv DIR | --cache DIR]\n       \
     jpdf-arlington generate --out FILE [--vendored DIR | --tsv DIR | --cache DIR]\n       \
     jpdf-arlington vendor --out DIR [--cache DIR]";

/// Default cache for the pinned commit: `target/arlington` of the checkout this tool was
/// built from, whatever the current directory (spec item 6).
fn default_cache() -> PathBuf {
    repo_root().join("target/arlington")
}

/// Default vendored copy: [`vendor::DATA_DIR`] of the checkout this tool was built from.
fn default_vendored() -> PathBuf {
    repo_root().join(vendor::DATA_DIR)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A parsed command line: each command carries exactly the fields it needs, so `main`
/// never re-checks what `parse` guarantees.
#[derive(Debug, PartialEq)]
enum Args {
    Check(CliSource),
    Generate { source: CliSource, out: PathBuf },
    Vendor { cache: PathBuf, out: PathBuf },
}

#[derive(Debug, PartialEq)]
enum CliSource {
    Vendored(PathBuf),
    Cache(PathBuf),
    Tsv(PathBuf),
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut it = args.iter();
    let command = it.next().ok_or("missing command")?.clone();
    if !["check", "generate", "vendor"].contains(&command.as_str()) {
        return Err(format!("unknown command {command:?}"));
    }
    let vendor = command == "vendor";
    let (mut cache, mut tsv, mut vendored, mut out) = (None, None, None, None);
    while let Some(flag) = it.next() {
        let slot = match flag.as_str() {
            "--cache" => &mut cache,
            "--tsv" if !vendor => &mut tsv,
            "--vendored" if !vendor => &mut vendored,
            "--out" if command != "check" => &mut out,
            _ => return Err(format!("unknown flag {flag:?}")),
        };
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        if slot.replace(PathBuf::from(value)).is_some() {
            return Err(format!("{flag} given twice"));
        }
    }
    if vendor {
        // `--tsv` and `--vendored` were refused above.
        let out = out.ok_or("vendor needs --out")?;
        return Ok(Args::Vendor {
            cache: cache.unwrap_or_else(default_cache),
            out,
        });
    }
    let source = match (cache, tsv, vendored) {
        (None, None, None) => CliSource::Vendored(default_vendored()),
        (Some(c), None, None) => CliSource::Cache(c),
        (None, Some(t), None) => CliSource::Tsv(t),
        (None, None, Some(v)) => CliSource::Vendored(v),
        _ => return Err("give one of --vendored, --tsv or --cache".into()),
    };
    match out {
        // `--out` was refused above for `check`.
        None if command == "check" => Ok(Args::Check(source)),
        None => Err(format!("{command} needs --out")),
        Some(out) => Ok(Args::Generate { source, out }),
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse(&raw) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("jpdf-arlington: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let (source, out) = match args {
        Args::Vendor { cache, out } => return vendor_cmd(&cache, &out),
        Args::Check(source) => (source, None),
        Args::Generate { source, out } => (source, Some(out)),
    };
    let source = match source {
        CliSource::Vendored(v) => Source::Vendored(v),
        CliSource::Cache(c) => Source::Pinned(c),
        CliSource::Tsv(t) => Source::Dir(t),
    };
    let model = match load(&source) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("jpdf-arlington: {e}");
            return ExitCode::from(1);
        }
    };
    eprintln!(
        "jpdf-arlington: {}: {} objects, {} keys, checks pass",
        source.describe(),
        model.objects.len(),
        model.key_count()
    );
    if let Some(out) = out {
        let text = generate::render(&model, &source.describe());
        if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty())
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            eprintln!("jpdf-arlington: {}: {e}", parent.display());
            return ExitCode::from(1);
        }
        if let Err(e) = std::fs::write(&out, text) {
            eprintln!("jpdf-arlington: {}: {e}", out.display());
            return ExitCode::from(1);
        }
        eprintln!("jpdf-arlington: wrote {}", out.display());
    }
    ExitCode::SUCCESS
}

/// `vendor --out DIR [--cache DIR]`.
fn vendor_cmd(cache: &Path, out: &Path) -> ExitCode {
    match vendor::vendor_pinned(cache, out) {
        Ok(n) => {
            eprintln!(
                "jpdf-arlington: wrote {} ({n} files, git ids match the pins)",
                out.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("jpdf-arlington: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Result<Args, String> {
        parse(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn arguments() {
        assert_eq!(
            p(&["check"]),
            Ok(Args::Check(CliSource::Vendored(default_vendored())))
        );
        assert_eq!(
            p(&["check", "--cache", "c"]),
            Ok(Args::Check(CliSource::Cache("c".into())))
        );
        assert_eq!(
            p(&["check", "--vendored", "v"]),
            Ok(Args::Check(CliSource::Vendored("v".into())))
        );
        assert_eq!(
            p(&["check", "--tsv", "t"]),
            Ok(Args::Check(CliSource::Tsv("t".into())))
        );
        assert_eq!(
            p(&["vendor", "--out", "o"]),
            Ok(Args::Vendor {
                cache: default_cache(),
                out: "o".into()
            })
        );
        assert_eq!(
            p(&["vendor", "--cache", "c", "--out", "o"]),
            Ok(Args::Vendor {
                cache: "c".into(),
                out: "o".into()
            })
        );
        assert_eq!(
            p(&["generate", "--out", "x.rs", "--tsv", "d"]),
            Ok(Args::Generate {
                source: CliSource::Tsv("d".into()),
                out: "x.rs".into()
            })
        );
        assert_eq!(
            p(&["generate", "--out", "x.rs"]),
            Ok(Args::Generate {
                source: CliSource::Vendored(default_vendored()),
                out: "x.rs".into()
            })
        );
        assert_eq!(
            p(&["generate", "--cache", "c", "--out", "x.rs"]),
            Ok(Args::Generate {
                source: CliSource::Cache("c".into()),
                out: "x.rs".into()
            })
        );
        assert_eq!(
            p(&["generate", "--vendored", "v", "--out", "x.rs"]),
            Ok(Args::Generate {
                source: CliSource::Vendored("v".into()),
                out: "x.rs".into()
            })
        );
        assert_eq!(p(&["vendor"]), Err("vendor needs --out".into()));
        assert_eq!(p(&["generate"]), Err("generate needs --out".into()));
        for bad in [
            &[][..],
            &["fetch"],
            &["check", "--out", "x"],
            &["generate"],
            &["check", "--tsv"],
            &["check", "--tsv", "a", "--tsv", "b"],
            &["check", "--tsv", "a", "--cache", "b"],
            &["check", "--vendored", "a", "--tsv", "b"],
            &["check", "--vendored", "a", "--vendored", "b"],
            &["vendor"],
            &["vendor", "--out", "o", "--tsv", "d"],
            &["vendor", "--out", "o", "--vendored", "d"],
            &["check", "--bogus", "1"],
        ] {
            assert!(p(bad).is_err(), "{bad:?}");
        }
    }
}
