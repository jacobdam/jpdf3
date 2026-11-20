use std::fmt;

/// The one error type every backend trait returns. It is deliberately small: callers branch
/// on the variant (a limit hit is reported differently from a damaged stream), and the
/// conformance suites check the variant for each kind of bad input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// The input violates its format in a way the backend cannot recover from.
    Malformed(&'static str),
    /// The input ended before any usable output could be produced. A backend that can
    /// salvage output returns it with [`crate::DecodeStatus::Truncated`] instead.
    Truncated,
    /// A [`crate::DecodeLimits`] bound (or another `Limits` value) would be exceeded.
    LimitExceeded(&'static str),
    /// Valid input that this backend does not handle.
    Unsupported(&'static str),
    /// A caller-supplied argument is invalid (key length, zero columns, …). This is a bug in
    /// the caller or hostile input that bypassed validation, never a property of the stream.
    InvalidArgument(&'static str),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(what) => write!(f, "malformed input: {what}"),
            Self::Truncated => f.write_str("input truncated"),
            Self::LimitExceeded(which) => write!(f, "limit exceeded: {which}"),
            Self::Unsupported(what) => write!(f, "not supported: {what}"),
            Self::InvalidArgument(what) => write!(f, "invalid argument: {what}"),
        }
    }
}

impl std::error::Error for BackendError {}
