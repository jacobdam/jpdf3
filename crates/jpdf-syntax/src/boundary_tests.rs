//! Tests that kill cargo-mutants survivors of the lexer, parser and cross-reference reader
//! (#87, audit 0001): exact limit boundaries, delimiter and digit classes, and error text. They
//! sit in their own module so they go through the same crate-internal entry points
//! (`Parser`, `XrefDocument`) as the code under test.

use crate::error::malformed;
use crate::lexer::{LexBudget, Parser};
use crate::{Limits, ObjRef, Object, SyntaxError, XrefDocument};

fn parse_with(src: &[u8], limits: &Limits) -> Result<Object, SyntaxError> {
    let budget = LexBudget::new(src.len(), limits);
    Parser::new(src, 0, limits, &budget).parse_object()
}

fn parse(src: &[u8]) -> Result<Object, SyntaxError> {
    parse_with(src, &Limits::default())
}

/// The CLI prints these messages (ops/judge/CONTRACT.md §1).
#[test]
fn display_text_of_every_variant() {
    let cases = [
        (malformed("bad", 9), "malformed PDF: bad at byte 9"),
        (
            SyntaxError::LimitExceeded("nesting depth"),
            "limit exceeded: nesting depth",
        ),
        (
            SyntaxError::Unsupported("encrypted documents"),
            "not implemented: encrypted documents",
        ),
    ];
    for (e, text) in cases {
        assert_eq!(e.to_string(), text, "{e:?}");
    }
}

#[test]
fn as_integer_reads_only_integers() {
    assert_eq!(Object::Integer(-4).as_integer(), Some(-4));
    for other in [
        Object::Real(4.0),
        Object::Null,
        Object::Ref(ObjRef {
            num: 4,
            generation: 0,
        }),
    ] {
        assert_eq!(other.as_integer(), None, "{other:?}");
    }
}

/// `max_objects_per_document` at its exact boundary, across parses: 3 + 2 objects reach a
/// limit of 5; one more object is refused.
#[test]
fn objects_per_document_limit_is_exact() {
    let limits = Limits {
        max_objects_per_document: 5,
        ..Limits::default()
    };
    let budget = LexBudget::new(16, &limits);
    let next = |src: &[u8]| Parser::new(src, 0, &limits, &budget).parse_object();
    assert!(next(b"[1 2]").is_ok());
    assert_eq!(next(b"[3]"), Ok(Object::Array(vec![Object::Integer(3)])));
    assert_eq!(
        next(b"4"),
        Err(SyntaxError::LimitExceeded("objects per document"))
    );
}

/// The lexing budget at its exact boundary. Every byte stepped over is charged and the total
/// is checked before the next token: a 4-byte string, then the end of input, fits a budget of
/// exactly 4 bytes and fails with 3.
#[test]
fn lex_budget_is_exact() {
    let src = b"(xx)";
    let lex = |slack| {
        let limits = Limits {
            lex_budget_factor: 0,
            lex_budget_slack: slack,
            ..Limits::default()
        };
        let budget = LexBudget::new(src.len(), &limits);
        let mut p = Parser::new(src, 0, &limits, &budget);
        let first = p.parse_object()?;
        assert_eq!(first, Object::String(b"xx".to_vec()));
        p.lexer.next_token().map(|t| t.is_none())
    };
    assert_eq!(lex(4), Ok(true));
    assert_eq!(
        lex(3),
        Err(SyntaxError::LimitExceeded("bytes lexed per document"))
    );
}

/// `max_objects_per_parse` at a small exact boundary: `[1 2]` is 3 objects.
#[test]
fn objects_per_parse_limit_is_exact() {
    let at = |max| {
        let limits = Limits {
            max_objects_per_parse: max,
            ..Limits::default()
        };
        parse_with(b"[1 2]", &limits).map(|_| ())
    };
    assert_eq!(at(3), Ok(()));
    assert_eq!(
        at(2),
        Err(SyntaxError::LimitExceeded("objects in one parse"))
    );
}

/// `max_nesting_depth` at a small exact boundary: `[[1]]` is depth 2.
#[test]
fn nesting_depth_limit_is_exact() {
    let at = |max| {
        let limits = Limits {
            max_nesting_depth: max,
            ..Limits::default()
        };
        parse_with(b"[[1]]", &limits).map(|_| ())
    };
    assert_eq!(at(2), Ok(()));
    assert_eq!(at(1), Err(SyntaxError::LimitExceeded("nesting depth")));
}

