//! Resource limits (PLAN.md §2.2). They live here, at the bottom of the dependency graph, so
//! every crate and every backend trait shares one definition (ADR 0004).

use crate::BackendError;

/// Resource limits enforced on every operation (PLAN.md §2.2). Never raise a check for speed
/// (CONSTITUTION 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Largest accepted file, in bytes.
    pub max_file_size: usize,
    /// Deepest nesting of arrays/dictionaries inside one object.
    pub max_nesting_depth: usize,
    /// Objects (scalars and containers) produced while parsing one top-level object;
    /// bounds memory for huge arrays/dictionaries.
    pub max_objects_per_parse: usize,
    /// Objects produced across all parses of one document (time bound: cheap-to-write objects
    /// such as `[]` are expensive to build).
    pub max_objects_per_document: usize,
    /// Decoded string and name bytes produced while parsing one top-level object (memory bound).
    pub max_string_bytes_per_parse: usize,
    /// Total cross-reference entries across all sections.
    pub max_xref_entries: usize,
    /// Cross-reference sections followed through `/Prev`.
    pub max_xref_sections: usize,
    /// Pages in one document.
    pub max_pages: usize,
    /// Bytes the lexer may scan per document, as a multiple of the file size (plus
    /// `lex_budget_slack`). Legitimate files lex each byte about once.
    pub lex_budget_factor: usize,
    /// Fixed allowance added to the lexing budget (covers tiny files).
    pub lex_budget_slack: usize,
    /// Bytes one backend call may produce: one decoded stream, image or cipher output
    /// (zip-bomb guard).
    pub max_decoded_bytes_per_stream: usize,
    /// Bytes all decoding of one document may produce over its lifetime. `u64` because this
    /// exceeds `usize` on wasm32. The engine charges every backend call against it. This is a
    /// time bound (many small bombs), not a memory bound: see
    /// `max_retained_bytes_per_document`.
    pub max_decoded_bytes_per_document: u64,
    /// Bytes one document may keep alive between calls: cached decoded streams, font programs
    /// held by cached faces, cached parsed objects. Every cache the engine adds charges what
    /// it holds here and evicts (or recomputes) instead of exceeding it.
    pub max_retained_bytes_per_document: u64,
    /// Pixels (width × height) in one decoded image.
    pub max_image_pixels: u64,
    /// Width or height of one decoded image, in pixels.
    pub max_image_dimension: u32,
    /// Peak bytes one image decode may hold at once, output included (JPEG, JPEG 2000, JBIG2,
    /// CCITT; specs/core/decode-work-limits.md). The decode term of the 2 GiB budget is this plus
    /// `max_decoded_bytes_per_stream`, the decoder's inputs together ([`Limits::image_input_budget`],
    /// specs/core/memory-budget.md).
    pub max_decode_working_bytes: u64,
    /// Operations one image decode may perform: at least one per output sample, plus work that
    /// produces no output (tiles, layers, scans, symbol instances). CPU bound per call.
    pub max_decode_ops: u64,
    /// Components (channels, including alpha) per decoded image pixel. ISO 32000-2 Annex C
    /// limits DeviceN to 32 colorants; one more allows an alpha channel (`SMaskInData`).
    pub max_image_components: u16,
    /// Bytes in one font program handed to a font backend (after stream decoding).
    pub max_font_program_bytes: usize,
    /// Glyphs in one font face.
    pub max_font_glyphs: u16,
    /// Points in one glyph outline, composites expanded (see
    /// [`crate::PathSegment::point_count`]).
    pub max_outline_points: u32,
    /// Operations in one outline call: charstring operators executed plus composite
    /// components visited (CPU bound for subroutine and composite loops).
    pub max_outline_ops: u32,
    /// Pixels in one rasterizer canvas (one tile; the engine renders larger views in tiles).
    pub max_canvas_pixels: u64,
    /// Segments in one rasterizer path, and dashes one dashed stroke may produce.
    pub max_path_segments: u32,
    /// Nesting depth of rasterizer clips.
    pub max_clip_depth: u16,
    /// Nesting depth of rasterizer layers (transparency groups).
    pub max_layer_depth: u16,
    /// Nominal working memory of one render (see [`crate::SceneStats::working_bytes`]).
    pub max_raster_working_bytes: u64,
}

