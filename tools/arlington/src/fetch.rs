//! Fetching the pinned Arlington commit (specs/tooling/arlington.md, "Pinning"; ADR 0006, ADR 0007).
//!
//! The data is vendored in `tools/arlington/data` (ADR 0007); this module is the bump path. `git` fetches exactly one commit into a bare cache
//! repository. Before reading, we run `git fsck --full --strict` (git re-hashes every object,
//! so a corrupted or swapped object file is caught), check the commit and `tsv/latest` tree
//! ids against the pins below, check every blob's size, and only then read the blobs. Git
//! runs with replace objects disabled, repository discovery off (`--git-dir`), and without
//! inherited `GIT_*` variables or system/global config.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Error;
use crate::parse::{MAX_FILE_BYTES, MAX_FILES, MAX_TOTAL_BYTES};

pub const REPO_URL: &str = "https://github.com/pdf-association/arlington-pdf-model.git";
/// Pinned commit ("Hack in integer overflow detection", 2026-11-19).
pub const COMMIT: &str = "cdc485e4238c55bbd6204f6fda31997245d54b71";
/// Git tree id of `tsv/latest` in [`COMMIT`]; checked so a cache can't swap the files.
pub const TSV_TREE: &str = "334fcfcac3829007105bcb22ba0eaf495aa89e7d";
pub const TSV_DIR: &str = "tsv/latest";
/// Blob ids of upstream's `LICENSE` and `NOTICE.txt` in [`COMMIT`], vendored next to the data
/// (owner ruling on #60, ADR 0007).
pub const LICENSE_BLOB: &str = "261eeb9e9f8b2b4b0d119366dda99c6fd7d35c64";
pub const NOTICE_BLOB: &str = "7b4bbf67a64a62a1da099edc9b803e3050282d92";
/// The ref that keeps the pinned commit alive in the cache (so `gc` never prunes it).
pub const PIN_REF: &str = "refs/jpdf/pinned";

/// Inherited variables that could point git at another repository, object store, config or
/// replace namespace.
const SCRUBBED_ENV: [&str; 13] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_REPLACE_REF_BASE",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_CEILING_DIRECTORIES",
    "GIT_SHALLOW_FILE",
];

/// Maximum size of the `tsv/latest` tree object (its listing).
const MAX_TREE_BYTES: u64 = 1 << 20;

/// The cache repository inside `cache_dir`.
pub fn repo_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("arlington-pdf-model.git")
}

/// Makes sure the pinned commit is in the cache (fetching it if needed) and returns the
/// `tsv/latest` files as `(name, bytes)`, sorted by name.
pub fn load_pinned(cache_dir: &Path) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let repo = repo_path(cache_dir);
    if !repo.exists() {
        std::fs::create_dir_all(cache_dir).map_err(|source| Error::Io {
            path: cache_dir.display().to_string(),
            source,
        })?;
        let mut init = Command::new("git");
        init.args(["init", "--bare", "--quiet"]).arg(&repo);
        run(init, "init", None)?;
    }
    // `--git-dir` turns discovery off, so this is the cache or an error, never a parent repo.
    let bare = git_text(&repo, &["rev-parse", "--is-bare-repository"]).map_err(|e| {
        Error::Fetch(format!(
            "{} is not a usable cache ({e}); delete it",
            repo.display()
        ))
    })?;
    if bare != "true" {
        return Err(Error::Fetch(format!(
            "{} is not a bare repository; delete it",
            repo.display()
        )));
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
    // Keep the commit reachable (also for caches made before the ref existed).
    if git_text(&repo, &["rev-parse", "--verify", "--quiet", PIN_REF])
        .ok()
        .as_deref()
        != Some(COMMIT)
    {
        git(&repo, &["update-ref", PIN_REF, COMMIT])?;
    }
    load_verified(&repo, COMMIT, TSV_TREE)
}

