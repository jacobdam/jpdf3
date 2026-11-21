//! Conformance suites for font program parsing (ISO 32000-2 §9.6–§9.9).
//!
//! jpdf-traits has no font writer, so the adapter supplies fixtures: fonts we generated
//! ourselves under a permissive license (never corpus files, CONSTITUTION 18) with the values
//! a reference tool reports. The suite adds format-independent checks on top: empty, truncated
//! and mutated programs, out-of-range glyph ids, every limit at its exact boundary, and
//! invariants on everything a damaged face returns (finite numbers, glyph ids in range).

use super::{Report, guarded, sample_offsets};
use crate::{
    BackendError, BuiltinEncoding, CmapId, FontEngine, FontFace, FontLimits, FontProgram, GlyphId,
    Outline, OutlineFormat, PathSegment, Point, Type1Parser,
};

/// What one glyph of a fixture must look like.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedGlyph {
    /// The glyph.
    pub glyph: GlyphId,
    /// Its advance, in glyph space.
    pub advance: f32,
    /// Its outline, compared segment by segment within the fixture's tolerance after
    /// dropping a `LineTo` back to the contour start right before `Close` (within the
    /// tolerance; backends differ on emitting it). TrueType implied on-curve points are
    /// explicit `QuadTo` end points.
    /// Empty for a glyph without contours.
    pub outline: Outline,
}

/// A program the backend must reject (self-referencing composites, recursive subroutines,
/// subroutine fan-out bombs). With a `control_glyph`, the program must load, the control
/// glyph must outline successfully (proving the hostile path is reachable), and each listed
/// glyph must fail with `LimitExceeded` or `Malformed`. Without one, the load or each listed
/// glyph must fail that way. A program that loads must report its declared `outline_format`.
///
/// Coverage (per container and outline pairing) counts only programs that prove their
/// format: the program loads as
/// its declared format and its control glyph outlines, so the listed glyphs sit on a path
/// the backend really reaches. A program that fails to load proves nothing (junk fails to
/// load too).
/// A backend that recurses without a bound overflows the stack, which aborts the test
/// process: that is a failure too, just not one the report can list.
#[derive(Debug, Clone, PartialEq)]
pub struct HostileFont {
    /// Name for failure messages.
    pub name: String,
    /// The font program.
    pub data: Vec<u8>,
    /// Its container ([`run_font_engine`] only).
    pub program: FontProgram,
    /// The outline format it attacks (coverage needs one hostile program per format).
    pub outline_format: OutlineFormat,
    /// Glyphs whose outline must fail if the program loads (at least one, all in range).
    pub glyphs: Vec<GlyphId>,
    /// A benign glyph in the same program that must outline successfully.
    pub control_glyph: Option<GlyphId>,
}

/// A font program and what it parses to.
#[derive(Debug, Clone, PartialEq)]
pub struct FontFixture {
    /// Name for failure messages.
    pub name: String,
    /// The font program.
    pub data: Vec<u8>,
    /// Its container ([`run_font_engine`] only; [`run_type1`] ignores it).
    pub program: FontProgram,
    /// Expected outline format.
    pub outline_format: OutlineFormat,
    /// Expected glyph count.
    pub glyph_count: u16,
    /// Expected font matrix, compared with a relative tolerance of 1e-4.
    pub font_matrix: [f32; 6],
    /// Expected metrics (within the tolerance; an expected `None` must be `None`).
    pub metrics: crate::FaceMetrics,
    /// Glyphs to check (at least one with points across all fixtures).
    pub glyphs: Vec<ExpectedGlyph>,
    /// `cmap` lookups: subtable, code, expected glyph.
    pub cmap: Vec<(CmapId, u32, Option<GlyphId>)>,
    /// Named glyphs, checked both ways ([`FontFace::glyph_name`] and
    /// [`FontFace::glyph_by_name`]).
    pub names: Vec<(GlyphId, String)>,
    /// CID lookups: CID, expected glyph. Every CID the suite asks about (0–63 and these)
    /// that is not listed must give `None`, so a fixture that is not CID-keyed lists none.
    pub cids: Vec<(u16, Option<GlyphId>)>,
    /// Expected built-in encoding.
    pub builtin_encoding: BuiltinEncoding,
    /// Glyphs with a known lower bound `n` (at least 1) on the operations their outline needs
    /// (charstring operators executed plus composite components visited, as
    /// [`FontFace::outline`] counts them): each outlines at the default limits and fails
    /// with `LimitExceeded` when `max_outline_ops` is `n - 1`. Coverage needs one glyph with
    /// a bound of at least [`HEAVY_OPS`] per container and outline pairing (a composite for
    /// TrueType). The bound may not exceed the default `max_outline_ops`.
    pub min_ops: Vec<(GlyphId, u32)>,
    /// Largest allowed difference, in font units, for advances, metrics and outline
    /// coordinates (backends may evaluate charstring arithmetic in different precisions).
    /// Finite, between 0 and 1.
    pub tolerance: f32,
}

/// The smallest `min_ops` bound that counts for coverage: high enough that a backend counting
/// one operation per glyph, or nothing for composite components, fails the check. (Whether
/// operators inside subroutines are counted depends on the fixture's glyph running them.)
pub const HEAVY_OPS: u32 = 100;

