//! Tokenizer and object parser. ISO 32000-2 §7.2 (lexical conventions), §7.3 (objects).

use crate::error::{SyntaxError, malformed};
use crate::limits::Limits;
use crate::object::{Dict, ObjRef, Object};

/// ISO 32000-2 §7.2.3, Table 1: white-space characters.
pub(crate) fn is_whitespace(b: u8) -> bool {
    matches!(b, 0x00 | 0x09 | 0x0A | 0x0C | 0x0D | 0x20)
}

/// ISO 32000-2 §7.2.3, Table 2: delimiter characters.
fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn is_regular(b: u8) -> bool {
    !is_whitespace(b) && !is_delimiter(b)
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token<'a> {
    Integer(i64),
    Real(f64),
    Name(Vec<u8>),
    String(Vec<u8>),
    ArrayStart,
    ArrayEnd,
    DictStart,
    DictEnd,
    /// Any other run of regular characters (`obj`, `R`, `true`, `xref`, …).
    Keyword(&'a [u8]),
}

pub(crate) struct Lexer<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Advances one byte. Callers only call this after `peek` returned `Some`, so `pos` stays
    /// `<= data.len()`, which can never overflow.
    fn bump(&mut self) {
        self.pos = self.pos.saturating_add(1);
    }

    /// ISO 32000-2 §7.2.4: comments run from `%` to the end of the line.
    fn skip_whitespace_and_comments(&mut self) {
        while let Some(b) = self.peek() {
            if is_whitespace(b) {
                self.bump();
            } else if b == b'%' {
                while let Some(c) = self.peek() {
                    if c == b'\r' || c == b'\n' {
                        break;
                    }
                    self.bump();
                }
            } else {
                break;
            }
        }
    }

    /// Returns the next token, or `None` at end of input.
    pub(crate) fn next_token(&mut self) -> Result<Option<Token<'a>>, SyntaxError> {
        self.skip_whitespace_and_comments();
        let start = self.pos;
        let Some(b) = self.peek() else {
            return Ok(None);
        };
        let tok = match b {
            b'[' => {
                self.bump();
                Token::ArrayStart
            }
            b']' => {
                self.bump();
                Token::ArrayEnd
            }
            b'<' => {
                self.bump();
                if self.peek() == Some(b'<') {
                    self.bump();
                    Token::DictStart
                } else {
                    Token::String(self.hex_string()?)
                }
            }
            b'>' => {
                self.bump();
                if self.peek() == Some(b'>') {
                    self.bump();
                    Token::DictEnd
                } else {
                    return Err(malformed("unexpected '>'", start));
                }
            }
            b'(' => {
                self.bump();
                Token::String(self.literal_string(start)?)
            }
            b'/' => {
                self.bump();
                Token::Name(self.name())
            }
            b')' | b'{' | b'}' => return Err(malformed("unexpected delimiter", start)),
            _ => {
                while self.peek().is_some_and(is_regular) {
                    self.bump();
                }
                let word = self.data.get(start..self.pos).unwrap_or_default();
                classify_regular(word, start)?
            }
        };
        Ok(Some(tok))
    }

    /// ISO 32000-2 §7.3.5: `#xx` escapes in names.
    fn name(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some(b) = self.peek().filter(|b| is_regular(*b)) {
            self.bump();
            if b == b'#' {
                let hi = self.peek().and_then(hex_value);
                let lo = self
                    .data
                    .get(self.pos.saturating_add(1))
                    .copied()
                    .and_then(hex_value);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    self.bump();
                    self.bump();
                    out.push(hi << 4 | lo);
                    continue;
                }
            }
            out.push(b);
        }
        out
    }

    /// ISO 32000-2 §7.3.4.3: hexadecimal strings. The opening `<` is consumed.
    fn hex_string(&mut self) -> Result<Vec<u8>, SyntaxError> {
        let start = self.pos;
        let mut out = Vec::new();
        let mut high: Option<u8> = None;
        loop {
            let Some(b) = self.peek() else {
                return Err(malformed("unterminated hexadecimal string", start));
            };
            self.bump();
            if b == b'>' {
                break;
            }
            if is_whitespace(b) {
                continue;
            }
            let Some(v) = hex_value(b) else {
                return Err(malformed("invalid hexadecimal string", start));
            };
            match high.take() {
                Some(h) => out.push(h << 4 | v),
                None => high = Some(v),
            }
        }
        // An odd final digit is followed by an implied 0.
        if let Some(h) = high {
            out.push(h << 4);
        }
        Ok(out)
    }

    /// ISO 32000-2 §7.3.4.2: literal strings. The opening `(` is consumed.
    fn literal_string(&mut self, start: usize) -> Result<Vec<u8>, SyntaxError> {
        let mut out = Vec::new();
        let mut depth: usize = 1;
        loop {
            let Some(b) = self.peek() else {
                return Err(malformed("unterminated literal string", start));
            };
            self.bump();
            match b {
                b'(' => {
                    depth = depth.saturating_add(1);
                    out.push(b);
                }
                b')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                    out.push(b);
                }
                b'\\' => self.escape(&mut out),
                b'\r' => {
                    // An end-of-line marker inside a string reads as a single LF.
                    if self.peek() == Some(b'\n') {
                        self.bump();
                    }
                    out.push(b'\n');
                }
                _ => out.push(b),
            }
        }
        Ok(out)
    }

    /// ISO 32000-2 §7.3.4.2, Table 3: escape sequences in literal strings.
    fn escape(&mut self, out: &mut Vec<u8>) {
        let Some(c) = self.peek() else {
            return;
        };
        self.bump();
        match c {
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'b' => out.push(0x08),
            b'f' => out.push(0x0C),
            b'0'..=b'7' => {
                let mut v = u16::from(c - b'0');
                for _ in 0..2 {
                    match self.peek() {
                        Some(d @ b'0'..=b'7') => {
                            self.bump();
                            v = v * 8 + u16::from(d - b'0');
                        }
                        _ => break,
                    }
                }
                // High-order overflow is ignored.
                out.push(u8::try_from(v % 256).unwrap_or_default());
            }
            b'\r' => {
                // Backslash + EOL: line continuation.
                if self.peek() == Some(b'\n') {
                    self.bump();
                }
            }
            b'\n' => {}
            // `\(`, `\)`, `\\` and unknown escapes: the backslash is ignored.
            other => out.push(other),
        }
    }
}

