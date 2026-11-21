//! Fetching the pinned Arlington commit (specs/tooling/arlington.md, "Pinning"; ADR 0006, ADR 0007).
//!
//! The data is vendored in `tools/arlington/data` (ADR 0007); this module is the bump
//! path. `git` fetches exactly one commit into a bare cache
//! repository. Before reading, we run `git fsck --full --strict` (git re-hashes every object,
//! so a corrupted or swapped object file is caught), check the commit and `tsv/latest` tree
//! ids against the pins below, check every blob's size, and only then read the blobs. Git
//! runs with replace objects disabled, repository discovery off (`--git-dir`), no hooks,
//! HTTPS as the only transport, a deadline per call, no inherited `GIT_*` variables and no
//! system/global config; a cache whose own config holds more than `git init --bare` writes
//! is refused ([`git_command`], [`ensure_pinned`]).

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

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

/// Wall-clock limit for one git call, the fetch included (spec item 2). A stalled transfer
/// is also cut off by `http.lowSpeedLimit`/`http.lowSpeedTime` below.
pub const GIT_DEADLINE: Duration = Duration::from_secs(600);

/// Keys a cache repository's own config may hold: what `git init --bare` writes on macOS,
/// Linux and Windows. Anything else (`core.hooksPath`, `credential.helper`, `include.path`,
/// `core.alternateRefsCommand`, `url.*.insteadOf`, …) makes the cache unusable.
const CACHE_CONFIG_KEYS: [&str; 6] = [
    "core.repositoryformatversion",
    "core.filemode",
    "core.bare",
    "core.ignorecase",
    "core.precomposeunicode",
    "core.symlinks",
];

/// Files that would point a cache at another repository's objects or config.
const FOREIGN_LINKS: [&str; 3] = [
    "commondir",
    "objects/info/alternates",
    "objects/info/http-alternates",
];

/// How often [`run`] checks whether git has exited.
const POLL: Duration = Duration::from_millis(10);
/// How long [`run`] waits for git's pipes to close after it exited.
const PIPE_GRACE: Duration = Duration::from_secs(5);

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
    ensure_pinned(&repo, REPO_URL, COMMIT, PIN_REF)?;
    load_verified(&repo, COMMIT, TSV_TREE)
}

/// Makes sure the bare repository `repo` holds `commit`, fetching it once from `url` with
/// `git fetch --depth 1 --no-tags`, and keeps it alive with `pin_ref`. Creates `repo` (and
/// its parents) if the path does not exist. An existing path is checked first: it must be a
/// bare repository whose own config holds only [`CACHE_CONFIG_KEYS`] and which has none of
/// [`FOREIGN_LINKS`]. Callers verify the content afterwards (`git fsck` plus their pins).
/// Shared by `jpdf-corpus` (specs/tooling/corpus.md item 5).
pub fn ensure_pinned(repo: &Path, url: &str, commit: &str, pin_ref: &str) -> Result<(), Error> {
    if !repo.exists() {
        if let Some(parent) = repo.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.display().to_string(),
                source,
            })?;
        }
        // An empty template: nothing from a system or inherited template directory.
        let mut init = git_command();
        init.args([
            "init",
            "--bare",
            "--quiet",
            "--template=",
            "--object-format=sha1",
        ])
        .arg(repo);
        run(init, "init", None, GIT_DEADLINE)?;
    }
    check_cache(repo)?;
    let commit_ref = format!("{commit}^{{commit}}");
    if git(repo, &["cat-file", "-e", &commit_ref]).is_err() {
        git(
            repo,
            &["fetch", "--quiet", "--depth", "1", "--no-tags", url, commit],
        )?;
    }
    // Keep the commit reachable (also for caches made before the ref existed).
    if git_text(repo, &["rev-parse", "--verify", "--quiet", pin_ref])
        .ok()
        .as_deref()
        != Some(commit)
    {
        git(repo, &["update-ref", pin_ref, commit])?;
    }
    Ok(())
}

