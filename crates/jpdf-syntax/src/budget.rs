//! The process memory budget: what the default `Limits` let one document hold at its peak
//! (specs/core/memory-budget.md, #109). The judge runs `jpdf-cli` under a 2 GiB address-space
//! limit (ops/judge/CONTRACT.md); running out is a crash, so the defaults must fit it with a
//! reserve for everything no limit charges.

use crate::Limits;
use crate::xref::Entry;

/// The judge's address-space limit for one `jpdf-cli` process.
pub const PROCESS_BUDGET_BYTES: u64 = 2 << 30;

/// The binary, its shared libraries and other fixed mappings.
pub const RESERVE_BINARY_BYTES: u64 = 64 << 20;
/// Thread stacks: the main thread (8 MiB by default) and the engine thread (#114).
pub const RESERVE_STACK_BYTES: u64 = 32 << 20;
/// glibc reserves a 64 MiB heap region for the first allocation on each thread other than the
/// main one, and the address-space limit counts it. One extra thread (the engine thread, #114).
pub const RESERVE_THREAD_ARENA_BYTES: u64 = 64 << 20;
/// Allocator metadata and fragmentation that per-allocation chunk sizes don't cover: freed
/// chunks the allocator keeps, page rounding of large mappings, small bookkeeping (`/Prev`
/// cycle set, error strings).
pub const RESERVE_FRAGMENTATION_BYTES: u64 = 96 << 20;

/// Room for what no limit charges: the sum of the `RESERVE_*` items.
pub const RESERVE_BYTES: u64 = RESERVE_BINARY_BYTES
    + RESERVE_STACK_BYTES
    + RESERVE_THREAD_ARENA_BYTES
    + RESERVE_FRAGMENTATION_BYTES;

/// Bytes one recorded span costs in `XrefDocument`'s `BTreeMap<usize, usize>`. A std B-tree
/// leaf holds at most 11 pairs in a 192-byte node (88 + 88 + parent pointer and lengths).
/// Nothing is ever removed, and a split leaves at least 5 pairs in each half, so every leaf
/// holds at least 5. Internal nodes (288 bytes, at least 6 children) add a geometric sum of
/// about 58 bytes per leaf: (192 + 58) / 5 = 50 bytes per span, about 54 with glibc chunk
/// headers. 64 leaves room above that.
pub const SPAN_ENTRY_BYTES: u64 = 64;

/// Bytes per page that `jpdf_cos::flat_page_count` holds while it resolves the kids: the kid
/// references (an 8-byte `ObjRef` each, collected by doubling, so up to 24) and the duplicate
/// check (a `HashSet<ObjRef>` with capacity for every kid: at most 16/7 buckets per kid, 9 bytes
/// each, about 21).
pub const PAGE_WALK_BYTES: u64 = 48;

/// Worst-case bytes one parsed object holds, string bytes aside. The parser builds every array
/// and dictionary with `Vec::new()` + `push`, so a container with one entry already holds 4
/// slots: a one-entry dictionary is a 224-byte buffer (a 240-byte chunk) plus its key (at
/// least capacity 8, a 32-byte chunk) = 272 bytes for one counted object (the dictionary; its
/// value is counted on its own). Larger containers cost less per entry: a dictionary slot is
/// 56 bytes, 168 at the growth peak, plus a key chunk of at most 48 below 23 bytes of key
/// (longer keys are within 3 × their string bytes). A short string or name object costs at
/// most a 48-byte chunk plus its slot. Pinned by `parse_object_bytes_bound_real_objects`
/// (xref.rs) on parsed objects.
pub const PARSE_OBJECT_BYTES: u64 = 272;

/// A growing `Vec` holds at most twice its length; while it reallocates, the old buffer (one
/// length) and the new one (two lengths) are alive together. A stable sort's scratch buffer
/// (at most one length) fits in the same factor, since it runs after the growth.
const GROWTH_FACTOR: u64 = 3;