/// `objects` written back to back after `prefix` (object `n` at the n-th offset, numbered from
/// 1), then an xref table whose entries carry `generations`, and a trailer.
fn xref_file(prefix: &[u8], objects: &[&[u8]], generations: &[u16]) -> Vec<u8> {
    let mut out = prefix.to_vec();
    let mut offsets = Vec::new();
    for body in objects {
        offsets.push(out.len());
        out.extend(*body);
    }
    let xref_at = out.len();
    let size = offsets.len() + 1;
    out.extend(format!("\nxref\n0 {size}\n0000000000 65535 f\r\n").as_bytes());
    for (o, g) in offsets.iter().zip(generations) {
        out.extend(format!("{o:010} {g:05} n\r\n").as_bytes());
    }
    let xref_at = xref_at + 1;
    out.extend(format!("trailer\n<< /Size {size} >>\nstartxref\n{xref_at}\n%%EOF\n").as_bytes());
    out
}

fn r(num: u32, generation: u16) -> ObjRef {
    ObjRef { num, generation }
}

/// ISO 32000-2 §7.3.10: the object header must carry the referenced number and generation.
/// Each half of the check is tested with the other half matching.
#[test]
fn resolve_checks_both_header_numbers() {
    let header = |body: &[u8]| {
        let data = xref_file(b"%PDF-1.7\n", &[body], &[0]);
        let doc = XrefDocument::parse(&data, &Limits::default()).expect("parses");
        doc.resolve(r(1, 0)).map(|o| (*o).clone())
    };
    assert_eq!(header(b"1 0 obj 7 endobj"), Ok(Object::Integer(7)));
    let mismatch = Err(malformed("object header does not match xref", 9));
    assert_eq!(header(b"1 1 obj 7 endobj"), mismatch);
    assert_eq!(header(b"2 0 obj 7 endobj"), mismatch);
}

/// A reference whose generation differs from the xref entry's is the null object, without
/// reading the object (ISO 32000-2 §7.3.10); the matching generation reads it.
#[test]
fn resolve_with_another_generation_is_null() {
    let data = xref_file(b"%PDF-1.7\n", &[b"1 3 obj 7 endobj"], &[3]);
    let doc = XrefDocument::parse(&data, &Limits::default()).expect("parses");
    assert_eq!(
        doc.resolve(r(1, 3)).map(|o| (*o).clone()),
        Ok(Object::Integer(7))
    );
    assert_eq!(doc.resolve(r(1, 0)).map(|o| (*o).clone()), Ok(Object::Null));
}

/// Two objects that touch (the second starts at the byte where the first one's value ends)
/// don't overlap, in either resolve order; a span that ends one byte later would.
#[test]
fn adjacent_objects_do_not_overlap() {
    let data = xref_file(b"%PDF-1.7\n", &[b"1 0 obj<<>>", b"2 0 obj<<>>"], &[0, 0]);
    for order in [[1, 2], [2, 1]] {
        let doc = XrefDocument::parse(&data, &Limits::default()).expect("parses");
        for num in order {
            let o = doc.resolve(r(num, 0)).map(|o| (*o).clone());
            assert_eq!(
                o,
                Ok(Object::Dict(crate::Dict::default())),
                "{order:?} {num}"
            );
        }
    }
}

/// `startxref` is searched for in the last 64 KiB (`STARTXREF_WINDOW`): a keyword starting
/// exactly 64 KiB before the end is found, one byte earlier it is not.
#[test]
fn startxref_window_is_exact() {
    let at_distance = |distance: usize| {
        let mut data = b"xref\n0 1\n0000000000 65535 f\r\ntrailer\n<< /Size 1 >>\n".to_vec();
        data.extend(b"startxref\n0\n%");
        let keyword_at = data.len() - b"startxref\n0\n%".len();
        data.resize(keyword_at + distance, b'x');
        (
            data.len(),
            XrefDocument::parse(&data, &Limits::default()).map(|_| ()),
        )
    };
    assert_eq!(at_distance(64 * 1024).1, Ok(()));
    let (len, past) = at_distance(64 * 1024 + 1);
    assert_eq!(past, Err(malformed("startxref not found", len)));
}

/// ISO 32000-2 §7.2.3, Table 2: `)`, `{` and `}` end a name and are not tokens of their own
/// here. If one of them were a regular character, the name would swallow it and the array
/// would parse.
#[test]
fn closing_paren_and_braces_are_delimiters() {
    for (src, at) in [(&b"[/A)]"[..], 3), (b"[/A{]", 3), (b"[/A}]", 3)] {
        assert_eq!(
            parse(src),
            Err(malformed("unexpected delimiter", at)),
            "{}",
            String::from_utf8_lossy(src)
        );
    }
    assert_eq!(
        parse(b"[/A]"),
        Ok(Object::Array(vec![Object::Name(b"A".to_vec())]))
    );
}

