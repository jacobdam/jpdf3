use std::fmt;

/// Typed errors from the syntax layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxError {
    /// The input violates ISO 32000-2 in a way we cannot recover from.
    Malformed { what: &'static str, offset: usize },
    /// A `Limits` bound was hit.
    LimitExceeded(&'static str),
    /// Valid PDF that this build does not handle yet.
    Unsupported(&'static str),
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed { what, offset } => write!(f, "malformed PDF: {what} at byte {offset}"),
            Self::LimitExceeded(which) => write!(f, "limit exceeded: {which}"),
            Self::Unsupported(what) => write!(f, "not implemented: {what}"),
        }
    }
}

impl std::error::Error for SyntaxError {}

pub(crate) fn malformed(what: &'static str, offset: usize) -> SyntaxError {
    SyntaxError::Malformed { what, offset }
}
