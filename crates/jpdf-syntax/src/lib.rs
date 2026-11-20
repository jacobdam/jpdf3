//! jpdf-syntax — see PLAN.md §2.3.
//!
//! Lexer, object parser and classic cross-reference reader. Everything here treats the input
//! as hostile: no panics, bounded recursion, checked arithmetic.
#![forbid(unsafe_code)]

mod error;
mod lexer;
mod limits;
mod object;
mod xref;

pub use error::SyntaxError;
pub use limits::Limits;
pub use object::{Dict, ObjRef, Object};
pub use xref::XrefDocument;
