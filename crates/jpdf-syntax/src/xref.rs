//! Classic cross-reference tables and trailers. ISO 32000-2 §7.5.4–§7.5.6.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use crate::Limits;
use crate::error::{SyntaxError, malformed};
use crate::lexer::{LexBudget, Parser, Token};
use crate::object::{Dict, ObjRef, Object};

/// How far from the end of the file `startxref` is searched for. ISO 32000-2 §7.5.5 puts it
/// in the last lines; real files often carry trailing junk, so read leniently.
const STARTXREF_WINDOW: usize = 64 * 1024;

/// One xref entry (ISO 32000-2 §7.5.4), packed into 12 bytes so the default 4M entries cost
/// 48 MiB (144 MiB at the growth peak; specs/core/memory-budget.md).
/// Offsets fit in `u32` because files are far below 4 GiB (`Limits::max_file_size`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entry {
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
    /// Byte spans (start → end) of xref sections and indirect objects parsed so far. Objects in a well-formed
    /// file never overlap (ISO 32000-2 §7.3.10, §7.5.3); hostile files overlap them so that
    /// many references re-parse the same expensive bytes. Rejecting overlap keeps total object
    /// parsing to about one pass over the file.
    spans: RefCell<BTreeMap<usize, usize>>,
    /// Objects parsed so far, so a shared object (a resource dictionary every page points to)
    /// is parsed once instead of once per reference. Holds at most
    /// `Limits::max_retained_bytes_per_document` by [`retained_cost`]; an object that doesn't
    /// fit isn't cached and is re-parsed (and charged to the lexing budget) on every resolve.
    cache: RefCell<BTreeMap<ObjRef, Arc<Object>>>,
    /// Sum of [`retained_cost`] over `cache`.
    retained: Cell<u64>,
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
        let mut spans: BTreeMap<usize, usize> = BTreeMap::new();
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
            // Sections, like objects, never overlap in a well-formed file; hostile files stack
            // many sections over one expensive trailer body.
            if overlaps(&spans, offset, None) {
                return Err(malformed("cross-reference sections overlap", offset));
            }
            let (section_trailer, end) = parse_section(
                data,
                offset,
                limits,
                &budget,
                &mut entries,
                &mut total_entries,
            )?;
            if overlaps(&spans, offset, Some(end)) {
                return Err(malformed("cross-reference sections overlap", offset));
            }
            spans.insert(offset, end);
            if section_trailer.get(b"XRefStm").is_some() {
                return Err(SyntaxError::Unsupported(
                    "hybrid cross-reference (/XRefStm)",
                ));
            }
            if section_trailer.get(b"Encrypt").is_some() {
                return Err(SyntaxError::Unsupported("encrypted documents"));
            }
            // Match by reference: cloning first would deep-copy a hostile /Prev (a huge array)
            // only to reject it.
            let prev = match section_trailer.get(b"Prev") {
                None => None,
                Some(Object::Integer(p)) => Some(
                    usize::try_from(*p).map_err(|_| malformed("negative /Prev offset", offset))?,
                ),
                Some(_) => return Err(malformed("/Prev is not an integer", offset)),
            };
            if trailer.is_none() {
                trailer = Some(trim_trailer(section_trailer));
            }
            match prev {
                None => break,
                Some(p) => offset = p,
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
            spans: RefCell::new(spans),
            cache: RefCell::new(BTreeMap::new()),
            retained: Cell::new(0),
        })
    }

    /// The newest trailer dictionary, trimmed to the entries the engine uses (`/Root` as a
    /// reference, `/Size` as an integer).
    pub fn trailer(&self) -> &Dict {
        &self.trailer
    }

    /// Bytes the object cache holds, charged against `Limits::max_retained_bytes_per_document`.
    pub fn retained_bytes(&self) -> u64 {
        self.retained.get()
    }

    /// Bytes lexed so far and object spans recorded (tests: proves whether a resolve read the
    /// file).
    #[cfg(test)]
    pub(crate) fn read_state(&self) -> (usize, usize) {
        (self.budget.used(), self.spans.borrow().len())
    }

    /// Resolves an indirect reference. ISO 32000-2 §7.3.10: a reference to a free or missing
    /// object is the null object.
    ///
    /// The value is shared: resolving the same reference again returns the cached object without
    /// re-parsing it, as long as it fit in the retained-bytes budget.
    pub fn resolve(&self, r: ObjRef) -> Result<Arc<Object>, SyntaxError> {
        if let Some(cached) = self.cache.borrow().get(&r) {
            return Ok(Arc::clone(cached));
        }
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
            return Ok(Arc::new(Object::Null));
        };
        if generation != r.generation {
            return Ok(Arc::new(Object::Null));
        }
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        self.check_no_overlap(offset, None)?;
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
        let value = p.parse_object()?;
        let end = p.lexer.pos();
        self.check_no_overlap(offset, Some(end))?;
        self.spans.borrow_mut().insert(offset, end);
        let value = Arc::new(value);
        self.remember(r, &value);
        Ok(value)
    }

    /// Caches `value` if it fits in what is left of the retained-bytes budget; otherwise leaves
    /// the cache as it is (no eviction), so the object is re-parsed when resolved again.
    fn remember(&self, r: ObjRef, value: &Arc<Object>) {
        let total = self.retained.get().saturating_add(retained_cost(value));
        if total <= self.limits.max_retained_bytes_per_document {
            self.cache.borrow_mut().insert(r, Arc::clone(value));
            self.retained.set(total);
        }
    }

    /// Fails if an object starting at `start` (and ending at `end`, once known) overlaps an
    /// object parsed earlier. Re-reading the same object (same start) is allowed.
    fn check_no_overlap(&self, start: usize, end: Option<usize>) -> Result<(), SyntaxError> {
        if overlaps(&self.spans.borrow(), start, end) {
            return Err(malformed("indirect objects overlap", start));
        }
        Ok(())
    }
}

