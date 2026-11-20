use std::fmt;

/// Every way loading, checking or generating the model can fail.
#[derive(Debug)]
pub enum Error {
    /// A TSV file breaks the grammar (specs/tooling/arlington.md, "Grammar").
    Parse {
        object: String,
        /// 1-based line; 0 for whole-file problems.
        line: usize,
        message: String,
    },
    /// The parsed model is inconsistent (missing link target, duplicate key, …).
    Check(Vec<String>),
    /// A size limit was exceeded.
    Limit(String),
    /// Fetching or verifying the pinned commit failed.
    Fetch(String),
    Io {
        path: String,
        source: std::io::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse {
                object,
                line,
                message,
            } => write!(f, "{object}.tsv:{line}: {message}"),
            Self::Check(problems) => {
                write!(f, "{} model problem(s):", problems.len())?;
                for p in problems {
                    write!(f, "\n  {p}")?;
                }
                Ok(())
            }
            Self::Limit(m) => write!(f, "limit exceeded: {m}"),
            Self::Fetch(m) => write!(f, "fetch: {m}"),
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