/// A container and outline format. The container is `None` for Type 1, which has one.
type Pairing = (Option<FontProgram>, OutlineFormat);

/// Every container and outline pairing a PDF can embed (ISO 32000-2 §9.9, Table 124).
const ENGINE_PAIRINGS: [(FontProgram, OutlineFormat); 4] = [
    (FontProgram::Sfnt, OutlineFormat::TrueType),
    (FontProgram::Sfnt, OutlineFormat::Cff),
    (FontProgram::Sfnt, OutlineFormat::Cff2),
    (FontProgram::BareCff, OutlineFormat::Cff),
];

/// `cmap` subtables asked about on every face, listed or not (an unlisted one must answer
/// `None`).
const STANDARD_CMAPS: [(u16, u16); 5] = [(3, 1), (3, 0), (1, 0), (0, 3), (3, 10)];

/// Runs every check against a [`FontEngine`]. The fixtures must cover TrueType, CFF and CFF2
/// outlines (each with a curve: `QuadTo` for TrueType, `CurveTo` for CFF and CFF2), every
/// container and outline pairing (sfnt with each format, bare CFF), a [`HEAVY_OPS`] `min_ops`
/// glyph per format, mapped `cmap` lookups in (3,1), (3,0) and (1,0), a mapped CID and a CFF
/// custom encoding. Hostile programs must prove every outline format (see [`HostileFont`]).
pub fn run_font_engine(
    engine: &dyn FontEngine,
    fixtures: &[FontFixture],
    hostile: &[HostileFont],
) -> Report {
    let mut report = Report::new("font");
    let mapped = |platform_id, encoding_id| {
        let id = CmapId {
            platform_id,
            encoding_id,
        };
        fixtures
            .iter()
            .any(|x| x.cmap.iter().any(|(c, _, g)| *c == id && g.is_some()))
    };
    let formats = [
        OutlineFormat::TrueType,
        OutlineFormat::Cff,
        OutlineFormat::Cff2,
    ];
    let pairings: Vec<Pairing> = ENGINE_PAIRINGS.iter().map(|&(p, o)| (Some(p), o)).collect();
    let covered = formats.iter().all(|f| has_curve(fixtures, *f))
        && pairings.iter().all(|pair| has_heavy(fixtures, *pair))
        && mapped(3, 1)
        && mapped(3, 0)
        && mapped(1, 0)
        && fixtures
            .iter()
            .any(|x| x.cids.iter().any(|(_, g)| g.is_some()))
        && fixtures.iter().any(|x| {
            x.outline_format == OutlineFormat::Cff
                && matches!(x.builtin_encoding, BuiltinEncoding::Custom(_))
        });
    report.check("font.fixture_coverage", covered, || {
        format!(
            "fixtures must cover TrueType, CFF and CFF2 outlines with curves, a min_ops glyph \
             of at least {HEAVY_OPS} in each pairing {ENGINE_PAIRINGS:?}, mapped cmap \
             lookups in (3,1), (3,0) and (1,0), a mapped CID and a CFF custom encoding"
        )
    });
    for f in fixtures {
        let load = |data: &[u8], limits: &FontLimits, q: &Questions| {
            guarded(|| exercise_load(engine.load(data, f.program, limits), q))
        };
        run_fixture(&mut report, f, f.program == FontProgram::Sfnt, &load);
    }
    let mut proven = Vec::new();
    for h in hostile {
        let load = |data: &[u8], limits: &FontLimits, q: &Questions| {
            guarded(|| exercise_load(engine.load(data, h.program, limits), q))
        };
        let proved = run_hostile(&mut report, h, h.program == FontProgram::Sfnt, &load);
        proven.extend(proved.map(|o| (Some(h.program), o)));
    }
    for program in [FontProgram::Sfnt, FontProgram::BareCff] {
        let empty = guarded(|| engine.load(&[], program, &FontLimits::default()).err());
        empty_input(&mut report, empty);
    }
    common(&mut report, fixtures, &proven, &pairings);
    report
}

/// Runs every check against a [`Type1Parser`]. Every fixture must expect
/// [`OutlineFormat::Type1`], one must have a `CurveTo`, one a [`BuiltinEncoding::Custom`]
/// encoding and one a [`HEAVY_OPS`] `min_ops` glyph. A hostile program must prove the
/// format (see [`HostileFont`]).
pub fn run_type1(
    parser: &dyn Type1Parser,
    fixtures: &[FontFixture],
    hostile: &[HostileFont],
) -> Report {
    let mut report = Report::new("type1");
    let covered = fixtures
        .iter()
        .all(|f| f.outline_format == OutlineFormat::Type1)
        && has_curve(fixtures, OutlineFormat::Type1)
        && has_heavy(fixtures, (None, OutlineFormat::Type1))
        && fixtures
            .iter()
            .any(|f| matches!(f.builtin_encoding, BuiltinEncoding::Custom(_)));
    report.check("font.fixture_coverage", covered, || {
        format!(
            "every Type 1 fixture must expect OutlineFormat::Type1, one must have a CurveTo, \
             one a custom built-in encoding and one a min_ops glyph of at least {HEAVY_OPS}"
        )
    });
    let load = |data: &[u8], limits: &FontLimits, q: &Questions| {
        guarded(|| exercise_load(parser.load(data, limits), q))
    };
    for f in fixtures {
        run_fixture(&mut report, f, false, &load);
    }
    let mut proven = Vec::new();
    for h in hostile {
        let proved = run_hostile(&mut report, h, false, &load);
        proven.extend(proved.map(|o| (None, o)));
    }
    let empty = guarded(|| parser.load(&[], &FontLimits::default()).err());
    empty_input(&mut report, empty);
    common(
        &mut report,
        fixtures,
        &proven,
        &[(None, OutlineFormat::Type1)],
    );
    report
}

