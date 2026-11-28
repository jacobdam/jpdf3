//! `Document::open` on generated files. Spec: specs/cli/jpdf-cli.md.

use jpdf_engine::{Document, EngineError, Limits};
use jpdf_testgen::{Revision, minimal_pdf, page_tree, write_revision};

/// Writes `objects` (numbered from `first_num`) followed by a classic xref table and trailer,
/// through `jpdf_testgen::write_revision`. Returns the file and the xref offset.
fn build<T: AsRef<[u8]>>(
    prefix: &[u8],
    first_num: usize,
    objects: &[T],
    trailer_extra: &str,
) -> (Vec<u8>, usize) {
    let mut out = prefix.to_vec();
    let bodies: Vec<&[u8]> = objects.iter().map(AsRef::as_ref).collect();
    let xref_at = write_revision(
        &mut out,
        &Revision {
            first_num,
            objects: &bodies,
            size: first_num + objects.len(),
            prev: None,
            trailer_extra: trailer_extra.as_bytes(),
        },
    );
    (out, xref_at)
}

fn open(bytes: &[u8]) -> Result<usize, EngineError> {
    Document::open(bytes, &Limits::default()).map(|d| d.page_count())
}

#[test]
fn counts_pages_of_minimal_files() {
    for n in [0, 1, 3, 50] {
        assert_eq!(open(&minimal_pdf(n)), Ok(n), "{n} pages");
    }
}

#[test]
fn follows_prev_into_older_sections() {
    let (mut file, first_xref) = build(b"%PDF-1.7\n", 1, &page_tree(1), "");
    // Incremental update (ISO 32000-2 §7.5.6): redefine object 2 with two kids, add page 4.
    let update = [
        (2, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>"),
        (4, "<< /Type /Page /Parent 2 0 R >>"),
    ];
    let mut tail: Vec<u8> = Vec::new();
    let mut offsets = Vec::new();
    for (num, body) in update {
        offsets.push((num, file.len() + tail.len()));
        tail.extend(format!("{num} 0 obj\n{body}\nendobj\n").as_bytes());
    }
    let xref_at = file.len() + tail.len();
    tail.extend(b"xref\n");
    for (num, o) in offsets {
        tail.extend(format!("{num} 1\n{o:010} 00000 n\r\n").as_bytes());
    }
    tail.extend(
        format!(
            "trailer\n<< /Size 5 /Root 1 0 R /Prev {first_xref} >>\nstartxref\n{xref_at}\n%%EOF\n"
        )
        .as_bytes(),
    );
    file.extend(tail);
    assert_eq!(open(&file), Ok(2));
}

#[test]
fn prev_loop_is_malformed() {
    // The xref offset does not depend on the trailer, so build once to learn it, then point
    // /Prev back at the same section.
    let (_, xref_at) = build(b"%PDF-1.7\n", 1, &page_tree(1), "");
    let (looped, _) = build(b"%PDF-1.7\n", 1, &page_tree(1), &format!("/Prev {xref_at}"));
    assert_eq!(
        open(&looped),
        Err(EngineError::Malformed {
            what: "/Prev loop in cross-reference chain",
            offset: Some(xref_at),
        })
    );
}

#[test]
fn unsupported_structures_are_reported_as_unsupported() {
    let (enc, _) = build(b"%PDF-1.7\n", 1, &page_tree(1), "/Encrypt 9 0 R");
    assert_eq!(
        open(&enc),
        Err(EngineError::Unsupported("encrypted documents"))
    );

    let nested = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Pages /Parent 2 0 R /Kids [4 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 3 0 R >>".to_string(),
    ];
    let (nested, _) = build(b"%PDF-1.7\n", 1, &nested, "");
    assert_eq!(
        open(&nested),
        Err(EngineError::Unsupported("nested page trees"))
    );
}

#[test]
fn inconsistent_page_trees_are_malformed() {
    let duplicate_kid = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R 3 0 R] /Count 2 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R >>".to_string(),
    ];
    let self_kid = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [2 0 R] /Count 1 >>".to_string(),
    ];
    let missing_kid = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [7 0 R] /Count 1 >>".to_string(),
    ];
    for objs in [&duplicate_kid[..], &self_kid[..], &missing_kid[..]] {
        let (pdf, _) = build(b"%PDF-1.7\n", 1, objs, "");
        assert!(
            matches!(open(&pdf), Err(EngineError::Malformed { .. })),
            "{objs:?}"
        );
    }
}

