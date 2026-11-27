//! The nightly's driver over a recording fake (specs/tooling/nightly.md items 10, 11, 12 and 15;
//! ADR 0013 "Verification"; #350).

use super::super::*;

/// Every step in the spec's order (item 15), as the log names them.
const FULL: [&str; 13] = [
    "previous",
    "build",
    "executable",
    "memory",
    "watchdog",
    "limits",
    "corpus",
    "fetch",
    "workdir",
    "runs",
    "results",
    "diff",
    "extra",
];

/// A recording fake of the nightly's steps: each step in `bad` returns its outcome there,
/// every other step `Ok`; `calls` records each call with the data the step got.
struct Fake {
    bad: Vec<(&'static str, Step)>,
    jobs: Vec<Job>,
    /// What the `memory` step returns.
    mem: bool,
    calls: Vec<String>,
}

fn job(blob: &str, verified: bool) -> Job {
    Job {
        blob: blob.into(),
        url: format!("https://example.invalid/{blob}.pdf"),
        path: verified.then(|| PathBuf::from(format!("/fake/cache/{blob}.pdf"))),
    }
}

/// One verified row (`a`) and one missing (`b`).
fn two() -> Vec<Job> {
    vec![job("a", true), job("b", false)]
}

/// The blobs, a missing one marked `-`.
fn blobs<'a>(names: impl Iterator<Item = (&'a str, bool)>) -> String {
    names
        .map(|(b, ok)| if ok { b.to_string() } else { format!("{b}-") })
        .collect::<Vec<_>>()
        .join(",")
}

fn job_blobs(jobs: &[Job]) -> String {
    blobs(jobs.iter().map(|j| (j.blob.as_str(), j.path.is_some())))
}

fn ran_blobs(ran: &Ran) -> String {
    blobs(
        ran.files
            .iter()
            .map(|f| (f.blob.as_str(), f.runs.is_some())),
    )
}

fn failed() -> Step {
    Step::Failed("boom".into())
}

fn infra() -> Step {
    Step::Infra("2 regress files could not be fetched".into())
}

impl Fake {
    fn new(jobs: Vec<Job>) -> Fake {
        Fake {
            bad: Vec::new(),
            jobs,
            mem: true,
            calls: Vec::new(),
        }
    }

    fn with(mut self, step: &'static str, o: Step) -> Fake {
        self.bad.push((step, o));
        self
    }

    fn outcome(&mut self, call: String) -> Step {
        let step = call.split(' ').next().unwrap_or_default().to_string();
        self.calls.push(call);
        self.bad
            .iter()
            .find(|(s, _)| *s == step)
            .map_or(Step::Ok, |(_, o)| o.clone())
    }

    fn data<T>(&mut self, call: &str, v: T) -> Result<T, Step> {
        match self.outcome(call.to_string()) {
            Step::Ok => Ok(v),
            o => Err(o),
        }
    }
}

impl Steps for Fake {
    fn previous(&mut self) -> Result<Previous, Step> {
        self.data("previous", Previous::Nothing("fake-previous".into()))
    }

    fn build(&mut self) -> Result<PathBuf, Step> {
        self.data("build", PathBuf::from("/fake/jpdf-cli"))
    }

    fn executable(&mut self, cli: &Path) -> Step {
        self.outcome(format!("executable {}", cli.display()))
    }

    fn memory(&mut self) -> Result<bool, Step> {
        let mem = self.mem;
        self.data("memory", mem)
    }

    fn watchdog(&mut self) -> Step {
        self.outcome("watchdog".into())
    }

    fn limits(&mut self, cli: &Path, mem: bool) -> Step {
        self.outcome(format!("limits {} {mem}", cli.display()))
    }

    fn corpus(&mut self) -> Result<Corpus, Step> {
        self.data(
            "corpus",
            Corpus {
                entries: Vec::new(),
                dir: PathBuf::from("/fake/cache"),
            },
        )
    }

    fn fetch(&mut self, corpus: &Corpus) -> (Step, Vec<Job>) {
        let o = self.outcome(format!("fetch {}", corpus.dir.display()));
        (o, self.jobs.clone())
    }

    fn workdir(&mut self) -> Result<PathBuf, Step> {
        self.data("workdir", PathBuf::from("/fake/work/render.png"))
    }

    fn runs(&mut self, jobs: &[Job], png: &Path, cli: &Path, mem: bool) -> (Step, Ran) {
        let o = self.outcome(format!(
            "runs {} {} {} {mem}",
            job_blobs(jobs),
            png.display(),
            cli.display()
        ));
        let files = jobs
            .iter()
            .map(|j| FileResult {
                blob: j.blob.clone(),
                url: j.url.clone(),
                runs: j.path.as_ref().map(|_| Vec::new()),
            })
            .collect();
        // `complete: false`, so a driver that drops it is seen.
        (
            o,
            Ran {
                files,
                complete: false,
            },
        )
    }

