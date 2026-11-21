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

/// Loads the vendored directory `dir` checked against [`PINNED`].
pub fn load_vendored(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, Error> {
    load_checked(dir, PINNED)
}

/// Loads `dir/tsv/*.tsv`, refusing anything else in `dir/tsv`, and checks the notice files'
/// blob ids and the TSV tree id against `pins`. Returns the files sorted by name.
pub fn load_checked(dir: &Path, pins: Pins<'_>) -> Result<Vec<(String, Vec<u8>)>, Error> {
    for (name, want) in [("LICENSE", pins.license), ("NOTICE.txt", pins.notice)] {
        let bytes = read_capped(&dir.join(name), name)?;
        let got = sha1::hex(&sha1::object_id("blob", &bytes));
        if got != want {
            return Err(Error::Vendor(format!(
                "{name} is blob {got}, pinned {want}"
            )));
        }
    }
    let tsv = dir.join(TSV_SUBDIR);
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
                "{TSV_SUBDIR}/{name} is not a regular .tsv file"
            )));
        }
        if files.len() >= MAX_FILES {
            return Err(Error::Limit(format!("more than {MAX_FILES} files")));
        }
        let bytes = read_capped(&entry.path(), &name)?;
        total = total.saturating_add(bytes.len());
        if total > MAX_TOTAL_BYTES {
            return Err(Error::Limit(format!(
                "more than {MAX_TOTAL_BYTES} bytes in all files"
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

/// Reads at most [`MAX_FILE_BYTES`]; a larger file is a limit error, and no read goes past
/// the cap. Symlinks are not followed for TSV files (checked by the caller); for the notice
/// files, `symlink_metadata` refuses them here.
fn read_capped(path: &Path, name: &str) -> Result<Vec<u8>, Error> {
    let io = |source| Error::Io {
        path: path.display().to_string(),
        source,
    };
    if !std::fs::symlink_metadata(path).map_err(io)?.is_file() {
        return Err(Error::Vendor(format!("{name} is not a regular file")));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(MAX_FILE_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(io)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(Error::Limit(format!(
            "{name} is over {MAX_FILE_BYTES} bytes"
        )));
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
/// ([`fetch::load_verified`]), reads the notice files by their pinned blob ids, writes `out`
/// and loads it back through [`load_checked`].
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
    write_dir(out, &files, &notices)?;
    load_checked(out, pins).map(|f| f.len())
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
pub fn write_dir(
    out: &Path,
    files: &[(String, Vec<u8>)],
    notices: &[(&str, Vec<u8>)],
) -> Result<(), Error> {
    let io = |path: &Path| {
        let path = path.display().to_string();
        move |source| Error::Io { path, source }
    };
    refuse_existing(out)?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(io(parent))?;
    }
    // `create_dir` (not `_all`) also fails if `out` appeared since the check.
    std::fs::create_dir(out).map_err(io(out))?;
    let tsv = out.join(TSV_SUBDIR);
    std::fs::create_dir(&tsv).map_err(io(&tsv))?;
    for (name, bytes) in files {
        // Names come from the verified tree listing; refuse anything that isn't a plain name.
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
            return Err(Error::Vendor(format!("bad file name {name:?}")));
        }
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
