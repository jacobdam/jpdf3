//! `fetch`: download a tier with hash checks (specs/tooling/corpus.md, item 6).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::manifest::{Entry, encode_url};
use crate::sha1::git_blob_id;

/// Attempts per file.
pub const ATTEMPTS: u32 = 3;
/// Allowed `--jobs` range.
pub const MAX_JOBS: usize = 32;

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
                    let Ok(mut r) = report.lock() else { return };
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
    let mut r = report.into_inner().unwrap_or_default();
    r.failed.sort();
    r.integrity.sort();
    r
}

/// Reads at most `size + 1` bytes and checks length and blob id.
fn verify(path: &Path, e: &Entry) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|x| x.to_string())?;
    let mut bytes = Vec::new();
    file.take(e.size.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|x| x.to_string())?;
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if len != e.size {
        return Err(format!("size is {len}, expected {}", e.size));
    }
    let id = git_blob_id(&bytes);
    if id != e.blob {
        return Err(format!("blob id is {id}, expected {}", e.blob));
    }
    Ok(())
}

fn fetch_one(e: &Entry, dir: &Path, opts: &Options) -> Outcome {
    let dst = dir.join(format!("{}.pdf", e.blob));
    if verify(&dst, e).is_ok() {
        return Outcome::Cached;
    }
    let part = dir.join(format!("{}.part", e.blob));
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
            Err(m) => {
                let _ = std::fs::remove_file(&part);
                Outcome::Integrity(m)
            }
        };
    }
    let _ = std::fs::remove_file(&part);
    Outcome::Failed(last)
}

fn run_curl(opts: &Options, e: &Entry, part: &Path) -> Result<(), (Option<i32>, String)> {
    let out = Command::new(&opts.curl)
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
            "5",
            "--connect-timeout",
            "30",
            "--max-time",
            "300",
            "--max-filesize",
        ])
        .arg(e.size.to_string())
        .arg("--output")
        .arg(part)
        .arg("--url")
        .arg(encode_url(&e.url))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|x| (None, format!("cannot run {}: {x}", opts.curl.display())))?;
    if out.status.success() {
        Ok(())
    } else {
        let code = out.status.code();
        let mut msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        msg.truncate(msg.floor_char_boundary(300));
        Err((code, format!("curl exit {code:?}: {msg}")))
    }
}
