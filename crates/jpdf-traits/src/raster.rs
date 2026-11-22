//! Rasterizing a display list into RGBA pixels (ISO 32000-2 §8.4–§8.5 paths and strokes,
//! §8.9.4 image space, §11.3 compositing and blend modes).
//!
//! `jpdf-render` turns a page into a list of [`DrawCommand`]s in device space and hands it to
//! a [`Rasterizer`]. Everything PDF-specific (colour spaces, shadings, text, soft-mask
//! luminosity, knockout groups) is resolved before this boundary, so swapping a backend can
//! only change anti-aliasing and rounding, never what a page shows.

use crate::{BackendError, PathSegment, RasterLimits};

/// An affine transform `[a b c d e f]` in PDF order (ISO 32000-2 §8.3.3): a point `(x, y)`
/// maps to `(a·x + c·y + e, b·x + d·y + f)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// `a`.
    pub a: f32,
    /// `b`.
    pub b: f32,
    /// `c`.
    pub c: f32,
    /// `d`.
    pub d: f32,
    /// `e`.
    pub e: f32,
    /// `f`.
    pub f: f32,
}

impl Transform {
    /// The identity.
    pub const IDENTITY: Self = Self::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    /// A transform from its six entries.
    pub const fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// Whether every entry is finite.
    pub fn is_finite(&self) -> bool {
        [self.a, self.b, self.c, self.d, self.e, self.f]
            .iter()
            .all(|v| v.is_finite())
    }
}

/// Which points a path covers (ISO 32000-2 §8.5.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule {
    /// Nonzero winding number.
    NonZero,
    /// Even-odd.
    EvenOdd,
}

/// A colour in the device's RGB space (already colour-managed by `jpdf-color`), with
/// straight (not premultiplied) alpha. Every component is finite and in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red.
    pub r: f32,
    /// Green.
    pub g: f32,
    /// Blue.
    pub b: f32,
    /// Alpha (PDF constant alpha `CA`/`ca` folded in).
    pub a: f32,
}

impl Color {
    /// A colour from its components.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    fn is_valid(&self) -> bool {
        [self.r, self.g, self.b, self.a]
            .iter()
            .all(|v| (0.0..=1.0).contains(v))
    }
}

/// The blend modes of ISO 32000-2 §11.3.5 (Tables 134 and 135).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendMode {
    /// `Normal` (and `Compatible`).
    Normal,
    /// `Multiply`.
    Multiply,
    /// `Screen`.
    Screen,
    /// `Overlay`.
    Overlay,
    /// `Darken`.
    Darken,
    /// `Lighten`.
    Lighten,
    /// `ColorDodge`.
    ColorDodge,
    /// `ColorBurn`.
    ColorBurn,
    /// `HardLight`.
    HardLight,
    /// `SoftLight`.
    SoftLight,
    /// `Difference`.
    Difference,
    /// `Exclusion`.
    Exclusion,
    /// `Hue` (non-separable).
    Hue,
    /// `Saturation` (non-separable).
    Saturation,
    /// `Color` (non-separable).
    Color,
    /// `Luminosity` (non-separable).
    Luminosity,
}

impl BlendMode {
    /// Every mode, in Table 134/135 order.
    pub const ALL: [Self; 16] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
        Self::ColorDodge,
        Self::ColorBurn,
        Self::HardLight,
        Self::SoftLight,
        Self::Difference,
        Self::Exclusion,
        Self::Hue,
        Self::Saturation,
        Self::Color,
        Self::Luminosity,
    ];
}

/// How a fill or stroke paints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    /// The colour.
    pub color: Color,
    /// How it composites with the backdrop.
    pub blend: BlendMode,
    /// Area coverage at edges when true; pixel-centre sampling when false.
    pub anti_alias: bool,
}

/// Line cap style (ISO 32000-2 §8.4.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    /// Butt cap.
    Butt,
    /// Round cap.
    Round,
    /// Projecting square cap.
    Square,
}

/// Line join style (ISO 32000-2 §8.4.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoin {
    /// Miter join, bevelled beyond the miter limit.
    Miter,
    /// Round join.
    Round,
    /// Bevel join.
    Bevel,
}