/// Whether a fixture of `format` expects a glyph with the format's curve segment.
fn has_curve(fixtures: &[FontFixture], format: OutlineFormat) -> bool {
    fixtures
        .iter()
        .filter(|f| f.outline_format == format)
        .flat_map(|f| &f.glyphs)
        .flat_map(|g| &g.outline.segments)
        .any(|s| match format {
            OutlineFormat::TrueType => matches!(s, PathSegment::QuadTo(..)),
            _ => matches!(s, PathSegment::CurveTo(..)),
        })
}

/// Whether a fixture of this pairing has a `min_ops` glyph with a bound of at least
/// [`HEAVY_OPS`] (the container is not compared for Type 1).
fn has_heavy(fixtures: &[FontFixture], (program, format): Pairing) -> bool {
    fixtures
        .iter()
        .filter(|f| f.outline_format == format && program.is_none_or(|p| p == f.program))
        .any(|f| f.min_ops.iter().any(|(_, n)| *n >= HEAVY_OPS))
}

/// Drops a `LineTo` back to the contour's start (within `tolerance`) right before `Close`
/// (backends differ on whether they emit it; the shape is the same).
fn normalize(outline: &Outline, tolerance: f32) -> Vec<PathSegment> {
    let mut out: Vec<PathSegment> = Vec::with_capacity(outline.segments.len());
    let mut start: Option<Point> = None;
    for segment in &outline.segments {
        match segment {
            PathSegment::MoveTo(p) => start = Some(*p),
            PathSegment::Close => {
                if let (Some(PathSegment::LineTo(p)), Some(s)) = (out.last(), start)
                    && close(p.x, s.x, tolerance)
                    && close(p.y, s.y, tolerance)
                {
                    out.pop();
                }
            }
            _ => {}
        }
        out.push(*segment);
    }
    out
}

/// Whether two outlines match segment by segment within `tolerance`.
fn outlines_match(got: &Outline, want: &Outline, tolerance: f32) -> bool {
    let near = |a: &Point, b: &Point| close(a.x, b.x, tolerance) && close(a.y, b.y, tolerance);
    let (got, want) = (normalize(got, tolerance), normalize(want, tolerance));
    got.len() == want.len()
        && got.iter().zip(&want).all(|pair| match pair {
            (PathSegment::MoveTo(a), PathSegment::MoveTo(b))
            | (PathSegment::LineTo(a), PathSegment::LineTo(b)) => near(a, b),
            (PathSegment::QuadTo(a1, a), PathSegment::QuadTo(b1, b)) => near(a1, b1) && near(a, b),
            (PathSegment::CurveTo(a1, a2, a), PathSegment::CurveTo(b1, b2, b)) => {
                near(a1, b1) && near(a2, b2) && near(a, b)
            }
            (PathSegment::Close, PathSegment::Close) => true,
            _ => false,
        })
}

/// Everything the suite reads from a face, collected inside one guarded call so a face that
/// borrows the input never escapes it.
#[derive(Debug)]
struct Probe {
    outline_format: OutlineFormat,
    glyph_count: u16,
    font_matrix: [f32; 6],
    bbox: [f32; 4],
    ascender: Option<f32>,
    descender: Option<f32>,
    builtin_encoding: BuiltinEncoding,
    cmap_subtables: Vec<CmapId>,
    queries: Queries,
}

/// Per-glyph answers, computed eagerly for the glyphs a check asks about.
#[derive(Debug, Default)]
struct Queries {
    advances: Vec<(GlyphId, Result<f32, BackendError>)>,
    outlines: Vec<(GlyphId, Result<Outline, BackendError>)>,
    cmap: Vec<(CmapId, u32, Option<GlyphId>)>,
    names: Vec<(GlyphId, Option<String>)>,
    by_name: Vec<(String, Option<GlyphId>)>,
    cids: Vec<(u16, Option<GlyphId>)>,
}

/// Loads a program and answers the questions while the face is alive (it may borrow the
/// input, so it never leaves the call). `None` means the backend panicked.
type LoadFn<'a> =
    dyn Fn(&[u8], &FontLimits, &Questions) -> Option<Result<Probe, BackendError>> + 'a;

/// What to ask a loaded face besides the standard questions.
#[derive(Debug, Clone, Default)]
struct Questions {
    /// Limits for `outline` calls.
    limits: FontLimits,
    glyphs: Vec<GlyphId>,
    cmap: Vec<(CmapId, u32)>,
    /// Glyphs whose names are asked (besides the standard and listed glyphs).
    named: Vec<GlyphId>,
    /// Names looked up with `glyph_by_name`, all of them (besides up to 128 names the face
    /// returned).
    names: Vec<String>,
    cids: Vec<u16>,
}