/// Refuses a cache that isn't a plain bare repository we could have made ourselves.
fn check_cache(repo: &Path) -> Result<(), Error> {
    let unusable = |why: String| {
        Error::Fetch(format!(
            "{} is not a usable cache ({why}); delete it",
            repo.display()
        ))
    };
    // `--git-dir` follows a gitfile (`gitdir: …`) and symlinks, so the cache and the
    // directories git writes into must be real directories here.
    for dir in [repo.to_path_buf(), repo.join("objects"), repo.join("refs")] {
        match std::fs::symlink_metadata(&dir) {
            Ok(m) if m.is_dir() => {}
            _ => {
                return Err(unusable(format!(
                    "{} is not a directory (symlinks and gitfiles are refused)",
                    dir.display()
                )));
            }
        }
    }
    for name in FOREIGN_LINKS {
        if std::fs::symlink_metadata(repo.join(name)).is_ok() {
            return Err(unusable(format!("it has {name}")));
        }
    }
    // `--git-dir` turns discovery off, so this is the cache or an error, never a parent repo.
    // `--local` lists only the cache's own file and doesn't follow `include.path`.
    let keys = git_text(repo, &["config", "--local", "--name-only", "--list"])
        .map_err(|e| unusable(e.to_string()))?;
    if let Some(key) = keys
        .lines()
        .find(|k| !CACHE_CONFIG_KEYS.contains(&k.to_ascii_lowercase().as_str()))
    {
        return Err(unusable(format!("its config sets {key:?}")));
    }
    let bare = git_text(repo, &["rev-parse", "--is-bare-repository"])
        .map_err(|e| unusable(e.to_string()))?;
    if bare != "true" {
        return Err(Error::Fetch(format!(
            "{} is not a bare repository; delete it",
            repo.display()
        )));
    }
    // Belt and braces: git must resolve the cache to itself.
    let want = repo.canonicalize().map_err(|e| unusable(e.to_string()))?;
    let got = git_text(repo, &["rev-parse", "--absolute-git-dir"])
        .map_err(|e| unusable(e.to_string()))?;
    let got = Path::new(&got)
        .canonicalize()
        .map_err(|e| unusable(e.to_string()))?;
    if got != want {
        return Err(unusable(format!("git resolves it to {}", got.display())));
    }
    Ok(())
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
        let [mode, kind, id] = meta.split(' ').collect::<Vec<_>>()[..] else {
            return Err(Error::Fetch(format!("bad ls-tree line {entry:?}")));
        };
        if kind != "blob" || !name.ends_with(".tsv") {
            return Err(Error::Fetch(format!("{TSV_DIR}/{name} is not a .tsv file")));
        }
        // A symlink (120000) is also listed as a blob; the vendored tree id assumes 100644.
        if mode != "100644" {
            return Err(Error::Fetch(format!(
                "{TSV_DIR}/{name} has mode {mode}, expected 100644"
            )));
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
/// module docs, within [`GIT_DEADLINE`], and returns its stdout. Also used by `jpdf-corpus`
/// (specs/tooling/corpus.md).
pub fn git_out(git_dir: &Path, args: &[&str], stdin: Option<Vec<u8>>) -> Result<Vec<u8>, Error> {
    let mut cmd = git_command();
    cmd.arg("--no-replace-objects")
        .arg("--git-dir")
        .arg(git_dir)
        .args(args);
    run(cmd, &args.join(" "), stdin, GIT_DEADLINE)
}

/// `git` with the hardening of spec item 2: config on the command line that beats any
/// repository config (no hooks, HTTPS only, a low-speed cutoff) and an environment without
/// any inherited `GIT_*` variable or system/global config. Tests that drive git use it too.
pub fn git_command() -> Command {
    let mut cmd = Command::new("git");
    for kv in hardening_config() {
        cmd.arg("-c").arg(kv);
    }
    scrub_env(&mut cmd, std::env::vars_os().map(|(k, _)| k));
    cmd
}

/// The `-c` settings of [`git_command`], in order.
fn hardening_config() -> [String; 10] {
    [
        format!("core.hooksPath={}", null_device()),
        "core.fsmonitor=false".to_string(),
        "protocol.allow=never".to_string(),
        "protocol.https.allow=always".to_string(),
        "http.lowSpeedLimit=1000".to_string(),
        "http.lowSpeedTime=60".to_string(),
        // No background `gc`/`maintenance` that daemonizes out of our process group.
        "gc.auto=0".to_string(),
        "maintenance.auto=false".to_string(),
        // Never ask for credentials (a 401 from a missing repository).
        "credential.interactive=false".to_string(),
        "core.askPass=".to_string(),
    ]
}

/// Removes every variable in `inherited` whose name starts with `GIT_` (in any case, as
/// Windows names are case-insensitive) and `SSH_ASKPASS`, then sets the few git needs from
/// us. An empty `GIT_ASKPASS` ends git's askpass lookup, so no prompt program ever runs.
/// Proxy and CA variables (`HTTPS_PROXY`, `SSL_CERT_FILE`, …) are inherited on purpose:
/// the pins, not the transport, guarantee integrity.
pub fn scrub_env(cmd: &mut Command, inherited: impl IntoIterator<Item = OsString>) {
    for name in inherited {
        let bytes = name.as_encoded_bytes();
        if bytes
            .get(..4)
            .is_some_and(|p| p.eq_ignore_ascii_case(b"GIT_"))
            || bytes.eq_ignore_ascii_case(b"SSH_ASKPASS")
        {
            cmd.env_remove(name);
        }
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device());
}

/// Runs `cmd` (in its own process group on unix) and returns its stdout. At `deadline` the
/// whole group is killed and the call fails; `what` names the command in errors.
fn run(
    mut cmd: Command,
    what: &str,
    stdin: Option<Vec<u8>>,
    deadline: Duration,
) -> Result<Vec<u8>, Error> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let start = Instant::now();
    let mut child = cmd
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
        (Some(data), Some(mut pipe)) => {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(pipe.write_all(&data));
            });
            Some(rx)
        }
        _ => None,
    };
    // Read stdout and stderr on their own threads, so neither pipe can fill up and block git.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if start.elapsed() >= deadline => {
                kill_tree(&mut child);
                return Err(Error::Fetch(format!(
                    "git {what} timed out after {} s",
                    deadline.as_secs_f64()
                )));
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(e) => {
                kill_tree(&mut child);
                return Err(Error::Fetch(format!("git: {e}")));
            }
        }
    };
    let Ok(stderr) = stderr.recv_timeout(PIPE_GRACE) else {
        kill_tree(&mut child);
        return Err(Error::Fetch(format!(
            "git {what} ({status}): output did not close"
        )));
    };
    // Report git's own failure (and its stderr) before a broken pipe it caused.
    if !status.success() {
        return Err(Error::Fetch(format!(
            "git {what} failed ({status}): {}",
            String::from_utf8_lossy(&stderr).trim()
        )));
    }
    let Ok(stdout) = stdout.recv_timeout(PIPE_GRACE) else {
        kill_tree(&mut child);
        return Err(Error::Fetch(format!("git {what}: output did not close")));
    };
    if let Some(w) = writer {
        match w.recv_timeout(PIPE_GRACE) {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(Error::Fetch("writing to git failed".into())),
            Err(_) => {
                kill_tree(&mut child);
                return Err(Error::Fetch(format!("git {what}: input did not close")));
            }
        }
    }
    Ok(stdout)
}