/// Fixed cost of one cache entry in the map: the key and the `Arc` pointer (16 bytes) in a
/// B-tree node at least 5/11 full, plus internal nodes and the node's allocator header (about
/// 54 bytes; specs/core/memory-budget.md).
const CACHE_ENTRY_OVERHEAD: u64 = 64;

/// Bytes the allocator really holds for a heap allocation of `n` bytes. This is glibc's
/// malloc on 64-bit Linux, where the judge runs: an 8-byte header, rounded up to 16, at least
/// 32 bytes. A 1-byte name costs 32, not 1 (specs/core/memory-budget.md, #109).
fn alloc_bytes(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    n.saturating_add(8 + 15).max(32 + 15) & !15
}

/// Bytes a cached object keeps alive: the entry overhead, the `Arc` allocation holding the
/// root `Object` and its two counters, and every heap allocation below it, by capacity, as
/// allocator chunks.
fn retained_cost(o: &Object) -> u64 {
    let arc = size_u64(size_of::<Object>()).saturating_add(16);
    CACHE_ENTRY_OVERHEAD
        .saturating_add(alloc_bytes(arc))
        .saturating_add(heap_bytes(o))
}

/// Heap bytes owned by `o`, not counting `o` itself. Recursion depth is the object's nesting
/// depth, which the parser that built it already bounded by `Limits::max_nesting_depth` with
/// larger stack frames than this one.
fn heap_bytes(o: &Object) -> u64 {
    match o {
        Object::String(s) | Object::Name(s) => alloc_bytes(size_u64(s.capacity())),
        Object::Array(items) => items.iter().fold(
            alloc_bytes(size_u64(items.capacity()).saturating_mul(size_u64(size_of::<Object>()))),
            |sum, item| sum.saturating_add(heap_bytes(item)),
        ),
        Object::Dict(d) => d.0.iter().fold(
            alloc_bytes(
                size_u64(d.0.capacity()).saturating_mul(size_u64(size_of::<(Vec<u8>, Object)>())),
            ),
            |sum, (k, v)| {
                sum.saturating_add(alloc_bytes(size_u64(k.capacity())))
                    .saturating_add(heap_bytes(v))
            },
        ),
        Object::Null | Object::Bool(_) | Object::Integer(_) | Object::Real(_) | Object::Ref(_) => 0,
    }
}

