//! `jpdf-arlington check|generate` (specs/tooling/arlington.md, "CLI").
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use jpdf_arlington::{Source, generate, load};

const USAGE: &str = "usage: jpdf-arlington check [--tsv DIR | --cache DIR]\n       \
     jpdf-arlington generate --out FILE [--tsv DIR | --cache DIR]";

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
    Cache(PathBuf),
    Tsv(PathBuf),
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut it = args.iter();
    let command = it.next().ok_or("missing command")?.clone();
    if command != "check" && command != "generate" {
        return Err(format!("unknown command {command:?}"));
    }
    let (mut cache, mut tsv, mut out) = (None, None, None);
    while let Some(flag) = it.next() {
        let slot = match flag.as_str() {
            "--cache" => &mut cache,
            "--tsv" => &mut tsv,
            "--out" if command == "generate" => &mut out,
            _ => return Err(format!("unknown flag {flag:?}")),
        };
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        if slot.replace(PathBuf::from(value)).is_some() {
            return Err(format!("{flag} given twice"));
        }
    }
    let source = match (cache, tsv) {
        (Some(_), Some(_)) => return Err("give --cache or --tsv, not both".into()),
        (None, Some(t)) => CliSource::Tsv(t),
        (c, None) => CliSource::Cache(c.unwrap_or_else(|| PathBuf::from(DEFAULT_CACHE))),
    };
    if command == "generate" && out.is_none() {
        return Err("generate needs --out FILE".into());
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
    let source = match args.source {
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
            Ok(CliSource::Cache(PathBuf::from(DEFAULT_CACHE)))
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
            &["check", "--bogus", "1"],
        ] {
            assert!(p(bad).is_err(), "{bad:?}");
        }
    }
}
