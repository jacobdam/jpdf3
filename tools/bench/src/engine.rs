//! Engines, operations and timed runs of external programs (spec items 4–7).

use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
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
    match probe(bin, args, limit, 0) {
        Some((status, took, _)) => (status, took),
        None => (Status::Crash, start.elapsed()),
    }
}

/// Runs `bin args` like [`run_once`] and also returns the first `stderr_cap` bytes of its
/// stderr (0 = stderr to the null device). `None` when the program could not be started at
/// all, so callers can tell "absent" from "exited with an error". [`run_child`] has the
/// details.
pub fn probe(
    bin: &Path,
    args: &[OsString],
    limit: Duration,
    stderr_cap: u64,
) -> Option<(Status, Duration, Vec<u8>)> {
    run_child(bin, args, limit, stderr_cap, Keep::Head).map(|o| (o.status, o.took, o.stderr))
}

/// Which part of a child's stderr [`run_child`] keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// The first `stderr_cap` bytes; the pipe is closed at the cap, so later writes get EPIPE
    /// (engine probes, specs/tooling/bench.md item 2).
    Head,
    /// The last `stderr_cap` bytes; the pipe is drained to its end through a bounded buffer,
    /// so a panic message after a long log is kept (specs/tooling/nightly.md item 5).
    Tail,
}

/// How one [`run_child`] ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub status: Status,
    /// The exit code, when the child exited by itself.
    pub code: Option<i32>,
    /// The signal that ended the child (unix), when it was not our time-limit kill.
    pub signal: Option<i32>,
    /// Wall time from spawn to exit (or to the kill).
    pub took: Duration,
    /// At most `stderr_cap` bytes of stderr: the first or the last ones, by [`Keep`].
    pub stderr: Vec<u8>,
}

/// Runs `bin args` with stdin and stdout on the null device and returns how it ended
/// (`None` when it could not be started). At a timeout, `code` and `signal` are `None`.
///
/// The child runs in its own process group. When it exits, and at a timeout, the whole
/// group is killed, so a wrapper script's children can't outlive it and skew later runs.
/// While it runs, the watchdog holds its group id, so a bench killed by a signal doesn't
/// leave it running either (spec item 7). The watchdog holds one group, so runs must not
/// overlap (the bench runs them one at a time). Captured stderr goes
/// through a pipe read on a helper thread. [`Keep::Head`] stops reading at the cap (later
/// writes get EPIPE); [`Keep::Tail`] reads to the end and keeps only the last `stderr_cap`
/// bytes. Either way a flooding program costs no disk and at most the cap plus one read chunk
/// of memory; a tail reader costs CPU until the child's group is killed (at exit or at the
/// limit), and stops at its next read once the run is collected.
pub fn run_child(
    bin: &Path,
    args: &[OsString],
    limit: Duration,
    stderr_cap: u64,
    keep: Keep,
) -> Option<Outcome> {
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(if stderr_cap > 0 {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let start = Instant::now();
    let mut child = cmd.spawn().ok()?;
    // The group id is the child's pid (`process_group(0)`).
    let group = child.id();
    watch(Some(group));
    let (tx, rx) = mpsc::channel();
    // Set once the run is collected, so a tail reader whose pipe an escaped grandchild holds
    // open stops at its next read instead of draining forever.
    let stop = Arc::new(AtomicBool::new(false));
    if let Some(pipe) = child.stderr.take() {
        let stop = Arc::clone(&stop);
        // A thread that can't be created means nothing is captured (the pipe is dropped,
        // the child gets EPIPE), never a panic.
        let _ = thread::Builder::new().spawn(move || {
            let buf = match keep {
                Keep::Head => {
                    let mut buf = Vec::new();
                    let _ = pipe.take(stderr_cap).read_to_end(&mut buf);
                    buf
                }
                Keep::Tail => read_tail(pipe, stderr_cap, &stop),
            };
            // The read end is dropped here; the receiver may be gone already.
            let _ = tx.send(buf);
        });
    } else {
        drop(tx);
    }
    let (status, exit, took) = loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let took = start.elapsed();
                // The leader is already reaped here (std has no wait-without-reap), so
                // the watchdog lets go of the group id first: a bench killed from now on
                // must not make it signal an id that may be reused.
                watch(None);
                // Background children the engine left behind die with their group. The
                // id stays reserved only while such a child is alive; if none is, the
                // kill finds no group (ESRCH), unless the id was reused within the few
                // milliseconds the `/bin/sh` kill takes to start.
                kill_group(group);
                break (classify_exit(st), Some(st), took);
            }
            Ok(None) if start.elapsed() >= limit => {
                let took = start.elapsed();
                kill_tree(&mut child);
                break (Status::Timeout, None, took);
            }
            Ok(None) => thread::sleep(POLL),
            Err(_) => {
                kill_tree(&mut child);
                break (Status::Crash, None, start.elapsed());
            }
        }
    };
    // A grandchild may hold the pipe open; don't wait for it longer than a moment.
    let stderr = rx.recv_timeout(Duration::from_secs(1)).unwrap_or_default();
    stop.store(true, Ordering::Relaxed);
    Some(Outcome {
        status,
        code: exit.and_then(|st| st.code()),
        signal: exit.and_then(signal_of),
        took,
        stderr,
    })
}

