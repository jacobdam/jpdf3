//! Engines, operations and timed runs of external programs (spec items 4–7).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How often a running child is polled for exit.
const POLL: Duration = Duration::from_micros(200);

/// A timed operation (spec item 4). Always page 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Info,
    Render72,
    Render150,
    Text,
}

impl Op {
    pub const ALL: [Op; 4] = [Op::Info, Op::Render72, Op::Render150, Op::Text];

    pub fn id(self) -> &'static str {
        match self {
            Op::Info => "info",
            Op::Render72 => "render72",
            Op::Render150 => "render150",
            Op::Text => "text",
        }
    }

    /// Per-run time limit, ops/judge/CONTRACT.md §1.
    pub fn time_limit(self) -> Duration {
        match self {
            Op::Info => Duration::from_secs(20),
            Op::Render72 | Op::Render150 => Duration::from_secs(30),
            Op::Text => Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Jpdf,
    Mutool,
    Pdfium,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Jpdf => "jpdf",
            Kind::Mutool => "mutool",
            Kind::Pdfium => "pdfium",
        }
    }
}

/// A program to time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Engine {
    pub kind: Kind,
    pub bin: PathBuf,
    /// First line of the version output, if the engine prints one.
    pub version: Option<String>,
}

impl Engine {
    /// The argv (without the program) for `op` on `file`, or `None` if this engine has no
    /// equivalent command. Output files go into `work`. For PDFium, `file` must already be a
    /// copy inside `work`, because `pdfium_test` writes next to its input.
    pub fn argv(&self, op: Op, file: &Path, work: &Path) -> Option<Vec<OsString>> {
        let f = file.as_os_str().to_os_string();
        let out = |ext: &str| {
            work.join(format!("out-{}.{ext}", self.kind.id()))
                .into_os_string()
        };
        let s = |x: &str| OsString::from(x);
        let v = match (self.kind, op) {
            (Kind::Jpdf, Op::Info) => vec![s("info"), f],
            (Kind::Jpdf, Op::Render72 | Op::Render150) => vec![
                s("render"),
                f,
                s("--page"),
                s("1"),
                s("--dpi"),
                s(if op == Op::Render72 { "72" } else { "150" }),
                s("--out"),
                out("png"),
            ],
            (Kind::Jpdf, Op::Text) => vec![s("text"), f, s("--page"), s("1")],
            (Kind::Mutool, Op::Info) => vec![s("show"), f, s("pages")],
            (Kind::Mutool, Op::Render72 | Op::Render150) => vec![
                s("draw"),
                s("-q"),
                s("-r"),
                s(if op == Op::Render72 { "72" } else { "150" }),
                s("-o"),
                out("png"),
                f,
                s("1"),
            ],
            (Kind::Mutool, Op::Text) => {
                vec![
                    s("draw"),
                    s("-q"),
                    s("-F"),
                    s("txt"),
                    s("-o"),
                    out("txt"),
                    f,
                    s("1"),
                ]
            }
            (Kind::Pdfium, Op::Info) => return None,
            (Kind::Pdfium, Op::Render72 | Op::Render150) => vec![
                s("--png"),
                // 150 / 72, to the 6 decimals pdfium_test parses.
                s(if op == Op::Render72 {
                    "--scale=1"
                } else {
                    "--scale=2.083333"
                }),
                s("--pages=0"),
                f,
            ],
            (Kind::Pdfium, Op::Text) => vec![s("--txt"), s("--pages=0"), f],
        };
        Some(v)
    }
}

/// How a run ended (spec item 6; ops/judge/CONTRACT.md §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Error,
    NotImplemented,
    Timeout,
    Crash,
}

impl Status {
    pub const ALL: [Status; 5] = [
        Status::Ok,
        Status::Error,
        Status::NotImplemented,
        Status::Timeout,
        Status::Crash,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Error => "error",
            Status::NotImplemented => "not_implemented",
            Status::Timeout => "timeout",
            Status::Crash => "crash",
        }
    }
}

/// Exit code (or `None` for a signal) → status.
pub fn classify(code: Option<i32>) -> Status {
    match code {
        Some(0) => Status::Ok,
        Some(2) => Status::Error,
        Some(3) => Status::NotImplemented,
        _ => Status::Crash,
    }
}

fn classify_exit(st: ExitStatus) -> Status {
    classify(st.code())
}

/// Runs `bin args` once with stdio on the null device; returns the status and wall time
/// from spawn to exit. The child is killed at `limit`.
pub fn run_once(bin: &Path, args: &[OsString], limit: Duration) -> (Status, Duration) {
    let start = Instant::now();
    let mut child = match Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return (Status::Crash, start.elapsed()),
    };
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return (classify_exit(st), start.elapsed()),
            Ok(None) if start.elapsed() >= limit => {
                // Errors here mean the child already exited; it is reaped either way.
                let _ = child.kill();
                let _ = child.wait();
                return (Status::Timeout, start.elapsed());
            }
            Ok(None) => thread::sleep(POLL),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return (Status::Crash, start.elapsed());
            }
        }
    }
}

/// One measurement: its status and, only when every run was `ok`, the median time in µs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    pub status: Status,
    pub micros: Option<f64>,
}

