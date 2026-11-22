//! `fetch`: download a tier with hash checks (specs/tooling/corpus.md, item 6).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use crate::manifest::{Entry, encode_url};
use crate::sha1::{Sha1, hex};

/// Attempts per file.
pub const ATTEMPTS: u32 = 3;
/// Allowed `--jobs` range.
pub const MAX_JOBS: usize = 32;
/// Bytes of curl's stderr kept for the error message.
const MAX_STDERR: u64 = 64 * 1024;

/// How to fetch.
#[derive(Debug, Clone)]
pub struct Options {
    /// The curl program (`curl` on `PATH`; tests pass a stand-in).
    pub curl: PathBuf,
    /// Parallel downloads, 1..=[`MAX_JOBS`].
    pub jobs: usize,
    /// Delay before the second attempt; doubled before the third.
    pub retry_delay: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            curl: PathBuf::from("curl"),
            jobs: 8,
            retry_delay: Duration::from_secs(1),
        }
    }
}

/// What happened to the rows.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub total: usize,
    /// Already present and verified.
    pub cached: usize,
    /// Downloaded and verified.
    pub downloaded: usize,
    /// `<url>: <problem>` for rows that could not be downloaded.
    pub failed: Vec<String>,
    /// `<url>: <problem>` for rows whose bytes had the wrong size or hash.
    pub integrity: Vec<String>,
}

impl Report {
    /// The judge's infrastructure rule: more than `max(1, total / 100)` rows missing.
    pub fn is_infra_failure(&self) -> bool {
        self.failed.len() > (self.total / 100).max(1)
    }

    /// The process exit code (spec "Errors"): 1 on any integrity failure (checked first),
    /// 75 above the infrastructure threshold, else 0.
    pub fn exit_code(&self) -> u8 {
        if !self.integrity.is_empty() {
            1
        } else if self.is_infra_failure() {
            75
        } else {
            0
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "fetched {}/{} ({} cached, {} failed, {} integrity)",
            self.cached.saturating_add(self.downloaded),
            self.total,
            self.cached,
            self.failed.len(),
            self.integrity.len()
        )
    }
}

enum Outcome {
    Cached,
    Downloaded,
    Failed(String),
    Integrity(String),
}

/// The rows a fetch covers: blob-id order, then the first `limit` rows.
pub fn select(mut entries: Vec<Entry>, limit: Option<usize>) -> Vec<Entry> {
    entries.sort_by(|x, y| x.blob.cmp(&y.blob));
    if let Some(n) = limit {
        entries.truncate(n);
    }
    entries
}

/// Downloads `entries` into `dir/<blob>.pdf` (the directory must exist).
pub fn fetch(entries: &[Entry], dir: &Path, opts: &Options) -> Report {
    let next = AtomicUsize::new(0);
    let report = Mutex::new(Report {
        total: entries.len(),
        ..Report::default()
    });
    let jobs = opts.jobs.clamp(1, MAX_JOBS).min(entries.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                while let Some(e) = entries.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let outcome = fetch_one(e, dir, opts);
                    let mut r = report.lock().unwrap_or_else(PoisonError::into_inner);
                    match outcome {
                        Outcome::Cached => r.cached = r.cached.saturating_add(1),
                        Outcome::Downloaded => r.downloaded = r.downloaded.saturating_add(1),
                        Outcome::Failed(m) => r.failed.push(format!("{}: {m}", e.url)),
                        Outcome::Integrity(m) => r.integrity.push(format!("{}: {m}", e.url)),
                    }
                }
            });
        }
    });
    let mut r = report.into_inner().unwrap_or_else(PoisonError::into_inner);
    r.failed.sort();
    r.integrity.sort();
    r
}

/// Why a file failed [`verify`].
enum VerifyError {
    /// Opening or reading failed (not evidence about the bytes).
    Io(String),
    /// Wrong size or blob id.
    Mismatch(String),
}

/// Streams the file through SHA-1 (fixed buffer), reading at most `size + 1` bytes, and
/// checks its length and git blob id.
fn verify(path: &Path, e: &Entry) -> Result<(), VerifyError> {
    let io = |x: std::io::Error| VerifyError::Io(x.to_string());
    // Only a regular file: a FIFO planted in the cache would block a plain open forever, so
    // the open is non-blocking and the opened handle is checked again.
    let not_file = || VerifyError::Io("not a regular file".into());
    if !std::fs::metadata(path).map_err(io)?.is_file() {
        return Err(not_file());
    }
    let file = jpdf_fsio::open_nonblocking(path).map_err(io)?;
    if !file.metadata().map_err(io)?.is_file() {
        return Err(not_file());
    }
    let mut file = file.take(e.size.saturating_add(1));
    let mut h = Sha1::new();
    h.update(format!("blob {}\0", e.size).as_bytes());
    let mut buf = [0u8; 64 * 1024];
    let mut len: u64 = 0;
    loop {
        let n = match file.read(&mut buf) {
            Ok(n) => n,
            Err(x) if x.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(x) => return Err(io(x)),
        };
        if n == 0 {
            break;
        }
        h.update(buf.get(..n).unwrap_or_default());
        len = len.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
    }
    if len != e.size {
        let got = if len > e.size {
            format!("more than {}", e.size)
        } else {
            len.to_string()
        };
        return Err(VerifyError::Mismatch(format!(
            "size is {got}, expected {}",
            e.size
        )));
    }
    let id = hex(&h.finish());
    if id != e.blob {
        return Err(VerifyError::Mismatch(format!(
            "blob id is {id}, expected {}",
            e.blob
        )));
    }
    Ok(())
}