/// Bytes read from a stderr pipe at a time by [`read_tail`].
const TAIL_CHUNK: usize = 8192;

/// Reads `r` to its end (or a read error, or until `stop` is set) and returns its last `cap`
/// bytes. Memory stays at most `cap` plus one chunk, whatever the reader yields.
fn read_tail(mut r: impl std::io::Read, cap: u64, stop: &AtomicBool) -> Vec<u8> {
    // A cap past the address space keeps everything, which no pipe can exceed anyway.
    let cap = usize::try_from(cap).unwrap_or(usize::MAX);
    let mut kept: VecDeque<u8> = VecDeque::new();
    let mut chunk = vec![0u8; TAIL_CHUNK];
    while !stop.load(Ordering::Relaxed) {
        let n = match r.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let got = chunk.get(..n).unwrap_or_default();
        // Only the chunk's last `cap` bytes can survive.
        let got = got.get(got.len().saturating_sub(cap)..).unwrap_or_default();
        kept.extend(got);
        let over = kept.len().saturating_sub(cap);
        kept.drain(..over);
    }
    kept.into()
}

#[cfg(unix)]
fn signal_of(st: ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    st.signal()
}

#[cfg(not(unix))]
fn signal_of(_: ExitStatus) -> Option<i32> {
    None
}

/// Kills the child's process group (unix) and the child itself, unregisters the group
/// from the watchdog, then reaps the child. The unreaped leader keeps the group id reserved
/// until both the group kill and the unregistering are done, so the watchdog never holds a
/// free id. Errors mean the processes are already gone.
fn kill_tree(child: &mut std::process::Child) {
    kill_group(child.id());
    let _ = child.kill();
    watch(None);
    #[cfg(test)]
    tests::record(tests::Event::Reap);
    let _ = child.wait();
}

/// Sends SIGKILL to every process in group `group` (unix). The shell builtin `kill`
/// signals the whole group and exists even where no `kill` binary is installed; std has no
/// safe group-kill API. An error means the group is already gone.
fn kill_group(group: u32) {
    #[cfg(test)]
    tests::record(tests::Event::KillGroup(group));
    #[cfg(unix)]
    {
        let _ = Command::new("/bin/sh")
            .args(["-c", "kill -s KILL -- \"-$1\"", "sh"])
            .arg(group.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(unix))]
    let _ = group;
}

/// The watchdog's script (spec item 7). It reads one line per event from the bench: a
/// process-group id when an engine starts, `-` when that engine's group is gone. At end of
/// input, which comes when the bench exits for any reason (a signal, SIGKILL, a panic), it
/// kills the last group still registered. It runs in its own process group, so a Ctrl-C
/// that stops the bench (SIGINT to the terminal's foreground group) doesn't reach it.
/// Only `2`..`9999999` without leading zeros kill (Linux `pid_max` is at most 4194304,
/// macOS pids stay below 100000): `kill -- -0` is the watchdog's own group, and `-1`
/// (also written `-01`) is every process of the user; neither can be an engine's group.
const WATCHDOG_SCRIPT: &str = "g=-; while IFS= read -r l; do g=$l; done; \
case $g in ''|*[!0-9]*|0*|1) ;; *) [ ${#g} -le 7 ] && kill -s KILL -- \"-$g\" ;; esac";