/// Peak bytes per term for one document under a set of limits. Persistent terms live as long
/// as the document (or overlap every phase); phase terms are alive one at a time (a document
/// parses one object, decodes one stream or renders one tile at a time), but a finished phase
/// can leave its heap mapped, so the largest phase counts twice
/// ([`MemoryBudget::heap_high_water`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBudget {
    /// The file bytes (`max_file_size`), read into memory once with an exact reservation.
    pub file: u64,
    /// Everything the document keeps between calls (`max_retained_bytes_per_document`),
    /// charged as allocator chunks: cached objects, decoded streams and font programs, and
    /// any phase output still alive in a later phase.
    pub retained: u64,
    /// The cross-reference entry table at its peak: `max_xref_entries` entries, times the
    /// growth factor.
    pub xref_table: u64,
    /// The overlap map: one span per xref section and per resolved object (at most one object
    /// per entry, since resolve goes through the table).
    pub spans: u64,
    /// The page-tree walk (`max_pages` × [`PAGE_WALK_BYTES`]). It is alive while the kids are
    /// parsed, so it counts as persistent.
    pub page_walk: u64,
    /// One parse: `max_objects_per_parse` objects at [`PARSE_OBJECT_BYTES`] each, plus
    /// `max_string_bytes_per_parse` string and name bytes times the growth factor.
    pub parse: u64,
    /// One decode: the filter chain's output feeding the image decoder
    /// (`max_decoded_bytes_per_stream`) plus the decoder's own working memory, output
    /// included (`max_decode_working_bytes`).
    pub decode: u64,
    /// One render's working memory (`max_raster_working_bytes`).
    pub raster: u64,
}

impl MemoryBudget {
    /// Every term under `limits`, with saturating arithmetic (loose limits give `u64::MAX`).
    pub fn of(limits: &Limits) -> Self {
        let entry = size_u64(size_of::<Entry>());
        let spans = size_u64(limits.max_xref_entries)
            .saturating_add(size_u64(limits.max_xref_sections))
            .saturating_mul(SPAN_ENTRY_BYTES);
        let parse = size_u64(limits.max_objects_per_parse)
            .saturating_mul(PARSE_OBJECT_BYTES)
            .saturating_add(
                size_u64(limits.max_string_bytes_per_parse).saturating_mul(GROWTH_FACTOR),
            );
        Self {
            file: size_u64(limits.max_file_size),
            retained: limits.max_retained_bytes_per_document,
            xref_table: size_u64(limits.max_xref_entries)
                .saturating_mul(entry)
                .saturating_mul(GROWTH_FACTOR),
            spans,
            page_walk: size_u64(limits.max_pages).saturating_mul(PAGE_WALK_BYTES),
            parse,
            decode: size_u64(limits.max_decoded_bytes_per_stream)
                .saturating_add(limits.max_decode_working_bytes),
            raster: limits.max_raster_working_bytes,
        }
    }

    /// Terms alive for the whole life of the document, or across every phase.
    pub fn persistent(&self) -> u64 {
        self.file
            .saturating_add(self.retained)
            .saturating_add(self.xref_table)
            .saturating_add(self.spans)
            .saturating_add(self.page_walk)
    }

    /// The largest of the phases that run one at a time.
    pub fn largest_phase(&self) -> u64 {
        self.parse.max(self.decode).max(self.raster)
    }

    /// Heap a finished phase can leave mapped while the next phase runs (#146). glibc serves
    /// requests below its mmap threshold (128 KiB at first, up to 32 MiB once raised) from the
    /// brk heap, which shrinks only from the top: one live allocation made during a phase (a
    /// span, a cache node) keeps everything below it mapped. Freed heap memory is reused by
    /// later small requests, but a request above the threshold gets a new mapping. So the
    /// worst case is one phase's whole memory left in the heap (every phase can allocate below
    /// the threshold) plus the next phase's whole memory in new mappings.
    pub fn heap_high_water(&self) -> u64 {
        self.largest_phase()
    }