/// Verifies the repository at `git_dir`, checks `commit` and its `tsv/latest` tree against
/// the pins, and reads the files.
pub fn load_verified(
    git_dir: &Path,
    commit: &str,
    tree: &str,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    // Re-hashes every object; catches object files whose content no longer matches their id.
    git(
        git_dir,
        &[
            "fsck",
            "--full",
            "--strict",
            "--no-dangling",
            "--no-progress",
        ],
    )?;
    let got = git_text(
        git_dir,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?;
    if got != commit {
        return Err(Error::Fetch(format!("commit is {got}, pinned {commit}")));
    }
    let got = git_text(
        git_dir,
        &["rev-parse", "--verify", &format!("{commit}:{TSV_DIR}")],
    )?;
    if got != tree {
        return Err(Error::Fetch(format!(
            "{TSV_DIR} tree is {got}, pinned {tree}"
        )));
    }
    let tree_size: u64 = git_text(git_dir, &["cat-file", "-s", tree])?
        .parse()
        .map_err(|_| Error::Fetch("bad tree size".into()))?;
    if tree_size > MAX_TREE_BYTES {
        return Err(Error::Limit(format!(
            "{TSV_DIR} tree is {tree_size} bytes (max {MAX_TREE_BYTES})"
        )));
    }
    let listing = git_out(git_dir, &["ls-tree", "-z", tree], None)?;
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
        if entries.len() >= MAX_FILES {
            return Err(Error::Limit(format!("more than {MAX_FILES} files")));
        }
        entries.push((name.to_string(), id.to_string()));
    }
    let ids: String = entries.iter().map(|(_, id)| format!("{id}\n")).collect();

    // Sizes first (`--batch-check` prints only headers), so nothing oversized is ever read.
    let check = git_out(
        git_dir,
        &["cat-file", "--batch-check"],
        Some(ids.clone().into_bytes()),
    )?;
    let check = String::from_utf8(check).map_err(|_| Error::Fetch("bad batch-check".into()))?;
    let mut lines = check.lines();
    let mut total: usize = 0;
    for (name, id) in &entries {
        let header = lines
            .next()
            .ok_or_else(|| Error::Fetch("truncated batch-check output".into()))?;
        let size = blob_size(header, id)?;
        if size > MAX_FILE_BYTES {
            return Err(Error::Limit(format!(
                "{name} is {size} bytes (max {MAX_FILE_BYTES})"
            )));
        }
        total = total.saturating_add(size);
        if total > MAX_TOTAL_BYTES {
            return Err(Error::Limit(format!(
                "{TSV_DIR} holds more than {MAX_TOTAL_BYTES} bytes"
            )));
        }
    }
    if lines.next().is_some() {
        return Err(Error::Fetch("unexpected extra batch-check output".into()));
    }

    let batch = git_out(git_dir, &["cat-file", "--batch"], Some(ids.into_bytes()))?;
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
        let size = blob_size(header, &id)?;
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
        if rest.get(end) != Some(&b'\n') {
            return Err(Error::Fetch("bad cat-file framing".into()));
        }
        files.push((name, content.to_vec()));
        rest = rest.get(end.saturating_add(1)..).unwrap_or_default();
    }
    if !rest.is_empty() {
        return Err(Error::Fetch("unexpected extra cat-file output".into()));
    }
    files.sort();
    Ok(files)
}

/// Reads `<commit>:<path>` from a cache that [`load_verified`] has just checked, after
/// checking that it is exactly the blob `id` and not over [`MAX_FILE_BYTES`].
pub fn read_pinned_blob(
    git_dir: &Path,
    commit: &str,
    path: &str,
    id: &str,
) -> Result<Vec<u8>, Error> {
    let got = git_text(
        git_dir,
        &["rev-parse", "--verify", &format!("{commit}:{path}")],
    )?;
    if got != id {
        return Err(Error::Fetch(format!("{path} is {got}, pinned {id}")));
    }
    let check = git_out(
        git_dir,
        &["cat-file", "--batch-check"],
        Some(format!("{id}\n").into_bytes()),
    )?;
    let header = String::from_utf8_lossy(&check);
    let size = blob_size(header.trim_end(), id)?;
    if size > MAX_FILE_BYTES {
        return Err(Error::Limit(format!(
            "{path} is {size} bytes (max {MAX_FILE_BYTES})"
        )));
    }
    let bytes = git_out(git_dir, &["cat-file", "blob", id], None)?;
    if bytes.len() != size {
        return Err(Error::Fetch(format!(
            "{path}: read {} bytes, expected {size}",
            bytes.len()
        )));
    }
    Ok(bytes)
}

/// Parses `"<id> blob <size>"` for the expected `id`.
fn blob_size(header: &str, id: &str) -> Result<usize, Error> {
    match header.split(' ').collect::<Vec<_>>()[..] {
        [hid, "blob", size] if hid == id => size
            .parse::<usize>()
            .map_err(|_| Error::Fetch(format!("bad size in {header:?}"))),
        _ => Err(Error::Fetch(format!(
            "unexpected cat-file header {header:?}"
        ))),
    }
}

fn git(git_dir: &Path, args: &[&str]) -> Result<(), Error> {
    git_out(git_dir, args, None).map(|_| ())
}

fn git_text(git_dir: &Path, args: &[&str]) -> Result<String, Error> {
    let out = git_out(git_dir, args, None)?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// Runs `git --no-replace-objects --git-dir <git_dir> <args>` hardened as described in the
/// module docs and returns its stdout. Also used by `jpdf-corpus` (specs/tooling/corpus.md).
pub fn git_out(git_dir: &Path, args: &[&str], stdin: Option<Vec<u8>>) -> Result<Vec<u8>, Error> {
    let mut cmd = Command::new("git");
    cmd.arg("--no-replace-objects")
        .arg("--git-dir")
        .arg(git_dir)
        .args(args);
    run(cmd, &args.join(" "), stdin)
}

/// Runs git with a scrubbed environment; `what` names the command in errors.
fn run(mut cmd: Command, what: &str, stdin: Option<Vec<u8>>) -> Result<Vec<u8>, Error> {
    for var in SCRUBBED_ENV {
        cmd.env_remove(var);
    }
    let mut child = cmd
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
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
    // Report git's own failure (and its stderr) before a broken pipe it caused.
    if !output.status.success() {
        return Err(Error::Fetch(format!(
            "git {what} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    if let Some(w) = writer {
        match w.join() {
            Ok(Ok(())) => {}
            _ => return Err(Error::Fetch("writing to git failed".into())),
        }
    }
    Ok(output.stdout)
}

fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}
