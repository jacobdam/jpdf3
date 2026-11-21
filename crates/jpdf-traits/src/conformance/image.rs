//! Conformance suites for the image decoders (ISO 32000-2 §7.4.6–§7.4.9).
//!
//! jpdf-traits has no encoders for these formats, so the adapter supplies fixtures: small
//! files we generated ourselves (never corpus files, CONSTITUTION 18) with the pixels a
//! reference decoder produced. The suite adds format-independent checks on top: empty and
//! truncated input, mutated bytes, and every limit at its exact boundary. It also requires
//! fixtures that make each decode option matter, and checks that flipping the option changes
//! the result, so an adapter that ignores an option fails.

use super::{Boundary, Report, describe, guarded, narrow, sample_offsets};
use crate::{
    BackendError, Bitmap, CcittDecoder, CcittParams, DecodeLimits, DecodeStatus, Decoded,
    DecodedImage, ImageColor, Jbig2Decoder, Jpeg2000Decoder, JpegDecoder, JpegOptions,
};

/// A cut of a fixture's input that the decoder must salvage (CONSTITUTION 17: read
/// leniently). Decoding `input[..cut]` must return `Ok` with [`DecodeStatus::Truncated`], the
/// fixture's width and header, a height from `rows` to the full height, and its first `rows`
/// rows equal to the expected image (within the fixture's tolerance).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Salvage {
    /// Input length to keep; below the input length.
    pub cut: usize,
    /// Rows from the top that must decode; at least 1 and at most the height.
    pub rows: u32,
}

/// A JPEG file and the image it decodes to.
#[derive(Debug, Clone)]
pub struct JpegFixture {
    /// Name for failure messages.
    pub name: String,
    /// The JPEG codestream.
    pub input: Vec<u8>,
    /// DCTDecode options to decode with.
    pub options: JpegOptions,
    /// The expected image.
    pub expected: DecodedImage,
    /// Largest allowed difference per sample (JPEG decoders may round IDCT output
    /// differently). At most [`max_lossy_tolerance`] of the image's `source_bits`; the mean
    /// difference must also stay within half of it.
    pub tolerance: u16,
    /// A cut the decoder must salvage, if any. At least one JPEG fixture needs one.
    pub salvage: Option<Salvage>,
}

/// A JPEG 2000 file and the image it decodes to.
#[derive(Debug, Clone)]
pub struct Jpeg2000Fixture {
    /// Name for failure messages.
    pub name: String,
    /// The JP2 file or codestream.
    pub input: Vec<u8>,
    /// The expected image.
    pub expected: DecodedImage,
    /// Whether the codestream uses the irreversible (lossy) wavelet. Lossless fixtures must
    /// have tolerance 0.
    pub lossy: bool,
    /// Largest allowed difference per sample: 0 for lossless fixtures, at most
    /// [`max_lossy_tolerance`] for lossy ones.
    pub tolerance: u16,
    /// A cut the decoder must salvage, if any. At least one JPEG 2000 fixture needs one.
    pub salvage: Option<Salvage>,
}

/// A JBIG2 embedded stream and the bitmap it decodes to (exact match).
#[derive(Debug, Clone)]
pub struct Jbig2Fixture {
    /// Name for failure messages.
    pub name: String,
    /// The embedded stream.
    pub input: Vec<u8>,
    /// The decoded `/JBIG2Globals` stream, if any. At least one fixture needs globals that
    /// the page depends on: decoding without them must not give the expected bitmap.
    pub globals: Option<Vec<u8>>,
    /// The expected bitmap.
    pub expected: Bitmap,
}

/// A CCITT stream and the bitmap it decodes to (exact match).
#[derive(Debug, Clone)]
pub struct CcittFixture {
    /// Name for failure messages.
    pub name: String,
    /// The encoded data.
    pub input: Vec<u8>,
    /// The decode parameters.
    pub params: CcittParams,
    /// The expected bitmap.
    pub expected: Bitmap,
    /// A cut the decoder must salvage, if any. At least one CCITT fixture needs one, and its
    /// `params.rows` must be set (with `Rows` 0 a cut can be a valid end).
    pub salvage: Option<Salvage>,
}

/// The largest per-sample tolerance a lossy fixture may have: `2^(source_bits - 7)`, so 2 for
/// 8-bit and 32 for 12-bit samples (1 below 8 bits). A larger tolerance would let a decoder
/// that returns a flat or wrong image pass.
pub fn max_lossy_tolerance(source_bits: u8) -> u16 {
    1u16 << source_bits.saturating_sub(7).min(15)
}

/// Runs every check against a [`JpegDecoder`]. At least one fixture is required.
pub fn run_jpeg(decoder: &dyn JpegDecoder, fixtures: &[JpegFixture]) -> Report {
    let mut report = Report::new("jpeg");
    for f in fixtures {
        let decode =
            |input: &[u8], limits: &DecodeLimits| decoder.decode(input, &f.options, limits);
        let spec = FixtureSpec {
            name: &f.name,
            input: &f.input,
            expected: &f.expected,
            tolerance: f.tolerance,
            max_tolerance: max_lossy_tolerance(f.expected.source_bits),
            salvage: f.salvage,
        };
        let valid = run_fixture(&mut report, &spec, &decode);

        // ColorTransform must be honoured: decoding with the opposite explicit value must not
        // give the expected image. (With 3 or 4 components the two readings differ: YCbCr or
        // YCCK values as stored versus converted.)
        if valid
            && let Some(transform) = f.options.color_transform
            && f.expected.components >= 3
        {
            let flipped = JpegOptions {
                color_transform: Some(!transform),
            };
            let result = guarded(|| decoder.decode(&f.input, &flipped, &DecodeLimits::default()));
            check_option_used(&mut report, &spec, "ColorTransform flipped", &result);
        }
    }
    let with_transform = |value: bool| {
        fixtures
            .iter()
            .any(|f| f.options.color_transform == Some(value) && f.expected.components >= 3)
    };
    report.check(
        "image.option_coverage",
        with_transform(true) && with_transform(false),
        || "fixtures must include 3- or 4-component images decoded with ColorTransform true and with ColorTransform false".into(),
    );
    report.check(
        "image.option_coverage",
        fixtures
            .iter()
            .any(|f| f.expected.color == ImageColor::Cmyk && f.expected.adobe_app14.is_some()),
        || "fixtures must include a CMYK image with an Adobe APP14 marker".into(),
    );
    salvage_coverage(&mut report, fixtures.iter().map(|f| f.salvage));
    let options = JpegOptions::default();
    let dims: Vec<(u32, u32)> = fixtures
        .iter()
        .map(|f| (f.expected.width, f.expected.height))
        .collect();
    common(&mut report, &dims, &|input, limits| {
        decoder.decode(input, &options, limits)
    });
    report
}

/// Runs every check against a [`Jpeg2000Decoder`]. At least one fixture is required.
pub fn run_jpeg2000(decoder: &dyn Jpeg2000Decoder, fixtures: &[Jpeg2000Fixture]) -> Report {
    let mut report = Report::new("jpeg2000");
    let decode = |input: &[u8], limits: &DecodeLimits| decoder.decode(input, limits);
    // JPEG 2000 samples are returned unscaled; a fixture below 8 bits catches an adapter
    // that rescales them (which breaks `/Indexed` overrides).
    report.check(
        "image.jpx_low_depth_fixture",
        fixtures.iter().any(|f| f.expected.source_bits < 8),
        || "fixtures must include an image with source_bits below 8".into(),
    );
    for f in fixtures {
        let spec = FixtureSpec {
            name: &f.name,
            input: &f.input,
            expected: &f.expected,
            tolerance: f.tolerance,
            max_tolerance: if f.lossy {
                max_lossy_tolerance(f.expected.source_bits)
            } else {
                0
            },
            salvage: f.salvage,
        };
        run_fixture(&mut report, &spec, &decode);
    }
    salvage_coverage(&mut report, fixtures.iter().map(|f| f.salvage));
    let dims: Vec<(u32, u32)> = fixtures
        .iter()
        .map(|f| (f.expected.width, f.expected.height))
        .collect();
    common(&mut report, &dims, &decode);
    report
}

