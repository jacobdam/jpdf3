//! Runs the bench and renders the report (spec items 2, 3, 8, 9).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::args::{Choice, Options};
use crate::engine::{self, Engine, Kind, Measurement, Op, Status};
use crate::json;
use crate::machine::Machine;
use crate::stats;

/// Page counts of the generated sample (spec item 3).
pub const GENERATED_PAGES: [usize; 4] = [1, 10, 100, 1000];

/// Why the bench could not run: a usage problem (exit 2) or an I/O failure (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchError {
    Usage(String),
    Io(String),
}

impl std::fmt::Display for BenchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BenchError::Usage(s) | BenchError::Io(s) => f.write_str(s),
        }
    }
}

/// A file to bench.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    /// Base name, as shown in the report.
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// Results for one file: per operation, per engine that has a command for it.
#[derive(Debug, Clone, PartialEq)]
pub struct FileResult {
    pub name: String,
    pub bytes: u64,
    pub results: Vec<(Op, Vec<(Kind, Measurement)>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub machine: Machine,
    pub revision: Option<String>,
    pub corpus: bool,
    pub runs: usize,
    pub warmup: usize,
    /// Every engine that ran (jpdf first).
    pub engines: Vec<Engine>,
    pub files: Vec<FileResult>,
}

/// Runs the whole bench.
pub fn run(opts: &Options) -> Result<Report, BenchError> {
    // Absolute paths: a relative path starting with `-` would otherwise read as an engine flag.
    let abs = |p: &Path| {
        std::path::absolute(p)
            .map_err(|e| BenchError::Io(format!("cannot resolve {}: {e}", p.display())))
    };
    let cli = abs(&opts.cli)?;
    let work = abs(&match &opts.work {
        Some(w) => w.clone(),
        None => default_work(&opts.out),
    })?;
    let corpus = opts.corpus.as_deref().map(abs).transpose()?;
    if !cli.is_file() {
        return Err(BenchError::Io(format!(
            "--cli {} is not a file",
            opts.cli.display()
        )));
    }
    if let Some(dir) = &corpus
        && let (Ok(c), Ok(w)) = (dir.canonicalize(), canonical_or_self(&work))
        && w.starts_with(&c)
    {
        return Err(BenchError::Usage(
            "the work dir must not be inside the corpus dir".to_string(),
        ));
    }
    // Before anything is written: a bad explicit engine path is a usage error.
    let mutool = detect(Kind::Mutool, &opts.mutool)?;
    let pdfium = detect(Kind::Pdfium, &opts.pdfium)?;
    std::fs::create_dir_all(&work)
        .map_err(|e| BenchError::Io(format!("cannot create {}: {e}", work.display())))?;
    let mut engines = vec![Engine {
        kind: Kind::Jpdf,
        bin: cli,
        version: None,
    }];
    engines.extend(mutool);
    engines.extend(pdfium);

    let inputs = match &corpus {
        Some(dir) => corpus_inputs(dir, opts.sample)?,
        None => generated_inputs(&work)?,
    };

    let mut files = Vec::with_capacity(inputs.len());
    for input in &inputs {
        files.push(bench_file(input, &engines, &work, opts.runs, opts.warmup)?);
    }
    Ok(Report {
        machine: Machine::detect(),
        revision: git_revision(),
        corpus: corpus.is_some(),
        runs: opts.runs,
        warmup: opts.warmup,
        engines,
        files,
    })
}

/// Canonical form of `p`, or of its nearest existing ancestor joined with the rest (the work
/// dir may not exist yet).
fn canonical_or_self(p: &Path) -> std::io::Result<PathBuf> {
    match p.canonicalize() {
        Ok(c) => Ok(c),
        Err(e) => match (p.parent(), p.file_name()) {
            (Some(parent), Some(name)) => Ok(canonical_or_self(parent)?.join(name)),
            _ => Err(e),
        },
    }
}

/// `<out>.work` next to the report.
pub fn default_work(out: &Path) -> PathBuf {
    let mut name = out.file_name().map(OsString::from).unwrap_or_default();
    name.push(".work");
    out.with_file_name(name)
}

fn bench_file(
    input: &Input,
    engines: &[Engine],
    work: &Path,
    runs: usize,
    warmup: usize,
) -> Result<FileResult, BenchError> {
    // pdfium_test writes its output next to its input, so it gets a copy inside `work`.
    let pdfium_copy = work.join("pdfium-input.pdf");
    if engines.iter().any(|e| e.kind == Kind::Pdfium) && input.path != pdfium_copy {
        std::fs::copy(&input.path, &pdfium_copy).map_err(|e| {
            BenchError::Io(format!("cannot copy {} into the work dir: {e}", input.name))
        })?;
    }
    let mut results = Vec::with_capacity(Op::ALL.len());
    for op in Op::ALL {
        let mut per_engine = Vec::with_capacity(engines.len());
        for e in engines {
            let file = if e.kind == Kind::Pdfium {
                &pdfium_copy
            } else {
                &input.path
            };
            if let Some(argv) = e.argv(op, file, work) {
                let m = engine::measure(&e.bin, &argv, op.time_limit(), runs, warmup);
                per_engine.push((e.kind, m));
            }
        }
        results.push((op, per_engine));
    }
    Ok(FileResult {
        name: input.name.clone(),
        bytes: input.bytes,
        results,
    })
}

/// The first `sample` `*.pdf` files directly inside `dir`, by base name in byte order.
pub fn corpus_inputs(dir: &Path, sample: usize) -> Result<Vec<Input>, BenchError> {
    let io = |e: std::io::Error| BenchError::Io(format!("cannot read {}: {e}", dir.display()));
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(io)? {
        let entry = entry.map_err(io)?;
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "pdf") {
            continue;
        }
        // Symlinks are followed; an entry that vanished or dangles is skipped, not fatal.
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        found.push((entry.file_name(), path, meta.len()));
    }
    found.sort_by(|a, b| a.0.as_encoded_bytes().cmp(b.0.as_encoded_bytes()));
    Ok(found
        .into_iter()
        .take(sample)
        .map(|(name, path, bytes)| Input {
            name: name.to_string_lossy().into_owned(),
            path,
            bytes,
        })
        .collect())
}

