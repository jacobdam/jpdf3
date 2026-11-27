//! `cargo xtask limits [--out DIR] [--mib N]`: the adversarial resource-limit harness (#25).
//! Seven generated attack files of about N MiB each go through `jpdf-cli info` under the
//! judge's 2 GiB address-space limit and its 20 s `info` limit. Spec: specs/tooling/limits.md
//! (specs/cli/jpdf-cli.md §Limits, ops/judge/CONTRACT.md §1). `cargo xtask nightly` runs it.

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use jpdf_bench::engine::{Outcome, Status};

/// The judge's `info` time limit (CONTRACT §1).
const INFO_LIMIT: Duration = Duration::from_secs(20);
/// Default size of each attack file, in MiB (spec item 2).
pub const DEFAULT_MIB: u64 = 500;
/// Largest `--mib`: one below `jpdf-cli`'s 512 MiB `max_file_size`, so no file is refused by
/// its size alone.
const MAX_MIB: u64 = 511;
const MIB: u64 = 1024 * 1024;

/// What a case must end with (spec item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// Exit 0: a hostile-but-valid file that must open within the limits.
    Opens,
    /// Exit 2 with this text on stderr: the defence the case is built for.
    Refused(&'static str),
}

/// One attack class (spec item 4).
#[derive(Debug, Clone, Copy)]
pub struct Case {
    pub id: &'static str,
    pub expect: Expect,
    generate: fn(&mut Out, u64) -> io::Result<()>,
}

/// The seven classes, in run order.
pub const CASES: [Case; 7] = [
    Case {
        id: "nested-strings",
        expect: Expect::Refused("malformed PDF: indirect objects overlap"),
        generate: nested_strings,
    },
    Case {
        id: "shared-body",
        expect: Expect::Refused("malformed PDF: indirect objects overlap"),
        generate: shared_body,
    },
    Case {
        id: "stacked-prev",
        expect: Expect::Refused("malformed PDF: cross-reference sections overlap"),
        generate: stacked_prev,
    },
    Case {
        id: "xref-entries",
        expect: Expect::Refused("limit exceeded: cross-reference entries"),
        generate: xref_entries,
    },
    Case {
        id: "max-per-object",
        expect: Expect::Opens,
        generate: max_per_object,
    },
    Case {
        id: "dense-kids",
        expect: Expect::Opens,
        generate: dense_kids,
    },
    Case {
        id: "long-comment",
        expect: Expect::Opens,
        generate: long_comment,
    },
];

/// A writer that knows its position (the byte offsets the xref tables need).
pub struct Out<'a> {
    w: &'a mut dyn Write,
    pos: u64,
}

impl std::fmt::Debug for Out<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Out").field("pos", &self.pos).finish()
    }
}

impl<'a> Out<'a> {
    pub fn new(w: &'a mut dyn Write) -> Self {
        Out { w, pos: 0 }
    }

    fn put(&mut self, b: &[u8]) -> io::Result<()> {
        self.w.write_all(b)?;
        self.pos = self.pos.saturating_add(len(b));
        Ok(())
    }

    fn text(&mut self, s: &str) -> io::Result<()> {
        self.put(s.as_bytes())
    }

    /// `n` copies of `byte`, in 64 KiB writes.
    fn fill(&mut self, byte: u8, n: u64) -> io::Result<()> {
        let chunk = [byte; 65536];
        let mut left = n;
        while left > 0 {
            let k = left.min(len(&chunk));
            self.put(
                chunk
                    .get(..usize::try_from(k).unwrap_or(0))
                    .unwrap_or_default(),
            )?;
            left -= k;
        }
        Ok(())
    }

    /// A classic xref table with one subsection from object 0 (free) and `offsets` for
    /// objects 1.., then the trailer and `startxref` (ISO 32000-2 §7.5.4, §7.5.5).
    fn xref(&mut self, offsets: &[u64], trailer_extra: &str) -> io::Result<()> {
        let at = self.pos;
        let size = offsets.len().saturating_add(1);
        self.text(&format!("xref\n0 {size}\n0000000000 65535 f\r\n"))?;
        for o in offsets {
            self.text(&format!("{o:010} 00000 n\r\n"))?;
        }
        self.text(&format!(
            "trailer\n<< /Size {size} /Root 1 0 R{trailer_extra} >>\nstartxref\n{at:010}\n%%EOF\n"
        ))
    }
}