/// Runs every check against a [`Jbig2Decoder`]. At least one fixture is required.
pub fn run_jbig2(decoder: &dyn Jbig2Decoder, fixtures: &[Jbig2Fixture]) -> Report {
    let mut report = Report::new("jbig2");
    for f in fixtures {
        let decode = |input: &[u8], limits: &DecodeLimits| {
            decoder.decode(input, f.globals.as_deref(), limits)
        };
        let spec = FixtureSpec {
            name: &f.name,
            input: &f.input,
            expected: &f.expected,
            tolerance: 0,
            max_tolerance: 0,
            salvage: None,
        };
        let valid = run_fixture(&mut report, &spec, &decode);
        // Damaged globals must be as safe as a damaged page stream.
        if let Some(globals) = &f.globals {
            let unlimited = DecodeLimits::default();
            for cut in sample_offsets(globals.len(), 64) {
                let prefix = globals.get(..cut).unwrap_or(&[]);
                let result = guarded(|| decoder.decode(&f.input, Some(prefix), &unlimited));
                report.check(
                    "image.truncated_globals",
                    ok_result_valid(&result, &unlimited),
                    || {
                        format!(
                            "{}: globals cut at {cut}: {}",
                            f.name,
                            describe_raster(&result)
                        )
                    },
                );
            }
            // The page must depend on its globals (symbol dictionaries, patterns), so an
            // adapter that drops them fails.
            if !valid {
                continue;
            }
            let result = guarded(|| decoder.decode(&f.input, None, &unlimited));
            check_option_used(&mut report, &spec, "globals left out", &result);
        }
    }
    report.check(
        "image.option_coverage",
        fixtures.iter().any(|f| f.globals.is_some()),
        || "fixtures must include a page that uses JBIG2Globals".into(),
    );
    let dims: Vec<(u32, u32)> = fixtures
        .iter()
        .map(|f| (f.expected.width, f.expected.height))
        .collect();
    common(&mut report, &dims, &|input, limits| {
        decoder.decode(input, None, limits)
    });
    report
}

/// Runs every check against a [`CcittDecoder`]. At least one fixture is required.
pub fn run_ccitt(decoder: &dyn CcittDecoder, fixtures: &[CcittFixture]) -> Report {
    let mut report = Report::new("ccitt");
    for f in fixtures {
        let decode = |input: &[u8], limits: &DecodeLimits| decoder.decode(input, &f.params, limits);
        let spec = FixtureSpec {
            name: &f.name,
            input: &f.input,
            expected: &f.expected,
            tolerance: 0,
            max_tolerance: 0,
            salvage: f.salvage,
        };
        if f.salvage.is_some() && f.params.rows == 0 {
            report.check("image.fixture_valid", false, || {
                format!("{}: a salvage cut needs Rows set", f.name)
            });
        }
        let valid = run_fixture(&mut report, &spec, &decode);
        ccitt_params(&mut report, decoder, f);
        if !valid {
            continue;
        }

        // K and EncodedByteAlign must be honoured: decoding with another coding scheme (or
        // without the alignment) must not give the expected bitmap.
        let other_k = if f.params.k == 0 { -1 } else { 0 };
        let result = guarded(|| {
            let params = CcittParams {
                k: other_k,
                ..f.params
            };
            decoder.decode(&f.input, &params, &DecodeLimits::default())
        });
        check_option_used(&mut report, &spec, &format!("K {other_k}"), &result);
        if f.params.encoded_byte_align {
            let result = guarded(|| {
                let params = CcittParams {
                    encoded_byte_align: false,
                    ..f.params
                };
                decoder.decode(&f.input, &params, &DecodeLimits::default())
            });
            check_option_used(&mut report, &spec, "EncodedByteAlign false", &result);
        }
    }
    for (scheme, has) in [
        ("K < 0 (Group 4)", fixtures.iter().any(|f| f.params.k < 0)),
        (
            "K = 0 (Group 3 1-D)",
            fixtures.iter().any(|f| f.params.k == 0),
        ),
        (
            "K > 0 (Group 3 2-D)",
            fixtures.iter().any(|f| f.params.k > 0),
        ),
        (
            "EncodedByteAlign",
            fixtures.iter().any(|f| f.params.encoded_byte_align),
        ),
    ] {
        report.check("image.option_coverage", has, || {
            format!("fixtures must include a stream with {scheme}")
        });
    }
    salvage_coverage(&mut report, fixtures.iter().map(|f| f.salvage));
    let params = CcittParams::default();
    let dims: Vec<(u32, u32)> = fixtures
        .iter()
        .map(|f| (f.expected.width, f.expected.height))
        .collect();
    common(&mut report, &dims, &|input, limits| {
        decoder.decode(input, &params, limits)
    });
    report
}

/// CCITT parameter checks for one fixture: `Columns` 0, the size limits on `Columns` and `Rows`
/// before any data is read, and extreme values that must not panic.
fn ccitt_params(report: &mut Report, decoder: &dyn CcittDecoder, f: &CcittFixture) {
    let name = &f.name;
    let unlimited = DecodeLimits::default();
    let zero = CcittParams {
        columns: 0,
        ..f.params
    };
    let result = guarded(|| decoder.decode(&f.input, &zero, &unlimited));
    report.check(
        "image.invalid_argument",
        matches!(result, Some(Err(BackendError::InvalidArgument(_)))),
        || {
            format!(
                "{name}: Columns 0: expected InvalidArgument, got {}",
                describe(&result)
            )
        },
    );

    // The parameters give the size before any data, so the limits apply with empty input:
    // one past a limit must be LimitExceeded, at the limit anything but LimitExceeded (the
    // empty data is the error then). An adapter that checks only the decoded output fails.
    let width = f.expected.width;
    let height = f.expected.height;
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    let sized = CcittParams {
        columns: width,
        rows: height,
        ..f.params
    };
    let probes: [(&str, CcittParams, DecodeLimits, bool); 6] = [
        (
            "Columns at max_image_dimension",
            sized,
            DecodeLimits {
                max_image_dimension: width.max(height),
                ..unlimited
            },
            false,
        ),
        (
            "Columns one past max_image_dimension",
            CcittParams { rows: 0, ..sized },
            DecodeLimits {
                max_image_dimension: width.saturating_sub(1),
                ..unlimited
            },
            true,
        ),
        (
            "Rows one past max_image_dimension",
            sized,
            DecodeLimits {
                max_image_dimension: height.saturating_sub(1).max(width),
                ..unlimited
            },
            height > width,
        ),
        (
            "Columns × Rows at max_image_pixels",
            sized,
            DecodeLimits {
                max_image_pixels: pixels,
                ..unlimited
            },
            false,
        ),
        (
            "Columns × Rows one past max_image_pixels",
            sized,
            DecodeLimits {
                max_image_pixels: pixels.saturating_sub(1),
                ..unlimited
            },
            true,
        ),
        (
            "Columns u32::MAX",
            CcittParams {
                columns: u32::MAX,
                rows: 0,
                ..f.params
            },
            unlimited,
            true,
        ),
    ];
    for (label, params, limits, want_limit) in probes {
        let result = guarded(|| decoder.decode(&[], &params, &limits));
        let ok = match &result {
            Some(Err(BackendError::LimitExceeded(_))) => want_limit,
            Some(_) => !want_limit,
            None => false,
        };
        report.check("image.ccitt_param_limits", ok, || {
            format!(
                "{name}: {label}, empty data: expected {}, got {}",
                if want_limit {
                    "LimitExceeded"
                } else {
                    "no LimitExceeded"
                },
                describe_raster(&result)
            )
        });
    }

    // Extreme values must not panic or overflow; any output stays valid.
    let capped = DecodeLimits {
        max_output_bytes: f.expected.expected_len().unwrap_or(0).saturating_mul(2),
        ..unlimited
    };
    for (label, params) in [
        (
            "K i32::MIN",
            CcittParams {
                k: i32::MIN,
                ..f.params
            },
        ),
        (
            "K i32::MAX",
            CcittParams {
                k: i32::MAX,
                ..f.params
            },
        ),
        (
            "DamagedRowsBeforeError u32::MAX with EndOfLine",
            CcittParams {
                damaged_rows_before_error: u32::MAX,
                end_of_line: true,
                ..f.params
            },
        ),
        (
            "Rows at max_image_dimension",
            CcittParams {
                rows: capped.max_image_dimension,
                ..f.params
            },
        ),
    ] {
        let result = guarded(|| decoder.decode(&f.input, &params, &capped));
        report.check(
            "image.ccitt_extreme_params",
            ok_result_valid(&result, &capped),
            || format!("{name}: {label}: {}", describe_raster(&result)),
        );
    }
}

