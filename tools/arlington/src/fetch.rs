//! Fetching the pinned Arlington commit (specs/tooling/arlington.md, "Pinning"; ADR 0006).
//!
//! The data is never committed. `git` fetches exactly one commit into a bare cache
//! repository; we check the commit and the `tsv/latest` tree ids against the pins below and
//! read the files straight from git's object store, so nothing on disk outside it is trusted.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Error;
use crate::parse::{MAX_FILE_BYTES, MAX_FILES};

pub const REPO_URL: &str = "https://github.com/pdf-association/arlington-pdf-model.git";
/// Pinned commit ("Hack in integer overflow detection", 2026-11-19).
pub const COMMIT: &str = "cdc485e4238c55bbd6204f6fda31997245d54b71";
/// Git tree id of `tsv/latest` in [`COMMIT`]; checked so a cache can't swap the files.
pub const TSV_TREE: &str = "334fcfcac3829007105bcb22ba0eaf495aa89e7d";
pub const TSV_DIR: &str = "tsv/latest";

/// The cache repository inside `cache_dir`.
pub fn repo_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("arlington-pdf-model.git")
}

/// Makes sure the pinned commit is in the cache (fetching it if needed) and returns the
/// `tsv/latest` files as `(name, bytes)`, sorted by name.
pub fn load_pinned(cache_dir: &Path) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let repo = repo_path(cache_dir);
    if !repo.join("HEAD").is_file() {
        std::fs::create_dir_all(&repo).map_err(|source| Error::Io {
            path: repo.display().to_string(),
            source,
        })?;
        git(&repo, &["init", "--bare", "--quiet"])?;
    }
    let commit_ref = format!("{COMMIT}^{{commit}}");
    if git(&repo, &["cat-file", "-e", &commit_ref]).is_err() {
        git(
            &repo,
            &[
                "fetch",
                "--quiet",
                "--depth",
                "1",
                "--no-tags",
                REPO_URL,
                COMMIT,
            ],
        )?;
    }
    load_verified(&repo, COMMIT, TSV_TREE)
}

/// Checks `commit` and its `tsv/latest` tree against the pins and reads the files.
pub fn load_verified(
    repo: &Path,
    commit: &str,
    tree: &str,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let got = git_text(
        repo,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?;
    if got != commit {
        return Err(Error::Fetch(format!("commit is {got}, pinned {commit}")));
    }
    let got = git_text(
        repo,
        &["rev-parse", "--verify", &format!("{commit}:{TSV_DIR}")],
    )?;
    if got != tree {
        return Err(Error::Fetch(format!(
            "{TSV_DIR} tree is {got}, pinned {tree}"
        )));
    }
    let listing = git_out(repo, &["ls-tree", "-z", tree], None)?;
    let mut entries = Vec::new();
    for entry in listing.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = std::str::from_utf8(entry)
            .map_err(|_| Error::Fetch("ls-tree output is not UTF-8".into()))?;
        let (meta, name) = entry
            .split_once('\t')
            .ok_or_else(|| Error::Fetch(format!("bad ls-tree line {entry:?}")))?;
        let [_mode, kind, id] = meta.split(' ').collect::<Vec<_>>()[..] else {
            return Err(Error::Fetch(format!("bad ls-tree line {entry:?}")));
        };
        if kind != "blob" || !name.ends_with(".tsv") {
            return Err(Error::Fetch(format!("{TSV_DIR}/{name} is not a .tsv file")));
        }
        entries.push((name.to_string(), id.to_string()));
        if entries.len() > MAX_FILES {
            return Err(Error::Limit(format!("more than {MAX_FILES} files")));
        }
    }
    let ids: String = entries.iter().map(|(_, id)| format!("{id}\n")).collect();
    let batch = git_out(repo, &["cat-file", "--batch"], Some(ids.into_bytes()))?;
    let mut rest = batch.as_slice();
    let mut files = Vec::with_capacity(entries.len());
    for (name, id) in entries {
        // Each object: "<id> blob <size>\n<content>\n".
        let nl = rest
            .iter()
            .position(|b| *b == b'\n')
            .ok_or_else(|| Error::Fetch("truncated cat-file output".into()))?;
        let header = std::str::from_utf8(rest.get(..nl).unwrap_or_default())
            .map_err(|_| Error::Fetch("bad cat-file header".into()))?;
        let size = match header.split(' ').collect::<Vec<_>>()[..] {
            [hid, "blob", size] if hid == id => size
                .parse::<usize>()
                .map_err(|_| Error::Fetch(format!("bad size in {header:?}")))?,
            _ => {
                return Err(Error::Fetch(format!(
                    "unexpected cat-file header {header:?}"
                )));
            }
        };
        if size > MAX_FILE_BYTES {
            return Err(Error::Limit(format!("{name} is {size} bytes")));
        }
        let start = nl.saturating_add(1);
        let end = start
            .checked_add(size)
            .ok_or_else(|| Error::Fetch("size overflow".into()))?;
        let content = rest
            .get(start..end)
            .ok_or_else(|| Error::Fetch("truncated cat-file output".into()))?;
        files.push((name, content.to_vec()));
        rest = rest.get(end.saturating_add(1)..).unwrap_or_default();
    }
    files.sort();
    Ok(files)
}

fn git(repo: &Path, args: &[&str]) -> Result<(), Error> {
    git_out(repo, args, None).map(|_| ())
}

fn git_text(repo: &Path, args: &[&str]) -> Result<String, Error> {
    let out = git_out(repo, args, None)?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

fn git_out(repo: &Path, args: &[&str], stdin: Option<Vec<u8>>) -> Result<Vec<u8>, Error> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Fetch(format!("cannot run git: {e}")))?;
    // Feed stdin from a thread so a full stdout pipe can't deadlock us.
    let writer = match (stdin, child.stdin.take()) {
        (Some(data), Some(mut pipe)) => Some(std::thread::spawn(move || pipe.write_all(&data))),
        _ => None,
    };
    // Reads stdout and stderr together, so neither pipe can fill up and block git.
    let output = child
        .wait_with_output()
        .map_err(|e| Error::Fetch(format!("git: {e}")))?;
    if let Some(w) = writer {
        match w.join() {
            Ok(Ok(())) => {}
            _ => return Err(Error::Fetch("writing to git failed".into())),
        }
    }
    if !output.status.success() {
        return Err(Error::Fetch(format!(
            "git {} failed ({}): {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}