fn len(b: &[u8]) -> u64 {
    u64::try_from(b.len()).unwrap_or(u64::MAX)
}

/// The header plus a comment line that pads the file so that `body` bytes follow and the
/// file is `target` bytes long (or as short as possible when `body` is already longer). The
/// engine finds objects through the xref, so it never lexes this comment; it only makes the
/// CLI read and hold a file of the target size.
fn padded_header(o: &mut Out, target: u64, body: u64) -> io::Result<()> {
    let head = b"%PDF-1.7\n%";
    let pad = target.saturating_sub(body).saturating_sub(len(head) + 1);
    o.put(head)?;
    o.fill(b'x', pad)?;
    o.put(b"\n")
}

/// Writes `body(base)` after a padded header, where `body` lays out its objects assuming it
/// starts at file offset `base`. Every offset is written with a fixed width, so the body's
/// length doesn't depend on `base`: it is built once at 0 to measure it, then at its place.
fn with_padding(o: &mut Out, target: u64, body: impl Fn(u64) -> Vec<u8>) -> io::Result<()> {
    let measured = len(&body(0));
    let start = o.pos;
    padded_header(o, target, measured)?;
    let base = o.pos.saturating_sub(start);
    o.put(&body(base))
}

/// Pushes `s` and returns the offset (`base` + position) it starts at.
fn push(v: &mut Vec<u8>, base: u64, s: &str) -> u64 {
    let at = base.saturating_add(len(v));
    v.extend_from_slice(s.as_bytes());
    at
}

/// A classic xref + trailer for a body built in memory that starts at file offset `base`
/// (see [`Out::xref`]).
fn push_xref(v: &mut Vec<u8>, base: u64, offsets: &[u64]) {
    let mut sink = Vec::new();
    let mut o = Out {
        w: &mut sink,
        pos: base.saturating_add(len(v)),
    };
    // Writing to a Vec can't fail.
    let _ = o.xref(offsets, "");
    v.extend_from_slice(&sink);
}

/// Class 1: each page object hides the next one inside a literal string (open.rs
/// `objects_nested_in_strings_are_rejected_as_overlapping`). Resolving every kid would rescan
/// the rest of the file; the second kid starts inside the first one's span. The page count
/// keeps the first kid's string under the 32 MiB string limit, so the overlap check is what
/// fires, not that limit.
fn nested_strings(o: &mut Out, target: u64) -> io::Result<()> {
    let pages = nested_pages(target);
    with_padding(o, target, |base| {
        let mut v = Vec::new();
        let mut offsets = vec![push(
            &mut v,
            base,
            "1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n",
        )];
        let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", i + 3)).collect();
        offsets.push(push(
            &mut v,
            base,
            &format!(
                "2 0 obj << /Type /Pages /Kids [{}] /Count {pages} >> endobj\n",
                kids.join(" ")
            ),
        ));
        for i in 0..pages {
            offsets.push(push(
                &mut v,
                base,
                &format!("{:07} 0 obj << /Type /Page /S (", i + 3),
            ));
        }
        for _ in 0..pages {
            v.extend_from_slice(b") >> endobj ");
        }
        v.push(b'\n');
        push_xref(&mut v, base, &offsets);
        v
    })
}

/// Class 2: the kid headers sit on one comment line, so every kid shares the one expensive
/// body that follows it (open.rs `overlapping_objects_sharing_one_body_are_rejected`). The
/// body holds a string just under the 32 MiB string limit.
fn shared_body(o: &mut Out, target: u64) -> io::Result<()> {
    let kids = shared_kids(target);
    let string = (target / 2).min(32 * MIB - 4096);
    with_padding(o, target, |base| {
        let mut v = Vec::new();
        let mut offsets = vec![push(
            &mut v,
            base,
            "1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n",
        )];
        let refs: Vec<String> = (0..kids).map(|i| format!("{} 0 R", i + 3)).collect();
        offsets.push(push(
            &mut v,
            base,
            &format!(
                "2 0 obj << /Type /Pages /Kids [{}] >> endobj\n%",
                refs.join(" ")
            ),
        ));
        for i in 0..kids {
            offsets.push(push(
                &mut v,
                base,
                &format!("{:07} 0 obj << /Type /Page %", i + 3),
            ));
        }
        v.extend_from_slice(b"\n/S (");
        v.resize(
            v.len().saturating_add(usize::try_from(string).unwrap_or(0)),
            b'a',
        );
        v.extend_from_slice(b") >> endobj\n");
        push_xref(&mut v, base, &offsets);
        v
    })
}

