//! The vendored copy of the pinned Arlington data (specs/tooling/arlington.md, "Vendored
//! data"; ADR 0007, owner ruling on #60).
//!
//! `tools/arlington/data/` holds `tsv/` (every file of the pinned `tsv/latest`, byte for byte)
//! and upstream's `LICENSE` and `NOTICE.txt` from the pinned commit. Loading it is offline:
//! the files are read once, and their git ids are computed from those same bytes and compared
//! with the pins in [`crate::fetch`]. A hand-edited, missing or extra file is refused.

use std::io::Read as _;
use std::path::Path;

use crate::parse::{MAX_FILE_BYTES, MAX_FILES, MAX_TOTAL_BYTES};
use crate::{Error, fetch, sha1};

/// The vendored directory, relative to the repository root.
pub const DATA_DIR: &str = "tools/arlington/data";
/// The TSV subdirectory of a vendored directory.
pub const TSV_SUBDIR: &str = "tsv";
/// Written into a new vendored directory so git never converts line endings (the tree id
/// covers the exact bytes).
pub const GITATTRIBUTES: &str = "# Vendored verbatim from the pinned Arlington commit; \
     the git ids are checked (ADR 0007).\n* -text\n";

/// The pins a vendored directory is checked against.
#[derive(Debug, Clone, Copy)]
pub struct Pins<'a> {
    pub tree: &'a str,
    pub license: &'a str,
    pub notice: &'a str,
}

/// The pins in [`crate::fetch`].
pub const PINNED: Pins<'static> = Pins {
    tree: fetch::TSV_TREE,
    license: fetch::LICENSE_BLOB,
    notice: fetch::NOTICE_BLOB,
};

/// Size caps for a vendored directory. [`Caps::DEFAULT`] is the spec's Limits; tests use
/// smaller values to reach every boundary cheaply.
#[derive(Debug, Clone, Copy)]
pub struct Caps {
    pub files: usize,
    pub file_bytes: usize,
    pub total_bytes: usize,
}

impl Caps {
    pub const DEFAULT: Caps = Caps {
        files: MAX_FILES,
        file_bytes: MAX_FILE_BYTES,
        total_bytes: MAX_TOTAL_BYTES,
    };
}

/// The only entries a vendored directory may hold.
const TOP_LEVEL: [&str; 4] = [TSV_SUBDIR, "LICENSE", "NOTICE.txt", ".gitattributes"];