/// `warmup` untimed runs, then `runs` timed runs (spec item 5). The first non-`ok` run
/// ends the measurement with that status and no time.
pub fn measure(
    bin: &Path,
    args: &[OsString],
    limit: Duration,
    runs: usize,
    warmup: usize,
) -> Measurement {
    measure_with(runs, warmup, || run_once(bin, args, limit))
}

/// [`measure`] with the runner injected (for tests).
pub fn measure_with(
    runs: usize,
    warmup: usize,
    mut run: impl FnMut() -> (Status, Duration),
) -> Measurement {
    let mut times = Vec::with_capacity(runs);
    for i in 0..warmup.saturating_add(runs) {
        let (status, took) = run();
        if status != Status::Ok {
            return Measurement {
                status,
                micros: None,
            };
        }
        if i >= warmup {
            times.push(u64::try_from(took.as_micros()).unwrap_or(u64::MAX));
        }
    }
    Measurement {
        status: Status::Ok,
        micros: crate::stats::median(&mut times),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes() {
        assert_eq!(classify(Some(0)), Status::Ok);
        assert_eq!(classify(Some(2)), Status::Error);
        assert_eq!(classify(Some(3)), Status::NotImplemented);
        assert_eq!(classify(Some(1)), Status::Crash);
        assert_eq!(classify(Some(101)), Status::Crash);
        assert_eq!(classify(Some(-1)), Status::Crash);
        assert_eq!(classify(None), Status::Crash);
    }

    #[test]
    fn measurement_uses_timed_runs_only() {
        let mut n = 0u64;
        let m = measure_with(3, 2, || {
            n += 1;
            // Warmups take 1000 ms; timed runs 3, 1, 2 ms.
            let ms = [1000, 1000, 3, 1, 2][usize::try_from(n - 1).unwrap()];
            (Status::Ok, Duration::from_millis(ms))
        });
        assert_eq!(n, 5);
        assert_eq!(m.status, Status::Ok);
        assert_eq!(m.micros, Some(2000.0));
    }

    #[test]
    fn failure_has_no_time_and_stops() {
        for bad in [
            Status::Error,
            Status::NotImplemented,
            Status::Timeout,
            Status::Crash,
        ] {
            let mut n = 0;
            let m = measure_with(5, 1, || {
                n += 1;
                if n == 3 {
                    (bad, Duration::from_millis(1))
                } else {
                    (Status::Ok, Duration::from_millis(1))
                }
            });
            assert_eq!(n, 3);
            assert_eq!(
                m,
                Measurement {
                    status: bad,
                    micros: None
                }
            );
        }
    }

    #[test]
    fn argv_tables() {
        let e = |kind| Engine {
            kind,
            bin: PathBuf::from("x"),
            version: None,
        };
        let f = Path::new("/c/a.pdf");
        let w = Path::new("/w");
        let strs = |v: Option<Vec<OsString>>| {
            v.map(|v| {
                v.into_iter()
                    .map(|s| s.into_string().unwrap())
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(
            strs(e(Kind::Jpdf).argv(Op::Info, f, w)).unwrap(),
            ["info", "/c/a.pdf"]
        );
        assert_eq!(
            strs(e(Kind::Jpdf).argv(Op::Render150, f, w)).unwrap(),
            [
                "render",
                "/c/a.pdf",
                "--page",
                "1",
                "--dpi",
                "150",
                "--out",
                "/w/out-jpdf.png"
            ]
        );
        assert_eq!(
            strs(e(Kind::Jpdf).argv(Op::Text, f, w)).unwrap(),
            ["text", "/c/a.pdf", "--page", "1"]
        );
        assert_eq!(
            strs(e(Kind::Mutool).argv(Op::Info, f, w)).unwrap(),
            ["show", "/c/a.pdf", "pages"]
        );
        assert_eq!(
            strs(e(Kind::Mutool).argv(Op::Render72, f, w)).unwrap(),
            [
                "draw",
                "-q",
                "-r",
                "72",
                "-o",
                "/w/out-mutool.png",
                "/c/a.pdf",
                "1"
            ]
        );
        assert_eq!(
            strs(e(Kind::Mutool).argv(Op::Text, f, w)).unwrap(),
            [
                "draw",
                "-q",
                "-F",
                "txt",
                "-o",
                "/w/out-mutool.txt",
                "/c/a.pdf",
                "1"
            ]
        );
        assert_eq!(strs(e(Kind::Pdfium).argv(Op::Info, f, w)), None);
        assert_eq!(
            strs(e(Kind::Pdfium).argv(Op::Render72, f, w)).unwrap(),
            ["--png", "--scale=1", "--pages=0", "/c/a.pdf"]
        );
        assert_eq!(
            strs(e(Kind::Pdfium).argv(Op::Render150, f, w)).unwrap(),
            ["--png", "--scale=2.083333", "--pages=0", "/c/a.pdf"]
        );
        assert_eq!(
            strs(e(Kind::Pdfium).argv(Op::Text, f, w)).unwrap(),
            ["--txt", "--pages=0", "/c/a.pdf"]
        );
    }

    #[test]
    fn time_limits_match_contract() {
        assert_eq!(Op::Info.time_limit(), Duration::from_secs(20));
        assert_eq!(Op::Render72.time_limit(), Duration::from_secs(30));
        assert_eq!(Op::Render150.time_limit(), Duration::from_secs(30));
        assert_eq!(Op::Text.time_limit(), Duration::from_secs(60));
    }
}
