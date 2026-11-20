//! `Document::open` on generated files. Spec: specs/cli/jpdf-cli.md.

use jpdf_engine::{Document, EngineError, Limits};

/// Writes `objects` (numbered from 1) followed by a classic xref table and trailer.
/// Returns the file and the xref offset.
fn build(
    prefix: &[u8],
    first_num: usize,
    objects: &[String],
    trailer_extra: &str,
) -> (Vec<u8>, usize) {
    let mut out = prefix.to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", first_num + i).as_bytes());
    }
    let xref_at = out.len();
    out.extend(b"xref\n");
    if first_num == 1 {
        out.extend(format!("0 {}\n", objects.len() + 1).as_bytes());
        out.extend(b"0000000000 65535 f\r\n");
    } else {
        out.extend(format!("{first_num} {}\n", objects.len()).as_bytes());
    }
    for o in offsets {
        out.extend(format!("{o:010} 00000 n\r\n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R {trailer_extra} >>\nstartxref\n{xref_at}\n%%EOF\n",
            first_num + objects.len()
        )
        .as_bytes(),
    );
    (out, xref_at)
}

fn page_objects(pages: usize) -> Vec<String> {
    let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", i + 3)).collect();
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {pages} >>",
            kids.join(" ")
        ),
    ];
    for _ in 0..pages {
        objs.push("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_string());
    }
    objs
}

fn minimal_pdf(pages: usize) -> Vec<u8> {
    build(
        b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n",
        1,
        &page_objects(pages),
        "",
    )
    .0
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
    let (mut file, first_xref) = build(b"%PDF-1.7\n", 1, &page_objects(1), "");
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
    let pdf = minimal_pdf(1);
    let text = String::from_utf8_lossy(&pdf).to_string();
    let xref_at = text.find("xref\n").unwrap();
    let looped = text.replace("/Root 1 0 R ", &format!("/Root 1 0 R /Prev {xref_at} "));
    assert!(matches!(
        open(looped.as_bytes()),
        Err(EngineError::Malformed { .. })
    ));
}

#[test]
fn unsupported_structures_are_reported_as_unsupported() {
    let (enc, _) = build(b"%PDF-1.7\n", 1, &page_objects(1), "/Encrypt 9 0 R");
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
    let wrong_count = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 2 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R >>".to_string(),
    ];
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
    for objs in [
        &wrong_count[..],
        &duplicate_kid[..],
        &self_kid[..],
        &missing_kid[..],
    ] {
        let (pdf, _) = build(b"%PDF-1.7\n", 1, objs, "");
        assert!(
            matches!(open(&pdf), Err(EngineError::Malformed { .. })),
            "{objs:?}"
        );
    }
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
