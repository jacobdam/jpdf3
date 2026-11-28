//! `CosError` messages and sources. The CLI prints these (ops/judge/CONTRACT.md §1), so the
//! text is pinned (#87 item 7).

use std::error::Error;

use jpdf_cos::CosError;
use jpdf_syntax::SyntaxError;

#[test]
fn display_text_of_every_variant() {
    let syntax = SyntaxError::LimitExceeded("pages");
    let cases = [
        (CosError::Syntax(syntax.clone()), "limit exceeded: pages"),
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
    let source = e.source().map(ToString::to_string);
    assert_eq!(source, Some(inner.to_string()));
    for other in [
        CosError::Malformed("m"),
        CosError::LimitExceeded("l"),
        CosError::Unsupported("u"),
    ] {
        assert!(other.source().is_none(), "{other:?}");
    }
}
