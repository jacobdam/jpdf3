//! `jpdf-corpus build|fetch` (specs/tooling/corpus.md).
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use jpdf_corpus::{Error, Tier, build, fetch, manifest, repo_root, resolve_cache, sources};

const USAGE: &str = "usage: jpdf-corpus build regress [--cache DIR]\n       \
     jpdf-corpus fetch <smoke|regress> [--cache DIR] [--limit N] [--jobs J]";

/// The judge's infrastructure marker (CONTRACT; `ops/judge/lib/common.py`).
const INFRA_MARKER: &str = "JUDGE-INFRA-FAILURE";

#[derive(Debug, PartialEq)]
struct Args {
    command: String,
    tier: Tier,
    cache: Option<PathBuf>,
    limit: Option<usize>,
    jobs: usize,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut it = args.iter();
    let command = it.next().ok_or("missing command")?.clone();
    if command != "build" && command != "fetch" {
        return Err(format!("unknown command {command:?}"));
    }
    let tier_name = it.next().ok_or("missing tier")?;
    let tier = Tier::parse(tier_name).ok_or_else(|| format!("unknown tier {tier_name:?}"))?;
    if command == "build" && tier != Tier::Regress {
        return Err(format!("build supports only regress, not {tier_name:?}"));
    }
    let (mut cache, mut limit, mut jobs) = (None, None, None);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        let twice = || format!("{flag} given twice");
        match flag.as_str() {
            "--cache" => {
                if cache.replace(PathBuf::from(value()?)).is_some() {
                    return Err(twice());
                }
            }
            "--limit" if command == "fetch" => {
                let v = value()?;
                let n: usize = v.parse().map_err(|_| format!("bad --limit {v:?}"))?;
                if limit.replace(n).is_some() {
                    return Err(twice());
                }
            }
            "--jobs" if command == "fetch" => {
                let v = value()?;
                let n: usize = v
                    .parse()
                    .ok()
                    .filter(|n| (1..=fetch::MAX_JOBS).contains(n))
                    .ok_or_else(|| format!("bad --jobs {v:?} (1-{})", fetch::MAX_JOBS))?;
                if jobs.replace(n).is_some() {
                    return Err(twice());
                }
            }
            _ => return Err(format!("unknown flag {flag:?}")),
        }
    }
    Ok(Args {
        command,
        tier,
        cache,
        limit,
        jobs: jobs.unwrap_or(8),
    })
}

fn run(a: Args) -> Result<ExitCode, Error> {
    let root = repo_root();
    let cache = resolve_cache(
        a.cache,
        std::env::var_os("JPDF_CORPUS_CACHE").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
        &root,
    )?;
    if a.command == "build" {
        let mut lists = Vec::new();
        for src in sources::REGRESS {
            let files = build::list_source(&cache, &src)?;
            eprintln!("{}: {} PDFs", src.name, files.len());
            lists.push((src, files));
        }
        let m = build::merge(a.tier.name(), &lists)?;
        let out = a.tier.manifest();
        let tmp = out.with_extension("tsv.tmp");
        let io = |source| Error::Io {
            path: out.display().to_string(),
            source,
        };
        std::fs::write(&tmp, manifest::write(&m)).map_err(io)?;
        std::fs::rename(&tmp, &out).map_err(io)?;
        let bytes: u64 = m
            .entries
            .iter()
            .map(|e| e.size)
            .fold(0, u64::saturating_add);
        eprintln!(
            "{}: {} files, {bytes} bytes",
            a.tier.manifest_rel(),
            m.entries.len()
        );
        return Ok(ExitCode::SUCCESS);
    }
    let m = manifest::load(&a.tier.manifest())?;
    let mut entries = m.entries;
    entries.sort_by(|x, y| x.blob.cmp(&y.blob));
    if let Some(n) = a.limit {
        entries.truncate(n);
    }
    let dir = cache.join(a.tier.name());
    std::fs::create_dir_all(&dir).map_err(|source| Error::Io {
        path: dir.display().to_string(),
        source,
    })?;
    let opts = fetch::Options {
        jobs: a.jobs,
        ..fetch::Options::default()
    };
    let r = fetch::fetch(&entries, &dir, &opts);
    for line in r.integrity.iter().chain(&r.failed) {
        eprintln!("{line}");
    }
    eprintln!("{}: {}", dir.display(), r.summary());
    Ok(if !r.integrity.is_empty() {
        ExitCode::FAILURE
    } else if r.is_infra_failure() {
        eprintln!(
            "{INFRA_MARKER}: {} corpus files could not be fetched",
            r.failed.len()
        );
        ExitCode::from(75)
    } else {
        ExitCode::SUCCESS
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = match parse(&args) {
        Ok(a) => a,
        Err(m) => {
            eprintln!("{m}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(parsed) {
        Ok(code) => code,
        Err(Error::Usage(m)) => {
            eprintln!("{m}\n{USAGE}");
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Args, String> {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn parses_commands() {
        let a = p(&[
            "fetch", "smoke", "--limit", "3", "--jobs", "2", "--cache", "/c",
        ])
        .unwrap();
        assert_eq!(
            a,
            Args {
                command: "fetch".into(),
                tier: Tier::Smoke,
                cache: Some("/c".into()),
                limit: Some(3),
                jobs: 2
            }
        );
        let a = p(&["build", "regress"]).unwrap();
        assert_eq!((a.tier, a.cache, a.jobs), (Tier::Regress, None, 8));
    }

    #[test]
    fn usage_errors() {
        for (args, want) in [
            (&[][..], "missing command"),
            (&["get"][..], "unknown command"),
            (&["fetch"][..], "missing tier"),
            (&["fetch", "exam"][..], "unknown tier"),
            (&["fetch", "ops/judge/corpus/smoke.tsv"][..], "unknown tier"),
            (&["build", "smoke"][..], "only regress"),
            (&["build", "regress", "--limit", "3"][..], "unknown flag"),
            (&["fetch", "smoke", "--jobs", "0"][..], "bad --jobs"),
            (&["fetch", "smoke", "--jobs", "33"][..], "bad --jobs"),
            (&["fetch", "smoke", "--limit", "-1"][..], "bad --limit"),
            (&["fetch", "smoke", "--limit"][..], "needs a value"),
            (
                &["fetch", "smoke", "--cache", "a", "--cache", "b"][..],
                "given twice",
            ),
            (&["fetch", "smoke", "--manifest", "x"][..], "unknown flag"),
        ] {
            let e = p(args).unwrap_err();
            assert!(e.contains(want), "{args:?}: {e}");
        }
    }
}
