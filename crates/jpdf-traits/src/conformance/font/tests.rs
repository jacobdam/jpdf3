use super::*;
use crate::{FaceMetrics, PathSegment, Point};

/// A toy font program standing in for every format: lines of text.
///
/// ```text
/// toyfont <tt|cff|cff2|type1> <units per em>
/// g <advance> <name or -> [segments]  one glyph: M x y, L x y, Q x y x y, C x y x y x y,
///                                       Z; or `loop` (an endless subroutine loop); or
///                                       `ref <gid>…` (a composite: each component visited
///                                       costs one operation)
/// c <platform> <encoding> <code> <gid> a cmap entry
/// i <cid> <gid>                         a CID entry
/// e <standard|expert>                   built-in encoding
/// e <code> <name>                       a custom encoding entry
/// ```
///
/// The suite is format-independent, so this exercises every check.
#[derive(Default, Clone, Copy)]
struct Toy {
    ignore_byte_limit: bool,
    ignore_glyph_limit: bool,
    ignore_point_limit: bool,
    panic_on_empty: bool,
    /// Advance and outline of an out-of-range glyph succeed.
    accept_bad_glyph: bool,
    /// Unmapped cmap codes return the first out-of-range glyph.
    cmap_off_by_one: bool,
    panic_on_bad_utf8: bool,
    nan_matrix: bool,
    /// Report `StandardEncoding` whatever the program says.
    ignore_encoding: bool,
    /// Give up on `loop` glyphs with an empty outline instead of an ops-limit error.
    ignore_ops_limit: bool,
    /// Drop every `Close`.
    no_close: bool,
    /// Unmapped cmap codes return glyph 0.
    cmap_zero: bool,
    /// Read the units per em as 2048 whatever the program says.
    fixed_upem: bool,
    /// Use a built-in ops budget instead of the caller's (still stops `loop` glyphs).
    ignore_caller_ops: bool,
    /// Count one operation per outline call, whatever the glyph runs.
    flat_ops: bool,
    /// Composite components cost no operations.
    free_components: bool,
    /// `cid_to_glyph` maps an unlisted CID to the glyph with the same number (in range).
    identity_cids: bool,
    /// `cmap_lookup` on a subtable the face doesn't have maps codes by identity (in range).
    identity_cmap: bool,
    /// `cmap_subtables` always reports (3,1), even for bare CFF and Type 1.
    fake_cmap_subtables: bool,
}

struct ToyGlyph {
    advance: f32,
    name: Option<String>,
    /// `None` for a `loop` glyph.
    outline: Option<Outline>,
    /// Components of a composite glyph (its own `outline` is empty).
    refs: Vec<u16>,
}

/// Parses `M x y`, `L x y`, `Q …`, `C …`, `Z` tokens into a well-formed outline.
fn segments<'a>(mut tokens: impl Iterator<Item = &'a str>) -> Result<Outline, BackendError> {
    let point = |t: &mut dyn Iterator<Item = &'a str>| -> Result<Point, BackendError> {
        Ok(Point {
            x: finite(t.next())?,
            y: finite(t.next())?,
        })
    };
    let mut segments = Vec::new();
    while let Some(op) = tokens.next() {
        segments.push(match op {
            "M" => PathSegment::MoveTo(point(&mut tokens)?),
            "L" => PathSegment::LineTo(point(&mut tokens)?),
            "Q" => PathSegment::QuadTo(point(&mut tokens)?, point(&mut tokens)?),
            "C" => PathSegment::CurveTo(
                point(&mut tokens)?,
                point(&mut tokens)?,
                point(&mut tokens)?,
            ),
            "Z" => PathSegment::Close,
            _ => return Err(BackendError::Malformed("segment")),
        });
    }
    let outline = Outline { segments };
    if outline.is_well_formed() {
        Ok(outline)
    } else {
        Err(BackendError::Malformed("contour"))
    }
}

struct ToyFace {
    toy: Toy,
    format: OutlineFormat,
    upem: f32,
    glyphs: Vec<ToyGlyph>,
    cmap: Vec<(CmapId, u32, u16)>,
    cids: Vec<(u16, u16)>,
    encoding: BuiltinEncoding,
}

fn num<T: std::str::FromStr>(field: Option<&str>) -> Result<T, BackendError> {
    field
        .and_then(|f| f.parse().ok())
        .ok_or(BackendError::Malformed("number"))
}

fn finite(field: Option<&str>) -> Result<f32, BackendError> {
    let v: f32 = num(field)?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(BackendError::Malformed("non-finite"))
    }
}