impl Limits {
    /// These limits with every field lowered to at most `ceiling`'s. A caller can tighten the
    /// limits but never loosen them past the ceiling: `Document::open` clamps whatever a
    /// front end passes (Swift, JS, CLI) to [`Limits::default`] (CONSTITUTION 4 and 5). Every
    /// field is an upper bound, so the field-wise minimum is never looser than either side.
    pub fn clamped_to(&self, ceiling: &Limits) -> Limits {
        // Exhaustive destructuring: a new field fails to compile until it is listed here. That
        // it is really clamped is checked by the tests' `all_fields` literal, which must list
        // it too.
        let Limits {
            max_file_size,
            max_nesting_depth,
            max_objects_per_parse,
            max_objects_per_document,
            max_string_bytes_per_parse,
            max_xref_entries,
            max_xref_sections,
            max_pages,
            lex_budget_factor,
            lex_budget_slack,
            max_decoded_bytes_per_stream,
            max_decoded_bytes_per_document,
            max_retained_bytes_per_document,
            max_image_pixels,
            max_image_dimension,
            max_decode_working_bytes,
            max_decode_ops,
            max_image_components,
            max_font_program_bytes,
            max_font_glyphs,
            max_outline_points,
            max_outline_ops,
            max_canvas_pixels,
            max_path_segments,
            max_clip_depth,
            max_layer_depth,
            max_raster_working_bytes,
        } = self;
        Limits {
            max_file_size: (*max_file_size).min(ceiling.max_file_size),
            max_nesting_depth: (*max_nesting_depth).min(ceiling.max_nesting_depth),
            max_objects_per_parse: (*max_objects_per_parse).min(ceiling.max_objects_per_parse),
            max_objects_per_document: (*max_objects_per_document)
                .min(ceiling.max_objects_per_document),
            max_string_bytes_per_parse: (*max_string_bytes_per_parse)
                .min(ceiling.max_string_bytes_per_parse),
            max_xref_entries: (*max_xref_entries).min(ceiling.max_xref_entries),
            max_xref_sections: (*max_xref_sections).min(ceiling.max_xref_sections),
            max_pages: (*max_pages).min(ceiling.max_pages),
            lex_budget_factor: (*lex_budget_factor).min(ceiling.lex_budget_factor),
            lex_budget_slack: (*lex_budget_slack).min(ceiling.lex_budget_slack),
            max_decoded_bytes_per_stream: (*max_decoded_bytes_per_stream)
                .min(ceiling.max_decoded_bytes_per_stream),
            max_decoded_bytes_per_document: (*max_decoded_bytes_per_document)
                .min(ceiling.max_decoded_bytes_per_document),
            max_retained_bytes_per_document: (*max_retained_bytes_per_document)
                .min(ceiling.max_retained_bytes_per_document),
            max_image_pixels: (*max_image_pixels).min(ceiling.max_image_pixels),
            max_image_dimension: (*max_image_dimension).min(ceiling.max_image_dimension),
            max_decode_working_bytes: (*max_decode_working_bytes)
                .min(ceiling.max_decode_working_bytes),
            max_decode_ops: (*max_decode_ops).min(ceiling.max_decode_ops),
            max_image_components: (*max_image_components).min(ceiling.max_image_components),
            max_font_program_bytes: (*max_font_program_bytes).min(ceiling.max_font_program_bytes),
            max_font_glyphs: (*max_font_glyphs).min(ceiling.max_font_glyphs),
            max_outline_points: (*max_outline_points).min(ceiling.max_outline_points),
            max_outline_ops: (*max_outline_ops).min(ceiling.max_outline_ops),
            max_canvas_pixels: (*max_canvas_pixels).min(ceiling.max_canvas_pixels),
            max_path_segments: (*max_path_segments).min(ceiling.max_path_segments),
            max_clip_depth: (*max_clip_depth).min(ceiling.max_clip_depth),
            max_layer_depth: (*max_layer_depth).min(ceiling.max_layer_depth),
            max_raster_working_bytes: (*max_raster_working_bytes)
                .min(ceiling.max_raster_working_bytes),
        }
    }

    /// Total bytes the lexer may scan for a file of `data_len` bytes.
    pub fn lex_budget(&self, data_len: usize) -> usize {
        data_len
            .saturating_mul(self.lex_budget_factor)
            .saturating_add(self.lex_budget_slack)
    }