/// Class 3: stacked `/Prev` sections, each trailer's `/Prev` pointing at a section hidden
/// inside its own string (open.rs `overlapping_prev_sections_are_rejected`), 1000 sections
/// (under the 1024-section limit). The innermost string carries filler up to 30 MiB, below
/// the 32 MiB string limit.
fn stacked_prev(o: &mut Out, target: u64) -> io::Result<()> {
    let sections = STACKED_SECTIONS;
    let filler = (target / 2).min(30 * MIB);
    with_padding(o, target, |base| {
        let opening =
            |next: u64| format!("xref\n0 0\ntrailer << /Root 1 0 R /Prev {next:010} /S (");
        let step = len(opening(0).as_bytes());
        let mut v = Vec::new();
        for i in 0..sections {
            let next = if i + 1 < sections {
                base + (i + 1) * step
            } else {
                base
            };
            v.extend_from_slice(opening(next).as_bytes());
        }
        v.resize(
            v.len().saturating_add(usize::try_from(filler).unwrap_or(0)),
            b'x',
        );
        for _ in 0..sections {
            v.extend_from_slice(b") >> ");
        }
        v.extend_from_slice(format!("\nstartxref\n{base:010}\n%%EOF\n").as_bytes());
        v
    })
}

/// Class 4: one xref subsection with about `target / 20` entries (26 M at 500 MiB), far over
/// the 2^21-entry limit. Streamed: the entries are the file.
fn xref_entries(o: &mut Out, target: u64) -> io::Result<()> {
    o.text("%PDF-1.7\n")?;
    let catalog = o.pos;
    o.text("1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n")?;
    let pages = o.pos;
    o.text("2 0 obj << /Type /Pages /Kids [] /Count 0 >> endobj\n")?;
    let at = o.pos;
    let tail = 128;
    let entries = target.saturating_sub(at + tail) / 20;
    o.text(&format!("xref\n0 {entries:010}\n0000000000 65535 f\r\n"))?;
    o.text(&format!("{catalog:010} 00000 n\r\n{pages:010} 00000 n\r\n"))?;
    let free = b"0000000000 00000 f\r\n";
    let mut block = Vec::with_capacity(free.len() * 4096);
    for _ in 0..4096 {
        block.extend_from_slice(free);
    }
    let mut left = entries.saturating_sub(3);
    while left > 0 {
        let k = left.min(4096);
        let n = usize::try_from(k * 20).unwrap_or(0);
        o.put(block.get(..n).unwrap_or_default())?;
        left -= k;
    }
    o.text(&format!(
        "trailer\n<< /Size {entries} /Root 1 0 R >>\nstartxref\n{at}\n%%EOF\n"
    ))
}

/// The string of a `max-per-object` string page: just under the 32 MiB string limit.
const MAX_STRING: u64 = 32 * MIB - 4096;
/// The entries of a `max-per-object` dictionary page: 2 objects each, just under 2^20.
const MAX_ENTRIES: u64 = (1 << 19) - 1024;

/// The bytes of one `max-per-object` page pair's two payloads (the string and the dictionary
/// entries), a lower bound on the pair's size.
fn pair_bytes() -> u64 {
    let entries: u64 = (0..MAX_ENTRIES)
        .map(|k| len(format!(" /k{k} 0").as_bytes()))
        .sum();
    MAX_STRING + entries
}

/// `max-per-object` page pairs for `target`: as many whole pairs as fit, at least one. Each
/// pair is at least [`pair_bytes`], so the file reaches half the target (one pair when the
/// target is under two pairs, else ⌊T / pair⌋ pairs ≥ T − pair ≥ T/2).
fn max_pairs(target: u64) -> u64 {
    (target / pair_bytes()).max(1)
}

