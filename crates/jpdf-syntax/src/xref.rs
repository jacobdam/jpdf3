//! Classic cross-reference tables and trailers. ISO 32000-2 §7.5.4–§7.5.6.

use std::collections::{HashMap, HashSet};

use crate::error::{SyntaxError, malformed};
use crate::lexer::{Parser, Token};
use crate::limits::Limits;
use crate::object::{Dict, ObjRef, Object};

/// How far from the end of the file `startxref` is searched for.
const STARTXREF_WINDOW: usize = 1024;

/// One xref entry: `Some((generation, offset))` for an in-use object, `None` for a free one.
type Entry = Option<(u16, usize)>;

/// A file whose cross-reference information has been read.
#[derive(Debug)]
pub struct XrefDocument<'a> {
    data: &'a [u8],
    limits: Limits,
    entries: HashMap<u32, Entry>,
    trailer: Dict,
}

impl<'a> XrefDocument<'a> {
    /// Reads `startxref`, then every cross-reference section reachable through `/Prev`.
    pub fn parse(data: &'a [u8], limits: &Limits) -> Result<Self, SyntaxError> {
        if data.len() > limits.max_file_size {
            return Err(SyntaxError::LimitExceeded("file size"));
        }
        let mut offset = find_startxref(data, limits)?;
        let mut entries: HashMap<u32, Entry> = HashMap::new();
        let mut trailer: Option<Dict> = None;
        let mut visited = HashSet::new();
        let mut total_entries: usize = 0;
        loop {
            if !visited.insert(offset) {
                return Err(malformed("/Prev loop in cross-reference chain", offset));
            }
            if visited.len() > limits.max_xref_sections {
                return Err(SyntaxError::LimitExceeded("cross-reference sections"));
            }
            let (section, section_trailer) =
                parse_section(data, offset, limits, &mut total_entries)?;
            // ISO 32000-2 §7.5.6: newer sections (read first) take precedence.
            for (num, entry) in section {
                entries.entry(num).or_insert(entry);
            }
            if section_trailer.get(b"XRefStm").is_some() {
                return Err(SyntaxError::Unsupported(
                    "hybrid cross-reference (/XRefStm)",
                ));
            }
            let prev = section_trailer.get(b"Prev").cloned();
            if trailer.is_none() {
                trailer = Some(section_trailer);
            }
            match prev {
                None => break,
                Some(Object::Integer(p)) => {
                    offset = usize::try_from(p)
                        .map_err(|_| malformed("negative /Prev offset", offset))?;
                }
                Some(_) => return Err(malformed("/Prev is not an integer", offset)),
            }
        }
        let trailer = trailer.ok_or_else(|| malformed("no trailer", data.len()))?;
        if trailer.get(b"Encrypt").is_some() {
            return Err(SyntaxError::Unsupported("encrypted documents"));
        }
        Ok(Self {
            data,
            limits: limits.clone(),
            entries,
            trailer,
        })
    }

    /// The newest trailer dictionary.
    pub fn trailer(&self) -> &Dict {
        &self.trailer
    }

    /// Resolves an indirect reference. ISO 32000-2 §7.3.10: a reference to a free or missing
    /// object is the null object.
    pub fn resolve(&self, r: ObjRef) -> Result<Object, SyntaxError> {
        let Some(Some((generation, offset))) = self.entries.get(&r.num).copied() else {
            return Ok(Object::Null);
        };
        if generation != r.generation {
            return Ok(Object::Null);
        }
        let mut p = Parser::new(self.data, offset, &self.limits);
        let header = (
            p.lexer.next_token()?,
            p.lexer.next_token()?,
            p.lexer.next_token()?,
        );
        match header {
            (Some(Token::Integer(n)), Some(Token::Integer(g)), Some(Token::Keyword(b"obj")))
                if n == i64::from(r.num) && g == i64::from(r.generation) => {}
            _ => return Err(malformed("object header does not match xref", offset)),
        }
        // `endobj` (or `stream`) is not required: the value is all we need.
        p.parse_object()
    }
}