#[test]
fn wrong_count_is_read_leniently() {
    // ISO 32000-2 §7.7.3.2 says /Count must match; real files get it wrong (CONSTITUTION 17).
    let wrong_count = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 2 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R >>".to_string(),
    ];
    let (pdf, _) = build(b"%PDF-1.7\n", 1, &wrong_count, "");
    assert_eq!(open(&pdf), Ok(1));
}

#[test]
fn deep_nesting_and_huge_numbers_in_objects() {
    let deep = format!(
        "<< /Type /Catalog /Pages 2 0 R /X {}{} >>",
        "[".repeat(300),
        "]".repeat(300)
    );
    let objs = [deep, "<< /Type /Pages /Kids [] /Count 0 >>".to_string()];
    let (pdf, _) = build(b"%PDF-1.7\n", 1, &objs, "");
    assert_eq!(open(&pdf), Err(EngineError::LimitExceeded("nesting depth")));

    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [] /Count 99999999999999999999 >>".to_string(),
    ];
    let (pdf, _) = build(b"%PDF-1.7\n", 1, &objs, "");
    assert!(matches!(open(&pdf), Err(EngineError::Malformed { .. })));
}

/// Each page object hides the next one inside a literal string, so resolving every kid would
/// rescan the rest of the file (quadratic). The second kid starts inside the first one's span,
/// so it is rejected as overlapping before any rescan.
#[test]
fn objects_nested_in_strings_are_rejected_as_overlapping() {
    let pages = 2000;
    let header = b"%PDF-1.7\n".to_vec();
    let catalog = "1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n";
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", i + 3)).collect();
    let tree = format!(
        "2 0 obj << /Type /Pages /Kids [{}] /Count {pages} >> endobj\n",
        kids.join(" ")
    );
    let mut file = header;
    let mut offsets = vec![file.len()];
    file.extend(catalog.as_bytes());
    offsets.push(file.len());
    file.extend(tree.as_bytes());
    for i in 0..pages {
        offsets.push(file.len());
        file.extend(format!("{} 0 obj << /Type /Page /S (", i + 3).as_bytes());
    }
    for _ in 0..pages {
        file.extend(b") >> endobj ");
    }
    let xref_at = file.len();
    file.extend(format!("\nxref\n0 {}\n0000000000 65535 f\r\n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        file.extend(format!("{o:010} 00000 n\r\n").as_bytes());
    }
    file.extend(
        format!(
            "trailer << /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            offsets.len() + 1,
            xref_at + 1
        )
        .as_bytes(),
    );
    assert_eq!(
        open(&file),
        Err(EngineError::Malformed {
            what: "indirect objects overlap",
            offset: Some(offsets[3]),
        })
    );
}

/// Each trailer's /Prev points to a section hidden inside its own string, so every section
/// would rescan the rest of the file. The second section starts inside the first one's span.
#[test]
fn overlapping_prev_sections_are_rejected() {
    let sections = 1000;
    let mut file = b"%PDF-1.7\n".to_vec();
    let start = file.len();
    let mut offsets = Vec::new();
    let mut at = start;
    let opening = |next: usize| format!("xref\n0 0\ntrailer << /Root 1 0 R /Prev {next:010} /S (");
    for _ in 0..sections {
        offsets.push(at);
        at += opening(0).len();
    }
    for (i, _) in offsets.iter().enumerate() {
        let next = offsets.get(i + 1).copied().unwrap_or(start);
        file.extend(opening(next).as_bytes());
    }
    file.extend(vec![b'x'; 200_000]);
    for _ in 0..sections {
        file.extend(b") >> ");
    }
    file.extend(format!("\nstartxref\n{start}\n%%EOF\n").as_bytes());
    assert_eq!(
        open(&file),
        Err(EngineError::Malformed {
            what: "cross-reference sections overlap",
            offset: Some(offsets[1]),
        })
    );
}

#[test]
fn file_size_limit_is_enforced() {
    let pdf = minimal_pdf(1);
    let limits = Limits {
        max_file_size: pdf.len() - 1,
        ..Limits::default()
    };
    assert_eq!(
        Document::open(&pdf, &limits).map(|d| d.page_count()),
        Err(EngineError::LimitExceeded("file size"))
    );
}

/// At the page limit the document opens; `page_limit_is_enforced` is the one-past side.
#[test]
fn page_limit_boundary_opens_at_the_limit() {
    let limits = Limits {
        max_pages: 5,
        ..Limits::default()
    };
    assert_eq!(
        Document::open(&minimal_pdf(5), &limits).map(|d| d.page_count()),
        Ok(5)
    );
}

#[test]
fn page_limit_is_enforced() {
    let pdf = minimal_pdf(5);
    let limits = Limits {
        max_pages: 4,
        ..Limits::default()
    };
    assert_eq!(
        Document::open(&pdf, &limits).map(|d| d.page_count()),
        Err(EngineError::LimitExceeded("pages"))
    );
}

/// A catalog holding `arrays` nested arrays.
fn nested_pdf(arrays: usize) -> Vec<u8> {
    let deep = format!(
        "<< /Type /Catalog /Pages 2 0 R /X {}{} >>",
        "[".repeat(arrays),
        "]".repeat(arrays)
    );
    let objs = [deep, "<< /Type /Pages /Kids [] /Count 0 >>".to_string()];
    build(b"%PDF-1.7\n", 1, &objs, "").0
}

/// Limits may come from Swift or JS. A depth of 10^7 must not reach the recursive parser: on
/// the old code 100,000 nested arrays overflowed the stack and aborted the test process.
#[test]
fn caller_limits_cannot_loosen_the_defaults() {
    let loose = Limits {
        max_nesting_depth: 10_000_000,
        max_objects_per_parse: usize::MAX,
        lex_budget_factor: usize::MAX,
        ..Limits::default()
    };
    // The document keeps the clamped limits for later operations.
    let doc = Document::open(&minimal_pdf(1), &loose).map_err(|e| e.to_string());
    assert_eq!(doc.map(|d| d.limits().clone()), Ok(Limits::default()));
    assert_eq!(
        Document::open(&nested_pdf(100_000), &loose).map(|d| d.page_count()),
        Err(EngineError::LimitExceeded("nesting depth"))
    );
    // Exact boundary with the loose limits: the catalog dictionary is level 1, so 255 arrays
    // reach the default depth of 256 and open; one more is refused.
    let default_depth = Limits::default().max_nesting_depth;
    assert_eq!(
        Document::open(&nested_pdf(default_depth - 1), &loose).map(|d| d.page_count()),
        Ok(0)
    );
    assert_eq!(
        Document::open(&nested_pdf(default_depth), &loose).map(|d| d.page_count()),
        Err(EngineError::LimitExceeded("nesting depth"))
    );
}

/// The base file plus `updates` incremental sections, each re-pointing object 1 at the original
/// catalog, chained by `/Prev`: `updates + 1` cross-reference sections in total.
fn prev_chain_pdf(updates: usize) -> Vec<u8> {
    let prefix = b"%PDF-1.7\n";
    let (mut file, mut prev) = build(prefix, 1, &page_tree(1), "");
    for _ in 0..updates {
        let xref_at = file.len();
        file.extend(
            format!(
                "xref\n1 1\n{:010} 00000 n\r\ntrailer\n<< /Size 4 /Root 1 0 R /Prev {prev} >>\nstartxref\n{xref_at}\n%%EOF\n",
                prefix.len()
            )
            .as_bytes(),
        );
        prev = xref_at;
    }
    file
}

/// A second clamped field, end to end: the caller's `max_xref_sections` can't exceed the
/// default either, so `open` passes the defaults as the ceiling for more than the depth.
#[test]
fn caller_xref_section_limit_is_clamped() {
    let loose = Limits {
        max_xref_sections: usize::MAX,
        ..Limits::default()
    };
    let max = Limits::default().max_xref_sections;
    assert_eq!(
        Document::open(&prev_chain_pdf(max - 1), &loose).map(|d| d.page_count()),
        Ok(1)
    );
    assert_eq!(
        Document::open(&prev_chain_pdf(max), &loose).map(|d| d.page_count()),
        Err(EngineError::LimitExceeded("cross-reference sections"))
    );
}

/// Clamping keeps tighter caller limits: a depth below the default still binds.
#[test]
fn caller_limits_can_tighten_the_defaults() {
    let tight = Limits {
        max_nesting_depth: 11,
        ..Limits::default()
    };
    assert_eq!(
        Document::open(&nested_pdf(10), &tight).map(|d| d.page_count()),
        Ok(0)
    );
    // The document keeps the tighter limits, not the defaults.
    let kept = Document::open(&nested_pdf(10), &tight).map(|d| d.limits().clone());
    assert_eq!(kept, Ok(tight.clone()));
    assert_eq!(
        Document::open(&nested_pdf(11), &tight).map(|d| d.page_count()),
        Err(EngineError::LimitExceeded("nesting depth"))
    );
}

#[test]
fn empty_and_garbage_input_are_errors() {
    assert!(open(b"").is_err());
    assert!(open(b"%PDF-1.7\n").is_err());
    assert!(open(&[0xFF; 4096]).is_err());
    assert!(open(b"startxref\n0\n%%EOF").is_err());
}

/// Deterministic xorshift64* generator (no `proptest` admitted yet; see #16).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).unwrap_or(u64::MAX)).unwrap_or(0)
    }
}