/// At least one fixture must carry a salvage cut.
fn salvage_coverage(report: &mut Report, mut salvages: impl Iterator<Item = Option<Salvage>>) {
    report.check(
        "image.salvage_fixture",
        salvages.any(|s| s.is_some()),
        || "fixtures must include one with a salvage cut (a truncated input that decodes Ok(Truncated))".into(),
    );
}

/// An option flipped (or left out) must not reproduce the expected output. An error or a
/// different image passes.
fn check_option_used<R: Raster>(
    report: &mut Report,
    spec: &FixtureSpec<'_, R>,
    label: &str,
    result: &Outcome<R>,
) {
    let same = matches!(result, Some(Ok(d)) if d.value.matches(spec.expected, spec.tolerance));
    report.check("image.option_used", result.is_some() && !same, || {
        format!(
            "{}: {label}: the result must differ from the expected image (an ignored option), got {}",
            spec.name,
            describe_raster(result)
        )
    });
}

/// What the suite needs to know about a decoded image or bitmap.
trait Raster: std::fmt::Debug {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn components(&self) -> Option<u16>;
    fn byte_len(&self) -> usize;
    fn expected_len(&self) -> Option<usize>;
    /// Every sample is below `2^source_bits` and padding bits are 0.
    fn samples_in_range(&self) -> bool;
    /// The image is not flat: samples span at least half the largest value (images), or both black
    /// and white pixels occur (bitmaps). A flat fixture would let a constant decoder pass.
    fn varied(&self) -> bool;
    /// Same header (except the height when `rows` is `Some`) and the first `rows` rows (all
    /// rows when `None`) within `tolerance` per sample and `tolerance / 2` on average.
    fn matches_rows(&self, expected: &Self, tolerance: u16, rows: Option<u32>) -> bool;
    /// The whole image matches.
    fn matches(&self, expected: &Self, tolerance: u16) -> bool {
        self.matches_rows(expected, tolerance, None)
    }
}

/// Each sample of `ours` within `tolerance` of `theirs`, and the mean difference within
/// `tolerance / 2` (sum × 2 ≤ tolerance × count). Both slices have the same length.
fn samples_close(ours: &[u8], theirs: &[u8], wide: bool, tolerance: u16) -> bool {
    let mut sum: u64 = 0;
    let mut count: u64 = 0;
    let mut add = |a: u16, b: u16| {
        let diff = a.abs_diff(b);
        sum = sum.saturating_add(u64::from(diff));
        count = count.saturating_add(1);
        diff <= tolerance
    };
    let each = if wide {
        let (a, _) = ours.as_chunks::<2>();
        let (b, _) = theirs.as_chunks::<2>();
        a.iter()
            .zip(b)
            .all(|(a, b)| add(u16::from_be_bytes(*a), u16::from_be_bytes(*b)))
    } else {
        ours.iter()
            .zip(theirs)
            .all(|(&a, &b)| add(u16::from(a), u16::from(b)))
    };
    each && sum.saturating_mul(2) <= u64::from(tolerance).saturating_mul(count)
}

/// The byte length of the first `rows` rows of a raster whose whole data is `len` bytes for
/// `height` rows, or `None` if `rows` exceeds `height` or the data doesn't split into rows.
fn row_prefix(len: usize, height: u32, rows: u32) -> Option<usize> {
    let height = usize::try_from(height).ok().filter(|&h| h > 0)?;
    let rows = usize::try_from(rows).ok().filter(|&r| r <= height)?;
    let row = len
        .checked_div(height)
        .filter(|r| r.checked_mul(height) == Some(len))?;
    row.checked_mul(rows)
}

impl Raster for DecodedImage {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn components(&self) -> Option<u16> {
        Some(self.components)
    }
    fn byte_len(&self) -> usize {
        self.samples.len()
    }
    fn expected_len(&self) -> Option<usize> {
        DecodedImage::expected_len(self)
    }
    fn samples_in_range(&self) -> bool {
        let Some(limit) = 1u32.checked_shl(u32::from(self.source_bits)) else {
            return false;
        };
        self.values().all(|s| u32::from(s) < limit)
    }
    fn varied(&self) -> bool {
        let (min, max) = self
            .values()
            .fold((u16::MAX, 0u16), |(lo, hi), v| (lo.min(v), hi.max(v)));
        // Half the largest value, at least 1: 7 for 4-bit, 127 for 8-bit samples.
        let half = 1u32
            .checked_shl(u32::from(self.source_bits))
            .map_or(u32::MAX, |range| ((range - 1) / 2).max(1));
        max >= min && u32::from(max - min) >= half
    }
    fn matches_rows(&self, expected: &Self, tolerance: u16, rows: Option<u32>) -> bool {
        let height_ok = match rows {
            None => self.height == expected.height,
            Some(r) => r <= self.height && self.height <= expected.height,
        };
        let header = self.width == expected.width
            && height_ok
            && self.components == expected.components
            && self.bits_per_component == expected.bits_per_component
            && self.source_bits == expected.source_bits
            && self.adobe_app14 == expected.adobe_app14
            && self.color == expected.color
            && self.has_alpha == expected.has_alpha;
        if !header {
            return false;
        }
        let (ours, theirs) = match rows {
            None if self.samples.len() == expected.samples.len() => {
                (&self.samples[..], &expected.samples[..])
            }
            None => return false,
            Some(r) => {
                let ours = row_prefix(self.samples.len(), self.height, r)
                    .and_then(|n| self.samples.get(..n));
                let theirs = row_prefix(expected.samples.len(), expected.height, r)
                    .and_then(|n| expected.samples.get(..n));
                match (ours, theirs) {
                    (Some(a), Some(b)) if a.len() == b.len() => (a, b),
                    _ => return false,
                }
            }
        };
        samples_close(ours, theirs, self.bits_per_component == 16, tolerance)
    }
}

impl DecodedImage {
    /// Sample values (big-endian pairs when the container is 16 bits).
    fn values(&self) -> impl Iterator<Item = u16> + '_ {
        let (bytes, pairs): (&[u8], &[[u8; 2]]) = if self.bits_per_component == 16 {
            (&[], self.samples.as_chunks::<2>().0)
        } else {
            (&self.samples, &[])
        };
        bytes
            .iter()
            .map(|&s| u16::from(s))
            .chain(pairs.iter().map(|p| u16::from_be_bytes(*p)))
    }
}

impl Raster for Bitmap {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn components(&self) -> Option<u16> {
        None
    }
    fn byte_len(&self) -> usize {
        self.data.len()
    }
    fn expected_len(&self) -> Option<usize> {
        Bitmap::expected_len(self)
    }
    fn samples_in_range(&self) -> bool {
        // Padding bits are specified as 0.
        let stride = self.stride();
        let used = self.width % 8;
        if used == 0 || stride == 0 {
            return true;
        }
        let padding = 0xffu8 >> used;
        self.data
            .chunks(stride)
            .all(|row| row.last().is_none_or(|&b| b & padding == 0))
    }
    fn varied(&self) -> bool {
        let black: u64 = self.data.iter().map(|b| u64::from(b.count_ones())).sum();
        let pixels = u64::from(self.width).saturating_mul(u64::from(self.height));
        black > 0 && black < pixels
    }
    fn matches_rows(&self, expected: &Self, _tolerance: u16, rows: Option<u32>) -> bool {
        match rows {
            // Padding bits are specified as 0, so whole-byte equality is exact.
            None => self == expected,
            Some(r) => {
                let n = row_prefix(expected.data.len(), expected.height, r);
                self.width == expected.width
                    && r <= self.height
                    && self.height <= expected.height
                    && n.is_some_and(|n| self.data.get(..n) == expected.data.get(..n))
            }
        }
    }
}

type DecodeFn<'a, R> = dyn Fn(&[u8], &DecodeLimits) -> Result<Decoded<R>, BackendError> + 'a;
type Outcome<R> = Option<Result<Decoded<R>, BackendError>>;

