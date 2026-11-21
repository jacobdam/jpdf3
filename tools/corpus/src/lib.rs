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
/// `$HOME/.cache/jpdf-corpus`. Refuses, before creating anything, a cache inside `repo` or
/// inside any git working tree (a worktree's parent checkout too), then creates it.
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
    let dir = std::path::absolute(&dir).map_err(|source| Error::Io {
        path: dir.display().to_string(),
        source,
    })?;
    let repo = repo.canonicalize().map_err(|source| Error::Io {
        path: repo.display().to_string(),
        source,
    })?;
    // The nearest existing ancestor, canonicalized, plus the components still to create.
    let mut existing = dir.as_path();
    let mut rest = Vec::new();
    while !existing.exists() {
        let (Some(parent), Some(name)) = (existing.parent(), existing.file_name()) else {
            break;
        };
        rest.push(name.to_os_string());
        existing = parent;
    }
    let mut planned = existing.canonicalize().map_err(|source| Error::Io {
        path: existing.display().to_string(),
        source,
    })?;
    planned.extend(rest.iter().rev());
    check_outside(&planned, &repo)?;
    std::fs::create_dir_all(&planned).map_err(|source| Error::Io {
        path: planned.display().to_string(),
        source,
    })?;
    Ok(planned)
}

fn check_outside(dir: &Path, repo: &Path) -> Result<(), Error> {
    let inside = |what: &str, root: &Path| {
        Err(Error::Usage(format!(
            "cache {} is inside {what} {} (PDFs are never committed)",
            dir.display(),
            root.display()
        )))
    };
    if dir.starts_with(repo) {
        return inside("the repository", repo);
    }
    if let Some(root) = dir.ancestors().find(|a| a.join(".git").exists()) {
        return inside("the git working tree", root);
    }
    Ok(())
}

/// Creates `<cache>/<name>` and checks it really is that directory (not a symlink elsewhere).
pub fn sub_dir(cache: &Path, name: &str) -> Result<PathBuf, Error> {
    let want = cache.join(name);
    let io = |source| Error::Io {
        path: want.display().to_string(),
        source,
    };
    std::fs::create_dir_all(&want).map_err(io)?;
    let got = want.canonicalize().map_err(io)?;
    if got != want {
        return Err(Error::Usage(format!(
            "{} resolves to {}; remove the link",
            want.display(),
            got.display()
        )));
    }
    Ok(want)
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
        assert!(!inside.exists(), "nothing is created before the check");
        // Relative and dot-dot spellings resolve to the same place.
        let sneaky = root.join("tools/../target/x/../jpdf-corpus-test-cache");
        assert!(resolve_cache(Some(sneaky), None, None, &root).is_err());
        match resolve_cache(None, None, None, &root) {
            Err(Error::Usage(m)) => assert!(m.contains("--cache"), "{m}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn cache_inside_another_git_tree_or_via_symlink_is_refused() {
        let base = std::env::temp_dir().join(format!("jpdf-corpus-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("checkout/.git")).unwrap();
        std::fs::create_dir_all(base.join("elsewhere")).unwrap();
        let root = repo_root();
        match resolve_cache(Some(base.join("checkout/scratch/c")), None, None, &root) {
            Err(Error::Usage(m)) => assert!(m.contains("inside the git working tree"), "{m}"),
            other => panic!("{other:?}"),
        }
        // A symlinked cache root resolves to its target, which is then checked.
        std::os::unix::fs::symlink(base.join("checkout"), base.join("link")).unwrap();
        assert!(resolve_cache(Some(base.join("link/c")), None, None, &root).is_err());
        // A tier directory that is a symlink is refused.
        let cache = resolve_cache(Some(base.join("elsewhere/c")), None, None, &root).unwrap();
        std::os::unix::fs::symlink(base.join("checkout"), cache.join("regress")).unwrap();
        match sub_dir(&cache, "regress") {
            Err(Error::Usage(m)) => assert!(m.contains("remove the link"), "{m}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(sub_dir(&cache, "smoke").unwrap(), cache.join("smoke"));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn cache_precedence() {
        let base = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("jpdf-corpus-prec-{}", std::process::id()));
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
