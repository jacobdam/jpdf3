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
    Outline, OutlineFormat, Type1Parser,
};

/// What one glyph of a fixture must look like.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedGlyph {
    /// The glyph.
    pub glyph: GlyphId,
    /// Its advance, in glyph space.
    pub advance: f32,
    /// Its [`Outline::point_count`]. TrueType implied on-curve points count, because the
    /// outline makes them explicit.
    pub points: u32,
    /// Its [`Outline::control_box`] (`None` for an empty glyph).
    pub control_box: Option<[f32; 4]>,
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
    /// Expected font matrix.
    pub font_matrix: [f32; 6],
    /// Glyphs to check (at least one with points across all fixtures).
    pub glyphs: Vec<ExpectedGlyph>,
    /// `cmap` lookups: subtable, code, expected glyph.
    pub cmap: Vec<(CmapId, u32, Option<GlyphId>)>,
    /// Named glyphs, checked both ways ([`FontFace::glyph_name`] and
    /// [`FontFace::glyph_by_name`]).
    pub names: Vec<(GlyphId, String)>,
    /// CID lookups: CID, expected glyph.
    pub cids: Vec<(u16, Option<GlyphId>)>,
    /// Expected built-in encoding.
    pub builtin_encoding: BuiltinEncoding,
    /// Largest allowed difference for advances, matrix entries and control boxes (backends
    /// may evaluate charstring arithmetic in different precisions).
    pub tolerance: f32,
}

/// Runs every check against a [`FontEngine`]. The fixtures must cover TrueType, CFF and CFF2
/// outlines, both [`FontProgram`] containers, a mapped `cmap` lookup, a mapped CID and a
/// built-in encoding other than [`BuiltinEncoding::None`].
pub fn run_font_engine(engine: &dyn FontEngine, fixtures: &[FontFixture]) -> Report {
    let mut report = Report::new("font");
    let formats = [
        OutlineFormat::TrueType,
        OutlineFormat::Cff,
        OutlineFormat::Cff2,
    ];
    let covered = formats
        .iter()
        .all(|f| fixtures.iter().any(|x| x.outline_format == *f))
        && [FontProgram::Sfnt, FontProgram::BareCff]
            .iter()
            .all(|p| fixtures.iter().any(|x| x.program == *p))
        && fixtures
            .iter()
            .any(|x| x.cmap.iter().any(|(_, _, g)| g.is_some()))
        && fixtures
            .iter()
            .any(|x| x.cids.iter().any(|(_, g)| g.is_some()))
        && fixtures
            .iter()
            .any(|x| x.builtin_encoding != BuiltinEncoding::None);
    report.check("font.fixture_coverage", covered, || {
        "fixtures must cover TrueType, CFF and CFF2 outlines, Sfnt and BareCff programs, a \
         mapped cmap lookup, a mapped CID (CID-keyed CFF) and a built-in encoding (CFF)"
            .into()
    });
    for f in fixtures {
        let load = |data: &[u8], limits: &FontLimits, q: &Questions| {
            guarded(|| exercise_load(engine.load(data, f.program, limits), q))
        };
        run_fixture(&mut report, f, &load);
    }
    let empty = guarded(|| {
        engine
            .load(&[], FontProgram::Sfnt, &FontLimits::default())
            .err()
    });
    common(&mut report, fixtures, empty);
    report
}

