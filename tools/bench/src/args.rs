//! Command-line parsing (spec item 2). Every error is a usage error (exit 2).

use std::ffi::OsString;
use std::path::PathBuf;

pub const USAGE: &str = "usage: jpdf-bench --cli PATH --out FILE.json [--corpus DIR] [--sample N] \
     [--runs R] [--warmup W] [--mutool PATH|none] [--pdfium PATH|none] [--work DIR]";

pub const DEFAULT_SAMPLE: usize = 20;
pub const MAX_SAMPLE: usize = 10_000;
pub const DEFAULT_RUNS: usize = 5;
pub const MAX_RUNS: usize = 100;
pub const DEFAULT_WARMUP: usize = 1;
pub const MAX_WARMUP: usize = 10;

/// How to find a reference engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Look for the default program name on `PATH`.
    Auto,
    /// `none`: don't run this engine.
    Off,
    /// An explicit program path.
    Path(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub cli: PathBuf,
    pub out: PathBuf,
    pub corpus: Option<PathBuf>,
    pub sample: usize,
    pub runs: usize,
    pub warmup: usize,
    pub mutool: Choice,
    pub pdfium: Choice,
    pub work: Option<PathBuf>,
}

/// Parses the arguments after the program name.
pub fn parse(args: &[OsString]) -> Result<Options, String> {
    let mut cli = None;
    let mut out = None;
    let mut corpus = None;
    let mut sample = DEFAULT_SAMPLE;
    let mut runs = DEFAULT_RUNS;
    let mut warmup = DEFAULT_WARMUP;
    let mut mutool = Choice::Auto;
    let mut pdfium = Choice::Auto;
    let mut work = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let name = flag
            .to_str()
            .ok_or_else(|| "flag is not UTF-8".to_string())?;
        let value = match name {
            "--cli" | "--out" | "--corpus" | "--sample" | "--runs" | "--warmup" | "--mutool"
            | "--pdfium" | "--work" => it.next().ok_or_else(|| format!("{name} needs a value"))?,
            _ => return Err(format!("unknown argument {name:?}")),
        };
        match name {
            "--cli" => cli = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--corpus" => corpus = Some(PathBuf::from(value)),
            "--work" => work = Some(PathBuf::from(value)),
            "--sample" => sample = number(name, value, 1, MAX_SAMPLE)?,
            "--runs" => runs = number(name, value, 1, MAX_RUNS)?,
            "--warmup" => warmup = number(name, value, 0, MAX_WARMUP)?,
            "--mutool" => mutool = choice(value),
            _ => pdfium = choice(value),
        }
    }
    Ok(Options {
        cli: cli.ok_or_else(|| "--cli is required".to_string())?,
        out: out.ok_or_else(|| "--out is required".to_string())?,
        corpus,
        sample,
        runs,
        warmup,
        mutool,
        pdfium,
        work,
    })
}

fn number(name: &str, value: &OsString, min: usize, max: usize) -> Result<usize, String> {
    let n = value
        .to_str()
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|s| s.parse::<usize>().ok())
        .ok_or_else(|| format!("{name} needs a number"))?;
    if (min..=max).contains(&n) {
        Ok(n)
    } else {
        Err(format!("{name} must be between {min} and {max}"))
    }
}

fn choice(value: &OsString) -> Choice {
    if value == "none" {
        Choice::Off
    } else {
        Choice::Path(PathBuf::from(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Options, String> {
        parse(&args.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn defaults() {
        let o = p(&["--cli", "c", "--out", "o.json"]).unwrap();
        assert_eq!(o.cli, PathBuf::from("c"));
        assert_eq!(o.out, PathBuf::from("o.json"));
        assert_eq!(o.corpus, None);
        assert_eq!(o.sample, 20);
        assert_eq!(o.runs, 5);
        assert_eq!(o.warmup, 1);
        assert_eq!(o.mutool, Choice::Auto);
        assert_eq!(o.pdfium, Choice::Auto);
        assert_eq!(o.work, None);
    }

    #[test]
    fn every_flag() {
        let o = p(&[
            "--cli",
            "c",
            "--out",
            "o",
            "--corpus",
            "d",
            "--sample",
            "3",
            "--runs",
            "7",
            "--warmup",
            "0",
            "--mutool",
            "none",
            "--pdfium",
            "/x/pdfium_test",
            "--work",
            "w",
        ])
        .unwrap();
        assert_eq!(o.corpus, Some(PathBuf::from("d")));
        assert_eq!((o.sample, o.runs, o.warmup), (3, 7, 0));
        assert_eq!(o.mutool, Choice::Off);
        assert_eq!(o.pdfium, Choice::Path(PathBuf::from("/x/pdfium_test")));
        assert_eq!(o.work, Some(PathBuf::from("w")));
    }

    #[test]
    fn range_boundaries() {
        let base = ["--cli", "c", "--out", "o"];
        let with = |f: &str, v: &str| {
            let mut a = base.to_vec();
            a.extend([f, v]);
            p(&a)
        };
        for (flag, lo, hi) in [
            ("--sample", 1, 10_000),
            ("--runs", 1, 100),
            ("--warmup", 0, 10),
        ] {
            assert!(with(flag, &lo.to_string()).is_ok(), "{flag} {lo}");
            assert!(with(flag, &hi.to_string()).is_ok(), "{flag} {hi}");
            assert!(
                with(flag, &(hi + 1).to_string()).is_err(),
                "{flag} {}",
                hi + 1
            );
            if lo > 0 {
                assert!(
                    with(flag, &(lo - 1).to_string()).is_err(),
                    "{flag} {}",
                    lo - 1
                );
            }
        }
        for bad in ["", "-1", "+3", "1.5", "x", "99999999999999999999999"] {
            assert!(with("--runs", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn usage_errors() {
        assert!(p(&[]).is_err());
        assert!(p(&["--cli", "c"]).is_err());
        assert!(p(&["--out", "o"]).is_err());
        assert!(p(&["--cli", "c", "--out"]).is_err());
        assert!(p(&["--cli", "c", "--out", "o", "--fast"]).is_err());
        assert!(p(&["--cli", "c", "--out", "o", "extra"]).is_err());
    }
}
