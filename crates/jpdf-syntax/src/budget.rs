//! The process memory budget: what the default `Limits` let one document hold at its peak
//! (specs/core/memory-budget.md, #109). The judge runs `jpdf-cli` under a 2 GiB address-space
//! limit (ops/judge/CONTRACT.md); running out is a crash, so the defaults must fit it with a
//! reserve for everything no limit charges.

use crate::Limits;
use crate::object::Object;
use crate::xref::Entry;

/// The judge's address-space limit for one `jpdf-cli` process.
pub const PROCESS_BUDGET_BYTES: u64 = 2 << 30;

/// Room for what no limit charges: the binary and its mappings, thread stacks (main and the
/// engine thread, #114), allocator metadata and fragmentation, and small bookkeeping
/// (`/Prev` cycle set, error strings).
pub const RESERVE_BYTES: u64 = 256 << 20;

/// Bytes one recorded span costs in `XrefDocument`'s `BTreeMap<usize, usize>`. A std B-tree
/// leaf holds at most 11 pairs in a 192-byte node (88 + 88 + parent pointer and lengths); a
/// split leaves at least 5 in each half, and spans arrive mostly in ascending order, which
/// leaves the left halves at 5. Internal nodes (288 bytes, at least 6 children) add at most
/// 48 bytes per leaf: (192 + 48) / 5 = 48 bytes per span. 64 adds allocator rounding.
pub const SPAN_ENTRY_BYTES: u64 = 64;

/// A growing `Vec` holds at most twice its length; while it reallocates, the old buffer (one
/// length) and the new one (two lengths) are alive together. A stable sort's scratch buffer
/// (at most one length) fits in the same factor, since it runs after the growth.
const GROWTH_FACTOR: u64 = 3;

/// Peak bytes per term for one document under a set of limits. Persistent terms live as long
/// as the document; phase terms are alive one at a time (a document parses one object, decodes
/// one stream or renders one tile at a time), so only the largest counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBudget {
    /// The file bytes (`max_file_size`), read into memory once with an exact reservation.
    pub file: u64,
    /// Everything the document keeps between calls (`max_retained_bytes_per_document`):
    /// cached objects, decoded streams and font programs, and the decoded images a render's
    /// display list borrows.
    pub retained: u64,
    /// The cross-reference entry table at its peak: `max_xref_entries` entries, times the
    /// growth factor.
    pub xref_table: u64,
    /// The overlap map: one span per xref section and per resolved object (at most one object
    /// per entry, since resolve goes through the table).
    pub spans: u64,
    /// One parse: `max_objects_per_parse` objects, each at most a dictionary entry, plus
    /// `max_string_bytes_per_parse` string and name bytes, both times the growth factor.
    pub parse: u64,
    /// One decode: the filter chain's output feeding the image decoder
    /// (`max_decoded_bytes_per_stream`) plus the decoder's own working memory, output
    /// included (`max_decode_working_bytes`).
    pub decode: u64,
    /// One render's working memory (`max_raster_working_bytes`).
    pub raster: u64,
}

impl MemoryBudget {
    pub fn of(limits: &Limits) -> Self {
        let entry = size_u64(size_of::<Entry>());
        let slot = size_u64(size_of::<(Vec<u8>, Object)>());
        let spans = size_u64(limits.max_xref_entries)
            .saturating_add(size_u64(limits.max_xref_sections))
            .saturating_mul(SPAN_ENTRY_BYTES);
        let parse = size_u64(limits.max_objects_per_parse)
            .saturating_mul(slot)
            .saturating_add(size_u64(limits.max_string_bytes_per_parse))
            .saturating_mul(GROWTH_FACTOR);
        Self {
            file: size_u64(limits.max_file_size),
            retained: limits.max_retained_bytes_per_document,
            xref_table: size_u64(limits.max_xref_entries)
                .saturating_mul(entry)
                .saturating_mul(GROWTH_FACTOR),
            spans,
            parse,
            decode: size_u64(limits.max_decoded_bytes_per_stream)
                .saturating_add(limits.max_decode_working_bytes),
            raster: limits.max_raster_working_bytes,
        }
    }

    /// Terms alive for the whole life of the document.
    pub fn persistent(&self) -> u64 {
        self.file
            .saturating_add(self.retained)
            .saturating_add(self.xref_table)
            .saturating_add(self.spans)
    }

    /// The largest of the phases that run one at a time.
    pub fn largest_phase(&self) -> u64 {
        self.parse.max(self.decode).max(self.raster)
    }