/// Distinguishes `.part` files of concurrent runs and of calls within one run.
static PART_SEQ: AtomicUsize = AtomicUsize::new(0);

fn fetch_one(e: &Entry, dir: &Path, opts: &Options) -> Outcome {
    let dst = dir.join(format!("{}.pdf", e.blob));
    if verify(&dst, e).is_ok() {
        return Outcome::Cached;
    }
    let seq = PART_SEQ.fetch_add(1, Ordering::Relaxed);
    let part = dir.join(format!("{}.{}.{seq}.part", e.blob, std::process::id()));
    let mut delay = opts.retry_delay;
    let mut last = String::new();
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(delay);
            delay = delay.saturating_mul(2);
        }
        let _ = std::fs::remove_file(&part);
        match run_curl(opts, e, &part) {
            Ok(()) => {}
            // 63: the server sent more than the manifest size.
            Err((Some(63), m)) => {
                let _ = std::fs::remove_file(&part);
                return Outcome::Integrity(m);
            }
            Err((_, m)) => {
                last = m;
                continue;
            }
        }
        return match verify(&part, e) {
            Ok(()) => match std::fs::rename(&part, &dst) {
                Ok(()) => Outcome::Downloaded,
                Err(x) => {
                    let _ = std::fs::remove_file(&part);
                    Outcome::Failed(format!("rename: {x}"))
                }
            },
            Err(VerifyError::Mismatch(m)) => {
                let _ = std::fs::remove_file(&part);
                Outcome::Integrity(m)
            }
            Err(VerifyError::Io(m)) => {
                let _ = std::fs::remove_file(&part);
                Outcome::Failed(format!("reading the download: {m}"))
            }
        };
    }
    let _ = std::fs::remove_file(&part);
    Outcome::Failed(last)
}

/// Runs curl with the body on stdout and copies at most `size + 1` bytes of it into `part`,
/// which must not exist yet (`create_new`, so a planted file or symlink is never written
/// through). curl < 8.4 doesn't enforce `--max-filesize` without a Content-Length; the cap
/// here does. On overflow curl is killed and the result is curl's exit 63 (integrity).
/// Redirects fail (`--max-redirs 0`, curl exit 47): no source host redirects, and a
/// redirect target's host was never checked.
fn run_curl(opts: &Options, e: &Entry, part: &Path) -> Result<(), (Option<i32>, String)> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(part)
        .map_err(|x| (None, format!("creating {}: {x}", part.display())))?;
    let mut child = Command::new(&opts.curl)
        // `-q` must come first: it stops curl from reading a .curlrc.
        .args([
            "-q",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-redirs",
            "0",
            "--connect-timeout",
            "30",
            "--max-time",
            "300",
            "--max-filesize",
        ])
        .arg(e.size.to_string())
        .arg("--url")
        .arg(encode_url(&e.url))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|x| (None, format!("cannot run {}: {x}", opts.curl.display())))?;
    let stderr = child.stderr.take().map(|pipe| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut pipe = pipe;
            let _ = (&mut pipe).take(MAX_STDERR).read_to_end(&mut buf);
            // Drain the rest, so a chatty curl never blocks on a full stderr pipe.
            let _ = std::io::copy(&mut pipe, &mut std::io::sink());
            buf
        })
    });
    let cap = e.size.saturating_add(1);
    let copied = match child.stdout.take() {
        Some(out) => std::io::copy(&mut out.take(cap), &mut file),
        None => Ok(0),
    };
    let overflow = matches!(copied, Ok(n) if n >= cap);
    if overflow {
        // More than the manifest size: stop curl instead of draining the rest.
        let _ = child.kill();
    }
    let status = child.wait();
    let stderr = stderr.and_then(|h| h.join().ok()).unwrap_or_default();
    if overflow {
        // Reported like curl's own exit 63 (an integrity failure, no retry), with the count
        // of bytes that reached `part`.
        let copied = copied.unwrap_or(0);
        return Err((
            Some(63),
            format!(
                "size is more than {}, expected {} (stopped after {copied} bytes)",
                e.size, e.size
            ),
        ));
    }
    let status = status.map_err(|x| (None, format!("curl: {x}")))?;
    if !status.success() {
        let code = status.code();
        let mut msg = String::from_utf8_lossy(&stderr).trim().to_string();
        msg.truncate(msg.floor_char_boundary(300));
        return Err((code, format!("curl exit {code:?}: {msg}")));
    }
    match copied {
        Ok(_) => Ok(()),
        Err(x) => Err((None, format!("writing {}: {x}", part.display()))),
    }
}
