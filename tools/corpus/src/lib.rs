//! Corpus tiers (specs/tooling/corpus.md, issue #22, PLAN §6.1).
//!
//! Builds the agent-owned manifests from pinned git test suites and fetches tiers into a
//! cache outside the repository, checking every file against its git blob id. Development
//! tool only: it never parses PDF bytes, and PDFs are never committed (CONSTITUTION 18).
//! Tiers are chosen by name from a fixed table, so the sealed tier can't be fetched
//! (CONSTITUTION 13).
#![forbid(unsafe_code)]

use std::fmt;
use std::path::{Path, PathBuf};

pub mod build;
pub mod fetch;
pub mod manifest;
pub mod sha1;
pub mod sources;

/// Every way building, parsing or fetching can fail.
#[derive(Debug)]
pub enum Error {
    /// A manifest breaks the format (`line` is 1-based; 0 for whole-file problems).
    Manifest {
        manifest: String,
        line: usize,
        message: String,
    },
    /// Fetching, verifying or listing a pinned source failed.
    Build(String),
    /// Bad command-line use (exit 2).
    Usage(String),
    Io {
        path: String,
        source: std::io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest {
                manifest,
                line,
                message,
            } => write!(f, "{manifest}:{line}: {message}"),
            Self::Build(m) => write!(f, "build: {m}"),
            Self::Usage(m) => write!(f, "{m}"),
            Self::Io { path, source } => write!(f, "{path}: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// A tier this tool may fetch (item 1). There is deliberately no sealed entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Smoke,
    Regress,
}

impl Tier {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "smoke" => Some(Self::Smoke),
            "regress" => Some(Self::Regress),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Smoke => "smoke",
            Self::Regress => "regress",
        }
    }

    /// The manifest path relative to the repository root.
    pub fn manifest_rel(self) -> &'static str {
        match self {
            Self::Smoke => "ops/judge/corpus/smoke.tsv",
            Self::Regress => "tools/corpus/manifests/regress.tsv",
        }
    }

    pub fn manifest(self) -> PathBuf {
        repo_root().join(self.manifest_rel())
    }
}

/// The checkout this tool was built from (item 8).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Resolves the cache directory (item 7): `--cache`, `$JPDF_CORPUS_CACHE`, or
/// `$HOME/.cache/jpdf-corpus`; creates it and refuses one inside `repo`.
pub fn resolve_cache(
    flag: Option<PathBuf>,
    env_cache: Option<PathBuf>,
    home: Option<PathBuf>,
    repo: &Path,
) -> Result<PathBuf, Error> {
    let dir = flag
        .or(env_cache)
        .or_else(|| home.map(|h| h.join(".cache/jpdf-corpus")))
        .ok_or_else(|| Error::Usage("no cache dir: pass --cache or set HOME".into()))?;
    let io = |source| Error::Io {
        path: dir.display().to_string(),
        source,
    };
    std::fs::create_dir_all(&dir).map_err(io)?;
    let dir = dir.canonicalize().map_err(io)?;
    let repo = repo.canonicalize().map_err(|source| Error::Io {
        path: repo.display().to_string(),
        source,
    })?;
    if dir.starts_with(&repo) {
        return Err(Error::Usage(format!(
            "cache {} is inside the repository {} (PDFs are never committed)",
            dir.display(),
            repo.display()
        )));
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_table_has_no_sealed_entry() {
        assert_eq!(Tier::parse("smoke"), Some(Tier::Smoke));
        assert_eq!(Tier::parse("regress"), Some(Tier::Regress));
        for other in ["exam", "sealed", "wide", "../x", "", "Smoke"] {
            assert_eq!(Tier::parse(other), None, "{other}");
        }
        for t in [Tier::Smoke, Tier::Regress] {
            assert!(t.manifest().is_file(), "{}", t.manifest().display());
        }
    }

    #[test]
    fn cache_inside_the_repository_is_refused() {
        let root = repo_root();
        let inside = root.join("target/jpdf-corpus-test-cache");
        match resolve_cache(Some(inside.clone()), None, None, &root) {
            Err(Error::Usage(m)) => assert!(m.contains("inside the repository"), "{m}"),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir(&inside);
        match resolve_cache(None, None, None, &root) {
            Err(Error::Usage(m)) => assert!(m.contains("--cache"), "{m}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn cache_precedence() {
        let base = std::env::temp_dir().join(format!("jpdf-corpus-prec-{}", std::process::id()));
        let root = repo_root();
        let (a, b) = (base.join("flag"), base.join("env"));
        let got = resolve_cache(Some(a.clone()), Some(b.clone()), Some(base.clone()), &root);
        assert_eq!(got.unwrap(), a.canonicalize().unwrap());
        let got = resolve_cache(None, Some(b.clone()), Some(base.clone()), &root);
        assert_eq!(got.unwrap(), b.canonicalize().unwrap());
        let got = resolve_cache(None, None, Some(base.clone()), &root).unwrap();
        assert!(got.ends_with(".cache/jpdf-corpus"), "{}", got.display());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