    /// The limits for one backend call, given how many decoded bytes the document may still
    /// produce. The output cap is the smaller of the per-stream limit and the remaining
    /// document budget.
    pub fn decode_limits(&self, remaining_document_bytes: u64) -> DecodeLimits {
        let remaining = usize::try_from(remaining_document_bytes).unwrap_or(usize::MAX);
        DecodeLimits {
            max_output_bytes: self.max_decoded_bytes_per_stream.min(remaining),
            max_image_pixels: self.max_image_pixels,
            max_image_dimension: self.max_image_dimension,
            max_image_components: self.max_image_components,
            max_working_bytes: self.max_decode_working_bytes,
            max_ops: self.max_decode_ops,
        }
    }

    /// The limits for rasterizer calls.
    pub fn raster_limits(&self) -> RasterLimits {
        RasterLimits {
            max_canvas_pixels: self.max_canvas_pixels,
            max_path_segments: self.max_path_segments,
            max_clip_depth: self.max_clip_depth,
            max_layer_depth: self.max_layer_depth,
            max_working_bytes: self.max_raster_working_bytes,
        }
    }

    /// The limits for font backend calls.
    pub fn font_limits(&self) -> FontLimits {
        FontLimits {
            max_font_program_bytes: self.max_font_program_bytes,
            max_glyphs: self.max_font_glyphs,
            max_outline_points: self.max_outline_points,
            max_outline_ops: self.max_outline_ops,
        }
    }
}

/// The defaults fit the judge's 2 GiB address space by the estimate in
/// specs/core/memory-budget.md, which counts the heap one earlier phase leaves mapped (#146
/// lowered the string, xref, decoded-stream, retained and raster defaults for it). It is not a
/// proven bound: holes several phases leave are a known gap (#146).
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_size: 512 * 1024 * 1024,
            max_nesting_depth: 256,
            max_objects_per_parse: 1 << 20,
            max_objects_per_document: 20_000_000,
            max_string_bytes_per_parse: 32 * 1024 * 1024,
            max_xref_entries: 1 << 21,
            max_xref_sections: 1024,
            max_pages: 1_000_000,
            lex_budget_factor: 4,
            lex_budget_slack: 1024 * 1024,
            max_decoded_bytes_per_stream: 128 * 1024 * 1024,
            max_decoded_bytes_per_document: 4 * 1024 * 1024 * 1024,
            max_retained_bytes_per_document: 128 * 1024 * 1024,
            max_image_pixels: 100_000_000,
            max_image_dimension: 1 << 20,
            max_decode_working_bytes: 256 * 1024 * 1024,
            max_decode_ops: 1 << 32,
            max_image_components: 33,
            max_font_program_bytes: 64 * 1024 * 1024,
            max_font_glyphs: u16::MAX,
            max_outline_points: 1 << 17,
            max_outline_ops: 1 << 20,
            max_canvas_pixels: 1 << 22,
            max_path_segments: 1 << 20,
            max_clip_depth: 64,
            max_layer_depth: 32,
            max_raster_working_bytes: 256 * 1024 * 1024,
        }
    }
}

/// The limits one backend call must enforce. Backends check them **before** allocating:
/// output larger than `max_output_bytes`, or an image whose header exceeds a pixel, dimension
/// or component limit, fails with [`crate::BackendError::LimitExceeded`]. Image decoders also
/// charge their working memory and operations before the allocation or work they cover, and
/// report the totals in [`crate::Decoded::usage`] (specs/core/decode-work-limits.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Most bytes the call may return.
    pub max_output_bytes: usize,
    /// Most pixels (width × height) in a returned image or bitmap.
    pub max_image_pixels: u64,
    /// Largest width or height of a returned image or bitmap.
    pub max_image_dimension: u32,
    /// Most components per pixel of a returned image.
    pub max_image_components: u16,
    /// Most bytes an image decode may hold at once, output included
    /// ([`crate::DecodeUsage::working_bytes`]). Inflate, Deflate and Cipher ignore it.
    pub max_working_bytes: u64,
    /// Most operations an image decode may perform ([`crate::DecodeUsage::ops`]). Inflate,
    /// Deflate and Cipher ignore it.
    pub max_ops: u64,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Limits::default().decode_limits(u64::MAX)
    }
}

impl Limits {
    /// The allowance the decoded inputs of one image decode share: `max_decoded_bytes_per_stream`
    /// in total, not per input. [`crate::Jbig2Decoder::decode`] takes the page data and the
    /// decoded `/JBIG2Globals` together; if each could use the whole per-stream cap, the decode
    /// phase would hold two caps of input, which the memory budget's decode term does not
    /// count (specs/core/memory-budget.md, #168).
    pub fn image_input_budget(&self) -> ImageInputBudget {
        ImageInputBudget {
            allowance: self.max_decoded_bytes_per_stream,
            remaining: self.max_decoded_bytes_per_stream,
        }
    }
}

