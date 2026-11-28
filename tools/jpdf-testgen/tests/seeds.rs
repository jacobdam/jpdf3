//! The generated files really are what the fuzz seeds claim. Spec: specs/tooling/fuzz.md.

use jpdf_engine::{Document, EngineError, Limits};
use jpdf_testgen::{
    CHUNK_SEPARATOR, ESCAPED_FF, MAX_FUZZ_OBJECTS, classic_pdf, engine_objects_seeds,
    engine_open_seeds, from_fuzz_chunks, minimal_pdf, to_fuzz_chunks,
};

fn open(bytes: &[u8]) -> Result<usize, EngineError> {
    Document::open(bytes, &Limits::default()).map(|d| d.page_count())
}

/// The engine does not walk nested page trees yet (`jpdf_cos::flat_page_count`). The nested
/// seeds are valid files that exercise that path; when nested trees land, this becomes
/// `Ok(4)` and the change is an expected, explained assertion update.
const NESTED: Result<usize, EngineError> = Err(EngineError::Unsupported("nested page trees"));

/// Every in-use xref entry of the last section points at `N 0 obj` for its own N.
fn assert_offsets_match(file: &[u8]) {
    assert_eq!(offset_problem(file), None);
}

fn offset_problem(file: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(file);
    let Some(xref) = text.rfind("\nxref\n") else {
        return Some("no xref section".into());
    };
    // Skips the newline before `xref`, then the `xref` line itself.
    let mut lines = text[xref..].lines().skip(2);
    let header = lines.next().unwrap_or_default();
    let mut parts = header.split(' ').map(|p| p.parse::<usize>());
    let (Some(Ok(first)), Some(Ok(count))) = (parts.next(), parts.next()) else {
        return Some(format!("bad subsection header {header:?}"));
    };
    let entries: Vec<&str> = lines.take(count).collect();
    if entries.len() != count {
        return Some(format!(
            "{count} entries announced, {} present",
            entries.len()
        ));
    }
    for (num, line) in (first..).zip(entries) {
        if line.ends_with(" f") {
            continue;
        }
        let Some(offset) = line.get(..10).and_then(|o| o.parse::<usize>().ok()) else {
            return Some(format!("bad entry {line:?}"));
        };
        let want = format!("{num} 0 obj");
        if !file
            .get(offset..)
            .is_some_and(|rest| rest.starts_with(want.as_bytes()))
        {
            return Some(format!(
                "entry {num} points at byte {offset}, which is not `{want}`"
            ));
        }
    }
    None
}

#[test]
fn minimal_files_open_with_their_page_count() {
    for n in [0, 1, 3, 50] {
        let file = minimal_pdf(n);
        assert_offsets_match(&file);
        assert_eq!(open(&file), Ok(n), "{n} pages");
    }
}

#[test]
fn engine_open_seeds_open_except_the_truncated_one() {
    let want = [
        ("empty-tree.pdf", Some(Ok(0))),
        ("one-page.pdf", Some(Ok(1))),
        ("three-pages.pdf", Some(Ok(3))),
        ("incremental.pdf", Some(Ok(2))),
        ("nested-tree.pdf", Some(NESTED)),
        ("truncated.pdf", None),
    ];
    let seeds = engine_open_seeds();
    assert_eq!(seeds.len(), want.len());
    for ((name, bytes), (want_name, pages)) in seeds.iter().zip(want) {
        assert_eq!(*name, want_name);
        match pages {
            Some(result) => assert_eq!(open(bytes), result, "{name}"),
            None => assert!(
                matches!(open(bytes), Err(EngineError::Malformed { .. })),
                "{name}: {:?}",
                open(bytes)
            ),
        }
    }
}

#[test]
fn engine_objects_seeds_open_through_the_template() {
    let want = [
        ("three-pages.bin", Ok(3)),
        ("nested-tree.bin", NESTED),
        ("inherited.bin", Ok(1)),
    ];
    let seeds = engine_objects_seeds();
    assert_eq!(seeds.len(), want.len());
    for ((name, chunks), (want_name, pages)) in seeds.iter().zip(want) {
        assert_eq!(*name, want_name);
        let file = from_fuzz_chunks(chunks);
        assert_offsets_match(&file);
        assert_eq!(open(&file), pages, "{name}");
    }
}

#[test]
fn chunks_round_trip() {
    let objects: [&[u8]; 3] = [b"<< /Type /Catalog /Pages 2 0 R >>", b"", b"(a)"];
    let data = to_fuzz_chunks(b"/Info 3 0 R", &objects);
    assert_eq!(
        from_fuzz_chunks(&data),
        classic_pdf(&objects, b"/Info 3 0 R")
    );
}

#[test]
fn template_never_panics_and_caps_objects() {
    // No separator: trailer only, zero objects.
    let file = from_fuzz_chunks(b"junk");
    assert!(file.ends_with(b"%%EOF\n"));
    assert!(open(&file).is_err());
    // Empty input.
    assert!(from_fuzz_chunks(b"").starts_with(b"%PDF-1.7"));
    // More chunks than the cap: only MAX_FUZZ_OBJECTS objects are written.
    let many = vec![CHUNK_SEPARATOR; MAX_FUZZ_OBJECTS + 10];
    let file = from_fuzz_chunks(&many);
    let header = format!("xref\n0 {}\n", MAX_FUZZ_OBJECTS + 1);
    assert!(
        file.windows(header.len()).any(|w| w == header.as_bytes()),
        "xref must list exactly {MAX_FUZZ_OBJECTS} objects"
    );
    assert_offsets_match(&file);
}

#[test]
fn escaped_ff_reaches_bodies() {
    // A UTF-16BE string (BOM FE FF) inside an object body survives the round trip.
    let objects: [&[u8]; 2] = [b"<< /Type /Catalog /Pages 2 0 R >>", b"(\xFE\xFF\x00a)"];
    let data = to_fuzz_chunks(b"", &objects);
    assert!(data.windows(2).any(|w| w == [CHUNK_SEPARATOR, ESCAPED_FF]));
    assert_eq!(from_fuzz_chunks(&data), classic_pdf(&objects, b""));
    // A trailing separator starts an empty last object; FF FF is two separators.
    assert_eq!(
        from_fuzz_chunks(b"\xFFa\xFF\xFF"),
        classic_pdf(&[b"a", b"", b""], b"")
    );
}

/// Deterministic pseudo-random inputs (xorshift64), dense in 0xFF and PDF tokens: the
/// template never panics, always writes correct offsets, and opening the result returns.
#[test]
fn template_holds_on_pseudo_random_inputs() {
    const ALPHABET: &[u8] = b"\xFF\x00<>[]()/ 0123456789RobjendstreamTypePagesKidsCount\n%";
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for round in 0..2000 {
        let len = usize::try_from(next() % 4096).unwrap();
        let input: Vec<u8> = (0..len)
            .map(|_| {
                let r = next();
                if r % 4 == 0 {
                    ((r >> 8) & 0xFF).to_le_bytes()[0]
                } else {
                    ALPHABET[usize::try_from((r >> 8) % ALPHABET.len() as u64).unwrap()]
                }
            })
            .collect();
        let file = from_fuzz_chunks(&input);
        assert_eq!(offset_problem(&file), None, "round {round}");
        let _ = open(&file);
        let _ = open(&input);
    }
}
