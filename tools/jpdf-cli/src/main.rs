//! jpdf-cli — the headless interface the judge grades (ops/judge/CONTRACT.md §1).
//! Spec: specs/cli/jpdf-cli.md.
//!
//! Exit codes: 0 success, 2 clean typed error, 3 not implemented. Nothing else, ever.
#![forbid(unsafe_code)]
// Input-derived arithmetic is checked; overflow panics in every profile (#77).
#![deny(clippy::arithmetic_side_effects)]

use std::ffi::{OsStr, OsString};
use std::fs::{self, Metadata};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jpdf_engine::{Document, EngineError, Limits};

const EXIT_ERROR: u8 = 2;
const EXIT_NOT_IMPLEMENTED: u8 = 3;

/// A command of CONTRACT §1: its required and optional `--flags` (each takes one value).
struct Command {
    name: &'static str,
    required: &'static [&'static str],
    optional: &'static [&'static str],
}

const COMMANDS: &[Command] = &[
    Command {
        name: "info",
        required: &[],
        optional: &["password"],
    },
    Command {
        name: "render",
        required: &["page", "dpi", "out"],
        optional: &["password"],
    },
    Command {
        name: "text",
        required: &["page"],
        optional: &["password"],
    },
    Command {
        name: "annotate",
        required: &["out", "page", "kind", "rect", "color", "save"],
        optional: &[],
    },
    Command {
        name: "rewrite",
        required: &["out", "mode"],
        optional: &[],
    },
    Command {
        name: "encrypt",
        required: &["out", "user-password", "owner-password"],
        optional: &[],
    },
    Command {
        name: "fill",
        required: &["out", "field", "value"],
        optional: &[],
    },
    Command {
        name: "redact",
        required: &["out", "page", "rect"],
        optional: &[],
    },
    Command {
        name: "sign",
        required: &["out", "p12", "p12-password", "field"],
        optional: &[],
    },
    Command {
        name: "verify-signatures",
        required: &[],
        optional: &[],
    },
];

const USAGE: &str = "usage: jpdf-cli <info|render|text|annotate|rewrite|encrypt|fill|redact|sign|verify-signatures> FILE [--flag VALUE]... (see ops/judge/CONTRACT.md)";

/// A failed run: exit code plus one line for stderr.
#[derive(Debug)]
struct Failure {
    code: u8,
    message: String,
}

impl Failure {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            code: EXIT_ERROR,
            message: message.into(),
        }
    }
}

