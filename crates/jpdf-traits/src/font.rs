//! Font program parsing for embedded and system fonts (ISO 32000-2 §9.6–§9.9).
//!
//! A backend turns font program bytes into a [`FontFace`]: glyph count, font matrix,
//! advances, outlines, `cmap` lookups, glyph names, CID mapping and the built-in encoding.
//! Choosing glyphs for a PDF string (simple-font encodings, `Differences`, CMaps, the
//! §9.6.5.4 `cmap` selection rules) happens in `jpdf-fonts`, so swapping a backend can't change
//! which glyph a page shows.

use crate::{BackendError, FontLimits};

/// A glyph index into a face (TrueType glyph id, CFF/CFF2 glyph index, or the position of a
/// Type 1 glyph in its `CharStrings`, `.notdef` first). Always below
/// [`FontFace::glyph_count`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphId(pub u16);

/// The container a font program comes in (ISO 32000-2 §9.9, Table 124).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontProgram {
    /// An sfnt file: `FontFile2` (TrueType) or `FontFile3` with `/Subtype /OpenType`, with
    /// `glyf`, `CFF ` or `CFF2` outlines. Font collections are not accepted.
    Sfnt,
    /// A bare CFF font: `FontFile3` with `/Subtype /Type1C` or `/CIDFontType0C`.
    BareCff,
}

/// How a face's glyph outlines are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineFormat {
    /// `glyf` quadratic outlines.
    TrueType,
    /// CFF (Type 2 charstrings), bare or in an sfnt.
    Cff,
    /// CFF2 charstrings in an sfnt (default instance; no variation coordinates).
    Cff2,
    /// Type 1 charstrings (Type1Parser only).
    Type1,
}

/// A point in glyph space (font units, before the font matrix).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate (y up).
    pub y: f32,
}

/// One outline segment. Every contour starts with `MoveTo` and ends with `Close`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    /// Starts a contour.
    MoveTo(Point),
    /// A straight line to the point.
    LineTo(Point),
    /// A quadratic Bézier: control point, end point.
    QuadTo(Point, Point),
    /// A cubic Bézier: two control points, end point.
    CurveTo(Point, Point, Point),
    /// Closes the current contour.
    Close,
}

impl PathSegment {
    /// The points this segment carries: 1 for `MoveTo`/`LineTo`, 2 for `QuadTo`, 3 for
    /// `CurveTo`, 0 for `Close`. This is the unit of [`FontLimits::max_outline_points`].
    pub fn point_count(&self) -> u32 {
        match self {
            Self::MoveTo(_) | Self::LineTo(_) => 1,
            Self::QuadTo(..) => 2,
            Self::CurveTo(..) => 3,
            Self::Close => 0,
        }
    }

    fn points(&self) -> impl Iterator<Item = Point> {
        let (a, b, c) = match *self {
            Self::MoveTo(p) | Self::LineTo(p) => (Some(p), None, None),
            Self::QuadTo(c1, p) => (Some(c1), Some(p), None),
            Self::CurveTo(c1, c2, p) => (Some(c1), Some(c2), Some(p)),
            Self::Close => (None, None, None),
        };
        a.into_iter().chain(b).chain(c)
    }
}

/// A glyph outline in glyph space. TrueType implied on-curve points (between two off-curve
/// points) are explicit end points of `QuadTo` segments.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outline {
    /// The segments; empty for a glyph without contours (a space).
    pub segments: Vec<PathSegment>,
}

impl Outline {
    /// Total points over all segments (saturating).
    pub fn point_count(&self) -> u32 {
        self.segments
            .iter()
            .fold(0u32, |n, s| n.saturating_add(s.point_count()))
    }

    /// `[x_min, y_min, x_max, y_max]` over every point, control points included, or `None`
    /// for an empty outline.
    pub fn control_box(&self) -> Option<[f32; 4]> {
        let mut points = self.segments.iter().flat_map(PathSegment::points);
        let first = points.next()?;
        Some(points.fold([first.x, first.y, first.x, first.y], |b, p| {
            [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)]
        }))
    }

    /// Whether every contour starts with `MoveTo` and ends with `Close`, with no segment
    /// outside a contour.
    pub fn is_well_formed(&self) -> bool {
        let mut open = false;
        for segment in &self.segments {
            match segment {
                PathSegment::MoveTo(_) if !open => open = true,
                PathSegment::LineTo(_) | PathSegment::QuadTo(..) | PathSegment::CurveTo(..)
                    if open => {}
                PathSegment::Close if open => open = false,
                _ => return false,
            }
        }
        !open
    }

    /// Whether every coordinate is finite.
    pub fn is_finite(&self) -> bool {
        self.segments
            .iter()
            .flat_map(PathSegment::points)
            .all(|p| p.x.is_finite() && p.y.is_finite())
    }
}