/// Glyph ids asked of every face with `count` glyphs: the first 64, the last one, and the
/// first two out-of-range ids.
fn standard_glyphs(count: u16) -> Vec<GlyphId> {
    let mut ids: Vec<u16> = (0..count.min(64)).collect();
    ids.extend([count.saturating_sub(1), count, count.saturating_add(1)]);
    ids.into_iter().map(GlyphId).collect()
}

/// Asks a freshly loaded face every question while it is alive.
fn exercise_load(
    loaded: Result<Box<dyn FontFace + '_>, BackendError>,
    q: &Questions,
) -> Result<Probe, BackendError> {
    let face = loaded?;
    let count = face.glyph_count();
    let metrics = face.metrics();
    let cmap_subtables = face.cmap_subtables();
    let mut glyphs = standard_glyphs(count);
    glyphs.extend(q.glyphs.iter().copied());
    glyphs.sort_unstable();
    glyphs.dedup();
    let mut cmap: Vec<(CmapId, u32)> = q.cmap.clone();
    // Every subtable (up to 16) for codes 0–255 and a few beyond the BMP.
    for id in cmap_subtables.iter().take(16) {
        cmap.extend(
            (0..=255u32)
                .chain([0xffff, 0x1_0000, 0x10_ffff])
                .map(|c| (*id, c)),
        );
    }
    // Standard subtables the face does not list, which must answer `None`.
    for (platform_id, encoding_id) in STANDARD_CMAPS {
        let id = CmapId {
            platform_id,
            encoding_id,
        };
        if !cmap_subtables.contains(&id) {
            cmap.extend((0..=255u32).map(|c| (id, c)));
        }
    }
    let mut named = glyphs.clone();
    named.extend(q.named.iter().copied());
    named.sort_unstable();
    named.dedup();
    let mut found: Vec<String> = Vec::new();
    let queries = Queries {
        advances: glyphs.iter().map(|&g| (g, face.advance(g))).collect(),
        outlines: glyphs
            .iter()
            .map(|&g| (g, face.outline(g, &q.limits)))
            .collect(),
        cmap: cmap
            .iter()
            .map(|&(id, code)| (id, code, face.cmap_lookup(id, code)))
            .collect(),
        names: named
            .iter()
            .map(|&g| {
                let name = face.glyph_name(g);
                if let Some(n) = &name
                    && found.len() < 128
                {
                    found.push(n.clone());
                }
                (g, name)
            })
            .collect(),
        by_name: Vec::new(),
        cids: q
            .cids
            .iter()
            .copied()
            .chain(0..64)
            .map(|c| (c, face.cid_to_glyph(c)))
            .collect(),
    };
    let by_name = q
        .names
        .iter()
        .cloned()
        .chain(found)
        .map(|n| {
            let g = face.glyph_by_name(&n);
            (n, g)
        })
        .collect();
    Ok(Probe {
        outline_format: face.outline_format(),
        glyph_count: count,
        font_matrix: face.font_matrix(),
        bbox: metrics.bbox,
        ascender: metrics.ascender,
        descender: metrics.descender,
        builtin_encoding: face.builtin_encoding(),
        cmap_subtables,
        queries: Queries { by_name, ..queries },
    })
}

impl Probe {
    fn advance(&self, g: GlyphId) -> Option<&Result<f32, BackendError>> {
        self.queries
            .advances
            .iter()
            .find(|(id, _)| *id == g)
            .map(|(_, r)| r)
    }

    fn outline(&self, g: GlyphId) -> Option<&Result<Outline, BackendError>> {
        self.queries
            .outlines
            .iter()
            .find(|(id, _)| *id == g)
            .map(|(_, r)| r)
    }