/// The write end of the watchdog's input; `None` when it could not be started.
static WATCHDOG: OnceLock<Option<Mutex<ChildStdin>>> = OnceLock::new();

/// Starts the watchdog once per process. Err when it can't be started; the bench then
/// refuses to run (an I/O error), because a stuck engine could outlive it.
pub fn start_watchdog() -> Result<(), String> {
    match watchdog() {
        Some(_) => Ok(()),
        None => Err("cannot start the engine watchdog (/bin/sh)".to_string()),
    }
}

fn watchdog() -> Option<&'static Mutex<ChildStdin>> {
    WATCHDOG
        .get_or_init(|| {
            let mut cmd = Command::new("/bin/sh");
            cmd.args(["-c", WATCHDOG_SCRIPT, "jpdf-bench-watchdog"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }
            // The child handle is dropped without waiting: the watchdog outlives the
            // bench by design and exits at end of input. Our end of the pipe is
            // close-on-exec, so engines never hold it open.
            let mut child = cmd.spawn().ok()?;
            child.stdin.take().map(Mutex::new)
        })
        .as_ref()
}

/// Tells the watchdog which group to kill if the bench dies now (`None`: no group). A
/// failed write (the watchdog is gone) leaves the run unprotected but still correct.
/// Only [`start_watchdog`] starts it: a bench that forgot to call it fails its tests
/// instead of starting one lazily (or running unprotected if it can't start).
fn watch(group: Option<u32>) {
    #[cfg(test)]
    tests::record(tests::Event::Watch(group));
    if let Some(Some(w)) = WATCHDOG.get() {
        let line = group.map_or_else(|| "-\n".to_string(), |g| format!("{g}\n"));
        // A poisoned lock only means another thread panicked mid-write; the pipe is fine.
        let mut pipe = w.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = pipe.write_all(line.as_bytes()).and_then(|()| pipe.flush());
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
    use std::cell::RefCell;

    /// What `probe` told the watchdog, and when it reaped a killed child (this thread only).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(super) enum Event {
        Watch(Option<u32>),
        KillGroup(u32),
        Reap,
    }

    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn record(e: Event) {
        EVENTS.with(|v| v.borrow_mut().push(e));
    }

    fn take_events() -> Vec<Event> {
        EVENTS.with(|v| std::mem::take(&mut *v.borrow_mut()))
    }

    #[cfg(unix)]
    #[test]
    fn the_watchdog_lets_go_of_every_group() {
        // spec item 7: each run registers its group and unregisters it, so the watchdog
        // never holds an id after its run; after a timeout, before the reap (the zombie
        // leader still reserves the id). Normal exit, then timeout.
        let sh = Path::new("/bin/sh");
        let _ = take_events();
        let (status, _) = run_once(sh, &["-c".into(), "exit 0".into()], Duration::from_secs(20));
        assert_eq!(status, Status::Ok);
        let ev = take_events();
        assert!(
            matches!(
                ev.as_slice(),
                [Event::Watch(Some(g)), Event::Watch(None), Event::KillGroup(k)] if *g > 1 && k == g
            ),
            "{ev:?}"
        );
        let (status, _) = run_once(
            sh,
            &["-c".into(), "exec sleep 10".into()],
            Duration::from_millis(100),
        );
        assert_eq!(status, Status::Timeout);
        let ev = take_events();
        assert!(
            matches!(
                ev.as_slice(),
                [
                    Event::Watch(Some(g)),
                    Event::KillGroup(k),
                    Event::Watch(None),
                    Event::Reap
                ] if k == g
            ),
            "{ev:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_child_reports_the_exit_code_or_the_signal() {
        // `cargo xtask nightly` records these (specs/tooling/nightly.md item 5).
        let sh = Path::new("/bin/sh");
        let limit = Duration::from_secs(20);
        let run = |script: &str, limit| {
            run_child(sh, &["-c".into(), script.into()], limit, 64, Keep::Head).unwrap()
        };
        let o = run("exit 0", limit);
        assert_eq!((o.status, o.code, o.signal), (Status::Ok, Some(0), None));
        let o = run("echo oops >&2; exit 7", limit);
        assert_eq!((o.status, o.code, o.signal), (Status::Crash, Some(7), None));
        assert_eq!(o.stderr, b"oops\n");
        let o = run("exit 3", limit);
        assert_eq!(
            (o.status, o.code, o.signal),
            (Status::NotImplemented, Some(3), None)
        );
        // SIGABRT (6), what a Rust abort or a failed allocation ends in.
        let o = run("kill -s ABRT $$", limit);
        assert_eq!((o.status, o.code, o.signal), (Status::Crash, None, Some(6)));
        // Our own kill at the limit is a timeout, not a signal.
        let o = run("exec sleep 10", Duration::from_millis(100));
        assert_eq!((o.status, o.code, o.signal), (Status::Timeout, None, None));
        assert!(run_child(Path::new("/nonexistent/x"), &[], limit, 0, Keep::Head).is_none());
    }

    #[test]
    fn read_tail_keeps_the_last_cap_bytes() {
        // specs/tooling/nightly.md item 5: the panic message after a long log survives.
        let go = AtomicBool::new(false);
        let tail = |input: &[u8], cap| read_tail(input, cap, &go);
        let mut log = vec![b'w'; 3 * TAIL_CHUNK + 17];
        log.extend_from_slice(b"panicked at the end\n");
        let t = tail(&log, 64);
        assert_eq!(t.len(), 64);
        assert!(t.ends_with(b"panicked at the end\n"), "{t:?}");
        assert_eq!(t, log.get(log.len() - 64..).unwrap());
        // Exactly the cap is kept whole; one byte more drops the first byte.
        let exact: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        assert_eq!(tail(&exact, 4096), exact);
        let mut more = vec![b'!'];
        more.extend_from_slice(&exact);
        assert_eq!(tail(&more, 4096), exact);
        // Less than the cap, nothing, and a zero cap.
        assert_eq!(tail(b"short", 4096), b"short");
        assert_eq!(tail(b"", 4096), b"");
        assert_eq!(tail(b"abc", 0), b"");
        // A cap larger than one chunk keeps bytes across chunks, in order.
        let big: Vec<u8> = (0..=255u8).cycle().take(5 * TAIL_CHUNK).collect();
        let t = tail(&big, 2 * TAIL_CHUNK as u64 + 3);
        assert_eq!(t, big.get(big.len() - (2 * TAIL_CHUNK + 3)..).unwrap());
        // A set stop flag reads nothing more.
        let stopped = AtomicBool::new(true);
        assert_eq!(read_tail(&log[..], 64, &stopped), b"");
    }

    #[test]
    fn read_tail_survives_interrupts_and_stops_at_an_error() {
        // A reader that yields one byte per call, an `Interrupted` error in between, then a
        // hard error: the bytes before the error are kept, in order.
        struct Flaky(Vec<std::io::Result<u8>>);
        impl std::io::Read for Flaky {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0.is_empty() {
                    return Ok(0);
                }
                match self.0.remove(0) {
                    Ok(b) => {
                        buf[0] = b;
                        Ok(1)
                    }
                    Err(e) => Err(e),
                }
            }
        }
        use std::io::{Error, ErrorKind};
        let r = Flaky(vec![
            Ok(b'a'),
            Err(Error::from(ErrorKind::Interrupted)),
            Ok(b'b'),
            Ok(b'c'),
            Err(Error::from(ErrorKind::BrokenPipe)),
            Ok(b'x'),
        ]);
        assert_eq!(read_tail(r, 2, &AtomicBool::new(false)), b"bc");
    }

    #[cfg(unix)]
    #[test]
    fn run_child_keeps_the_head_or_the_tail_of_a_flood() {
        // 20 000 bytes of warnings, then the panic line: the tail keeps it, the head doesn't.
        let sh = Path::new("/bin/sh");
        let script =
            "head -c 20000 /dev/zero | tr '\\0' w >&2; echo 'panicked: last' >&2; exit 101";
        let run = |keep| {
            run_child(
                sh,
                &["-c".into(), script.into()],
                Duration::from_secs(20),
                4096,
                keep,
            )
            .unwrap()
        };
        let t = run(Keep::Tail);
        assert_eq!((t.status, t.code), (Status::Crash, Some(101)));
        assert_eq!(t.stderr.len(), 4096);
        let mut want = vec![b'w'; 4096 - 15];
        want.extend_from_slice(b"panicked: last\n");
        assert_eq!(t.stderr, want);
        let h = run(Keep::Head);
        assert_eq!(h.stderr, vec![b'w'; 4096]);
        // A run with no stderr at all keeps nothing in either mode.
        let o = run_child(
            sh,
            &["-c".into(), "exit 0".into()],
            Duration::from_secs(20),
            4096,
            Keep::Tail,
        )
        .unwrap();
        assert_eq!((o.status, o.stderr.len()), (Status::Ok, 0));
    }

    #[cfg(unix)]
    #[test]
    fn watchdog_kills_only_plain_group_ids() {
        // spec item 7. `kill` is replaced by `echo` in the script under test, so no signal
        // is ever sent here, whatever the guard does with `1` or `01`.
        assert_eq!(WATCHDOG_SCRIPT.matches("kill -s KILL").count(), 1);
        let script = WATCHDOG_SCRIPT.replace("kill -s KILL", "echo");
        let run = |input: &str| {
            let mut dog = Command::new("/bin/sh")
                .args(["-c", &script, "watchdog"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let mut pipe = dog.stdin.take().unwrap();
            pipe.write_all(input.as_bytes()).unwrap();
            drop(pipe);
            let out = dog.wait_with_output().unwrap();
            String::from_utf8(out.stdout).unwrap()
        };
        assert_eq!(run("12345\n"), "-- -12345\n");
        assert_eq!(run("2\n"), "-- -2\n");
        assert_eq!(run("9999999\n"), "-- -9999999\n");
        for spare in [
            "",
            "-\n",
            "0\n",
            "1\n",
            "00\n",
            "01\n",
            "012345\n",
            "10000000\n",
            "4294967297\n",
            "12x\n",
            "-12\n",
            " 12\n",
            "12345\n-\n",
        ] {
            assert_eq!(run(spare), "", "{spare:?}");
        }
    }

    /// Runs the watchdog script on `input` (then end of input) against two `sleep`s, each in
    /// its own group (`{g}` and `{h}` in `input`); returns whether each survived.
    #[cfg(unix)]
    fn sleepers_survive(input: &str) -> (bool, bool) {
        use std::os::unix::process::CommandExt;
        let sleeper = || {
            Command::new("sleep")
                .arg("30")
                .process_group(0)
                .spawn()
                .unwrap()
        };
        let (mut g, mut h) = (sleeper(), sleeper());
        let mut dog = Command::new("/bin/sh")
            .args(["-c", WATCHDOG_SCRIPT, "watchdog"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let text = input
            .replace("{g}", &g.id().to_string())
            .replace("{h}", &h.id().to_string());
        let mut pipe = dog.stdin.take().unwrap();
        pipe.write_all(text.as_bytes()).unwrap();
        drop(pipe);
        assert!(dog.wait().unwrap().success());
        // A killed sleeper is reaped by us, so try_wait sees it exit.
        let start = Instant::now();
        let mut dead = (false, false);
        while start.elapsed() < Duration::from_secs(2) && !(dead.0 && dead.1) {
            dead.0 = dead.0 || g.try_wait().unwrap().is_some();
            dead.1 = dead.1 || h.try_wait().unwrap().is_some();
            thread::sleep(Duration::from_millis(20));
        }
        for c in [&mut g, &mut h] {
            let _ = c.kill();
            let _ = c.wait();
        }
        (!dead.0, !dead.1)
    }

    #[cfg(unix)]
    #[test]
    fn watchdog_kills_only_the_group_still_registered() {
        // spec item 7: end of input kills the last registered group, `-` unregisters it,
        // and a later registration replaces an earlier one. Only ids of our own sleepers
        // are ever written: a broken script must not get a chance at `kill -- -1`.
        assert_eq!(sleepers_survive("{g}\n"), (false, true));
        assert_eq!(sleepers_survive("{g}\n-\n"), (true, true));
        assert_eq!(sleepers_survive(""), (true, true));
        assert_eq!(sleepers_survive("{h}\n-\n{g}\n"), (false, true));
        assert_eq!(sleepers_survive("{h}\n{g}\n"), (false, true));
        // Anything but digits kills nothing (not even `kill -- -` forms).
        assert_eq!(sleepers_survive("{g}\n-{h}\n"), (true, true));
        assert_eq!(sleepers_survive("{g}\n{g}x\n"), (true, true));
    }

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