impl Toy {
    fn parse(&self, data: &[u8], limits: &FontLimits) -> Result<ToyFace, BackendError> {
        if self.panic_on_empty && data.is_empty() {
            panic!("empty font");
        }
        if !self.ignore_byte_limit && data.len() > limits.max_font_program_bytes {
            return Err(BackendError::LimitExceeded("font program bytes"));
        }
        let text = match std::str::from_utf8(data) {
            Ok(t) => t,
            Err(_) if self.panic_on_bad_utf8 => panic!("bad utf-8"),
            Err(_) => return Err(BackendError::Malformed("utf-8")),
        };
        let mut lines = text.lines();
        let mut header = lines
            .next()
            .ok_or(BackendError::Truncated)?
            .split_whitespace();
        if header.next() != Some("toyfont") {
            return Err(BackendError::Malformed("magic"));
        }
        let format = match header.next() {
            Some("tt") => OutlineFormat::TrueType,
            Some("cff") => OutlineFormat::Cff,
            Some("cff2") => OutlineFormat::Cff2,
            Some("type1") => OutlineFormat::Type1,
            _ => return Err(BackendError::Malformed("format")),
        };
        let upem: u16 = num(header.next())?;
        if upem == 0 {
            return Err(BackendError::Malformed("upem"));
        }
        let mut face = ToyFace {
            toy: *self,
            format,
            upem: if self.fixed_upem {
                2048.0
            } else {
                f32::from(upem)
            },
            glyphs: Vec::new(),
            cmap: Vec::new(),
            cids: Vec::new(),
            encoding: BuiltinEncoding::None,
        };
        let mut custom = Vec::new();
        for line in lines {
            let mut f = line.split_whitespace();
            match f.next() {
                Some("g") => {
                    let advance = finite(f.next())?;
                    let name = match f.next() {
                        Some("-") => None,
                        Some(n) => Some(n.to_owned()),
                        None => return Err(BackendError::Malformed("glyph")),
                    };
                    let mut rest = f.peekable();
                    let mut refs = Vec::new();
                    let outline = match rest.peek() {
                        Some(&"loop") => None,
                        Some(&"ref") => {
                            rest.next();
                            for r in rest {
                                refs.push(num(Some(r))?);
                            }
                            Some(Outline::default())
                        }
                        _ => Some(segments(rest)?),
                    };
                    face.glyphs.push(ToyGlyph {
                        advance,
                        name,
                        outline,
                        refs,
                    });
                    if !self.ignore_glyph_limit
                        && face.glyphs.len() > usize::from(limits.max_glyphs)
                    {
                        return Err(BackendError::LimitExceeded("glyphs"));
                    }
                }
                Some("c") => face.cmap.push((
                    CmapId {
                        platform_id: num(f.next())?,
                        encoding_id: num(f.next())?,
                    },
                    num(f.next())?,
                    num(f.next())?,
                )),
                Some("i") => face.cids.push((num(f.next())?, num(f.next())?)),
                Some("e") => match (f.next(), f.next()) {
                    (Some("standard"), None) => face.encoding = BuiltinEncoding::Standard,
                    (Some("expert"), None) => face.encoding = BuiltinEncoding::Expert,
                    (code, Some(name)) => custom.push((num(code)?, name.to_owned())),
                    _ => return Err(BackendError::Malformed("encoding")),
                },
                _ => return Err(BackendError::Malformed("line")),
            }
        }
        let count =
            u16::try_from(face.glyphs.len()).map_err(|_| BackendError::Malformed("count"))?;
        if count == 0 {
            return Err(BackendError::Truncated);
        }
        let in_range = face.cmap.iter().all(|e| e.2 < count)
            && face.cids.iter().all(|e| e.1 < count)
            && face.glyphs.iter().flat_map(|g| &g.refs).all(|r| *r < count);
        if !in_range {
            return Err(BackendError::Malformed("glyph id"));
        }
        if !custom.is_empty() {
            custom.sort_unstable();
            custom.dedup_by_key(|e: &mut (u8, String)| e.0);
            face.encoding = BuiltinEncoding::Custom(custom);
        }
        Ok(face)
    }
}

impl ToyFace {
    fn glyph(&self, glyph: GlyphId) -> Result<&ToyGlyph, BackendError> {
        self.glyphs
            .get(usize::from(glyph.0))
            .ok_or(BackendError::InvalidArgument("glyph id"))
    }

    fn count(&self) -> u16 {
        u16::try_from(self.glyphs.len()).unwrap_or(u16::MAX)
    }
}

impl FontFace for ToyFace {
    fn outline_format(&self) -> OutlineFormat {
        self.format
    }

    fn glyph_count(&self) -> u16 {
        self.count()
    }

    fn font_matrix(&self) -> [f32; 6] {
        let s = if self.toy.nan_matrix {
            f32::NAN
        } else {
            1.0 / self.upem
        };
        [s, 0.0, 0.0, s, 0.0, 0.0]
    }

    fn metrics(&self) -> FaceMetrics {
        FaceMetrics {
            bbox: [0.0, 0.0, self.upem, self.upem],
            ascender: None,
            descender: None,
        }
    }

