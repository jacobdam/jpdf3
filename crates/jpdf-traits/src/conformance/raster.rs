//! Conformance suite for rasterizers (ISO 32000-2 §8.4–§8.5, §8.9.4, §11.3).
//!
//! The suite builds small display lists on a 32 × 32 canvas and checks pixels where every
//! correct rasterizer agrees: shapes on pixel boundaries (exact), pixel-centre sampling with
//! anti-aliasing off (exact), area coverage with anti-aliasing on (a tolerance), and the
//! §11.3 compositing and blend-mode formulas (±2 per 8-bit channel). It also feeds invalid
//! and degenerate lists, checks each limit at its exact boundary, and checks every output for
//! the right size and premultiplied alpha.

use super::{Report, guarded, pseudo_random};
use crate::{
    BackendError, BlendMode, Color, DrawCommand, FillRule, LineCap, LineJoin, Paint, PathSegment,
    Pixmap, Point, RasterLimits, Rasterizer, RgbaImage, StrokeStyle, Transform,
};

const SIZE: u32 = 32;
/// Allowed difference per 8-bit channel where backends round differently (blending,
/// premultiplication, opacity).
const ROUNDING: f32 = 2.0;
/// Allowed difference in alpha for an anti-aliased partial pixel (area coverage).
const COVERAGE: f32 = 20.0;

/// Runs every check against a [`Rasterizer`].
pub fn run(rasterizer: &dyn Rasterizer) -> Report {
    let mut s = Suite {
        r: rasterizer,
        report: Report::new("raster"),
    };
    s.fills();
    s.fill_rules();
    s.anti_aliasing();
    s.strokes();
    s.dashes();
    s.clips();
    s.layers();
    s.blending();
    s.images();
    s.invalid();
    s.degenerate();
    s.limits();
    s.hostile();
    s.report
}

struct Suite<'a> {
    r: &'a dyn Rasterizer,
    report: Report,
}

fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

/// A closed rectangle from `(x0, y0)` to `(x1, y1)`.
fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo(pt(x0, y0)),
        PathSegment::LineTo(pt(x1, y0)),
        PathSegment::LineTo(pt(x1, y1)),
        PathSegment::LineTo(pt(x0, y1)),
        PathSegment::Close,
    ]
}

/// The same rectangle wound the other way.
fn rect_reversed(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo(pt(x0, y0)),
        PathSegment::LineTo(pt(x0, y1)),
        PathSegment::LineTo(pt(x1, y1)),
        PathSegment::LineTo(pt(x1, y0)),
        PathSegment::Close,
    ]
}

fn line(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo(pt(x0, y0)),
        PathSegment::LineTo(pt(x1, y1)),
    ]
}

fn paint(color: Color, anti_alias: bool) -> Paint {
    Paint {
        color,
        blend: BlendMode::Normal,
        anti_alias,
    }
}

const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);
const OPAQUE_RED: [f32; 4] = [255.0, 0.0, 0.0, 255.0];
const OPAQUE_BLUE: [f32; 4] = [0.0, 0.0, 255.0, 255.0];
const CLEAR: [f32; 4] = [0.0; 4];

fn fill<'a>(path: &'a [PathSegment], color: Color, anti_alias: bool) -> DrawCommand<'a> {
    fill_with(
        path,
        FillRule::NonZero,
        Transform::IDENTITY,
        color,
        anti_alias,
    )
}

fn fill_with<'a>(
    path: &'a [PathSegment],
    rule: FillRule,
    transform: Transform,
    color: Color,
    anti_alias: bool,
) -> DrawCommand<'a> {
    DrawCommand::Fill {
        path,
        rule,
        transform,
        paint: paint(color, anti_alias),
    }
}

fn stroke<'a>(path: &'a [PathSegment], style: &'a StrokeStyle) -> DrawCommand<'a> {
    stroke_with(path, style, Transform::IDENTITY, false)
}

fn stroke_with<'a>(
    path: &'a [PathSegment],
    style: &'a StrokeStyle,
    transform: Transform,
    anti_alias: bool,
) -> DrawCommand<'a> {
    DrawCommand::Stroke {
        path,
        style,
        transform,
        paint: paint(RED, anti_alias),
    }
}

fn clip(path: &[PathSegment], rule: FillRule, anti_alias: bool) -> DrawCommand<'_> {
    DrawCommand::PushClip {
        path,
        rule,
        transform: Transform::IDENTITY,
        anti_alias,
    }
}

fn layer(opacity: f32, blend: BlendMode) -> DrawCommand<'static> {
    DrawCommand::PushLayer {
        opacity,
        blend,
        mask: None,
    }
}

fn u8_color(r: u8, g: u8, b: u8) -> Color {
    Color::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    )
}

/// Every output must have the requested size and premultiplied alpha.
fn output_problem(pixmap: &Pixmap, width: u32, height: u32) -> Option<String> {
    if pixmap.width != width || pixmap.height != height {
        return Some(format!(
            "pixmap is {}x{}, asked for {width}x{height}",
            pixmap.width, pixmap.height
        ));
    }
    let len = u64::from(width) * u64::from(height) * 4;
    if u64::try_from(pixmap.data.len()).ok() != Some(len) {
        return Some(format!("{} bytes, expected {len}", pixmap.data.len()));
    }
    pixmap
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .position(|[r, g, b, a]| r > a || g > a || b > a)
        .map(|i| format!("pixel {i} is not premultiplied"))
}

