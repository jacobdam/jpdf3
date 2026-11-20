//! Classic cross-reference tables and trailers. ISO 32000-2 §7.5.4–§7.5.6.

use std::collections::HashSet;

use crate::error::{SyntaxError, malformed};
use crate::lexer::{LexBudget, Parser, Token};
use crate::limits::Limits;
use crate::object::{Dict, ObjRef, Object};

/// How far from the end of the file `startxref` is searched for. ISO 32000-2 §7.5.5 puts it
/// in the last lines; real files often carry trailing junk, so read leniently.
const STARTXREF_WINDOW: usize = 64 * 1024;

/// One xref entry (ISO 32000-2 §7.5.4), packed into 12 bytes so 10M entries cost ~120 MB.
/// Offsets fit in `u32` because files are far below 4 GiB (`Limits::max_file_size`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    num: u32,
    offset: u32,
    generation: u16,
    in_use: bool,
}

/// A file whose cross-reference information has been read.
#[derive(Debug)]
pub struct XrefDocument<'a> {
    data: &'a [u8],
    limits: Limits,
    /// Sorted by object number, one entry per number (the newest).
    entries: Vec<Entry>,
    trailer: Dict,
    budget: LexBudget,
}

impl<'a> XrefDocument<'a> {
    /// Reads `startxref`, then every cross-reference section reachable through `/Prev`.
    pub fn parse(data: &'a [u8], limits: &Limits) -> Result<Self, SyntaxError> {
        if data.len() > limits.max_file_size {
            return Err(SyntaxError::LimitExceeded("file size"));
        }
        let budget = LexBudget::new(data.len(), limits);
        let mut offset = find_startxref(data, limits, &budget)?;
        let mut entries: Vec<Entry> = Vec::new();
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
            let section_trailer = parse_section(
                data,
                offset,
                limits,
                &budget,
                &mut entries,
                &mut total_entries,
            )?;
            if section_trailer.get(b"XRefStm").is_some() {
                return Err(SyntaxError::Unsupported(
                    "hybrid cross-reference (/XRefStm)",
                ));
            }
            if section_trailer.get(b"Encrypt").is_some() {
                return Err(SyntaxError::Unsupported("encrypted documents"));
            }
            let prev = section_trailer.get(b"Prev").cloned();
            if trailer.is_none() {
                trailer = Some(trim_trailer(section_trailer));
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
        // ISO 32000-2 §7.5.6: sections were read newest first; the stable sort keeps that order
        // among equal numbers and `dedup` keeps the first, so the newest entry wins.
        entries.sort_by_key(|e| e.num);
        entries.dedup_by_key(|e| e.num);
        Ok(Self {
            data,
            limits: limits.clone(),
            entries,
            trailer,
            budget,
        })
    }

    /// The newest trailer dictionary, trimmed to the entries the engine uses (`/Root` as a
    /// reference, `/Size` as an integer).
    pub fn trailer(&self) -> &Dict {
        &self.trailer
    }

    /// Resolves an indirect reference. ISO 32000-2 §7.3.10: a reference to a free or missing
    /// object is the null object.
    pub fn resolve(&self, r: ObjRef) -> Result<Object, SyntaxError> {
        let entry = self
            .entries
            .binary_search_by_key(&r.num, |e| e.num)
            .ok()
            .and_then(|i| self.entries.get(i));
        let Some(&Entry {
            offset,
            generation,
            in_use: true,
            ..
        }) = entry
        else {
            return Ok(Object::Null);
        };
        if generation != r.generation {
            return Ok(Object::Null);
        }
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        let mut p = Parser::new(self.data, offset, &self.limits, &self.budget);
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

/// Keeps only the trailer entries the engine reads (ISO 32000-2 §7.5.5, Table 15), so a hostile
/// trailer full of junk does not stay in memory while the rest of the document is parsed.
fn trim_trailer(trailer: Dict) -> Dict {
    Dict(
        trailer
            .0
            .into_iter()
            .filter(|(k, v)| match k.as_slice() {
                b"Root" => matches!(v, Object::Ref(_)),
                b"Size" => matches!(v, Object::Integer(_)),
                _ => false,
            })
            .collect(),
    )
}

/// ISO 32000-2 §7.5.5: the last `startxref` keyword and the byte offset after it.
fn find_startxref(data: &[u8], limits: &Limits, budget: &LexBudget) -> Result<usize, SyntaxError> {
    const KEY: &[u8] = b"startxref";
    let from = data.len().saturating_sub(STARTXREF_WINDOW);
    let tail = data.get(from..).unwrap_or_default();
    let at = tail
        .windows(KEY.len())
        .rposition(|w| w == KEY)
        .and_then(|i| i.checked_add(from))
        .ok_or_else(|| malformed("startxref not found", data.len()))?;
    let mut p = Parser::new(data, at.saturating_add(KEY.len()), limits, budget);
    let offset = match p.lexer.next_token()? {
        Some(Token::Integer(o)) => usize::try_from(o).ok(),
        _ => None,
    };
    match offset {
        Some(o) if o < data.len() => Ok(o),
        _ => Err(malformed("invalid startxref offset", at)),
    }
}

/// ISO 32000-2 §7.5.4: `xref`, subsections `start count` + entries, then `trailer << … >>`.
/// Entries are appended to `entries` in file order (deduplicated by the caller). Returns the
/// trailer dictionary.
fn parse_section(
    data: &[u8],
    offset: usize,
    limits: &Limits,
    budget: &LexBudget,
    entries: &mut Vec<Entry>,
    total_entries: &mut usize,
) -> Result<Dict, SyntaxError> {
    let mut p = Parser::new(data, offset, limits, budget);
    match p.lexer.next_token()? {
        Some(Token::Keyword(b"xref")) => {}
        Some(Token::Integer(_)) => {
            return Err(SyntaxError::Unsupported("cross-reference streams"));
        }
        _ => return Err(malformed("expected 'xref'", offset)),
    }
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
                    match (u32::try_from(o), u16::try_from(g), k) {
                        (Ok(offset), Ok(generation), b"n") => Some(Entry {
                            num,
                            offset,
                            generation,
                            in_use: true,
                        }),
                        (_, Ok(generation), b"f") => Some(Entry {
                            num,
                            offset: 0,
                            generation,
                            in_use: false,
                        }),
                        _ => None,
                    }
                }
                _ => None,
            };
            let Some(parsed) = parsed else {
                return Err(malformed("invalid xref entry", at));
            };
            entries.push(parsed);
            if i.saturating_add(1) < count {
                num = num
                    .checked_add(1)
                    .ok_or_else(|| malformed("object number out of range", at))?;
            }
        }
    }
    let at = p.lexer.pos();
    match p.parse_object()? {
        Object::Dict(d) => Ok(d),
        _ => Err(malformed("trailer is not a dictionary", at)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startxref_must_point_inside_the_file() {
        let l = Limits::default();
        let b = LexBudget::new(0, &l);
        assert!(find_startxref(b"startxref\n999\n%%EOF", &l, &b).is_err());
        assert!(find_startxref(b"startxref\n-1\n%%EOF", &l, &b).is_err());
        assert!(find_startxref(b"nothing here", &l, &b).is_err());
        assert_eq!(find_startxref(b"xref startxref 0 %%EOF", &l, &b), Ok(0));
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
    fn xref_entries_stay_compact() {
        // Memory bound: 10M entries must stay near 120 MB (2 GiB address-space limit).
        assert_eq!(std::mem::size_of::<Entry>(), 12);
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