fn size_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Whether a range starting at `start` (ending at `end`, once known) overlaps a recorded span.
/// A range with the same start as a recorded one is the same object re-read: not an overlap.
fn overlaps(spans: &BTreeMap<usize, usize>, start: usize, end: Option<usize>) -> bool {
    if spans.contains_key(&start) {
        return false;
    }
    let inside_earlier = spans
        .range(..start)
        .next_back()
        .is_some_and(|(_, e)| *e > start);
    let covers_later = end.is_some_and(|end| {
        spans
            .range(start.saturating_add(1)..)
            .next()
            .is_some_and(|(s, _)| *s < end)
    });
    inside_earlier || covers_later
}

/// Keeps only the trailer entries the engine reads (ISO 32000-2 §7.5.5, Table 15), so a hostile
/// trailer full of junk does not stay in memory while the rest of the document is parsed. Keys
/// are unique in a well-formed dictionary (§7.3.7); with duplicates, only the first usable
/// `/Root` (a reference) and the first usable `/Size` (an integer) are kept, which is what
/// `Dict::get` returned before, so the trimmed trailer holds at most 2 entries in an exact
/// buffer (specs/core/memory-budget.md).
fn trim_trailer(trailer: Dict) -> Dict {
    let mut root = None;
    let mut size = None;
    for (k, v) in trailer.0 {
        match (k.as_slice(), &v) {
            (b"Root", Object::Ref(_)) if root.is_none() => root = Some((k, v)),
            (b"Size", Object::Integer(_)) if size.is_none() => size = Some((k, v)),
            _ => {}
        }
    }
    let mut kept: Vec<(Vec<u8>, Object)> = Vec::new();
    let n = usize::from(root.is_some()).saturating_add(usize::from(size.is_some()));
    kept.reserve_exact(n);
    kept.extend(root.into_iter().chain(size).map(|(mut k, v)| {
        k.shrink_to_fit();
        (k, v)
    }));
    Dict(kept)
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
/// trailer dictionary and the offset just past it.
fn parse_section(
    data: &[u8],
    offset: usize,
    limits: &Limits,
    budget: &LexBudget,
    entries: &mut Vec<Entry>,
    total_entries: &mut usize,
) -> Result<(Dict, usize), SyntaxError> {
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
                        // An offset beyond any accepted file cannot resolve: read it as free
                        // (a reference to it is null, ISO 32000-2 §7.3.10) instead of failing.
                        (Err(_), Ok(generation), b"n") if o > 0 => Some(Entry {
                            num,
                            offset: 0,
                            generation,
                            in_use: false,
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
        Object::Dict(d) => Ok((d, p.lexer.pos())),
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

    /// The default entry limit (2^21 since #146) at its exact boundary: a subsection header
    /// declaring 2 097 152 entries passes the count check (it then fails on the missing first
    /// entry, at byte 14), one more is refused before any entry is read. Two subsections are
    /// summed.
    #[test]
    fn default_entry_limit_is_exact() {
        let l = Limits::default();
        assert_eq!(l.max_xref_entries, 2_097_152);
        let at = b"xref\n0 2097152\ntrailer\n<<>>\nstartxref\n0\n%%EOF";
        assert_eq!(
            XrefDocument::parse(at, &l).map(|_| ()),
            Err(malformed("invalid xref entry", 14))
        );
        let past = b"xref\n0 2097153\ntrailer\n<<>>\nstartxref\n0\n%%EOF";
        assert_eq!(
            XrefDocument::parse(past, &l).map(|_| ()),
            Err(SyntaxError::LimitExceeded("cross-reference entries"))
        );
        // Two subsections: 1 + 2 097 151 is at the limit, 1 + 2 097 152 one past.
        let head = b"xref\n0 1\n0000000000 65535 f\r\n1 ";
        let summed = |count: &str| {
            let mut f = head.to_vec();
            f.extend(count.as_bytes());
            f.extend(b"\ntrailer\n<<>>\nstartxref\n0\n%%EOF");
            XrefDocument::parse(&f, &l).map(|_| ())
        };
        assert_eq!(summed("2097151"), Err(malformed("invalid xref entry", 38)));
        assert_eq!(
            summed("2097152"),
            Err(SyntaxError::LimitExceeded("cross-reference entries"))
        );
    }

    /// A trailer with 10 000 duplicate `/Root` and `/Size` keys keeps 2 entries: the first
    /// usable of each (a non-reference `/Root` and a non-integer `/Size` are skipped, as before),
    /// in an exact buffer.
    #[test]
    fn trailer_with_duplicate_keys_is_trimmed_to_two_entries() {
        let mut body = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\n".to_vec();
        let xref_at = body.len();
        body.extend(
            b"xref\n0 2\n0000000000 65535 f\r\n0000000009 00000 n\r\ntrailer\n<< /Root 7 /Size (x)",
        );
        for _ in 0..10_000 {
            body.extend(b" /Root 1 0 R /Size 2 /Root 5 0 R /Size 9 /Junk 1");
        }
        body.extend(format!(" >>\nstartxref\n{xref_at}\n%%EOF\n").as_bytes());
        let doc = XrefDocument::parse(&body, &Limits::default()).expect("parses");
        let t = &doc.trailer().0;
        assert_eq!(
            t,
            &vec![
                (b"Root".to_vec(), Object::Ref(obj(1))),
                (b"Size".to_vec(), Object::Integer(2)),
            ]
        );
        assert_eq!(t.capacity(), 2);
        assert!(t.iter().all(|(k, _)| k.capacity() == 4));
    }

    /// Objects and string bytes in `o` the way the parser counts them: every object once, and
    /// string, name and key bytes.
    fn parse_counts(o: &Object) -> (u64, u64) {
        match o {
            Object::String(s) | Object::Name(s) => (1, size_u64(s.len())),
            Object::Array(items) => items.iter().fold((1, 0), |(n, b), item| {
                let (n2, b2) = parse_counts(item);
                (n.saturating_add(n2), b.saturating_add(b2))
            }),
            Object::Dict(d) => d.0.iter().fold((1, 0), |(n, b), (k, v)| {
                let (n2, b2) = parse_counts(v);
                (
                    n.saturating_add(n2),
                    b.saturating_add(size_u64(k.len())).saturating_add(b2),
                )
            }),
            _ => (1, 0),
        }
    }

    /// `budget::PARSE_OBJECT_BYTES` bounds what real parsed objects hold, as allocator chunks:
    /// the worst shape (nested one-entry dictionaries) costs exactly 272 bytes per dictionary,
    /// and other shapes stay below `objects × 272 + 3 × string bytes`.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn parse_object_bytes_bound_real_objects() {
        use crate::budget::PARSE_OBJECT_BYTES;
        let nested = [b"<</a ".repeat(200), b"null".to_vec(), b">>".repeat(200)].concat();
        let many_names = [b"[".to_vec(), b"/a ".repeat(1000), b"]".to_vec()].concat();
        let many_keys = [b"<<".to_vec(), b"/k 1 ".repeat(1000), b">>".to_vec()].concat();
        let small_arrays = [b"[".to_vec(), b"[1] ".repeat(1000), b"]".to_vec()].concat();
        let short_strings = [
            b"[".to_vec(),
            b"(abcdefghijklmnopqrstuv) ".repeat(500),
            b"]".to_vec(),
        ]
        .concat();
        let bodies = [nested, many_names, many_keys, small_arrays, short_strings];
        let data = file(&bodies);
        let doc = XrefDocument::parse(&data, &Limits::default()).expect("parses");
        for n in 1..=5 {
            let o = doc.resolve(obj(n)).expect("resolves");
            let held = heap_bytes(&o);
            let (objects, strings) = parse_counts(&o);
            assert!(
                held <= objects * PARSE_OBJECT_BYTES + 3 * strings,
                "object {n}: {held} bytes for {objects} objects, {strings} string bytes"
            );
            if n == 1 {
                // 200 dictionaries, each a 240-byte buffer chunk and a 32-byte key chunk.
                assert_eq!((objects, held), (201, 200 * 272));
            }
        }
    }

    /// The default `max_objects_per_parse` (2^20, lowered by #109) at its exact boundary: an
    /// array and 2^20 - 1 integers (2^20 objects) parses, one integer more is refused.
    #[test]
    fn default_objects_per_parse_limit_is_exact() {
        let l = Limits::default();
        assert_eq!(l.max_objects_per_parse, 1 << 20);
        let array = |n: usize| [b"[".to_vec(), b"1 ".repeat(n), b"]".to_vec()].concat();
        let data = file(&[array((1 << 20) - 1), array(1 << 20)]);
        let doc = XrefDocument::parse(&data, &l).expect("parses");
        let at = doc.resolve(obj(1)).expect("2^20 objects parse");
        assert_eq!(at.as_array().map(<[Object]>::len), Some((1 << 20) - 1));
        drop(at);
        assert_eq!(
            doc.resolve(obj(2)).map(|_| ()),
            Err(SyntaxError::LimitExceeded("objects in one parse"))
        );
    }

    /// The lowered `max_objects_per_parse` still reaches `max_pages`: a `/Pages` dictionary
    /// whose `/Kids` holds 1 000 000 references parses under the default limits (each `n g R`
    /// counts as one object).
    #[test]
    fn flat_page_tree_at_max_pages_fits_one_parse() {
        let l = Limits::default();
        assert_eq!(l.max_pages, 1_000_000);
        let body = [
            b"<< /Type /Pages /Kids [".to_vec(),
            b"2 0 R ".repeat(l.max_pages),
            b"] /Count 1000000 >>".to_vec(),
        ]
        .concat();
        let data = file(&[body]);
        let doc = XrefDocument::parse(&data, &l).expect("parses");
        let pages = doc
            .resolve(obj(1))
            .expect("a flat page tree at max_pages parses");
        let kids = pages
            .as_dict()
            .and_then(|d| d.get(b"Kids"))
            .and_then(Object::as_array)
            .map(<[Object]>::len);
        assert_eq!(kids, Some(1_000_000));
    }

    /// The lowered default `max_string_bytes_per_parse` (32 MiB, #146) at its exact boundary:
    /// an object whose two strings decode to exactly 32 MiB parses, one byte more is refused.
    /// Two strings, so a check that looks at one token instead of the parse's total fails.
    #[test]
    fn default_string_bytes_limit_is_exact() {
        let l = Limits::default();
        assert_eq!(l.max_string_bytes_per_parse, 32 << 20);
        let half = 16 << 20;
        let pair = |second: usize| {
            [
                b"[(".to_vec(),
                b"a".repeat(half),
                b") <".to_vec(),
                b"62".repeat(second),
                b">]".to_vec(),
            ]
            .concat()
        };
        let data = file(&[pair(half), pair(half + 1)]);
        let doc = XrefDocument::parse(&data, &l).expect("parses");
        let at = doc.resolve(obj(1)).expect("32 MiB of string bytes parse");
        let lens: Option<Vec<usize>> = at.as_array().map(|a| {
            a.iter()
                .filter_map(|o| match o {
                    Object::String(s) => Some(s.len()),
                    _ => None,
                })
                .collect()
        });
        assert_eq!(lens, Some(vec![half, half]));
        drop(at);
        assert_eq!(
            doc.resolve(obj(2)).map(|_| ()),
            Err(SyntaxError::LimitExceeded("string bytes in one object"))
        );
    }

    #[test]
    fn xref_entries_stay_compact() {
        // Memory bound: the budget (crate::budget) multiplies this size; it is pinned there too.
        assert_eq!(std::mem::size_of::<Entry>(), 12);
    }

    /// `Arc` (not `Rc`) in the cache keeps the document `Send` (specs/core/resolve-cache.md).
    #[test]
    fn xref_document_is_send() {
        fn send<T: Send>() {}
        send::<XrefDocument<'static>>();
    }

    /// A classic file whose objects are numbered from 1 in order; the trailer's /Root is 1 0 R.
    fn file(objects: &[Vec<u8>]) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offsets = Vec::new();
        for (num, body) in (1..).zip(objects) {
            offsets.push(out.len());
            out.extend(format!("{num} 0 obj\n").as_bytes());
            out.extend(body);
            out.extend(b"\nendobj\n");
        }
        with_xref(out, &offsets)
    }

    /// Appends an xref table (objects 1.. at `offsets`) and a trailer to `out`.
    fn with_xref(mut out: Vec<u8>, offsets: &[usize]) -> Vec<u8> {
        let xref_at = out.len();
        let size = offsets.len().saturating_add(1);
        out.extend(format!("xref\n0 {size}\n0000000000 65535 f\r\n").as_bytes());
        for o in offsets {
            out.extend(format!("{o:010} 00000 n\r\n").as_bytes());
        }
        out.extend(
            format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n")
                .as_bytes(),
        );
        out
    }

    fn obj(num: u32) -> ObjRef {
        ObjRef { num, generation: 0 }
    }

    fn with_retained(max: u64) -> Limits {
        Limits {
            max_retained_bytes_per_document: max,
            ..Limits::default()
        }
    }

    /// Issue #107: one 2 MiB dictionary that every page would share. Eleven resolves
    /// cost 22 MiB of lexing without a cache, over the default budget of 4 × file + 1 MiB (about
    /// 9 MiB); with the cache the object is parsed once.
    #[test]
    fn shared_object_resolved_many_times_fits_the_default_budget() {
        let mut big = b"<< /Type /ExtGState /Junk (".to_vec();
        big.extend(vec![b'x'; 2 * 1024 * 1024]);
        big.extend(b") >>");
        let data = file(&[big]);
        let doc = XrefDocument::parse(&data, &Limits::default()).expect("parses");
        let first = doc.resolve(obj(1)).expect("first resolve");
        let used = doc.budget.used();
        for _ in 0..10 {
            let again = doc.resolve(obj(1)).expect("cached resolve");
            assert!(Arc::ptr_eq(&first, &again));
        }
        assert_eq!(doc.budget.used(), used, "a cached resolve lexes nothing");
        assert_eq!(doc.retained_bytes(), retained_cost(&first));

        // Control: with no room to cache, the same resolves re-parse the object and run out
        // of lexing budget, so the test above really exercises the re-parse path.
        let doc = XrefDocument::parse(&data, &with_retained(0)).expect("parses");
        let results: Vec<_> = (0..10).map(|_| doc.resolve(obj(1)).map(|_| ())).collect();
        // The budget (about 9 MiB) covers four 2 MiB parses; the fifth runs out.
        assert!(results[..4].iter().all(Result::is_ok), "{results:?}");
        assert_eq!(
            results[4],
            Err(SyntaxError::LimitExceeded("bytes lexed per document"))
        );
        assert_eq!(doc.retained_bytes(), 0);
    }

    /// The retained-bytes budget at the limit caches the object; one byte less does not, and the
    /// object is then re-parsed (and charged) on every resolve.
    #[test]
    fn retained_budget_boundary_for_one_object() {
        let data = file(&[b"<< /Type /Catalog /Pages 2 0 R /Name (abc) >>".to_vec()]);
        let cost = {
            let doc = XrefDocument::parse(&data, &Limits::default()).expect("parses");
            let value = doc.resolve(obj(1)).expect("resolves");
            retained_cost(&value)
        };
        let at = XrefDocument::parse(&data, &with_retained(cost)).expect("parses");
        let a = at.resolve(obj(1)).expect("resolves");
        let used = at.budget.used();
        let b = at.resolve(obj(1)).expect("resolves");
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(at.budget.used(), used);
        assert_eq!(at.retained_bytes(), cost);

        let past = XrefDocument::parse(&data, &with_retained(cost - 1)).expect("parses");
        let a = past.resolve(obj(1)).expect("resolves");
        let used = past.budget.used();
        let b = past.resolve(obj(1)).expect("resolves");
        assert!(!Arc::ptr_eq(&a, &b));
        assert_eq!(a, b);
        assert!(past.budget.used() > used, "an uncached resolve re-parses");
        assert_eq!(past.retained_bytes(), 0);
    }

    /// The budget is per document: costs add up across objects, and an object that doesn't fit
    /// leaves the cache unchanged (no eviction) while later, smaller objects may still fit.
    #[test]
    fn retained_budget_adds_up_across_objects() {
        let data = file(&[
            b"<< /A (first) >>".to_vec(),
            b"<< /B [1 2 3 4 5 6 7 8 9 10] /C (second, longer) >>".to_vec(),
            b"null".to_vec(),
        ]);
        let costs: Vec<u64> = {
            let doc = XrefDocument::parse(&data, &Limits::default()).expect("parses");
            (1..=3)
                .map(|n| retained_cost(&doc.resolve(obj(n)).expect("resolves")))
                .collect()
        };
        let both = costs[0] + costs[1];
        let doc = XrefDocument::parse(&data, &with_retained(both)).expect("parses");
        doc.resolve(obj(1)).expect("resolves");
        doc.resolve(obj(2)).expect("resolves");
        assert_eq!(doc.retained_bytes(), both);
        assert_eq!(doc.cache.borrow().len(), 2);

        let doc = XrefDocument::parse(&data, &with_retained(both - 1)).expect("parses");
        doc.resolve(obj(1)).expect("resolves");
        doc.resolve(obj(2)).expect("resolves");
        assert_eq!(doc.retained_bytes(), costs[0]);
        assert_eq!(doc.cache.borrow().len(), 1);
        // Object 3 (`null`) is smaller than object 2, so it still fits after 2 was refused.
        let three = costs[0] + costs[2];
        assert!(three < both - 1);
        doc.resolve(obj(3)).expect("resolves");
        assert_eq!(doc.retained_bytes(), three);
        assert_eq!(doc.cache.borrow().len(), 2);
    }

    /// Free, missing and generation-mismatched references are null and not cached; a cached
    /// object is found only under its own generation.
    #[test]
    fn null_references_and_errors_are_not_cached() {
        let data = file(&[b"<< /A 1 >>".to_vec(), b"<< /B 2 >>".to_vec()]);
        let doc = XrefDocument::parse(&data, &Limits::default()).expect("parses");
        let one = doc.resolve(obj(1)).expect("resolves");
        let cached = doc.retained_bytes();
        assert!(cached > 0);
        for r in [
            obj(0),
            obj(99),
            ObjRef {
                num: 1,
                generation: 1,
            },
        ] {
            assert_eq!(doc.resolve(r).as_deref(), Ok(&Object::Null), "{r:?}");
        }
        assert_eq!(doc.retained_bytes(), cached);
        assert_eq!(doc.cache.borrow().len(), 1);
        assert!(Arc::ptr_eq(&one, &doc.resolve(obj(1)).expect("cached")));

        // An object whose header doesn't match its xref entry fails every time; nothing is
        // cached for it.
        let mut bad = data.clone();
        let at = bad
            .windows(7)
            .position(|w| w == b"2 0 obj")
            .expect("object 2");
        bad[at] = b'3';
        let doc = XrefDocument::parse(&bad, &Limits::default()).expect("parses");
        for _ in 0..2 {
            assert!(matches!(
                doc.resolve(obj(2)),
                Err(SyntaxError::Malformed {
                    what: "object header does not match xref",
                    ..
                })
            ));
        }
        assert_eq!(doc.retained_bytes(), 0);
    }

    /// An object rejected by the end-of-object overlap check (it covers an object parsed
    /// earlier) is not cached: resolving it again fails the same way instead of returning a
    /// cached value that skipped the check.
    #[test]
    fn object_rejected_for_overlap_is_not_cached() {
        let mut out = b"%PDF-1.7\n".to_vec();
        let one = out.len();
        out.extend(b"1 0 obj\n(");
        let two = out.len();
        out.extend(b"2 0 obj\n<< /A 1 >>\nendobj\n");
        out.extend(b")\nendobj\n");
        let data = with_xref(out, &[one, two]);
        let doc = XrefDocument::parse(&data, &Limits::default()).expect("parses");
        assert_eq!(
            doc.resolve(obj(2)).as_deref(),
            Ok(&Object::Dict(Dict(vec![(
                b"A".to_vec(),
                Object::Integer(1)
            )])))
        );
        let cached = doc.retained_bytes();
        for _ in 0..2 {
            assert_eq!(
                doc.resolve(obj(1)).map(|_| ()),
                Err(SyntaxError::Malformed {
                    what: "indirect objects overlap",
                    offset: one,
                })
            );
        }
        assert_eq!(doc.retained_bytes(), cached);
        assert_eq!(doc.cache.borrow().len(), 1);
        assert!(!doc.cache.borrow().contains_key(&obj(1)));
    }

    /// glibc chunk sizes: 8-byte header, 16-byte rounding, 32-byte minimum; no allocation is
    /// free. Literals, so a cheaper model shows up as a test change.
    #[test]
    fn alloc_bytes_follow_glibc_chunks() {
        let cases = [
            (0, 0),
            (1, 32),
            (24, 32),
            (25, 48),
            (40, 48),
            (41, 64),
            (100, 112),
        ];
        for (n, chunk) in cases {
            assert_eq!(alloc_bytes(n), chunk, "{n} bytes");
        }
        assert_eq!(alloc_bytes(u64::MAX), 0xFFFF_FFFF_FFFF_FFF0);
    }

    /// The cost is the entry overhead, the `Arc` chunk, and every heap allocation by capacity,
    /// as allocator chunks. All literals (64-bit: `Object` is 32 bytes, a dictionary slot 56).
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn retained_cost_counts_every_allocation() {
        // 64 for the map entry + the 48-byte `Arc` allocation as a 64-byte chunk.
        let base = 128;
        assert_eq!(retained_cost(&Object::Integer(7)), base);
        assert_eq!(retained_cost(&Object::Null), base);
        let mut s = Vec::with_capacity(100);
        s.extend(b"abc");
        assert_eq!(retained_cost(&Object::String(s)), base + 112);
        // A 4-byte name is a minimum (32-byte) chunk.
        assert_eq!(retained_cost(&Object::Name(b"Type".to_vec())), base + 32);
        let mut items = Vec::with_capacity(3);
        items.push(Object::Integer(1));
        items.push(Object::Name(b"xy".to_vec()));
        // 3 slots (96 bytes, a 112-byte chunk) + the name (32).
        assert_eq!(retained_cost(&Object::Array(items)), base + 112 + 32);
        let dict = Object::Dict(Dict(vec![
            (
                b"K".to_vec(),
                Object::Array(vec![Object::String(b"zz".to_vec())]),
            ),
            (b"LL".to_vec(), Object::Ref(obj(1))),
        ]));
        // 2 slots (112 bytes, a 128-byte chunk), key K (32), a 1-slot array (32 bytes, a
        // 48-byte chunk), its string (32), key LL (32).
        assert_eq!(retained_cost(&dict), base + 128 + 32 + 48 + 32 + 32);

        // Spare capacity counts everywhere: the dictionary buffer, keys and names (the parser
        // grows them by pushing, so real ones usually have some).
        let mut key = Vec::with_capacity(40);
        key.push(b'K');
        let mut name = Vec::with_capacity(20);
        name.extend(b"Nm");
        let mut entries = Vec::with_capacity(5);
        entries.push((key, Object::Name(name)));
        // 5 slots (280 bytes, a 288-byte chunk), key (capacity 40, a 48-byte chunk; its length
        // 1 would be a 32-byte one), name (32).
        assert_eq!(
            retained_cost(&Object::Dict(Dict(entries))),
            base + 288 + 48 + 32
        );
        // Capacity 40 (a 48-byte chunk): capacities 1 to 24 all cost a 32-byte chunk, so a
        // smaller one could not tell capacity from length.
        let mut name = Vec::with_capacity(40);
        name.push(b'N');
        assert_eq!(retained_cost(&Object::Name(name)), base + 48);
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