/// ISO 32000-2 §7.5.5: the last `startxref` keyword and the byte offset after it.
fn find_startxref(data: &[u8], limits: &Limits) -> Result<usize, SyntaxError> {
    const KEY: &[u8] = b"startxref";
    let from = data.len().saturating_sub(STARTXREF_WINDOW);
    let tail = data.get(from..).unwrap_or_default();
    let at = tail
        .windows(KEY.len())
        .rposition(|w| w == KEY)
        .and_then(|i| i.checked_add(from))
        .ok_or_else(|| malformed("startxref not found", data.len()))?;
    let mut p = Parser::new(data, at.saturating_add(KEY.len()), limits);
    let offset = match p.lexer.next_token()? {
        Some(Token::Integer(o)) => usize::try_from(o).ok(),
        _ => None,
    };
    match offset {
        Some(o) if o < data.len() => Ok(o),
        _ => Err(malformed("invalid startxref offset", at)),
    }
}

type Section = (Vec<(u32, Entry)>, Dict);

/// ISO 32000-2 §7.5.4: `xref`, subsections `start count` + entries, then `trailer << … >>`.
fn parse_section(
    data: &[u8],
    offset: usize,
    limits: &Limits,
    total_entries: &mut usize,
) -> Result<Section, SyntaxError> {
    let mut p = Parser::new(data, offset, limits);
    match p.lexer.next_token()? {
        Some(Token::Keyword(b"xref")) => {}
        Some(Token::Integer(_)) => {
            return Err(SyntaxError::Unsupported("cross-reference streams"));
        }
        _ => return Err(malformed("expected 'xref'", offset)),
    }
    let mut entries = Vec::new();
    loop {
        let at = p.lexer.pos();
        let start = match p.lexer.next_token()? {
            Some(Token::Keyword(b"trailer")) => break,
            Some(Token::Integer(s)) => {
                u32::try_from(s).map_err(|_| malformed("invalid subsection start", at))?
            }
            _ => return Err(malformed("expected xref subsection or 'trailer'", at)),
        };
        let count = match p.lexer.next_token()? {
            Some(Token::Integer(c)) => {
                usize::try_from(c).map_err(|_| malformed("invalid subsection count", at))?
            }
            _ => return Err(malformed("expected xref subsection count", at)),
        };
        *total_entries = total_entries
            .checked_add(count)
            .filter(|t| *t <= limits.max_xref_entries)
            .ok_or(SyntaxError::LimitExceeded("cross-reference entries"))?;
        let mut num = start;
        for i in 0..count {
            let at = p.lexer.pos();
            let entry = (
                p.lexer.next_token()?,
                p.lexer.next_token()?,
                p.lexer.next_token()?,
            );
            let parsed = match entry {
                (Some(Token::Integer(o)), Some(Token::Integer(g)), Some(Token::Keyword(k))) => {
                    match (usize::try_from(o), u16::try_from(g), k) {
                        (Ok(o), Ok(g), b"n") => Some(Some((g, o))),
                        (_, Ok(_), b"f") => Some(None),
                        _ => None,
                    }
                }
                _ => None,
            };
            let Some(parsed) = parsed else {
                return Err(malformed("invalid xref entry", at));
            };
            entries.push((num, parsed));
            if i.saturating_add(1) < count {
                num = num
                    .checked_add(1)
                    .ok_or_else(|| malformed("object number out of range", at))?;
            }
        }
    }
    let at = p.lexer.pos();
    match p.parse_object()? {
        Object::Dict(d) => Ok((entries, d)),
        _ => Err(malformed("trailer is not a dictionary", at)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startxref_must_point_inside_the_file() {
        let l = Limits::default();
        assert!(find_startxref(b"startxref\n999\n%%EOF", &l).is_err());
        assert!(find_startxref(b"startxref\n-1\n%%EOF", &l).is_err());
        assert!(find_startxref(b"nothing here", &l).is_err());
        assert_eq!(find_startxref(b"xref startxref 0 %%EOF", &l), Ok(0));
    }

    #[test]
    fn huge_subsection_count_hits_the_limit_before_allocating() {
        let l = Limits::default();
        let data = b"xref\n0 99999999999\ntrailer\n<<>>\nstartxref\n0\n%%EOF";
        assert_eq!(
            XrefDocument::parse(data, &l).map(|_| ()),
            Err(SyntaxError::LimitExceeded("cross-reference entries"))
        );
    }

    #[test]
    fn xref_stream_is_unsupported() {
        let l = Limits::default();
        let data = b"1 0 obj\n<</Type/XRef>>\nendobj\nstartxref\n0\n%%EOF";
        assert_eq!(
            XrefDocument::parse(data, &l).map(|_| ()),
            Err(SyntaxError::Unsupported("cross-reference streams"))
        );
    }
}