/// `nested-strings` pages: the first kid's string holds every later header and close (45
/// bytes a page), so 600 000 keeps it at 27 MB, under the 32 MiB string limit.
fn nested_pages(target: u64) -> u64 {
    (target / 256).clamp(2, 600_000)
}

/// `shared-body` kids.
fn shared_kids(target: u64) -> u64 {
    (target / 1024).clamp(2, 100_000)
}

/// `stacked-prev` sections: under the 1024-section limit.
const STACKED_SECTIONS: u64 = 1000;

/// `dense-kids` pages: the 1 000 000-page limit from 128 MiB up.
fn dense_pages(target: u64) -> u64 {
    (target / 128).clamp(1, 1_000_000)
}

/// Class 5: pages that each hold one per-parse maximum: a string just under the 32 MiB
/// string limit, or a dictionary of just under 2^19 entries (2^20 objects, the
/// objects-per-parse limit). A valid file: it must open, with every page resolved.
fn max_per_object(o: &mut Out, target: u64) -> io::Result<()> {
    let (string, entries) = (MAX_STRING, MAX_ENTRIES);
    let pages = max_pairs(target) * 2;
    o.text("%PDF-1.7\n")?;
    let mut offsets = vec![o.pos];
    o.text("1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n")?;
    offsets.push(o.pos);
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", i + 3)).collect();
    o.text(&format!(
        "2 0 obj << /Type /Pages /Kids [{}] /Count {pages} >> endobj\n",
        kids.join(" ")
    ))?;
    for i in 0..pages {
        offsets.push(o.pos);
        o.text(&format!("{} 0 obj << /Type /Page /Parent 2 0 R ", i + 3))?;
        if i % 2 == 0 {
            o.text("/S (")?;
            o.fill(b'a', string)?;
            o.text(")")?;
        } else {
            o.text("/D <<")?;
            for k in 0..entries {
                o.text(&format!(" /k{k} 0"))?;
            }
            o.text(" >>")?;
        }
        o.text(" >> endobj\n")?;
    }
    o.xref(&offsets, "")
}

/// Class 6: dense, non-overlapping kids: 1 000 000 pages (the page limit), each padded with
/// a string so the file reaches the target size. A valid file: it must open.
fn dense_kids(o: &mut Out, target: u64) -> io::Result<()> {
    let pages = dense_pages(target);
    // Per page: the kid reference, the object and its xref entry, at most 100 bytes.
    let pad = (target / pages).saturating_sub(100).max(1);
    o.text("%PDF-1.7\n")?;
    let mut offsets = Vec::with_capacity(usize::try_from(pages + 2).unwrap_or(0));
    offsets.push(o.pos);
    o.text("1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n")?;
    offsets.push(o.pos);
    o.text("2 0 obj << /Type /Pages /Kids [")?;
    for i in 0..pages {
        o.text(&format!("{} 0 R ", i + 3))?;
    }
    o.text(&format!("] /Count {pages} >> endobj\n"))?;
    let filler = vec![b'p'; usize::try_from(pad).unwrap_or(1)];
    for i in 0..pages {
        offsets.push(o.pos);
        o.text(&format!(
            "{} 0 obj << /Type /Page /Parent 2 0 R /P (",
            i + 3
        ))?;
        o.put(&filler)?;
        o.text(") >> endobj\n")?;
    }
    o.xref(&offsets, "")
}

/// Class 7: one comment line of almost the whole file inside the catalog dictionary, which
/// the lexer must skip to reach `/Pages`. A valid file: it must open.
fn long_comment(o: &mut Out, target: u64) -> io::Result<()> {
    o.text("%PDF-1.7\n")?;
    let catalog = o.pos;
    o.text("1 0 obj << /Type /Catalog %")?;
    let tail = 512;
    let comment = target.saturating_sub(o.pos + tail);
    o.fill(b'c', comment)?;
    o.text("\n/Pages 2 0 R >> endobj\n")?;
    let pages = o.pos;
    o.text("2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n")?;
    let page = o.pos;
    o.text("3 0 obj << /Type /Page /Parent 2 0 R >> endobj\n")?;
    o.xref(&[catalog, pages, page], "")
}

