use super::*;
use crate::check_scene;

/// A slow, simple rasterizer: 4 × 4 supersampling (or the pixel centre without
/// anti-aliasing), winding numbers in f64, strokes as unions of polygons, f32 compositing.
/// Each flag breaks one thing the suite must catch.
#[derive(Default, Clone, Copy)]
struct Toy {
    /// Validate input but enforce no limit (dashes included).
    ignore_limits: bool,
    /// Skip validation entirely (and panic on a NaN).
    no_validation: bool,
    /// Sample pixel centres even when anti-aliasing is on.
    no_aa: bool,
    /// Apply layer opacity to each object instead of the group.
    per_object_opacity: bool,
    /// Blend without the `(1 − αb)` term, so blending over nothing darkens.
    naive_blend: bool,
    ignore_miter_limit: bool,
    /// Draw images upside down.
    flip_image: bool,
    /// Return straight alpha.
    straight_output: bool,
    ignore_even_odd: bool,
    ignore_dash_phase: bool,
    /// Count nested clips as one level.
    ignore_clip_depth: bool,
}

type P = (f64, f64);

/// `v` floored and saturated to `0..=u32::MAX`, without an `as` cast (the workspace denies
/// truncating casts): set bits from the top while the value stays at most `v`.
fn to_u32(v: f64) -> u32 {
    let mut out = 0u32;
    for bit in (0..32).rev() {
        let candidate = out | (1 << bit);
        if f64::from(candidate) <= v {
            out = candidate;
        }
    }
    out
}
type Polygon = Vec<P>;

fn apply(t: &Transform, (x, y): P) -> P {
    let [a, b, c, d, e, f] = [t.a, t.b, t.c, t.d, t.e, t.f].map(f64::from);
    (a * x + c * y + e, b * x + d * y + f)
}

/// Subpaths as polylines in user space, curves split into 16 lines; whether each is closed.
fn flatten(path: &[PathSegment]) -> Vec<(Polygon, bool)> {
    let mut out: Vec<(Polygon, bool)> = Vec::new();
    let mut current: Polygon = Vec::new();
    let q = |p: Point| (f64::from(p.x), f64::from(p.y));
    for segment in path {
        let last = current.last().copied().unwrap_or((0.0, 0.0));
        match *segment {
            PathSegment::MoveTo(p) => {
                if !current.is_empty() {
                    out.push((std::mem::take(&mut current), false));
                }
                current.push(q(p));
            }
            PathSegment::LineTo(p) => current.push(q(p)),
            PathSegment::QuadTo(c, p) => {
                let (c, p) = (q(c), q(p));
                for i in 1..=16 {
                    let t = f64::from(i) / 16.0;
                    let u = 1.0 - t;
                    current.push((
                        u * u * last.0 + 2.0 * u * t * c.0 + t * t * p.0,
                        u * u * last.1 + 2.0 * u * t * c.1 + t * t * p.1,
                    ));
                }
            }
            PathSegment::CurveTo(c1, c2, p) => {
                let (c1, c2, p) = (q(c1), q(c2), q(p));
                for i in 1..=16 {
                    let t = f64::from(i) / 16.0;
                    let u = 1.0 - t;
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    current.push((
                        w[0] * last.0 + w[1] * c1.0 + w[2] * c2.0 + w[3] * p.0,
                        w[0] * last.1 + w[1] * c1.1 + w[2] * c2.1 + w[3] * p.1,
                    ));
                }
            }
            PathSegment::Close => out.push((std::mem::take(&mut current), true)),
        }
    }
    if !current.is_empty() {
        out.push((current, false));
    }
    out
}

/// Winding number of `polygons` (implicitly closed) around `(x, y)`.
fn winding(polygons: &[Polygon], (x, y): P) -> i32 {
    let mut w = 0;
    for poly in polygons {
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            if (a.1 <= y) != (b.1 <= y) {
                let cross_x = a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0);
                if cross_x > x {
                    w += if b.1 > a.1 { 1 } else { -1 };
                }
            }
        }
    }
    w
}