/// Loads the vendored directory `dir` checked against [`PINNED`].
pub fn load_vendored(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, Error> {
    load_checked(dir, PINNED, Caps::DEFAULT)
}

/// Loads `dir/tsv/*.tsv` and checks the whole directory against `pins`:
/// - `dir` holds exactly `tsv/`, `LICENSE`, `NOTICE.txt` and `.gitattributes`, none a symlink;
/// - the notice files have their pinned blob ids, and `.gitattributes` is [`GITATTRIBUTES`];
/// - `tsv/` holds only regular `.tsv` files, whose tree id is the pinned one.
///
/// Returns the TSV files sorted by name: the same bytes that were hashed.
pub fn load_checked(
    dir: &Path,
    pins: Pins<'_>,
    caps: Caps,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let tsv = dir.join(TSV_SUBDIR);
    check_top_level(dir, &tsv)?;
    for (name, want) in [("LICENSE", pins.license), ("NOTICE.txt", pins.notice)] {
        let bytes = read_capped(&dir.join(name), name, caps.file_bytes)?;
        let got = sha1::hex(&sha1::object_id("blob", &bytes));
        if got != want {
            return Err(Error::Vendor(format!(
                "{name} is blob {got}, pinned {want}"
            )));
        }
    }
    let attrs = read_capped(
        &dir.join(".gitattributes"),
        ".gitattributes",
        caps.file_bytes,
    )?;
    if attrs != GITATTRIBUTES.as_bytes() {
        return Err(Error::Vendor(
            ".gitattributes differs from the one `vendor` writes".into(),
        ));
    }
    let io = |source| Error::Io {
        path: tsv.display().to_string(),
        source,
    };
    let mut files = Vec::new();
    let mut total: usize = 0;
    for entry in std::fs::read_dir(&tsv).map_err(io)? {
        let entry = entry.map_err(io)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|n| Error::Vendor(format!("{TSV_SUBDIR}/{n:?} is not a UTF-8 name")))?;
        // `DirEntry::file_type` does not follow symlinks, so a link is refused here.
        if !name.ends_with(".tsv") || !entry.file_type().map_err(io)?.is_file() {
            return Err(Error::Vendor(format!(
                "{TSV_SUBDIR}/{name} is not a regular .tsv file (re-vendor or remove stray files)"
            )));
        }
        if files.len() >= caps.files {
            return Err(Error::Limit(format!("more than {} files", caps.files)));
        }
        let bytes = read_capped(&entry.path(), &name, caps.file_bytes)?;
        total = total.saturating_add(bytes.len());
        if total > caps.total_bytes {
            return Err(Error::Limit(format!(
                "more than {} bytes in all files",
                caps.total_bytes
            )));
        }
        files.push((name, bytes));
    }
    // Byte order of the names: git's order for a tree of blobs.
    files.sort();
    let got = sha1::hex(&sha1::tree_id(&files));
    if got != pins.tree {
        return Err(Error::Vendor(format!(
            "{TSV_SUBDIR} is tree {got}, pinned {} (the data must equal the pinned \
             {}; re-vendor instead of editing it)",
            pins.tree,
            fetch::TSV_DIR
        )));
    }
    Ok(files)
}

/// `dir` and `dir/tsv` are real directories (not symlinks), and `dir` holds only
/// [`TOP_LEVEL`] entries.
fn check_top_level(dir: &Path, tsv: &Path) -> Result<(), Error> {
    for d in [dir, tsv] {
        let meta = std::fs::symlink_metadata(d).map_err(|source| Error::Io {
            path: d.display().to_string(),
            source,
        })?;
        if !meta.is_dir() {
            return Err(Error::Vendor(format!(
                "{} is not a directory (symlinks are refused)",
                d.display()
            )));
        }
    }
    let io = |source| Error::Io {
        path: dir.display().to_string(),
        source,
    };
    for entry in std::fs::read_dir(dir).map_err(io)? {
        let name = entry.map_err(io)?.file_name();
        if !TOP_LEVEL.iter().any(|t| name == *t) {
            return Err(Error::Vendor(format!(
                "unexpected entry {name:?} (re-vendor or remove stray files)"
            )));
        }
    }
    Ok(())
}

/// Reads at most `cap` bytes; a larger file is a limit error, and no read goes past the cap.
/// The path must not be a symlink, and the opened file must be a regular file (checked again
/// on the open handle, so a swap to a FIFO or device after the first check is refused).
fn read_capped(path: &Path, name: &str, cap: usize) -> Result<Vec<u8>, Error> {
    let io = |source| Error::Io {
        path: path.display().to_string(),
        source,
    };
    let not_regular = || Error::Vendor(format!("{name} is not a regular file"));
    if !std::fs::symlink_metadata(path).map_err(io)?.is_file() {
        return Err(not_regular());
    }
    let file = std::fs::File::open(path).map_err(io)?;
    if !file.metadata().map_err(io)?.is_file() {
        return Err(not_regular());
    }
    let mut bytes = Vec::new();
    let limit = u64::try_from(cap).unwrap_or(u64::MAX).saturating_add(1);
    file.take(limit).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() > cap {
        return Err(Error::Limit(format!("{name} is over {cap} bytes")));
    }
    Ok(bytes)
}