/// Writes the generated sample into `work` (spec item 3).
pub fn generated_inputs(work: &Path) -> Result<Vec<Input>, BenchError> {
    GENERATED_PAGES
        .iter()
        .map(|&pages| {
            let name = format!("gen-pages-{pages:04}.pdf");
            let path = work.join(&name);
            let data = jpdf_testgen::minimal_pdf(pages);
            std::fs::write(&path, &data)
                .map_err(|e| BenchError::Io(format!("cannot write {}: {e}", path.display())))?;
            Ok(Input {
                name,
                path,
                bytes: u64::try_from(data.len()).unwrap_or(u64::MAX),
            })
        })
        .collect()
}

/// How long a reference engine's start-up probe may take.
const PROBE_LIMIT: Duration = Duration::from_secs(5);
/// Most bytes of probe output read back (for the version line).
const PROBE_OUTPUT_CAP: u64 = 4096;

/// Finds a reference engine with one probe run (`mutool -v`, bare `pdfium_test`) under
/// [`PROBE_LIMIT`] and the usual group kill. Its stdout goes to the null device; at most
/// [`PROBE_OUTPUT_CAP`] bytes of mutool's stderr are read from a pipe, so neither a hang nor
/// endless output can stall the bench or fill memory or disk. `Auto` that can't be started → absent; an
/// explicit path that can't be started → usage error. Any exit or a timeout means present.
fn detect(kind: Kind, choice: &Choice) -> Result<Option<Engine>, BenchError> {
    let (bin, explicit) = match choice {
        Choice::Off => return Ok(None),
        Choice::Auto => (
            PathBuf::from(if kind == Kind::Mutool {
                "mutool"
            } else {
                "pdfium_test"
            }),
            false,
        ),
        Choice::Path(p) => (p.clone(), true),
    };
    let args: Vec<OsString> = if kind == Kind::Mutool {
        vec![OsString::from("-v")]
    } else {
        Vec::new()
    };
    let cap = if kind == Kind::Mutool {
        PROBE_OUTPUT_CAP
    } else {
        0
    };
    let Some((status, _, stderr)) = engine::probe(&bin, &args, PROBE_LIMIT, cap) else {
        return if explicit {
            Err(BenchError::Usage(format!(
                "--{} {}: cannot run",
                kind.id(),
                bin.display()
            )))
        } else {
            Ok(None)
        };
    };
    // Only a line that really is mutool's version is kept, so a wrong program can't put its
    // usage text (often with absolute paths) into the report.
    let version = (kind == Kind::Mutool && status != Status::Timeout)
        .then(|| first_line(&stderr))
        .flatten()
        .filter(|l| l.starts_with("mutool version"));
    Ok(Some(Engine { kind, bin, version }))
}

