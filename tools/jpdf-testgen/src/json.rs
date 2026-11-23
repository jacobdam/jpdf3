//! A small strict JSON reader (RFC 8259) for tool output that test tooling must understand:
//! `xcresulttool` test results, `plutil -convert json` entitlements and `gh` lists (xtask), and
//! `cargo metadata` (the jpdf-cli dependency gate, #235). Neither takes a third-party
//! dependency (docs/adr/0002 Context; the lockfile has none), so this replaces a JSON crate.
//! Input is bounded by the caller; nesting depth is bounded here; nothing panics on malformed
//! input.

/// Most nested arrays/objects accepted. xcresulttool trees are under 10 levels deep.
const MAX_DEPTH: usize = 128;

/// A parsed JSON value. Numbers keep their source text (xtask never computes with them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Value>),
    /// Members in source order; duplicate keys are rejected.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// The member `key` of an object.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Parses one JSON document (surrounding whitespace allowed, nothing else).
pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.pos != p.bytes.len() {
        return Err(p.err("trailing data"));
    }
    Ok(v)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, what: &str) -> String {
        format!("JSON: {what} at byte {}", self.pos)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos = self.pos.saturating_add(1);
        Some(b)
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos = self.pos.saturating_add(1);
        }
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        if self.bump() == Some(b) {
            Ok(())
        } else {
            Err(self.err(&format!("expected `{}`", char::from(b))))
        }
    }

    fn literal(&mut self, word: &str, v: Value) -> Result<Value, String> {
        let end = self.pos.saturating_add(word.len());
        if self.bytes.get(self.pos..end) == Some(word.as_bytes()) {
            self.pos = end;
            Ok(v)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth >= MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unexpected character")),
            None => Err(self.err("unexpected end")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        self.expect(b'{')?;
        let mut members: Vec<(String, Value)> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.pos = self.pos.saturating_add(1);
            return Ok(Value::Object(members));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a string key"));
            }
            let key = self.string()?;
            if !seen.insert(key.clone()) {
                return Err(self.err(&format!("duplicate key {key:?}")));
            }
            self.ws();
            self.expect(b':')?;
            self.ws();
            let v = self.value(depth.saturating_add(1))?;
            members.push((key, v));
            self.ws();
            match self.bump() {
                Some(b',') => {}
                Some(b'}') => return Ok(Value::Object(members)),
                _ => return Err(self.err("expected `,` or `}`")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.pos = self.pos.saturating_add(1);
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth.saturating_add(1))?);
            self.ws();
            match self.bump() {
                Some(b',') => {}
                Some(b']') => return Ok(Value::Array(items)),
                _ => return Err(self.err("expected `,` or `]`")),
            }
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos = self.pos.saturating_add(1);
        }
        self.pos.saturating_sub(start)
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos = self.pos.saturating_add(1);
        }
        match self.peek() {
            Some(b'0') => self.pos = self.pos.saturating_add(1),
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.err("invalid number")),
        }
        if self.peek() == Some(b'.') {
            self.pos = self.pos.saturating_add(1);
            if self.digits() == 0 {
                return Err(self.err("invalid number"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos = self.pos.saturating_add(1);
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos = self.pos.saturating_add(1);
            }
            if self.digits() == 0 {
                return Err(self.err("invalid number"));
            }
        }
        let text = self
            .bytes
            .get(start..self.pos)
            .and_then(|b| std::str::from_utf8(b).ok())
            .ok_or_else(|| self.err("invalid number"))?;
        Ok(Value::Number(text.to_owned()))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut v: u32 = 0;
        for _ in 0..4 {
            let d = self
                .bump()
                .and_then(|b| char::from(b).to_digit(16))
                .ok_or_else(|| self.err("invalid \\u escape"))?;
            v = v
                .checked_mul(16)
                .and_then(|x| x.checked_add(d))
                .unwrap_or(0);
        }
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            match self.bump() {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => break,
                Some(b'\\') => {
                    let c = match self.bump() {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'/') => '/',
                        Some(b'b') => '\u{8}',
                        Some(b'f') => '\u{c}',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        Some(b'u') => self.unicode_escape()?,
                        _ => return Err(self.err("invalid escape")),
                    };
                    let mut buf = [0_u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                Some(b) if b < 0x20 => return Err(self.err("control character in string")),
                Some(b) => out.push(b),
            }
        }
        // The input is a &str, so raw bytes are valid UTF-8 and escapes add valid chars.
        String::from_utf8(out).map_err(|_| self.err("invalid UTF-8 in string"))
    }

    fn unicode_escape(&mut self) -> Result<char, String> {
        let hi = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&hi) {
            if self.bump() != Some(b'\\') || self.bump() != Some(b'u') {
                return Err(self.err("unpaired surrogate"));
            }
            let lo = self.hex4()?;
            if !(0xDC00..0xE000).contains(&lo) {
                return Err(self.err("unpaired surrogate"));
            }
            0x10000_u32
                .checked_add(((hi - 0xD800) << 10) | (lo - 0xDC00))
                .ok_or_else(|| self.err("invalid surrogate pair"))?
        } else {
            hi
        };
        char::from_u32(code).ok_or_else(|| self.err("unpaired surrogate"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Value {
        Value::String(v.to_owned())
    }

    #[test]
    fn parses_values() {
        assert_eq!(parse(" null "), Ok(Value::Null));
        assert_eq!(parse("true"), Ok(Value::Bool(true)));
        assert_eq!(parse("-0.5e+3"), Ok(Value::Number("-0.5e+3".to_owned())));
        assert_eq!(
            parse(r#""a\"\\\/\n\u00e9\ud83d\ude00""#),
            Ok(s("a\"\\/\né😀"))
        );
        assert_eq!(parse("\"é\""), Ok(s("é")));
        let v = parse(r#"{"a": [1, {"b": "c"}], "d": {}}"#).unwrap();
        assert_eq!(
            v.get("a").and_then(Value::as_array).map(<[_]>::len),
            Some(2)
        );
        assert_eq!(
            v.get("a")
                .and_then(Value::as_array)
                .and_then(|a| a.get(1))
                .and_then(|o| o.get("b"))
                .and_then(Value::as_str),
            Some("c")
        );
        assert_eq!(v.get("d"), Some(&Value::Object(vec![])));
        assert_eq!(v.get("zz"), None);
        assert_eq!(parse("[]"), Ok(Value::Array(vec![])));
    }

    #[test]
    fn rejects_malformed() {
        for bad in [
            "",
            " ",
            "nul",
            "truex",
            "01",
            "1.",
            "1e",
            "-",
            ".5",
            "+1",
            "\"abc",
            "\"\\x\"",
            "\"\\u12\"",
            "\"\\ud800\"",
            "\"\\ud800\\u0041\"",
            "\"\\udc00\"",
            "\"a\u{1}b\"",
            "[1,]",
            "[1 2]",
            "{\"a\":1,}",
            "{\"a\" 1}",
            "{a:1}",
            "{\"a\":1,\"a\":2}",
            "[] []",
            "[",
            "{",
            "\u{feff}{}",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn bounds_depth() {
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(&ok).is_ok());
        let deep = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        assert!(parse(&deep).is_err());
        let huge = "[".repeat(1_000_000);
        assert!(parse(&huge).is_err(), "no stack overflow");
    }
}