/// What is left of the shared input allowance of one image decode ([`Limits::image_input_budget`]).
///
/// It counts the buffers the caller allocates, not the slices the decoder gets. For each input
/// the caller runs the filter chain one stage at a time with [`ImageInputBudget::cap`] applied
/// to the stage's [`DecodeLimits`], so a stage fails with [`BackendError::LimitExceeded`] before
/// it outgrows what the buffers still alive left. After a stage it shrinks the output
/// (`Vec::shrink_to_fit`) and charges its **capacity** with [`ImageInputBudget::charge`], then
/// drops the stage's input buffer and gives its charge back with [`ImageInputBudget::release`].
/// An input that borrows the file bytes (no filter before the image filter, no encryption)
/// holds no new buffer and is not charged; every input held in its own buffer is, cached ones
/// included.
///
/// Neither `Copy` nor `Clone`: a copy charged in place of the original would lose the charge.
#[derive(Debug, PartialEq, Eq)]
pub struct ImageInputBudget {
    allowance: usize,
    remaining: usize,
}

impl ImageInputBudget {
    /// Bytes the buffers not yet charged may still hold.
    pub fn remaining(&self) -> usize {
        self.remaining
    }

    /// `limits` with `max_output_bytes` lowered to [`ImageInputBudget::remaining`] (never
    /// raised), for the next filter stage.
    pub fn cap(&self, limits: DecodeLimits) -> DecodeLimits {
        DecodeLimits {
            max_output_bytes: limits.max_output_bytes.min(self.remaining),
            ..limits
        }
    }

    /// Charges a buffer of `capacity` bytes (its `Vec::capacity()`, not its length: spare
    /// capacity is memory too). Fails with [`BackendError::LimitExceeded`] and charges nothing
    /// when it is more than what remains.
    pub fn charge(&mut self, capacity: usize) -> Result<(), BackendError> {
        match self.remaining.checked_sub(capacity) {
            Some(left) => {
                self.remaining = left;
                Ok(())
            }
            None => Err(BackendError::LimitExceeded(
                "decoded bytes of one image decode's inputs",
            )),
        }
    }

    /// Gives back the charge of a buffer the caller has dropped (the same `capacity` it
    /// charged). What remains never grows past the allowance, so releasing more than was
    /// charged cannot raise it.
    pub fn release(&mut self, capacity: usize) {
        self.remaining = self.remaining.saturating_add(capacity).min(self.allowance);
    }
}

/// The limits font backend calls must enforce ([`crate::FontEngine`], [`crate::Type1Parser`],
/// [`crate::FontFace::outline`]). Font output is structured, so it is bounded by counts
/// rather than bytes: a face holds at most `max_glyphs` glyphs and one outline at most
/// `max_outline_points` points. Exceeding a bound fails with
/// [`crate::BackendError::LimitExceeded`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontLimits {
    /// Longest accepted font program, checked before parsing.
    pub max_font_program_bytes: usize,
    /// Most glyphs in a face, checked at load.
    pub max_glyphs: u16,
    /// Most points in one outline, composites and subroutines expanded.
    pub max_outline_points: u32,
    /// Most operations in one outline call (charstring operators executed plus composite
    /// components visited).
    pub max_outline_ops: u32,
}

impl Default for FontLimits {
    fn default() -> Self {
        Limits::default().font_limits()
    }
}

/// The limits a [`crate::Rasterizer`] must enforce before drawing
/// ([`crate::check_scene`] checks them all). Exceeding one fails with
/// [`crate::BackendError::LimitExceeded`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterLimits {
    /// Most canvas pixels (width × height).
    pub max_canvas_pixels: u64,
    /// Most segments in one path; also the most dashes one dashed stroke may produce.
    pub max_path_segments: u32,
    /// Deepest clip nesting.
    pub max_clip_depth: u16,
    /// Deepest layer nesting.
    pub max_layer_depth: u16,
    /// Most nominal working bytes (see [`crate::SceneStats::working_bytes`]).
    pub max_working_bytes: u64,
}