    /// Peak process memory: persistent terms, the largest phase and [`RESERVE_BYTES`].
    pub fn peak(&self) -> u64 {
        self.persistent()
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

    const MIB: u64 = 1 << 20;

    /// The type sizes the formula multiplies, as literals: a change shows up here and makes
    /// the budget be re-checked.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn type_sizes_are_pinned() {
        assert_eq!(size_of::<Entry>(), 12);
        assert_eq!(size_of::<(Vec<u8>, Object)>(), 56);
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
                retained: 256 * MIB,
                // 4 194 304 entries × 12 bytes × 3.
                xref_table: 144 * MIB,
                // (4 194 304 + 1 024) spans × 64 bytes.
                spans: 256 * MIB + 64 * 1024,
                // (2 000 000 × 56 + 64 MiB) × 3.
                parse: 336_000_000 + 192 * MIB,
                decode: 512 * MIB,
                raster: 512 * MIB,
            }
        );
        assert_eq!(b.persistent(), 1168 * MIB + 64 * 1024);
        assert_eq!(b.largest_phase(), 336_000_000 + 192 * MIB);
    }

    /// The defaults fit the judge's 2 GiB with the 256 MiB reserve, and with at least 64 MiB
    /// to spare, so a small change to one term can't silently use up the reserve.
    #[test]
    fn defaults_fit_the_process_budget_with_the_reserve() {
        assert_eq!(PROCESS_BUDGET_BYTES, 2048 * MIB);
        assert_eq!(RESERVE_BYTES, 256 * MIB);
        let peak = MemoryBudget::of(&Limits::default()).peak();
        assert!(
            peak.saturating_add(64 * MIB) <= PROCESS_BUDGET_BYTES,
            "peak {peak} bytes"
        );
    }

    /// Each term follows its own limit fields, and only those: raising one field by one unit
    /// moves exactly the term (and amount) the formula says.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn each_term_follows_its_fields() {
        let base = Limits::default();
        let b0 = MemoryBudget::of(&base);
        let bump = |f: fn(&mut Limits)| {
            let mut l = base.clone();
            f(&mut l);
            let b = MemoryBudget::of(&l);
            [
                b.file - b0.file,
                b.retained - b0.retained,
                b.xref_table - b0.xref_table,
                b.spans - b0.spans,
                b.parse - b0.parse,
                b.decode - b0.decode,
                b.raster - b0.raster,
            ]
        };
        assert_eq!(bump(|l| l.max_file_size += 1), [1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            bump(|l| l.max_retained_bytes_per_document += 1),
            [0, 1, 0, 0, 0, 0, 0]
        );
        assert_eq!(bump(|l| l.max_xref_entries += 1), [0, 0, 36, 64, 0, 0, 0]);
        assert_eq!(bump(|l| l.max_xref_sections += 1), [0, 0, 0, 64, 0, 0, 0]);
        assert_eq!(
            bump(|l| l.max_objects_per_parse += 1),
            [0, 0, 0, 0, 168, 0, 0]
        );
        assert_eq!(
            bump(|l| l.max_string_bytes_per_parse += 1),
            [0, 0, 0, 0, 3, 0, 0]
        );
        assert_eq!(
            bump(|l| l.max_decoded_bytes_per_stream += 1),
            [0, 0, 0, 0, 0, 1, 0]
        );
        assert_eq!(
            bump(|l| l.max_decode_working_bytes += 1),
            [0, 0, 0, 0, 0, 1, 0]
        );
        assert_eq!(
            bump(|l| l.max_raster_working_bytes += 1),
            [0, 0, 0, 0, 0, 0, 1]
        );
    }

    /// Only the largest phase counts, whichever it is, and the peak adds the reserve.
    #[test]
    fn peak_is_persistent_plus_largest_phase_plus_reserve() {
        let b = MemoryBudget {
            file: 1,
            retained: 2,
            xref_table: 4,
            spans: 8,
            parse: 16,
            decode: 32,
            raster: 64,
        };
        assert_eq!(b.persistent(), 15);
        assert_eq!(b.peak(), 15 + 64 + RESERVE_BYTES);
        for (parse, decode, raster) in [(64, 32, 16), (16, 64, 32), (32, 16, 64)] {
            let b = MemoryBudget {
                parse,
                decode,
                raster,
                ..b
            };
            assert_eq!(b.largest_phase(), 64);
        }
        let max = MemoryBudget {
            file: u64::MAX,
            ..b
        };
        assert_eq!(max.peak(), u64::MAX);
    }
}