const TOKENS: &[&[u8]] = &[
    b"<<",
    b">>",
    b"[",
    b"]",
    b"(",
    b")",
    b"<",
    b">",
    b"/",
    b"%",
    b"\\",
    b"#",
    b" ",
    b"\n",
    b"\r",
    b"0",
    b"1",
    b"9",
    b"-",
    b".",
    b"R",
    b"obj",
    b"endobj",
    b"xref",
    b"trailer",
    b"startxref",
    b"/Root",
    b"/Pages",
    b"/Kids",
    b"/Count",
    b"/Type",
    b"/Page",
    b"/Prev",
    b"0000000000",
    b"n",
    b"f",
];

#[test]
fn random_and_mutated_input_never_panics() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    // Random token soup ending in a plausible startxref.
    for _ in 0..3000 {
        let mut buf = Vec::new();
        for _ in 0..rng.below(200) {
            buf.extend(TOKENS[rng.below(TOKENS.len())]);
        }
        buf.extend(format!("startxref\n{}\n%%EOF", rng.below(buf.len() + 2)).as_bytes());
        let _ = open(&buf);
    }
    // Raw random bytes.
    for _ in 0..1000 {
        let buf: Vec<u8> = (0..rng.below(2048))
            .map(|_| u8::try_from(rng.next() >> 56).unwrap_or(0))
            .collect();
        let _ = open(&buf);
    }
    // Every truncation and many mutations of a valid file.
    let valid = minimal_pdf(3);
    for cut in 0..valid.len() {
        let _ = open(&valid[..cut]);
    }
    for _ in 0..5000 {
        let mut buf = valid.clone();
        for _ in 0..=rng.below(8) {
            let i = rng.below(buf.len());
            match rng.below(3) {
                0 => buf[i] ^= 1 << rng.below(8),
                1 => buf[i] = TOKENS[rng.below(TOKENS.len())][0],
                _ => {
                    let tok = TOKENS[rng.below(TOKENS.len())];
                    buf.splice(i..i, tok.iter().copied());
                }
            }
        }
        let _ = open(&buf);
    }
}