impl Default for RasterLimits {
    fn default() -> Self {
        Limits::default().raster_limits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT_ERROR: BackendError =
        BackendError::LimitExceeded("decoded bytes of one image decode's inputs");

    /// JBIG2 page data and globals together (#168): at the allowance both inputs fit, in either
    /// order; one byte past it the second input is refused and nothing is charged.
    #[test]
    fn image_inputs_share_one_allowance() {
        let limits = Limits {
            max_decoded_bytes_per_stream: 100,
            ..Limits::default()
        };
        for (first, second) in [(60, 40), (40, 60), (0, 100), (100, 0)] {
            let mut budget = limits.image_input_budget();
            assert_eq!(budget.remaining(), 100);
            assert_eq!(budget.charge(first), Ok(()), "{first} + {second}");
            assert_eq!(budget.remaining(), 100 - first);
            assert_eq!(budget.charge(second), Ok(()), "{first} + {second}");
            assert_eq!(budget.remaining(), 0);
            // Nothing more fits, not even one byte; an empty input still does.
            assert_eq!(budget.charge(1), Err(LIMIT_ERROR));
            assert_eq!(budget.charge(0), Ok(()));
        }
        for (first, second) in [(60, 41), (41, 60), (0, 101), (101, 0)] {
            let mut budget = limits.image_input_budget();
            let (fits, over) = if first > 100 {
                (second, first)
            } else {
                (first, second)
            };
            assert_eq!(budget.charge(fits), Ok(()), "{first} + {second}");
            assert_eq!(budget.charge(over), Err(LIMIT_ERROR), "{first} + {second}");
            // The refused charge took nothing: what remained still fits exactly.
            assert_eq!(budget.remaining(), 100 - fits);
            assert_eq!(budget.charge(100 - fits), Ok(()));
            assert_eq!(budget.remaining(), 0);
        }
    }

    /// The same boundary at the default limits: 64 MiB of page data plus 64 MiB of globals is
    /// the whole 128 MiB allowance; one byte more is refused.
    #[test]
    fn default_image_input_allowance_is_exact() {
        const MIB: usize = 1 << 20;
        let limits = Limits::default();
        let mut budget = limits.image_input_budget();
        assert_eq!(budget.remaining(), 128 * MIB);
        assert_eq!(budget.charge(64 * MIB), Ok(()));
        assert_eq!(budget.charge(64 * MIB), Ok(()));
        assert_eq!(budget.remaining(), 0);
        let mut budget = limits.image_input_budget();
        assert_eq!(budget.charge(64 * MIB), Ok(()));
        assert_eq!(budget.charge(64 * MIB + 1), Err(LIMIT_ERROR));
        // A full-cap page alone leaves nothing for globals.
        let mut budget = limits.image_input_budget();
        assert_eq!(budget.charge(128 * MIB), Ok(()));
        assert_eq!(budget.charge(1), Err(LIMIT_ERROR));
    }

    /// `cap` lowers only the output cap, to what remains, and never raises it; every other
    /// field (all distinct, none at its default) passes through unchanged.
    #[test]
    fn image_input_cap_lowers_only_the_output_cap() {
        let limits = Limits {
            max_decoded_bytes_per_stream: 100,
            ..Limits::default()
        };
        let tightened = |max_output_bytes| DecodeLimits {
            max_output_bytes,
            max_image_pixels: 7,
            max_image_dimension: 8,
            max_image_components: 9,
            max_working_bytes: 10,
            max_ops: 11,
        };
        let mut budget = limits.image_input_budget();
        assert_eq!(budget.cap(tightened(1000)), tightened(100));
        assert_eq!(budget.charge(70), Ok(()));
        // The allowance binds.
        assert_eq!(budget.cap(tightened(1000)), tightened(30));
        // A smaller document budget stays the binding cap.
        assert_eq!(budget.cap(tightened(10)), tightened(10));
        assert_eq!(budget.charge(30), Ok(()));
        assert_eq!(budget.cap(tightened(1000)), tightened(0));
    }

    /// Spare capacity is charged (R1.1 on PR #186): a first input of 10 bytes left in a buffer
    /// reserved at the whole allowance takes all of it, so the second input gets nothing; once
    /// it is shrunk to its length, the rest is available.
    #[test]
    fn image_input_budget_charges_capacity() {
        let limits = Limits {
            max_decoded_bytes_per_stream: 100,
            ..Limits::default()
        };
        let mut budget = limits.image_input_budget();
        assert_eq!(budget.charge(100), Ok(()));
        assert_eq!(
            budget.cap(limits.decode_limits(u64::MAX)).max_output_bytes,
            0
        );
        assert_eq!(budget.charge(1), Err(LIMIT_ERROR));
        let mut budget = limits.image_input_budget();
        assert_eq!(budget.charge(10), Ok(()));
        assert_eq!(
            budget.cap(limits.decode_limits(u64::MAX)).max_output_bytes,
            90
        );
        assert_eq!(budget.charge(90), Ok(()));
        assert_eq!(budget.charge(1), Err(LIMIT_ERROR));
    }

    /// A two-stage chain: the first stage's output stays charged while the second grows, and
    /// its charge comes back once it is dropped. `release` never lifts what remains past the
    /// allowance.
    #[test]
    fn image_input_budget_release_returns_dropped_stages() {
        let limits = Limits {
            max_decoded_bytes_per_stream: 100,
            ..Limits::default()
        };
        let mut budget = limits.image_input_budget();
        // Stage 1 of the page chain: 30 bytes.
        assert_eq!(budget.charge(30), Ok(()));
        // Stage 2 may grow to 70 while stage 1 is alive, not 71.
        assert_eq!(
            budget.cap(limits.decode_limits(u64::MAX)).max_output_bytes,
            70
        );
        assert_eq!(budget.charge(70), Ok(()));
        // Stage 1 dropped: 30 bytes come back for the globals, not 31.
        budget.release(30);
        assert_eq!(budget.remaining(), 30);
        assert_eq!(budget.charge(31), Err(LIMIT_ERROR));
        assert_eq!(budget.charge(30), Ok(()));
        assert_eq!(budget.remaining(), 0);
        // Over-release clamps at the allowance, from a full budget and from a partly used one.
        let mut budget = limits.image_input_budget();
        budget.release(5);
        assert_eq!(budget.remaining(), 100);
        assert_eq!(budget.charge(60), Ok(()));
        budget.release(usize::MAX);
        assert_eq!(budget.remaining(), 100);
        assert_eq!(budget.charge(101), Err(LIMIT_ERROR));
    }

    /// Huge lengths and loose limits neither overflow nor wrap.
    #[test]
    fn image_input_budget_saturates() {
        let mut budget = Limits::default().image_input_budget();
        assert_eq!(budget.charge(usize::MAX), Err(LIMIT_ERROR));
        assert_eq!(budget.remaining(), 128 << 20);
        let loose = Limits {
            max_decoded_bytes_per_stream: usize::MAX,
            ..Limits::default()
        };
        let mut budget = loose.image_input_budget();
        assert_eq!(budget.charge(usize::MAX - 1), Ok(()));
        assert_eq!(budget.charge(2), Err(LIMIT_ERROR));
        assert_eq!(budget.charge(1), Ok(()));
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn decode_limits_take_the_smaller_of_stream_and_document_budget() {
        let limits = Limits::default();
        assert_eq!(
            limits.decode_limits(10).max_output_bytes,
            10,
            "remaining document budget smaller than the stream limit"
        );
        assert_eq!(
            limits.decode_limits(u64::MAX).max_output_bytes,
            limits.max_decoded_bytes_per_stream
        );
        assert_eq!(limits.decode_limits(0).max_output_bytes, 0);
    }

    #[test]
    fn decode_limits_copy_the_work_budgets() {
        let limits = Limits {
            max_decode_working_bytes: 7,
            max_decode_ops: 8,
            ..Limits::default()
        };
        let d = limits.decode_limits(3);
        assert_eq!((d.max_working_bytes, d.max_ops), (7, 8));
        // The document budget lowers only the output cap.
        assert_eq!(d.max_output_bytes, 3);
        let d = Limits::default().decode_limits(0);
        assert_eq!(d.max_working_bytes, 256 * 1024 * 1024);
        assert_eq!(d.max_ops, 1 << 32);
    }

    /// The defaults #146 lowered, plus `max_decode_working_bytes` (kept at 256 MiB; the decode
    /// term depends on it), as literals (specs/core/memory-budget.md). Raising one makes the
    /// memory budget be re-checked.
    #[test]
    fn lowered_defaults_are_pinned() {
        let l = Limits::default();
        assert_eq!(l.max_xref_entries, 1 << 21);
        assert_eq!(l.max_retained_bytes_per_document, 128 << 20);
        assert_eq!(l.max_string_bytes_per_parse, 32 << 20);
        assert_eq!(l.max_decoded_bytes_per_stream, 128 << 20);
        assert_eq!(l.max_decode_working_bytes, 256 << 20);
        assert_eq!(l.max_raster_working_bytes, 256 << 20);
        assert_eq!(l.max_canvas_pixels, 1 << 22);
    }

    #[test]
    fn defaults_fit_a_32_bit_usize() {
        let limits = Limits::default();
        assert!(u32::try_from(limits.max_decoded_bytes_per_stream).is_ok());
        assert!(u32::try_from(limits.max_file_size).is_ok());
        assert!(u32::try_from(limits.max_font_program_bytes).is_ok());
    }

    #[test]
    fn raster_limits_copy_the_document_limits() {
        let limits = Limits {
            max_canvas_pixels: 1,
            max_path_segments: 2,
            max_clip_depth: 3,
            max_layer_depth: 4,
            max_raster_working_bytes: 5,
            ..Limits::default()
        };
        assert_eq!(
            limits.raster_limits(),
            RasterLimits {
                max_canvas_pixels: 1,
                max_path_segments: 2,
                max_clip_depth: 3,
                max_layer_depth: 4,
                max_working_bytes: 5,
            }
        );
    }

    #[test]
    fn default_raster_limits_fit_the_memory_budget() {
        // A full-size canvas (4 bytes per pixel) with 4 soft-masked layers (5 each), 32 clips
        // (1 each) and one canvas-sized image (4 per pixel) stays within the working bytes:
        // 2^22 x 60 = 240 MiB. This is `check_scene`'s formula, `pixels x (4 + S) + 4 x I`.
        let r = RasterLimits::default();
        let px = r.max_canvas_pixels;
        assert!(px * (4 + 4 * 5 + 32) + 4 * px <= r.max_working_bytes);
        // An eighth of the 2 GiB process budget (#146; jpdf_syntax::MemoryBudget checks the sum).
        assert_eq!(r.max_working_bytes, 256 * 1024 * 1024);
        // The canvas default: 2^22 pixels, e.g. a 2048 x 2048 tile.
        assert_eq!(r.max_canvas_pixels, 1 << 22);
    }

    /// Every field set to `v` (one literal, so a new field must be listed here too).
    fn all_fields(v: u16) -> Limits {
        let n = usize::from(v);
        let w = u64::from(v);
        Limits {
            max_file_size: n,
            max_nesting_depth: n,
            max_objects_per_parse: n,
            max_objects_per_document: n,
            max_string_bytes_per_parse: n,
            max_xref_entries: n,
            max_xref_sections: n,
            max_pages: n,
            lex_budget_factor: n,
            lex_budget_slack: n,
            max_decoded_bytes_per_stream: n,
            max_decoded_bytes_per_document: w,
            max_retained_bytes_per_document: w,
            max_image_pixels: w,
            max_image_dimension: u32::from(v),
            max_decode_working_bytes: w,
            max_decode_ops: w,
            max_image_components: v,
            max_font_program_bytes: n,
            max_font_glyphs: v,
            max_outline_points: u32::from(v),
            max_outline_ops: u32::from(v),
            max_canvas_pixels: w,
            max_path_segments: u32::from(v),
            max_clip_depth: v,
            max_layer_depth: v,
            max_raster_working_bytes: w,
        }
    }

    #[test]
    fn clamping_lowers_every_looser_field_to_the_ceiling() {
        let ceiling = Limits::default();
        let loose = Limits {
            max_file_size: usize::MAX,
            max_nesting_depth: usize::MAX,
            max_objects_per_parse: usize::MAX,
            max_objects_per_document: usize::MAX,
            max_string_bytes_per_parse: usize::MAX,
            max_xref_entries: usize::MAX,
            max_xref_sections: usize::MAX,
            max_pages: usize::MAX,
            lex_budget_factor: usize::MAX,
            lex_budget_slack: usize::MAX,
            max_decoded_bytes_per_stream: usize::MAX,
            max_decoded_bytes_per_document: u64::MAX,
            max_retained_bytes_per_document: u64::MAX,
            max_image_pixels: u64::MAX,
            max_image_dimension: u32::MAX,
            max_decode_working_bytes: u64::MAX,
            max_decode_ops: u64::MAX,
            max_image_components: u16::MAX,
            max_font_program_bytes: usize::MAX,
            max_font_glyphs: u16::MAX,
            max_outline_points: u32::MAX,
            max_outline_ops: u32::MAX,
            max_canvas_pixels: u64::MAX,
            max_path_segments: u32::MAX,
            max_clip_depth: u16::MAX,
            max_layer_depth: u16::MAX,
            max_raster_working_bytes: u64::MAX,
        };
        assert_eq!(loose.clamped_to(&ceiling), ceiling);
        // At the ceiling stays at the ceiling.
        assert_eq!(ceiling.clamped_to(&ceiling), ceiling);
    }

    #[test]
    fn clamping_keeps_every_tighter_field() {
        let tight = all_fields(1);
        assert_eq!(tight.clamped_to(&Limits::default()), tight);
        assert_eq!(all_fields(0).clamped_to(&Limits::default()), all_fields(0));
    }

    #[test]
    fn clamping_works_per_field_one_past_the_ceiling() {
        // Ceiling 5 everywhere: 6 (one past) comes back as 5, 5 and 4 are kept. A field
        // clamped to the wrong ceiling field, or skipped, shows up as a mismatch.
        let ceiling = all_fields(5);
        assert_eq!(all_fields(6).clamped_to(&ceiling), ceiling);
        assert_eq!(all_fields(5).clamped_to(&ceiling), all_fields(5));
        assert_eq!(all_fields(4).clamped_to(&ceiling), all_fields(4));
    }

    #[test]
    fn clamping_uses_each_field_of_the_ceiling() {
        // Every ceiling field distinct (1..=27), so clamping a field against a different
        // ceiling field gives a wrong value even where two defaults coincide.
        let ceiling = Limits {
            max_file_size: 1,
            max_nesting_depth: 2,
            max_objects_per_parse: 3,
            max_objects_per_document: 4,
            max_string_bytes_per_parse: 5,
            max_xref_entries: 6,
            max_xref_sections: 7,
            max_pages: 8,
            lex_budget_factor: 9,
            lex_budget_slack: 10,
            max_decoded_bytes_per_stream: 11,
            max_decoded_bytes_per_document: 12,
            max_retained_bytes_per_document: 13,
            max_image_pixels: 14,
            max_image_dimension: 15,
            max_decode_working_bytes: 26,
            max_decode_ops: 27,
            max_image_components: 16,
            max_font_program_bytes: 17,
            max_font_glyphs: 18,
            max_outline_points: 19,
            max_outline_ops: 20,
            max_canvas_pixels: 21,
            max_path_segments: 22,
            max_clip_depth: 23,
            max_layer_depth: 24,
            max_raster_working_bytes: 25,
        };
        assert_eq!(all_fields(u16::MAX).clamped_to(&ceiling), ceiling);
        assert_eq!(all_fields(0).clamped_to(&ceiling), all_fields(0));
    }

    #[test]
    fn charged_budgets_do_not_exceed_the_process_budget() {
        // The four budgeted terms (the file held in memory, what a document
        // retains, one decode's working memory or output, whichever is larger, and one
        // render's working memory) sum to at most the judge's 2 GiB address-space cap
        // (PLAN §2.2). This is a coarse ceiling on the budgeted terms only; the full budget,
        // with the xref table, spans, one parse, the decoder's input and a reserve, is
        // `jpdf_syntax::MemoryBudget` (specs/core/memory-budget.md, #109).
        let l = Limits::default();
        let file = u64::try_from(l.max_file_size).unwrap_or(u64::MAX);
        let stream = u64::try_from(l.max_decoded_bytes_per_stream)
            .unwrap_or(u64::MAX)
            .max(l.max_decode_working_bytes);
        let total = file
            .saturating_add(l.max_retained_bytes_per_document)
            .saturating_add(stream)
            .saturating_add(l.max_raster_working_bytes);
        assert!(total <= 2 << 30, "{total} bytes");
    }

    /// Lowering `max_objects_per_parse` (#109) must keep `max_pages` reachable: a flat
    /// `/Kids` array of `max_pages` references, plus room for the rest of the `/Pages`
    /// dictionary, fits in one parse.
    #[test]
    fn per_parse_objects_hold_a_flat_page_tree() {
        let l = Limits::default();
        assert!(l.max_objects_per_parse >= l.max_pages.saturating_add(32_768));
    }

    #[test]
    fn font_limits_copy_the_document_limits() {
        let limits = Limits {
            max_font_program_bytes: 7,
            max_font_glyphs: 8,
            max_outline_points: 9,
            max_outline_ops: 10,
            ..Limits::default()
        };
        assert_eq!(
            limits.font_limits(),
            FontLimits {
                max_font_program_bytes: 7,
                max_glyphs: 8,
                max_outline_points: 9,
                max_outline_ops: 10,
            }
        );
    }
}