/// ISO 32000-2 §7.3.3: numbers; anything else is a keyword.
fn classify_regular(word: &[u8], offset: usize) -> Result<Token<'_>, SyntaxError> {
    let (negative, body) = match word.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, word),
    };
    let dots = body.iter().filter(|b| **b == b'.').count();
    let digits = body.iter().filter(|b| b.is_ascii_digit()).count();
    let numeric = digits > 0 && dots <= 1 && digits.saturating_add(dots) == body.len();
    if !numeric {
        return Ok(Token::Keyword(word));
    }
    if dots == 0 {
        let mut v: i64 = 0;
        for d in body {
            v = v
                .checked_mul(10)
                .and_then(|v| v.checked_add(i64::from(d - b'0')))
                .ok_or_else(|| malformed("integer out of range", offset))?;
        }
        return Ok(Token::Integer(if negative { -v } else { v }));
    }
    // Reals: normalise to a form `f64::from_str` always accepts ("0<int>.<frac>0").
    let mut text = String::with_capacity(body.len().saturating_add(3));
    if negative {
        text.push('-');
    }
    text.push('0');
    text.extend(body.iter().map(|b| char::from(*b)));
    text.push('0');
    let v: f64 = text
        .parse()
        .map_err(|_| malformed("invalid real number", offset))?;
    if !v.is_finite() {
        return Err(malformed("real number out of range", offset));
    }
    Ok(Token::Real(v))
}

/// Builds objects from tokens with bounded depth and a bounded number of produced objects.
pub(crate) struct Parser<'a, 'l> {
    pub(crate) lexer: Lexer<'a>,
    limits: &'l Limits,
    produced: usize,
}

impl<'a, 'l> Parser<'a, 'l> {
    pub(crate) fn new(data: &'a [u8], pos: usize, limits: &'l Limits) -> Self {
        Self {
            lexer: Lexer::new(data, pos),
            limits,
            produced: 0,
        }
    }

    pub(crate) fn parse_object(&mut self) -> Result<Object, SyntaxError> {
        let at = self.lexer.pos();
        let tok = self
            .lexer
            .next_token()?
            .ok_or_else(|| malformed("unexpected end of file", at))?;
        self.object_from(tok, at, 0)
    }

    fn count(&mut self) -> Result<(), SyntaxError> {
        self.produced = self.produced.saturating_add(1);
        if self.produced > self.limits.max_objects_per_parse {
            return Err(SyntaxError::LimitExceeded("objects in one parse"));
        }
        Ok(())
    }

    fn next_in_container(&mut self, what: &'static str) -> Result<(Token<'a>, usize), SyntaxError> {
        let at = self.lexer.pos();
        let tok = self
            .lexer
            .next_token()?
            .ok_or_else(|| malformed(what, at))?;
        Ok((tok, at))
    }

    fn object_from(
        &mut self,
        tok: Token<'a>,
        at: usize,
        depth: usize,
    ) -> Result<Object, SyntaxError> {
        self.count()?;
        Ok(match tok {
            Token::Integer(i) => self.maybe_reference(i, at)?,
            Token::Real(r) => Object::Real(r),
            Token::Name(n) => Object::Name(n),
            Token::String(s) => Object::String(s),
            Token::ArrayStart => {
                let depth = self.deeper(depth)?;
                let mut items = Vec::new();
                loop {
                    let (tok, at) = self.next_in_container("unterminated array")?;
                    if tok == Token::ArrayEnd {
                        break;
                    }
                    items.push(self.object_from(tok, at, depth)?);
                }
                Object::Array(items)
            }
            Token::DictStart => {
                let depth = self.deeper(depth)?;
                let mut entries = Vec::new();
                loop {
                    let (tok, at) = self.next_in_container("unterminated dictionary")?;
                    let key = match tok {
                        Token::DictEnd => break,
                        Token::Name(k) => k,
                        _ => return Err(malformed("dictionary key is not a name", at)),
                    };
                    let (tok, at) = self.next_in_container("unterminated dictionary")?;
                    if tok == Token::DictEnd {
                        return Err(malformed("dictionary key without value", at));
                    }
                    entries.push((key, self.object_from(tok, at, depth)?));
                }
                Object::Dict(Dict(entries))
            }
            Token::Keyword(b"true") => Object::Bool(true),
            Token::Keyword(b"false") => Object::Bool(false),
            Token::Keyword(b"null") => Object::Null,
            Token::Keyword(_) => return Err(malformed("unexpected keyword", at)),
            Token::ArrayEnd | Token::DictEnd => {
                return Err(malformed("unexpected container end", at));
            }
        })
    }