    fn advance(&self, glyph: GlyphId) -> Result<f32, BackendError> {
        match self.glyph(glyph) {
            Err(_) if self.toy.accept_bad_glyph => Ok(0.0),
            g => Ok(g?.advance),
        }
    }

    fn outline(&self, glyph: GlyphId, limits: &FontLimits) -> Result<Outline, BackendError> {
        if self.glyph(glyph).is_err() && self.toy.accept_bad_glyph {
            return Ok(Outline::default());
        }
        let ops_limit = if self.toy.ignore_caller_ops {
            1 << 20
        } else {
            limits.max_outline_ops
        };
        let mut ops: u32 = 0;
        let mut spend = |n: usize| -> Result<(), BackendError> {
            let n = u32::try_from(n).map_err(|_| BackendError::LimitExceeded("outline ops"))?;
            ops = ops.saturating_add(n);
            if ops > ops_limit {
                return Err(BackendError::LimitExceeded("outline ops"));
            }
            Ok(())
        };
        if self.toy.flat_ops {
            spend(1)?;
        }
        let mut out = Outline::default();
        let mut points: u32 = 0;
        // Components are expanded with an explicit stack, so a self-referencing composite
        // runs into the ops limit instead of overflowing the stack.
        let mut pending = vec![glyph.0];
        while let Some(id) = pending.pop() {
            let g = self.glyph(GlyphId(id))?;
            let Some(outline) = &g.outline else {
                // A `loop` glyph never ends: a real interpreter runs until the ops budget is
                // spent.
                return if self.toy.ignore_ops_limit {
                    Ok(Outline::default())
                } else {
                    Err(BackendError::LimitExceeded("outline ops"))
                };
            };
            if !self.toy.flat_ops {
                if !self.toy.free_components {
                    spend(g.refs.len())?;
                }
                // One charstring operator per segment; TrueType simple glyphs run none.
                if self.format != OutlineFormat::TrueType {
                    spend(outline.segments.len())?;
                }
            }
            pending.extend(g.refs.iter().rev());
            out.segments.extend(outline.segments.iter().copied());
            points = points.saturating_add(outline.point_count());
            if points > limits.max_outline_points && !self.toy.ignore_point_limit {
                return Err(BackendError::LimitExceeded("outline points"));
            }
        }
        if self.toy.no_close {
            out.segments.retain(|s| *s != PathSegment::Close);
        }
        Ok(out)
    }

    fn cmap_subtables(&self) -> Vec<CmapId> {
        let mut ids: Vec<CmapId> = Vec::new();
        for (id, _, _) in &self.cmap {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        if self.toy.fake_cmap_subtables && !ids.contains(&cmap(3, 1)) {
            ids.push(cmap(3, 1));
        }
        ids
    }

    fn cmap_lookup(&self, cmap: CmapId, code: u32) -> Option<GlyphId> {
        if self.toy.identity_cmap && !self.cmap.iter().any(|e| e.0 == cmap) {
            return u16::try_from(code)
                .ok()
                .filter(|c| *c > 0 && *c < self.count())
                .map(GlyphId);
        }
        let found = self
            .cmap
            .iter()
            .find(|(id, c, _)| *id == cmap && *c == code)
            .map(|e| GlyphId(e.2))
            .filter(|g| g.0 != 0);
        if found.is_none() && self.toy.cmap_off_by_one {
            return Some(GlyphId(self.count()));
        }
        if found.is_none() && self.toy.cmap_zero {
            return Some(GlyphId(0));
        }
        found
    }

    fn glyph_name(&self, glyph: GlyphId) -> Option<String> {
        self.glyph(glyph).ok()?.name.clone()
    }

    fn glyph_by_name(&self, name: &str) -> Option<GlyphId> {
        let index = self
            .glyphs
            .iter()
            .position(|g| g.name.as_deref() == Some(name))?;
        u16::try_from(index).ok().map(GlyphId)
    }

    fn cid_to_glyph(&self, cid: u16) -> Option<GlyphId> {
        let listed = self.cids.iter().find(|e| e.0 == cid).map(|e| GlyphId(e.1));
        if listed.is_none() && self.toy.identity_cids && cid < self.count() {
            return Some(GlyphId(cid));
        }
        listed
    }

    fn builtin_encoding(&self) -> BuiltinEncoding {
        if self.toy.ignore_encoding {
            BuiltinEncoding::Standard
        } else {
            self.encoding.clone()
        }
    }
}

impl FontEngine for Toy {
    fn load<'a>(
        &self,
        data: &'a [u8],
        program: FontProgram,
        limits: &FontLimits,
    ) -> Result<Box<dyn FontFace + 'a>, BackendError> {
        let face = self.parse(data, limits)?;
        match (program, face.format) {
            (FontProgram::BareCff, OutlineFormat::Cff)
            | (
                FontProgram::Sfnt,
                OutlineFormat::TrueType | OutlineFormat::Cff | OutlineFormat::Cff2,
            ) => {}
            _ => return Err(BackendError::Malformed("container")),
        }
        Ok(Box::new(face))
    }
}