    /// Peak process memory: persistent terms, the heap high-water mark of an earlier phase, the
    /// largest phase and [`RESERVE_BYTES`].
    pub fn peak(&self) -> u64 {
        self.persistent()
            .saturating_add(self.heap_high_water())
            .saturating_add(self.largest_phase())
            .saturating_add(RESERVE_BYTES)
    }
}

fn size_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Object;

    const MIB: u64 = 1 << 20;

    /// The type sizes the formula multiplies, as literals: a change shows up here and makes
    /// the budget be re-checked.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn type_sizes_are_pinned() {
        assert_eq!(size_of::<Entry>(), 12);
        assert_eq!(size_of::<(Vec<u8>, Object)>(), 56);
        assert_eq!(size_of::<Object>(), 32);
        // The key of the span, page-walk and cache maps.
        assert_eq!(size_of::<crate::ObjRef>(), 8);
        assert_eq!(PARSE_OBJECT_BYTES, 272);
        assert_eq!(PAGE_WALK_BYTES, 48);
        assert_eq!(SPAN_ENTRY_BYTES, 64);
    }

    /// The reserve, item by item, as literals.
    #[test]
    fn reserve_is_itemized() {
        assert_eq!(
            [
                RESERVE_BINARY_BYTES,
                RESERVE_STACK_BYTES,
                RESERVE_THREAD_ARENA_BYTES,
                RESERVE_FRAGMENTATION_BYTES,
            ],
            [64 * MIB, 32 * MIB, 64 * MIB, 96 * MIB]
        );
        assert_eq!(RESERVE_BYTES, 256 * MIB);
    }

    /// Every term at the defaults, as literals (not rebuilt from the constants under test).
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn default_terms() {
        let b = MemoryBudget::of(&Limits::default());
        assert_eq!(
            b,
            MemoryBudget {
                file: 512 * MIB,
                retained: 128 * MIB,
                // 2 097 152 entries × 12 bytes × 3.
                xref_table: 72 * MIB,
                // (2 097 152 + 1 024) spans × 64 bytes.
                spans: 128 * MIB + 64 * 1024,
                // 1 000 000 pages × 48 bytes.
                page_walk: 48_000_000,
                // 1 048 576 × 272 + 32 MiB × 3.
                parse: 272 * MIB + 96 * MIB,
                // 128 MiB decoded + 256 MiB working.
                decode: 384 * MIB,
                raster: 256 * MIB,
            }
        );
        assert_eq!(b.persistent(), 840 * MIB + 64 * 1024 + 48_000_000);
        assert_eq!(b.largest_phase(), 384 * MIB);
        assert_eq!(b.heap_high_water(), 384 * MIB);
        // 840 + 384 + 384 + 256 MiB, plus the odd spans and page-walk bytes: about
        // 1909.84 MiB, 138.16 MiB under the 2 GiB.
        assert_eq!(b.peak(), 1864 * MIB + 64 * 1024 + 48_000_000);
    }

    /// The defaults fit the judge's 2 GiB with the 256 MiB reserve, and with at least 64 MiB
    /// to spare, so a small change to one term can't silently use up the reserve.
    #[test]
    fn defaults_fit_the_process_budget_with_the_reserve() {
        assert_eq!(PROCESS_BUDGET_BYTES, 2048 * MIB);
        let peak = MemoryBudget::of(&Limits::default()).peak();
        assert!(
            peak.saturating_add(64 * MIB) <= PROCESS_BUDGET_BYTES,
            "peak {peak} bytes"
        );
    }

    /// A field name, how to lower it by one, and how much each term falls.
    type Case = (&'static str, fn(&mut Limits), [u64; 8]);

    /// Lowers one field by one unit and returns how much each term fell, in field order.
    fn drop_by_one(f: fn(&mut Limits)) -> [u64; 8] {
        let base = Limits::default();
        let mut l = base.clone();
        f(&mut l);
        let (b0, b) = (MemoryBudget::of(&base), MemoryBudget::of(&l));
        [
            b0.file.abs_diff(b.file),
            b0.retained.abs_diff(b.retained),
            b0.xref_table.abs_diff(b.xref_table),
            b0.spans.abs_diff(b.spans),
            b0.page_walk.abs_diff(b.page_walk),
            b0.parse.abs_diff(b.parse),
            b0.decode.abs_diff(b.decode),
            b0.raster.abs_diff(b.raster),
        ]
    }

    /// Each term follows its own limit fields, and only those: lowering any one `Limits` field
    /// by one unit moves exactly the terms (and amounts) the formula says, and every other
    /// field moves nothing. Every default is above zero, so lowering by one always changes it.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn each_term_follows_its_fields() {
        // Exhaustive: a new `Limits` field fails to compile until it is listed below.
        let Limits {
            max_file_size: _,
            max_nesting_depth: _,
            max_objects_per_parse: _,
            max_objects_per_document: _,
            max_string_bytes_per_parse: _,
            max_xref_entries: _,
            max_xref_sections: _,
            max_pages: _,
            lex_budget_factor: _,
            lex_budget_slack: _,
            max_decoded_bytes_per_stream: _,
            max_decoded_bytes_per_document: _,
            max_retained_bytes_per_document: _,
            max_image_pixels: _,
            max_image_dimension: _,
            max_decode_working_bytes: _,
            max_decode_ops: _,
            max_image_components: _,
            max_font_program_bytes: _,
            max_font_glyphs: _,
            max_outline_points: _,
            max_outline_ops: _,
            max_canvas_pixels: _,
            max_path_segments: _,
            max_clip_depth: _,
            max_layer_depth: _,
            max_raster_working_bytes: _,
        } = Limits::default();
        let cases: [Case; 27] = [
            (
                "file size",
                |l| l.max_file_size -= 1,
                [1, 0, 0, 0, 0, 0, 0, 0],
            ),
            ("nesting", |l| l.max_nesting_depth -= 1, [0; 8]),
            (
                "objects/parse",
                |l| l.max_objects_per_parse -= 1,
                [0, 0, 0, 0, 0, 272, 0, 0],
            ),
            ("objects/doc", |l| l.max_objects_per_document -= 1, [0; 8]),
            (
                "strings/parse",
                |l| l.max_string_bytes_per_parse -= 1,
                [0, 0, 0, 0, 0, 3, 0, 0],
            ),
            (
                "xref entries",
                |l| l.max_xref_entries -= 1,
                [0, 0, 36, 64, 0, 0, 0, 0],
            ),
            (
                "xref sections",
                |l| l.max_xref_sections -= 1,
                [0, 0, 0, 64, 0, 0, 0, 0],
            ),
            ("pages", |l| l.max_pages -= 1, [0, 0, 0, 0, 48, 0, 0, 0]),
            ("lex factor", |l| l.lex_budget_factor -= 1, [0; 8]),
            ("lex slack", |l| l.lex_budget_slack -= 1, [0; 8]),
            (
                "decoded/stream",
                |l| l.max_decoded_bytes_per_stream -= 1,
                [0, 0, 0, 0, 0, 0, 1, 0],
            ),
            (
                "decoded/doc",
                |l| l.max_decoded_bytes_per_document -= 1,
                [0; 8],
            ),
            (
                "retained",
                |l| l.max_retained_bytes_per_document -= 1,
                [0, 1, 0, 0, 0, 0, 0, 0],
            ),
            ("image pixels", |l| l.max_image_pixels -= 1, [0; 8]),
            ("image dimension", |l| l.max_image_dimension -= 1, [0; 8]),
            (
                "decode working",
                |l| l.max_decode_working_bytes -= 1,
                [0, 0, 0, 0, 0, 0, 1, 0],
            ),
            ("decode ops", |l| l.max_decode_ops -= 1, [0; 8]),
            ("components", |l| l.max_image_components -= 1, [0; 8]),
            ("font program", |l| l.max_font_program_bytes -= 1, [0; 8]),
            ("glyphs", |l| l.max_font_glyphs -= 1, [0; 8]),
            ("outline points", |l| l.max_outline_points -= 1, [0; 8]),
            ("outline ops", |l| l.max_outline_ops -= 1, [0; 8]),
            ("canvas", |l| l.max_canvas_pixels -= 1, [0; 8]),
            ("segments", |l| l.max_path_segments -= 1, [0; 8]),
            ("clips", |l| l.max_clip_depth -= 1, [0; 8]),
            ("layers", |l| l.max_layer_depth -= 1, [0; 8]),
            (
                "raster working",
                |l| l.max_raster_working_bytes -= 1,
                [0, 0, 0, 0, 0, 0, 0, 1],
            ),
        ];
        for (name, f, want) in cases {
            assert_eq!(drop_by_one(f), want, "{name}");
        }
    }

    /// Loose caller limits saturate every term instead of overflowing.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn loose_limits_saturate_every_term() {
        let loose = Limits {
            max_file_size: usize::MAX,
            max_retained_bytes_per_document: u64::MAX,
            max_xref_entries: usize::MAX,
            max_xref_sections: usize::MAX,
            max_pages: usize::MAX,
            max_objects_per_parse: usize::MAX,
            max_string_bytes_per_parse: usize::MAX,
            max_decoded_bytes_per_stream: usize::MAX,
            max_decode_working_bytes: u64::MAX,
            max_raster_working_bytes: u64::MAX,
            ..Limits::default()
        };
        let b = MemoryBudget::of(&loose);
        assert_eq!(
            b,
            MemoryBudget {
                file: u64::MAX,
                retained: u64::MAX,
                xref_table: u64::MAX,
                spans: u64::MAX,
                page_walk: u64::MAX,
                parse: u64::MAX,
                decode: u64::MAX,
                raster: u64::MAX,
            }
        );
        assert_eq!(b.peak(), u64::MAX);
        // One field loose at a time: each product saturates on its own (a wrapping product
        // would come out small).
        let one = |f: fn(&mut Limits)| {
            let mut l = Limits::default();
            f(&mut l);
            MemoryBudget::of(&l)
        };
        assert_eq!(
            one(|l| l.max_xref_entries = usize::MAX).xref_table,
            u64::MAX
        );
        assert_eq!(one(|l| l.max_xref_entries = usize::MAX).spans, u64::MAX);
        assert_eq!(one(|l| l.max_xref_sections = usize::MAX).spans, u64::MAX);
        assert_eq!(one(|l| l.max_pages = usize::MAX).page_walk, u64::MAX);
        assert_eq!(
            one(|l| l.max_objects_per_parse = usize::MAX).parse,
            u64::MAX
        );
        assert_eq!(
            one(|l| l.max_string_bytes_per_parse = usize::MAX).parse,
            u64::MAX
        );
        assert_eq!(
            one(|l| l.max_decode_working_bytes = u64::MAX).decode,
            u64::MAX
        );
    }

    /// Only the largest phase counts, whichever it is, twice (once as the heap an earlier phase
    /// left mapped, #146), and the peak adds the reserve.
    #[test]
    fn peak_is_persistent_plus_largest_phase_twice_plus_reserve() {
        let b = MemoryBudget {
            file: 1,
            retained: 2,
            xref_table: 4,
            spans: 8,
            page_walk: 16,
            parse: 32,
            decode: 64,
            raster: 128,
        };
        assert_eq!(b.persistent(), 31);
        assert_eq!(b.peak(), 31 + 128 + 128 + RESERVE_BYTES);
        for (parse, decode, raster) in [(128, 64, 32), (32, 128, 64), (64, 32, 128)] {
            let b = MemoryBudget {
                parse,
                decode,
                raster,
                ..b
            };
            assert_eq!(b.largest_phase(), 128);
            assert_eq!(b.heap_high_water(), 128);
            assert_eq!(b.peak(), 31 + 256 + RESERVE_BYTES);
        }
        let max = MemoryBudget {
            file: u64::MAX,
            ..b
        };
        assert_eq!(max.peak(), u64::MAX);
    }
}
