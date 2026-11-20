//! jpdf-testgen — writes small PDFs whose cross-reference data is correct by construction, for
//! fuzz seeds and fuzz templates. Spec: specs/tooling/fuzz.md.
//!
//! Only test tooling uses this crate; the engine never depends on it. Inputs are trusted (our
//! own tests and fuzz targets), but nothing here panics on any input either, because fuzz
//! targets feed it arbitrary bytes.
#![forbid(unsafe_code)]

use std::io::Write;

/// File header with the binary comment line that ISO 32000-2 §7.5.2 recommends.
pub const HEADER: &[u8] = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n";

/// Separates the chunks of an `engine_objects` fuzz input (see [`from_fuzz_chunks`]). 0xFF is
/// not a PDF delimiter or token start; the pair `FF 00` stands for a literal 0xFF byte, so
/// bodies can still contain every byte value (UTF-16BE BOMs, binary data).
pub const CHUNK_SEPARATOR: u8 = 0xFF;
/// Follows [`CHUNK_SEPARATOR`] to mean a literal 0xFF byte instead of a separator.
pub const ESCAPED_FF: u8 = 0x00;

/// Most objects [`from_fuzz_chunks`] writes; later chunks are ignored.
pub const MAX_FUZZ_OBJECTS: usize = 64;

/// One revision: a run of objects numbered consecutively, its xref section and its trailer.
#[derive(Debug, Clone, Copy)]
pub struct Revision<'a> {
    /// Number of the first object in `objects`.
    pub first_num: usize,
    /// Object bodies (what goes between `N 0 obj` and `endobj`).
    pub objects: &'a [&'a [u8]],
    /// The trailer's `/Size`.
    pub size: usize,
    /// Offset of the previous xref section (`/Prev`, ISO 32000-2 §7.5.6), if any.
    pub prev: Option<usize>,
    /// Extra trailer entries, written verbatim after `/Size`, `/Root` and `/Prev`.
    pub trailer_extra: &'a [u8],
}

/// Appends `rev` to `out` (ISO 32000-2 §7.5.4, §7.5.5) and returns the offset of its `xref`
/// keyword. A revision whose `first_num` is 1 also writes the free entry for object 0.
pub fn write_revision(out: &mut Vec<u8>, rev: &Revision<'_>) -> usize {
    let mut offsets = Vec::with_capacity(rev.objects.len());
    for (i, body) in rev.objects.iter().enumerate() {
        offsets.push(out.len());
        let num = rev.first_num.saturating_add(i);
        // Writing into a Vec cannot fail.
        let _ = writeln!(out, "{num} 0 obj");
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(b"xref\n");
    if rev.first_num == 1 {
        let _ = write!(
            out,
            "0 {}\n0000000000 65535 f\r\n",
            rev.objects.len().saturating_add(1)
        );
    } else {
        let _ = writeln!(out, "{} {}", rev.first_num, rev.objects.len());
    }
    for o in offsets {
        let _ = write!(out, "{o:010} 00000 n\r\n");
    }
    let _ = write!(out, "trailer\n<< /Size {} /Root 1 0 R ", rev.size);
    if let Some(p) = rev.prev {
        let _ = write!(out, "/Prev {p} ");
    }
    out.extend_from_slice(rev.trailer_extra);
    let _ = write!(out, " >>\nstartxref\n{xref_at}\n%%EOF\n");
    xref_at
}

/// A one-revision file: [`HEADER`], then `objects` numbered from 1 (object 1 is the root).
pub fn classic_pdf(objects: &[&[u8]], trailer_extra: &[u8]) -> Vec<u8> {
    let mut out = HEADER.to_vec();
    write_revision(
        &mut out,
        &Revision {
            first_num: 1,
            objects,
            size: objects.len().saturating_add(1),
            prev: None,
            trailer_extra,
        },
    );
    out
}

/// Catalog (1), a flat page tree (2) and `pages` leaf pages (3…).
pub fn page_tree(pages: usize) -> Vec<Vec<u8>> {
    let kids: Vec<String> = (0..pages)
        .map(|i| format!("{} 0 R", i.saturating_add(3)))
        .collect();
    let mut objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {pages} >>",
            kids.join(" ")
        )
        .into_bytes(),
    ];
    objs.extend(
        (0..pages).map(|_| b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_vec()),
    );
    objs
}

/// A valid file with `pages` empty pages.
pub fn minimal_pdf(pages: usize) -> Vec<u8> {
    let objs = page_tree(pages);
    classic_pdf(&borrow(&objs), b"")
}

