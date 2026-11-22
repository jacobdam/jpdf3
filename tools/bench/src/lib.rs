//! jpdf-bench — times `jpdf-cli` and reference engines, all as external programs
//! (CONSTITUTION 16). Spec: specs/tooling/bench.md.
//!
//! The bench never parses PDFs; it passes paths to other programs as argv lists and records
//! how long they took and how they exited (ops/judge/CONTRACT.md §1 exit codes).
#![forbid(unsafe_code)]

pub mod args;
pub mod bench;
pub mod engine;
pub mod json;
pub mod machine;
pub mod stats;
