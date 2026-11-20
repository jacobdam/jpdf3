//! jpdf-cos — see PLAN.md §2.3.
//!
//! Document object model. For now: the document catalog and a flat page tree.
#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fmt;

use jpdf_syntax::{Limits, Object, SyntaxError, XrefDocument};

/// Typed errors from the object-model layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CosError {
    Syntax(SyntaxError),
    /// The object graph violates ISO 32000-2.
    Malformed(&'static str),
    /// A `Limits` bound was hit.
    LimitExceeded(&'static str),
    /// Valid structure this build does not handle yet.
    Unsupported(&'static str),
}

impl fmt::Display for CosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(e) => e.fmt(f),
            Self::Malformed(what) => write!(f, "malformed PDF: {what}"),
            Self::LimitExceeded(which) => write!(f, "limit exceeded: {which}"),
            Self::Unsupported(what) => write!(f, "not implemented: {what}"),
        }
    }
}

impl std::error::Error for CosError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Syntax(e) => Some(e),
            _ => None,
        }
    }
}

impl From<SyntaxError> for CosError {
    fn from(e: SyntaxError) -> Self {
        Self::Syntax(e)
    }
}

/// Number of pages in a document whose page tree is a single `/Pages` node with only
/// `/Page` leaves. ISO 32000-2 §7.7.2 (catalog), §7.7.3.2 (page tree nodes).
pub fn flat_page_count(doc: &XrefDocument<'_>, limits: &Limits) -> Result<usize, CosError> {
    let root = doc
        .trailer()
        .get(b"Root")
        .and_then(Object::as_ref)
        .ok_or(CosError::Malformed("trailer /Root is not a reference"))?;
    let catalog = doc.resolve(root)?;
    let pages_ref = catalog
        .as_dict()
        .ok_or(CosError::Malformed("catalog is not a dictionary"))?
        .get(b"Pages")
        .and_then(Object::as_ref)
        .ok_or(CosError::Malformed("catalog /Pages is not a reference"))?;
    let pages = doc.resolve(pages_ref)?;
    let pages = pages
        .as_dict()
        .ok_or(CosError::Malformed("page tree root is not a dictionary"))?;
    let kids = pages
        .get(b"Kids")
        .and_then(Object::as_array)
        .ok_or(CosError::Malformed("page tree /Kids is not an array"))?;
    if kids.len() > limits.max_pages {
        return Err(CosError::LimitExceeded("pages"));
    }
    let mut seen = HashSet::with_capacity(kids.len());
    for kid in kids {
        let r = kid
            .as_ref()
            .ok_or(CosError::Malformed("page tree kid is not a reference"))?;
        if r == pages_ref || !seen.insert(r) {
            return Err(CosError::Malformed("page tree node appears twice"));
        }
        let node = doc.resolve(r)?;
        let node_type = node
            .as_dict()
            .ok_or(CosError::Malformed("page tree kid is not a dictionary"))?
            .get(b"Type")
            .and_then(Object::as_name);
        match node_type {
            Some(b"Page") => {}
            Some(b"Pages") => return Err(CosError::Unsupported("nested page trees")),
            _ => return Err(CosError::Malformed("page tree kid has no valid /Type")),
        }
    }
    let count = pages
        .get(b"Count")
        .and_then(Object::as_integer)
        .ok_or(CosError::Malformed("page tree /Count is not an integer"))?;
    if usize::try_from(count).ok() != Some(kids.len()) {
        return Err(CosError::Malformed("page tree /Count does not match /Kids"));
    }
    Ok(kids.len())
}