/// One fixture, format-independent.
struct FixtureSpec<'a, R> {
    name: &'a str,
    input: &'a [u8],
    expected: &'a R,
    tolerance: u16,
    /// The largest tolerance this fixture may have.
    max_tolerance: u16,
    salvage: Option<Salvage>,
}

/// Checks that need no fixture.
/// `dims`: each fixture's (width, height).
fn common<R: Raster>(report: &mut Report, dims: &[(u32, u32)], decode: &DecodeFn<'_, R>) {
    // Landscape and portrait fixtures are both needed, or a decoder that checks only one axis
    // against `max_image_dimension` would pass `image.dimension_limit`.
    report.check(
        "image.fixture_orientations",
        dims.iter().any(|(w, h)| w > h) && dims.iter().any(|(w, h)| h > w),
        || "fixtures must include a landscape (width > height) and a portrait (height > width) image".into(),
    );
    let fixture_count = dims.len();
    report.check("image.has_fixtures", fixture_count > 0, || {
        "no fixtures supplied; the adapter must provide at least one".into()
    });
    let result = guarded(|| decode(&[], &DecodeLimits::default()));
    report.check("image.empty_input", matches!(result, Some(Err(_))), || {
        format!("expected an error, got {}", describe_raster(&result))
    });
}

/// Why a fixture can't be trusted, if it can't.
fn fixture_problem<R: Raster>(spec: &FixtureSpec<'_, R>) -> Option<String> {
    let e = spec.expected;
    if e.expected_len() != Some(e.byte_len()) || !e.samples_in_range() {
        return Some(
            "the expected output's length does not match its header, a sample is not below \
             2^source_bits, or a padding bit is set"
                .into(),
        );
    }
    if spec.tolerance > spec.max_tolerance {
        return Some(format!(
            "tolerance {} is above the {} allowed for this fixture",
            spec.tolerance, spec.max_tolerance
        ));
    }
    if !e.varied() {
        return Some(
            "the expected output is flat (samples must span half the range; bitmaps need black \
             and white pixels)"
                .into(),
        );
    }
    if let Some(s) = spec.salvage
        && (s.cut >= spec.input.len() || s.rows == 0 || s.rows > e.height())
    {
        return Some(format!(
            "salvage cut {} must be below the input length {} and rows {} within 1..={}",
            s.cut,
            spec.input.len(),
            s.rows,
            e.height()
        ));
    }
    None
}

/// Runs the per-fixture checks; returns whether the fixture is valid (the option checks
/// compare against it only then).
fn run_fixture<R: Raster>(
    report: &mut Report,
    spec: &FixtureSpec<'_, R>,
    decode: &DecodeFn<'_, R>,
) -> bool {
    let &FixtureSpec {
        name,
        input,
        expected,
        tolerance,
        ..
    } = spec;
    let unlimited = DecodeLimits::default();
    let call = |input: &[u8], limits: &DecodeLimits| guarded(|| decode(input, limits));
    if let Some(problem) = fixture_problem(spec) {
        report.check("image.fixture_valid", false, || {
            format!("{name}: {problem}")
        });
        return false;
    }
    let len = expected.byte_len();

    let complete_match =
        |d: &Decoded<R>| d.status == DecodeStatus::Complete && d.value.matches(expected, tolerance);
    let result = call(input, &unlimited);
    let ok = matches!(&result, Some(Ok(d)) if complete_match(d));
    report.check("image.fixture_decodes", ok, || {
        format!(
            "{name}: expected {expected:?}, got {}",
            describe_raster(&result)
        )
    });

    // Exact limits: each at the fixture's size passes (Complete and matching), one below and
    // 0 fail with LimitExceeded.
    let pixels = u64::from(expected.width()).saturating_mul(u64::from(expected.height()));
    let side = expected.width().max(expected.height());
    type SetFn = fn(u64) -> DecodeLimits;
    let mut boundaries: Vec<(&'static str, u64, SetFn)> = vec![
        (
            "image.output_limit",
            u64::try_from(len).unwrap_or(u64::MAX),
            |n| DecodeLimits {
                max_output_bytes: narrow(n),
                ..DecodeLimits::default()
            },
        ),
        ("image.pixel_limit", pixels, |n| DecodeLimits {
            max_image_pixels: n,
            ..DecodeLimits::default()
        }),
        ("image.dimension_limit", u64::from(side), |n| DecodeLimits {
            max_image_dimension: narrow(n),
            ..DecodeLimits::default()
        }),
    ];
    if let Some(components) = expected.components() {
        boundaries.push(("image.component_limit", u64::from(components), |n| {
            DecodeLimits {
                max_image_components: narrow(n),
                ..DecodeLimits::default()
            }
        }));
    }
    for (check, at, set) in boundaries {
        Boundary { check, name, at }.probe(
            report,
            set,
            |limits| call(input, limits),
            complete_match,
            describe_raster,
        );
    }

    // The salvage cut must decode what precedes it.
    if let Some(salvage) = spec.salvage {
        let prefix = input.get(..salvage.cut).unwrap_or(&[]);
        let result = call(prefix, &unlimited);
        let ok = match &result {
            Some(Ok(d)) => {
                d.status == DecodeStatus::Truncated
                    && ok_result_valid(&result, &unlimited)
                    && d.value
                        .matches_rows(expected, tolerance, Some(salvage.rows))
            }
            _ => false,
        };
        report.check("image.salvage", ok, || {
            format!(
                "{name}: cut at {}: expected Ok(Truncated) with the first {} rows, got {}",
                salvage.cut,
                salvage.rows,
                describe_raster(&result)
            )
        });
    }

    // Truncated and mutated input: no panic, and any output obeys its header and the limits.
    // A cap of twice the fixture keeps runaway output visible and the run fast.
    let capped = DecodeLimits {
        max_output_bytes: len.saturating_mul(2),
        ..unlimited
    };
    for cut in sample_offsets(input.len(), 256) {
        let prefix = input.get(..cut).unwrap_or(&[]);
        let result = call(prefix, &capped);
        report.check("image.truncated", ok_result_valid(&result, &capped), || {
            format!(
                "{name}: cut at {cut}/{}: {}",
                input.len(),
                describe_raster(&result)
            )
        });
    }
    for at in sample_offsets(input.len(), 256) {
        let mut mutated = input.to_vec();
        if let Some(byte) = mutated.get_mut(at) {
            *byte ^= 0xff;
        }
        let result = call(&mutated, &capped);
        report.check("image.mutated", ok_result_valid(&result, &capped), || {
            format!("{name}: byte {at} flipped: {}", describe_raster(&result))
        });
    }
    true
}

/// An error, or output whose length matches its header and which stays within `limits`.
fn ok_result_valid<R: Raster>(result: &Outcome<R>, limits: &DecodeLimits) -> bool {
    match result {
        None => false,
        Some(Err(_)) => true,
        Some(Ok(d)) => {
            let r = &d.value;
            let pixels = u64::from(r.width()).saturating_mul(u64::from(r.height()));
            r.expected_len() == Some(r.byte_len())
                && r.samples_in_range()
                && r.byte_len() <= limits.max_output_bytes
                && pixels <= limits.max_image_pixels
                && r.width() <= limits.max_image_dimension
                && r.height() <= limits.max_image_dimension
                && r.components()
                    .is_none_or(|c| c <= limits.max_image_components)
        }
    }
}