/// Per-pixel coverage of the region `inside`.
fn coverage(w: u32, h: u32, aa: bool, inside: &dyn Fn(P) -> bool) -> Vec<f32> {
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let (x, y) = (f64::from(x), f64::from(y));
            out.push(if aa {
                let mut n = 0;
                for j in 0..4 {
                    for i in 0..4 {
                        let s = (
                            x + (f64::from(i) + 0.5) / 4.0,
                            y + (f64::from(j) + 0.5) / 4.0,
                        );
                        if inside(s) {
                            n += 1;
                        }
                    }
                }
                n as f32 / 16.0
            } else if inside((x + 0.5, y + 0.5)) {
                1.0
            } else {
                0.0
            });
        }
    }
    out
}

/// Per-pixel coverage of the union of `polys`, testing each polygon only over the samples
/// inside its bounding box (strokes of long dashed lines make many off-canvas polygons).
fn union_coverage(w: u32, h: u32, aa: bool, polys: &[Polygon]) -> Vec<f32> {
    let k: u32 = if aa { 4 } else { 1 };
    let (gw, gh) = (w * k, h * k);
    let mut grid = vec![false; (gw * gh) as usize];
    let scale = f64::from(k);
    for poly in polys {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &(x, y) in poly {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        let range = |lo: f64, hi: f64, n: u32| {
            let a = to_u32((lo * scale - 0.5).floor()).min(n);
            let b = to_u32((hi * scale + 0.5).ceil()).min(n);
            a..b
        };
        let one = std::slice::from_ref(poly);
        for j in range(y0, y1, gh) {
            for i in range(x0, x1, gw) {
                let cell = &mut grid[(j * gw + i) as usize];
                if !*cell {
                    let s = ((f64::from(i) + 0.5) / scale, (f64::from(j) + 0.5) / scale);
                    *cell = winding(one, s) != 0;
                }
            }
        }
    }
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let mut n = 0u32;
            for j in 0..k {
                for i in 0..k {
                    n += u32::from(grid[((y * k + j) * gw + x * k + i) as usize]);
                }
            }
            out.push(n as f32 / (k * k) as f32);
        }
    }
    out
}

/// Splits a polyline into dashes (user space).
fn dash_polyline(
    points: &Polygon,
    dash: &[f32],
    phase: f32,
    max: usize,
) -> Result<Vec<Polygon>, BackendError> {
    let pattern: Vec<f64> = if dash.len() % 2 == 1 {
        dash.iter().chain(dash).map(|d| f64::from(*d)).collect()
    } else {
        dash.iter().map(|d| f64::from(*d)).collect()
    };
    let period: f64 = pattern.iter().sum();
    let mut index = 0;
    let mut left = pattern[0];
    let mut skip = f64::from(phase) % period;
    while skip > 0.0 {
        if skip >= left {
            skip -= left;
            index = (index + 1) % pattern.len();
            left = pattern[index];
        } else {
            left -= skip;
            skip = 0.0;
        }
    }
    let mut pieces: Vec<Polygon> = Vec::new();
    let mut current: Polygon = Vec::new();
    let mut steps = 0usize;
    if index % 2 == 0 {
        current.push(points[0]);
    }
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let mut pos = 0.0;
        while len - pos > left {
            // Bounded even when `pos + left == pos` (lengths near 1e30).
            steps += 1;
            if steps > max.saturating_mul(2).saturating_add(2) {
                return Err(BackendError::LimitExceeded("dashes"));
            }
            pos += left;
            let t = pos / len;
            let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            if index % 2 == 0 {
                current.push(p);
                pieces.push(std::mem::take(&mut current));
            } else {
                current.push(p);
            }
            index = (index + 1) % pattern.len();
            left = pattern[index];
        }
        left -= len - pos;
        if index % 2 == 0 {
            current.push(b);
        }
    }
    if index % 2 == 0 && !current.is_empty() {
        pieces.push(current);
    }
    if pieces.len() > max {
        return Err(BackendError::LimitExceeded("dashes"));
    }
    Ok(pieces)
}