    fn results(&mut self, ran: &Ran, mem: bool) -> Step {
        self.outcome(format!(
            "results {} complete={} {mem}",
            ran_blobs(ran),
            ran.complete
        ))
    }

    fn diff(&mut self, previous: &Previous, ran: &Ran) -> Step {
        let p = match previous {
            Previous::Nothing(n) => n.as_str(),
            Previous::Loaded(p, _) => p.as_str(),
        };
        self.outcome(format!("diff {p} {}", ran_blobs(ran)))
    }

    fn extra(&mut self, jobs: &[Job], mem: bool) -> Step {
        self.outcome(format!("extra {} {mem}", job_blobs(jobs)))
    }
}

fn names(calls: &[String]) -> Vec<&str> {
    calls
        .iter()
        .map(|c| c.split(' ').next().unwrap_or_default())
        .collect()
}

/// All steps `Ok`: the spec's full order, each step called once with the data the earlier
/// steps produced (the limits part before the fetch, #350).
#[test]
fn every_step_runs_once_in_the_spec_order_with_its_data() {
    let mut f = Fake::new(two());
    let log = drive(&mut f, Log::default());
    assert_eq!(log.names(), FULL);
    assert!(log.entries().iter().all(|(_, o)| *o == Step::Ok));
    assert_eq!(steps::exit_code(&log), 0);
    assert_eq!(
        f.calls,
        [
            "previous",
            "build",
            "executable /fake/jpdf-cli",
            "memory",
            "watchdog",
            "limits /fake/jpdf-cli true",
            "corpus",
            "fetch /fake/cache",
            "workdir",
            "runs a,b- /fake/work/render.png /fake/jpdf-cli true",
            "results a,b- complete=false true",
            "diff fake-previous a,b-",
            "extra a,b- true",
        ]
    );
}

/// The memory step's answer reaches every step that runs `jpdf-cli` (a probe that fails on
/// macOS must not put the runs through the wrapper).
#[test]
fn the_memory_answer_reaches_every_step_that_runs_the_cli() {
    let mut f = Fake::new(two());
    f.mem = false;
    let log = drive(&mut f, Log::default());
    assert_eq!(log.names(), FULL);
    assert_eq!(f.calls[5], "limits /fake/jpdf-cli false");
    assert_eq!(
        f.calls[9],
        "runs a,b- /fake/work/render.png /fake/jpdf-cli false"
    );
    assert_eq!(f.calls[10], "results a,b- complete=false false");
    assert_eq!(f.calls[12], "extra a,b- false");
}

/// One run per failing step (ADR 0013 "Verification"): a stopping step ends the log at
/// itself; after the others the night goes on to the end.
#[test]
fn a_failing_step_stops_exactly_what_the_spec_says() {
    let stops = [
        "previous",
        "build",
        "executable",
        "memory",
        "watchdog",
        "corpus",
        "workdir",
        "results",
    ];
    for step in stops {
        let at = FULL.iter().position(|s| *s == step).unwrap();
        let mut f = Fake::new(two()).with(step, failed());
        let log = drive(&mut f, Log::default());
        assert_eq!(log.names(), FULL[..=at], "{step}");
        assert_eq!(log.entries()[at].1, failed(), "{step}");
        assert_eq!(
            names(&f.calls),
            FULL[..=at],
            "{step}: nothing called after it"
        );
        assert_eq!(steps::exit_code(&log), 1, "{step}");
    }
    // The stopping steps stop on any outcome but Ok (one that isn't a tool failure).
    let mut f = Fake::new(two()).with("watchdog", infra());
    assert_eq!(drive(&mut f, Log::default()).names(), FULL[..=4]);
    // Item 11: the regress part runs after a limits tool failure or a failed case; item 12:
    // the bench and the fuzz pass run after a failed diff; a run that couldn't start, or a
    // fetch that failed, still writes the results.
    let goes_on = [
        ("limits", failed(), 1),
        ("limits", Step::Finding, 4),
        ("fetch", failed(), 1),
        ("fetch", infra(), 75),
        ("runs", failed(), 1),
        ("diff", failed(), 1),
        ("extra", failed(), 1),
    ];
    for (step, o, code) in goes_on.clone() {
        let mut f = Fake::new(two()).with(step, o.clone());
        let log = drive(&mut f, Log::default());
        assert_eq!(log.names(), FULL, "{step} {o:?}");
        assert_eq!(names(&f.calls), FULL, "{step} {o:?}");
        let at = FULL.iter().position(|s| *s == step).unwrap();
        assert_eq!(log.entries()[at].1, o, "{step}");
        assert_eq!(steps::exit_code(&log), code, "{step} {o:?}");
    }
    // Every step is in one of the two lists.
    for step in FULL {
        assert!(
            stops.contains(&step) || goes_on.iter().any(|(s, _, _)| *s == step),
            "{step} untested"
        );
    }
}

