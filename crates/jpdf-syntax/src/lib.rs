//! jpdf-syntax — see PLAN.md §2.3.
//!
//! Lexer, object parser and classic cross-reference reader. Everything here treats the input
//! as hostile: no panics, bounded recursion, checked arithmetic.
#![forbid(unsafe_code)]
// Input-derived arithmetic is checked; overflow panics in every profile (#77).
#![deny(clippy::arithmetic_side_effects)]

mod budget;
mod error;
mod lexer;
mod object;
mod xref;

pub use budget::{
    MemoryBudget, PAGE_WALK_BYTES, PARSE_OBJECT_BYTES, PROCESS_BUDGET_BYTES, RESERVE_BYTES,
    SPAN_ENTRY_BYTES,
};
pub use error::SyntaxError;
pub use jpdf_traits::Limits;
pub use object::{Dict, ObjRef, Object};
pub use xref::XrefDocument;
