//! Conformance suites: generic checks every implementation of a backend trait must pass
//! (PLAN.md §3). Each adapter in `jpdf-backends` calls the suite for its trait from a test:
//!
//! ```ignore
//! let report = jpdf_traits::conformance::flate::run(&MyFlate, &MyFlate);
//! assert!(report.passed(), "{report}");
//! ```
//!
//! Suites never panic. A panic inside the implementation is caught and reported as a failure
//! (CONSTITUTION 5: a backend must return typed errors, never panic), so one run lists every
//! failing check instead of stopping at the first.

pub mod crypto;
pub mod flate;
pub mod font;
pub mod image;
pub mod raster;
mod vectors;

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// One failed check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// Stable check name (`suite.check`), used by tests and in CI logs.
    pub check: &'static str,
    /// What went wrong, for a human.
    pub detail: String,
}

/// The outcome of a conformance run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Suite name.
    pub suite: &'static str,
    /// Number of checks run (a check over many inputs counts once per input).
    pub checks_run: usize,
    failures: Vec<Failure>,
}

impl Report {
    fn new(suite: &'static str) -> Self {
        Self {
            suite,
            checks_run: 0,
            failures: Vec::new(),
        }
    }

    /// Whether every check passed and at least one ran.
    pub fn passed(&self) -> bool {
        self.failures.is_empty() && self.checks_run > 0
    }

    /// Every failed check, in the order they ran.
    pub fn failures(&self) -> &[Failure] {
        &self.failures
    }

    /// Whether the named check failed at least once.
    pub fn failed(&self, check: &str) -> bool {
        self.failures.iter().any(|f| f.check == check)
    }

    /// The distinct names of failed checks, sorted.
    pub fn failed_checks(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.failures.iter().map(|f| f.check).collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Records one check: passes when `ok` is true, otherwise adds a failure built by
    /// `detail`.
    fn check(&mut self, check: &'static str, ok: bool, detail: impl FnOnce() -> String) {
        self.checks_run = self.checks_run.saturating_add(1);
        if !ok {
            self.failures.push(Failure {
                check,
                detail: detail(),
            });
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.passed() {
            return write!(f, "{}: {} checks passed", self.suite, self.checks_run);
        }
        if self.checks_run == 0 {
            return write!(f, "{}: no checks ran", self.suite);
        }
        writeln!(
            f,
            "{}: {} of {} checks failed",
            self.suite,
            self.failures.len(),
            self.checks_run
        )?;
        for failure in &self.failures {
            writeln!(f, "  {}: {}", failure.check, failure.detail)?;
        }
        Ok(())
    }
}

/// Runs `call`, turning a panic into `None`.
fn guarded<T>(call: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(call)).ok()
}

/// Offsets at which to cut or mutate an input of `len` bytes: every offset for short inputs,
/// otherwise about `samples` evenly spaced ones plus the last 16 (where trailers live).
fn sample_offsets(len: usize, samples: usize) -> Vec<usize> {
    if len <= samples.saturating_add(16) {
        return (0..len).collect();
    }
    let step = (len / samples.max(1)).max(1);
    let mut offsets: Vec<usize> = (0..len).step_by(step).collect();
    offsets.extend(len.saturating_sub(16)..len);
    offsets.sort_unstable();
    offsets.dedup();
    offsets
}

/// Deterministic pseudo-random bytes (a 64-bit LCG), for round-trip inputs that do not
/// compress.
fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state.to_be_bytes().first().copied().unwrap_or(0)
        })
        .collect()
}

/// A short description of a result for failure messages, without dumping large outputs.
fn describe<T>(result: &Option<Result<T, crate::BackendError>>) -> String {
    match result {
        None => "panicked".into(),
        Some(Ok(_)) => "Ok".into(),
        Some(Err(e)) => format!("Err({e:?})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_does_not_pass() {
        let report = Report::new("x");
        assert!(!report.passed());
        assert_eq!(report.to_string(), "x: no checks ran");
    }

    #[test]
    fn report_lists_failures() {
        let mut report = Report::new("x");
        report.check("x.a", true, String::new);
        report.check("x.b", false, || "broke".into());
        report.check("x.b", false, || "again".into());
        assert!(!report.passed());
        assert!(report.failed("x.b"));
        assert!(!report.failed("x.a"));
        assert_eq!(report.failed_checks(), vec!["x.b"]);
        assert_eq!(
            report.to_string(),
            "x: 2 of 3 checks failed\n  x.b: broke\n  x.b: again\n"
        );
    }

    #[test]
    fn guarded_catches_panics() {
        assert_eq!(guarded(|| 3), Some(3));
        let caught = guarded(|| -> u8 { panic!("boom") });
        assert_eq!(caught, None);
    }

    #[test]
    fn sample_offsets_cover_short_inputs_and_trailers() {
        assert_eq!(sample_offsets(3, 256), vec![0, 1, 2]);
        let offsets = sample_offsets(10_000, 100);
        assert!(offsets.len() < 140);
        assert!(offsets.contains(&9_999));
        assert!(offsets.contains(&9_984));
        assert!(offsets.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn pseudo_random_is_deterministic() {
        assert_eq!(pseudo_random(8, 1), pseudo_random(8, 1));
        assert_ne!(pseudo_random(8, 1), pseudo_random(8, 2));
    }
}