    /// The first broken invariant, if any: numbers finite, matrix invertible, glyph ids in
    /// range, out-of-range ids rejected, glyph and point counts within `limits`, and the
    /// promised empty answers (`sfnt` is false for bare CFF and Type 1 programs).
    fn invariant_violation(&self, limits: &FontLimits, sfnt: bool) -> Option<String> {
        let count = self.glyph_count;
        if count == 0 || count > limits.max_glyphs {
            return Some(format!(
                "glyph_count {count} is 0 or above max_glyphs {}",
                limits.max_glyphs
            ));
        }
        let m = self.font_matrix;
        let det = m[0] * m[3] - m[1] * m[2];
        if !m.iter().all(|v| v.is_finite()) || !det.is_normal() {
            return Some(format!("font matrix {m:?} is not finite and invertible"));
        }
        let finite_opt = |v: Option<f32>| v.is_none_or(f32::is_finite);
        if !self.bbox.iter().all(|v| v.is_finite())
            || !finite_opt(self.ascender)
            || !finite_opt(self.descender)
        {
            return Some("metrics are not finite".into());
        }
        let in_range = |g: &GlyphId| g.0 < count;
        for (g, r) in &self.queries.advances {
            match r {
                Ok(a) if !in_range(g) => return Some(format!("advance of {g:?} gave {a}")),
                Ok(a) if !a.is_finite() => return Some(format!("advance of {g:?} is {a}")),
                Err(e) if !in_range(g) && !matches!(e, BackendError::InvalidArgument(_)) => {
                    return Some(format!(
                        "advance of {g:?}: expected InvalidArgument, got {e:?}"
                    ));
                }
                _ => {}
            }
        }
        for (g, r) in &self.queries.outlines {
            match r {
                Ok(_) if !in_range(g) => return Some(format!("outline of {g:?} is Ok")),
                Ok(o)
                    if o.point_count() > limits.max_outline_points
                        || !o.is_finite()
                        || !o.is_well_formed() =>
                {
                    return Some(format!(
                        "outline of {g:?} has {} points (limit {}), non-finite coordinates \
                         or a contour not opened by MoveTo and closed by Close",
                        o.point_count(),
                        limits.max_outline_points
                    ));
                }
                Err(e) if !in_range(g) && !matches!(e, BackendError::InvalidArgument(_)) => {
                    return Some(format!(
                        "outline of {g:?}: expected InvalidArgument, got {e:?}"
                    ));
                }
                _ => {}
            }
        }
        if self
            .queries
            .cmap
            .iter()
            .any(|(_, _, g)| *g == Some(GlyphId(0)))
        {
            return Some("cmap_lookup returned glyph 0 (unmapped)".into());
        }
        if !sfnt && !self.cmap_subtables.is_empty() {
            return Some(format!(
                "a program that is not sfnt reports cmap subtables {:?}",
                self.cmap_subtables
            ));
        }
        if let Some((id, code, g)) = self
            .queries
            .cmap
            .iter()
            .find(|(id, _, g)| g.is_some() && !self.cmap_subtables.contains(id))
        {
            return Some(format!(
                "cmap {id:?} is not listed by cmap_subtables but maps {code:#x} to {g:?}"
            ));
        }
        if self.outline_format != OutlineFormat::Cff
            && let Some((cid, g)) = self.queries.cids.iter().find(|(_, g)| g.is_some())
        {
            return Some(format!(
                "a {:?} face maps CID {cid} to {g:?} (only CID-keyed CFF has CIDs)",
                self.outline_format
            ));
        }
        if let Some((g, _)) = self
            .queries
            .names
            .iter()
            .find(|(_, n)| n.as_ref().is_some_and(|n| n.len() > 127))
        {
            return Some(format!("the name of {g:?} is longer than 127 bytes"));
        }
        let returned = self
            .queries
            .cmap
            .iter()
            .filter_map(|(_, _, g)| *g)
            .chain(self.queries.by_name.iter().filter_map(|(_, g)| *g))
            .chain(self.queries.cids.iter().filter_map(|(_, g)| *g));
        for g in returned {
            if !in_range(&g) {
                return Some(format!("a lookup returned {g:?} of {count} glyphs"));
            }
        }
        if let Some((g, _)) = self
            .queries
            .names
            .iter()
            .find(|(g, n)| !in_range(g) && n.is_some())
        {
            return Some(format!("out-of-range {g:?} has a name"));
        }
        if let BuiltinEncoding::Custom(entries) = &self.builtin_encoding
            && entries
                .iter()
                .zip(entries.iter().skip(1))
                .any(|(a, b)| a.0 >= b.0)
        {
            return Some("custom encoding is not sorted by code with unique codes".into());
        }
        None
    }
}

