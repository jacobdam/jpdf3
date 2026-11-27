//! The step log of a multi-step `cargo xtask` command (ADR 0013, docs/PRINCIPLES.md P4): each
//! step's result is an [`Outcome`] recorded under the step's name, one driver per tool decides
//! the order, and the exit code is [`exit_code`] of the log alone. Spec:
//! specs/tooling/nightly.md items 10 and 15 (the first tool on it).

/// How one step ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Done; also a measured result the spec records without failing the tool (a regress
    /// run's `crash`, a bench `timeout`, a fuzz `crash`).
    Ok,
    /// A result the spec gives exit 4 (today only a failed limits case).
    Finding,
    /// An infrastructure failure (exit 75), with why.
    Infra(String),
    /// A tool failure (exit 1), with why.
    Failed(String),
}

impl Outcome {
    /// Precedence rank: `Failed` beats `Infra` beats `Finding` beats `Ok`.
    fn rank(&self) -> u8 {
        match self {
            Outcome::Ok => 0,
            Outcome::Finding => 1,
            Outcome::Infra(_) => 2,
            Outcome::Failed(_) => 3,
        }
    }

    fn code(&self) -> u8 {
        match self {
            Outcome::Ok => 0,
            Outcome::Finding => 4,
            Outcome::Infra(_) => 75,
            Outcome::Failed(_) => 1,
        }
    }
}

/// The steps that ran, in the order they ran, each with its outcome.
#[derive(Debug, Default)]
pub struct Log {
    /// Printed before a `Failed` reason when it is recorded (`xtask: nightly: `); `None`
    /// prints nothing.
    prefix: Option<&'static str>,
    entries: Vec<(&'static str, Outcome)>,
}

impl Log {
    /// A log that prints each `Failed` reason on stderr as `<prefix><reason>` when it is
    /// recorded, so a tool failure shows when it happens, not only at the exit.
    pub fn printing(prefix: &'static str) -> Log {
        Log {
            prefix: Some(prefix),
            entries: Vec::new(),
        }
    }

    /// Records `step`'s outcome.
    pub fn push(&mut self, step: &'static str, outcome: Outcome) {
        if let Some(line) = self.line(&outcome) {
            eprintln!("{line}");
        }
        self.entries.push((step, outcome));
    }

    /// What [`Log::push`] prints for `outcome`: `<prefix><reason>` for `Failed` in a printing
    /// log, else nothing.
    fn line(&self, outcome: &Outcome) -> Option<String> {
        match (self.prefix, outcome) {
            (Some(prefix), Outcome::Failed(why)) => Some(format!("{prefix}{why}")),
            _ => None,
        }
    }

    /// Records a step that stops the tool unless it is `Ok`; returns whether it was.
    pub fn proceed(&mut self, step: &'static str, outcome: Outcome) -> bool {
        let ok = outcome == Outcome::Ok;
        self.push(step, outcome);
        ok
    }