/// An sfnt `cmap` subtable identifier (platform id, encoding id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CmapId {
    /// Platform id (0 Unicode, 1 Macintosh, 3 Windows).
    pub platform_id: u16,
    /// Platform-specific encoding id (for platform 3: 0 Symbol, 1 Unicode BMP, 10 full
    /// Unicode).
    pub encoding_id: u16,
}

/// The encoding built into a Type 1 or CFF font program (ISO 32000-2 §9.6.5). `jpdf-fonts`
/// resolves names to glyphs with [`FontFace::glyph_by_name`] and owns the Standard and Expert
/// tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinEncoding {
    /// The program has no built-in encoding (sfnt fonts, CID-keyed CFF).
    None,
    /// `StandardEncoding` (Type 1 `/Encoding StandardEncoding`, CFF encoding offset 0).
    Standard,
    /// `ExpertEncoding` (CFF encoding offset 1).
    Expert,
    /// An explicit table: `(code, glyph name)` pairs sorted by code, each code at most once.
    /// Codes not listed are unmapped (`.notdef`).
    Custom(Vec<(u8, String)>),
}

/// Face-wide metrics in glyph space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceMetrics {
    /// Font bounding box `[x_min, y_min, x_max, y_max]` (`head` or `FontBBox`).
    pub bbox: [f32; 4],
    /// Typographic ascender (`hhea`/`OS/2`), when the program has one.
    pub ascender: Option<f32>,
    /// Typographic descender (negative below the baseline), when the program has one.
    pub descender: Option<f32>,
}

/// A parsed font program. All values are in glyph space; [`FontFace::font_matrix`] maps them
/// to text space (ISO 32000-2 §9.2.4).
///
/// Every method is total: a damaged glyph gives an error for that glyph, never a panic, and
/// every number returned is finite. Every [`GlyphId`] returned is below
/// [`FontFace::glyph_count`]. Faces are cached per document and shared across rendering
/// threads, hence `Send + Sync`.
pub trait FontFace: Send + Sync {
    /// How the outlines are stored.
    fn outline_format(&self) -> OutlineFormat;

    /// Number of glyphs, `.notdef` included (≥ 1, ≤ [`FontLimits::max_glyphs`] at load).
    fn glyph_count(&self) -> u16;

    /// Glyph space to text space: `[1/upem, 0, 0, 1/upem, 0, 0]` for sfnt fonts, the
    /// `FontMatrix` for Type 1 and CFF (the Top DICT's). Finite and invertible. In a
    /// CID-keyed CFF whose FDArray dictionaries carry their own `FontMatrix`, the adapter
    /// transforms each glyph's outline and advance into this matrix's glyph space, so one
    /// matrix serves every glyph.
    fn font_matrix(&self) -> [f32; 6];

    /// Face-wide metrics.
    fn metrics(&self) -> FaceMetrics;

    /// The horizontal advance (`hmtx`, or the charstring width). A glyph id not below
    /// [`FontFace::glyph_count`] fails with [`BackendError::InvalidArgument`].
    fn advance(&self, glyph: GlyphId) -> Result<f32, BackendError>;

    /// The glyph outline. A glyph id out of range fails with
    /// [`BackendError::InvalidArgument`]; an outline with more than
    /// `limits.max_outline_points` points ([`Outline::point_count`]) fails with
    /// [`BackendError::LimitExceeded`] as soon as the count is passed, and so does one that
    /// needs more than `limits.max_outline_ops` operations (charstring operators executed
    /// plus composite components visited), which bounds CPU on programs that loop through
    /// subroutines or empty components without emitting points. Recursion in composites or
    /// subroutines is an error, never a stack overflow. The outline is well formed
    /// ([`Outline::is_well_formed`]).
    fn outline(&self, glyph: GlyphId, limits: &FontLimits) -> Result<Outline, BackendError>;

