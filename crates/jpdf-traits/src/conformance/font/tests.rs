use super::*;
use crate::{FaceMetrics, PathSegment, Point};

/// A toy font program standing in for every format: lines of text.
///
/// ```text
/// toyfont <tt|cff|cff2|type1> <units per em>
/// g <advance> <name or -> [x y ...]    one glyph; points become MoveTo, LineTo…, Close
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
    /// Type 1: report `StandardEncoding` whatever the program says.
    ignore_encoding: bool,
}

struct ToyGlyph {
    advance: f32,
    name: Option<String>,
    points: Vec<Point>,
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
            upem: f32::from(upem),
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
                    let coords = f.map(|c| finite(Some(c))).collect::<Result<Vec<_>, _>>()?;
                    if coords.len() % 2 != 0 {
                        return Err(BackendError::Malformed("odd coordinates"));
                    }
                    let points = coords
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|&[x, y]| Point { x, y })
                        .collect();
                    face.glyphs.push(ToyGlyph {
                        advance,
                        name,
                        points,
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
        let in_range =
            face.cmap.iter().all(|e| e.2 < count) && face.cids.iter().all(|e| e.1 < count);
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
        let points = match self.glyph(glyph) {
            Err(_) if self.toy.accept_bad_glyph => return Ok(Outline::default()),
            g => &g?.points,
        };
        let too_many = u32::try_from(points.len()).map_or(true, |n| n > limits.max_outline_points);
        if too_many && !self.toy.ignore_point_limit {
            return Err(BackendError::LimitExceeded("outline points"));
        }
        let mut segments: Vec<PathSegment> = Vec::new();
        for (i, p) in points.iter().enumerate() {
            segments.push(if i == 0 {
                PathSegment::MoveTo(*p)
            } else {
                PathSegment::LineTo(*p)
            });
        }
        if !segments.is_empty() {
            segments.push(PathSegment::Close);
        }
        Ok(Outline { segments })
    }

    fn cmap_subtables(&self) -> Vec<CmapId> {
        let mut ids: Vec<CmapId> = Vec::new();
        for (id, _, _) in &self.cmap {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        ids
    }

    fn cmap_lookup(&self, cmap: CmapId, code: u32) -> Option<GlyphId> {
        let found = self
            .cmap
            .iter()
            .find(|(id, c, _)| *id == cmap && *c == code)
            .map(|e| GlyphId(e.2))
            .filter(|g| g.0 != 0);
        if found.is_none() && self.toy.cmap_off_by_one {
            return Some(GlyphId(self.count()));
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
        self.cids.iter().find(|e| e.0 == cid).map(|e| GlyphId(e.1))
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

const WIN_UNICODE: CmapId = CmapId {
    platform_id: 3,
    encoding_id: 1,
};

/// A fixture with `.notdef` (4 points), `space` (empty) and `A` (5 points, the largest).
fn fixture(
    name: &str,
    format: &str,
    program: FontProgram,
    outline_format: OutlineFormat,
    extra: &str,
) -> FontFixture {
    let data = format!(
        "toyfont {format} 1000\n\
         g 500 .notdef 0 0 400 0 400 700 0 700\n\
         g 250 space\n\
         g 600 A 0 0 300 700 600 0 450 -20 150 -20\n\
         {extra}"
    );
    FontFixture {
        name: name.into(),
        data: data.into_bytes(),
        program,
        outline_format,
        glyph_count: 3,
        font_matrix: [0.001, 0.0, 0.0, 0.001, 0.0, 0.0],
        glyphs: vec![
            ExpectedGlyph {
                glyph: GlyphId(0),
                advance: 500.0,
                points: 4,
                control_box: Some([0.0, 0.0, 400.0, 700.0]),
            },
            ExpectedGlyph {
                glyph: GlyphId(1),
                advance: 250.0,
                points: 0,
                control_box: None,
            },
            ExpectedGlyph {
                glyph: GlyphId(2),
                advance: 600.0,
                points: 5,
                control_box: Some([0.0, -20.0, 600.0, 700.0]),
            },
        ],
        cmap: Vec::new(),
        names: vec![(GlyphId(1), "space".into()), (GlyphId(2), "A".into())],
        cids: Vec::new(),
        builtin_encoding: BuiltinEncoding::None,
        tolerance: 0.01,
    }
}

fn engine_fixtures() -> Vec<FontFixture> {
    let mut tt = fixture(
        "tt",
        "tt",
        FontProgram::Sfnt,
        OutlineFormat::TrueType,
        "c 3 1 32 1\nc 3 1 65 2\n",
    );
    tt.cmap = vec![
        (WIN_UNICODE, 0x20, Some(GlyphId(1))),
        (WIN_UNICODE, 0x41, Some(GlyphId(2))),
        (WIN_UNICODE, 0x42, None),
    ];
    let mut cff = fixture(
        "cff",
        "cff",
        FontProgram::BareCff,
        OutlineFormat::Cff,
        "e standard\n",
    );
    cff.builtin_encoding = BuiltinEncoding::Standard;
    let mut cid = fixture(
        "cid",
        "cff",
        FontProgram::BareCff,
        OutlineFormat::Cff,
        "i 0 0\ni 7 2\n",
    );
    cid.cids = vec![(7, Some(GlyphId(2))), (8, None)];
    let cff2 = fixture("cff2", "cff2", FontProgram::Sfnt, OutlineFormat::Cff2, "");
    vec![tt, cff, cid, cff2]
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
    let report = run_font_engine(&toy, fixtures);
    assert!(report.checks_run > 0);
    report.failed_checks()
}

#[test]
fn correct_engine_passes() {
    let report = run_font_engine(&Toy::default(), &engine_fixtures());
    assert!(report.passed(), "{report}");
}

#[test]
fn correct_type1_parser_passes() {
    let report = run_type1(&Toy::default(), &type1_fixtures());
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
        vec!["font.invalid_glyph", "font.invariants", "font.truncated"]
    );
}

#[test]
fn out_of_range_cmap_results_are_caught() {
    let toy = Toy {
        cmap_off_by_one: true,
        ..Toy::default()
    };
    assert_eq!(
        engine_failures(toy, &engine_fixtures()),
        vec!["font.fixture_loads", "font.invariants", "font.truncated"]
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
        vec!["font.fixture_loads", "font.invariants", "font.truncated"]
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
    let mut fixtures = engine_fixtures();
    fixtures[0].glyphs[2].points = 6;
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads", "font.outline_points_limit"]
    );
    let mut fixtures = engine_fixtures();
    fixtures[0].glyphs[0].control_box = Some([0.0, 0.0, 400.0, 701.0]);
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
    fixtures[2].cids[0].1 = Some(GlyphId(1));
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_loads"]
    );
}

#[test]
fn an_invalid_fixture_is_reported() {
    let mut fixtures = engine_fixtures();
    fixtures[0].glyphs[0].glyph = GlyphId(3);
    assert_eq!(
        engine_failures(Toy::default(), &fixtures),
        vec!["font.fixture_valid"]
    );
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
    let report = run_type1(&toy, &type1_fixtures());
    assert_eq!(report.failed_checks(), vec!["font.fixture_loads"]);
}

#[test]
fn type1_requires_a_custom_encoding_fixture() {
    let fixtures: Vec<FontFixture> = type1_fixtures().into_iter().skip(1).collect();
    let report = run_type1(&Toy::default(), &fixtures);
    assert_eq!(report.failed_checks(), vec!["font.fixture_coverage"]);
    let report = run_type1(&Toy::default(), &engine_fixtures());
    assert!(report.failed("font.fixture_coverage"));
}