impl From<EngineError> for Failure {
    fn from(e: EngineError) -> Self {
        let code = match e {
            EngineError::Unsupported(_) => EXIT_NOT_IMPLEMENTED,
            EngineError::Malformed { .. } | EngineError::LimitExceeded(_) => EXIT_ERROR,
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

/// Parsed command line.
#[derive(Debug)]
struct Invocation {
    command: &'static str,
    file: PathBuf,
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => {
            // Ignore stderr write errors: there is nowhere left to report them.
            let _ = writeln!(io::stderr(), "jpdf-cli: {}", f.message);
            ExitCode::from(f.code)
        }
    }
}

fn run(args: &[OsString]) -> Result<(), Failure> {
    let inv = parse_args(args)?;
    match inv.command {
        "info" => info(&inv),
        other => Err(Failure {
            code: EXIT_NOT_IMPLEMENTED,
            message: format!("not implemented: {other}"),
        }),
    }
}

fn parse_args(args: &[OsString]) -> Result<Invocation, Failure> {
    let (first, rest) = args.split_first().ok_or_else(|| Failure::usage(USAGE))?;
    let name = first.to_str().unwrap_or_default();
    let cmd = COMMANDS
        .iter()
        .find(|c| c.name == name)
        .ok_or_else(|| Failure::usage(format!("unknown command {first:?}; {USAGE}")))?;

    let mut file: Option<PathBuf> = None;
    let mut seen: Vec<&'static str> = Vec::new();
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        let looks_like_flag = arg.as_encoded_bytes().starts_with(b"--");
        if looks_like_flag && arg.to_str().is_none() {
            return Err(Failure::usage(format!("flag {arg:?} is not valid UTF-8")));
        }
        let text = arg.to_str().unwrap_or_default();
        if let Some(flag) = text.strip_prefix("--").filter(|_| looks_like_flag) {
            let known = cmd
                .required
                .iter()
                .chain(cmd.optional)
                .find(|f| **f == flag)
                .ok_or_else(|| Failure::usage(format!("{}: unknown flag --{flag}", cmd.name)))?;
            if seen.contains(known) {
                return Err(Failure::usage(format!("--{known} given twice")));
            }
            let value = iter
                .next()
                .ok_or_else(|| Failure::usage(format!("--{known} needs a value")))?;
            validate_flag(known, value)?;
            seen.push(known);
        } else if file.is_some() {
            return Err(Failure::usage(format!("unexpected argument {arg:?}")));
        } else {
            file = Some(PathBuf::from(arg));
        }
    }
    let file = file.ok_or_else(|| Failure::usage(format!("{}: missing FILE", cmd.name)))?;
    if let Some(missing) = cmd.required.iter().find(|r| !seen.contains(r)) {
        return Err(Failure::usage(format!("{}: missing --{missing}", cmd.name)));
    }
    Ok(Invocation {
        command: cmd.name,
        file,
    })
}

/// Syntactic checks for flag values (CONTRACT §1). Paths, passwords and names are free text.
fn validate_flag(flag: &str, value: &OsStr) -> Result<(), Failure> {
    let bad = |why: &str| Failure::usage(format!("--{flag}: {why}"));
    let text = || value.to_str().ok_or_else(|| bad("not valid UTF-8"));
    match flag {
        "page" => match text()?.parse::<u32>() {
            Ok(n) if n >= 1 => Ok(()),
            _ => Err(bad("expected a page number >= 1")),
        },
        "dpi" => match text()?.parse::<u32>() {
            Ok(n) if (1..=2400).contains(&n) => Ok(()),
            _ => Err(bad("expected an integer in 1..=2400")),
        },
        "rect" => {
            let parts: Result<Vec<f64>, _> = text()?.split(',').map(str::parse::<f64>).collect();
            match parts {
                Ok(p) if p.len() == 4 && p.iter().all(|v| v.is_finite()) => Ok(()),
                _ => Err(bad("expected x0,y0,x1,y1")),
            }
        }
        "color" => {
            let parts: Result<Vec<u8>, _> = text()?.split(',').map(str::parse::<u8>).collect();
            match parts {
                Ok(p) if p.len() == 3 => Ok(()),
                _ => Err(bad("expected R,G,B with each in 0..=255")),
            }
        }
        "kind" => expect_word(text()?, "highlight", bad),
        "save" => expect_word(text()?, "incremental", bad),
        "mode" => expect_word(text()?, "full", bad),
        _ => Ok(()),
    }
}

fn expect_word(
    got: &str,
    want: &'static str,
    bad: impl Fn(&str) -> Failure,
) -> Result<(), Failure> {
    if got == want {
        Ok(())
    } else {
        Err(bad(&format!("only `{want}` is supported")))
    }
}

/// `info FILE [--password PW]` → `{"pages": N}`. The password is unused until encryption lands.
fn info(inv: &Invocation) -> Result<(), Failure> {
    let limits = Limits::default();
    let bytes = read_limited(&inv.file, limits.max_file_size)?;
    let doc = Document::open(&bytes, &limits)?;
    writeln!(io::stdout().lock(), "{{\"pages\": {}}}", doc.page_count())
        .map_err(|e| Failure::usage(format!("cannot write output: {e}")))
}

/// Accepts only a regular file of at most `max` bytes, judged from metadata alone, and
/// returns its length.
fn check_metadata(path: &Path, meta: &Metadata, max: usize) -> Result<usize, Failure> {
    if !meta.is_file() {
        return Err(Failure::usage(format!(
            "cannot read {}: not a regular file",
            path.display()
        )));
    }
    match usize::try_from(meta.len()) {
        Ok(len) if len <= max => Ok(len),
        _ => Err(Failure::usage("limit exceeded: file size")),
    }
}

/// Reads at most `max` bytes. Non-regular files (FIFOs, devices, directories) and files larger
/// than `max` are rejected from `stat` before the file is opened, so nothing is read. The open
/// never blocks where `jpdf_fsio::OPEN_FLAGS` is set; the opened file is checked again (the path may have been swapped), and the
/// read stops after `max + 1` bytes in case the file grows while we read it.
fn read_limited(path: &Path, max: usize) -> Result<Vec<u8>, Failure> {
    let cannot = |e: io::Error| Failure::usage(format!("cannot read {}: {e}", path.display()));
    check_metadata(path, &fs::metadata(path).map_err(cannot)?, max)?;
    let file = jpdf_fsio::open_nonblocking(path).map_err(cannot)?;
    let expected = check_metadata(path, &file.metadata().map_err(cannot)?, max)?;
    read_capped(file, expected, max).map_err(|e| match e {
        Capped::Io(e) => cannot(e),
        Capped::Fail(f) => f,
    })
}

#[derive(Debug)]
enum Capped {
    Io(io::Error),
    Fail(Failure),
}

/// Reads `reader` to the end, but never more than `max + 1` bytes: a source longer than its
/// metadata said (a growing file) fails with the size limit instead of growing memory.
/// `expected` is the size from metadata, reserved exactly up front: `read_to_end` doubling
/// would otherwise reserve up to twice the file size, which matters under the judge's 2 GiB
/// address-space limit.
fn read_capped(reader: impl Read, expected: usize, max: usize) -> Result<Vec<u8>, Capped> {
    let cap = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected.min(max).saturating_add(1))
        .map_err(|_| Capped::Fail(Failure::usage("cannot allocate memory for the file")))?;
    reader
        .take(cap)
        .read_to_end(&mut bytes)
        .map_err(Capped::Io)?;
    if bytes.len() > max {
        return Err(Capped::Fail(Failure::usage("limit exceeded: file size")));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The size cap holds even when the source is longer than its metadata said (a growing
    /// file): exactly `max` bytes pass, `max + 1` and an endless source fail with the limit.
    #[test]
    fn read_stops_one_byte_past_the_limit() {
        let at = read_capped(&[7u8; 100][..], 0, 100);
        assert_eq!(at.map(|b| b.len()).ok(), Some(100));
        for over in [
            read_capped(&[7u8; 101][..], 0, 100),
            read_capped(io::repeat(0), 0, 100),
        ] {
            match over {
                Err(Capped::Fail(f)) => assert_eq!(f.message, "limit exceeded: file size"),
                other => panic!("expected the size limit, got {other:?}"),
            }
        }
    }

    /// The `stat` check refuses FIFOs before opening, but the path can be swapped between the
    /// two calls; the open itself must then not block, and the check on the opened file must
    /// refuse it (#77). Opening a FIFO with no writer blocks without `O_NONBLOCK`, so a wrong
    /// flag value hangs here and fails the timeout.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn open_never_blocks_on_a_fifo() {
        // A plain open of a FIFO blocks by design where the flags are unknown.
        if jpdf_fsio::OPEN_FLAGS.is_none() {
            return;
        }
        use std::process::Command;
        use std::sync::mpsc;
        use std::time::Duration;
        let dir = std::env::temp_dir().join(format!("jpdf-cli-unit-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let fifo = dir.join("fifo");
        let _ = fs::remove_file(&fifo);
        assert!(
            Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let (tx, rx) = mpsc::channel();
        let path = fifo.clone();
        std::thread::spawn(move || {
            let checked = jpdf_fsio::open_nonblocking(&path)
                .map(|f| f.metadata().map(|m| check_metadata(&path, &m, 1 << 20)));
            let _ = tx.send(match checked {
                Ok(Ok(Err(f))) => f.message,
                other => format!("unexpected: {other:?}"),
            });
        });
        let got = rx.recv_timeout(Duration::from_secs(10));
        // On a timeout the reader thread stays blocked in its open; it ends with the test
        // process (nothing can unblock it once the FIFO is removed below).
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(
            got,
            Ok(format!(
                "cannot read {}: not a regular file",
                fifo.display()
            )),
            "open of a FIFO blocked, failed or was accepted"
        );
    }
}
