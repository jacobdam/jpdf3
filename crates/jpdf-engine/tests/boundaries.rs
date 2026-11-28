//! Exact limit boundaries and error text of `Document::open` (#87, audit 0001). Spec:
//! specs/cli/jpdf-cli.md (the CLI prints these messages, ops/judge/CONTRACT.md §1).

use jpdf_engine::{Document, EngineError, Limits};
use jpdf_testgen::minimal_pdf;

/// `max_file_size` at the file's exact length opens; one byte less refuses it. This pins the
/// boundary of `Document::open` as a whole: the engine's own check and the one in
/// `XrefDocument::parse` give the same error, so the one-past side can't tell which refused it,
/// and the at-limit side shows that neither is stricter than `>`.
#[test]
fn file_size_limit_is_exact() {
    let pdf = minimal_pdf(1);
    let at = |max_file_size| {
        let limits = Limits {
            max_file_size,
            ..Limits::default()
        };
        Document::open(&pdf, &limits).map(|d| d.page_count())
    };
    assert_eq!(at(pdf.len()), Ok(1));
    assert_eq!(
        at(pdf.len().saturating_sub(1)),
        Err(EngineError::LimitExceeded("file size"))
    );
}

/// `max_xref_entries` at its exact boundary on a one-page PDF that opens: its table has 4
/// entries (the free head, the catalog, the page tree and the page), so a limit of 4 opens it
/// and 3 refuses it.
#[test]
fn xref_entry_limit_is_exact_on_a_document_that_opens() {
    let pdf = minimal_pdf(1);
    let at = |max_xref_entries| {
        let limits = Limits {
            max_xref_entries,
            ..Limits::default()
        };
        Document::open(&pdf, &limits).map(|d| d.page_count())
    };
    assert_eq!(at(4), Ok(1));
    assert_eq!(
        at(3),
        Err(EngineError::LimitExceeded("cross-reference entries"))
    );
}

#[test]
fn display_text_of_every_variant() {
    let cases = [
        (
            EngineError::Malformed {
                what: "bad xref",
                offset: Some(12),
            },
            "malformed PDF: bad xref at byte 12",
        ),
        (
            EngineError::Malformed {
                what: "bad kid",
                offset: None,
            },
            "malformed PDF: bad kid",
        ),
        (EngineError::LimitExceeded("pages"), "limit exceeded: pages"),
        (
            EngineError::Unsupported("nested page trees"),
            "not implemented: nested page trees",
        ),
    ];
    for (e, text) in cases {
        assert_eq!(e.to_string(), text, "{e:?}");
    }
}

/// The message a real failure prints, end to end: an empty file has no `startxref`.
#[test]
fn open_error_text_names_the_offset() {
    let e = Document::open(b"", &Limits::default()).map(|d| d.page_count());
    assert_eq!(
        e.map_err(|e| e.to_string()),
        Err("malformed PDF: startxref not found at byte 0".to_string())
    );
}