impl Type1Parser for Toy {
    fn load<'a>(
        &self,
        data: &'a [u8],
        limits: &FontLimits,
    ) -> Result<Box<dyn FontFace + 'a>, BackendError> {
        let face = self.parse(data, limits)?;
        if face.format != OutlineFormat::Type1 {
            return Err(BackendError::Malformed("not Type 1"));
        }
        Ok(Box::new(face))
    }
}

/// Segments for the toy syntax: `M x y`, `L x y`, `Q …`, `C …`, `Z`.
fn outline(text: &str) -> Outline {
    segments(text.split_whitespace()).unwrap_or_default()
}

/// A fixture with `.notdef` (a box), `space` (empty), `A` (a curve of the format's kind; the
/// largest glyph) and `heavy` (a composite of [`HEAVY_OPS`] empty `space` components: a
/// benign glyph that needs exactly that many operations in TrueType, more in the others).
fn fixture(
    name: &str,
    format: &str,
    program: FontProgram,
    outline_format: OutlineFormat,
    extra: &str,
) -> FontFixture {
    let notdef = "M 0 0 L 400 0 L 400 700 L 0 700 Z";
    let a = if outline_format == OutlineFormat::TrueType {
        "M 0 0 Q 300 700 600 0 L 450 -20 L 150 -20 L 0 0 Z"
    } else {
        "M 0 0 C 100 700 500 700 600 0 L 150 -20 Z"
    };
    let heavy = " 1".repeat(100);
    let data = format!(
        "toyfont {format} 1000\n\
         g 500 .notdef {notdef}\n\
         g 250 space\n\
         g 600 A {a}\n\
         g 0 heavy ref{heavy}\n\
         {extra}"
    );
    FontFixture {
        name: name.into(),
        data: data.into_bytes(),
        program,
        outline_format,
        glyph_count: 4,
        font_matrix: [0.001, 0.0, 0.0, 0.001, 0.0, 0.0],
        metrics: FaceMetrics {
            bbox: [0.0, 0.0, 1000.0, 1000.0],
            ascender: None,
            descender: None,
        },
        glyphs: vec![
            ExpectedGlyph {
                glyph: GlyphId(0),
                advance: 500.0,
                outline: outline(notdef),
            },
            ExpectedGlyph {
                glyph: GlyphId(1),
                advance: 250.0,
                outline: Outline::default(),
            },
            ExpectedGlyph {
                glyph: GlyphId(2),
                advance: 600.0,
                // The trailing `L 0 0` before `Z` is normalized away.
                outline: outline(&a.replace(" L 0 0 Z", " Z")),
            },
        ],
        cmap: Vec::new(),
        names: vec![(GlyphId(1), "space".into()), (GlyphId(2), "A".into())],
        cids: Vec::new(),
        builtin_encoding: BuiltinEncoding::None,
        min_ops: vec![(GlyphId(3), HEAVY_OPS)],
        tolerance: 0.01,
    }
}

fn cmap(platform_id: u16, encoding_id: u16) -> CmapId {
    CmapId {
        platform_id,
        encoding_id,
    }
}

fn engine_fixtures() -> Vec<FontFixture> {
    let mut tt = fixture(
        "tt",
        "tt",
        FontProgram::Sfnt,
        OutlineFormat::TrueType,
        "c 3 1 32 1\nc 3 1 65 2\nc 3 0 61505 2\nc 1 0 65 2\n",
    );
    tt.cmap = vec![
        (cmap(3, 1), 0x20, Some(GlyphId(1))),
        (cmap(3, 1), 0x41, Some(GlyphId(2))),
        (cmap(3, 1), 0x42, None),
        (cmap(3, 0), 0xf041, Some(GlyphId(2))),
        (cmap(1, 0), 0x41, Some(GlyphId(2))),
    ];
    let mut cff = fixture(
        "cff",
        "cff",
        FontProgram::BareCff,
        OutlineFormat::Cff,
        "e 65 A\ne 32 space\n",
    );
    cff.builtin_encoding = BuiltinEncoding::Custom(vec![(32, "space".into()), (65, "A".into())]);
    let mut cid = fixture(
        "cid",
        "cff",
        FontProgram::BareCff,
        OutlineFormat::Cff,
        "i 0 0\ni 7 2\n",
    );
    cid.cids = vec![(0, Some(GlyphId(0))), (7, Some(GlyphId(2))), (8, None)];
    let cff2 = fixture("cff2", "cff2", FontProgram::Sfnt, OutlineFormat::Cff2, "");
    let otf = fixture("cff-otf", "cff", FontProgram::Sfnt, OutlineFormat::Cff, "");
    vec![tt, cff, cid, cff2, otf]
}

