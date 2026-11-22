//! jpdf-cos — see PLAN.md §2.3.
//!
//! Document object model. For now: the document catalog and a flat page tree.
#![forbid(unsafe_code)]
// Input-derived arithmetic is checked; overflow panics in every profile (#77).
#![deny(clippy::arithmetic_side_effects)]

use std::collections::HashSet;
use std::fmt;

use jpdf_syntax::{Limits, ObjRef, Object, SyntaxError, XrefDocument};

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
    // Beyond the resolve cache (bounded by `max_retained_bytes_per_document`), only one parsed
    // object is held at a time: our handles to the catalog and the page-tree root are dropped
    // once the references we need are extracted, so an uncached object is freed right away.
    let pages_ref = doc
        .resolve(root)?
        .as_dict()
        .ok_or(CosError::Malformed("catalog is not a dictionary"))?
        .get(b"Pages")
        .and_then(Object::as_ref)
        .ok_or(CosError::Malformed("catalog /Pages is not a reference"))?;
    let kids = kid_refs(&*doc.resolve(pages_ref)?, limits)?;
    let mut seen = HashSet::with_capacity(kids.len());
    for &r in &kids {
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
    // ISO 32000-2 §7.7.3.2 requires /Count to equal the number of leaves. Real files get it
    // wrong; read leniently (CONSTITUTION 17) and trust the kids we actually found.
    Ok(kids.len())
}

/// The `/Kids` of a page-tree node as compact references. ISO 32000-2 §7.7.3.2, Table 30.
fn kid_refs(node: &Object, limits: &Limits) -> Result<Vec<ObjRef>, CosError> {
    let kids = node
        .as_dict()
        .ok_or(CosError::Malformed("page tree root is not a dictionary"))?
        .get(b"Kids")
        .and_then(Object::as_array)
        .ok_or(CosError::Malformed("page tree /Kids is not an array"))?;
    if kids.len() > limits.max_pages {
        return Err(CosError::LimitExceeded("pages"));
    }
    kids.iter()
        .map(|k| {
            k.as_ref()
                .ok_or(CosError::Malformed("page tree kid is not a reference"))
        })
        .collect()
}
