//! A tiny JSON writer for the report (spec item 9). The crate has no dependencies, so this
//! replaces a JSON crate (same reasoning as docs/adr/0002 for xtask).

use std::fmt::Write as _;

/// Appends `s` as a JSON string literal (RFC 8259 §7).
pub fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Other C0 controls, DEL and the line/paragraph separators (which break some
            // JavaScript consumers) as \u escapes.
            c if c < ' ' || c == '\u{7f}' || c == '\u{2028}' || c == '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Appends an optional string; `None` is `null`.
pub fn opt_string(out: &mut String, s: Option<&str>) {
    match s {
        Some(s) => string(out, s),
        None => out.push_str("null"),
    }
}

/// Appends a number with at most 3 decimals; `None`, NaN and infinities are `null`.
pub fn number(out: &mut String, x: Option<f64>) {
    match x {
        Some(x) if x.is_finite() => {
            let mut s = format!("{x:.3}");
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
            if s == "-0" {
                s = "0".to_string();
            }
            out.push_str(&s);
        }
        _ => out.push_str("null"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> String {
        let mut o = String::new();
        string(&mut o, x);
        o
    }

    fn n(x: Option<f64>) -> String {
        let mut o = String::new();
        number(&mut o, x);
        o
    }

    #[test]
    fn escapes() {
        assert_eq!(s(""), r#""""#);
        assert_eq!(s(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(s("\n\r\t"), r#""\n\r\t""#);
        assert_eq!(s("\u{0}\u{1f}\u{7f}"), r#""\u0000\u001f\u007f""#);
        assert_eq!(s("\u{2028}\u{2029}"), "\"\\u2028\\u2029\"");
        assert_eq!(s("Ünïcødé 文書.pdf"), "\"Ünïcødé 文書.pdf\"");
        assert_eq!(s("</script>"), "\"</script>\"");
    }

    #[test]
    fn numbers() {
        assert_eq!(n(None), "null");
        assert_eq!(n(Some(f64::NAN)), "null");
        assert_eq!(n(Some(f64::INFINITY)), "null");
        assert_eq!(n(Some(0.0)), "0");
        assert_eq!(n(Some(-0.0001)), "0");
        assert_eq!(n(Some(2.0)), "2");
        assert_eq!(n(Some(1.23456)), "1.235");
        assert_eq!(n(Some(0.5)), "0.5");
        assert_eq!(n(Some(1500.25)), "1500.25");
        assert_eq!(n(Some(-3.1)), "-3.1");
    }
}