/// One hostile toy program per format: glyph 1 loops forever, glyph 0 is the control.
fn hostile_for(formats: &[(&str, FontProgram, OutlineFormat)]) -> Vec<HostileFont> {
    formats
        .iter()
        .map(|&(format, program, outline_format)| HostileFont {
            name: format!("{format}-loop"),
            data: format!(
                "toyfont {format} 1000\ng 500 .notdef M 0 0 L 1 0 L 1 1 Z\ng 500 bomb loop\n"
            )
            .into_bytes(),
            program,
            outline_format,
            glyphs: vec![GlyphId(1)],
            control_glyph: Some(GlyphId(0)),
        })
        .collect()
}

fn engine_hostile() -> Vec<HostileFont> {
    hostile_for(&[
        ("tt", FontProgram::Sfnt, OutlineFormat::TrueType),
        ("cff", FontProgram::BareCff, OutlineFormat::Cff),
        ("cff2", FontProgram::Sfnt, OutlineFormat::Cff2),
    ])
}

fn type1_hostile() -> Vec<HostileFont> {
    hostile_for(&[("type1", FontProgram::Sfnt, OutlineFormat::Type1)])
}

fn type1_fixtures() -> Vec<FontFixture> {
    let mut custom = fixture(
        "type1-custom",
        "type1",
        FontProgram::Sfnt,
        OutlineFormat::Type1,
        "e 65 A\ne 32 space\n",
    );
    custom.builtin_encoding = BuiltinEncoding::Custom(vec![(32, "space".into()), (65, "A".into())]);
    let mut standard = fixture(
        "type1-standard",
        "type1",
        FontProgram::Sfnt,
        OutlineFormat::Type1,
        "e standard\n",
    );
    standard.builtin_encoding = BuiltinEncoding::Standard;
    vec![custom, standard]
}

fn engine_failures(toy: Toy, fixtures: &[FontFixture]) -> Vec<&'static str> {
    let report = run_font_engine(&toy, fixtures, &engine_hostile());
    assert!(report.checks_run > 0);
    report.failed_checks()
}

#[test]
fn correct_engine_passes() {
    let report = run_font_engine(&Toy::default(), &engine_fixtures(), &engine_hostile());
    assert!(report.passed(), "{report}");
}

#[test]
fn correct_type1_parser_passes() {
    let report = run_type1(&Toy::default(), &type1_fixtures(), &type1_hostile());
    assert!(report.passed(), "{report}");
}

#[test]
fn ignoring_the_byte_limit_is_caught() {
    let toy = Toy {
        ignore_byte_limit: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.program_bytes_limit"]
    );
}

#[test]
fn ignoring_the_glyph_limit_is_caught() {
    let toy = Toy {
        ignore_glyph_limit: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.glyph_limit"]
    );
}

#[test]
fn ignoring_the_point_limit_is_caught() {
    let toy = Toy {
        ignore_point_limit: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.outline_points_limit"]
    );
}

#[test]
fn panicking_on_empty_input_is_caught() {
    let toy = Toy {
        panic_on_empty: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.empty_input", "font.truncated"]
    );
}

#[test]
fn panicking_on_mutated_input_is_caught() {
    let toy = Toy {
        panic_on_bad_utf8: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.mutated"]
    );
}

#[test]
fn accepting_out_of_range_glyphs_is_caught() {
    let toy = Toy {
        accept_bad_glyph: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec![
            "font.hostile",
            "font.invalid_glyph",
            "font.invariants",
            "font.truncated"
        ]
    );
}

#[test]
fn out_of_range_cmap_results_are_caught() {
    let toy = Toy {
        cmap_off_by_one: true,
        ..Toy::default()
    };
    // Hostile faces are asked the standard subtables too, so they catch it as well.
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec![
            "font.fixture_loads",
            "font.hostile",
            "font.invariants",
            "font.truncated"
        ]
    );
}

#[test]
fn a_non_finite_font_matrix_is_caught() {
    let toy = Toy {
        nan_matrix: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec![
            "font.fixture_loads",
            "font.hostile",
            "font.invariants",
            "font.truncated"
        ]
    );
}

#[test]
fn a_wrong_expected_value_fails_only_the_fixture_check() {
    let mut fixtures = engine_fixtures();
    fixtures[0].glyphs[2].advance = 601.0;
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads"]
    );
    // Same point count and control box, but the contour runs the other way.
    let mut fixtures = engine_fixtures();
    fixtures[0].glyphs[0].outline = outline("M 0 0 L 0 700 L 400 700 L 400 0 Z");
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads"]
    );
    let mut fixtures = engine_fixtures();
    fixtures[0].glyphs[0].outline = outline("M 0 0 L 400 0 L 400 700 L 0 700.5 Z");
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads"]
    );
    let mut fixtures = engine_fixtures();
    fixtures[0].metrics.ascender = Some(800.0);
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads"]
    );
    let mut fixtures = engine_fixtures();
    fixtures[0].names[1].1 = "B".into();
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads"]
    );
    let mut fixtures = engine_fixtures();
    fixtures[2].cids[1].1 = Some(GlyphId(1));
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads"]
    );
}