/// A large legitimate file (no overlapping objects) stays inside the default `4 × size`
/// budget. Reference look-ahead re-reads up to two tokens after each integer, so integer-heavy
/// arrays cost up to about 3× their size: they must fit in 3× with no slack.
#[test]
fn large_valid_files_fit_the_lexing_budget() {
    assert_eq!(open(&minimal_pdf(100_000)), Ok(100_000));
    let numbers: Vec<String> = (0..200_000).map(|i| i.to_string()).collect();
    let objs = [
        format!(
            "<< /Type /Catalog /Pages 2 0 R /Big [{}] >>",
            numbers.join(" ")
        ),
        "<< /Type /Pages /Kids [] /Count 0 >>".to_string(),
    ];
    let (pdf, _) = build(b"%PDF-1.7\n", 1, &objs, "");
    let tight = Limits {
        lex_budget_factor: 3,
        lex_budget_slack: 0,
        ..Limits::default()
    };
    assert_eq!(Document::open(&pdf, &tight).map(|d| d.page_count()), Ok(0));
}

/// Kid headers hidden on one comment line all share the body that follows, so every kid would
/// re-parse the same expensive dictionary. Overlapping objects are rejected instead.
#[test]
fn overlapping_objects_sharing_one_body_are_rejected() {
    let kids = 50;
    let mut file = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![file.len()];
    file.extend(b"1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n");
    offsets.push(file.len());
    let refs: Vec<String> = (0..kids).map(|i| format!("{} 0 R", i + 3)).collect();
    file.extend(
        format!(
            "2 0 obj << /Type /Pages /Kids [{}] >> endobj\n%",
            refs.join(" ")
        )
        .as_bytes(),
    );
    for i in 0..kids {
        offsets.push(file.len());
        file.extend(format!("{} 0 obj << /Type /Page %", i + 3).as_bytes());
    }
    file.extend(b"\n");
    for _ in 0..10_000 {
        file.extend(b"/a 1 ");
    }
    file.extend(b">> endobj\n");
    let xref_at = file.len();
    file.extend(format!("xref\n0 {}\n0000000000 65535 f\r\n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        file.extend(format!("{o:010} 00000 n\r\n").as_bytes());
    }
    file.extend(
        format!(
            "trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    assert_eq!(
        open(&file),
        Err(EngineError::Malformed {
            what: "indirect objects overlap",
            offset: Some(offsets[3]),
        })
    );
}

#[test]
fn out_of_range_xref_offset_reads_as_null() {
    let mut bad = minimal_pdf(1);
    // Point object 3 (the only page) at an offset beyond u32: the kid resolves to null.
    let marker = b" 00000 n\r\n";
    let entry_at = bad
        .windows(marker.len())
        .rposition(|w| w == marker)
        .unwrap()
        - 10;
    bad[entry_at..entry_at + 10].copy_from_slice(b"9999999999");
    // The xref still parses; only the page itself is missing.
    assert_eq!(
        open(&bad),
        Err(EngineError::Malformed {
            what: "page tree kid is not a dictionary",
            offset: None,
        })
    );
}

/// #78: an incremental update whose trailer omits `/Root` (§7.5.6 says it should repeat it; real
/// writers don't) takes it from the older trailer.
#[test]
fn update_trailer_without_root_uses_the_older_root() {
    let (mut file, first_xref) = build(b"%PDF-1.7\n", 1, &page_tree(1), "");
    let update = |file: &[u8], trailer: &str| {
        let mut out = file.to_vec();
        let page_at = out.len();
        out.extend(b"4 0 obj\n<< /Type /Page /Parent 2 0 R >>\nendobj\n");
        let pages_at = out.len();
        out.extend(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>\nendobj\n");
        let xref_at = out.len();
        out.extend(
            format!("xref\n2 1\n{pages_at:010} 00000 n\r\n4 1\n{page_at:010} 00000 n\r\n")
                .as_bytes(),
        );
        out.extend(
            format!("trailer\n<< {trailer} /Prev {first_xref} >>\nstartxref\n{xref_at}\n%%EOF\n")
                .as_bytes(),
        );
        out
    };
    assert_eq!(open(&update(&file, "/Size 5")), Ok(2));
    assert_eq!(open(&update(&file, "")), Ok(2));
    assert_eq!(open(&update(&file, "/Size 5 /Root 9")), Ok(2));
    // Near miss: no trailer in the chain has a usable /Root.
    let (no_root, _) = build(b"%PDF-1.7\n", 1, &page_tree(1), "");
    let no_root = String::from_utf8_lossy(&no_root).replace("/Root 1 0 R", "/Root 1    ");
    file = no_root.into_bytes();
    assert_eq!(
        open(&update(&file, "/Size 5")),
        Err(EngineError::Malformed {
            what: "trailer /Root is not a reference",
            offset: None,
        })
    );
}

/// #78: a page tree kid without a `/Type` name is a page unless it has `/Kids`; a wrong name is
/// still malformed.
#[test]
fn kid_without_type_is_inferred_from_its_shape() {
    let with_kid = |kid: &str| {
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            kid.to_string(),
            "<< /Type /Page /Parent 3 0 R >>".to_string(),
        ];
        open(&build(b"%PDF-1.7\n", 1, &objs, "").0)
    };
    assert_eq!(
        with_kid("<< /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
        Ok(1)
    );
    assert_eq!(with_kid("<< /Type 7 /Parent 2 0 R >>"), Ok(1));
    assert_eq!(with_kid("<< /Type (Page) /Parent 2 0 R >>"), Ok(1));
    assert_eq!(with_kid("<< /Type /Page /Kids [4 0 R] >>"), Ok(1));
    assert_eq!(with_kid("<< /Parent 2 0 R /Kids null >>"), Ok(1));
    assert_eq!(
        with_kid("<< /Parent 2 0 R /Kids [4 0 R] /Count 1 >>"),
        Err(EngineError::Unsupported("nested page trees"))
    );
    assert_eq!(
        with_kid("<< /Parent 2 0 R /Kids 5 >>"),
        Err(EngineError::Unsupported("nested page trees"))
    );
    for wrong in [
        "<< /Type /Catalog >>",
        "<< /Type /Foo /Parent 2 0 R >>",
        "<< /Type /page >>",
    ] {
        assert_eq!(
            with_kid(wrong),
            Err(EngineError::Malformed {
                what: "page tree kid has no valid /Type",
                offset: None,
            }),
            "{wrong}"
        );
    }
}

/// #78: the only page's xref offset in `u32` but at or past the end of the file reads as free,
/// so the kid is null; one byte before the end it is in use and its header is checked.
#[test]
fn xref_offset_past_end_of_file_reads_as_null() {
    let good = minimal_pdf(1);
    let marker = b" 00000 n\r\n";
    let entry_at = good
        .windows(marker.len())
        .rposition(|w| w == marker)
        .unwrap()
        - 10;
    let pointing_at = |offset: usize| {
        let mut bad = good.clone();
        bad[entry_at..entry_at + 10].copy_from_slice(format!("{offset:010}").as_bytes());
        open(&bad)
    };
    let len = good.len();
    let null_kid = Err(EngineError::Malformed {
        what: "page tree kid is not a dictionary",
        offset: None,
    });
    assert_eq!(pointing_at(len), null_kid);
    assert_eq!(pointing_at(len + 1), null_kid);
    assert_eq!(pointing_at(4_294_967_295), null_kid);
    assert_eq!(
        pointing_at(len - 1),
        Err(EngineError::Malformed {
            what: "object header does not match xref",
            offset: Some(len - 1),
        })
    );
}

/// #78: `startxref` at an integer that starts no cross-reference stream is malformed (exit 2),
/// not unsupported (exit 3).
#[test]
fn startxref_at_a_non_xref_object_is_malformed() {
    let (pdf, xref_at) = build(b"%PDF-1.7\n", 1, &page_tree(1), "");
    let tail = format!("startxref\n{xref_at}\n");
    let at_catalog = String::from_utf8_lossy(&pdf).replace(&tail, "startxref\n9\n");
    assert_eq!(
        open(at_catalog.as_bytes()),
        Err(EngineError::Malformed {
            what: "expected 'xref'",
            offset: Some(9),
        })
    );
    let mut stream = b"%PDF-1.7\n".to_vec();
    stream.extend(
        b"1 0 obj\n<< /Type /XRef /Size 2 /W [1 2 1] /Length 0 >>\nstream\n\nendstream\nendobj\n",
    );
    stream.extend(b"startxref\n9\n%%EOF\n");
    assert_eq!(
        open(&stream),
        Err(EngineError::Unsupported("cross-reference streams"))
    );
}

/// #78: a hybrid file (§7.5.8.4) is read through its classic table; `/XRefStm` is ignored, so
/// the stream can neither override a table entry nor fill a free one.
#[test]
fn hybrid_file_is_read_through_its_table() {
    // Object 5 is a real cross-reference stream (§7.5.8), `/W [1 1 1]`, `/Index [3 2]`: object 3
    // free (type 0) and object 4 compressed in object stream 6 at index 0 (type 2). Object 6 is
    // that object stream (§7.5.7), holding `compressed` as object 4. `/XRefStm` points at object
    // 5 (its offset doesn't depend on the trailer).
    let hybrid = |compressed: &str| {
        let body = format!("4 0 {compressed}");
        let mut objs = page_tree(2);
        objs.push(
            b"<< /Type /XRef /Size 7 /Index [3 2] /W [1 1 1] /Length 6 >>\nstream\n\x00\x00\x00\x02\x06\x00\nendstream"
                .to_vec(),
        );
        objs.push(
            format!(
                "<< /Type /ObjStm /N 1 /First 4 /Length {} >>\nstream\n{body}\nendstream",
                body.len()
            )
            .into_bytes(),
        );
        let (probe, _) = build(b"%PDF-1.7\n", 1, &objs, "");
        let stream_at = probe.windows(8).position(|w| w == b"5 0 obj\n").unwrap();
        let extra = format!("/XRefStm {stream_at}");
        let (pdf, _) = build(b"%PDF-1.7\n", 1, &objs, &extra);
        assert_eq!(pdf.len(), probe.len() + extra.len());
        pdf
    };
    // The table lists both pages in use, and the stream would make object 3 free and object 4 an
    // intermediate node: a reader that let either stream entry override the table would lose
    // page 3 or report nested page trees.
    assert_eq!(open(&hybrid("<< /Type /Pages /Kids [] /Count 0 >>")), Ok(2));
    // Object 4's table record replaced by a complete free record (next free object 0,
    // generation 1), with a complete compressed page in the stream: a reader that filled free
    // table slots from the stream would count 2 pages. The kid is null instead.
    let pdf = hybrid("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> >>");
    let text = String::from_utf8_lossy(&pdf).into_owned();
    let entries: Vec<usize> = text.match_indices(" 00000 n\r\n").map(|(i, _)| i).collect();
    assert_eq!(entries.len(), 6);
    let record = entries[3] - 10;
    let mut only_in_stream = pdf.clone();
    only_in_stream[record..record + 18].copy_from_slice(b"0000000000 00001 f");
    assert_eq!(
        open(&only_in_stream),
        Err(EngineError::Malformed {
            what: "page tree kid is not a dictionary",
            offset: None,
        })
    );
}