/// Smallest `--mib` (spec item 1): every class still reaches half its target.
const MIN_MIB: u64 = 64;

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub out: PathBuf,
    pub mib: u64,
}

/// Parses the arguments after `limits` (spec item 1).
pub fn parse(args: &[String]) -> Result<Options, String> {
    let (mut out, mut mib) = (None, None);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let v = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        let dup = match flag.as_str() {
            "--out" => out.replace(PathBuf::from(v)).is_some(),
            "--mib" => mib
                .replace(crate::nightly::number(flag, v, MIN_MIB, MAX_MIB)?)
                .is_some(),
            other => return Err(format!("unknown argument {other:?}")),
        };
        if dup {
            return Err(format!("{flag} given twice"));
        }
    }
    Ok(Options {
        out: out.unwrap_or_else(|| PathBuf::from("target/xtask-out")),
        mib: mib.unwrap_or(DEFAULT_MIB),
    })
}

/// How one case ended (spec item 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResult {
    pub id: &'static str,
    pub bytes: u64,
    /// `pass`, `crash`, `timeout`, `wrong_exit` or `wrong_error`.
    pub status: &'static str,
    pub exit: Option<i32>,
    pub signal: Option<i32>,
    pub ms: Option<u128>,
    /// Kept for every status but `pass`.
    pub stderr: Option<String>,
}

/// The verdict for one run (spec item 5). `None`: `jpdf-cli` could not be started.
fn verdict(expect: Expect, o: Option<&Outcome>) -> &'static str {
    let Some(o) = o else { return "crash" };
    match (o.status, expect) {
        (Status::Crash, _) => "crash",
        (Status::Timeout, _) => "timeout",
        (Status::Ok, Expect::Opens) => "pass",
        (Status::Error, Expect::Refused(text)) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            if stderr.contains(&format!("jpdf-cli: {text}")) {
                "pass"
            } else {
                "wrong_error"
            }
        }
        _ => "wrong_exit",
    }
}

/// Builds each case's file at `file` (removed afterwards), checks its size, and runs
/// `exec(file)` once (spec items 2–5). `Err`: a file could not be written or missed its size,
/// a tool failure.
pub fn run_cases(
    target: u64,
    file: &Path,
    exec: &mut dyn FnMut(&Path) -> Option<Outcome>,
) -> Result<Vec<CaseResult>, String> {
    let mut results = Vec::with_capacity(CASES.len());
    for case in CASES {
        let bytes = generate(case, target, file);
        let result = bytes.and_then(|bytes| {
            if !size_ok(bytes, target) {
                return Err(format!(
                    "{bytes} bytes, outside {}..={}",
                    target / 2,
                    512 * MIB
                ));
            }
            let o = exec(file);
            let status = verdict(case.expect, o.as_ref());
            Ok(CaseResult {
                id: case.id,
                bytes,
                status,
                exit: o.as_ref().and_then(|o| o.code),
                signal: o.as_ref().and_then(|o| o.signal),
                ms: o.as_ref().map(|o| o.took.as_millis()),
                stderr: (status != "pass").then(|| {
                    o.as_ref().map_or("cannot start jpdf-cli".into(), |o| {
                        String::from_utf8_lossy(&o.stderr).into_owned()
                    })
                }),
            })
        });
        let _ = std::fs::remove_file(file);
        results.push(result.map_err(|e| format!("case {}: {e}", case.id))?);
    }
    Ok(results)
}

/// A case's file must reach half the target, so a generator bug can't pass with a small file,
/// and must not pass `jpdf-cli`'s 512 MiB `max_file_size`, which would refuse it unread
/// (spec item 3).
fn size_ok(bytes: u64, target: u64) -> bool {
    (target / 2..=512 * MIB).contains(&bytes)
}