#[test]
fn an_invalid_fixture_is_reported() {
    let broken: [fn(&mut FontFixture); 4] = [
        |f| f.glyphs[0].glyph = GlyphId(f.glyph_count),
        |f| f.tolerance = 1.5,
        |f| f.tolerance = f32::NAN,
        |f| f.glyphs[0].outline.segments.pop().map_or((), |_| ()),
    ];
    for (i, breakage) in broken.iter().enumerate() {
        let mut fixtures = engine_fixtures();
        breakage(&mut fixtures[0]);
        assert_eq!(
            engine_failures(Toy::default(), &fixtures),
            vec!["font.fixture_valid"],
            "breakage {i}"
        );
    }
}

#[test]
fn a_wrong_units_per_em_is_caught() {
    let toy = Toy {
        fixed_upem: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.fixture_loads"]
    );
}

#[test]
fn ignoring_the_ops_limit_is_caught() {
    let toy = Toy {
        ignore_ops_limit: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.hostile"]
    );
}

#[test]
fn unclosed_contours_are_caught() {
    let toy = Toy {
        no_close: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec![
            "font.fixture_loads",
            "font.hostile",
            "font.invariants",
            "font.truncated"
        ]
    );
}

#[test]
fn cmap_glyph_zero_is_caught() {
    let toy = Toy {
        cmap_zero: true,
        ..Toy::default()
    };
    // Hostile faces are asked the standard subtables too, so they catch it as well.
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec![
            "font.fixture_loads",
            "font.hostile",
            "font.invariants",
            "font.truncated"
        ]
    );
}

#[test]
fn ignoring_the_callers_ops_limit_is_caught() {
    let toy = Toy {
        ignore_caller_ops: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.outline_ops_limit"]
    );
}

#[test]
fn weak_hostile_fixtures_fail() {
    const BOTH: &[&str] = &["font.has_hostile_fixtures", "font.hostile"];
    type Case = (fn(&mut HostileFont), &'static [&'static str]);
    let cases: [Case; 5] = [
        // An out-of-range glyph fails with InvalidArgument for every backend. The program
        // still loads and its control glyph outlines, so it proves the format.
        (|h| h.glyphs = vec![GlyphId(9999)], &["font.hostile"]),
        // A program that fails to load for a dull reason proves nothing.
        (|h| h.data = b"not a font".to_vec(), BOTH),
        // The control glyph must outline.
        (|h| h.control_glyph = Some(GlyphId(1)), BOTH),
        (|h| h.glyphs.clear(), BOTH),
        // A program that loads as another format than it declares.
        (|h| h.outline_format = OutlineFormat::Cff2, BOTH),
    ];
    for (i, (breakage, want)) in cases.iter().enumerate() {
        let mut hostile = engine_hostile();
        breakage(&mut hostile[0]);
        let report = run_font_engine(&Toy::default(), &engine_fixtures(), &hostile);
        assert_eq!(report.failed_checks(), *want, "case {i}");
    }
    // Without a control glyph, a load failure must still be LimitExceeded or Malformed. Such
    // a program passes font.hostile (alongside the proving ones) but proves nothing.
    let mut hostile = engine_hostile();
    let mut extra = hostile[0].clone();
    extra.control_glyph = None;
    extra.data = b"toyfont tt 1000\ng 500 .notdef M 0 0 L 1 0 L 1 1 Z\nzzz\n".to_vec();
    hostile.push(extra);
    assert!(run_font_engine(&Toy::default(), &engine_fixtures(), &hostile).passed());
    let toy = Toy {
        panic_on_bad_utf8: true,
        ..Toy::default()
    };
    if let Some(last) = hostile.last_mut() {
        last.data = vec![0xff];
    }
    let report = run_font_engine(&toy, &engine_fixtures(), &hostile);
    assert!(report.failed("font.hostile"));
}

#[test]
fn junk_without_control_glyphs_does_not_cover_the_formats() {
    // Audit 0001: junk labelled with each format, without control glyphs, passed the suite.
    let hostile: Vec<HostileFont> = engine_hostile()
        .into_iter()
        .map(|h| HostileFont {
            data: b"not a font".to_vec(),
            control_glyph: None,
            ..h
        })
        .collect();
    let report = run_font_engine(&Toy::default(), &engine_fixtures(), &hostile);
    assert_eq!(report.failed_checks(), vec!["font.has_hostile_fixtures"]);
    let hostile: Vec<HostileFont> = type1_hostile()
        .into_iter()
        .map(|h| HostileFont {
            data: b"not a font".to_vec(),
            control_glyph: None,
            ..h
        })
        .collect();
    let report = run_type1(&Toy::default(), &type1_fixtures(), &hostile);
    assert_eq!(report.failed_checks(), vec!["font.has_hostile_fixtures"]);
}