fn close(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn describe(result: &Option<Result<Probe, BackendError>>) -> String {
    match result {
        None => "panicked".into(),
        Some(Ok(p)) => format!("Ok({:?}, {} glyphs)", p.outline_format, p.glyph_count),
        Some(Err(e)) => format!("Err({e:?})"),
    }
}

fn empty_input(report: &mut Report, empty: Option<Option<BackendError>>) {
    report.check("font.empty_input", matches!(empty, Some(Some(_))), || {
        format!(
            "expected an error, got {}",
            match empty {
                None => "a panic",
                Some(None) => "a face",
                Some(Some(_)) => "an error",
            }
        )
    });
}

/// Fixture-independent checks.
/// `proven` holds the pairing of every hostile program that proved its format; `required`
/// is every pairing that needs one.
fn common(report: &mut Report, fixtures: &[FontFixture], proven: &[Pairing], required: &[Pairing]) {
    report.check("font.has_fixtures", !fixtures.is_empty(), || {
        "no fixtures supplied; the adapter must provide at least one".into()
    });
    report.check(
        "font.fixture_has_outline",
        fixtures
            .iter()
            .any(|f| f.glyphs.iter().any(|g| g.outline.point_count() > 0)),
        || "fixtures must expect at least one glyph with points (for the point limit)".into(),
    );
    let covered = required.iter().all(|pair| proven.contains(pair));
    report.check("font.has_hostile_fixtures", covered, || {
        format!(
            "every pairing in {required:?} needs a hostile program in that container that \
             loads as that format with a control glyph that outlines; proven: {proven:?}"
        )
    });
}

/// A hostile program never panics and is rejected the way [`HostileFont`] describes. Returns
/// the declared format when the program proved it (see [`HostileFont`]).
fn run_hostile(
    report: &mut Report,
    h: &HostileFont,
    sfnt: bool,
    load: &LoadFn<'_>,
) -> Option<OutlineFormat> {
    let full = FontLimits::default();
    let mut glyphs = h.glyphs.clone();
    glyphs.extend(h.control_glyph);
    let q = Questions {
        limits: full,
        glyphs,
        ..Questions::default()
    };
    let rejected = |e: &BackendError| {
        matches!(
            e,
            BackendError::LimitExceeded(_) | BackendError::Malformed(_)
        )
    };
    let result = load(&h.data, &full, &q);
    let problem = match &result {
        _ if h.glyphs.is_empty() => Some("no glyphs listed".to_owned()),
        None => Some("panicked".to_owned()),
        Some(Err(e)) if h.control_glyph.is_some() => {
            Some(format!("has a control glyph but failed to load: {e:?}"))
        }
        Some(Err(e)) if !rejected(e) => Some(format!("load failed with {e:?}")),
        Some(Err(_)) => None,
        Some(Ok(p)) => {
            let out_of_range = h
                .glyphs
                .iter()
                .chain(&h.control_glyph)
                .find(|g| g.0 >= p.glyph_count);
            let control_failed = h
                .control_glyph
                .filter(|g| !matches!(p.outline(*g), Some(Ok(_))));
            if p.outline_format != h.outline_format {
                Some(format!(
                    "declared {:?} but loaded as {:?}",
                    h.outline_format, p.outline_format
                ))
            } else if let Some(g) = out_of_range {
                Some(format!(
                    "glyph {} is not below glyph_count {}",
                    g.0, p.glyph_count
                ))
            } else if let Some(g) = control_failed {
                Some(format!("control glyph {} did not outline", g.0))
            } else {
                h.glyphs
                    .iter()
                    .find(|g| !matches!(p.outline(**g), Some(Err(e)) if rejected(e)))
                    .map(|g| {
                        format!(
                            "glyph {}: expected LimitExceeded or Malformed, got {:?}",
                            g.0,
                            p.outline(*g)
                        )
                    })
                    .or_else(|| p.invariant_violation(&full, sfnt))
            }
        }
    };
    report.check("font.hostile", problem.is_none(), || {
        format!("{}: {}", h.name, problem.clone().unwrap_or_default())
    });
    // Proof is about the program, not the backend's verdict on it: it loaded as the declared
    // format and its control glyph outlined, so the listed glyphs sit on a reachable path.
    let proved = !h.glyphs.is_empty()
        && matches!(&result, Some(Ok(p)) if p.outline_format == h.outline_format
            && h.control_glyph.is_some_and(|g| matches!(p.outline(g), Some(Ok(_)))));
    proved.then_some(h.outline_format)
}

/// `sfnt` is false for bare CFF and Type 1 programs (which promise no `cmap` subtables).
fn run_fixture(report: &mut Report, f: &FontFixture, sfnt: bool, load: &LoadFn<'_>) {
    let name = &f.name;
    let full = FontLimits::default();
    let questions = Questions {
        limits: full,
        glyphs: f.glyphs.iter().map(|g| g.glyph).collect(),
        cmap: f.cmap.iter().map(|(id, code, _)| (*id, *code)).collect(),
        named: f.names.iter().map(|(g, _)| *g).collect(),
        names: f.names.iter().map(|(_, n)| n.clone()).collect(),
        cids: f.cids.iter().map(|(c, _)| *c).collect(),
    };
    let ask = load;

    let valid = f.glyph_count > 0
        && f.glyphs
            .iter()
            .all(|g| g.glyph.0 < f.glyph_count && g.outline.is_well_formed())
        && f.min_ops
            .iter()
            .all(|(g, n)| g.0 < f.glyph_count && (1..=full.max_outline_ops).contains(n))
        && f.tolerance.is_finite()
        && (0.0..=1.0).contains(&f.tolerance);
    report.check("font.fixture_valid", valid, || {
        format!(
            "{name}: glyph_count is 0, an expected or min_ops glyph is out of range, an \
             expected outline is malformed, a min_ops bound is 0 or above the default \
             max_outline_ops, or the tolerance is not \
             between 0 and 1"
        )
    });
    if !valid {
        return;
    }

    // The fixture parses to exactly what it says.
    let result = ask(&f.data, &full, &questions);
    let mismatch = match &result {
        Some(Ok(p)) => fixture_mismatch(p, f),
        other => Some(describe(other)),
    };
    report.check("font.fixture_loads", mismatch.is_none(), || {
        format!("{name}: {}", mismatch.clone().unwrap_or_default())
    });
    let violation = match &result {
        Some(Ok(p)) => p.invariant_violation(&full, sfnt),
        _ => None,
    };
    report.check("font.invariants", violation.is_none(), || {
        format!("{name}: {}", violation.clone().unwrap_or_default())
    });

    // Out-of-range glyph ids are InvalidArgument for advance and outline.
    let bad = GlyphId(f.glyph_count);
    let rejected = matches!(&result, Some(Ok(p))
        if matches!(p.advance(bad), Some(Err(BackendError::InvalidArgument(_))))
        && matches!(p.outline(bad), Some(Err(BackendError::InvalidArgument(_)))));
    report.check("font.invalid_glyph", rejected, || {
        format!(
            "{name}: glyph {} of {}: expected InvalidArgument from advance and outline",
            bad.0, f.glyph_count
        )
    });

    // Exact limits: at the fixture's size the load succeeds, one below it fails.
    let len = f.data.len();
    let loads = [
        (
            "font.program_bytes_limit",
            FontLimits {
                max_font_program_bytes: len,
                ..full
            },
            FontLimits {
                max_font_program_bytes: len.saturating_sub(1),
                ..full
            },
        ),
        (
            "font.glyph_limit",
            FontLimits {
                max_glyphs: f.glyph_count,
                ..full
            },
            FontLimits {
                max_glyphs: f.glyph_count.saturating_sub(1),
                ..full
            },
        ),
    ];
    let quiet = Questions {
        limits: full,
        ..Questions::default()
    };
    for (check, at, below) in loads {
        let result = ask(&f.data, &at, &quiet);
        report.check(check, matches!(result, Some(Ok(_))), || {
            format!("{name}: limit equal to the fixture: {}", describe(&result))
        });
        let result = ask(&f.data, &below, &quiet);
        report.check(
            check,
            matches!(result, Some(Err(BackendError::LimitExceeded(_)))),
            || {
                format!(
                    "{name}: limit one below the fixture: expected LimitExceeded, got {}",
                    describe(&result)
                )
            },
        );
    }
    // The point limit applies per outline call: the fixture's largest glyph passes at its
    // own point count and fails one below it. The count is the backend's own (it may or may
    // not emit the closing `LineTo` that `normalize` drops).
    let largest = f
        .glyphs
        .iter()
        .filter(|g| g.outline.point_count() > 0)
        .max_by_key(|g| g.outline.point_count())
        .and_then(|g| match &result {
            Some(Ok(p)) => match p.outline(g.glyph) {
                Some(Ok(o)) if o.point_count() > 0 => Some((g, o.point_count())),
                _ => None,
            },
            _ => None,
        });
    if let Some((g, points)) = largest {
        for (limit, want_ok) in [(points, true), (points.saturating_sub(1), false)] {
            let q = Questions {
                limits: FontLimits {
                    max_outline_points: limit,
                    ..full
                },
                glyphs: vec![g.glyph],
                ..Questions::default()
            };
            let result = ask(&f.data, &full, &q);
            let outline = match &result {
                Some(Ok(p)) => p.outline(g.glyph).cloned(),
                _ => None,
            };
            let ok = if want_ok {
                matches!(&outline, Some(Ok(o)) if o.point_count() == points)
            } else {
                matches!(outline, Some(Err(BackendError::LimitExceeded(_))))
            };
            report.check("font.outline_points_limit", ok, || {
                format!(
                    "{name}: glyph {} ({} points) with max_outline_points {limit}: got {:?}",
                    g.glyph.0,
                    points,
                    outline
                        .as_ref()
                        .map(|r| r.as_ref().map(Outline::point_count))
                )
            });
        }
    }

    // The ops limit comes from the caller: every Type 1/CFF/CFF2 glyph with points runs at
    // least one charstring operator, so with `max_outline_ops` 0 it must fail. (TrueType
    // simple glyphs count no operations.)
    if let Some((g, _)) = largest.filter(|_| f.outline_format != OutlineFormat::TrueType) {
        let q = Questions {
            limits: FontLimits {
                max_outline_ops: 0,
                ..full
            },
            glyphs: vec![g.glyph],
            ..Questions::default()
        };
        let result = ask(&f.data, &full, &q);
        let outline = match &result {
            Some(Ok(p)) => p.outline(g.glyph).cloned(),
            _ => None,
        };
        report.check(
            "font.outline_ops_limit",
            matches!(outline, Some(Err(BackendError::LimitExceeded(_)))),
            || {
                format!(
                    "{name}: glyph {} with max_outline_ops 0: expected LimitExceeded, got {:?}",
                    g.glyph.0,
                    outline
                        .as_ref()
                        .map(|r| r.as_ref().map(Outline::point_count))
                )
            },
        );
    }

    // Known lower bounds on operations: each glyph passes at the default limits and fails one
    // below its bound, so counting too little (one per glyph, nothing for composite
    // components) is caught where the check at 0 above cannot see it.
    for &(g, bound) in &f.min_ops {
        for (limit, want_ok) in [
            (full.max_outline_ops, true),
            (bound.saturating_sub(1), false),
        ] {
            let q = Questions {
                limits: FontLimits {
                    max_outline_ops: limit,
                    ..full
                },
                glyphs: vec![g],
                ..Questions::default()
            };
            let result = ask(&f.data, &full, &q);
            let outline = match &result {
                Some(Ok(p)) => p.outline(g).cloned(),
                _ => None,
            };
            let ok = if want_ok {
                matches!(outline, Some(Ok(_)))
            } else {
                matches!(outline, Some(Err(BackendError::LimitExceeded(_))))
            };
            report.check("font.outline_ops_limit", ok, || {
                format!(
                    "{name}: glyph {} (needs at least {bound} operations) with max_outline_ops \
                     {limit}: got {:?}",
                    g.0,
                    outline
                        .as_ref()
                        .map(|r| r.as_ref().map(Outline::point_count))
                )
            });
        }
    }

    // Truncated and mutated programs: no panic, and every answer from a face that loads keeps
    // the invariants. Small limits keep runaway output visible and the run fast.
    let most_points = f
        .glyphs
        .iter()
        .map(|g| g.outline.point_count())
        .max()
        .unwrap_or(0);
    let capped = FontLimits {
        max_font_program_bytes: len,
        max_glyphs: f.glyph_count,
        max_outline_points: most_points.saturating_mul(2).max(64),
        ..full
    };
    let capped_q = Questions {
        limits: capped,
        ..questions.clone()
    };
    let survives = |result: &Option<Result<Probe, BackendError>>| -> Option<String> {
        match result {
            None => Some("panicked".into()),
            Some(Err(_)) => None,
            Some(Ok(p)) => p.invariant_violation(&capped, sfnt),
        }
    };
    for cut in sample_offsets(len, 128) {
        let prefix = f.data.get(..cut).unwrap_or(&[]);
        let result = ask(prefix, &capped, &capped_q);
        let problem = survives(&result);
        report.check("font.truncated", problem.is_none(), || {
            format!(
                "{name}: cut at {cut}/{len}: {}",
                problem.clone().unwrap_or_default()
            )
        });
    }
    for at in sample_offsets(len, 128) {
        let mut mutated = f.data.clone();
        if let Some(byte) = mutated.get_mut(at) {
            *byte ^= 0xff;
        }
        let result = ask(&mutated, &capped, &capped_q);
        let problem = survives(&result);
        report.check("font.mutated", problem.is_none(), || {
            format!(
                "{name}: byte {at} flipped: {}",
                problem.clone().unwrap_or_default()
            )
        });
    }
}

/// How the probe differs from the fixture, or `None` when it matches.
fn fixture_mismatch(p: &Probe, f: &FontFixture) -> Option<String> {
    let t = f.tolerance;
    if p.outline_format != f.outline_format {
        return Some(format!(
            "outline format {:?}, expected {:?}",
            p.outline_format, f.outline_format
        ));
    }
    if p.glyph_count != f.glyph_count {
        return Some(format!(
            "glyph count {}, expected {}",
            p.glyph_count, f.glyph_count
        ));
    }
    // Relative to the matrix's largest entry, so a near-zero skew still compares sanely.
    let scale = f.font_matrix.iter().fold(0f32, |m, v| m.max(v.abs()));
    let rel = |a: f32, b: f32| (a - b).abs() <= 1e-4 * scale;
    if !p
        .font_matrix
        .iter()
        .zip(f.font_matrix)
        .all(|(a, b)| rel(*a, b))
    {
        return Some(format!(
            "font matrix {:?}, expected {:?}",
            p.font_matrix, f.font_matrix
        ));
    }
    let opt = |a: Option<f32>, b: Option<f32>| match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => close(a, b, t),
        _ => false,
    };
    let want = &f.metrics;
    if !p.bbox.iter().zip(want.bbox).all(|(a, b)| close(*a, b, t))
        || !opt(p.ascender, want.ascender)
        || !opt(p.descender, want.descender)
    {
        return Some(format!(
            "metrics bbox {:?} ascender {:?} descender {:?}, expected {want:?}",
            p.bbox, p.ascender, p.descender
        ));
    }
    if p.builtin_encoding != f.builtin_encoding {
        return Some(format!(
            "built-in encoding {:?}, expected {:?}",
            p.builtin_encoding, f.builtin_encoding
        ));
    }
    for g in &f.glyphs {
        match p.advance(g.glyph) {
            Some(Ok(a)) if close(*a, g.advance, t) => {}
            other => {
                return Some(format!(
                    "glyph {}: advance {other:?}, expected {}",
                    g.glyph.0, g.advance
                ));
            }
        }
        match p.outline(g.glyph) {
            Some(Ok(o)) if outlines_match(o, &g.outline, t) => {}
            Some(Ok(o)) => {
                return Some(format!(
                    "glyph {}: outline {:?}, expected {:?}",
                    g.glyph.0, o.segments, g.outline.segments
                ));
            }
            other => return Some(format!("glyph {}: outline {other:?}", g.glyph.0)),
        }
    }
    for (id, code, want) in &f.cmap {
        let got = p
            .queries
            .cmap
            .iter()
            .find(|(i, c, _)| i == id && c == code)
            .and_then(|(_, _, g)| *g);
        if got != *want {
            return Some(format!(
                "cmap {id:?} code {code:#x}: {got:?}, expected {want:?}"
            ));
        }
    }
    for (glyph, name) in &f.names {
        let got = p
            .queries
            .names
            .iter()
            .find(|(g, _)| g == glyph)
            .and_then(|(_, n)| n.as_deref());
        let back = p
            .queries
            .by_name
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, g)| *g);
        if got != Some(name.as_str()) || back != Some(*glyph) {
            return Some(format!(
                "glyph {} named {got:?} and {name:?} maps to {back:?}, expected {name:?} both ways",
                glyph.0
            ));
        }
    }
    for (cid, want) in &f.cids {
        let got = p
            .queries
            .cids
            .iter()
            .find(|(c, _)| c == cid)
            .and_then(|(_, g)| *g);
        if got != *want {
            return Some(format!("CID {cid}: {got:?}, expected {want:?}"));
        }
    }
    // A CID the fixture does not list is unmapped (and every CID is, for a program that is
    // not CID-keyed).
    if let Some((cid, g)) = p
        .queries
        .cids
        .iter()
        .find(|(c, g)| g.is_some() && !f.cids.iter().any(|(listed, _)| listed == c))
    {
        return Some(format!(
            "CID {cid}: {g:?}, but the fixture does not list it (expected None)"
        ));
    }
    None
}

#[cfg(test)]
mod tests;