/// Runs every check against a [`Type1Parser`]. Every fixture must expect
/// [`OutlineFormat::Type1`], and at least one must have a [`BuiltinEncoding::Custom`]
/// encoding.
pub fn run_type1(parser: &dyn Type1Parser, fixtures: &[FontFixture]) -> Report {
    let mut report = Report::new("type1");
    let covered = fixtures
        .iter()
        .all(|f| f.outline_format == OutlineFormat::Type1)
        && fixtures
            .iter()
            .any(|f| matches!(f.builtin_encoding, BuiltinEncoding::Custom(_)));
    report.check("font.fixture_coverage", covered, || {
        "every Type 1 fixture must expect OutlineFormat::Type1, and one must have a custom \
         built-in encoding"
            .into()
    });
    for f in fixtures {
        let load = |data: &[u8], limits: &FontLimits, q: &Questions| {
            guarded(|| exercise_load(parser.load(data, limits), q))
        };
        run_fixture(&mut report, f, &load);
    }
    let empty = guarded(|| parser.load(&[], &FontLimits::default()).err());
    common(&mut report, fixtures, empty);
    report
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
    let mut names: Vec<String> = q.names.clone();
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
        names: glyphs
            .iter()
            .map(|&g| {
                let name = face.glyph_name(g);
                if let Some(n) = &name {
                    names.push(n.clone());
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
    let by_name = names
        .into_iter()
        .take(128)
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
    /// range, out-of-range ids rejected, glyph and point counts within `limits`.
    fn invariant_violation(&self, limits: &FontLimits) -> Option<String> {
        let count = self.glyph_count;
        if count == 0 || count > limits.max_glyphs {
            return Some(format!(
                "glyph_count {count} is 0 or above max_glyphs {}",
                limits.max_glyphs
            ));
        }
        let m = self.font_matrix;
        let det = m[0] * m[3] - m[1] * m[2];
        if !m.iter().all(|v| v.is_finite()) || det == 0.0 || !det.is_finite() {
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
                Ok(o) if o.point_count() > limits.max_outline_points || !o.is_finite() => {
                    return Some(format!(
                        "outline of {g:?} has {} points (limit {}) or non-finite coordinates",
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

/// Fixture-independent checks.
fn common(report: &mut Report, fixtures: &[FontFixture], empty: Option<Option<BackendError>>) {
    report.check("font.has_fixtures", !fixtures.is_empty(), || {
        "no fixtures supplied; the adapter must provide at least one".into()
    });
    report.check(
        "font.fixture_has_outline",
        fixtures
            .iter()
            .any(|f| f.glyphs.iter().any(|g| g.points > 0)),
        || "fixtures must expect at least one glyph with points (for the point limit)".into(),
    );
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

fn run_fixture(report: &mut Report, f: &FontFixture, load: &LoadFn<'_>) {
    let name = &f.name;
    let full = FontLimits::default();
    let questions = Questions {
        limits: full,
        glyphs: f.glyphs.iter().map(|g| g.glyph).collect(),
        cmap: f.cmap.iter().map(|(id, code, _)| (*id, *code)).collect(),
        names: f.names.iter().map(|(_, n)| n.clone()).collect(),
        cids: f.cids.iter().map(|(c, _)| *c).collect(),
    };
    let ask = load;

    let valid = f.glyph_count > 0 && f.glyphs.iter().all(|g| g.glyph.0 < f.glyph_count);
    report.check("font.fixture_valid", valid, || {
        format!("{name}: glyph_count is 0 or an expected glyph is out of range")
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
        Some(Ok(p)) => p.invariant_violation(&full),
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
    // own point count and fails one below it.
    let largest = f
        .glyphs
        .iter()
        .filter(|g| g.points > 0)
        .max_by_key(|g| g.points);
    if let Some(g) = largest {
        for (limit, want_ok) in [(g.points, true), (g.points.saturating_sub(1), false)] {
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
                matches!(&outline, Some(Ok(o)) if o.point_count() == g.points)
            } else {
                matches!(outline, Some(Err(BackendError::LimitExceeded(_))))
            };
            report.check("font.outline_points_limit", ok, || {
                format!(
                    "{name}: glyph {} ({} points) with max_outline_points {limit}: got {:?}",
                    g.glyph.0,
                    g.points,
                    outline
                        .as_ref()
                        .map(|r| r.as_ref().map(Outline::point_count))
                )
            });
        }
    }

    // Truncated and mutated programs: no panic, and every answer from a face that loads keeps
    // the invariants. Small limits keep runaway output visible and the run fast.
    let most_points = f.glyphs.iter().map(|g| g.points).max().unwrap_or(0);
    let capped = FontLimits {
        max_font_program_bytes: len,
        max_glyphs: f.glyph_count,
        max_outline_points: most_points.saturating_mul(2).max(64),
    };
    let capped_q = Questions {
        limits: capped,
        ..questions.clone()
    };
    let survives = |result: &Option<Result<Probe, BackendError>>| -> Option<String> {
        match result {
            None => Some("panicked".into()),
            Some(Err(_)) => None,
            Some(Ok(p)) => p.invariant_violation(&capped),
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
    if !p
        .font_matrix
        .iter()
        .zip(f.font_matrix)
        .all(|(a, b)| close(*a, b, t))
    {
        return Some(format!(
            "font matrix {:?}, expected {:?}",
            p.font_matrix, f.font_matrix
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
            Some(Ok(o)) => {
                let boxes_match = match (o.control_box(), g.control_box) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.iter().zip(b).all(|(x, y)| close(*x, y, t)),
                    _ => false,
                };
                if o.point_count() != g.points || !boxes_match {
                    return Some(format!(
                        "glyph {}: {} points with box {:?}, expected {} points with box {:?}",
                        g.glyph.0,
                        o.point_count(),
                        o.control_box(),
                        g.points,
                        g.control_box
                    ));
                }
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
    None
}

#[cfg(test)]
mod tests;
