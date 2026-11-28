//! Tokenizer and object parser. ISO 32000-2 §7.2 (lexical conventions), §7.3 (objects).

use std::cell::Cell;

use crate::Limits;
use crate::error::{SyntaxError, malformed};
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

/// Value of an ASCII octal digit; callers match `b'0'..=b'7'` first.
fn octal_value(b: u8) -> u16 {
    char::from(b)
        .to_digit(8)
        .and_then(|d| u16::try_from(d).ok())
        .unwrap_or_default()
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' => char::from(b)
            .to_digit(16)
            .and_then(|d| u8::try_from(d).ok()),
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

/// Bytes lexed so far for one document, against `Limits::lex_budget`. Every byte the lexer
/// steps over is charged, including look-ahead that is later re-read, so lexing fails while
/// scanning rather than after. Objects and xref sections may overlap in hostile files (an
/// object hidden inside another's string, a trailer whose `/Prev` points into its own string);
/// without one budget per document, resolving them rescans the same bytes quadratically.
#[derive(Debug)]
///
/// It also counts objects produced across all parses of the document: non-overlapping objects
/// can still be cheap to write and expensive to build (`[]` is 2 bytes), so time is bounded by
/// objects as well as bytes.
pub(crate) struct LexBudget {
    used: Cell<usize>,
    max: usize,
    objects: Cell<usize>,
    max_objects: usize,
}

impl LexBudget {
    pub(crate) fn new(data_len: usize, limits: &Limits) -> Self {
        Self {
            used: Cell::new(0),
            max: limits.lex_budget(data_len),
            objects: Cell::new(0),
            max_objects: limits.max_objects_per_document,
        }
    }

    fn charge_object(&self) -> Result<(), SyntaxError> {
        let n = self.objects.get().saturating_add(1);
        self.objects.set(n);
        if n > self.max_objects {
            return Err(SyntaxError::LimitExceeded("objects per document"));
        }
        Ok(())
    }

    /// Bytes lexed so far (tests: proves whether an object was re-parsed).
    #[cfg(test)]
    pub(crate) fn used(&self) -> usize {
        self.used.get()
    }

    fn charge_one(&self) {
        self.used.set(self.used.get().saturating_add(1));
    }

    fn check(&self) -> Result<(), SyntaxError> {
        if self.used.get() > self.max {
            return Err(SyntaxError::LimitExceeded("bytes lexed per document"));
        }
        Ok(())
    }
}

pub(crate) struct Lexer<'a, 'b> {
    data: &'a [u8],
    pos: usize,
    budget: &'b LexBudget,
    /// Decoded string and name bytes still allowed in the current parse (memory bound).
    string_room: usize,
    /// The last two whitespace-and-comment runs skipped, as `(from, to)`, newest first. The
    /// reference look-ahead rewinds over up to two such runs (before the generation and before
    /// `R`), so a later skip from the same position jumps to `to` instead of scanning the run
    /// again (#113: a long comment after `1 1` was scanned 3 times).
    skipped: [Option<(usize, usize)>; 2],
}

impl<'a, 'b> Lexer<'a, 'b> {
    pub(crate) fn new(
        data: &'a [u8],
        pos: usize,
        budget: &'b LexBudget,
        string_room: usize,
    ) -> Self {
        Self {
            data,
            pos,
            budget,
            string_room,
            skipped: [None, None],
        }
    }

    fn check(&self) -> Result<(), SyntaxError> {
        self.budget.check()
    }

    /// Fails if the per-document lexing budget is spent. `next_token` checks it before each
    /// token, so a caller that decides from the last token it read checks it again here.
    pub(crate) fn check_budget(&self) -> Result<(), SyntaxError> {
        self.check()
    }

    /// Fails once a string or name being decoded exceeds the remaining room for this parse.
    fn check_string(&self, out: &[u8]) -> Result<(), SyntaxError> {
        if out.len() > self.string_room {
            return Err(SyntaxError::LimitExceeded("string bytes in one object"));
        }
        self.check()
    }

    /// Charges a finished string or name to the room for this parse. The checks inside the
    /// decoding loops (`check_string`) bound memory: they stop a long token mid-scan, at most one
    /// byte past the room, and must stay. They run before each push, so they miss the last byte
    /// (a name's final byte, an odd hex string's implied 0); this final check makes the limit
    /// exact for every token: a total of exactly `max_string_bytes_per_parse` passes, one byte
    /// more fails (#113).
    fn take_string(&mut self, out: Vec<u8>) -> Result<Vec<u8>, SyntaxError> {
        if out.len() > self.string_room {
            return Err(SyntaxError::LimitExceeded("string bytes in one object"));
        }
        self.string_room = self.string_room.saturating_sub(out.len());
        Ok(out)
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Advances one byte. Callers only call this after `peek` returned `Some`, so `pos` stays
    /// `<= data.len()`, which can never overflow.
    fn bump(&mut self) {
        self.pos = self.pos.saturating_add(1);
        self.budget.charge_one();
    }

    /// ISO 32000-2 §7.2.4: comments run from `%` to the end of the line.
    fn skip_whitespace_and_comments(&mut self) -> Result<(), SyntaxError> {
        let from = self.pos;
        if let Some((_, end)) = self
            .skipped
            .iter()
            .flatten()
            .find(|(start, _)| *start == from)
        {
            // Already scanned (and charged) from here; the input doesn't change.
            self.pos = *end;
            return Ok(());
        }
        self.skip_run()?;
        let [newest, _] = self.skipped;
        self.skipped = [Some((from, self.pos)), newest];
        Ok(())
    }

    fn skip_run(&mut self) -> Result<(), SyntaxError> {
        while let Some(b) = self.peek() {
            self.check()?;
            if is_whitespace(b) {
                self.bump();
            } else if b == b'%' {
                while let Some(c) = self.peek() {
                    if c == b'\r' || c == b'\n' {
                        break;
                    }
                    self.bump();
                    self.check()?;
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Steps over a run of regular characters and returns it.
    fn regular_run(&mut self) -> Result<&'a [u8], SyntaxError> {
        let start = self.pos;
        while self.peek().is_some_and(is_regular) {
            self.bump();
            self.check()?;
        }
        Ok(self.data.get(start..self.pos).unwrap_or_default())
    }

    /// Steps over a run of regular characters if it is at most `max` bytes long. A longer run
    /// steps over `max` bytes and returns `None` if byte `max + 1` is also regular (the caller
    /// rewinds).
    fn short_regular_run(&mut self, max: usize) -> Result<Option<&'a [u8]>, SyntaxError> {
        let start = self.pos;
        let mut len: usize = 0;
        while self.peek().is_some_and(is_regular) {
            if len == max {
                return Ok(None);
            }
            len = len.saturating_add(1);
            self.bump();
            self.check()?;
        }
        Ok(self.data.get(start..self.pos))
    }

    /// After an integer: if `gen R` follows (ISO 32000-2 §7.3.10), consumes it and returns the
    /// generation token value. Allocation-free: never decodes strings during look-ahead.
    ///
    /// The look-ahead is bounded (#113): a generation token longer than
    /// `MAX_GENERATION_TOKEN` bytes, or a third token other than exactly `R`, is not a reference,
    /// and the scan stops there. So a long token after an integer is stepped over and classified
    /// once, as itself, rather than up to three times by the look-ahead of the integers before it.
    pub(crate) fn reference_tail(&mut self) -> Result<Option<i64>, SyntaxError> {
        let save = self.pos;
        self.skip_whitespace_and_comments()?;
        let at = self.pos;
        let generation = match self.short_regular_run(MAX_GENERATION_TOKEN)? {
            Some(word) => match classify_regular(word, at) {
                Ok(Token::Integer(g)) => Some(g),
                _ => None,
            },
            None => None,
        };
        if let Some(g) = generation {
            self.skip_whitespace_and_comments()?;
            if self.short_regular_run(1)? == Some(b"R") {
                return Ok(Some(g));
            }
        }
        self.pos = save;
        Ok(None)
    }

    /// Returns the next token, or `None` at end of input.
    pub(crate) fn next_token(&mut self) -> Result<Option<Token<'a>>, SyntaxError> {
        self.skip_whitespace_and_comments()?;
        self.check()?;
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
                    let s = self.hex_string()?;
                    Token::String(self.take_string(s)?)
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
                let s = self.literal_string(start)?;
                Token::String(self.take_string(s)?)
            }
            b'/' => {
                self.bump();
                let n = self.name()?;
                Token::Name(self.take_string(n)?)
            }
            b')' | b'{' | b'}' => return Err(malformed("unexpected delimiter", start)),
            _ => {
                let word = self.regular_run()?;
                classify_regular(word, start)?
            }
        };
        Ok(Some(tok))
    }

    /// ISO 32000-2 §7.3.5: `#xx` escapes in names.
    fn name(&mut self) -> Result<Vec<u8>, SyntaxError> {
        let mut out = Vec::new();
        while let Some(b) = self.peek().filter(|b| is_regular(*b)) {
            self.check_string(&out)?;
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
        Ok(out)
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
            self.check_string(&out)?;
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
            self.check_string(&out)?;
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
                let mut v = octal_value(c);
                for _ in 0..2 {
                    match self.peek() {
                        Some(d @ b'0'..=b'7') => {
                            self.bump();
                            // At most three octal digits: v ≤ 0o777, so this never saturates.
                            v = v.saturating_mul(8).saturating_add(octal_value(d));
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

/// Longest token read as a generation number in `num gen R` (#113). A generation is a
/// non-negative integer that fits `u16` (ISO 32000-2 §7.3.10, §7.5.4: at most 65 535), so 5 digits;
/// the bound leaves room for a sign and leading zeros as written by lenient producers.
const MAX_GENERATION_TOKEN: usize = 64;

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
        // Accumulate with the token's sign so that `i64::MIN` is representable.
        let mut v: i64 = 0;
        for d in body {
            let digit = i64::from(char::from(*d).to_digit(10).unwrap_or_default());
            v = v
                .checked_mul(10)
                .and_then(|v| {
                    if negative {
                        v.checked_sub(digit)
                    } else {
                        v.checked_add(digit)
                    }
                })
                .ok_or_else(|| malformed("integer out of range", offset))?;
        }
        return Ok(Token::Integer(v));
    }
    // Reals: the token is parsed in place, without a copy (a copy of a huge digit run was an
    // uncharged allocation the size of the token). It is `[+-]? digits? . digits?` with at least
    // one digit, which `f64::from_str` accepts as is ("5.", ".5", "+.5", "-3.").
    let v: f64 = std::str::from_utf8(word)
        .ok()
        .and_then(|text| text.parse().ok())
        .ok_or_else(|| malformed("invalid real number", offset))?;
    if !v.is_finite() {
        return Err(malformed("real number out of range", offset));
    }
    Ok(Token::Real(v))
}

/// Builds objects from tokens with bounded depth and a bounded number of produced objects.
pub(crate) struct Parser<'a, 'b> {
    pub(crate) lexer: Lexer<'a, 'b>,
    limits: &'b Limits,
    produced: usize,
}

impl<'a, 'b> Parser<'a, 'b> {
    pub(crate) fn new(
        data: &'a [u8],
        pos: usize,
        limits: &'b Limits,
        budget: &'b LexBudget,
    ) -> Self {
        Self {
            lexer: Lexer::new(data, pos, budget, limits.max_string_bytes_per_parse),
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
        self.lexer.budget.charge_object()
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
        let Some(generation) = self.lexer.reference_tail()? else {
            return Ok(Object::Integer(num));
        };
        match (u32::try_from(num), u16::try_from(generation)) {
            (Ok(num), Ok(generation)) => Ok(Object::Ref(ObjRef { num, generation })),
            _ => Err(malformed("invalid object reference", at)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &[u8]) -> Result<Object, SyntaxError> {
        let limits = Limits::default();
        let budget = LexBudget::new(src.len(), &limits);
        Parser::new(src, 0, &limits, &budget).parse_object()
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

    /// Issue #77: integers span exactly `i64::MIN..=i64::MAX`; one past either end is malformed.
    #[test]
    fn integer_range_boundaries() {
        assert_eq!(parse(b"9223372036854775807"), Ok(Object::Integer(i64::MAX)));
        assert_eq!(
            parse(b"+9223372036854775807"),
            Ok(Object::Integer(i64::MAX))
        );
        assert_eq!(
            parse(b"-9223372036854775808"),
            Ok(Object::Integer(i64::MIN))
        );
        assert_eq!(parse(b"-0"), Ok(Object::Integer(0)));
        assert_eq!(parse(b"007"), Ok(Object::Integer(7)));
        for past in [&b"9223372036854775808"[..], b"-9223372036854775809"] {
            assert!(
                matches!(parse(past), Err(SyntaxError::Malformed { .. })),
                "{past:?}"
            );
        }
    }

    /// Issue #77: reals are parsed in place (no copy of the token). Every accepted shape still
    /// parses to the same value, and a very long real is read correctly.
    #[test]
    fn reals_parse_in_place() {
        let cases: &[(&[u8], f64)] = &[
            (b"5.", 5.0),
            (b".5", 0.5),
            (b"-.5", -0.5),
            (b"+.5", 0.5),
            (b"-3.", -3.0),
            (b"+3.25", 3.25),
            (b"0.0", 0.0),
            (b"-0.", -0.0),
            (b"123.456", 123.456),
        ];
        for (src, want) in cases {
            assert_eq!(parse(src), Ok(Object::Real(*want)), "{src:?}");
        }
        assert_eq!(
            parse(b"-0.").map(|o| matches!(o, Object::Real(r) if r.is_sign_negative())),
            Ok(true)
        );
        // 1 MB of digits: 1.000…0001 rounds to 1.0, 0.000…1 underflows to 0.0.
        let mut long = b"1.".to_vec();
        long.extend(std::iter::repeat_n(b'0', 1_000_000));
        long.push(b'1');
        assert_eq!(parse(&long), Ok(Object::Real(1.0)));
        let mut tiny = b"-.".to_vec();
        tiny.extend(std::iter::repeat_n(b'0', 1_000_000));
        tiny.push(b'7');
        assert_eq!(parse(&tiny), Ok(Object::Real(-0.0)));
        // A huge integer part overflows f64: out of range, not infinity.
        let mut huge = vec![b'9'; 400];
        huge.push(b'.');
        assert!(matches!(parse(&huge), Err(SyntaxError::Malformed { .. })));
        // Not numbers: keywords, so a lone object is malformed.
        for src in [&b"."[..], b"-.", b"1.2.3", b"+-1", b"1e5", b"inf", b"NaN"] {
            assert!(parse(src).is_err(), "{src:?}");
        }
    }

    /// Octal escapes take at most three digits; high-order overflow is ignored (§7.3.4.2).
    #[test]
    fn octal_escape_bounds() {
        assert_eq!(parse(b"(\\0)"), Ok(Object::String(vec![0])));
        assert_eq!(parse(b"(\\7)"), Ok(Object::String(vec![7])));
        assert_eq!(parse(b"(\\377)"), Ok(Object::String(vec![0xFF])));
        assert_eq!(parse(b"(\\400)"), Ok(Object::String(vec![0])));
        assert_eq!(parse(b"(\\1234)"), Ok(Object::String(b"S4".to_vec())));
        assert_eq!(parse(b"(\\18)"), Ok(Object::String(vec![1, b'8'])));
        assert_eq!(
            parse(b"<0a1B fF>"),
            Ok(Object::String(vec![0x0A, 0x1B, 0xFF]))
        );
    }

    #[test]
    fn object_budget() {
        let limits = Limits {
            max_objects_per_parse: 3,
            ..Limits::default()
        };
        let budget = LexBudget::new(7, &limits);
        let r = Parser::new(b"[1 2 3]", 0, &limits, &budget).parse_object();
        assert_eq!(r, Err(SyntaxError::LimitExceeded("objects in one parse")));
    }

    #[test]
    fn objects_per_document_are_bounded_across_parses() {
        let limits = Limits {
            max_objects_per_document: 5,
            ..Limits::default()
        };
        let budget = LexBudget::new(16, &limits);
        assert!(
            Parser::new(b"[1 2]", 0, &limits, &budget)
                .parse_object()
                .is_ok()
        );
        let r = Parser::new(b"[1 2]", 0, &limits, &budget).parse_object();
        assert_eq!(r, Err(SyntaxError::LimitExceeded("objects per document")));
    }

    #[test]
    fn string_bytes_per_parse_are_bounded() {
        let limits = Limits {
            max_string_bytes_per_parse: 8,
            ..Limits::default()
        };
        let src = b"[(abcd) <61626364> (e)]";
        let budget = LexBudget::new(src.len(), &limits);
        let r = Parser::new(src, 0, &limits, &budget).parse_object();
        assert_eq!(
            r,
            Err(SyntaxError::LimitExceeded("string bytes in one object"))
        );
        let budget = LexBudget::new(src.len(), &limits);
        let r = Parser::new(b"[(abcd) <61626364>]", 0, &limits, &budget).parse_object();
        assert!(r.is_ok());
        let budget = LexBudget::new(src.len(), &limits);
        let r = Parser::new(b"[/abcdef (abc)]", 0, &limits, &budget).parse_object();
        assert_eq!(
            r,
            Err(SyntaxError::LimitExceeded("string bytes in one object"))
        );
    }

    fn parse_with_room(src: &[u8], room: usize) -> Result<Object, SyntaxError> {
        let limits = Limits {
            max_string_bytes_per_parse: room,
            ..Limits::default()
        };
        let budget = LexBudget::new(src.len(), &limits);
        Parser::new(src, 0, &limits, &budget).parse_object()
    }

    const STRING_LIMIT: Result<Object, SyntaxError> =
        Err(SyntaxError::LimitExceeded("string bytes in one object"));

    /// Issue #113: every token kind decodes exactly up to the room. At the limit the object
    /// parses with the right bytes; one decoded byte more is the limit error.
    #[test]
    fn string_room_is_exact_for_every_token_kind() {
        let name = |b: &[u8]| Ok(Object::Name(b.to_vec()));
        let string = |b: &[u8]| Ok(Object::String(b.to_vec()));
        // (at the limit, its result, one decoded byte past the limit)
        type Case<'a> = (&'a [u8], Result<Object, SyntaxError>, &'a [u8]);
        let cases: &[Case<'_>] = &[
            // Names, including `#xx` escapes (one decoded byte per escape).
            (b"/abcd", name(b"abcd"), b"/abcde"),
            (b"/a#62c#64", name(b"abcd"), b"/a#62c#64e"),
            (b"/abc#64", name(b"abcd"), b"/abc#64#65"),
            // Hex strings, even and odd (the odd one's last byte is the implied 0).
            (b"<61626364>", string(b"abcd"), b"<6162636465>"),
            (b"<6162636>", string(b"abc`"), b"<616263646>"),
            (b"<61 62 63 6 >", string(b"abc`"), b"<61 62 63 64 6 >"),
            // Literal strings, with an escape last and an end-of-line pair last.
            (b"(abcd)", string(b"abcd"), b"(abcde)"),
            (b"(abc\\\\)", string(b"abc\\"), b"(abcd\\\\)"),
            (b"(abc\r\n)", string(b"abc\n"), b"(abcd\r\n)"),
        ];
        for (at, want, past) in cases {
            assert_eq!(&parse_with_room(at, 4), want, "{}", at.escape_ascii());
            assert_eq!(
                parse_with_room(past, 4),
                STRING_LIMIT,
                "{}",
                past.escape_ascii()
            );
        }
        // An empty token fits a zero room; a 1-byte one does not.
        assert_eq!(parse_with_room(b"/", 0), name(b""));
        assert_eq!(parse_with_room(b"<>", 0), string(b""));
        assert_eq!(parse_with_room(b"()", 0), string(b""));
        for past in [&b"/a"[..], b"<4>", b"<41>", b"(a)"] {
            assert_eq!(parse_with_room(past, 0), STRING_LIMIT, "{past:?}");
        }
    }

    /// Issue #113 review: the in-loop checks stop a long token mid-scan (the memory bound), so
    /// an over-long string, name or hex string fails with the limit error after a few bytes,
    /// never reaching the unterminated end or decoding the whole token.
    #[test]
    fn string_room_stops_the_scan_early() {
        let tail = 100_000;
        let cases: [Vec<u8>; 3] = [
            [&b"(abcde"[..], &vec![b'x'; tail]].concat(),
            [&b"/abcde"[..], &vec![b'x'; tail]].concat(),
            [&b"<6162636465"[..], &b"61".repeat(tail)].concat(),
        ];
        for src in cases {
            let limits = Limits {
                max_string_bytes_per_parse: 4,
                ..Limits::default()
            };
            let budget = LexBudget::new(src.len(), &limits);
            let r = Parser::new(&src, 0, &limits, &budget).parse_object();
            assert_eq!(r, STRING_LIMIT, "{:?}", src.get(..6));
            let used = budget.used.get();
            assert!(used <= 16, "{:?}: lexed {used} bytes", src.get(..6));
        }
    }

    /// Issue #113: the room is shared by every token of one object, so once it is used up, each
    /// further 1-byte name or odd hex string is refused (they used to add a byte each).
    #[test]
    fn string_room_is_shared_and_exact_across_tokens() {
        for (unit, decoded) in [(&b"/a "[..], &b"a"[..]), (b"<4> ", b"@"), (b"(a) ", b"a")] {
            let src = |n: usize| {
                let mut s = b"[".to_vec();
                for _ in 0..n {
                    s.extend_from_slice(unit);
                }
                s.push(b']');
                s
            };
            let r = parse_with_room(&src(18), 18);
            let Ok(Object::Array(items)) = r else {
                panic!("{unit:?}: expected an array, got {r:?}");
            };
            assert_eq!(items.len(), 18, "{unit:?}");
            assert!(
                items.iter().all(|o| match o {
                    Object::Name(b) | Object::String(b) => b == decoded,
                    _ => false,
                }),
                "{unit:?}"
            );
            assert_eq!(parse_with_room(&src(19), 18), STRING_LIMIT, "{unit:?}");
            assert_eq!(parse_with_room(&src(1000), 18), STRING_LIMIT, "{unit:?}");
        }
    }

    /// Issue #113: the reference look-ahead steps over a long token after an integer only up to
    /// its bound, so the token is lexed once (as itself), not three times.
    #[test]
    fn reference_look_ahead_is_bounded() {
        // An integer, a real and a keyword, each 100 001 bytes long.
        for (lead, fill) in [(b'0', b'0'), (b'.', b'1'), (b'x', b'x')] {
            let mut src = b"[1 1 ".to_vec();
            src.push(lead);
            src.extend(std::iter::repeat_n(fill, 100_000));
            src.push(b']');
            let long = char::from(lead);
            let limits = Limits::default();
            let budget = LexBudget::new(src.len(), &limits);
            let r = Parser::new(&src, 0, &limits, &budget).parse_object();
            assert_eq!(r.is_ok(), lead != b'x', "{long:?}: {r:?}");
            let used = budget.used.get();
            // One pass over the input, plus the second `1`'s look-ahead into the long token (it
            // steps over `MAX` bytes and peeks at one more; the first `1`'s look-ahead reads the
            // second `1` as the generation and stops after 1 byte of the `R` candidate) and
            // 1-byte re-reads.
            // Measured: `MAX + 2` (`MAX + 1` for the keyword).
            let slack = MAX_GENERATION_TOKEN.saturating_add(4);
            assert!(
                used <= src.len().saturating_add(slack),
                "{long:?}: lexed {used} bytes of {}",
                src.len()
            );
        }
    }

    /// Issue #113 review: the look-ahead rewinds over whitespace and comments too, so a long run
    /// of either after an integer was scanned up to 3 times. Each run is now scanned once, also
    /// when one look-ahead crosses two long runs, and references across them still resolve.
    #[test]
    fn whitespace_and_comments_are_scanned_once() {
        let ws = vec![b' '; 100_000];
        let mut comment = b"%".to_vec();
        comment.extend(vec![b'c'; 100_000]);
        comment.push(b'\n');
        let cases: [(Vec<u8>, Object); 2] = [
            (
                [&b"[1"[..], &ws, b"1", &comment, b"1", &ws, b"]"].concat(),
                Object::Array(vec![Object::Integer(1); 3]),
            ),
            (
                [&b"[7"[..], &ws, b"3", &comment, b"R]"].concat(),
                Object::Array(vec![Object::Ref(ObjRef {
                    num: 7,
                    generation: 3,
                })]),
            ),
        ];
        for (src, want) in cases {
            let limits = Limits::default();
            let budget = LexBudget::new(src.len(), &limits);
            let r = Parser::new(&src, 0, &limits, &budget).parse_object();
            assert_eq!(r, Ok(want));
            let used = budget.used.get();
            // Only the 1-byte tokens between the runs are re-read.
            assert!(
                used <= src.len().saturating_add(8),
                "lexed {used} bytes of {}",
                src.len()
            );
        }
    }

    /// Issue #113: a generation token of up to `MAX_GENERATION_TOKEN` bytes still makes a
    /// reference; one byte longer does not (so `R` is then an unexpected keyword).
    #[test]
    fn generation_token_length_boundary() {
        let src = |gen_len: usize| {
            let mut s = b"[7 ".to_vec();
            s.extend(std::iter::repeat_n(b'0', gen_len.saturating_sub(1)));
            s.extend(b"3 R]");
            s
        };
        let r7_3 = Object::Ref(ObjRef {
            num: 7,
            generation: 3,
        });
        assert_eq!(MAX_GENERATION_TOKEN, 64);
        assert_eq!(parse(&src(1)), Ok(Object::Array(vec![r7_3.clone()])));
        assert_eq!(parse(&src(64)), Ok(Object::Array(vec![r7_3])));
        // `7`, a 65-byte integer, then a stray `R` (the parser reports the offset where it
        // started reading the token, before the space: 3 + 65).
        assert_eq!(parse(&src(65)), Err(malformed("unexpected keyword", 68)));
        // A signed generation still reads as before, and `R` must stand alone.
        assert_eq!(
            parse(b"[7 +3 R]"),
            Ok(Object::Array(vec![Object::Ref(ObjRef {
                num: 7,
                generation: 3
            })]))
        );
        assert_eq!(parse(b"[7 3 Rx]"), Err(malformed("unexpected keyword", 4)));
        assert_eq!(
            parse(b"[7 3]"),
            Ok(Object::Array(vec![Object::Integer(7), Object::Integer(3)]))
        );
        assert_eq!(
            parse(b"[7 3 /R]"),
            Ok(Object::Array(vec![
                Object::Integer(7),
                Object::Integer(3),
                Object::Name(b"R".to_vec())
            ]))
        );
    }

    #[test]
    fn lexing_fails_mid_scan_once_the_budget_is_spent() {
        let limits = Limits {
            lex_budget_factor: 0,
            lex_budget_slack: 10,
            ..Limits::default()
        };
        let mut src = b"(".to_vec();
        src.extend(vec![b'x'; 1000]);
        src.push(b')');
        let budget = LexBudget::new(src.len(), &limits);
        let r = Parser::new(&src, 0, &limits, &budget).parse_object();
        assert_eq!(
            r,
            Err(SyntaxError::LimitExceeded("bytes lexed per document"))
        );
        // Charged per byte: the scan stopped right after the budget, not at the end.
        assert!(budget.used.get() <= 12);
    }

    #[test]
    fn reference_look_ahead_does_not_decode_strings() {
        let limits = Limits {
            max_string_bytes_per_parse: 4,
            ..Limits::default()
        };
        // `1 (abcd)`: the look-ahead after `1` must not count the string twice.
        let src = b"[1 (abcd)]";
        let budget = LexBudget::new(src.len(), &limits);
        let r = Parser::new(src, 0, &limits, &budget).parse_object();
        assert_eq!(
            r,
            Ok(Object::Array(vec![
                Object::Integer(1),
                Object::String(b"abcd".to_vec())
            ]))
        );
    }
}