#[test]
fn a_mislabelled_hostile_program_does_not_cover_its_format() {
    // The CFF slot holds a TrueType program declared as CFF.
    let mut hostile = engine_hostile();
    hostile[1].data = hostile[0].data.clone();
    hostile[1].program = FontProgram::Sfnt;
    let report = run_font_engine(&Toy::default(), &engine_fixtures(), &hostile);
    assert_eq!(
        report.failed_checks(),
        vec!["font.has_hostile_fixtures", "font.hostile"]
    );
}

#[test]
fn a_self_referencing_composite_is_hostile() {
    let mut hostile = engine_hostile();
    hostile[0].data =
        b"toyfont tt 1000\ng 500 .notdef M 0 0 L 1 0 L 1 1 Z\ng 0 bomb ref 0 1\n".to_vec();
    assert!(run_font_engine(&Toy::default(), &engine_fixtures(), &hostile).passed());
    let toy = Toy {
        free_components: true,
        ..Toy::default()
    };
    // Components that cost nothing never reach the ops limit; the point limit stops the
    // growing outline instead, which still rejects it, so only the min_ops checks see it.
    let report = run_font_engine(&toy, &engine_fixtures(), &hostile);
    assert_eq!(report.failed_checks(), vec!["font.outline_ops_limit"]);
}

#[test]
fn counting_too_few_operations_is_caught() {
    for toy in [
        Toy {
            flat_ops: true,
            ..Toy::default()
        },
        Toy {
            free_components: true,
            ..Toy::default()
        },
    ] {
        assert_eq!(
            engine_failures(toy, &engine_fixtures()),
            vec!["font.outline_ops_limit"]
        );
        let report = run_type1(&toy, &type1_fixtures(), &type1_hostile());
        assert_eq!(report.failed_checks(), vec!["font.outline_ops_limit"]);
    }
}

#[test]
fn heavy_min_ops_glyphs_are_required_per_format() {
    for i in 0..engine_fixtures().len() {
        let mut fixtures = engine_fixtures();
        let format = fixtures[i].outline_format;
        for f in fixtures.iter_mut().filter(|f| f.outline_format == format) {
            f.min_ops.clear();
        }
        assert_eq!(
            engine_failures(Toy::default(), &fixtures),
            vec!["font.fixture_coverage"],
            "without min_ops for {format:?}"
        );
    }
    // A bound below HEAVY_OPS checks the glyph but does not cover the format.
    let mut fixtures = engine_fixtures();
    fixtures[0].min_ops[0].1 = HEAVY_OPS - 1;
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_coverage"]
    );
    let mut fixtures = type1_fixtures();
    for f in &mut fixtures {
        f.min_ops.clear();
    }
    let report = run_type1(&Toy::default(), &fixtures, &type1_hostile());
    assert_eq!(report.failed_checks(), vec!["font.fixture_coverage"]);
}

#[test]
fn an_overstated_min_ops_bound_fails() {
    // The TrueType composite needs exactly HEAVY_OPS operations; claiming one more makes the
    // check expect LimitExceeded where a correct backend succeeds.
    let mut fixtures = engine_fixtures();
    fixtures[0].min_ops[0].1 = HEAVY_OPS + 1;
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.outline_ops_limit"]
    );
}

#[test]
fn invalid_min_ops_entries_are_reported() {
    let broken: [fn(&mut FontFixture); 2] = [
        |f| f.min_ops[0].0 = GlyphId(f.glyph_count),
        |f| f.min_ops.push((GlyphId(0), 0)),
    ];
    for (i, breakage) in broken.iter().enumerate() {
        let mut fixtures = engine_fixtures();
        breakage(&mut fixtures[0]);
        assert_eq!(
            engine_failures(Toy::default(), &fixtures),
            vec!["font.fixture_valid"],
            "breakage {i}"
        );
    }
}

#[test]
fn identity_cids_are_caught() {
    let toy = Toy {
        identity_cids: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec![
            "font.fixture_loads",
            "font.hostile",
            "font.invariants",
            "font.truncated"
        ]
    );
    let report = run_type1(&toy, &type1_fixtures(), &type1_hostile());
    assert!(report.failed("font.invariants"));
    // An unlisted CID that the CID-keyed fixture maps fails it too.
    let mut fixtures = engine_fixtures();
    fixtures[2].cids.remove(0);
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads"]
    );
}