/// Reads `pipe` to the end on a helper thread; the result arrives on the channel (an empty
/// buffer on a read error or a missing pipe, which the exit status then explains).
fn drain(pipe: Option<impl Read + Send + 'static>) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    match pipe {
        Some(mut pipe) => {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = pipe.read_to_end(&mut buf);
                let _ = tx.send(buf);
            });
        }
        None => {
            let _ = tx.send(Vec::new());
        }
    }
    rx
}

/// Kills the child's process group (unix) and the child itself, then reaps the child.
/// Errors mean the processes are already gone.
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        // The group id is the child's pid (`process_group(0)`); std has no safe group-kill
        // API, and the shell builtin exists even where no `kill` binary is installed.
        let _ = Command::new("/bin/sh")
            .args(["-c", "kill -s KILL -- \"-$1\"", "sh"])
            .arg(child.id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn null_device() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn sh(script: &str) -> Command {
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", script]);
        cmd
    }

    fn scratch(name: &str) -> PathBuf {
        let d = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/scratch/arlington-fetch")
            .join(name);
        let _ = std::fs::remove_dir_all(&d);
        // A failure here shows up as a missing pid file in the test itself.
        let _ = std::fs::create_dir_all(&d);
        d
    }

    fn alive(pid: &str) -> bool {
        Command::new("/bin/sh")
            .args(["-c", "kill -0 \"$1\" 2>/dev/null", "sh", pid])
            .status()
            .is_ok_and(|st| st.success())
    }

    /// The deadline kills git and everything it started (a stalled `git-remote-https`).
    #[test]
    fn the_deadline_kills_the_whole_group() {
        let d = scratch("deadline");
        let pid_file = d.join("pid");
        // The background child holds stdout open, so a plain wait would hang too.
        let script = format!(
            "sleep 30 & echo $! > '{p}.tmp'; mv '{p}.tmp' '{p}'; sleep 30",
            p = pid_file.display()
        );
        let start = Instant::now();
        let e = run(sh(&script), "stall", None, Duration::from_secs(2)).unwrap_err();
        let took = start.elapsed();
        assert!(
            e.to_string().contains("git stall timed out after 2 s"),
            "{e}"
        );
        assert!(took >= Duration::from_secs(2), "{took:?}");
        assert!(took < Duration::from_secs(15), "{took:?}");
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        // SIGKILL delivery is asynchronous; give the kernel a moment to reap it.
        let gone = (0..100).any(|_| {
            std::thread::sleep(Duration::from_millis(20));
            !alive(pid.trim())
        });
        assert!(gone, "background child {pid} survived");
    }

    /// Under the deadline, output and failures come through unchanged.
    #[test]
    fn commands_within_the_deadline_pass_through() {
        let out = run(
            sh("sleep 0.2; head -c 300000 /dev/zero; printf done"),
            "ok",
            None,
            Duration::from_secs(20),
        )
        .unwrap();
        assert_eq!(out.len(), 300_004);
        assert!(out.ends_with(b"done"));
        let piped = run(
            sh("cat"),
            "cat",
            Some(b"x".repeat(200_000)),
            Duration::from_secs(20),
        )
        .unwrap();
        assert_eq!(piped.len(), 200_000);
        let e = run(
            sh("echo nope >&2; exit 3"),
            "bad",
            None,
            Duration::from_secs(20),
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("git bad failed") && e.contains("nope"), "{e}");
    }

    /// Every inherited `GIT_*` name is removed, not just a list of known ones.
    #[test]
    fn every_inherited_git_variable_is_removed() {
        let mut cmd = Command::new("git");
        let names = [
            "GIT_EXEC_PATH",
            "GIT_TEMPLATE_DIR",
            "GIT_SSL_NO_VERIFY",
            "GIT_TRACE",
            "GIT_DEFAULT_HASH",
            "GIT_ALLOW_PROTOCOL",
            "GIT_CONFIG_KEY_0",
            "GIT_FUTURE_VARIABLE",
            "Git_Dir",
            "git_exec_path",
            "SSH_ASKPASS",
            "GITX",
            "PATH",
            "GI",
        ];
        scrub_env(&mut cmd, names.map(OsString::from));
        let envs: Vec<(String, Option<String>)> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        for name in names
            .iter()
            .filter(|n| n.to_ascii_uppercase().starts_with("GIT_") || **n == "SSH_ASKPASS")
        {
            assert!(envs.contains(&(name.to_string(), None)), "{name}: {envs:?}");
        }
        for name in ["GITX", "PATH", "GI"] {
            assert!(envs.iter().all(|(k, _)| k != name), "{name}: {envs:?}");
        }
        for (k, v) in [
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GIT_ASKPASS", ""),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ] {
            assert!(envs.contains(&(k.to_string(), Some(v.to_string()))), "{k}");
        }
        // The real builder applies it to the real environment, and every setting is on the
        // command line, in order, before anything else.
        let real = git_command();
        let args: Vec<String> = real
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let want: Vec<String> = [
            "core.hooksPath=/dev/null",
            "core.fsmonitor=false",
            "protocol.allow=never",
            "protocol.https.allow=always",
            "http.lowSpeedLimit=1000",
            "http.lowSpeedTime=60",
            "gc.auto=0",
            "maintenance.auto=false",
            "credential.interactive=false",
            "core.askPass=",
        ]
        .iter()
        .flat_map(|kv| ["-c".to_string(), kv.to_string()])
        .collect();
        assert_eq!(args, want);
        for (k, _) in std::env::vars_os() {
            if k.as_encoded_bytes().starts_with(b"GIT_") {
                let set = real.get_envs().find(|(n, _)| *n == k).map(|(_, v)| v);
                assert!(set.is_some(), "{k:?} is inherited");
            }
        }
    }
}