fn describe_raster<R: Raster>(result: &Outcome<R>) -> String {
    match result {
        Some(Ok(d)) => format!(
            "Ok({:?}, {}x{}, {} bytes)",
            d.status,
            d.value.width(),
            d.value.height(),
            d.value.byte_len()
        ),
        other => describe(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A toy codec standing in for every format: `width u16 BE, height u16 BE, flags u8, then
    /// samples`. The suite is format-independent, so this exercises every check.
    ///
    /// Flags: the low nibble is the component count. The high nibble is the source depth
    /// (0 means 8, 1–8 as is), or an 8-bit image with an "APP14" transform flag: 9 → 0,
    /// 10 → 2, 11 → 1. A "transform" XORs every sample with 0x40; JPEG decodes it when
    /// ColorTransform says so, else when the APP14 flag is 1 or 2.
    ///
    /// Bitmaps (JBIG2, CCITT) are packed rows after the header. JBIG2 XORs them with the first
    /// globals byte, CCITT with a key from K and EncodedByteAlign ([`ccitt_key`]); padding
    /// bits are then cleared.
    #[derive(Default)]
    struct Toy {
        ignore_limits: bool,
        width_only_dimension_check: bool,
        /// Rescales sub-8-bit samples to 0–255 (the bug `source_bits` exists to prevent).
        scale_low_depth: bool,
        panic_on_mutation: bool,
        lie_about_length: bool,
        /// Reads `max_output_bytes` 0 as "no limit".
        zero_unlimited_output: bool,
        /// Returns zeros instead of the samples.
        all_zero: bool,
        /// Ignores ColorTransform, JBIG2 globals, K and EncodedByteAlign.
        ignore_options: bool,
        /// Fails every truncated input instead of salvaging it.
        fail_salvage: bool,
        /// Salvages truncated input as an all-zero image.
        zero_salvage: bool,
        /// Calls salvaged output `Complete`.
        truncated_as_complete: bool,
        /// Reports `Truncated` when the output cap equals the output size.
        truncated_at_output_limit: bool,
        /// CCITT: checks no parameter against the limits before decoding.
        ccitt_no_param_checks: bool,
        /// CCITT: checks only `Columns` before decoding.
        ccitt_columns_only: bool,
        /// CCITT: panics on K = i32::MIN.
        panic_on_extreme_k: bool,
    }

    /// The toy CCITT key for K and EncodedByteAlign.
    fn ccitt_key(params: &CcittParams) -> u8 {
        let k = match params.k {
            ..0 => 0x00,
            0 => 0x5a,
            1.. => 0xa5,
        };
        if params.encoded_byte_align {
            k ^ 0xf0
        } else {
            k
        }
    }

    /// Clears the padding bits of packed rows.
    fn mask_padding(data: &mut [u8], width: u32) {
        let stride = usize::try_from(width.div_ceil(8)).unwrap();
        if width.is_multiple_of(8) {
            return;
        }
        let keep = !(0xffu8 >> (width % 8));
        for row in data.chunks_mut(stride) {
            if let Some(last) = row.last_mut() {
                *last &= keep;
            }
        }
    }

    impl Toy {
        /// `packed`: one bit per pixel, rows padded to bytes (bitmaps). `transform`: the
        /// caller's ColorTransform (`None` lets the APP14 flag decide).
        fn decode_image(
            &self,
            input: &[u8],
            limits: &DecodeLimits,
            packed: bool,
            transform: Option<bool>,
        ) -> Result<Decoded<DecodedImage>, BackendError> {
            let header = input.get(..5).ok_or(BackendError::Truncated)?;
            if self.panic_on_mutation && header[4] & 0x0f > 4 {
                panic!("bad components");
            }
            let width = u32::from(u16::from_be_bytes([header[0], header[1]]));
            let height = u32::from(u16::from_be_bytes([header[2], header[3]]));
            let components = u16::from(header[4] & 0x0f);
            let (source_bits, adobe_app14) = match header[4] >> 4 {
                0 => (8, None),
                b @ 1..=8 => (b, None),
                9 => (8, Some(0)),
                10 => (8, Some(2)),
                11 => (8, Some(1)),
                _ => return Err(BackendError::Malformed("depth")),
            };
            if width == 0 || height == 0 || components == 0 {
                return Err(BackendError::Malformed("header"));
            }
            if !self.ignore_limits
                && (if self.width_only_dimension_check {
                    width
                } else {
                    width.max(height)
                } > limits.max_image_dimension
                    || u64::from(width) * u64::from(height) > limits.max_image_pixels
                    || components > limits.max_image_components)
            {
                return Err(BackendError::LimitExceeded("image"));
            }
            let len = if packed {
                usize::try_from(width.div_ceil(8) * height).unwrap()
            } else {
                usize::try_from(width * height).unwrap() * usize::from(components)
            };
            let max_output = if self.zero_unlimited_output && limits.max_output_bytes == 0 {
                usize::MAX
            } else {
                limits.max_output_bytes
            };
            if !self.ignore_limits && len > max_output {
                return Err(BackendError::LimitExceeded("output"));
            }
            let body = &input[5..];
            let mut samples = body[..body.len().min(len)].to_vec();
            if !packed && samples.iter().any(|&v| u32::from(v) >= 1 << source_bits) {
                return Err(BackendError::Malformed("sample"));
            }
            if self.scale_low_depth && source_bits < 8 {
                let max = (1u32 << source_bits) - 1;
                for v in &mut samples {
                    *v = u8::try_from(u32::from(*v) * 255 / max).unwrap();
                }
            }
            let transform = match transform {
                Some(t) if !self.ignore_options => t,
                _ => matches!(adobe_app14, Some(1 | 2)),
            };
            if !packed && transform && components >= 3 {
                for v in &mut samples {
                    *v ^= 0x40;
                }
            }
            if self.all_zero {
                samples.fill(0);
            }
            let status = if samples.len() < len {
                if samples.is_empty() || self.fail_salvage {
                    return Err(BackendError::Truncated);
                }
                if self.zero_salvage {
                    samples.fill(0);
                }
                if !self.lie_about_length {
                    samples.resize(len, 0);
                }
                if self.truncated_as_complete {
                    DecodeStatus::Complete
                } else {
                    DecodeStatus::Truncated
                }
            } else if self.truncated_at_output_limit && len == limits.max_output_bytes {
                DecodeStatus::Truncated
            } else {
                DecodeStatus::Complete
            };
            Ok(Decoded {
                value: DecodedImage {
                    width,
                    height,
                    components,
                    bits_per_component: 8,
                    source_bits: if packed { 8 } else { source_bits },
                    adobe_app14,
                    color: match components {
                        1 => ImageColor::Gray,
                        4 => ImageColor::Cmyk,
                        _ => ImageColor::Unknown,
                    },
                    has_alpha: false,
                    samples,
                },
                status,
            })
        }

        /// Decodes a bitmap whose packed rows are XORed with `key`.
        fn decode_bitmap(
            &self,
            input: &[u8],
            limits: &DecodeLimits,
            key: u8,
        ) -> Result<Decoded<Bitmap>, BackendError> {
            let d = self.decode_image(input, limits, true, None)?;
            let key = if self.ignore_options { 0 } else { key };
            let mut data = d.value.samples;
            for byte in &mut data {
                *byte ^= key;
            }
            mask_padding(&mut data, d.value.width);
            Ok(Decoded {
                value: Bitmap {
                    width: d.value.width,
                    height: d.value.height,
                    data,
                },
                status: d.status,
            })
        }
    }

    impl JpegDecoder for Toy {
        fn decode(
            &self,
            input: &[u8],
            options: &JpegOptions,
            limits: &DecodeLimits,
        ) -> Result<Decoded<DecodedImage>, BackendError> {
            self.decode_image(input, limits, false, options.color_transform)
        }
    }

    impl Jpeg2000Decoder for Toy {
        fn decode(
            &self,
            input: &[u8],
            limits: &DecodeLimits,
        ) -> Result<Decoded<DecodedImage>, BackendError> {
            self.decode_image(input, limits, false, None)
        }
    }

    impl Jbig2Decoder for Toy {
        fn decode(
            &self,
            input: &[u8],
            globals: Option<&[u8]>,
            limits: &DecodeLimits,
        ) -> Result<Decoded<Bitmap>, BackendError> {
            let key = match globals {
                Some(g) if g.len() < 2 => return Err(BackendError::Malformed("globals")),
                Some(g) => g[0],
                None => 0,
            };
            self.decode_bitmap(input, limits, key)
        }
    }

    impl CcittDecoder for Toy {
        fn decode(
            &self,
            input: &[u8],
            params: &CcittParams,
            limits: &DecodeLimits,
        ) -> Result<Decoded<Bitmap>, BackendError> {
            if params.columns == 0 {
                return Err(BackendError::InvalidArgument("Columns"));
            }
            if !self.ccitt_no_param_checks {
                let rows_checked = !self.ccitt_columns_only;
                if params.columns > limits.max_image_dimension
                    || (rows_checked
                        && (params.rows > limits.max_image_dimension
                            || u64::from(params.columns) * u64::from(params.rows)
                                > limits.max_image_pixels))
                {
                    return Err(BackendError::LimitExceeded("CCITT parameters"));
                }
            }
            if self.panic_on_extreme_k && params.k == i32::MIN {
                panic!("K");
            }
            self.decode_bitmap(input, limits, ccitt_key(params))
        }
    }

    /// A toy image: `(input, expected)` with samples `i * 7 mod 256` (masked to the depth),
    /// as stored. `flags` is the toy flags byte.
    fn toy_image(width: u16, height: u16, flags: u8) -> (Vec<u8>, DecodedImage) {
        let components = flags & 0x0f;
        let (source_bits, adobe_app14) = match flags >> 4 {
            0 => (8, None),
            b @ 1..=8 => (b, None),
            9 => (8, Some(0)),
            10 => (8, Some(2)),
            _ => (8, Some(1)),
        };
        let len = usize::from(width) * usize::from(height) * usize::from(components);
        let mask = u8::try_from((1u16 << source_bits) - 1).unwrap();
        let samples: Vec<u8> = (0..len)
            .map(|i| u8::try_from(i * 7 % 256).unwrap() & mask)
            .collect();
        let [w0, w1] = width.to_be_bytes();
        let [h0, h1] = height.to_be_bytes();
        let mut input = vec![w0, w1, h0, h1, flags];
        input.extend_from_slice(&samples);
        let image = DecodedImage {
            width: u32::from(width),
            height: u32::from(height),
            components: u16::from(components),
            bits_per_component: 8,
            source_bits,
            adobe_app14,
            color: match components {
                1 => ImageColor::Gray,
                4 => ImageColor::Cmyk,
                _ => ImageColor::Unknown,
            },
            has_alpha: false,
            samples,
        };
        (input, image)
    }

    /// A packed bitmap after the toy header, stored XORed with `key`; the expected bitmap has
    /// its padding cleared.
    fn toy_bitmap(width: u16, height: u16, key: u8) -> (Vec<u8>, Bitmap) {
        let len = usize::from(width.div_ceil(8)) * usize::from(height);
        let mut data: Vec<u8> = (0..len)
            .map(|i| u8::try_from(i * 37 % 256).unwrap())
            .collect();
        mask_padding(&mut data, u32::from(width));
        let [w0, w1] = width.to_be_bytes();
        let [h0, h1] = height.to_be_bytes();
        let mut input = vec![w0, w1, h0, h1, 1];
        input.extend(data.iter().map(|b| b ^ key));
        let bitmap = Bitmap {
            width: u32::from(width),
            height: u32::from(height),
            data,
        };
        (input, bitmap)
    }

    /// The salvage cut every toy fixture uses: the last byte missing, all rows but the last
    /// intact.
    fn last_byte_cut(input: &[u8], height: u32) -> Option<Salvage> {
        Some(Salvage {
            cut: input.len() - 1,
            rows: height - 1,
        })
    }

    /// A toy JPEG fixture. The expected samples are transformed when the toy decodes them so.
    fn jpeg_fixture(
        (w, h): (u16, u16),
        flags: u8,
        color_transform: Option<bool>,
        salvage: bool,
    ) -> JpegFixture {
        let (input, mut expected) = toy_image(w, h, flags);
        let transform = color_transform.unwrap_or(matches!(expected.adobe_app14, Some(1 | 2)));
        if transform && expected.components >= 3 {
            for v in &mut expected.samples {
                *v ^= 0x40;
            }
        }
        JpegFixture {
            name: format!("toy {w}x{h} {flags:#04x}"),
            salvage: if salvage {
                last_byte_cut(&input, expected.height)
            } else {
                None
            },
            input,
            options: JpegOptions { color_transform },
            expected,
            tolerance: 0,
        }
    }

    /// Landscape 4×3 RGB with APP14 1 and ColorTransform true (salvage cut), portrait 2×5 RGB
    /// with APP14 0 and ColorTransform false, square 3×3 CMYK with APP14 2 and no
    /// ColorTransform.
    fn jpeg_fixtures() -> Vec<JpegFixture> {
        vec![
            jpeg_fixture((4, 3), 0xb3, Some(true), true),
            jpeg_fixture((2, 5), 0x93, Some(false), false),
            jpeg_fixture((3, 3), 0xa4, None, false),
        ]
    }

    /// Landscape 4×3 (salvage cut) and portrait 2×5 RGB, plus a 4-bit 3×2 image; lossless.
    fn jpx_fixtures() -> Vec<Jpeg2000Fixture> {
        [
            ((4, 3), 0x03, true),
            ((2, 5), 0x03, false),
            ((3, 2), 0x43, false),
        ]
        .into_iter()
        .map(|((w, h), flags, salvage)| {
            let (input, expected) = toy_image(w, h, flags);
            Jpeg2000Fixture {
                name: format!("toy {w}x{h} {flags:#04x}"),
                salvage: if salvage {
                    last_byte_cut(&input, expected.height)
                } else {
                    None
                },
                input,
                expected,
                lossy: false,
                tolerance: 0,
            }
        })
        .collect()
    }

    /// Landscape 12×2 and portrait 3×9 pages that use their globals (first byte 0x80, the
    /// toy's XOR key). `key` is what the input is stored with: 0x80, or 0 for pages that
    /// don't depend on their globals.
    fn jbig2_fixtures(key: u8) -> Vec<Jbig2Fixture> {
        [(12, 2), (3, 9)]
            .into_iter()
            .map(|(w, h)| {
                let (input, expected) = toy_bitmap(w, h, key);
                Jbig2Fixture {
                    name: format!("toy {w}x{h}"),
                    input,
                    globals: Some(vec![0x80, 2, 3, 4]),
                    expected,
                }
            })
            .collect()
    }

    /// Landscape 12×2 Group 4 with Rows set (salvage cut), portrait 3×9 Group 3 1-D with
    /// EncodedByteAlign, landscape 10×3 Group 3 2-D. `keyed`: inputs stored with the toy's
    /// K/alignment key (false for streams that decode the same under any K).
    fn ccitt_fixtures(keyed: bool) -> Vec<CcittFixture> {
        [
            ((12, 2), -1, false, true),
            ((3, 9), 0, true, false),
            ((10, 3), 1, false, false),
        ]
        .into_iter()
        .map(|((w, h), k, encoded_byte_align, salvage)| {
            let params = CcittParams {
                k,
                encoded_byte_align,
                columns: u32::from(w),
                rows: if salvage { u32::from(h) } else { 0 },
                ..CcittParams::default()
            };
            let key = if keyed { ccitt_key(&params) } else { 0 };
            let (input, expected) = toy_bitmap(w, h, key);
            CcittFixture {
                name: format!("toy {w}x{h} K {k}"),
                salvage: if salvage {
                    last_byte_cut(&input, expected.height)
                } else {
                    None
                },
                input,
                params,
                expected,
            }
        })
        .collect()
    }

    #[test]
    fn jpx_needs_a_low_depth_fixture_and_unscaled_samples() {
        let mut fixtures = jpx_fixtures();
        fixtures.pop();
        let report = run_jpeg2000(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.jpx_low_depth_fixture"],
            "{report}"
        );
        let report = run_jpeg2000(
            &Toy {
                scale_low_depth: true,
                ..Toy::default()
            },
            &jpx_fixtures(),
        );
        assert!(report.failed("image.fixture_decodes"), "{report}");
        assert!(report.failed("image.truncated"), "{report}");
    }

    #[test]
    fn out_of_range_expected_samples_are_a_bad_fixture() {
        let mut fixtures = jpx_fixtures();
        fixtures.last_mut().unwrap().expected.samples[0] = 16;
        let report = run_jpeg2000(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
    }

    fn jpeg(toy: &Toy) -> Report {
        run_jpeg(toy, &jpeg_fixtures())
    }

    #[test]
    fn a_correct_decoder_passes_every_suite() {
        let toy = Toy::default();
        let report = jpeg(&toy);
        assert!(report.passed(), "{report}");

        let report = run_jpeg2000(&toy, &jpx_fixtures());
        assert!(report.passed(), "{report}");

        let report = run_jbig2(&toy, &jbig2_fixtures(0x80));
        assert!(report.passed(), "{report}");
        assert!(report.checks_run > 50, "{report}");

        let report = run_ccitt(&toy, &ccitt_fixtures(true));
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn no_fixtures_fails() {
        let report = run_jpeg(&Toy::default(), &[]);
        assert_eq!(
            report.failed_checks(),
            vec![
                "image.fixture_orientations",
                "image.has_fixtures",
                "image.option_coverage",
                "image.salvage_fixture"
            ],
            "{report}"
        );
    }

    #[test]
    fn one_orientation_is_not_enough() {
        let mut fixtures = jpeg_fixtures();
        // The portrait 2×5 turned landscape, with the same options.
        fixtures[1] = jpeg_fixture((5, 2), 0x93, Some(false), false);
        let report = run_jpeg(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_orientations"],
            "{report}"
        );
    }

    #[test]
    fn checking_only_the_width_is_caught() {
        let report = jpeg(&Toy {
            width_only_dimension_check: true,
            ..Toy::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["image.dimension_limit"],
            "{report}"
        );
    }

    #[test]
    fn ignored_limits_are_caught() {
        let report = jpeg(&Toy {
            ignore_limits: true,
            ..Toy::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec![
                "image.component_limit",
                "image.dimension_limit",
                // A flipped width byte makes the toy produce more than the cap.
                "image.mutated",
                "image.output_limit",
                "image.pixel_limit",
            ],
            "{report}"
        );
    }

    #[test]
    fn reading_a_zero_output_cap_as_unlimited_is_caught() {
        let report = jpeg(&Toy {
            zero_unlimited_output: true,
            ..Toy::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["image.output_limit"],
            "{report}"
        );
        assert!(
            report
                .failures()
                .iter()
                .all(|f| f.detail.contains("limit 0 (zero)")),
            "{report}"
        );
    }

    #[test]
    fn a_truncated_status_at_the_limit_is_caught() {
        let report = jpeg(&Toy {
            truncated_at_output_limit: true,
            ..Toy::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["image.output_limit"],
            "{report}"
        );
        assert!(
            report
                .failures()
                .iter()
                .all(|f| f.detail.contains("(equal to what the input needs)")),
            "{report}"
        );
    }

    #[test]
    fn panics_on_mutated_input_are_caught() {
        let report = jpeg(&Toy {
            panic_on_mutation: true,
            ..Toy::default()
        });
        assert_eq!(report.failed_checks(), vec!["image.mutated"], "{report}");
    }

    #[test]
    fn output_shorter_than_its_header_is_caught() {
        let report = jpeg(&Toy {
            lie_about_length: true,
            ..Toy::default()
        });
        // The salvage cut is a truncated input too.
        assert_eq!(
            report.failed_checks(),
            vec!["image.salvage", "image.truncated"],
            "{report}"
        );
    }

    #[test]
    fn a_wrong_pixel_is_caught_and_tolerance_applies() {
        let fixtures = |tolerance| {
            let mut fixtures = jpeg_fixtures();
            fixtures[0].expected.samples[5] = fixtures[0].expected.samples[5].wrapping_add(2);
            for f in &mut fixtures {
                f.tolerance = tolerance;
            }
            fixtures
        };
        let report = run_jpeg(&Toy::default(), &fixtures(1));
        assert!(report.failed("image.fixture_decodes"), "{report}");
        let report = run_jpeg(&Toy::default(), &fixtures(2));
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn the_mean_error_is_bounded_by_half_the_tolerance() {
        // Every second sample (or every sample) off by exactly the tolerance, 2.
        let fixtures = |every: usize| {
            let mut fixtures = jpeg_fixtures();
            for f in &mut fixtures {
                f.tolerance = 2;
            }
            for v in fixtures[0].expected.samples.iter_mut().step_by(every) {
                *v = if *v >= 2 { *v - 2 } else { *v + 2 };
            }
            fixtures
        };
        let report = run_jpeg(&Toy::default(), &fixtures(2));
        assert!(report.passed(), "{report}");
        let report = run_jpeg(&Toy::default(), &fixtures(1));
        assert!(report.failed("image.fixture_decodes"), "{report}");
    }

    #[test]
    fn max_lossy_tolerance_scales_with_depth() {
        assert_eq!(max_lossy_tolerance(1), 1);
        assert_eq!(max_lossy_tolerance(7), 1);
        assert_eq!(max_lossy_tolerance(8), 2);
        assert_eq!(max_lossy_tolerance(12), 32);
        assert_eq!(max_lossy_tolerance(16), 512);
    }

    #[test]
    fn tolerance_above_the_cap_is_a_bad_fixture() {
        let with = |tolerance| {
            let mut fixtures = jpeg_fixtures();
            fixtures[1].tolerance = tolerance;
            run_jpeg(&Toy::default(), &fixtures)
        };
        assert!(with(2).passed(), "{}", with(2));
        let report = with(3);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
    }

    #[test]
    fn an_all_zero_decoder_needs_a_tolerance_that_is_refused() {
        let mut fixtures = jpeg_fixtures();
        for f in &mut fixtures {
            f.tolerance = 255;
        }
        let report = run_jpeg(
            &Toy {
                all_zero: true,
                ..Toy::default()
            },
            &fixtures,
        );
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
    }

    #[test]
    fn lossless_jpx_needs_zero_tolerance() {
        let with = |lossy, tolerance| {
            let mut fixtures = jpx_fixtures();
            for f in &mut fixtures {
                f.lossy = lossy;
                f.tolerance = tolerance;
            }
            run_jpeg2000(&Toy::default(), &fixtures)
        };
        // The 4-bit fixture's lossy cap is 1.
        assert!(with(true, 1).passed(), "{}", with(true, 1));
        let report = with(false, 1);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
        let report = with(true, 2);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
        assert_eq!(report.failures().len(), 1, "{report}");
    }

    #[test]
    fn flat_fixtures_are_refused() {
        // A JPEG fixture with every sample 7 (decodes fine, proves nothing).
        let mut fixtures = jpeg_fixtures();
        let f = &mut fixtures[1];
        f.input[5..].fill(7);
        f.expected.samples.fill(7);
        let report = run_jpeg(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
        // An all-white and an all-black bitmap.
        for byte in [0x00, 0xff] {
            let mut fixtures = jbig2_fixtures(0x80);
            let f = &mut fixtures[0];
            f.input[5..].fill(byte ^ 0x80);
            f.expected.data.fill(byte);
            mask_padding(&mut f.expected.data, f.expected.width);
            let report = run_jbig2(&Toy::default(), &fixtures);
            assert_eq!(
                report.failed_checks(),
                vec!["image.fixture_valid"],
                "{byte}: {report}"
            );
        }
    }

    #[test]
    fn bitmap_padding_must_be_zero() {
        let mut fixtures = jbig2_fixtures(0x80);
        // 12 pixels wide: the low 4 bits of every second byte are padding.
        fixtures[0].expected.data[1] |= 0x01;
        let report = run_jbig2(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
        assert!(
            Bitmap {
                width: 12,
                height: 1,
                data: vec![0, 0xf0],
            }
            .samples_in_range()
        );
        assert!(
            !Bitmap {
                width: 12,
                height: 1,
                data: vec![0, 0x08],
            }
            .samples_in_range()
        );
    }

    #[test]
    fn a_decoder_that_does_not_salvage_is_caught() {
        for toy in [
            Toy {
                fail_salvage: true,
                ..Toy::default()
            },
            Toy {
                zero_salvage: true,
                ..Toy::default()
            },
            Toy {
                truncated_as_complete: true,
                ..Toy::default()
            },
        ] {
            let report = jpeg(&toy);
            assert_eq!(report.failed_checks(), vec!["image.salvage"], "{report}");
            let report = run_jpeg2000(&toy, &jpx_fixtures());
            assert_eq!(report.failed_checks(), vec!["image.salvage"], "{report}");
            let report = run_ccitt(&toy, &ccitt_fixtures(true));
            assert_eq!(report.failed_checks(), vec!["image.salvage"], "{report}");
        }
    }

    #[test]
    fn a_salvage_cut_is_required_and_validated() {
        let mut fixtures = jpeg_fixtures();
        fixtures[0].salvage = None;
        let report = run_jpeg(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.salvage_fixture"],
            "{report}"
        );
        let mut fixtures = jpx_fixtures();
        fixtures[0].salvage = None;
        let report = run_jpeg2000(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.salvage_fixture"],
            "{report}"
        );

        let len = jpeg_fixtures()[0].input.len();
        for (cut, rows) in [(len, 1), (len - 1, 0), (len - 1, 4)] {
            let mut fixtures = jpeg_fixtures();
            fixtures[0].salvage = Some(Salvage { cut, rows });
            let report = run_jpeg(&Toy::default(), &fixtures);
            assert_eq!(
                report.failed_checks(),
                vec!["image.fixture_valid"],
                "cut {cut} rows {rows}: {report}"
            );
        }
        // At the edges: the full height, and the shortest cut that still has row 0 intact.
        let mut fixtures = jpeg_fixtures();
        fixtures[0].salvage = Some(Salvage {
            cut: 5 + 12,
            rows: 1,
        });
        assert!(run_jpeg(&Toy::default(), &fixtures).passed());

        let mut fixtures = ccitt_fixtures(true);
        fixtures[0].params.rows = 0;
        let report = run_ccitt(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
        let mut fixtures = ccitt_fixtures(true);
        fixtures[0].salvage = None;
        let report = run_ccitt(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.salvage_fixture"],
            "{report}"
        );
    }

    #[test]
    fn salvaged_rows_compare_with_the_tolerance() {
        let expected = toy_image(4, 3, 0x03).1;
        let mut partial = expected.clone();
        // Row 0 off by 1 everywhere except one sample, rows 1–2 garbage.
        for v in &mut partial.samples[1..12] {
            *v ^= 1;
        }
        partial.samples[12..].fill(0xee);
        assert!(!partial.matches_rows(&expected, 0, Some(1)));
        assert!(!partial.matches_rows(&expected, 1, Some(1)), "mean bound");
        assert!(partial.matches_rows(&expected, 2, Some(1)));
        assert!(!partial.matches_rows(&expected, 2, Some(2)));
        // A shorter salvaged image is fine as long as it holds the rows.
        let mut short = expected.clone();
        short.height = 1;
        short.samples.truncate(12);
        assert!(short.matches_rows(&expected, 0, Some(1)));
        assert!(!short.matches_rows(&expected, 0, Some(2)));
        assert!(!short.matches(&expected, 0));
    }

    #[test]
    fn ignored_options_are_caught() {
        let toy = Toy {
            ignore_options: true,
            ..Toy::default()
        };
        // The JPEG fixtures' APP14 flags agree with their ColorTransform, so only the flip
        // shows the option is ignored.
        let report = jpeg(&toy);
        assert_eq!(
            report.failed_checks(),
            vec!["image.option_used"],
            "{report}"
        );
        assert_eq!(report.failures().len(), 2, "{report}");

        // Fixtures that decode the same without their globals or under any K.
        let report = run_jbig2(&toy, &jbig2_fixtures(0));
        assert_eq!(
            report.failed_checks(),
            vec!["image.option_used"],
            "{report}"
        );
        let report = run_ccitt(&toy, &ccitt_fixtures(false));
        assert_eq!(
            report.failed_checks(),
            vec!["image.option_used"],
            "{report}"
        );
        // Three K flips and one EncodedByteAlign flip.
        assert_eq!(report.failures().len(), 4, "{report}");
    }

    #[test]
    fn option_coverage_is_required() {
        let toy = Toy::default();
        // No ColorTransform false, or no CMYK with APP14.
        for drop in [1, 2] {
            let mut fixtures = jpeg_fixtures();
            fixtures.remove(drop);
            if drop == 1 {
                fixtures.push(jpeg_fixture((2, 5), 0x93, None, false));
            }
            let report = run_jpeg(&toy, &fixtures);
            assert_eq!(
                report.failed_checks(),
                vec!["image.option_coverage"],
                "drop {drop}: {report}"
            );
        }
        let mut fixtures = jbig2_fixtures(0);
        for f in &mut fixtures {
            f.globals = None;
        }
        let report = run_jbig2(&toy, &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.option_coverage"],
            "{report}"
        );
        // Each CCITT scheme in turn replaced: Group 4 by 1-D, the others by Group 4 (the
        // aligned 1-D fixture loses both K = 0 and EncodedByteAlign).
        for (i, k) in [(0, 0), (1, -1), (2, -1)] {
            let mut fixtures = ccitt_fixtures(true);
            let f = &mut fixtures[i];
            let (w, h) = (f.expected.width, f.expected.height);
            f.params.k = k;
            f.params.encoded_byte_align = false;
            let key = ccitt_key(&f.params);
            let (input, _) = toy_bitmap(u16::try_from(w).unwrap(), u16::try_from(h).unwrap(), key);
            f.salvage = f.salvage.map(|_| last_byte_cut(&input, h).unwrap());
            f.input = input;
            let report = run_ccitt(&toy, &fixtures);
            assert_eq!(
                report.failed_checks(),
                vec!["image.option_coverage"],
                "fixture {i}: {report}"
            );
        }
    }

    #[test]
    fn ccitt_parameters_must_be_checked_before_decoding() {
        let report = run_ccitt(
            &Toy {
                ccitt_no_param_checks: true,
                ..Toy::default()
            },
            &ccitt_fixtures(true),
        );
        assert_eq!(
            report.failed_checks(),
            vec!["image.ccitt_param_limits"],
            "{report}"
        );

        let report = run_ccitt(
            &Toy {
                ccitt_columns_only: true,
                ..Toy::default()
            },
            &ccitt_fixtures(true),
        );
        assert_eq!(
            report.failed_checks(),
            vec!["image.ccitt_param_limits"],
            "{report}"
        );
        let details: Vec<&str> = report
            .failures()
            .iter()
            .map(|f| f.detail.as_str())
            .collect();
        // Rows only on the portrait fixture; the pixel product on all three.
        assert_eq!(details.len(), 4, "{report}");
        assert_eq!(
            details
                .iter()
                .filter(|d| d.contains("Rows one past max_image_dimension"))
                .count(),
            1,
            "{report}"
        );
        assert!(
            details
                .iter()
                .filter(|d| !d.contains("Rows one past max_image_dimension"))
                .all(|d| d.contains("one past max_image_pixels")),
            "{report}"
        );
    }

    #[test]
    fn panics_on_extreme_ccitt_parameters_are_caught() {
        let report = run_ccitt(
            &Toy {
                panic_on_extreme_k: true,
                ..Toy::default()
            },
            &ccitt_fixtures(true),
        );
        assert_eq!(
            report.failed_checks(),
            vec!["image.ccitt_extreme_params"],
            "{report}"
        );
    }

    #[test]
    fn header_metadata_must_match() {
        for change in [
            // The 4-bit fixture's samples are all below 16, so 5 bits is still a valid
            // fixture, just not what the decoder reports.
            |e: &mut DecodedImage| e.source_bits = 5,
            |e: &mut DecodedImage| e.adobe_app14 = Some(2),
            |e: &mut DecodedImage| e.color = ImageColor::Rgb,
            |e: &mut DecodedImage| e.has_alpha = true,
        ] {
            let mut fixtures = jpx_fixtures();
            change(&mut fixtures.last_mut().unwrap().expected);
            let report = run_jpeg2000(&Toy::default(), &fixtures);
            assert_eq!(
                report.failed_checks(),
                vec![
                    "image.component_limit",
                    "image.dimension_limit",
                    "image.fixture_decodes",
                    "image.output_limit",
                    "image.pixel_limit"
                ],
                "{report}"
            );
        }
    }

    #[test]
    fn sixteen_bit_tolerance_compares_whole_samples() {
        // Two samples, one off by 1, so the mean bound (half the tolerance) holds at
        // tolerance 1.
        let base = DecodedImage {
            width: 2,
            height: 1,
            components: 1,
            bits_per_component: 16,
            source_bits: 16,
            adobe_app14: None,
            color: ImageColor::Gray,
            has_alpha: false,
            samples: vec![0x01, 0xff, 0x80, 0x00],
        };
        let next = DecodedImage {
            samples: vec![0x02, 0x00, 0x80, 0x00],
            ..base.clone()
        };
        // 0x01ff vs 0x0200 differ by 1, although the high bytes differ.
        assert!(base.matches(&next, 1));
        assert!(!base.matches(&next, 0));
    }

    #[test]
    fn a_bad_fixture_is_reported_not_trusted() {
        let mut fixtures = jpeg_fixtures();
        fixtures[0].expected.samples.pop();
        let report = run_jpeg(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
    }
}