#[test]
fn identity_cmap_on_unknown_subtables_is_caught() {
    let toy = Toy {
        identity_cmap: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.hostile", "font.invariants", "font.truncated"]
    );
    let report = run_type1(&toy, &type1_fixtures(), &type1_hostile());
    assert!(report.failed("font.invariants"));
}

#[test]
fn cmap_subtables_on_non_sfnt_programs_are_caught() {
    let toy = Toy {
        fake_cmap_subtables: true,
        ..Toy::default()
    };
    // The sfnt fixtures may have (3,1); the bare CFF ones and the CFF hostile program may not.
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.hostile", "font.invariants", "font.truncated"]
    );
    let report = run_type1(&toy, &type1_fixtures(), &type1_hostile());
    assert_eq!(
        report.failed_checks(),
        vec!["font.hostile", "font.invariants", "font.truncated"]
    );
}

#[test]
fn names_past_glyph_64_and_beyond_128_names_are_checked() {
    // Audit 0001: names on glyph ids of 64 and up were never asked, and only 128 names were
    // looked up, so a correct backend failed.
    let mut fixtures = engine_fixtures();
    let f = &mut fixtures[0];
    let mut data = String::from_utf8(f.data.clone()).unwrap_or_default();
    for i in 0..200u16 {
        data.push_str(&format!("g 100 n{i}\n"));
        f.names.push((GlyphId(4 + i), format!("n{i}")));
    }
    f.data = data.into_bytes();
    f.glyph_count = 204;
    assert!(engine_failures(Toy::default(), &fixtures).is_empty());
    // And a wrong name on a late glyph is caught.
    if let Some(last) = fixtures[0].names.last_mut() {
        last.1 = "n0".into();
    }
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads"]
    );
}

#[test]
fn hostile_coverage_needs_every_format() {
    let mut hostile = engine_hostile();
    hostile.remove(1);
    let report = run_font_engine(&Toy::default(), &engine_fixtures(), &hostile);
    assert_eq!(report.failed_checks(), vec!["font.has_hostile_fixtures"]);
}

#[test]
fn hostile_fixtures_are_required() {
    let report = run_font_engine(&Toy::default(), &engine_fixtures(), &[]);
    assert_eq!(report.failed_checks(), vec!["font.has_hostile_fixtures"]);
    let report = run_type1(&Toy::default(), &type1_fixtures(), &[]);
    assert_eq!(report.failed_checks(), vec!["font.has_hostile_fixtures"]);
}

#[test]
fn engine_coverage_needs_each_cmap_and_a_cff_custom_encoding() {
    for drop in [cmap(3, 0), cmap(1, 0), cmap(3, 1)] {
        let mut fixtures = engine_fixtures();
        fixtures[0].cmap.retain(|(id, _, _)| *id != drop);
        assert_eq!(
            engine_failures(Toy::default(), &fixtures),
            vec!["font.fixture_coverage"],
            "without {drop:?}"
        );
    }
    let mut fixtures = engine_fixtures();
    fixtures[1].data = String::from_utf8_lossy(&fixtures[1].data)
        .replace("e 65 A\ne 32 space\n", "e standard\n")
        .into_bytes();
    fixtures[1].builtin_encoding = BuiltinEncoding::Standard;
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_coverage"]
    );
    // A TrueType fixture without a QuadTo does not cover TrueType.
    let mut fixtures = engine_fixtures();
    fixtures[0].glyphs[2].outline = outline("M 0 0 L 600 0 L 150 -20 Z");
    assert!(engine_failures(Toy::default(), &fixtures).contains(&"font.fixture_coverage"));
}

#[test]
fn fixture_coverage_is_required() {
    let fixtures = engine_fixtures();
    for skip in 0..fixtures.len() {
        let mut subset = fixtures.clone();
        subset.remove(skip);
        assert_eq!(
            engine_failures(Toy::default(), &subset),
            vec!["font.fixture_coverage"],
            "without fixture {skip}"
        );
    }
    assert_eq!(
        engine_failures(Toy::default(), &[]),
        vec![
            "font.fixture_coverage",
            "font.fixture_has_outline",
            "font.has_fixtures"
        ]
    );
}

#[test]
fn type1_ignoring_the_encoding_is_caught() {
    let toy = Toy {
        ignore_encoding: true,
        ..Toy::default()
    };
    let report = run_type1(&toy, &type1_fixtures(), &type1_hostile());
    assert_eq!(report.failed_checks(), vec!["font.fixture_loads"]);
}

#[test]
fn type1_requires_a_custom_encoding_fixture() {
    let fixtures: Vec<FontFixture> = type1_fixtures().into_iter().skip(1).collect();
    let report = run_type1(&Toy::default(), &fixtures, &type1_hostile());
    assert_eq!(report.failed_checks(), vec!["font.fixture_coverage"]);
    let report = run_type1(&Toy::default(), &engine_fixtures(), &type1_hostile());
    assert!(report.failed("font.fixture_coverage"));
}