    /// The `cmap` subtables the backend can look up, in file order (empty for non-sfnt
    /// programs).
    fn cmap_subtables(&self) -> Vec<CmapId>;

    /// Looks `code` up in one `cmap` subtable; `None` when the subtable or the code is not
    /// mapped (glyph 0 counts as unmapped, so `Some(GlyphId(0))` is never returned).
    fn cmap_lookup(&self, cmap: CmapId, code: u32) -> Option<GlyphId>;

    /// The glyph's PostScript name (`post` table, CFF charset or Type 1 `CharStrings`), if
    /// the program names it, at most 127 bytes (the PostScript name limit; a longer name is
    /// damage and gives `None`). Not available for CID-keyed CFF.
    fn glyph_name(&self, glyph: GlyphId) -> Option<String>;

    /// The glyph with this PostScript name, if any.
    fn glyph_by_name(&self, name: &str) -> Option<GlyphId>;

    /// CID-keyed CFF only (bare `CIDFontType0C`, or a CID-keyed `CFF ` table in an OpenType
    /// program): the glyph whose charset entry is `cid`. `None` for every other program.
    fn cid_to_glyph(&self, cid: u16) -> Option<GlyphId>;

    /// The built-in encoding ([`BuiltinEncoding::None`] for sfnt programs).
    fn builtin_encoding(&self) -> BuiltinEncoding;
}

/// TrueType, CFF and CFF2 parsing (`FontFile2`, `FontFile3`; ISO 32000-2 §9.9).
pub trait FontEngine: Send + Sync {
    /// Parses a font program. Fails with [`BackendError::LimitExceeded`] when `data` is longer
    /// than `limits.max_font_program_bytes` (before parsing) or the glyph count exceeds
    /// `limits.max_glyphs`. The face may borrow `data`.
    fn load<'a>(
        &self,
        data: &'a [u8],
        program: FontProgram,
        limits: &FontLimits,
    ) -> Result<Box<dyn FontFace + 'a>, BackendError>;
}

/// Type 1 parsing (`FontFile`, ISO 32000-2 §9.9): the cleartext part, the `eexec`-encrypted
/// part and the trailer, found by content rather than the stream's `Length1`–`Length3`
/// (which real files often get wrong; CONSTITUTION 17).
pub trait Type1Parser: Send + Sync {
    /// Parses a Type 1 program, with the same limits as [`FontEngine::load`]. The face's
    /// [`FontFace::outline_format`] is [`OutlineFormat::Type1`].
    fn load<'a>(
        &self,
        data: &'a [u8],
        limits: &FontLimits,
    ) -> Result<Box<dyn FontFace + 'a>, BackendError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    #[test]
    fn outline_point_count_and_control_box() {
        let outline = Outline {
            segments: vec![
                PathSegment::MoveTo(p(0.0, 0.0)),
                PathSegment::LineTo(p(10.0, 0.0)),
                PathSegment::QuadTo(p(20.0, 5.0), p(10.0, 10.0)),
                PathSegment::CurveTo(p(5.0, 12.0), p(-3.0, 8.0), p(0.0, 0.0)),
                PathSegment::Close,
            ],
        };
        assert_eq!(outline.point_count(), 7);
        assert_eq!(outline.control_box(), Some([-3.0, 0.0, 20.0, 12.0]));
        assert!(outline.is_finite());
        assert_eq!(Outline::default().control_box(), None);
        assert_eq!(Outline::default().point_count(), 0);
    }

    #[test]
    fn outline_structure() {
        let m = PathSegment::MoveTo(p(0.0, 0.0));
        let l = PathSegment::LineTo(p(1.0, 0.0));
        let z = PathSegment::Close;
        let of = |segments: Vec<PathSegment>| Outline { segments }.is_well_formed();
        assert!(of(vec![]));
        assert!(of(vec![m, l, z, m, l, l, z]));
        assert!(!of(vec![l, z]), "segment before MoveTo");
        assert!(!of(vec![m, l]), "unclosed contour");
        assert!(!of(vec![m, m, z]), "MoveTo inside a contour");
        assert!(!of(vec![m, z, z]), "Close outside a contour");
    }

    #[test]
    fn non_finite_outline_is_detected() {
        let outline = Outline {
            segments: vec![PathSegment::MoveTo(p(f32::NAN, 0.0)), PathSegment::Close],
        };
        assert!(!outline.is_finite());
    }
}