/// Writes a new vendored directory at `out` (which must not exist) from the pinned commit in
/// the cache `cache_dir`, fetching it if needed, then loads it back through [`load_vendored`].
/// Used when bumping the pin (spec "Bumping the pin"). Returns the number of TSV files.
pub fn vendor_pinned(cache_dir: &Path, out: &Path) -> Result<usize, Error> {
    refuse_existing(out)?;
    // Fetches if needed and verifies; `vendor_from` verifies the same cache again.
    fetch::load_pinned(cache_dir)?;
    vendor_from(&fetch::repo_path(cache_dir), fetch::COMMIT, PINNED, out)
}

/// [`vendor_pinned`] from an existing repository `git_dir` and explicit pins: verifies it
/// ([`fetch::load_verified`]), reads the notice files by their pinned blob ids, writes a
/// sibling staging directory, loads it back through [`load_checked`], and only then renames
/// it to `out`. On any failure the staging directory is removed and `out` is not created.
pub fn vendor_from(
    git_dir: &Path,
    commit: &str,
    pins: Pins<'_>,
    out: &Path,
) -> Result<usize, Error> {
    refuse_existing(out)?;
    let files = fetch::load_verified(git_dir, commit, pins.tree)?;
    let mut notices = Vec::new();
    for (name, id) in [("LICENSE", pins.license), ("NOTICE.txt", pins.notice)] {
        notices.push((name, fetch::read_pinned_blob(git_dir, commit, name, id)?));
    }
    let file_name = out
        .file_name()
        .ok_or_else(|| Error::Vendor(format!("{} has no file name", out.display())))?;
    let mut staged_name = file_name.to_os_string();
    staged_name.push(format!(".staging-{}", std::process::id()));
    let staged = out.with_file_name(staged_name);
    let result = write_dir(&staged, &files, &notices)
        .and_then(|()| load_checked(&staged, pins, Caps::DEFAULT))
        .and_then(|loaded| {
            refuse_existing(out)?;
            std::fs::rename(&staged, out).map_err(|source| Error::Io {
                path: out.display().to_string(),
                source,
            })?;
            Ok(loaded.len())
        });
    if result.is_err() {
        // Only ever a directory this call created (write_dir refuses an existing path).
        let _ = std::fs::remove_dir_all(&staged);
    }
    result
}

fn refuse_existing(out: &Path) -> Result<(), Error> {
    // `symlink_metadata` also sees a dangling symlink.
    if std::fs::symlink_metadata(out).is_ok() {
        return Err(Error::Vendor(format!(
            "{} exists; delete it first",
            out.display()
        )));
    }
    Ok(())
}

/// Writes `out/tsv/<files>`, the notice files and `.gitattributes`. `out` must not exist.
/// Every name is checked before anything is created.
pub fn write_dir(
    out: &Path,
    files: &[(String, Vec<u8>)],
    notices: &[(&str, Vec<u8>)],
) -> Result<(), Error> {
    let io = |path: &Path| {
        let path = path.display().to_string();
        move |source| Error::Io { path, source }
    };
    // Names come from the verified tree listing; refuse anything that isn't a plain name.
    for (name, _) in files {
        if name.is_empty()
            || name.contains(['/', '\\'])
            || name.starts_with('.')
            || !name.ends_with(".tsv")
        {
            return Err(Error::Vendor(format!("bad file name {name:?}")));
        }
    }
    for (name, _) in notices {
        if !["LICENSE", "NOTICE.txt"].contains(name) {
            return Err(Error::Vendor(format!("bad notice name {name:?}")));
        }
    }
    refuse_existing(out)?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(io(parent))?;
    }
    // `create_dir` (not `_all`) also fails if `out` appeared since the check.
    std::fs::create_dir(out).map_err(io(out))?;
    let tsv = out.join(TSV_SUBDIR);
    std::fs::create_dir(&tsv).map_err(io(&tsv))?;
    for (name, bytes) in files {
        let p = tsv.join(name);
        std::fs::write(&p, bytes).map_err(io(&p))?;
    }
    for (name, bytes) in notices {
        let p = out.join(name);
        std::fs::write(&p, bytes).map_err(io(&p))?;
    }
    let p = out.join(".gitattributes");
    std::fs::write(&p, GITATTRIBUTES).map_err(io(&p))
}