/// Item 10's no-file stop: nothing after it runs (#350), and it is a tool failure of its own
/// unless the fetch was an infrastructure failure.
#[test]
fn with_no_verified_file_the_night_stops_after_the_fetch() {
    let none = || vec![job("a", false), job("b", false)];
    let to_fetch = &FULL[..=7];
    let mut f = Fake::new(none());
    let log = drive(&mut f, Log::default());
    assert_eq!(log.names(), [to_fetch, &["files"]].concat());
    assert_eq!(
        log.entries()[8].1,
        Step::Failed("no regress file to run (2 rows missing)".into())
    );
    assert_eq!(names(&f.calls), to_fetch, "no workdir, runs or extra");
    assert_eq!(steps::exit_code(&log), 1);
    // No rows at all.
    let log = drive(&mut Fake::new(Vec::new()), Log::default());
    assert_eq!(
        log.entries().last().unwrap().1,
        Step::Failed("no regress file to run (0 rows missing)".into())
    );
    // The fetch's integrity failure: both entries, exit 1.
    let log = drive(
        &mut Fake::new(none()).with("fetch", failed()),
        Log::default(),
    );
    assert_eq!(log.names(), [to_fetch, &["files"]].concat());
    assert_eq!(
        log.entries()[8].1,
        Step::Failed("no regress file to run (2 rows missing)".into())
    );
    assert_eq!(steps::exit_code(&log), 1);
    // No network: the fetch's 75 alone, with the marker.
    let mut f = Fake::new(none()).with("fetch", infra());
    let log = drive(&mut f, Log::default());
    assert_eq!(log.names(), to_fetch);
    assert_eq!(names(&f.calls), to_fetch);
    assert_eq!(steps::exit_code(&log), 75);
    assert_eq!(
        steps::infra_line(&log).as_deref(),
        Some("JUDGE-INFRA-FAILURE: 2 regress files could not be fetched")
    );
    // The deliberate change to item 10 (ADR 0013 task 1): no network and a limits tool
    // failure is exit 1 with no marker (before, the marker was printed at the stop).
    let log = drive(
        &mut Fake::new(none())
            .with("limits", failed())
            .with("fetch", infra()),
        Log::default(),
    );
    assert_eq!(steps::exit_code(&log), 1);
    assert_eq!(steps::infra_line(&log), None);
    // A failed limits case on that night stays 75 (rerun).
    let log = drive(
        &mut Fake::new(none())
            .with("limits", Step::Finding)
            .with("fetch", infra()),
        Log::default(),
    );
    assert_eq!(steps::exit_code(&log), 75);
    // The near miss: one verified file among missing ones runs the whole night.
    let mut f = Fake::new(vec![job("a", false), job("b", true)]);
    assert_eq!(drive(&mut f, Log::default()).names(), FULL);
    assert_eq!(
        f.calls[9],
        "runs a-,b /fake/work/render.png /fake/jpdf-cli true"
    );
}

/// Item 10 with the limits part (item 11): a tool failure in either part is 1, then the
/// regress part's 75, then a failed case's 4; the regress part runs in every case. (This
/// replaces `nightly_exit_codes_with_the_limits_cases`, which tested the removed
/// `with_limits` fold, case for case.)
#[test]
fn exit_codes_with_the_limits_cases() {
    let regress = [
        (None, "0"),
        (Some(("fetch", infra())), "75"),
        (Some(("runs", failed())), "1"),
    ];
    let limits = [Step::Ok, Step::Finding, failed()];
    let want: [[u8; 3]; 3] = [[0, 75, 1], [4, 75, 1], [1, 1, 1]];
    for (i, l) in limits.iter().enumerate() {
        for (j, (r, what)) in regress.iter().enumerate() {
            let mut f = Fake::new(two()).with("limits", l.clone());
            if let Some((step, o)) = r {
                f = f.with(step, o.clone());
            }
            let log = drive(&mut f, Log::default());
            assert_eq!(log.names(), FULL, "limits {l:?}, regress {what}");
            assert_eq!(
                steps::exit_code(&log),
                want[i][j],
                "limits {l:?}, regress {what}"
            );
        }
    }
}