fn first_line(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(line.chars().take(200).collect())
}

/// `git rev-parse HEAD` in the current directory, if it is a 40-hex commit id.
fn git_revision() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())).then_some(s)
}

/// Summary of one engine on one operation (spec item 8).
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// Count per status, in [`Status::ALL`] order.
    pub counts: [usize; 5],
    pub median_ms: Option<f64>,
    pub p95_ms: Option<f64>,
}

fn measurements(report: &Report, op: Op, kind: Kind) -> Vec<Measurement> {
    report
        .files
        .iter()
        .flat_map(|f| f.results.iter())
        .filter(|(o, _)| *o == op)
        .flat_map(|(_, per)| per.iter())
        .filter(|(k, _)| *k == kind)
        .map(|(_, m)| *m)
        .collect()
}

/// Status counts and timing summary; `None` if the engine has no command for `op`.
pub fn summarize(report: &Report, op: Op, kind: Kind) -> Option<Summary> {
    let ms = measurements(report, op, kind);
    if ms.is_empty() {
        return None;
    }
    let mut counts = [0usize; 5];
    for m in &ms {
        if let Some(i) = Status::ALL.iter().position(|s| *s == m.status)
            && let Some(c) = counts.get_mut(i)
        {
            *c = c.saturating_add(1);
        }
    }
    let mut times: Vec<f64> = ms
        .iter()
        .filter(|m| m.status == Status::Ok)
        .filter_map(|m| m.micros)
        .map(|us| us / 1000.0)
        .collect();
    let median_ms = stats::median_f64(&mut times);
    let p95_ms = stats::p95(&mut times);
    Some(Summary {
        counts,
        median_ms,
        p95_ms,
    })
}

/// Geometric-mean ratio jpdf / `reference` on `op` over files where both are `ok`, with
/// the number of such files.
pub fn ratio(report: &Report, op: Op, reference: Kind) -> (Option<f64>, usize) {
    let ok_time = |per: &[(Kind, Measurement)], kind: Kind| {
        per.iter()
            .find(|(k, _)| *k == kind)
            .filter(|(_, m)| m.status == Status::Ok)
            .and_then(|(_, m)| m.micros)
    };
    let pairs: Vec<(f64, f64)> = report
        .files
        .iter()
        .flat_map(|f| f.results.iter())
        .filter(|(o, _)| *o == op)
        .filter_map(|(_, per)| Some((ok_time(per, Kind::Jpdf)?, ok_time(per, reference)?)))
        .collect();
    (stats::ratio(&pairs), pairs.len())
}

