//! Resource limits (PLAN.md §2.2). They live here, at the bottom of the dependency graph, so
//! every crate and every backend trait shares one definition (ADR 0004).

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
        // Exhaustive destructuring: a new field fails to compile until it is clamped here.
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

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_size: 512 * 1024 * 1024,
            max_nesting_depth: 256,
            max_objects_per_parse: 2_000_000,
            max_objects_per_document: 20_000_000,
            max_string_bytes_per_parse: 64 * 1024 * 1024,
            max_xref_entries: 10_000_000,
            max_xref_sections: 1024,
            max_pages: 1_000_000,
            lex_budget_factor: 4,
            lex_budget_slack: 1024 * 1024,
            max_decoded_bytes_per_stream: 256 * 1024 * 1024,
            max_decoded_bytes_per_document: 4 * 1024 * 1024 * 1024,
            max_retained_bytes_per_document: 256 * 1024 * 1024,
            max_image_pixels: 100_000_000,
            max_image_dimension: 1 << 20,
            max_image_components: 33,
            max_font_program_bytes: 64 * 1024 * 1024,
            max_font_glyphs: u16::MAX,
            max_outline_points: 1 << 17,
            max_outline_ops: 1 << 20,
            max_canvas_pixels: 1 << 24,
            max_path_segments: 1 << 20,
            max_clip_depth: 64,
            max_layer_depth: 32,
            max_raster_working_bytes: 1 << 30,
        }
    }
}

/// The limits one backend call must enforce. Backends check them **before** allocating:
/// output larger than `max_output_bytes`, or an image whose header exceeds a pixel, dimension
/// or component limit, fails with [`crate::BackendError::LimitExceeded`].
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
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Limits::default().decode_limits(u64::MAX)
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
        // A full-size canvas with 4 layers and 32 clips stays within the working bytes.
        let r = RasterLimits::default();
        assert!(r.max_canvas_pixels * (4 * (1 + 4) + 32) <= r.max_working_bytes);
        assert!(
            r.max_working_bytes <= 1 << 30,
            "half of the 2 GiB process budget"
        );
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
        // Every ceiling field distinct (1..=25), so clamping a field against a different
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
    fn simultaneous_worst_cases_fit_the_process_budget() {
        // One file held in memory, everything a document may retain, one stream being decoded
        // and one render's working memory, all at once, stay within the judge's 2 GiB
        // address-space cap (PLAN §2.2).
        let l = Limits::default();
        let file = u64::try_from(l.max_file_size).unwrap_or(u64::MAX);
        let stream = u64::try_from(l.max_decoded_bytes_per_stream).unwrap_or(u64::MAX);
        let total = file
            .saturating_add(l.max_retained_bytes_per_document)
            .saturating_add(stream)
            .saturating_add(l.max_raster_working_bytes);
        assert!(total <= 2 << 30, "{total} bytes");
        assert!(
            l.max_retained_bytes_per_document
                >= 2 * u64::try_from(l.max_font_program_bytes).unwrap_or(u64::MAX),
            "two of the largest font programs can stay cached"
        );
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