/// Stroke parameters (ISO 32000-2 §8.4.3), in user space: the draw command's transform
/// applies to the pen too.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeStyle {
    /// Line width, finite and at least 0. 0 is the thinnest line the device can draw.
    pub width: f32,
    /// Cap at open ends and dashes.
    pub cap: LineCap,
    /// Join between segments.
    pub join: LineJoin,
    /// Miter limit, finite and at least 1 (§8.4.3.5).
    pub miter_limit: f32,
    /// Dash array (§8.4.3.6): empty for a solid line; otherwise finite, non-negative and not
    /// all zero. An odd-length array repeats, so `[3]` means 3 on, 3 off.
    pub dash: Vec<f32>,
    /// Dash phase, finite. A negative phase or one beyond the period is reduced modulo the
    /// period of the expanded, even-length pattern (twice the sum for an odd-length array:
    /// `[4]` has period 8). So -2 with `[4 4]` is 6, and 12 with `[4]` is 4.
    pub dash_phase: f32,
}

impl StrokeStyle {
    /// A solid stroke of `width` with butt caps and miter joins (limit 10, the PDF default).
    pub fn solid(width: f32) -> Self {
        Self {
            width,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 10.0,
            dash: Vec::new(),
            dash_phase: 0.0,
        }
    }
}

/// An image to draw: 8-bit RGBA with straight alpha, rows top to bottom, no padding. Colour
/// conversion, `Decode` and soft masks are already applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaImage<'a> {
    /// Width in pixels, at least 1.
    pub width: u32,
    /// Height in pixels, at least 1.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub data: &'a [u8],
}

/// One display-list command. Paths are in user space and use [`PathSegment`]s: each subpath
/// starts with `MoveTo` and may end with `Close`; after a `Close` the next segment must be a
/// `MoveTo`. Fills close open subpaths implicitly; strokes leave them open.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand<'a> {
    /// Fills a path.
    Fill {
        /// The path.
        path: &'a [PathSegment],
        /// Fill rule.
        rule: FillRule,
        /// User space to device space.
        transform: Transform,
        /// Paint.
        paint: Paint,
    },
    /// Strokes a path.
    Stroke {
        /// The path.
        path: &'a [PathSegment],
        /// Pen.
        style: &'a StrokeStyle,
        /// User space to device space (applies to the pen as well).
        transform: Transform,
        /// Paint.
        paint: Paint,
    },
    /// Draws an image into the parallelogram the transform makes of the unit square. As in
    /// ISO 32000-2 §8.9.4, the image's first row maps to `v = 1`: the top-left pixel is at
    /// image-space `(0, 1)`.
    Image {
        /// The pixels.
        image: RgbaImage<'a>,
        /// Unit square to device space.
        transform: Transform,
        /// Smooth when scaling (`/Interpolate`); nearest neighbour when false. Either way a
        /// 1:1, pixel-aligned draw reproduces the source pixels, so the smoothing filter
        /// must interpolate (bilinear, or an interpolating bicubic such as Catmull-Rom; not
        /// a blurring one such as Mitchell).
        interpolate: bool,
        /// Multiplies the image's alpha; finite, in `0.0..=1.0`.
        opacity: f32,
        /// How it composites with the backdrop.
        blend: BlendMode,
    },
    /// Intersects the clip with a path until the matching [`DrawCommand::PopClip`].
    PushClip {
        /// The path.
        path: &'a [PathSegment],
        /// Fill rule (`W` or `W*`).
        rule: FillRule,
        /// User space to device space.
        transform: Transform,
        /// Soft clip edges when true; pixel-centre sampling when false.
        anti_alias: bool,
    },
    /// Ends the innermost clip.
    PopClip,
    /// Starts an isolated, non-knockout transparency group (§11.4.7): following commands
    /// draw into a transparent buffer until the matching [`DrawCommand::PopLayer`], which
    /// composites it onto the backdrop.
    PushLayer {
        /// Group opacity; finite, in `0.0..=1.0`.
        opacity: f32,
        /// How the group composites with the backdrop.
        blend: BlendMode,
        /// Optional soft mask, one byte per canvas pixel (rows top to bottom), already
        /// computed by the caller (§11.6.5); multiplies the group's alpha.
        mask: Option<&'a [u8]>,
    },
    /// Ends the innermost layer.
    PopLayer,
}