/// Writes one case's file; returns its size.
fn generate(case: Case, target: u64, file: &Path) -> Result<u64, String> {
    let shown = |e: io::Error| format!("cannot write {}: {e}", file.display());
    let f = std::fs::File::create(file).map_err(shown)?;
    let mut w = BufWriter::with_capacity(1 << 20, f);
    let mut o = Out::new(&mut w);
    (case.generate)(&mut o, target).map_err(shown)?;
    let pos = o.pos;
    w.flush().map_err(shown)?;
    let size = std::fs::metadata(file).map_err(shown)?.len();
    if size != pos {
        return Err(format!("wrote {pos} bytes but the file has {size}"));
    }
    Ok(size)
}

/// `limits.json` (spec item 6).
pub fn results_json(results: &[CaseResult], target: u64, mem: bool) -> String {
    let failed = results.iter().filter(|r| r.status != "pass").count();
    let mut s = format!(
        "{{\"schema\":1,\"target_bytes\":{target},\"memory_limit_kib\":{},\"time_limit_ms\":{},\"failed\":{failed},\"cases\":[",
        if mem {
            crate::nightly::MEM_KIB.to_string()
        } else {
            "null".into()
        },
        INFO_LIMIT.as_millis()
    );
    for (i, r) in results.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let opt = |v: Option<String>| v.unwrap_or_else(|| "null".into());
        s.push_str(&format!(
            "{{\"id\":\"{}\",\"bytes\":{},\"status\":\"{}\",\"exit\":{},\"signal\":{},\"ms\":{}",
            r.id,
            r.bytes,
            r.status,
            opt(r.exit.map(|x| x.to_string())),
            opt(r.signal.map(|x| x.to_string())),
            opt(r.ms.map(|x| x.to_string())),
        ));
        if let Some(e) = &r.stderr {
            s.push_str(",\"stderr\":");
            jpdf_bench::json::string(&mut s, e);
        }
        s.push('}');
    }
    s.push_str("]}\n");
    s
}

/// Runs the cases with `cli` and writes `DIR/limits.json` (spec items 2–6). `Ok(true)`: every
/// case passed. The caller has started the watchdog and decided `mem`.
pub fn run_with(cli: &Path, mem: bool, out: &Path, target: u64) -> Result<bool, String> {
    run_results(cli, mem, out, target).map(|r| r.iter().all(|c| c.status == "pass"))
}

/// [`run_with`], returning every case's result (the nightly files issues from them,
/// specs/tooling/nightly.md item 16).
pub fn run_results(
    cli: &Path,
    mem: bool,
    out: &Path,
    target: u64,
) -> Result<Vec<CaseResult>, String> {
    let work = out.join("work");
    std::fs::create_dir_all(&work).map_err(|e| format!("cannot write {}: {e}", work.display()))?;
    let file = std::path::absolute(work.join("limits.pdf")).map_err(|e| e.to_string())?;
    let results = run_cases(target, &file, &mut |f| {
        crate::nightly::exec(cli, mem, &["info".into(), f.into()], INFO_LIMIT)
    })?;
    for r in &results {
        eprintln!(
            "xtask: limits: {}: {} ({} bytes, {} ms)",
            r.id,
            r.status,
            r.bytes,
            r.ms.map_or("-".into(), |m| m.to_string())
        );
    }
    let path = out.join("limits.json");
    crate::nightly::write(&path, &results_json(&results, target, mem))?;
    let failed = results.iter().filter(|r| r.status != "pass").count();
    eprintln!(
        "xtask: limits: {failed} of {} cases failed; wrote {}",
        results.len(),
        path.display()
    );
    Ok(results)
}

/// `cargo xtask limits` (spec item 7): 0 when every case passed, 4 when one failed;
/// `Err` is a tool failure (exit 1).
pub fn run(root: &Path, opts: &Options) -> Result<u8, String> {
    let cli = std::path::absolute(crate::nightly::build_cli(root)?).map_err(|e| e.to_string())?;
    if !crate::nightly::is_executable_file(&cli) {
        return Err(format!("no executable jpdf-cli at {}", cli.display()));
    }
    let mem = crate::nightly::memory_limit_allowed(
        crate::nightly::memory_limit_applies(),
        cfg!(target_os = "linux"),
    )?;
    jpdf_bench::engine::start_watchdog()?;
    let passed = run_with(&cli, mem, &opts.out, opts.mib.saturating_mul(MIB))?;
    Ok(if passed { 0 } else { 4 })
}

#[cfg(test)]
mod tests;