    fn deeper(&self, depth: usize) -> Result<usize, SyntaxError> {
        let next = depth.saturating_add(1);
        if next > self.limits.max_nesting_depth {
            return Err(SyntaxError::LimitExceeded("nesting depth"));
        }
        Ok(next)
    }

    /// ISO 32000-2 §7.3.10: `num gen R`.
    fn maybe_reference(&mut self, num: i64, at: usize) -> Result<Object, SyntaxError> {
        let save = self.lexer.pos();
        if let Ok(Some(Token::Integer(generation))) = self.lexer.next_token()
            && let Ok(Some(Token::Keyword(b"R"))) = self.lexer.next_token()
        {
            return match (u32::try_from(num), u16::try_from(generation)) {
                (Ok(num), Ok(generation)) => Ok(Object::Ref(ObjRef { num, generation })),
                _ => Err(malformed("invalid object reference", at)),
            };
        }
        self.lexer.set_pos(save);
        Ok(Object::Integer(num))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &[u8]) -> Result<Object, SyntaxError> {
        let limits = Limits::default();
        Parser::new(src, 0, &limits).parse_object()
    }

    #[test]
    fn scalars() {
        assert_eq!(parse(b"  42 "), Ok(Object::Integer(42)));
        assert_eq!(parse(b"-17"), Ok(Object::Integer(-17)));
        assert_eq!(parse(b"+.5"), Ok(Object::Real(0.5)));
        assert_eq!(parse(b"-3."), Ok(Object::Real(-3.0)));
        assert_eq!(parse(b"true"), Ok(Object::Bool(true)));
        assert_eq!(parse(b"null"), Ok(Object::Null));
        assert_eq!(parse(b"/A#20B"), Ok(Object::Name(b"A B".to_vec())));
        assert_eq!(parse(b"/A#2"), Ok(Object::Name(b"A#2".to_vec())));
    }

    #[test]
    fn strings() {
        assert_eq!(
            parse(b"(a(b)c\\)\\101\\\r\nd\re)"),
            Ok(Object::String(b"a(b)c)Ad\ne".to_vec()))
        );
        assert_eq!(parse(b"(\\777)"), Ok(Object::String(vec![0xFF])));
        assert_eq!(
            parse(b"<48 65 6C6C6F7>"),
            Ok(Object::String(b"Hellop".to_vec()))
        );
        assert!(parse(b"(abc").is_err());
        assert!(parse(b"<4G>").is_err());
    }

    #[test]
    fn containers_and_references() {
        let o = parse(b"<< /Kids [1 0 R 2 0 R 3] /N 5 /M 0 >>");
        let Ok(Object::Dict(d)) = o else {
            panic!("expected dict, got {o:?}");
        };
        assert_eq!(
            d.get(b"Kids"),
            Some(&Object::Array(vec![
                Object::Ref(ObjRef {
                    num: 1,
                    generation: 0
                }),
                Object::Ref(ObjRef {
                    num: 2,
                    generation: 0
                }),
                Object::Integer(3),
            ]))
        );
        assert_eq!(d.get(b"N"), Some(&Object::Integer(5)));
        assert_eq!(d.get(b"M"), Some(&Object::Integer(0)));
        // `5 0 >>` is an integer pair, not a reference, and `0` is not a valid key.
        assert!(parse(b"<< /N 5 0 >>").is_err());
    }

    #[test]
    fn hostile_numbers_and_structure() {
        assert!(matches!(
            parse(b"99999999999999999999"),
            Err(SyntaxError::Malformed { .. })
        ));
        assert!(parse(b"-1 0 R").is_err());
        assert!(parse(b"1 70000 R").is_err());
        assert!(parse(b"<< 1 2 >>").is_err());
        assert!(parse(b"<< /A >>").is_err());
        assert!(parse(b"]").is_err());
        assert!(parse(b"obj").is_err());
        assert!(parse(b"").is_err());
        let deep = vec![b'['; 100_000];
        assert_eq!(
            parse(&deep),
            Err(SyntaxError::LimitExceeded("nesting depth"))
        );
    }

    #[test]
    fn object_budget() {
        let limits = Limits {
            max_objects_per_parse: 3,
            ..Limits::default()
        };
        let r = Parser::new(b"[1 2 3]", 0, &limits).parse_object();
        assert_eq!(r, Err(SyntaxError::LimitExceeded("objects in one parse")));
    }
}