/// The rendered canvas: 8-bit RGBA, **premultiplied** alpha, rows top to bottom, no padding.
/// Every colour byte is at most its pixel's alpha byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixmap {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub data: Vec<u8>,
}

impl Pixmap {
    /// The premultiplied RGBA of pixel `(x, y)`, or `None` outside the canvas.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y as usize)
            .checked_mul(self.width as usize)?
            .checked_add(x as usize)?
            .checked_mul(4)?;
        self.data.get(index..index.checked_add(4)?)?.try_into().ok()
    }
}

/// What [`check_scene`] measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneStats {
    /// Deepest clip nesting.
    pub clip_depth: u16,
    /// Deepest layer nesting.
    pub layer_depth: u16,
    /// Nominal working memory: `pixels × (4 + S) + 4 × I`. `S` is the most bytes per pixel
    /// the clip/layer stack holds at any point: 1 per clip (an 8-bit mask), 4 per layer (an
    /// RGBA buffer), 5 per layer with a soft mask (plus the adapter's copy of the mask). `I`
    /// is the largest image's pixel count (the adapter's premultiplied copy).
    pub working_bytes: u64,
}

/// Turns a display list into pixels. Implementations are stateless and shared across
/// rendering threads.
pub trait Rasterizer: Send + Sync {
    /// Renders `commands` onto a transparent `width × height` canvas. Device space has its
    /// origin at the top-left corner, x to the right and y down; pixel `(i, j)` covers
    /// `[i, i+1) × [j, j+1)`. Compositing is the §11.3.6 source-over formula with the
    /// command's blend mode, in the device's encoded RGB values.
    ///
    /// Before drawing, an implementation validates the whole list and enforces `limits`
    /// ([`check_scene`] does both). Invalid input gives
    /// [`BackendError::InvalidArgument`]; a limit gives [`BackendError::LimitExceeded`],
    /// including a dashed stroke that would produce more than `max_path_segments` dashes.
    /// That count covers the **whole** path, on and off the canvas: compute it from the
    /// path length and the dash period before generating dashes, so it is known up front.
    ///
    /// Geometry must stay bounded and exact for any finite input, a hang being a failure
    /// even though no error type describes it:
    /// - flatten curves after clipping to the canvas, or with a capped subdivision;
    /// - clip fills and clips geometrically before a backend with a coordinate range
    ///   (tiny-skia skips paths beyond about ±2^29) sees them, so a ±1e30 rect still
    ///   covers the canvas;
    /// - a stroke wider than the canvas diagonal still paints its full outline (fill the
    ///   clipped outline instead of handing the backend the width);
    /// - device coordinates can overflow to infinity after transforming even though the
    ///   list is finite: treat such a draw as empty or as `LimitExceeded`, never as a panic.
    ///
    /// Stroke adjustment (§10.7.5) is out of scope: `jpdf-render` snaps hairlines itself.
    fn render(
        &self,
        width: u32,
        height: u32,
        commands: &[DrawCommand<'_>],
        limits: &RasterLimits,
    ) -> Result<Pixmap, BackendError>;
}

/// Validates a display list and checks it against `limits`, without drawing. Adapters call
/// this first. Errors:
/// - `InvalidArgument`: a zero-size canvas; a non-finite number anywhere; a colour, opacity
///   or mask outside its range; a negative stroke width; a miter limit below 1; a negative
///   or all-zero dash array; an empty image or one whose data length isn't
///   `width × height × 4`; a mask whose length isn't the canvas's pixel count; a malformed
///   path; a pop that doesn't match the innermost push; a push left open at the end.
/// - `LimitExceeded`: canvas pixels, segments in one path, clip or layer depth, or
///   [`SceneStats::working_bytes`] above the limit.
///
/// Time is not bounded here: a list of a million full-canvas fills passes. `render` can't
/// be cancelled, so `jpdf-render` must keep each call within the job budget (PLAN §2.2),
/// for example by capping the commands per call (#73).
pub fn check_scene(
    width: u32,
    height: u32,
    commands: &[DrawCommand<'_>],
    limits: &RasterLimits,
) -> Result<SceneStats, BackendError> {
    use BackendError::{InvalidArgument, LimitExceeded};
    if width == 0 || height == 0 {
        return Err(InvalidArgument("zero-size canvas"));
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > limits.max_canvas_pixels {
        return Err(LimitExceeded("canvas pixels"));
    }
    let check_path = |path: &[PathSegment]| -> Result<(), BackendError> {
        if u64::try_from(path.len()).unwrap_or(u64::MAX) > u64::from(limits.max_path_segments) {
            return Err(LimitExceeded("path segments"));
        }
        if !path_is_valid(path) {
            return Err(InvalidArgument("malformed path"));
        }
        Ok(())
    };
    let check_transform = |t: &Transform| {
        if t.is_finite() {
            Ok(())
        } else {
            Err(InvalidArgument("non-finite transform"))
        }
    };
    let unit = |v: f32| (0.0..=1.0).contains(&v);
    // (true = clip / false = layer, bytes per canvas pixel it holds).
    let mut stack: Vec<(bool, u64)> = Vec::new();
    let (mut clips, mut layers) = (0u16, 0u16);
    let (mut max_clips, mut max_layers) = (0u16, 0u16);
    let (mut stack_bytes, mut max_stack_bytes) = (0u64, 0u64);
    let mut max_image_pixels = 0u64;
    for command in commands {
        match command {
            DrawCommand::Fill {
                path,
                transform,
                paint,
                ..
            } => {
                check_path(path)?;
                check_transform(transform)?;
                if !paint.color.is_valid() {
                    return Err(InvalidArgument("colour out of range"));
                }
            }
            DrawCommand::Stroke {
                path,
                style,
                transform,
                paint,
            } => {
                check_path(path)?;
                check_transform(transform)?;
                if !paint.color.is_valid() {
                    return Err(InvalidArgument("colour out of range"));
                }
                check_stroke(style)?;
            }
            DrawCommand::Image {
                image,
                transform,
                opacity,
                ..
            } => {
                check_transform(transform)?;
                if !unit(*opacity) {
                    return Err(InvalidArgument("opacity out of range"));
                }
                // u32 × u32 fits a u64; × 4 may not (checked).
                let image_pixels = u64::from(image.width) * u64::from(image.height);
                let expected = image_pixels.checked_mul(4);
                if image_pixels == 0
                    || expected.is_none()
                    || u64::try_from(image.data.len()).ok() != expected
                {
                    return Err(InvalidArgument("image size"));
                }
                max_image_pixels = max_image_pixels.max(image_pixels);
            }
            DrawCommand::PushClip {
                path, transform, ..
            } => {
                check_path(path)?;
                check_transform(transform)?;
                clips = clips
                    .checked_add(1)
                    .filter(|d| *d <= limits.max_clip_depth)
                    .ok_or(LimitExceeded("clip depth"))?;
                max_clips = max_clips.max(clips);
                stack.push((true, 1));
                stack_bytes = stack_bytes.saturating_add(1);
                max_stack_bytes = max_stack_bytes.max(stack_bytes);
            }
            DrawCommand::PushLayer { opacity, mask, .. } => {
                if !unit(*opacity) {
                    return Err(InvalidArgument("opacity out of range"));
                }
                if let Some(mask) = mask
                    && u64::try_from(mask.len()).ok() != Some(pixels)
                {
                    return Err(InvalidArgument("mask size"));
                }
                layers = layers
                    .checked_add(1)
                    .filter(|d| *d <= limits.max_layer_depth)
                    .ok_or(LimitExceeded("layer depth"))?;
                max_layers = max_layers.max(layers);
                // An RGBA buffer, plus the adapter's copy of the mask.
                let cost = if mask.is_some() { 5 } else { 4 };
                stack.push((false, cost));
                stack_bytes = stack_bytes.saturating_add(cost);
                max_stack_bytes = max_stack_bytes.max(stack_bytes);
            }
            DrawCommand::PopClip => {
                let Some((true, cost)) = stack.pop() else {
                    return Err(InvalidArgument("unmatched PopClip"));
                };
                clips = clips.saturating_sub(1);
                stack_bytes = stack_bytes.saturating_sub(cost);
            }
            DrawCommand::PopLayer => {
                let Some((false, cost)) = stack.pop() else {
                    return Err(InvalidArgument("unmatched PopLayer"));
                };
                layers = layers.saturating_sub(1);
                stack_bytes = stack_bytes.saturating_sub(cost);
            }
        }
    }
    if !stack.is_empty() {
        return Err(InvalidArgument("unclosed clip or layer"));
    }
    let working_bytes = max_stack_bytes
        .checked_add(4)
        .and_then(|per_pixel| pixels.checked_mul(per_pixel))
        .and_then(|canvas| canvas.checked_add(max_image_pixels.checked_mul(4)?))
        .unwrap_or(u64::MAX);
    if working_bytes > limits.max_working_bytes {
        return Err(LimitExceeded("working bytes"));
    }
    Ok(SceneStats {
        clip_depth: max_clips,
        layer_depth: max_layers,
        working_bytes,
    })
}

fn check_stroke(style: &StrokeStyle) -> Result<(), BackendError> {
    use BackendError::InvalidArgument;
    if !(style.width.is_finite() && style.width >= 0.0) {
        return Err(InvalidArgument("stroke width"));
    }
    if !(style.miter_limit.is_finite() && style.miter_limit >= 1.0) {
        return Err(InvalidArgument("miter limit"));
    }
    if !style.dash_phase.is_finite()
        || style.dash.iter().any(|d| !(d.is_finite() && *d >= 0.0))
        || (!style.dash.is_empty() && style.dash.iter().all(|d| *d == 0.0))
    {
        return Err(InvalidArgument("dash array"));
    }
    Ok(())
}

/// Whether every subpath starts with `MoveTo`, nothing but `MoveTo` follows a `Close`, and
/// every coordinate is finite. An empty path is valid (it draws nothing).
pub fn path_is_valid(path: &[PathSegment]) -> bool {
    let mut open = false;
    for segment in path {
        if !segment.points().all(|p| p.x.is_finite() && p.y.is_finite()) {
            return false;
        }
        match segment {
            PathSegment::MoveTo(_) => open = true,
            PathSegment::Close if open => open = false,
            _ if open => {}
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;

    fn p(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    #[test]
    fn paths_must_start_subpaths_with_move_to() {
        use PathSegment::*;
        assert!(path_is_valid(&[]));
        assert!(path_is_valid(&[MoveTo(p(0.0, 0.0)), LineTo(p(1.0, 0.0))]));
        assert!(path_is_valid(&[
            MoveTo(p(0.0, 0.0)),
            LineTo(p(1.0, 0.0)),
            Close,
            MoveTo(p(2.0, 2.0)),
            MoveTo(p(3.0, 3.0)),
        ]));
        assert!(!path_is_valid(&[LineTo(p(1.0, 0.0))]));
        assert!(!path_is_valid(&[Close]));
        assert!(!path_is_valid(&[MoveTo(p(0.0, 0.0)), Close, Close]));
        assert!(!path_is_valid(&[
            MoveTo(p(0.0, 0.0)),
            Close,
            LineTo(p(1.0, 1.0))
        ]));
        assert!(!path_is_valid(&[MoveTo(p(f32::NAN, 0.0))]));
        assert!(!path_is_valid(&[
            MoveTo(p(0.0, 0.0)),
            CurveTo(p(0.0, 0.0), p(f32::INFINITY, 0.0), p(1.0, 1.0)),
        ]));
    }

    /// The lowered default canvas (#109, specs/core/memory-budget.md) at its exact boundary:
    /// 2^23 pixels pass with `RasterLimits::default()`, one pixel more fails.
    #[test]
    fn default_canvas_limit_is_exact() {
        let l = RasterLimits::default();
        assert!(check_scene(4096, 2048, &[], &l).is_ok());
        assert!(check_scene(8_388_608, 1, &[], &l).is_ok());
        assert_eq!(
            check_scene(8_388_609, 1, &[], &l),
            Err(BackendError::LimitExceeded("canvas pixels"))
        );
        assert_eq!(
            check_scene(4097, 2048, &[], &l),
            Err(BackendError::LimitExceeded("canvas pixels"))
        );
    }

    #[test]
    fn working_bytes_follow_the_formula() {
        let path = [PathSegment::MoveTo(p(0.0, 0.0))];
        let clip = DrawCommand::PushClip {
            path: &path,
            rule: FillRule::NonZero,
            transform: Transform::IDENTITY,
            anti_alias: true,
        };
        let layer = DrawCommand::PushLayer {
            opacity: 1.0,
            blend: BlendMode::Normal,
            mask: None,
        };
        let commands = [
            layer.clone(),
            clip.clone(),
            clip,
            DrawCommand::PopClip,
            DrawCommand::PopClip,
            DrawCommand::PopLayer,
            layer,
            DrawCommand::PopLayer,
        ];
        let stats = check_scene(10, 3, &commands, &RasterLimits::default());
        assert_eq!(
            stats,
            Ok(SceneStats {
                clip_depth: 2,
                layer_depth: 1,
                working_bytes: 30 * (4 + 4 + 2),
            })
        );
        let tight = RasterLimits {
            max_working_bytes: 299,
            ..RasterLimits::default()
        };
        assert_eq!(
            check_scene(10, 3, &commands, &tight),
            Err(BackendError::LimitExceeded("working bytes"))
        );
    }

    #[test]
    fn masks_and_images_count_towards_working_bytes() {
        let mask = [0u8; 30];
        let pixels = [0u8; 4 * 7];
        let commands = [
            DrawCommand::PushLayer {
                opacity: 1.0,
                blend: BlendMode::Normal,
                mask: Some(&mask),
            },
            DrawCommand::PopLayer,
            // Sequential layers don't add up: the stack peaks at one masked layer.
            DrawCommand::PushLayer {
                opacity: 1.0,
                blend: BlendMode::Normal,
                mask: None,
            },
            DrawCommand::PopLayer,
            DrawCommand::Image {
                image: RgbaImage {
                    width: 7,
                    height: 1,
                    data: &pixels,
                },
                transform: Transform::IDENTITY,
                interpolate: false,
                opacity: 1.0,
                blend: BlendMode::Normal,
            },
        ];
        let stats = check_scene(10, 3, &commands, &RasterLimits::default());
        assert_eq!(stats.map(|s| s.working_bytes), Ok(30 * (4 + 5) + 4 * 7));
    }

    #[test]
    fn huge_image_dimensions_are_invalid_not_a_panic() {
        for (width, height) in [(u32::MAX, u32::MAX), (1 << 31, 1 << 31), (u32::MAX, 0)] {
            let commands = [DrawCommand::Image {
                image: RgbaImage {
                    width,
                    height,
                    data: &[],
                },
                transform: Transform::IDENTITY,
                interpolate: false,
                opacity: 1.0,
                blend: BlendMode::Normal,
            }];
            assert_eq!(
                check_scene(4, 4, &commands, &RasterLimits::default()),
                Err(BackendError::InvalidArgument("image size"))
            );
        }
    }

    #[test]
    fn depth_counters_never_saturate_silently() {
        let path = [PathSegment::MoveTo(p(0.0, 0.0))];
        let clip = DrawCommand::PushClip {
            path: &path,
            rule: FillRule::NonZero,
            transform: Transform::IDENTITY,
            anti_alias: false,
        };
        let open = RasterLimits {
            max_clip_depth: u16::MAX,
            max_working_bytes: u64::MAX,
            ..RasterLimits::default()
        };
        let deep = vec![clip; usize::from(u16::MAX) + 1];
        assert_eq!(
            check_scene(1, 1, &deep, &open),
            Err(BackendError::LimitExceeded("clip depth"))
        );
    }

    #[test]
    fn pixel_reads_are_bounds_checked() {
        let pixmap = Pixmap {
            width: 2,
            height: 1,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        assert_eq!(pixmap.pixel(1, 0), Some([5, 6, 7, 8]));
        assert_eq!(pixmap.pixel(2, 0), None);
        assert_eq!(pixmap.pixel(0, 1), None);
        let short = Pixmap {
            width: 2,
            height: 1,
            data: vec![0; 5],
        };
        assert_eq!(short.pixel(1, 0), None);
    }
}
