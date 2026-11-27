//! The nightly's driver over a recording fake (specs/tooling/nightly.md items 10, 11, 12 and 15;
//! ADR 0013 "Verification"; #350, #352).

use super::super::*;

/// Every step in the spec's order (item 15), as the log names them.
const FULL: [&str; 17] = [
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
    "stale-summary",
    "bench",
    "fuzz",
    "summary",
    "issues",
];

/// A recording fake of the nightly's steps: each step in `bad` returns its outcome there,
/// every other step `Ok`; `calls` records each call with the data the step got.
struct Fake {
    bad: Vec<(&'static str, Step)>,
    jobs: Vec<Job>,
    /// What the `memory` step returns.
    mem: bool,
    /// The cases the `limits` step returns (whatever its outcome).
    cases: Vec<CaseResult>,
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
            cases: Vec::new(),
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

    fn limits(&mut self, cli: &Path, mem: bool) -> (Step, Vec<CaseResult>) {
        let o = self.outcome(format!("limits {} {mem}", cli.display()));
        (o, self.cases.clone())
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

    fn stale_summary(&mut self) -> Step {
        self.outcome("stale-summary".into())
    }

    fn bench(&mut self, jobs: &[Job], mem: bool) -> (Step, extra::BenchResult) {
        let o = self.outcome(format!("bench {} {mem}", job_blobs(jobs)));
        // A result no default would give, so a driver that hands `summary` another one fails.
        let b = extra::BenchResult {
            status: "timeout",
            files: jobs.iter().filter(|j| j.path.is_some()).count(),
            exit: None,
            signal: Some(9),
            ms: Some(60_000),
            report: false,
            error: None,
        };
        (o, b)
    }

    fn fuzz(&mut self) -> (Step, extra::FuzzResult) {
        let o = self.outcome("fuzz".into());
        let f = extra::FuzzResult {
            status: "crash",
            seconds: 9,
            targets: Vec::new(),
            error: None,
        };
        (o, f)
    }

    fn summary(&mut self, bench: &extra::BenchResult, fuzz: &extra::FuzzResult) -> Step {
        self.outcome(format!(
            "summary {}/{} {}/{}",
            bench.status, bench.files, fuzz.status, fuzz.seconds
        ))
    }

    fn issues(
        &mut self,
        limits: &[CaseResult],
        previous: &Previous,
        files: Option<&[FileResult]>,
    ) -> Step {
        let p = match previous {
            Previous::Nothing(n) => n.as_str(),
            Previous::Loaded(p, _) => p.as_str(),
        };
        let cases: Vec<&str> = limits.iter().map(|c| c.id).collect();
        let files = files.map_or("none".into(), |f| {
            blobs(f.iter().map(|f| (f.blob.as_str(), f.runs.is_some())))
        });
        self.outcome(format!("issues [{}] {p} {files}", cases.join(",")))
    }
}

fn case(id: &'static str, status: &'static str) -> CaseResult {
    CaseResult {
        id,
        bytes: 1,
        status,
        exit: None,
        signal: None,
        ms: None,
        stderr: None,
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
            "stale-summary",
            "bench a,b- true",
            "fuzz",
            "summary timeout/1 crash/9",
            "issues [] fake-previous a,b-",
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
    assert_eq!(f.calls[13], "bench a,b- false");
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
    let limits = FULL.iter().position(|s| *s == "limits").unwrap();
    for step in stops {
        let at = FULL.iter().position(|s| *s == step).unwrap();
        let mut f = Fake::new(two()).with(step, failed());
        let log = drive(&mut f, Log::default());
        // Item 16: after a stop, `issues` still runs once the limits part has run.
        let want = if at > limits {
            [&FULL[..=at], &["issues"]].concat()
        } else {
            FULL[..=at].to_vec()
        };
        assert_eq!(log.names(), want, "{step}");
        assert_eq!(log.entries()[at].1, failed(), "{step}");
        assert_eq!(
            names(&f.calls),
            want,
            "{step}: nothing but issues called after it"
        );
        assert_eq!(steps::exit_code(&log), 1, "{step}");
    }
    // The stopping steps stop on any outcome but Ok (one that isn't a tool failure).
    let mut f = Fake::new(two()).with("watchdog", infra());
    assert_eq!(drive(&mut f, Log::default()).names(), FULL[..=4]);
    // Item 11: the regress part runs after a limits tool failure or a failed case; item 12:
    // the bench and the fuzz pass run after a failed diff; a run that couldn't start, or a
    // fetch that failed, still writes the results; items 12–14: a failed removal, bench, fuzz
    // pass or summary write stops nothing.
    let goes_on = [
        ("limits", failed(), 1),
        ("limits", Step::Finding, 4),
        ("fetch", failed(), 1),
        ("fetch", infra(), 75),
        ("runs", failed(), 1),
        ("diff", failed(), 1),
        ("stale-summary", failed(), 1),
        ("bench", failed(), 1),
        ("fuzz", failed(), 1),
        ("summary", failed(), 1),
        ("issues", failed(), 1),
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
    assert_eq!(log.names(), [to_fetch, &["files", "issues"]].concat());
    assert_eq!(
        log.entries()[8].1,
        Step::Failed("no regress file to run (2 rows missing)".into())
    );
    assert_eq!(
        names(&f.calls),
        [to_fetch, &["issues"]].concat(),
        "no workdir, runs, bench, fuzz pass or summary"
    );
    assert_eq!(
        f.calls.last().unwrap(),
        "issues [] fake-previous none",
        "no regress files"
    );
    assert_eq!(steps::exit_code(&log), 1);
    // No rows at all.
    let log = drive(&mut Fake::new(Vec::new()), Log::default());
    assert_eq!(
        log.entries()[8].1,
        Step::Failed("no regress file to run (0 rows missing)".into())
    );
    // The fetch's integrity failure: both entries, exit 1.
    let log = drive(
        &mut Fake::new(none()).with("fetch", failed()),
        Log::default(),
    );
    assert_eq!(log.names(), [to_fetch, &["files", "issues"]].concat());
    assert_eq!(
        log.entries()[8].1,
        Step::Failed("no regress file to run (2 rows missing)".into())
    );
    assert_eq!(steps::exit_code(&log), 1);
    // No network: the fetch's 75 alone, with the marker.
    let mut f = Fake::new(none()).with("fetch", infra());
    let log = drive(&mut f, Log::default());
    let to_issues = [to_fetch, &["issues"]].concat();
    assert_eq!(log.names(), to_issues);
    assert_eq!(names(&f.calls), to_issues);
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

/// Item 16: `issues` gets the limits step's cases (a failed case, a tool failure's empty list),
/// the previous night and this night's files, or `None` when the night stopped before the runs.
#[test]
fn the_issues_step_gets_what_the_night_found() {
    let mut f = Fake::new(two()).with("limits", Step::Finding);
    f.cases = vec![case("deep-nesting", "crash"), case("big-xref", "pass")];
    let log = drive(&mut f, Log::default());
    assert_eq!(log.names(), FULL);
    assert_eq!(
        f.calls.last().unwrap(),
        "issues [deep-nesting,big-xref] fake-previous a,b-"
    );
    assert_eq!(steps::exit_code(&log), 4);
    // A stop after the runs (a failed `regress.json` write) still hands over the files.
    let mut f = Fake::new(two()).with("results", failed());
    f.cases = vec![case("deep-nesting", "crash")];
    drive(&mut f, Log::default());
    assert_eq!(
        f.calls.last().unwrap(),
        "issues [deep-nesting] fake-previous a,b-"
    );
    // A stop before the runs: no files.
    let mut f = Fake::new(two()).with("corpus", failed());
    f.cases = vec![case("deep-nesting", "crash")];
    drive(&mut f, Log::default());
    assert_eq!(
        f.calls.last().unwrap(),
        "issues [deep-nesting] fake-previous none"
    );
    // A stop before the limits part: no `issues` call at all.
    let mut f = Fake::new(two()).with("watchdog", failed());
    drive(&mut f, Log::default());
    assert!(!names(&f.calls).contains(&"issues"));
    // A failed `issues` step is a tool failure, after the night's 4.
    let mut f = Fake::new(two())
        .with("limits", Step::Finding)
        .with("issues", failed());
    let log = drive(&mut f, Log::default());
    assert_eq!(log.entries().last().unwrap(), &("issues", failed()));
    assert_eq!(steps::exit_code(&log), 1);
}

/// #352: a stale `nightly.json` that can't be removed is its own `Failed` entry when this
/// night's summary is written, and a failed write is its own entry when the removal worked
/// (item 14; their places, before the bench and after the fuzz pass, are pinned by the full
/// order above).
#[test]
fn a_failed_removal_and_a_failed_write_are_separate_entries() {
    let at = |step: &str| FULL.iter().position(|s| *s == step).unwrap();
    let log = drive(
        &mut Fake::new(two()).with("stale-summary", failed()),
        Log::default(),
    );
    assert_eq!(log.names(), FULL);
    assert_eq!(log.entries()[at("stale-summary")].1, failed());
    assert_eq!(log.entries()[at("summary")].1, Step::Ok);
    assert_eq!(steps::exit_code(&log), 1);
    let log = drive(
        &mut Fake::new(two()).with("summary", failed()),
        Log::default(),
    );
    assert_eq!(log.entries()[at("stale-summary")].1, Step::Ok);
    assert_eq!(log.entries()[at("summary")].1, failed());
    assert_eq!(steps::exit_code(&log), 1);
    // A bench tool failure alone is a 1 too, and the summary still gets the bench's result.
    let mut f = Fake::new(two()).with("bench", failed());
    let log = drive(&mut f, Log::default());
    assert_eq!(steps::exit_code(&log), 1);
    assert!(f.calls.contains(&"summary timeout/1 crash/9".to_string()));
}