    /// Records a step that yields data a later step needs: `Ok(v)` is `Outcome::Ok` and gives
    /// `v`; `Err(o)` records `o` and gives `None` (the driver stops).
    pub fn take<T>(&mut self, step: &'static str, r: Result<T, Outcome>) -> Option<T> {
        match r {
            Ok(v) => {
                self.push(step, Outcome::Ok);
                Some(v)
            }
            Err(o) => {
                self.push(step, o);
                None
            }
        }
    }
}

#[cfg(test)]
impl Log {
    pub fn entries(&self) -> &[(&'static str, Outcome)] {
        &self.entries
    }

    /// The step names, in order.
    pub fn names(&self) -> Vec<&'static str> {
        self.entries.iter().map(|(n, _)| *n).collect()
    }
}

/// The exit code of a log: `Failed` 1, else `Infra` 75, else `Finding` 4, else 0
/// (specs/tooling/nightly.md item 10).
pub fn exit_code(log: &Log) -> u8 {
    log.entries
        .iter()
        .map(|(_, o)| o)
        .max_by_key(|o| o.rank())
        .map_or(0, Outcome::code)
}

/// The `JUDGE-INFRA-FAILURE` line (specs/tooling/nightly.md item 10): exactly when the log's
/// exit code is 75, from its first `Infra` reason.
pub fn infra_line(log: &Log) -> Option<String> {
    if exit_code(log) != 75 {
        return None;
    }
    log.entries.iter().find_map(|(_, o)| match o {
        Outcome::Infra(why) => Some(format!("JUDGE-INFRA-FAILURE: {why}")),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(outcomes: &[Outcome]) -> Log {
        let mut l = Log::default();
        for o in outcomes {
            l.push("s", o.clone());
        }
        l
    }

    fn kinds() -> [Outcome; 4] {
        [
            Outcome::Ok,
            Outcome::Finding,
            Outcome::Infra("i".into()),
            Outcome::Failed("f".into()),
        ]
    }

    /// Item 10's precedence over every ordered pair of kinds (and each kind alone).
    #[test]
    fn exit_code_over_every_pair_of_kinds() {
        let code = |o: &Outcome| match o {
            Outcome::Ok => 0u8,
            Outcome::Finding => 4,
            Outcome::Infra(_) => 75,
            Outcome::Failed(_) => 1,
        };
        assert_eq!(exit_code(&Log::default()), 0, "an empty log");
        for a in kinds() {
            assert_eq!(exit_code(&log(std::slice::from_ref(&a))), code(&a), "{a:?}");
        }
        // Rows: the first entry; columns: the second. Order must not matter.
        let want: [[u8; 4]; 4] = [[0, 4, 75, 1], [4, 4, 75, 1], [75, 75, 75, 1], [1, 1, 1, 1]];
        for (i, a) in kinds().into_iter().enumerate() {
            for (j, b) in kinds().into_iter().enumerate() {
                assert_eq!(
                    exit_code(&log(&[a.clone(), b.clone()])),
                    want[i][j],
                    "{a:?} then {b:?}"
                );
            }
        }
        // A later Ok doesn't hide an earlier failure.
        assert_eq!(
            exit_code(&log(&[Outcome::Finding, Outcome::Ok, Outcome::Ok])),
            4
        );
    }

    #[test]
    fn infra_line_only_when_the_code_is_75() {
        let infra = || Outcome::Infra("3 regress files could not be fetched".into());
        assert_eq!(
            infra_line(&log(&[Outcome::Ok, infra(), Outcome::Finding])).as_deref(),
            Some("JUDGE-INFRA-FAILURE: 3 regress files could not be fetched")
        );
        // The near miss: an Infra entry in a log whose code is 1.
        assert_eq!(
            infra_line(&log(&[infra(), Outcome::Failed("x".into())])),
            None
        );
        assert_eq!(infra_line(&log(&[Outcome::Finding])), None);
        assert_eq!(infra_line(&Log::default()), None);
        // Two Infra entries: one line, the first reason.
        assert_eq!(
            infra_line(&log(&[infra(), Outcome::Infra("later".into())])).as_deref(),
            Some("JUDGE-INFRA-FAILURE: 3 regress files could not be fetched")
        );
    }

    /// The only stderr path for tool failures (`main` prints none for the nightly).
    #[test]
    fn a_printing_log_prints_failed_reasons_only() {
        let l = Log::printing("xtask: nightly: ");
        assert_eq!(
            l.line(&Outcome::Failed("cannot write x".into())).as_deref(),
            Some("xtask: nightly: cannot write x")
        );
        for o in [Outcome::Ok, Outcome::Finding, Outcome::Infra("i".into())] {
            assert_eq!(l.line(&o), None, "{o:?}");
        }
        assert_eq!(Log::default().line(&Outcome::Failed("x".into())), None);
    }

    #[test]
    fn take_and_proceed_record_the_outcome() {
        let mut l = Log::default();
        assert_eq!(l.take("a", Ok::<u8, Outcome>(7)), Some(7));
        assert_eq!(l.take::<u8>("b", Err(Outcome::Failed("x".into()))), None);
        assert!(l.proceed("c", Outcome::Ok));
        assert!(!l.proceed("d", Outcome::Finding));
        assert!(!l.proceed("e", Outcome::Infra("y".into())));
        assert_eq!(l.names(), ["a", "b", "c", "d", "e"]);
        assert_eq!(l.entries()[1].1, Outcome::Failed("x".into()));
        assert_eq!(l.entries()[0].1, Outcome::Ok);
    }
}
