//! jpdf-engine — see PLAN.md §2.3.
//!
//! The facade every front end (CLI, bindings) goes through. The engine never touches the file
//! system: callers hand in bytes.
#![forbid(unsafe_code)]
// Input-derived arithmetic is checked; overflow panics in every profile (#77).
#![deny(clippy::arithmetic_side_effects)]

use std::fmt;

use jpdf_cos::CosError;
use jpdf_syntax::{SyntaxError, XrefDocument};

pub use jpdf_syntax::Limits;

/// Typed engine errors. Every failure on hostile input ends up here; nothing panics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// Invalid or corrupt input. `offset` is a byte position when known.
    Malformed {
        what: &'static str,
        offset: Option<usize>,
    },
    /// A `Limits` bound was hit.
    LimitExceeded(&'static str),
    /// Valid input that needs a feature this build lacks.
    Unsupported(&'static str),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed {
                what,
                offset: Some(o),
            } => write!(f, "malformed PDF: {what} at byte {o}"),
            Self::Malformed { what, offset: None } => write!(f, "malformed PDF: {what}"),
            Self::LimitExceeded(which) => write!(f, "limit exceeded: {which}"),
            Self::Unsupported(what) => write!(f, "not implemented: {what}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<SyntaxError> for EngineError {
    fn from(e: SyntaxError) -> Self {
        match e {
            SyntaxError::Malformed { what, offset } => Self::Malformed {
                what,
                offset: Some(offset),
            },
            SyntaxError::LimitExceeded(w) => Self::LimitExceeded(w),
            SyntaxError::Unsupported(w) => Self::Unsupported(w),
        }
    }
}

impl From<CosError> for EngineError {
    fn from(e: CosError) -> Self {
        match e {
            CosError::Syntax(s) => s.into(),
            CosError::Malformed(what) => Self::Malformed { what, offset: None },
            CosError::LimitExceeded(w) => Self::LimitExceeded(w),
            CosError::Unsupported(w) => Self::Unsupported(w),
        }
    }
}

/// An opened document.
#[derive(Debug)]
pub struct Document {
    page_count: usize,
    /// The clamped limits from `open`. Later operations on this document use these, so a
    /// caller can't hand in looser limits after opening (specs/core/limits-ceiling.md).
    limits: Limits,
}

impl Document {
    /// Opens a document from its bytes.
    ///
    /// `limits` can only tighten the defaults: every field is clamped to
    /// [`Limits::default`] first, because the value may come from Swift or JS
    /// (CONSTITUTION 5). A depth of 10^7, for example, would overflow the parser's stack.
    pub fn open(bytes: &[u8], limits: &Limits) -> Result<Self, EngineError> {
        let limits = limits.clamped_to(&Limits::default());
        let limits = &limits;
        if bytes.len() > limits.max_file_size {
            return Err(EngineError::LimitExceeded("file size"));
        }
        let xref = XrefDocument::parse(bytes, limits)?;
        let page_count = jpdf_cos::flat_page_count(&xref, limits)?;
        Ok(Self {
            page_count,
            limits: limits.clone(),
        })
    }

    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// The limits this document enforces: the caller's, clamped to the defaults.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }
}