fn circle((cx, cy): P, r: f64) -> Polygon {
    (0..64)
        .map(|i| {
            let a = f64::from(i) * std::f64::consts::TAU / 64.0;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Stroke outline polygons (in the polyline's space) for one polyline.
fn stroke_polygons(
    points: &Polygon,
    closed: bool,
    hw: f64,
    style: &StrokeStyle,
    toy: Toy,
    out: &mut Vec<Polygon>,
) {
    let mut pts: Polygon = Vec::new();
    for p in points {
        if pts.last() != Some(p) {
            pts.push(*p);
        }
    }
    if closed && pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    if pts.len() < 2 {
        if style.cap == LineCap::Round
            && let Some(p) = pts.first()
        {
            out.push(circle(*p, hw));
        }
        return;
    }
    let n = pts.len();
    let segs: Vec<(P, P)> = (0..if closed { n } else { n - 1 })
        .map(|i| (pts[i], pts[(i + 1) % n]))
        .collect();
    let dir = |(a, b): (P, P)| {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        ((b.0 - a.0) / len, (b.1 - a.1) / len)
    };
    for &(a, b) in &segs {
        let d = dir((a, b));
        let nx = -d.1 * hw;
        let ny = d.0 * hw;
        out.push(vec![
            (a.0 + nx, a.1 + ny),
            (b.0 + nx, b.1 + ny),
            (b.0 - nx, b.1 - ny),
            (a.0 - nx, a.1 - ny),
        ]);
    }
    if !closed {
        for (p, d) in [
            (pts[0], dir(segs[0])),
            (pts[n - 1], {
                let e = dir(segs[segs.len() - 1]);
                (-e.0, -e.1)
            }),
        ] {
            // `d` points into the line from the cap's end.
            let (nx, ny) = (-d.1 * hw, d.0 * hw);
            match style.cap {
                LineCap::Butt => {}
                LineCap::Round => out.push(circle(p, hw)),
                LineCap::Square => {
                    let back = (p.0 - d.0 * hw, p.1 - d.1 * hw);
                    out.push(vec![
                        (p.0 + nx, p.1 + ny),
                        (back.0 + nx, back.1 + ny),
                        (back.0 - nx, back.1 - ny),
                        (p.0 - nx, p.1 - ny),
                    ]);
                }
            }
        }
    }
    let joins: Vec<usize> = if closed {
        (0..n).collect()
    } else {
        (1..n - 1).collect()
    };
    for i in joins {
        let d0 = dir(segs[(i + segs.len() - 1) % segs.len()]);
        let d1 = dir(segs[i % segs.len()]);
        let v = pts[i];
        let cross = d0.0 * d1.1 - d0.1 * d1.0;
        if cross.abs() < 1e-12 {
            continue;
        }
        let s = if cross > 0.0 { -1.0 } else { 1.0 };
        let n0 = (-d0.1 * s, d0.0 * s);
        let n1 = (-d1.1 * s, d1.0 * s);
        let o0 = (v.0 + n0.0 * hw, v.1 + n0.1 * hw);
        let o1 = (v.0 + n1.0 * hw, v.1 + n1.1 * hw);
        match style.join {
            LineJoin::Round => out.push(circle(v, hw)),
            LineJoin::Bevel => out.push(vec![v, o0, o1]),
            LineJoin::Miter => {
                let m = (n0.0 + n1.0, n0.1 + n1.1);
                let m2 = m.0 * m.0 + m.1 * m.1;
                let ratio = 2.0 / m2.sqrt();
                if ratio <= f64::from(style.miter_limit) || toy.ignore_miter_limit {
                    let tip = (v.0 + m.0 * 2.0 * hw / m2, v.1 + m.1 * 2.0 * hw / m2);
                    out.push(vec![v, o0, tip, o1]);
                } else {
                    out.push(vec![v, o0, o1]);
                }
            }
        }
    }
}

impl Toy {
    fn shape(
        &self,
        path: &[PathSegment],
        rule: FillRule,
        t: &Transform,
        w: u32,
        h: u32,
        aa: bool,
    ) -> Vec<f32> {
        let polys: Vec<Polygon> = flatten(path)
            .into_iter()
            .map(|(p, _)| p.into_iter().map(|q| apply(t, q)).collect())
            .collect();
        let even_odd = rule == FillRule::EvenOdd && !self.ignore_even_odd;
        coverage(w, h, aa && !self.no_aa, &|s| {
            let n = winding(&polys, s);
            if even_odd { n % 2 != 0 } else { n != 0 }
        })
    }

    fn stroke_shape(
        &self,
        path: &[PathSegment],
        style: &StrokeStyle,
        t: &Transform,
        (w, h): (u32, u32),
        aa: bool,
        max: usize,
    ) -> Result<Vec<f32>, BackendError> {
        let mut polys: Vec<Polygon> = Vec::new();
        let hairline = style.width == 0.0;
        let hw = if hairline {
            0.5
        } else {
            f64::from(style.width) / 2.0
        };
        let mut dashes = 0usize;
        for (points, closed) in flatten(path) {
            let pieces: Vec<(Polygon, bool)> = if style.dash.is_empty() {
                vec![(points, closed)]
            } else {
                let mut line = points.clone();
                if closed && let Some(first) = points.first() {
                    line.push(*first);
                }
                let phase = if self.ignore_dash_phase {
                    0.0
                } else {
                    style.dash_phase
                };
                let limit = if self.ignore_limits {
                    1 << 15
                } else {
                    max.saturating_sub(dashes)
                };
                let pieces = dash_polyline(&line, &style.dash, phase, limit)?;
                dashes += pieces.len();
                pieces.into_iter().map(|p| (p, false)).collect()
            };
            let reach = if hairline {
                1.0
            } else {
                hw * [t.a, t.b, t.c, t.d]
                    .iter()
                    .map(|v| f64::from(v.abs()))
                    .sum::<f64>()
                    + 1.0
            };
            for (piece, closed) in pieces {
                // Skip pieces whose pen can't reach the canvas.
                let device: Polygon = piece.iter().map(|q| apply(t, *q)).collect();
                let off = device.iter().all(|p| p.0 < -reach)
                    || device.iter().all(|p| p.1 < -reach)
                    || device.iter().all(|p| p.0 > f64::from(w) + reach)
                    || device.iter().all(|p| p.1 > f64::from(h) + reach);
                if off {
                    continue;
                }
                let mut out = Vec::new();
                if hairline {
                    stroke_polygons(&device, closed, hw, style, *self, &mut out);
                    polys.extend(out);
                } else {
                    stroke_polygons(&piece, closed, hw, style, *self, &mut out);
                    polys.extend(
                        out.into_iter()
                            .map(|p| p.into_iter().map(|q| apply(t, q)).collect::<Polygon>()),
                    );
                }
            }
        }
        Ok(union_coverage(w, h, aa && !self.no_aa, &polys))
    }
}

/// Composites a straight-alpha source over a premultiplied backdrop (§11.3.6).
fn composite(dst: &mut [f32; 4], cs: [f32; 3], a_s: f32, mode: BlendMode, naive: bool) {
    if a_s <= 0.0 {
        return;
    }
    let ab = dst[3];
    let cb = if ab > 0.0 {
        [dst[0] / ab, dst[1] / ab, dst[2] / ab]
    } else {
        [0.0; 3]
    };
    let b = blend(mode, cb, cs);
    for i in 0..3 {
        dst[i] = if naive {
            (1.0 - a_s) * dst[i] + a_s * b[i]
        } else {
            (1.0 - a_s) * dst[i] + a_s * (1.0 - ab) * cs[i] + a_s * ab * b[i]
        };
    }
    dst[3] = ab + a_s - ab * a_s;
}

struct Layer {
    buf: Vec<[f32; 4]>,
    opacity: f32,
    blend: BlendMode,
    mask: Option<Vec<f32>>,
}

impl Rasterizer for Toy {
    fn render(
        &self,
        width: u32,
        height: u32,
        commands: &[DrawCommand<'_>],
        limits: &RasterLimits,
    ) -> Result<Pixmap, BackendError> {
        if self.no_validation {
            let finite = commands.iter().all(|c| match c {
                DrawCommand::Fill { path, .. }
                | DrawCommand::Stroke { path, .. }
                | DrawCommand::PushClip { path, .. } => path
                    .iter()
                    .all(|s| s.points().all(|p| p.x.is_finite() && p.y.is_finite())),
                _ => true,
            });
            assert!(finite, "NaN in a path");
        } else if self.ignore_limits {
            let open = RasterLimits {
                max_canvas_pixels: u64::MAX,
                max_path_segments: u32::MAX,
                max_clip_depth: u16::MAX,
                max_layer_depth: u16::MAX,
                max_working_bytes: u64::MAX,
            };
            check_scene(width, height, commands, &open)?;
        } else if self.ignore_clip_depth {
            let flat_clips = RasterLimits {
                max_clip_depth: u16::MAX,
                max_working_bytes: u64::MAX,
                ..*limits
            };
            check_scene(width, height, commands, &flat_clips)?;
        } else {
            check_scene(width, height, commands, limits)?;
        }
        let (w, h) = (width, height);
        let n = (w * h) as usize;
        let max_dashes = usize::try_from(limits.max_path_segments).unwrap_or(usize::MAX);
        let mut base = vec![[0.0f32; 4]; n];
        let mut layers: Vec<Layer> = Vec::new();
        let mut clips: Vec<Vec<f32>> = Vec::new();
        let ones = vec![1.0f32; n];
        for command in commands {
            let clip = clips.last().unwrap_or(&ones).clone();
            let object_opacity: f32 = if self.per_object_opacity {
                layers.iter().map(|l| l.opacity).product()
            } else {
                1.0
            };
            let target = match layers.last_mut() {
                Some(l) => &mut l.buf,
                None => &mut base,
            };
            let mut paint_cov = |cov: &[f32], paint: &Paint| {
                let c = paint.color;
                for i in 0..n {
                    let a = c.a * cov[i] * clip[i] * object_opacity;
                    composite(
                        &mut target[i],
                        [c.r, c.g, c.b],
                        a,
                        paint.blend,
                        self.naive_blend,
                    );
                }
            };
            match command {
                DrawCommand::Fill {
                    path,
                    rule,
                    transform,
                    paint,
                } => {
                    let cov = self.shape(path, *rule, transform, w, h, paint.anti_alias);
                    paint_cov(&cov, paint);
                }
                DrawCommand::Stroke {
                    path,
                    style,
                    transform,
                    paint,
                } => {
                    let cov = self.stroke_shape(
                        path,
                        style,
                        transform,
                        (w, h),
                        paint.anti_alias,
                        max_dashes,
                    )?;
                    paint_cov(&cov, paint);
                }
                DrawCommand::Image {
                    image,
                    transform: t,
                    opacity,
                    blend,
                    ..
                } => {
                    let [a, b, c, d, e, f] = [t.a, t.b, t.c, t.d, t.e, t.f].map(f64::from);
                    let det = a * d - b * c;
                    if det == 0.0 || !det.is_finite() {
                        continue;
                    }
                    for y in 0..h {
                        for x in 0..w {
                            let (px, py) = (f64::from(x) + 0.5 - e, f64::from(y) + 0.5 - f);
                            let u = (d * px - c * py) / det;
                            let v = (-b * px + a * py) / det;
                            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                                continue;
                            }
                            let col = to_u32(u * f64::from(image.width)).min(image.width - 1);
                            let row_up = to_u32(v * f64::from(image.height)).min(image.height - 1);
                            let row = if self.flip_image {
                                row_up
                            } else {
                                image.height - 1 - row_up
                            };
                            let k = ((row * image.width + col) * 4) as usize;
                            let px = &image.data[k..k + 4];
                            let i = (y * w + x) as usize;
                            let alpha =
                                f32::from(px[3]) / 255.0 * opacity * clip[i] * object_opacity;
                            let cs = [px[0], px[1], px[2]].map(|v| f32::from(v) / 255.0);
                            composite(&mut target[i], cs, alpha, *blend, self.naive_blend);
                        }
                    }
                }
                DrawCommand::PushClip {
                    path,
                    rule,
                    transform,
                    anti_alias,
                } => {
                    let cov = self.shape(path, *rule, transform, w, h, *anti_alias);
                    clips.push(cov.iter().zip(&clip).map(|(a, b)| a * b).collect());
                }
                DrawCommand::PopClip => {
                    clips.pop();
                }
                DrawCommand::PushLayer {
                    opacity,
                    blend,
                    mask,
                } => layers.push(Layer {
                    buf: vec![[0.0; 4]; n],
                    opacity: *opacity,
                    blend: *blend,
                    mask: mask.map(|m| m.iter().map(|v| f32::from(*v) / 255.0).collect()),
                }),
                DrawCommand::PopLayer => {
                    let Some(layer) = layers.pop() else { continue };
                    let parent = match layers.last_mut() {
                        Some(l) => &mut l.buf,
                        None => &mut base,
                    };
                    let group_opacity = if self.per_object_opacity {
                        1.0
                    } else {
                        layer.opacity
                    };
                    for i in 0..n {
                        let [r, g, b, a] = layer.buf[i];
                        if a <= 0.0 {
                            continue;
                        }
                        let m = layer.mask.as_ref().map_or(1.0, |m| m[i]);
                        composite(
                            &mut parent[i],
                            [r / a, g / a, b / a],
                            a * group_opacity * m * clip[i],
                            layer.blend,
                            self.naive_blend,
                        );
                    }
                }
            }
        }
        let data = base
            .iter()
            .flat_map(|p| {
                let px = if self.straight_output && p[3] > 0.0 {
                    [p[0] / p[3], p[1] / p[3], p[2] / p[3], p[3]]
                } else {
                    *p
                };
                px.map(|v| {
                    u8::try_from(to_u32(f64::from(v.clamp(0.0, 1.0) * 255.0).round()))
                        .unwrap_or(255)
                })
            })
            .collect();
        Ok(Pixmap {
            width,
            height,
            data,
        })
    }
}

fn failed(toy: Toy) -> Vec<&'static str> {
    let report = run(&toy);
    assert!(report.checks_run > 300, "{}", report.checks_run);
    report.failed_checks()
}

#[test]
fn the_toy_passes() {
    let report = run(&Toy::default());
    assert!(report.passed(), "{report}");
}

#[test]
fn ignoring_limits_fails_every_limit_check() {
    assert_eq!(
        failed(Toy {
            ignore_limits: true,
            ..Toy::default()
        }),
        vec![
            "raster.limits.canvas",
            "raster.limits.clip_depth",
            "raster.limits.dashes",
            "raster.limits.layer_depth",
            "raster.limits.segments",
            "raster.limits.working_bytes",
        ]
    );
}

#[test]
fn skipping_validation_fails_invalid() {
    assert_eq!(
        failed(Toy {
            no_validation: true,
            ..Toy::default()
        }),
        vec![
            "raster.invalid",
            "raster.limits.canvas",
            "raster.limits.clip_depth",
            "raster.limits.layer_depth",
            "raster.limits.segments",
            "raster.limits.working_bytes"
        ]
    );
}

/// Each broken variant fails exactly the checks listed (one test each, so they run in
/// parallel).
macro_rules! caught {
    ($($name:ident: $flag:ident => [$($check:literal),*];)*) => {$(
        #[test]
        fn $name() {
            let expected: Vec<&str> = vec![$($check),*];
            assert_eq!(failed(Toy { $flag: true, ..Toy::default() }), expected);
        }
    )*};
}

caught! {
    caught_no_aa: no_aa => ["raster.anti_alias", "raster.clip"];
    caught_per_object_opacity: per_object_opacity => ["raster.layer"];
    caught_naive_blend: naive_blend => ["raster.blend"];
    caught_ignore_miter_limit: ignore_miter_limit => ["raster.stroke"];
    caught_flip_image: flip_image => ["raster.image"];
    caught_straight_output: straight_output => ["raster.fill", "raster.image", "raster.layer", "raster.output"];
    caught_ignore_even_odd: ignore_even_odd => ["raster.clip", "raster.fill_rule"];
    caught_ignore_dash_phase: ignore_dash_phase => ["raster.dash"];
    caught_ignore_clip_depth: ignore_clip_depth => ["raster.limits.clip_depth", "raster.limits.working_bytes"];
}

/// A rasterizer that panics: the suite reports, never propagates.
struct Panicky;

impl Rasterizer for Panicky {
    fn render(
        &self,
        _: u32,
        _: u32,
        _: &[DrawCommand<'_>],
        _: &RasterLimits,
    ) -> Result<Pixmap, BackendError> {
        panic!("boom")
    }
}

#[test]
fn panics_are_failures() {
    let report = run(&Panicky);
    assert!(!report.passed());
    assert!(report.failed("raster.fill"));
    assert!(report.failed("raster.invalid"));
    assert!(report.failed("raster.hostile"));
}

#[test]
fn blend_formulas_match_hand_computed_values() {
    let cb = [0.8, 0.4, 0.2];
    let cs = [0.2, 0.6, 0.9];
    let close =
        |got: [f32; 3], want: [f32; 3]| got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-5);
    assert!(close(
        blend(BlendMode::Multiply, cb, cs),
        [0.16, 0.24, 0.18]
    ));
    assert!(close(blend(BlendMode::Screen, cb, cs), [0.84, 0.76, 0.92]));
    // Overlay = HardLight(Cs, Cb): 0.8 > 0.5 → screen(0.2, 0.6) = 0.68; 0.4 → 0.6·0.8 = 0.48;
    // 0.2 → 0.9·0.4 = 0.36.
    assert!(close(blend(BlendMode::Overlay, cb, cs), [0.68, 0.48, 0.36]));
    assert!(close(blend(BlendMode::Difference, cb, cs), [0.6, 0.2, 0.7]));
    assert!(close(
        blend(BlendMode::Exclusion, cb, cs),
        [0.68, 0.52, 0.74]
    ));
    // ColorDodge: 0.8 / 0.8 = 1; 0.4 / 0.4 = 1; 0.2 / 0.1 → 1.
    assert!(close(blend(BlendMode::ColorDodge, cb, cs), [1.0, 1.0, 1.0]));
    // ColorBurn: 1 − min(1, 0.2/0.2) = 0; 1 − 0.6/0.6 = 0; 1 − min(1, 0.8/0.9).
    assert!(close(
        blend(BlendMode::ColorBurn, cb, cs),
        [0.0, 0.0, 1.0 - 0.8 / 0.9]
    ));
    // SoftLight: Cs 0.2 → 0.8 − 0.6·0.8·0.2 = 0.704; Cs 0.6 → 0.4 + 0.2·(√0.4 − 0.4);
    // Cs 0.9, Cb 0.2 ≤ 0.25 → D = ((3.2 − 12)·0.2 + 4)·0.2 = 0.448 → 0.2 + 0.8·0.248.
    let soft = [0.704, 0.4 + 0.2 * (0.4f32.sqrt() - 0.4), 0.2 + 0.8 * 0.248];
    assert!(close(blend(BlendMode::SoftLight, cb, cs), soft));
    // Luminosity keeps Cb's hue and takes Cs's luminosity: Lum(Cs) = 0.06 + 0.354 + 0.099
    // = 0.513, Lum(Cb) = 0.24 + 0.236 + 0.022 = 0.498, so every component rises by 0.015.
    assert!(close(
        blend(BlendMode::Luminosity, cb, cs),
        [0.815, 0.415, 0.215]
    ));
    // Gray has no hue or saturation: Hue/Saturation/Color of gray keep the luminosity.
    let gray = [0.5, 0.5, 0.5];
    assert!(close(blend(BlendMode::Saturation, gray, cs), gray));
    // Color takes Cb's luminosity: Lum(0.2, 0.6, 0.9) = 0.513.
    assert!(close(blend(BlendMode::Color, cs, gray), [0.513; 3]));
    // ClipColor pulls an out-of-gamut result back to [0, 1] keeping luminosity.
    let lifted = set_lum([0.0, 0.0, 1.0], 0.9);
    assert!(lifted.iter().all(|v| (0.0..=1.0 + 1e-6).contains(v)));
    assert!((lum(lifted) - 0.9).abs() < 1e-5);
}