/// Hex digits in either case (ISO 32000-2 §7.3.4.3, §7.3.5) and how two nibbles form a byte.
/// The `|` in `hi << 4 | lo` can't be told from `^` or `+` by any input: the shifted high
/// nibble has no low bits, so cargo-mutants' `|` → `^` mutants there are equivalent.
#[test]
fn hex_digits_of_both_cases_combine_into_bytes() {
    assert_eq!(parse(b"<6a6B>"), Ok(Object::String(b"jk".to_vec())));
    assert_eq!(parse(b"<ff>"), Ok(Object::String(vec![0xFF])));
    assert_eq!(parse(b"<f>"), Ok(Object::String(vec![0xF0])));
    assert_eq!(
        parse(b"/#ff#FF#3a"),
        Ok(Object::Name(vec![0xFF, 0xFF, b':']))
    );
    assert_eq!(parse(b"/#Fg"), Ok(Object::Name(b"#Fg".to_vec())));
}

/// A one-object file with an empty trailer; `startxref` names the xref table.
fn tiny_file() -> Vec<u8> {
    xref_file(b"%PDF-1.7\n", &[b"1 0 obj 7 endobj"], &[0])
}

/// `max_file_size` at the exact file length parses; one byte less refuses it before reading.
#[test]
fn xref_file_size_limit_is_exact() {
    let data = tiny_file();
    let at = |max_file_size| {
        let limits = Limits {
            max_file_size,
            ..Limits::default()
        };
        XrefDocument::parse(&data, &limits).map(|_| ())
    };
    assert_eq!(at(data.len()), Ok(()));
    assert_eq!(
        at(data.len() - 1),
        Err(SyntaxError::LimitExceeded("file size"))
    );
}

/// The `startxref` offset must be inside the file: in an 80-byte file, offset 79 (the last
/// byte) is, 80 (the file length) is not.
#[test]
fn startxref_offset_at_the_file_length_is_invalid() {
    let with_offset = |offset: usize| {
        let mut data = b"xref\n0 1\n0000000000 65535 f\r\ntrailer\n<< /Size 1 >>\n".to_vec();
        let keyword_at = data.len();
        data.extend(format!("startxref\n{offset} ").as_bytes());
        // Pad with spaces until the file is exactly 80 bytes long.
        data.resize(80, b' ');
        (
            keyword_at,
            XrefDocument::parse(&data, &Limits::default()).map(|_| ()),
        )
    };
    let (keyword_at, at_len) = with_offset(80);
    assert_eq!(
        at_len,
        Err(malformed("invalid startxref offset", keyword_at))
    );
    // One byte less points at the last byte, a space: the offset is accepted and the section
    // read there fails on its content instead.
    assert_eq!(with_offset(79).1, Err(malformed("expected 'xref'", 79)));
}

/// An in-use entry with a negative offset is not an entry (only a positive offset too large
/// for any accepted file reads as free).
#[test]
fn negative_xref_offset_is_an_invalid_entry() {
    let data = b"xref\n0 1\n-000000001 00000 n\r\ntrailer\n<< /Size 1 >>\nstartxref\n0\n%%EOF";
    assert_eq!(
        XrefDocument::parse(data, &Limits::default()).map(|_| ()),
        Err(malformed("invalid xref entry", 8))
    );
}

/// A subsection may end at the largest object number: the number advances only between
/// entries, so the last entry of `4294967295 1` doesn't overflow it.
#[test]
fn subsection_ending_at_the_largest_object_number_parses() {
    let data =
        b"xref\n4294967295 1\n0000000000 00000 f\r\ntrailer\n<< /Size 1 >>\nstartxref\n0\n%%EOF";
    let doc = XrefDocument::parse(data, &Limits::default()).map(|_| ());
    assert_eq!(doc, Ok(()));
    // Two entries from there do overflow it, after the first entry (read from byte 17).
    let two = b"xref\n4294967295 2\n0000000000 00000 f\r\n0000000000 00000 f\r\ntrailer\n<< >>\nstartxref\n0\n%%EOF";
    assert_eq!(
        XrefDocument::parse(two, &Limits::default()).map(|_| ()),
        Err(malformed("object number out of range", 17))
    );
}

/// ISO 32000-2 §7.3.3: a number has at least one digit. A lone sign or point is a keyword,
/// which is not an object, so the parser rejects it as one; with the digit count unchecked, `-`
/// would read as the integer 0 and `.` as a malformed real.
#[test]
fn sign_or_point_alone_is_not_a_number() {
    for src in [&b"-"[..], b"+", b"."] {
        assert_eq!(
            parse(src),
            Err(malformed("unexpected keyword", 0)),
            "{}",
            String::from_utf8_lossy(src)
        );
    }
    assert_eq!(parse(b"-0"), Ok(Object::Integer(0)));
    assert_eq!(parse(b"-.0"), Ok(Object::Real(0.0)));
}
