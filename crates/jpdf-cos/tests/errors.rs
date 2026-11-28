//! `CosError`'s own text and source (#87 item 7). The CLI prints the `EngineError` a
//! `CosError` becomes: crates/jpdf-engine/tests/boundaries.rs pins `EngineError`'s text, and
//! crates/jpdf-engine/tests/open.rs the conversion.

use std::error::Error;

use jpdf_cos::CosError;
use jpdf_syntax::SyntaxError;

#[test]
fn display_text_of_every_variant() {
    let syntax = SyntaxError::Malformed {
        what: "bad xref",
        offset: 12,
    };
    let cases = [
        (
            CosError::Syntax(syntax),
            "malformed PDF: bad xref at byte 12",
        ),
        (CosError::Malformed("bad kid"), "malformed PDF: bad kid"),
        (CosError::LimitExceeded("pages"), "limit exceeded: pages"),
        (CosError::Unsupported("nested"), "not implemented: nested"),
    ];
    for (e, text) in cases {
        assert_eq!(e.to_string(), text, "{e:?}");
    }
}

/// Only a wrapped syntax error has a source, and it is that error.
#[test]
fn source_is_the_wrapped_syntax_error() {
    let inner = SyntaxError::Malformed {
        what: "x",
        offset: 7,
    };
    let e = CosError::from(inner.clone());
    assert_eq!(e, CosError::Syntax(inner.clone()));
    let source = e.source().and_then(|s| s.downcast_ref::<SyntaxError>());
    assert_eq!(source, Some(&inner));
    for other in [
        CosError::Malformed("m"),
        CosError::LimitExceeded("l"),
        CosError::Unsupported("u"),
    ] {
        assert!(other.source().is_none(), "{other:?}");
    }
}
