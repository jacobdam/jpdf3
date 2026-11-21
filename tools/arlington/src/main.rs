//! `jpdf-arlington check|generate|vendor` (specs/tooling/arlington.md, "CLI").
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use jpdf_arlington::{Source, generate, load, vendor};

const USAGE: &str = "usage: jpdf-arlington check [--vendored DIR | --tsv DIR | --cache DIR]\n       \
     jpdf-arlington generate --out FILE [--vendored DIR | --tsv DIR | --cache DIR]\n       \
     jpdf-arlington vendor --out DIR [--cache DIR]";

/// Default cache for the pinned commit, relative to the current directory.
const DEFAULT_CACHE: &str = "target/arlington";

#[derive(Debug, PartialEq)]
struct Args {
    command: String,
    source: CliSource,
    out: Option<PathBuf>,
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
    let source = match (cache, tsv, vendored) {
        (None, None, None) if vendor => CliSource::Cache(PathBuf::from(DEFAULT_CACHE)),
        (None, None, None) => CliSource::Vendored(PathBuf::from(vendor::DATA_DIR)),
        (Some(c), None, None) => CliSource::Cache(c),
        (None, Some(t), None) => CliSource::Tsv(t),
        (None, None, Some(v)) => CliSource::Vendored(v),
        _ => return Err("give one of --vendored, --tsv or --cache".into()),
    };
    if command != "check" && out.is_none() {
        return Err(format!("{command} needs --out"));
    }
    Ok(Args {
        command,
        source,
        out,
    })
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
    if args.command == "vendor" {
        let (CliSource::Cache(cache), Some(out)) = (&args.source, &args.out) else {
            eprintln!("jpdf-arlington: vendor needs --out\n{USAGE}");
            return ExitCode::from(2);
        };
        return match vendor::vendor_pinned(cache, out) {
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
        };
    }
    let source = match args.source {
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
    if let Some(out) = args.out {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &[&str]) -> Result<Args, String> {
        parse(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn arguments() {
        assert_eq!(
            p(&["check"]).map(|a| a.source),
            Ok(CliSource::Vendored(PathBuf::from(vendor::DATA_DIR)))
        );
        assert_eq!(
            p(&["check", "--cache", "c"]).map(|a| a.source),
            Ok(CliSource::Cache("c".into()))
        );
        assert_eq!(
            p(&["check", "--vendored", "v"]).map(|a| a.source),
            Ok(CliSource::Vendored("v".into()))
        );
        assert_eq!(
            p(&["vendor", "--out", "o"]).map(|a| (a.source, a.out)),
            Ok((
                CliSource::Cache(PathBuf::from(DEFAULT_CACHE)),
                Some("o".into())
            ))
        );
        assert_eq!(
            p(&["generate", "--out", "x.rs", "--tsv", "d"]).map(|a| (a.source, a.out)),
            Ok((CliSource::Tsv("d".into()), Some("x.rs".into())))
        );
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