/// Renders the report (spec item 9), with a trailing newline.
pub fn to_json(r: &Report) -> String {
    let mut o = String::new();
    o.push_str("{\"schema\":\"jpdf-bench/1\",\"machine\":{\"id\":");
    json::string(&mut o, &r.machine.id());
    o.push_str(",\"os\":");
    json::string(&mut o, &r.machine.os);
    o.push_str(",\"arch\":");
    json::string(&mut o, &r.machine.arch);
    o.push_str(",\"cpu\":");
    json::string(&mut o, &r.machine.cpu);
    o.push_str(&format!(
        ",\"logical_cpus\":{},\"memory_bytes\":{}}}",
        r.machine.logical_cpus, r.machine.memory_bytes
    ));
    o.push_str(",\"revision\":");
    json::opt_string(&mut o, r.revision.as_deref());
    o.push_str(&format!(
        ",\"config\":{{\"sample\":\"{}\",\"files\":{},\"runs\":{},\"warmup\":{}}}",
        if r.corpus { "corpus" } else { "generated" },
        r.files.len(),
        r.runs,
        r.warmup
    ));
    o.push_str(",\"engines\":{\"jpdf\":{\"present\":true}");
    for kind in [Kind::Mutool, Kind::Pdfium] {
        let e = r.engines.iter().find(|e| e.kind == kind);
        o.push_str(&format!(
            ",\"{}\":{{\"present\":{},\"version\":",
            kind.id(),
            e.is_some()
        ));
        json::opt_string(&mut o, e.and_then(|e| e.version.as_deref()));
        o.push('}');
    }
    o.push_str("},\"operations\":{");
    for (i, op) in Op::ALL.into_iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        o.push_str(&format!("\"{}\":{{\"engines\":{{", op.id()));
        let mut first = true;
        for e in &r.engines {
            let Some(s) = summarize(r, op, e.kind) else {
                continue;
            };
            if !first {
                o.push(',');
            }
            first = false;
            o.push_str(&format!("\"{}\":{{", e.kind.id()));
            for (j, (status, n)) in Status::ALL.iter().zip(s.counts).enumerate() {
                if j > 0 {
                    o.push(',');
                }
                o.push_str(&format!("\"{}\":{n}", status.id()));
            }
            o.push_str(",\"median_ms\":");
            json::number(&mut o, s.median_ms);
            o.push_str(",\"p95_ms\":");
            json::number(&mut o, s.p95_ms);
            o.push('}');
        }
        o.push_str("},\"ratio\":{");
        for (j, reference) in [Kind::Mutool, Kind::Pdfium].into_iter().enumerate() {
            if j > 0 {
                o.push(',');
            }
            let (value, samples) = ratio(r, op, reference);
            o.push_str(&format!("\"{}\":{{\"value\":", reference.id()));
            json::number(&mut o, value);
            o.push_str(&format!(",\"samples\":{samples}}}"));
        }
        o.push_str("}}");
    }
    o.push_str("},\"files\":[");
    for (i, f) in r.files.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        o.push_str("\n{\"name\":");
        json::string(&mut o, &f.name);
        o.push_str(&format!(",\"bytes\":{},\"results\":{{", f.bytes));
        for (j, (op, per)) in f.results.iter().enumerate() {
            if j > 0 {
                o.push(',');
            }
            o.push_str(&format!("\"{}\":{{", op.id()));
            for (k, (kind, m)) in per.iter().enumerate() {
                if k > 0 {
                    o.push(',');
                }
                o.push_str(&format!(
                    "\"{}\":{{\"status\":\"{}\",\"ms\":",
                    kind.id(),
                    m.status.id()
                ));
                json::number(&mut o, m.micros.map(|us| us / 1000.0));
                o.push('}');
            }
            o.push('}');
        }
        o.push_str("}}");
    }
    o.push_str("]}\n");
    o
}

/// Writes the report to `out`, creating parent directories.
pub fn write(out: &Path, report: &Report) -> Result<(), BenchError> {
    let io = |e: std::io::Error| BenchError::Io(format!("cannot write {}: {e}", out.display()));
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(io)?;
    }
    // Write then rename, so a failure part-way never leaves a truncated report.
    let mut tmp = out.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let result = std::fs::write(&tmp, to_json(report)).and_then(|()| std::fs::rename(&tmp, out));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(io)
}