impl Suite<'_> {
    /// Renders and checks the output's invariants (`raster.output`).
    fn render_with(
        &mut self,
        width: u32,
        height: u32,
        commands: &[DrawCommand<'_>],
        limits: &RasterLimits,
    ) -> Option<Result<Pixmap, BackendError>> {
        let r = self.r;
        let out = guarded(|| r.render(width, height, commands, limits));
        if let Some(Ok(pixmap)) = &out {
            let problem = output_problem(pixmap, width, height);
            self.report.check("raster.output", problem.is_none(), || {
                problem.unwrap_or_default()
            });
        }
        out
    }

    /// Renders a valid list on the standard canvas; a failure is reported under `check`.
    fn draw(
        &mut self,
        check: &'static str,
        what: &str,
        commands: &[DrawCommand<'_>],
    ) -> Option<Pixmap> {
        let out = self.render_with(SIZE, SIZE, commands, &RasterLimits::default());
        match out {
            Some(Ok(pixmap)) => Some(pixmap),
            other => {
                let got = super::describe(&other);
                self.report
                    .check(check, false, || format!("{what}: render gave {got}"));
                None
            }
        }
    }

    /// Checks every pixel: `want(x, y)` gives the expected premultiplied RGBA (0–255) and
    /// tolerance, or `None` for a pixel the check skips. Reports the first mismatch.
    fn expect(
        &mut self,
        check: &'static str,
        what: &str,
        pixmap: Option<&Pixmap>,
        want: impl Fn(u32, u32) -> Option<([f32; 4], f32)>,
    ) {
        let Some(pixmap) = pixmap else { return };
        let mismatch = (0..pixmap.height)
            .flat_map(|y| (0..pixmap.width).map(move |x| (x, y)))
            .find_map(|(x, y)| {
                let (expected, tolerance) = want(x, y)?;
                let got = pixmap.pixel(x, y)?;
                let ok = got
                    .iter()
                    .zip(expected.iter())
                    .all(|(g, e)| (f32::from(*g) - e).abs() <= tolerance);
                (!ok).then(|| {
                    format!(
                        "{what}: pixel ({x}, {y}) is {got:?}, expected {expected:?} ±{tolerance}"
                    )
                })
            });
        self.report
            .check(check, mismatch.is_none(), || mismatch.unwrap_or_default());
    }

    /// Checks that `inside` pixels are exactly `color` and every other pixel is clear.
    fn expect_region(
        &mut self,
        check: &'static str,
        what: &str,
        pixmap: Option<&Pixmap>,
        color: [f32; 4],
        inside: impl Fn(u32, u32) -> bool,
    ) {
        self.expect(check, what, pixmap, |x, y| {
            Some((if inside(x, y) { color } else { CLEAR }, 0.0))
        });
    }

    /// Checks one pixel's alpha against `expected ± tolerance`.
    fn expect_alpha(
        &mut self,
        check: &'static str,
        what: &str,
        pixmap: Option<&Pixmap>,
        (px, py): (u32, u32),
        expected: f32,
        tolerance: f32,
    ) {
        let Some(pixmap) = pixmap else { return };
        let got = pixmap.pixel(px, py).map(|p| f32::from(p[3]));
        let ok = got.is_some_and(|a| (a - expected).abs() <= tolerance);
        self.report.check(check, ok, || {
            format!("{what}: alpha at ({px}, {py}) is {got:?}, expected {expected} ±{tolerance}")
        });
    }

    fn fills(&mut self) {
        let c = "raster.fill";
        let r = rect(4.0, 4.0, 20.0, 12.0);
        for aa in [true, false] {
            let out = self.draw(c, "axis-aligned rect", &[fill(&r, RED, aa)]);
            self.expect_region(
                c,
                &format!("rect (aa {aa})"),
                out.as_ref(),
                OPAQUE_RED,
                |x, y| (4..20).contains(&x) && (4..12).contains(&y),
            );
        }
        let small = rect(0.0, 0.0, 4.0, 4.0);
        let scaled = Transform::new(2.0, 0.0, 0.0, 2.0, 8.0, 8.0);
        let out = self.draw(
            c,
            "scaled rect",
            &[fill_with(&small, FillRule::NonZero, scaled, RED, true)],
        );
        self.expect_region(c, "scaled rect", out.as_ref(), OPAQUE_RED, |x, y| {
            (8..16).contains(&x) && (8..16).contains(&y)
        });
        // (x, y) → (24 − y, x + 4): the rect (0,0)–(8,4) lands on x 20–24, y 4–12.
        let wide = rect(0.0, 0.0, 8.0, 4.0);
        let rotated = Transform::new(0.0, 1.0, -1.0, 0.0, 24.0, 4.0);
        let out = self.draw(
            c,
            "rotated rect",
            &[fill_with(&wide, FillRule::NonZero, rotated, RED, true)],
        );
        self.expect_region(c, "rotated rect", out.as_ref(), OPAQUE_RED, |x, y| {
            (20..24).contains(&x) && (4..12).contains(&y)
        });
        let half = Color::new(0.0, 0.0, 1.0, 0.5);
        let out = self.draw(c, "half-alpha rect", &[fill(&r, half, true)]);
        self.expect(c, "half-alpha rect", out.as_ref(), |x, y| {
            let inside = (4..20).contains(&x) && (4..12).contains(&y);
            Some(if inside {
                ([0.0, 0.0, 127.5, 127.5], 1.0)
            } else {
                (CLEAR, 0.0)
            })
        });
        // A circle of radius 10 around (16, 16) from four cubic arcs.
        let k = 10.0 * 0.552_284_8;
        let circle = [
            PathSegment::MoveTo(pt(26.0, 16.0)),
            PathSegment::CurveTo(pt(26.0, 16.0 + k), pt(16.0 + k, 26.0), pt(16.0, 26.0)),
            PathSegment::CurveTo(pt(16.0 - k, 26.0), pt(6.0, 16.0 + k), pt(6.0, 16.0)),
            PathSegment::CurveTo(pt(6.0, 16.0 - k), pt(16.0 - k, 6.0), pt(16.0, 6.0)),
            PathSegment::CurveTo(pt(16.0 + k, 6.0), pt(26.0, 16.0 - k), pt(26.0, 16.0)),
            PathSegment::Close,
        ];
        let out = self.draw(c, "cubic circle", &[fill(&circle, RED, true)]);
        for (at, alpha) in [
            ((16, 16), 255.0),
            ((16, 7), 255.0),
            ((7, 16), 255.0),
            ((7, 7), 0.0),
            ((24, 24), 0.0),
        ] {
            self.expect_alpha(c, "cubic circle", out.as_ref(), at, alpha, 0.0);
        }
        // A quadratic arch peaking at y = 16.
        let arch = [
            PathSegment::MoveTo(pt(4.0, 28.0)),
            PathSegment::QuadTo(pt(16.0, 4.0), pt(28.0, 28.0)),
            PathSegment::Close,
        ];
        let out = self.draw(c, "quadratic arch", &[fill(&arch, RED, true)]);
        self.expect_alpha(c, "quadratic arch", out.as_ref(), (16, 20), 255.0, 0.0);
        self.expect_alpha(c, "quadratic arch", out.as_ref(), (16, 12), 0.0, 0.0);
    }

    fn fill_rules(&mut self) {
        let c = "raster.fill_rule";
        let mut same = rect(4.0, 4.0, 28.0, 28.0);
        same.extend(rect(10.0, 10.0, 22.0, 22.0));
        let mut opposite = rect(4.0, 4.0, 28.0, 28.0);
        opposite.extend(rect_reversed(10.0, 10.0, 22.0, 22.0));
        let outer = |x: u32, y: u32| (4..28).contains(&x) && (4..28).contains(&y);
        let ring =
            |x: u32, y: u32| outer(x, y) && !((10..22).contains(&x) && (10..22).contains(&y));
        let cases: [(&str, &[PathSegment], FillRule, bool); 4] = [
            ("nonzero, same winding", &same, FillRule::NonZero, false),
            (
                "nonzero, opposite winding",
                &opposite,
                FillRule::NonZero,
                true,
            ),
            ("even-odd, same winding", &same, FillRule::EvenOdd, true),
            (
                "even-odd, opposite winding",
                &opposite,
                FillRule::EvenOdd,
                true,
            ),
        ];
        for (what, path, rule, hole) in cases {
            let out = self.draw(
                c,
                what,
                &[fill_with(path, rule, Transform::IDENTITY, RED, false)],
            );
            self.expect_region(c, what, out.as_ref(), OPAQUE_RED, |x, y| {
                if hole { ring(x, y) } else { outer(x, y) }
            });
        }
    }

    fn anti_aliasing(&mut self) {
        let c = "raster.anti_alias";
        // Column 4 is 3/4 covered by x 4.25–5 and 1/4 covered by x 4–4.25.
        let three_quarters = rect(4.25, 4.0, 5.0, 8.0);
        let quarter = rect(4.0, 4.0, 4.25, 8.0);
        let out = self.draw(c, "3/4 pixel, aa", &[fill(&three_quarters, RED, true)]);
        self.expect_alpha(c, "3/4 pixel, aa", out.as_ref(), (4, 5), 191.25, COVERAGE);
        let out = self.draw(c, "1/4 pixel, aa", &[fill(&quarter, RED, true)]);
        self.expect_alpha(c, "1/4 pixel, aa", out.as_ref(), (4, 5), 63.75, COVERAGE);
        // Without anti-aliasing the pixel centre (4.5) decides.
        let out = self.draw(c, "3/4 pixel, no aa", &[fill(&three_quarters, RED, false)]);
        self.expect_alpha(c, "3/4 pixel, no aa", out.as_ref(), (4, 5), 255.0, 0.0);
        let out = self.draw(c, "1/4 pixel, no aa", &[fill(&quarter, RED, false)]);
        self.expect_alpha(c, "1/4 pixel, no aa", out.as_ref(), (4, 5), 0.0, 0.0);
    }

    fn strokes(&mut self) {
        let c = "raster.stroke";
        let horizontal = line(4.0, 8.0, 28.0, 8.0);
        let mut style = StrokeStyle::solid(4.0);
        let out = self.draw(c, "butt cap", &[stroke(&horizontal, &style)]);
        self.expect_region(c, "butt cap", out.as_ref(), OPAQUE_RED, |x, y| {
            (4..28).contains(&x) && (6..10).contains(&y)
        });
        style.cap = LineCap::Square;
        let out = self.draw(c, "square cap", &[stroke(&horizontal, &style)]);
        self.expect_region(c, "square cap", out.as_ref(), OPAQUE_RED, |x, y| {
            (2..30).contains(&x) && (6..10).contains(&y)
        });
        style.cap = LineCap::Round;
        let out = self.draw(c, "round cap", &[stroke(&horizontal, &style)]);
        for (at, alpha) in [
            ((3, 8), 255.0),
            ((28, 8), 255.0),
            ((1, 8), 0.0),
            ((30, 8), 0.0),
            ((16, 6), 255.0),
            ((16, 10), 0.0),
            // Corners a square cap would cover: centre (2.5, 6.5) is 2.12 from (4, 8).
            ((2, 6), 0.0),
            ((29, 6), 0.0),
        ] {
            self.expect_alpha(c, "round cap", out.as_ref(), at, alpha, 0.0);
        }
        // A right-angle corner at (20, 4) with an 8-wide pen. The miter square reaches
        // (24, 0), so pixel (23, 0) (centre 4.95 from the vertex, beyond the bevel line
        // x − y = 20) is covered by the miter only. Pixel (22, 1) (centre 3.54 from the
        // vertex, x − y = 21) is inside the round join and the miter but outside the bevel.
        // Miter limit 1 is below √2, so that miter bevels.
        let corner = [
            PathSegment::MoveTo(pt(4.0, 4.0)),
            PathSegment::LineTo(pt(20.0, 4.0)),
            PathSegment::LineTo(pt(20.0, 20.0)),
        ];
        for (join, limit, tip, arc, what) in [
            (LineJoin::Miter, 10.0, 255.0, 255.0, "miter join"),
            (LineJoin::Miter, 1.0, 0.0, 0.0, "miter join over its limit"),
            (LineJoin::Bevel, 10.0, 0.0, 0.0, "bevel join"),
            (LineJoin::Round, 10.0, 0.0, 255.0, "round join"),
        ] {
            let style = StrokeStyle {
                join,
                miter_limit: limit,
                ..StrokeStyle::solid(8.0)
            };
            let out = self.draw(c, what, &[stroke(&corner, &style)]);
            self.expect_alpha(c, what, out.as_ref(), (23, 0), tip, 0.0);
            self.expect_alpha(c, what, out.as_ref(), (22, 1), arc, 0.0);
            self.expect_alpha(c, what, out.as_ref(), (19, 3), 255.0, 0.0);
        }
        // A closed square joins at its start; the same path left open gets butt caps there.
        let open = [
            PathSegment::MoveTo(pt(8.0, 8.0)),
            PathSegment::LineTo(pt(24.0, 8.0)),
            PathSegment::LineTo(pt(24.0, 24.0)),
            PathSegment::LineTo(pt(8.0, 24.0)),
            PathSegment::LineTo(pt(8.0, 8.0)),
        ];
        let mut closed = open.to_vec();
        closed.push(PathSegment::Close);
        let style = StrokeStyle::solid(2.0);
        let out = self.draw(c, "closed subpath", &[stroke(&closed, &style)]);
        self.expect_alpha(c, "closed subpath", out.as_ref(), (7, 7), 255.0, 0.0);
        let out = self.draw(c, "open subpath", &[stroke(&open, &style)]);
        self.expect_alpha(c, "open subpath", out.as_ref(), (7, 7), 0.0, 0.0);
        // The transform scales the pen: width 2 at scale 2 is the 4-pixel butt stroke.
        let short = line(2.0, 4.0, 14.0, 4.0);
        let style = StrokeStyle::solid(2.0);
        let scale = Transform::new(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);
        let out = self.draw(
            c,
            "scaled pen",
            &[stroke_with(&short, &style, scale, false)],
        );
        self.expect_region(c, "scaled pen", out.as_ref(), OPAQUE_RED, |x, y| {
            (4..28).contains(&x) && (6..10).contains(&y)
        });
        // Width 0 is the thinnest visible line.
        let hair = line(4.0, 8.5, 28.0, 8.5);
        let style = StrokeStyle::solid(0.0);
        let out = self.draw(
            c,
            "hairline",
            &[stroke_with(&hair, &style, Transform::IDENTITY, true)],
        );
        let visible = out
            .as_ref()
            .and_then(|p| p.pixel(16, 8))
            .is_some_and(|p| p[3] > 0);
        if out.is_some() {
            self.report
                .check(c, visible, || "hairline: pixel (16, 8) is clear".into());
        }
        self.expect_alpha(c, "hairline", out.as_ref(), (16, 6), 0.0, 0.0);
        self.expect_alpha(c, "hairline", out.as_ref(), (16, 10), 0.0, 0.0);
    }

    fn dashes(&mut self) {
        let c = "raster.dash";
        let across = line(0.0, 8.0, 32.0, 8.0);
        for (dash, phase, offset, what) in [
            (vec![4.0, 4.0], 0.0, 0, "dash [4 4] 0"),
            (vec![4.0, 4.0], 2.0, 2, "dash [4 4] 2"),
            (vec![4.0], 0.0, 0, "dash [4] 0"),
            (vec![4.0, 4.0], 10.0, 2, "dash [4 4] 10"),
            (vec![4.0, 4.0], -2.0, 6, "dash [4 4] -2"),
            (vec![4.0], 12.0, 4, "dash [4] 12"),
            // f32 1e30 is a multiple of 2^76, so ±1e30 reduces to phase 0 exactly.
            (vec![4.0, 4.0], 1e30, 0, "dash phase 1e30"),
            (vec![4.0, 4.0], -1e30, 0, "dash phase -1e30"),
        ] {
            let style = StrokeStyle {
                dash,
                dash_phase: phase,
                ..StrokeStyle::solid(2.0)
            };
            let out = self.draw(c, what, &[stroke(&across, &style)]);
            self.expect_region(c, what, out.as_ref(), OPAQUE_RED, |x, y| {
                (7..9).contains(&y) && (x + offset) % 8 < 4
            });
        }
        // Dash lengths are in user space: [2 2] at scale 2 has an 8-pixel period.
        let half = line(0.0, 4.0, 16.0, 4.0);
        let style = StrokeStyle {
            dash: vec![2.0, 2.0],
            ..StrokeStyle::solid(1.0)
        };
        let scale = Transform::new(2.0, 0.0, 0.0, 2.0, 0.0, 0.0);
        let out = self.draw(
            c,
            "dash under a transform",
            &[stroke_with(&half, &style, scale, false)],
        );
        self.expect_region(
            c,
            "dash under a transform",
            out.as_ref(),
            OPAQUE_RED,
            |x, y| (7..9).contains(&y) && x % 8 < 4,
        );
        // Hazards for naive dash loops: each must end quickly with `Ok` or (for 1.6e31
        // dashes) `LimitExceeded`.
        let across = line(0.0, 8.0, 32.0, 8.0);
        for (dash, phase, must_fail, what) in [
            (vec![1e30], 0.0, false, "dash [1e30]"),
            (vec![1e-30, 1e-30], 0.0, true, "dash [1e-30 1e-30]"),
            (vec![f32::MIN_POSITIVE], 0.0, true, "dash [smallest normal]"),
        ] {
            let style = StrokeStyle {
                dash,
                dash_phase: phase,
                ..StrokeStyle::solid(2.0)
            };
            let out = self.render_with(
                SIZE,
                SIZE,
                &[stroke(&across, &style)],
                &RasterLimits::default(),
            );
            let ok = if must_fail {
                matches!(out, Some(Err(BackendError::LimitExceeded(_))))
            } else {
                matches!(out, Some(Ok(_)))
            };
            self.report
                .check(c, ok, || format!("{what}: {}", super::describe(&out)));
        }
    }

    fn clips(&mut self) {
        let c = "raster.clip";
        let canvas = rect(0.0, 0.0, 32.0, 32.0);
        let a = rect(8.0, 8.0, 16.0, 16.0);
        let b = rect(12.0, 0.0, 32.0, 32.0);
        let in_a = |x: u32, y: u32| (8..16).contains(&x) && (8..16).contains(&y);
        // The clip's own transform: (0,0)–(4,4) at scale 2, offset 8 is the same square.
        let small = rect(0.0, 0.0, 4.0, 4.0);
        let out = self.draw(
            c,
            "clip transform",
            &[
                DrawCommand::PushClip {
                    path: &small,
                    rule: FillRule::NonZero,
                    transform: Transform::new(2.0, 0.0, 0.0, 2.0, 8.0, 8.0),
                    anti_alias: false,
                },
                fill(&canvas, RED, false),
                DrawCommand::PopClip,
            ],
        );
        self.expect_region(c, "clip transform", out.as_ref(), OPAQUE_RED, in_a);
        for aa in [true, false] {
            let what = format!("one clip (aa {aa})");
            let out = self.draw(
                c,
                &what,
                &[
                    clip(&a, FillRule::NonZero, aa),
                    fill(&canvas, RED, aa),
                    DrawCommand::PopClip,
                ],
            );
            self.expect_region(c, &what, out.as_ref(), OPAQUE_RED, in_a);
        }
        let out = self.draw(
            c,
            "nested clips",
            &[
                clip(&a, FillRule::NonZero, false),
                clip(&b, FillRule::NonZero, false),
                fill(&canvas, RED, false),
                DrawCommand::PopClip,
                DrawCommand::PopClip,
            ],
        );
        self.expect_region(c, "nested clips", out.as_ref(), OPAQUE_RED, |x, y| {
            in_a(x, y) && x >= 12
        });
        let out = self.draw(
            c,
            "pop restores the outer clip",
            &[
                clip(&a, FillRule::NonZero, false),
                clip(&b, FillRule::NonZero, false),
                DrawCommand::PopClip,
                fill(&canvas, BLUE, false),
                DrawCommand::PopClip,
            ],
        );
        self.expect_region(
            c,
            "pop restores the outer clip",
            out.as_ref(),
            OPAQUE_BLUE,
            in_a,
        );
        let out = self.draw(
            c,
            "pop ends the clip",
            &[
                clip(&a, FillRule::NonZero, false),
                DrawCommand::PopClip,
                fill(&canvas, RED, false),
            ],
        );
        self.expect_region(c, "pop ends the clip", out.as_ref(), OPAQUE_RED, |_, _| {
            true
        });
        let mut ring = rect(4.0, 4.0, 28.0, 28.0);
        ring.extend(rect(10.0, 10.0, 22.0, 22.0));
        let out = self.draw(
            c,
            "even-odd clip",
            &[
                clip(&ring, FillRule::EvenOdd, false),
                fill(&canvas, RED, false),
                DrawCommand::PopClip,
            ],
        );
        self.expect_region(c, "even-odd clip", out.as_ref(), OPAQUE_RED, |x, y| {
            (4..28).contains(&x)
                && (4..28).contains(&y)
                && !((10..22).contains(&x) && (10..22).contains(&y))
        });
        let sliver = rect(8.0, 0.0, 8.5, 32.0);
        let out = self.draw(
            c,
            "anti-aliased clip edge",
            &[
                clip(&sliver, FillRule::NonZero, true),
                fill(&canvas, RED, false),
                DrawCommand::PopClip,
            ],
        );
        self.expect_alpha(
            c,
            "anti-aliased clip edge",
            out.as_ref(),
            (8, 16),
            127.5,
            COVERAGE,
        );
    }

    fn layers(&mut self) {
        let c = "raster.layer";
        let canvas = rect(0.0, 0.0, 32.0, 32.0);
        let out = self.draw(
            c,
            "layer opacity",
            &[
                layer(0.5, BlendMode::Normal),
                fill(&canvas, RED, false),
                DrawCommand::PopLayer,
            ],
        );
        self.expect(c, "layer opacity", out.as_ref(), |_, _| {
            Some(([127.5, 0.0, 0.0, 127.5], ROUNDING))
        });
        // Group opacity applies to the group's result once: where blue covers red inside the
        // group, the result is half-transparent blue, not blue over red.
        let first = rect(4.0, 4.0, 20.0, 20.0);
        let second = rect(12.0, 12.0, 28.0, 28.0);
        let out = self.draw(
            c,
            "group opacity",
            &[
                layer(0.5, BlendMode::Normal),
                fill(&first, RED, false),
                fill(&second, BLUE, false),
                DrawCommand::PopLayer,
            ],
        );
        self.expect(c, "group opacity", out.as_ref(), |x, y| {
            (x == 16 && y == 16).then_some(([0.0, 0.0, 127.5, 127.5], ROUNDING))
        });
        let out = self.draw(
            c,
            "nested opacity",
            &[
                layer(0.5, BlendMode::Normal),
                layer(0.5, BlendMode::Normal),
                fill(&canvas, RED, false),
                DrawCommand::PopLayer,
                DrawCommand::PopLayer,
            ],
        );
        self.expect(c, "nested opacity", out.as_ref(), |_, _| {
            Some(([63.75, 0.0, 0.0, 63.75], ROUNDING))
        });
        // Mask: 255 on the left half, 0 on the right, 128 on the left of the bottom rows.
        let mask: Vec<u8> = (0..SIZE)
            .flat_map(|y| {
                (0..SIZE).map(move |x| match (x < 16, y >= 24) {
                    (true, false) => 255,
                    (true, true) => 128,
                    _ => 0,
                })
            })
            .collect();
        let out = self.draw(
            c,
            "soft mask",
            &[
                DrawCommand::PushLayer {
                    opacity: 1.0,
                    blend: BlendMode::Normal,
                    mask: Some(&mask),
                },
                fill(&canvas, RED, false),
                DrawCommand::PopLayer,
            ],
        );
        self.expect(c, "soft mask", out.as_ref(), |x, y| {
            Some(match (x < 16, y >= 24) {
                (true, false) => (OPAQUE_RED, 0.0),
                (true, true) => ([128.0, 0.0, 0.0, 128.0], ROUNDING),
                _ => (CLEAR, 0.0),
            })
        });
    }

    fn blending(&mut self) {
        let c = "raster.blend";
        let canvas = rect(0.0, 0.0, 32.0, 32.0);
        let cb = [204u8, 102, 51];
        let cs = [51u8, 153, 230];
        let backdrop = u8_color(cb[0], cb[1], cb[2]);
        let source = u8_color(cs[0], cs[1], cs[2]);
        let unit = |c: [u8; 3]| c.map(|v| f32::from(v) / 255.0);
        for mode in BlendMode::ALL {
            let b = blend(mode, unit(cb), unit(cs));
            let want = [b[0] * 255.0, b[1] * 255.0, b[2] * 255.0, 255.0];
            let what = format!("{mode:?} fill");
            let out = self.draw(
                c,
                &what,
                &[
                    fill(&canvas, backdrop, false),
                    DrawCommand::Fill {
                        path: &canvas,
                        rule: FillRule::NonZero,
                        transform: Transform::IDENTITY,
                        paint: Paint {
                            color: source,
                            blend: mode,
                            anti_alias: false,
                        },
                    },
                ],
            );
            self.expect(c, &what, out.as_ref(), |_, _| Some((want, ROUNDING)));
            let what = format!("{mode:?} layer");
            let out = self.draw(
                c,
                &what,
                &[
                    fill(&canvas, backdrop, false),
                    layer(1.0, mode),
                    fill(&canvas, source, false),
                    DrawCommand::PopLayer,
                ],
            );
            self.expect(c, &what, out.as_ref(), |_, _| Some((want, ROUNDING)));
        }
        // Over a transparent backdrop every mode gives the source (§11.3.6, αb = 0).
        for mode in [
            BlendMode::Multiply,
            BlendMode::Difference,
            BlendMode::Luminosity,
        ] {
            let what = format!("{mode:?} over nothing");
            let out = self.draw(
                c,
                &what,
                &[DrawCommand::Fill {
                    path: &canvas,
                    rule: FillRule::NonZero,
                    transform: Transform::IDENTITY,
                    paint: Paint {
                        color: source,
                        blend: mode,
                        anti_alias: false,
                    },
                }],
            );
            let want = [f32::from(cs[0]), f32::from(cs[1]), f32::from(cs[2]), 255.0];
            self.expect(c, &what, out.as_ref(), |_, _| Some((want, ROUNDING)));
        }
        // A half-transparent Multiply source: (1 − αs)·Cb + αs·B(Cb, Cs).
        let b = blend(BlendMode::Multiply, unit(cb), unit(cs));
        let mix = |i: usize| {
            let backdrop = cb.get(i).map_or(0.0, |v| f32::from(*v));
            let blended = b.get(i).copied().unwrap_or(0.0) * 255.0;
            0.5 * backdrop + 0.5 * blended
        };
        let want = [mix(0), mix(1), mix(2), 255.0];
        let out = self.draw(
            c,
            "half-alpha Multiply",
            &[
                fill(&canvas, backdrop, false),
                DrawCommand::Fill {
                    path: &canvas,
                    rule: FillRule::NonZero,
                    transform: Transform::IDENTITY,
                    paint: Paint {
                        color: Color { a: 0.5, ..source },
                        blend: BlendMode::Multiply,
                        anti_alias: false,
                    },
                },
            ],
        );
        self.expect(c, "half-alpha Multiply", out.as_ref(), |_, _| {
            Some((want, ROUNDING))
        });
    }

    fn images(&mut self) {
        let c = "raster.image";
        // 3 × 2, straight alpha: red, green, half-transparent blue / white, black, clear.
        let data: Vec<u8> = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 128, //
            255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 0, 0,
        ];
        let image = RgbaImage {
            width: 3,
            height: 2,
            data: &data,
        };
        let premultiplied = |col: u32, row: u32| -> [f32; 4] {
            let i = usize::try_from((row * 3 + col) * 4).unwrap_or(usize::MAX);
            let px = data.get(i..i.saturating_add(4)).unwrap_or(&[0, 0, 0, 0]);
            let a = px.get(3).map_or(0.0, |v| f32::from(*v));
            let ch = |k: usize| px.get(k).map_or(0.0, |v| f32::from(*v)) * a / 255.0;
            [ch(0), ch(1), ch(2), a]
        };
        let draw_image = |transform, interpolate, opacity| DrawCommand::Image {
            image,
            transform,
            interpolate,
            opacity,
            blend: BlendMode::Normal,
        };
        // The unit square onto x 5–8, y 7–9; the first row on top.
        let place = Transform::new(3.0, 0.0, 0.0, -2.0, 5.0, 9.0);
        for interpolate in [false, true] {
            let what = format!("1:1 image (interpolate {interpolate})");
            let out = self.draw(c, &what, &[draw_image(place, interpolate, 1.0)]);
            self.expect(c, &what, out.as_ref(), |x, y| {
                let inside = (5..8).contains(&x) && (7..9).contains(&y);
                Some(if inside {
                    (premultiplied(x - 5, y - 7), 1.0)
                } else {
                    (CLEAR, 0.0)
                })
            });
        }
        let scaled = Transform::new(12.0, 0.0, 0.0, -8.0, 4.0, 20.0);
        let out = self.draw(c, "4x nearest image", &[draw_image(scaled, false, 1.0)]);
        self.expect(c, "4x nearest image", out.as_ref(), |x, y| {
            let inside = (4..16).contains(&x) && (12..20).contains(&y);
            Some(if inside {
                (premultiplied((x - 4) / 4, (y - 12) / 4), 1.0)
            } else {
                (CLEAR, 0.0)
            })
        });
        // Images blend too: opaque red Multiply over (204, 102, 51) gives (204, 0, 0).
        let canvas = rect(0.0, 0.0, 32.0, 32.0);
        let out = self.draw(
            c,
            "image blend mode",
            &[
                fill(&canvas, u8_color(204, 102, 51), false),
                DrawCommand::Image {
                    image,
                    transform: place,
                    interpolate: false,
                    opacity: 1.0,
                    blend: BlendMode::Multiply,
                },
            ],
        );
        self.expect(c, "image blend mode", out.as_ref(), |x, y| {
            (x == 5 && y == 7).then_some(([204.0, 0.0, 0.0, 255.0], ROUNDING))
        });
        let out = self.draw(c, "image opacity", &[draw_image(place, false, 0.5)]);
        self.expect(c, "image opacity", out.as_ref(), |x, y| {
            (x == 5 && y == 7).then_some(([127.5, 0.0, 0.0, 127.5], ROUNDING))
        });
    }

    fn invalid(&mut self) {
        let c = "raster.invalid";
        let r = rect(4.0, 4.0, 20.0, 20.0);
        let nan_path = rect(4.0, f32::NAN, 20.0, 20.0);
        let inf_curve = [
            PathSegment::MoveTo(pt(0.0, 0.0)),
            PathSegment::CurveTo(pt(1.0, 1.0), pt(f32::INFINITY, 2.0), pt(3.0, 3.0)),
        ];
        let no_move = [PathSegment::LineTo(pt(4.0, 4.0))];
        let after_close = [
            PathSegment::MoveTo(pt(0.0, 0.0)),
            PathSegment::Close,
            PathSegment::LineTo(pt(4.0, 4.0)),
        ];
        let ok_style = StrokeStyle::solid(1.0);
        let styles = [
            ("negative width", StrokeStyle::solid(-1.0)),
            ("NaN width", StrokeStyle::solid(f32::NAN)),
            (
                "miter limit below 1",
                StrokeStyle {
                    miter_limit: 0.5,
                    ..StrokeStyle::solid(1.0)
                },
            ),
            (
                "negative dash",
                StrokeStyle {
                    dash: vec![2.0, -1.0],
                    ..StrokeStyle::solid(1.0)
                },
            ),
            (
                "all-zero dash",
                StrokeStyle {
                    dash: vec![0.0, 0.0],
                    ..StrokeStyle::solid(1.0)
                },
            ),
            (
                "NaN dash phase",
                StrokeStyle {
                    dash: vec![2.0],
                    dash_phase: f32::NAN,
                    ..StrokeStyle::solid(1.0)
                },
            ),
            ("infinite width", StrokeStyle::solid(f32::INFINITY)),
            (
                "NaN miter limit",
                StrokeStyle {
                    miter_limit: f32::NAN,
                    ..StrokeStyle::solid(1.0)
                },
            ),
            (
                "infinite miter limit",
                StrokeStyle {
                    miter_limit: f32::INFINITY,
                    ..StrokeStyle::solid(1.0)
                },
            ),
            (
                "infinite dash",
                StrokeStyle {
                    dash: vec![2.0, f32::INFINITY],
                    ..StrokeStyle::solid(1.0)
                },
            ),
        ];
        let pixels = [0u8; 4 * 4];
        let image = |width, height, data| DrawCommand::Image {
            image: RgbaImage {
                width,
                height,
                data,
            },
            transform: Transform::new(4.0, 0.0, 0.0, 4.0, 0.0, 0.0),
            interpolate: false,
            opacity: 1.0,
            blend: BlendMode::Normal,
        };
        let short_mask = [255u8; 10];
        let nan_transform = Transform::new(1.0, 0.0, 0.0, f32::NAN, 0.0, 0.0);
        let inf_transform = Transform::new(1.0, 0.0, 0.0, 1.0, f32::INFINITY, 0.0);
        let mut cases: Vec<(String, u32, u32, Vec<DrawCommand<'_>>)> = vec![
            ("zero width".into(), 0, SIZE, vec![]),
            ("zero height".into(), SIZE, 0, vec![]),
            (
                "NaN coordinate".into(),
                SIZE,
                SIZE,
                vec![fill(&nan_path, RED, true)],
            ),
            (
                "infinite control point".into(),
                SIZE,
                SIZE,
                vec![stroke(&inf_curve, &ok_style)],
            ),
            (
                "NaN transform".into(),
                SIZE,
                SIZE,
                vec![fill_with(&r, FillRule::NonZero, nan_transform, RED, true)],
            ),
            (
                "infinite transform".into(),
                SIZE,
                SIZE,
                vec![stroke_with(&r, &ok_style, inf_transform, true)],
            ),
            (
                "colour above 1".into(),
                SIZE,
                SIZE,
                vec![fill(&r, Color::new(1.5, 0.0, 0.0, 1.0), true)],
            ),
            (
                "negative alpha".into(),
                SIZE,
                SIZE,
                vec![fill(&r, Color::new(0.0, 0.0, 0.0, -0.1), true)],
            ),
            (
                "NaN colour".into(),
                SIZE,
                SIZE,
                vec![fill(&r, Color::new(f32::NAN, 0.0, 0.0, 1.0), true)],
            ),
            (
                "path without MoveTo".into(),
                SIZE,
                SIZE,
                vec![fill(&no_move, RED, true)],
            ),
            (
                "segment after Close".into(),
                SIZE,
                SIZE,
                vec![fill(&after_close, RED, true)],
            ),
            (
                "clip without MoveTo".into(),
                SIZE,
                SIZE,
                vec![
                    clip(&no_move, FillRule::NonZero, true),
                    DrawCommand::PopClip,
                ],
            ),
            (
                "image with no pixels".into(),
                SIZE,
                SIZE,
                vec![image(0, 4, &[])],
            ),
            (
                "image data too short".into(),
                SIZE,
                SIZE,
                vec![image(2, 3, &pixels)],
            ),
            (
                "image data too long".into(),
                SIZE,
                SIZE,
                vec![image(1, 3, &pixels)],
            ),
            (
                "NaN image opacity".into(),
                SIZE,
                SIZE,
                vec![DrawCommand::Image {
                    image: RgbaImage {
                        width: 2,
                        height: 2,
                        data: &pixels,
                    },
                    transform: Transform::IDENTITY,
                    interpolate: false,
                    opacity: f32::NAN,
                    blend: BlendMode::Normal,
                }],
            ),
            (
                "negative image opacity".into(),
                SIZE,
                SIZE,
                vec![DrawCommand::Image {
                    image: RgbaImage {
                        width: 2,
                        height: 2,
                        data: &pixels,
                    },
                    transform: Transform::IDENTITY,
                    interpolate: false,
                    opacity: -0.5,
                    blend: BlendMode::Normal,
                }],
            ),
            (
                "layer opacity above 1".into(),
                SIZE,
                SIZE,
                vec![layer(1.5, BlendMode::Normal), DrawCommand::PopLayer],
            ),
            (
                "NaN layer opacity".into(),
                SIZE,
                SIZE,
                vec![layer(f32::NAN, BlendMode::Normal), DrawCommand::PopLayer],
            ),
            (
                "image with zero height".into(),
                SIZE,
                SIZE,
                vec![image(4, 0, &[])],
            ),
            (
                "image of u32::MAX × u32::MAX".into(),
                SIZE,
                SIZE,
                vec![image(u32::MAX, u32::MAX, &[])],
            ),
            (
                "mask of the wrong size".into(),
                SIZE,
                SIZE,
                vec![
                    DrawCommand::PushLayer {
                        opacity: 1.0,
                        blend: BlendMode::Normal,
                        mask: Some(&short_mask),
                    },
                    DrawCommand::PopLayer,
                ],
            ),
            (
                "PopClip without a clip".into(),
                SIZE,
                SIZE,
                vec![DrawCommand::PopClip],
            ),
            (
                "PopLayer without a layer".into(),
                SIZE,
                SIZE,
                vec![DrawCommand::PopLayer],
            ),
            (
                "PopLayer closing a clip".into(),
                SIZE,
                SIZE,
                vec![clip(&r, FillRule::NonZero, true), DrawCommand::PopLayer],
            ),
            (
                "PopClip closing a layer".into(),
                SIZE,
                SIZE,
                vec![layer(1.0, BlendMode::Normal), DrawCommand::PopClip],
            ),
            (
                "unclosed clip".into(),
                SIZE,
                SIZE,
                vec![clip(&r, FillRule::NonZero, true), fill(&r, RED, true)],
            ),
            (
                "unclosed layer".into(),
                SIZE,
                SIZE,
                vec![layer(1.0, BlendMode::Normal), fill(&r, RED, true)],
            ),
        ];
        for (what, style) in &styles {
            cases.push(((*what).into(), SIZE, SIZE, vec![stroke(&r, style)]));
        }
        for (what, width, height, commands) in &cases {
            let out = self.render_with(*width, *height, commands, &RasterLimits::default());
            let ok = matches!(out, Some(Err(BackendError::InvalidArgument(_))));
            self.report.check(c, ok, || {
                format!(
                    "{what}: expected InvalidArgument, got {}",
                    super::describe(&out)
                )
            });
        }
    }

    fn degenerate(&mut self) {
        let c = "raster.degenerate";
        let all = |_, _| true;
        let none = |_, _| false;
        let out = self.draw(c, "empty list", &[]);
        self.expect_region(c, "empty list", out.as_ref(), OPAQUE_RED, none);
        let empty: [PathSegment; 0] = [];
        let style = StrokeStyle::solid(2.0);
        let out = self.draw(
            c,
            "empty path",
            &[fill(&empty, RED, true), stroke(&empty, &style)],
        );
        self.expect_region(c, "empty path", out.as_ref(), OPAQUE_RED, none);
        let flat = rect(4.0, 4.0, 4.0, 20.0);
        let out = self.draw(c, "zero-area fill", &[fill(&flat, RED, true)]);
        self.expect_region(c, "zero-area fill", out.as_ref(), OPAQUE_RED, none);
        let lone = [PathSegment::MoveTo(pt(8.0, 8.0))];
        self.draw(
            c,
            "lone MoveTo",
            &[fill(&lone, RED, true), stroke(&lone, &style)],
        );
        let singular = Transform::new(0.0, 0.0, 0.0, 0.0, 16.0, 16.0);
        let big = rect(0.0, 0.0, 32.0, 32.0);
        let out = self.draw(
            c,
            "singular transform",
            &[
                fill_with(&big, FillRule::NonZero, singular, RED, true),
                stroke_with(&big, &style, singular, false),
            ],
        );
        self.expect_region(c, "singular transform", out.as_ref(), OPAQUE_RED, none);
        let canvas = rect(0.0, 0.0, 32.0, 32.0);
        let out = self.draw(
            c,
            "empty clip",
            &[
                clip(&empty, FillRule::NonZero, true),
                fill(&canvas, RED, true),
                DrawCommand::PopClip,
            ],
        );
        self.expect_region(c, "empty clip", out.as_ref(), OPAQUE_RED, none);
        let huge = rect(-1e30, -1e30, 1e30, 1e30);
        for aa in [true, false] {
            let what = format!("huge rect (aa {aa})");
            let out = self.draw(c, &what, &[fill(&huge, RED, aa)]);
            self.expect_region(c, &what, out.as_ref(), OPAQUE_RED, all);
        }
        let wild = [
            PathSegment::MoveTo(pt(-1e30, 3.0)),
            PathSegment::CurveTo(pt(1e30, -1e30), pt(-1e30, 1e30), pt(1e30, 5.0)),
            PathSegment::QuadTo(pt(1e-30, 1e30), pt(f32::MIN_POSITIVE, 0.0)),
            PathSegment::Close,
        ];
        let wide = StrokeStyle {
            cap: LineCap::Round,
            join: LineJoin::Round,
            ..StrokeStyle::solid(3.0)
        };
        self.draw(
            c,
            "huge curves",
            &[
                fill(&wild, RED, true),
                stroke(&wild, &wide),
                clip(&wild, FillRule::EvenOdd, true),
                DrawCommand::PopClip,
            ],
        );
        let fat = StrokeStyle::solid(1e30);
        let across = line(0.0, 16.0, 32.0, 16.0);
        let out = self.draw(c, "huge stroke width", &[stroke(&across, &fat)]);
        self.expect_region(c, "huge stroke width", out.as_ref(), OPAQUE_RED, all);
        let dots = StrokeStyle {
            dash: vec![0.0, 4.0],
            cap: LineCap::Round,
            ..StrokeStyle::solid(2.0)
        };
        // §8.4.3.6: a zero-length dash with round caps is a dot (radius 1 at x = 0, 4, 8, …).
        let out = self.draw(c, "zero-length dashes", &[stroke(&across, &dots)]);
        self.expect_alpha(c, "zero-length dashes", out.as_ref(), (8, 16), 255.0, 0.0);
        self.expect_alpha(c, "zero-length dashes", out.as_ref(), (10, 16), 0.0, 0.0);
        let out = self.render_with(1, 1, &[fill(&canvas, RED, true)], &RasterLimits::default());
        let ok = matches!(&out, Some(Ok(p)) if p.pixel(0, 0) == Some([255, 0, 0, 255]));
        self.report
            .check(c, ok, || format!("1x1 canvas: {}", super::describe(&out)));
    }

    fn limits(&mut self) {
        let base = RasterLimits::default();
        let boundary = |s: &mut Self,
                        check: &'static str,
                        what: &str,
                        (width, height): (u32, u32),
                        commands: &[DrawCommand<'_>],
                        at: RasterLimits,
                        below: RasterLimits| {
            let pass = s.render_with(width, height, commands, &at);
            let fail = s.render_with(width, height, commands, &below);
            let ok = matches!(pass, Some(Ok(_)))
                && matches!(fail, Some(Err(BackendError::LimitExceeded(_))));
            s.report.check(check, ok, || {
                format!(
                    "{what}: at the limit {}, one below {}",
                    super::describe(&pass),
                    super::describe(&fail)
                )
            });
        };
        let r = rect(4.0, 4.0, 20.0, 20.0);
        let paint_r = [fill(&r, RED, true)];
        for (w, h) in [(32, 32), (64, 16), (16, 64)] {
            let px = u64::from(w) * u64::from(h);
            boundary(
                self,
                "raster.limits.canvas",
                &format!("{w}x{h} canvas"),
                (w, h),
                &paint_r,
                RasterLimits {
                    max_canvas_pixels: px,
                    ..base
                },
                RasterLimits {
                    max_canvas_pixels: px - 1,
                    ..base
                },
            );
        }
        let style = StrokeStyle::solid(2.0);
        let five = RasterLimits {
            max_path_segments: 5,
            ..base
        };
        let four = RasterLimits {
            max_path_segments: 4,
            ..base
        };
        boundary(
            self,
            "raster.limits.segments",
            "fill",
            (SIZE, SIZE),
            &[fill(&r, RED, true)],
            five,
            four,
        );
        boundary(
            self,
            "raster.limits.segments",
            "stroke",
            (SIZE, SIZE),
            &[stroke(&r, &style)],
            five,
            four,
        );
        boundary(
            self,
            "raster.limits.segments",
            "clip",
            (SIZE, SIZE),
            &[clip(&r, FillRule::NonZero, true), DrawCommand::PopClip],
            five,
            four,
        );
        // 1000 dashes of 0.5 along a 1000-unit line (most of it off the canvas).
        let long = line(0.0, 8.0, 1000.0, 8.0);
        let fine = StrokeStyle {
            dash: vec![0.5, 0.5],
            ..StrokeStyle::solid(1.0)
        };
        let dashed = [stroke(&long, &fine)];
        let ok = self.render_with(
            SIZE,
            SIZE,
            &dashed,
            &RasterLimits {
                max_path_segments: 2000,
                ..base
            },
        );
        let over = self.render_with(
            SIZE,
            SIZE,
            &dashed,
            &RasterLimits {
                max_path_segments: 100,
                ..base
            },
        );
        let pass =
            matches!(ok, Some(Ok(_))) && matches!(over, Some(Err(BackendError::LimitExceeded(_))));
        self.report.check("raster.limits.dashes", pass, || {
            format!(
                "1000 dashes: limit 2000 gave {}, limit 100 gave {}",
                super::describe(&ok),
                super::describe(&over)
            )
        });
        let nested_clips = [
            clip(&r, FillRule::NonZero, true),
            clip(&r, FillRule::NonZero, true),
            clip(&r, FillRule::NonZero, true),
            fill(&r, RED, true),
            DrawCommand::PopClip,
            DrawCommand::PopClip,
            DrawCommand::PopClip,
        ];
        boundary(
            self,
            "raster.limits.clip_depth",
            "3 nested clips",
            (SIZE, SIZE),
            &nested_clips,
            RasterLimits {
                max_clip_depth: 3,
                ..base
            },
            RasterLimits {
                max_clip_depth: 2,
                ..base
            },
        );
        let sequential: Vec<DrawCommand<'_>> = (0..3)
            .flat_map(|_| [clip(&r, FillRule::NonZero, true), DrawCommand::PopClip])
            .collect();
        let out = self.render_with(
            SIZE,
            SIZE,
            &sequential,
            &RasterLimits {
                max_clip_depth: 1,
                ..base
            },
        );
        self.report.check(
            "raster.limits.clip_depth",
            matches!(out, Some(Ok(_))),
            || {
                format!(
                    "3 sequential clips at depth limit 1: {}",
                    super::describe(&out)
                )
            },
        );
        let nested_layers = [
            layer(1.0, BlendMode::Normal),
            layer(1.0, BlendMode::Normal),
            layer(1.0, BlendMode::Normal),
            fill(&r, RED, true),
            DrawCommand::PopLayer,
            DrawCommand::PopLayer,
            DrawCommand::PopLayer,
        ];
        boundary(
            self,
            "raster.limits.layer_depth",
            "3 nested layers",
            (SIZE, SIZE),
            &nested_layers,
            RasterLimits {
                max_layer_depth: 3,
                ..base
            },
            RasterLimits {
                max_layer_depth: 2,
                ..base
            },
        );
        let sequential: Vec<DrawCommand<'_>> = (0..3)
            .flat_map(|_| [layer(1.0, BlendMode::Normal), DrawCommand::PopLayer])
            .collect();
        let out = self.render_with(
            SIZE,
            SIZE,
            &sequential,
            &RasterLimits {
                max_layer_depth: 1,
                ..base
            },
        );
        self.report.check(
            "raster.limits.layer_depth",
            matches!(out, Some(Ok(_))),
            || {
                format!(
                    "3 sequential layers at depth limit 1: {}",
                    super::describe(&out)
                )
            },
        );
        // 2 layers and 3 clips deep: 1024 × (4 × 3 + 3) bytes.
        let mixed = [
            layer(1.0, BlendMode::Normal),
            clip(&r, FillRule::NonZero, true),
            layer(1.0, BlendMode::Normal),
            clip(&r, FillRule::NonZero, true),
            clip(&r, FillRule::NonZero, true),
            fill(&r, RED, true),
            DrawCommand::PopClip,
            DrawCommand::PopClip,
            DrawCommand::PopLayer,
            DrawCommand::PopClip,
            DrawCommand::PopLayer,
        ];
        let bytes = 1024 * (4 * 3 + 3);
        boundary(
            self,
            "raster.limits.working_bytes",
            "2 layers, 3 clips",
            (SIZE, SIZE),
            &mixed,
            RasterLimits {
                max_working_bytes: bytes,
                ..base
            },
            RasterLimits {
                max_working_bytes: bytes - 1,
                ..base
            },
        );
    }

    /// Pseudo-random valid lists with extreme coordinates: never a panic, always `Ok` or
    /// `LimitExceeded`.
    fn hostile(&mut self) {
        const VALUES: [f32; 12] = [
            -1e30,
            -1e6,
            -1.0,
            0.0,
            1e-30,
            0.5,
            16.0,
            31.99,
            32.0,
            1e6,
            1e30,
            f32::MIN_POSITIVE,
        ];
        for seed in 0..48u64 {
            let bytes = pseudo_random(256, seed.wrapping_add(0x5eed));
            let mut it = bytes.iter().copied().cycle();
            let mut next = move || it.next().unwrap_or(0);
            let mut value = || {
                VALUES
                    .get(usize::from(next()) % VALUES.len())
                    .copied()
                    .unwrap_or(0.0)
            };
            let mut paths: Vec<Vec<PathSegment>> = Vec::new();
            for _ in 0..4 {
                let mut path = vec![PathSegment::MoveTo(pt(value(), value()))];
                path.push(PathSegment::LineTo(pt(value(), value())));
                path.push(PathSegment::CurveTo(
                    pt(value(), value()),
                    pt(value(), value()),
                    pt(value(), value()),
                ));
                path.push(PathSegment::QuadTo(
                    pt(value(), value()),
                    pt(value(), value()),
                ));
                path.push(PathSegment::Close);
                paths.push(path);
            }
            let transform = Transform::new(value(), value(), value(), value(), value(), value());
            let style = StrokeStyle {
                width: value().abs(),
                cap: LineCap::Round,
                join: LineJoin::Miter,
                miter_limit: 4.0,
                dash: match (seed % 2, value().abs(), value().abs()) {
                    (1, on, off) if on > 0.0 || off > 0.0 => vec![on, off],
                    _ => vec![],
                },
                dash_phase: value(),
            };
            let [a, b, c, d] =
                [0usize, 1, 2, 3].map(|i| paths.get(i).map_or(&[][..], Vec::as_slice));
            let commands = [
                fill_with(a, FillRule::EvenOdd, transform, RED, seed % 3 == 0),
                DrawCommand::PushClip {
                    path: b,
                    rule: FillRule::NonZero,
                    transform,
                    anti_alias: true,
                },
                layer(
                    0.5,
                    BlendMode::ALL
                        .get(usize::try_from(seed % 16).unwrap_or(0))
                        .copied()
                        .unwrap_or(BlendMode::Normal),
                ),
                stroke_with(c, &style, transform, true),
                fill(d, BLUE, true),
                DrawCommand::PopLayer,
                DrawCommand::PopClip,
            ];
            let out = self.render_with(SIZE, SIZE, &commands, &RasterLimits::default());
            let ok = matches!(out, Some(Ok(_)) | Some(Err(BackendError::LimitExceeded(_))));
            self.report.check("raster.hostile", ok, || {
                format!("seed {seed}: {}", super::describe(&out))
            });
        }
    }
}

/// `B(Cb, Cs)` for one blend mode (ISO 32000-2 §11.3.5, Tables 134 and 135), on colour
/// components in `0..=1`.
fn blend(mode: BlendMode, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    let separable = |f: fn(f32, f32) -> f32| [f(cb[0], cs[0]), f(cb[1], cs[1]), f(cb[2], cs[2])];
    match mode {
        BlendMode::Normal => cs,
        BlendMode::Multiply => separable(|b, s| b * s),
        BlendMode::Screen => separable(screen),
        BlendMode::Overlay => separable(|b, s| hard_light(s, b)),
        BlendMode::Darken => separable(f32::min),
        BlendMode::Lighten => separable(f32::max),
        BlendMode::ColorDodge => separable(|b, s| {
            if b == 0.0 {
                0.0
            } else if s >= 1.0 {
                1.0
            } else {
                (b / (1.0 - s)).min(1.0)
            }
        }),
        BlendMode::ColorBurn => separable(|b, s| {
            if b >= 1.0 {
                1.0
            } else if s == 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - b) / s).min(1.0)
            }
        }),
        BlendMode::HardLight => separable(hard_light),
        BlendMode::SoftLight => separable(|b, s| {
            if s <= 0.5 {
                b - (1.0 - 2.0 * s) * b * (1.0 - b)
            } else {
                let d = if b <= 0.25 {
                    ((16.0 * b - 12.0) * b + 4.0) * b
                } else {
                    b.sqrt()
                };
                b + (2.0 * s - 1.0) * (d - b)
            }
        }),
        BlendMode::Difference => separable(|b, s| (b - s).abs()),
        BlendMode::Exclusion => separable(|b, s| b + s - 2.0 * b * s),
        BlendMode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        BlendMode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        BlendMode::Color => set_lum(cs, lum(cb)),
        BlendMode::Luminosity => set_lum(cb, lum(cs)),
    }
}

fn screen(b: f32, s: f32) -> f32 {
    b + s - b * s
}

fn hard_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 {
        b * 2.0 * s
    } else {
        screen(b, 2.0 * s - 1.0)
    }
}

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 {
        out = out.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1.0 {
        out = out.map(|v| l + (v - l) * (1.0 - l) / (x - l));
    }
    out
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|v| v + d))
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

/// `SetSat`: the largest component becomes `s`, the smallest 0, the middle one scales.
fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    if max > min {
        c.map(|v| (v - min) * s / (max - min))
    } else {
        [0.0; 3]
    }
}

#[cfg(test)]
mod tests;
