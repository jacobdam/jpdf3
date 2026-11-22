//! Arlington PDF Model ingestion (specs/tooling/arlington.md, issue #20, ADR 0007).
//!
//! Loads the PDF Association's Arlington model (machine-readable ISO 32000-2 object
//! definitions) from its vendored copy in `tools/arlington/data` (checked against the pinned
//! git ids) or from the pinned commit, parses its TSV files into a typed [`Model`], checks
//! it, and renders Rust tables for tests and fuzz seeds. Development tool only: nothing here
//! ships (owner ruling on #60: tests and tools only).
#![forbid(unsafe_code)]

pub mod check;
mod error;
pub mod fetch;
pub mod generate;
pub mod model;
pub mod parse;
pub mod sha1;
pub mod vendor;

pub use error::Error;
pub use model::Model;

/// Where a model's files come from.
#[derive(Debug, Clone)]
pub enum Source {
    /// The vendored copy in this directory, checked against the pins (the default).
    Vendored(std::path::PathBuf),
    /// The pinned commit, cached in this directory (fetched if missing).
    Pinned(std::path::PathBuf),
    /// A local directory of `.tsv` files (tests, offline experiments).
    Dir(std::path::PathBuf),
}

impl Source {
    /// A one-line description for generated headers and logs.
    pub fn describe(&self) -> String {
        match self {
            Self::Vendored(_) => format!("commit {} ({}, vendored)", fetch::COMMIT, fetch::TSV_DIR),
            Self::Pinned(_) => format!("commit {} ({})", fetch::COMMIT, fetch::TSV_DIR),
            Self::Dir(d) => format!("local directory {}", d.display()),
        }
    }
}

/// Loads, parses and checks the model.
pub fn load(source: &Source) -> Result<Model, Error> {
    let files = match source {
        Source::Vendored(dir) => vendor::load_vendored(dir)?,
        Source::Pinned(cache) => fetch::load_pinned(cache)?,
        Source::Dir(dir) => read_dir(dir)?,
    };
    let model = parse::parse_files(&files)?;
    check::check(&model)?;
    Ok(model)
}

/// Reads every `*.tsv` file in `dir` (not recursive), sorted by name. Other files are
/// ignored; files over the size limit are an error, and no read goes past the limit.
pub fn read_dir(dir: &std::path::Path) -> Result<Vec<(String, Vec<u8>)>, Error> {
    use std::io::Read as _;
    let io = |source| Error::Io {
        path: dir.display().to_string(),
        source,
    };
    let mut files = Vec::new();
    let mut total: usize = 0;
    for entry in std::fs::read_dir(dir).map_err(io)? {
        let entry = entry.map_err(io)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".tsv") || !entry.file_type().map_err(io)?.is_file() {
            continue;
        }
        if files.len() >= parse::MAX_FILES {
            return Err(Error::Limit(format!(
                "more than {} files",
                parse::MAX_FILES
            )));
        }
        let path = entry.path();
        // Bounded read: a file that grows after `metadata` still can't exceed the cap. The
        // open never waits for a FIFO writer where `jpdf_fsio::OPEN_FLAGS` is set
        // (specs/tooling/fsio.md), and the opened handle must still be a
        // regular file, so a FIFO swapped in after the entry check is refused.
        let mut bytes = Vec::new();
        jpdf_fsio::open_nonblocking(&path)
            .and_then(|f| {
                if !f.metadata()?.is_file() {
                    return Err(std::io::Error::other("not a regular file"));
                }
                f.take(parse::MAX_FILE_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
            })
            .map_err(|source| Error::Io {
                path: path.display().to_string(),
                source,
            })?;
        if bytes.len() > parse::MAX_FILE_BYTES {
            return Err(Error::Limit(format!(
                "{name} is over {} bytes",
                parse::MAX_FILE_BYTES
            )));
        }
        total = total.saturating_add(bytes.len());
        if total > parse::MAX_TOTAL_BYTES {
            return Err(Error::Limit(format!(
                "more than {} bytes in all files",
                parse::MAX_TOTAL_BYTES
            )));
        }
        files.push((name, bytes));
    }
    files.sort();
    Ok(files)
}