/// The `engine_objects` fuzz template. `data` is split on [`CHUNK_SEPARATOR`] (`FF 00` is a
/// literal 0xFF): the first chunk is extra trailer entries, each further chunk an object body
/// numbered from 1 (at most [`MAX_FUZZ_OBJECTS`]; later chunks are ignored). The result always
/// has a correct xref table, so the fuzzer spends its time inside objects instead of
/// rediscovering file structure. Output size is linear in the input size.
pub fn from_fuzz_chunks(data: &[u8]) -> Vec<u8> {
    let mut chunks: Vec<Vec<u8>> = vec![Vec::new()];
    let mut bytes = data.iter().copied().peekable();
    while let Some(b) = bytes.next() {
        if b != CHUNK_SEPARATOR {
            if let Some(chunk) = chunks.last_mut() {
                chunk.push(b);
            }
        } else if bytes.next_if_eq(&ESCAPED_FF).is_some() {
            if let Some(chunk) = chunks.last_mut() {
                chunk.push(CHUNK_SEPARATOR);
            }
        } else if chunks.len() > MAX_FUZZ_OBJECTS {
            break;
        } else {
            chunks.push(Vec::new());
        }
    }
    let Some((trailer_extra, objects)) = chunks.split_first() else {
        return classic_pdf(&[], b"");
    };
    let objects: Vec<&[u8]> = objects.iter().map(Vec::as_slice).collect();
    classic_pdf(&objects, trailer_extra)
}

/// Inverse of [`from_fuzz_chunks`], for writing seeds.
pub fn to_fuzz_chunks(trailer_extra: &[u8], objects: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    escape_into(&mut out, trailer_extra);
    for body in objects {
        out.push(CHUNK_SEPARATOR);
        escape_into(&mut out, body);
    }
    out
}

fn escape_into(out: &mut Vec<u8>, bytes: &[u8]) {
    for &b in bytes {
        out.push(b);
        if b == CHUNK_SEPARATOR {
            out.push(ESCAPED_FF);
        }
    }
}

/// Seed inputs for the raw-bytes target `engine_open`: valid files and near-misses.
pub fn engine_open_seeds() -> Vec<(&'static str, Vec<u8>)> {
    let mut seeds = vec![
        ("empty-tree.pdf", minimal_pdf(0)),
        ("one-page.pdf", minimal_pdf(1)),
        ("three-pages.pdf", minimal_pdf(3)),
        ("incremental.pdf", incremental_update()),
        ("nested-tree.pdf", nested_tree()),
    ];
    let one = minimal_pdf(1);
    let half = one.len() / 2;
    seeds.push((
        "truncated.pdf",
        one.get(..half).unwrap_or_default().to_vec(),
    ));
    seeds
}

/// Seed inputs for the template target `engine_objects` (see [`from_fuzz_chunks`]).
pub fn engine_objects_seeds() -> Vec<(&'static str, Vec<u8>)> {
    let three = page_tree(3);
    vec![
        ("three-pages.bin", to_fuzz_chunks(b"", &borrow(&three))),
        (
            "nested-tree.bin",
            to_fuzz_chunks(b"", &borrow(&nested_tree_objects())),
        ),
        (
            "inherited.bin",
            to_fuzz_chunks(
                b"/Info 4 0 R",
                &[
                    b"<< /Type /Catalog /Pages 2 0 R >>",
                    b"<< /Type /Pages /Kids [3 0 R] /Count 1 /MediaBox [0 0 595 842] /Rotate 90 >>",
                    b"<< /Type /Page /Parent 2 0 R /Resources << >> >>",
                    b"<< /Title (seed) /Producer <6A706466> >>",
                ],
            ),
        ),
    ]
}

/// Root, an intermediate `/Pages` node and two leaves under each of two branches (4 pages).
fn nested_tree_objects() -> Vec<Vec<u8>> {
    vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 4 >>".to_vec(),
        b"<< /Type /Pages /Parent 2 0 R /Kids [5 0 R 6 0 R] /Count 2 >>".to_vec(),
        b"<< /Type /Pages /Parent 2 0 R /Kids [7 0 R 8 0 R] /Count 2 >>".to_vec(),
        b"<< /Type /Page /Parent 3 0 R >>".to_vec(),
        b"<< /Type /Page /Parent 3 0 R >>".to_vec(),
        b"<< /Type /Page /Parent 4 0 R >>".to_vec(),
        b"<< /Type /Page /Parent 4 0 R >>".to_vec(),
    ]
}

fn nested_tree() -> Vec<u8> {
    classic_pdf(&borrow(&nested_tree_objects()), b"")
}

/// One page, then two incremental updates (ISO 32000-2 §7.5.6): the first redefines the page
/// tree with two kids, the second adds the new kid (page 4).
fn incremental_update() -> Vec<u8> {
    let objs = page_tree(1);
    let mut out = HEADER.to_vec();
    let first = write_revision(
        &mut out,
        &Revision {
            first_num: 1,
            objects: &borrow(&objs),
            size: 4,
            prev: None,
            trailer_extra: b"",
        },
    );
    let second = write_revision(
        &mut out,
        &Revision {
            first_num: 2,
            objects: &[b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>"],
            size: 5,
            prev: Some(first),
            trailer_extra: b"",
        },
    );
    write_revision(
        &mut out,
        &Revision {
            first_num: 4,
            objects: &[b"<< /Type /Page /Parent 2 0 R >>"],
            size: 5,
            prev: Some(second),
            trailer_extra: b"",
        },
    );
    out
}

fn borrow(objs: &[Vec<u8>]) -> Vec<&[u8]> {
    objs.iter().map(Vec::as_slice).collect()
}
