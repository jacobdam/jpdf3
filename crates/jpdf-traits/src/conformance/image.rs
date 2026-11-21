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
    /// The same image encoded with less work, if any. JPEG, JPEG 2000 and JBIG2 need one
    /// valid fixture whose claim is heavy ([`HEAVY_WORK_FACTOR`]).
    pub extra_work: Option<ExtraWork>,
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
    /// The same image encoded with less work, if any. JPEG, JPEG 2000 and JBIG2 need one
    /// valid fixture whose claim is heavy ([`HEAVY_WORK_FACTOR`]).
    pub extra_work: Option<ExtraWork>,
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
    /// The same image encoded with less work, if any. JPEG, JPEG 2000 and JBIG2 need one
    /// valid fixture whose claim is heavy ([`HEAVY_WORK_FACTOR`]).
    pub extra_work: Option<ExtraWork>,
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

/// A claim that a fixture's input needs more work than a lighter encoding of the same image
/// (specs/core/decode-work-limits.md). The suite measures the difference between the two
/// decodes, so a decoder that charges only per output sample, however many per sample, reports
/// no difference and fails `image.extra_work`.
#[derive(Debug, Clone)]
pub struct ExtraWork {
    /// The same image with less work (one scan instead of several, one quality layer, no
    /// overlapping symbol instances). It must decode Complete to the fixture's expected output.
    pub lighter: Vec<u8>,
    /// A lower bound on `ops(input) - ops(lighter)`, justified from the two streams.
    pub min_extra_ops: u64,
}

/// A heavy-work claim's `min_extra_ops` is at least this many times the output samples.
/// JPEG, JPEG 2000 and JBIG2 need a valid fixture with one (`image.work_fixture`).
pub const HEAVY_WORK_FACTOR: u64 = 4;

/// The largest per-sample tolerance a lossy fixture may have: 1 below 8 bits, 2 otherwise.
/// It does not grow with the sample range, so a 12- or 16-bit decoder that works at lower
/// precision (errors up to 15 when 4 bits are dropped) fails.
pub fn max_lossy_tolerance(source_bits: u8) -> u16 {
    if source_bits < 8 { 1 } else { 2 }
}

/// The ColorTransform a JPEG with `components` and an APP14 flag gets when the PDF gives none
/// (ISO 32000-2 §7.4.8, Table 13): the APP14 flag decides (0 = none), otherwise only
/// 3-component images are converted.
fn implied_transform(components: u16, adobe_app14: Option<u8>) -> bool {
    match adobe_app14 {
        Some(flag) => flag != 0,
        None => components == 3,
    }
}

/// Runs every check against a [`JpegDecoder`]. At least one fixture is required.
pub fn run_jpeg(decoder: &dyn JpegDecoder, fixtures: &[JpegFixture]) -> Report {
    let mut report = Report::new("jpeg");
    let mut heavy = false;
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
            refused: None,
            extra_work: f.extra_work.as_ref(),
        };
        let valid = run_fixture(&mut report, &spec, &decode);
        heavy |= valid && spec.heavy();
        if valid && f.expected.components >= 3 {
            color_transform(&mut report, decoder, f, &spec);
        }
    }
    work_coverage(&mut report, heavy);
    let has = |pred: &dyn Fn(&JpegFixture) -> bool| fixtures.iter().any(pred);
    // Explicit values must override the implied transform in both directions, or an adapter
    // that reads one explicit value as "the codestream decides" passes.
    let coverage: [(&str, bool); 6] = [
        (
            "a 3- or 4-component image with ColorTransform true and APP14 transform 0",
            has(&|f| {
                f.expected.components >= 3
                    && f.options.color_transform == Some(true)
                    && f.expected.adobe_app14 == Some(0)
            }),
        ),
        (
            "a 4-component image with ColorTransform true and without APP14",
            has(&|f| {
                f.expected.components == 4
                    && f.options.color_transform == Some(true)
                    && f.expected.adobe_app14.is_none()
            }),
        ),
        (
            "a 3- or 4-component image with ColorTransform false and APP14 transform 1 or 2",
            has(&|f| {
                f.expected.components >= 3
                    && f.options.color_transform == Some(false)
                    && matches!(f.expected.adobe_app14, Some(1 | 2))
            }),
        ),
        (
            "a 3-component image with ColorTransform false and without APP14",
            has(&|f| {
                f.expected.components == 3
                    && f.options.color_transform == Some(false)
                    && f.expected.adobe_app14.is_none()
            }),
        ),
        (
            "a 3-component image without APP14 and without ColorTransform (converted by default)",
            has(&|f| {
                f.options.color_transform.is_none()
                    && f.expected.components == 3
                    && f.expected.adobe_app14.is_none()
            }),
        ),
        (
            "a CMYK image with APP14 transform 2 (YCCK) and without ColorTransform",
            has(&|f| {
                f.options.color_transform.is_none()
                    && f.expected.color == ImageColor::Cmyk
                    && f.expected.adobe_app14 == Some(2)
            }),
        ),
    ];
    for (what, ok) in coverage {
        report.check("image.option_coverage", ok, || {
            format!("fixtures must include {what}")
        });
    }
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

/// ColorTransform must be honoured. Changing it never makes a JPEG undecodable, so the
/// opposite value must decode `Ok` with different pixels (an error would hide an adapter that
/// refuses rather than applies the option). Without an explicit value, the implied one must
/// reproduce the fixture and the opposite must differ.
fn color_transform(
    report: &mut Report,
    decoder: &dyn JpegDecoder,
    f: &JpegFixture,
    spec: &FixtureSpec<'_, DecodedImage>,
) {
    let unlimited = DecodeLimits::default();
    let with = |value: bool| {
        let options = JpegOptions {
            color_transform: Some(value),
        };
        guarded(|| decoder.decode(&f.input, &options, &unlimited))
    };
    let applied = f
        .options
        .color_transform
        .unwrap_or_else(|| implied_transform(f.expected.components, f.expected.adobe_app14));
    if f.options.color_transform.is_none() {
        let result = with(applied);
        let ok = matches!(&result, Some(Ok(d))
            if d.status == DecodeStatus::Complete && d.value.matches(&f.expected, f.tolerance))
            && ok_result_valid(&result, &unlimited);
        report.check("image.option_used", ok, || {
            format!(
                "{}: no ColorTransform must decode like ColorTransform {applied} (the APP14 \
                 flag, else 3 components convert), got {}",
                f.name,
                describe_raster(&result)
            )
        });
    }
    let result = with(!applied);
    let decoded = matches!(&result, Some(Ok(d))
        if d.status == DecodeStatus::Complete
            && ok_result_valid(&result, &unlimited)
            && same_header(&d.value, &f.expected));
    check_option_used(
        report,
        spec,
        &format!("ColorTransform {}", !applied),
        &result,
    );
    report.check("image.option_used", decoded, || {
        format!(
            "{}: ColorTransform {} must still decode Complete with the fixture's header, got {}",
            f.name,
            !applied,
            describe_raster(&result)
        )
    });
}

/// Everything but the sample values matches.
fn same_header(a: &DecodedImage, b: &DecodedImage) -> bool {
    a.width == b.width
        && a.height == b.height
        && a.components == b.components
        && a.bits_per_component == b.bits_per_component
        && a.source_bits == b.source_bits
        && a.adobe_app14 == b.adobe_app14
        && a.color == b.color
        && a.has_alpha == b.has_alpha
        && a.samples.len() == b.samples.len()
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
    let mut heavy = false;
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
            refused: None,
            extra_work: f.extra_work.as_ref(),
        };
        let valid = run_fixture(&mut report, &spec, &decode);
        heavy |= valid && spec.heavy();
    }
    work_coverage(&mut report, heavy);
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
    let mut heavy = false;
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
            refused: None,
            extra_work: f.extra_work.as_ref(),
        };
        let valid = run_fixture(&mut report, &spec, &decode);
        heavy |= valid && spec.heavy();
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
    work_coverage(&mut report, heavy);
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
            refused: (f.salvage.is_some() && f.params.rows == 0)
                .then_some("a salvage cut needs Rows set (with Rows 0 a cut can be a valid end)"),
            extra_work: None,
        };
        if !run_fixture(&mut report, &spec, &decode) {
            continue;
        }
        ccitt_params(&mut report, decoder, f);

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
        // K = 1 makes every line 1-D (Table 11), so 2-D coding needs K ≥ 2.
        (
            "K ≥ 2 (Group 3 2-D)",
            fixtures.iter().any(|f| f.params.k >= 2),
        ),
        (
            "EncodedByteAlign and K < 0",
            fixtures
                .iter()
                .any(|f| f.params.encoded_byte_align && f.params.k < 0),
        ),
        (
            "EncodedByteAlign and K ≥ 0",
            fixtures
                .iter()
                .any(|f| f.params.encoded_byte_align && f.params.k >= 0),
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
    let probes: [(&str, CcittParams, DecodeLimits, bool); 9] = [
        (
            "Columns and Rows at max_image_dimension",
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
        (
            "Columns u32::MAX with Rows set",
            CcittParams {
                columns: u32::MAX,
                ..sized
            },
            unlimited,
            true,
        ),
        // Both at the dimension limit: the product (2^40 with the defaults) overflows u32, so
        // an adapter multiplying in u32 wraps past the pixel check (or panics).
        (
            "Columns and Rows at max_image_dimension, product above max_image_pixels",
            CcittParams {
                columns: unlimited.max_image_dimension,
                rows: unlimited.max_image_dimension,
                ..f.params
            },
            unlimited,
            true,
        ),
        (
            "Rows u32::MAX",
            CcittParams {
                rows: u32::MAX,
                ..sized
            },
            unlimited,
            true,
        ),
    ];
    for (label, params, limits, want_limit) in probes {
        let result = guarded(|| decoder.decode(&[], &params, &limits));
        let ok = match &result {
            Some(Err(BackendError::LimitExceeded(_))) => want_limit,
            Some(_) => !want_limit && ok_result_valid(&result, &limits),
            None => false,
        };
        report.check("image.ccitt_param_limits", ok, || {
            format!(
                "{name}: {label}, empty data: expected {}, got {}",
                if want_limit {
                    "LimitExceeded"
                } else {
                    "no LimitExceeded (and any output valid)"
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
        // The most rows the limits allow for this width: a decoder must stop at the data's
        // end (or the output cap), not allocate or loop for every promised row.
        (
            "Rows at the largest value the limits allow",
            CcittParams {
                rows: u32::try_from(
                    (capped.max_image_pixels / u64::from(width.max(1)))
                        .min(u64::from(capped.max_image_dimension)),
                )
                .unwrap_or(u32::MAX),
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
/// different, valid image passes ([`ok_result_valid`] at the default limits).
fn check_option_used<R: Raster>(
    report: &mut Report,
    spec: &FixtureSpec<'_, R>,
    label: &str,
    result: &Outcome<R>,
) {
    let same = matches!(result, Some(Ok(d)) if d.value.matches(spec.expected, spec.tolerance));
    // Option probes run at the default limits; any Ok must still be valid (usage included).
    let valid = ok_result_valid(result, &DecodeLimits::default());
    report.check("image.option_used", valid && !same, || {
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
    /// Output samples: width × height × components (bitmaps: one per pixel), saturating.
    fn samples(&self) -> u64 {
        u64::from(self.width())
            .saturating_mul(u64::from(self.height()))
            .saturating_mul(u64::from(self.components().unwrap_or(1)))
    }
    /// Every sample is below `2^source_bits` and padding bits are 0.
    fn samples_in_range(&self) -> bool;
    /// The image is not flat: samples span at least half the largest value (images), or both black
    /// and white pixels occur (bitmaps). A flat fixture would let a constant decoder pass.
    fn varied(&self) -> bool;
    /// [`Raster::varied`] for the first `rows` rows alone.
    fn rows_varied(&self, rows: u32) -> bool;
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
        // Half the largest value, at least 1: 7 for 4-bit, 127 for 8-bit samples. Every
        // component on its own, or a grey RGB image would hide swapped or wrongly converted
        // channels, and a CMYK image with K all 0 would hide a dropped K.
        let half = 1u32
            .checked_shl(u32::from(self.source_bits))
            .map_or(u32::MAX, |range| ((range - 1) / 2).max(1));
        // An alpha channel (last component) may be flat: mostly opaque masks are normal.
        let step = usize::from(self.components.max(1));
        let colour = if self.has_alpha {
            step.saturating_sub(1).max(1)
        } else {
            step
        };
        (0..colour).all(|c| {
            let (min, max) = self
                .values()
                .skip(c)
                .step_by(step)
                .fold((u16::MAX, 0u16), |(lo, hi), v| (lo.min(v), hi.max(v)));
            max >= min && u32::from(max - min) >= half
        })
    }
    fn rows_varied(&self, rows: u32) -> bool {
        row_prefix(self.samples.len(), self.height, rows)
            .and_then(|n| self.samples.get(..n))
            .is_some_and(|samples| {
                DecodedImage {
                    width: self.width,
                    height: rows,
                    components: self.components,
                    bits_per_component: self.bits_per_component,
                    source_bits: self.source_bits,
                    color: ImageColor::Unknown,
                    has_alpha: self.has_alpha,
                    adobe_app14: None,
                    samples: samples.to_vec(),
                }
                .varied()
            })
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
    fn rows_varied(&self, rows: u32) -> bool {
        row_prefix(self.data.len(), self.height, rows)
            .and_then(|n| self.data.get(..n))
            .is_some_and(|data| {
                Bitmap {
                    width: self.width,
                    height: rows,
                    data: data.to_vec(),
                }
                .varied()
            })
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
    /// A format-specific reason the fixture is refused, if any.
    refused: Option<&'static str>,
    /// The fixture's extra-work claim, if any.
    extra_work: Option<&'a ExtraWork>,
}

impl<R: Raster> FixtureSpec<'_, R> {
    /// Whether the fixture claims at least [`HEAVY_WORK_FACTOR`] extra operations per output
    /// sample.
    fn heavy(&self) -> bool {
        self.extra_work.is_some_and(|w| {
            w.min_extra_ops >= self.expected.samples().saturating_mul(HEAVY_WORK_FACTOR)
        })
    }
}

/// At least one valid fixture must need work beyond its output (`heavy`: whether any fixture
/// is valid and [`FixtureSpec::heavy`]).
fn work_coverage(report: &mut Report, heavy: bool) {
    report.check("image.work_fixture", heavy, || {
        format!(
            "fixtures must include a valid one with an extra_work claim of at least \
             {HEAVY_WORK_FACTOR} × its output samples (work that produces no output: scans, \
             layers, symbol instances)"
        )
    });
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
    if let Some(reason) = spec.refused {
        return Some(reason.into());
    }
    if let Some(w) = spec.extra_work {
        let max_ops = DecodeLimits::default().max_ops;
        if w.min_extra_ops > max_ops {
            return Some(format!(
                "min_extra_ops {} is above the default max_decode_ops {max_ops}",
                w.min_extra_ops
            ));
        }
        if w.lighter.as_slice() == spec.input {
            return Some("the extra_work lighter input is the fixture's own input".into());
        }
        // A longer heavy input would let a per-input-byte rate pass as extra work; pad the
        // lighter stream (JPEG COM segment, JP2 free box, JBIG2 extension segment).
        if w.lighter.len() < spec.input.len() {
            return Some(format!(
                "the extra_work lighter input ({} bytes) is shorter than the input ({} bytes)",
                w.lighter.len(),
                spec.input.len()
            ));
        }
    }
    if let Some(s) = spec.salvage {
        // A cut at 0 is empty input, which must be an error.
        if s.cut == 0 || s.cut >= spec.input.len() || s.rows == 0 || s.rows > e.height() {
            return Some(format!(
                "salvage cut {} must be within 1..{} and rows {} within 1..={}",
                s.cut,
                spec.input.len(),
                s.rows,
                e.height()
            ));
        }
        // Blank salvaged rows (a white top margin) would let a decoder that salvages a blank
        // image pass.
        if !e.rows_varied(s.rows) {
            return Some(format!(
                "the {} salvaged rows are flat; cut later or pick rows with content",
                s.rows
            ));
        }
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
    // Usage never depends on the limits (specs/core/decode-work-limits.md), so every at-limit
    // success must report what the default-limits decode reported.
    let reference = match &result {
        Some(Ok(d)) => Some(d.usage),
        _ => None,
    };
    let same_usage = |d: &Decoded<R>| reference.is_none_or(|u| d.usage == u);
    for (check, at, set) in boundaries {
        Boundary { check, name, at }.probe(
            report,
            set,
            |limits| call(input, limits),
            |d: &Decoded<R>| complete_match(d) && same_usage(d),
            describe_raster,
        );
    }

    // Working memory and operations (specs/core/decode-work-limits.md): lower bounds on what
    // the decoder reported, then exact boundaries at it.
    if let Some(usage) = reference {
        let samples = expected.samples();
        let len_u64 = u64::try_from(len).unwrap_or(u64::MAX);
        // The default-limits decode itself must fit its limits too (an adapter that checks
        // only non-default limits would otherwise pass).
        report.check(
            "image.usage_valid",
            usage.working_bytes >= len_u64
                && usage.ops >= samples
                && ok_result_valid(&result, &unlimited),
            || {
                format!(
                    "{name}: usage {usage:?} must have working_bytes >= the {len_u64} output \
                     bytes and ops >= the {samples} output samples, and fit the default limits"
                )
            },
        );
        // The extra-work claim: the lighter encoding decodes to the same image, and the
        // difference in reported operations is at least the claim.
        if let Some(w) = spec.extra_work {
            let lighter = call(&w.lighter, &unlimited);
            let ok = match &lighter {
                Some(Ok(l)) => {
                    complete_match(l)
                        && ok_result_valid(&lighter, &unlimited)
                        && usage
                            .ops
                            .checked_sub(l.usage.ops)
                            .is_some_and(|extra| extra >= w.min_extra_ops)
                }
                _ => false,
            };
            report.check("image.extra_work", ok, || {
                format!(
                    "{name}: the lighter input must decode Complete to the expected image with at \
                     least {} fewer ops than the {} reported for the input, got {}",
                    w.min_extra_ops,
                    usage.ops,
                    describe_raster(&lighter)
                )
            });
        }
        let work: [(&'static str, u64, SetFn); 2] = [
            ("image.working_limit", usage.working_bytes, |n| {
                DecodeLimits {
                    max_working_bytes: n,
                    ..DecodeLimits::default()
                }
            }),
            ("image.ops_limit", usage.ops, |n| DecodeLimits {
                max_ops: n,
                ..DecodeLimits::default()
            }),
        ];
        for (check, at, set) in work {
            Boundary { check, name, at }.probe(
                report,
                set,
                |limits| call(input, limits),
                |d: &Decoded<R>| complete_match(d) && same_usage(d),
                describe_raster,
            );
        }
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

/// An error, or output whose length matches its header, which stays within `limits`, and whose
/// usage is within `limits` and at least the output's bytes and samples.
fn ok_result_valid<R: Raster>(result: &Outcome<R>, limits: &DecodeLimits) -> bool {
    match result {
        None => false,
        Some(Err(_)) => true,
        Some(Ok(d)) => {
            let r = &d.value;
            let pixels = u64::from(r.width()).saturating_mul(u64::from(r.height()));
            let len = u64::try_from(r.byte_len()).unwrap_or(u64::MAX);
            // Usage within the limits and above both lower bounds
            // (specs/core/decode-work-limits.md).
            let usage_ok = d.usage.working_bytes <= limits.max_working_bytes
                && d.usage.ops <= limits.max_ops
                && d.usage.working_bytes >= len
                && d.usage.ops >= r.samples();
            usage_ok
                && r.expected_len() == Some(r.byte_len())
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
            "Ok({:?}, {}x{}, {} bytes, {} working bytes, {} ops)",
            d.status,
            d.value.width(),
            d.value.height(),
            d.value.byte_len(),
            d.usage.working_bytes,
            d.usage.ops
        ),
        other => describe(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecodeUsage;

    /// A toy codec standing in for every format: `width u16 BE, height u16 BE, flags u8, then
    /// samples`. The suite is format-independent, so this exercises every check.
    ///
    /// Flags: the low nibble is the component count. The high nibble is the source depth
    /// (0 means 8, 1–8 as is), or an 8-bit image with an "APP14" transform flag: 9 → 0,
    /// 10 → 2, 11 → 1. A "transform" XORs every sample with 0x40; JPEG decodes it when
    /// ColorTransform says so, else when the APP14 flag is 1 or 2.
    ///
    /// Usage: working bytes are the output plus 64 ("tables"); operations are one per output
    /// sample plus [`TOY_OPS_PER_UNIT`] per unit of value in the bytes after the samples (a
    /// "repeat count" standing in for work that produces no output; it depends on what the
    /// bytes say, not on how many there are).
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
        /// CCITT: computes Columns × Rows in u32 (wrapping) for the pixel check.
        ccitt_u32_product: bool,
        /// JPEG: refuses (Unsupported) an explicit ColorTransform that contradicts APP14,
        /// instead of applying it.
        err_on_contradiction: bool,
        /// JPEG: reads a missing ColorTransform as false.
        none_as_false: bool,
        /// Never checks the working-memory and operations limits.
        ignore_work_limits: bool,
        /// Rejects usage equal to a work limit (`>=` instead of `>`).
        work_limits_inclusive: bool,
        /// Reports one operation fewer whenever `max_ops` isn't the default (usage that
        /// depends on the limits).
        usage_from_limits: bool,
        /// Leaves the output out of working memory. The toy's 64 table bytes would still cover
        /// its small outputs, so it reports 0: then the exact boundary (at 0) still passes and
        /// only the lower bound catches it.
        working_without_output: bool,
        /// Counts only output samples, not the trailing-byte work.
        ops_output_only: bool,
        /// Charges a flat 100 operations per output sample and nothing for the trailing-byte
        /// work (a per-sample overcount that covers any absolute bound).
        flat_ops_per_sample: bool,
        /// Charges 256 operations per input byte instead of the trailing values (a per-byte
        /// rate: a longer heavy input would cover any claim).
        ops_per_input_byte: bool,
        /// Reports one operation more whenever `max_output_bytes` isn't the default.
        usage_from_output_limit: bool,
        /// CCITT: reports 0 working bytes for Group 4 without EncodedByteAlign (a stream only
        /// the option checks decode).
        ccitt_option_usage_zero: bool,
        /// At exactly the default limits, skips the work checks and reports `max_ops + 1`
        /// operations.
        default_usage_over_limit: bool,
        /// Reports one operation more whenever the pixel, dimension, component or working-bytes
        /// limit isn't the default.
        usage_from_other_limits: bool,
        /// Reports one operation fewer than the output samples (and no trailing work).
        ops_below_samples: bool,
        /// JPEG: reports 0 working bytes for an explicit ColorTransform true on a 3-component
        /// image without APP14 (only the "implied value" probe and one flip decode that way).
        jpeg_option_usage_zero: bool,
        /// JBIG2: reports 0 working bytes when the globals are missing or shorter than 4
        /// bytes (only the "globals left out" probe and truncated globals decode that way).
        jbig2_globals_usage_zero: bool,
        /// Reports `max_ops + 1` operations on truncated results.
        truncated_usage_over_limit: bool,
    }

    /// Toy operations per unit of trailing-byte value.
    const TOY_OPS_PER_UNIT: u64 = 64;

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
        /// `packed`: one bit per pixel, rows padded to bytes (bitmaps). `jpeg`: the caller's
        /// ColorTransform for JPEG (`Some(None)` lets the APP14 flag decide); `None` for the
        /// other codecs, which never transform.
        fn decode_image(
            &self,
            input: &[u8],
            limits: &DecodeLimits,
            packed: bool,
            jpeg: Option<Option<bool>>,
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
            let per_pixel = if packed { 1 } else { u64::from(components) };
            let samples = u64::from(width) * u64::from(height) * per_pixel;
            let usage = self.charge(input, len, samples, limits)?;
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
            let implied = implied_transform(components, adobe_app14);
            let transform = match jpeg {
                None => false,
                Some(_) if self.ignore_options => implied,
                Some(Some(t)) => {
                    if self.err_on_contradiction && adobe_app14.is_some() && t != implied {
                        return Err(BackendError::Unsupported("ColorTransform vs APP14"));
                    }
                    t
                }
                Some(None) if self.none_as_false => false,
                Some(None) => implied,
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
            let usage = if self.truncated_usage_over_limit && status == DecodeStatus::Truncated {
                DecodeUsage {
                    ops: limits.max_ops.saturating_add(1),
                    ..usage
                }
            } else {
                usage
            };
            Ok(Decoded {
                usage,
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

        /// Computes and checks the toy's usage before "decoding" (see [`Toy`]).
        /// `len`: output bytes; `samples`: output samples (bitmaps: pixels).
        fn charge(
            &self,
            input: &[u8],
            len: usize,
            samples: u64,
            limits: &DecodeLimits,
        ) -> Result<DecodeUsage, BackendError> {
            let over = BackendError::LimitExceeded("toy usage");
            let len = u64::try_from(len).map_err(|_| over.clone())?;
            let start = usize::try_from(len)
                .map_err(|_| over.clone())?
                .saturating_add(5);
            let units: u64 = input
                .get(start..)
                .unwrap_or(&[])
                .iter()
                .map(|&b| u64::from(b))
                .sum();
            let samples = if self.flat_ops_per_sample {
                samples.checked_mul(100).ok_or(over.clone())?
            } else {
                samples
            };
            let extra = if self.ops_output_only || self.flat_ops_per_sample {
                0
            } else if self.ops_per_input_byte {
                u64::try_from(input.len())
                    .ok()
                    .and_then(|n| n.checked_mul(256))
                    .ok_or(over.clone())?
            } else {
                units.checked_mul(TOY_OPS_PER_UNIT).ok_or(over.clone())?
            };
            let mut usage = DecodeUsage {
                working_bytes: if self.working_without_output {
                    0
                } else {
                    len.checked_add(64).ok_or(over.clone())?
                },
                ops: samples.checked_add(extra).ok_or(over.clone())?,
            };
            let exceeds = |used: u64, cap: u64| {
                if self.work_limits_inclusive {
                    used >= cap
                } else {
                    used > cap
                }
            };
            let at_default = *limits == DecodeLimits::default();
            if self.default_usage_over_limit && at_default {
                usage.ops = limits.max_ops.saturating_add(1);
                return Ok(usage);
            }
            if !self.ignore_work_limits
                && (exceeds(usage.working_bytes, limits.max_working_bytes)
                    || exceeds(usage.ops, limits.max_ops))
            {
                return Err(over);
            }
            if self.usage_from_limits && limits.max_ops != DecodeLimits::default().max_ops {
                usage.ops = usage.ops.saturating_sub(1);
            }
            if self.ops_below_samples {
                usage.ops = samples.saturating_sub(1);
            }
            let d = DecodeLimits::default();
            if self.usage_from_other_limits
                && (limits.max_image_pixels != d.max_image_pixels
                    || limits.max_image_dimension != d.max_image_dimension
                    || limits.max_image_components != d.max_image_components
                    || limits.max_working_bytes != d.max_working_bytes)
            {
                usage.ops = usage.ops.saturating_add(1);
            }
            if self.usage_from_output_limit
                && limits.max_output_bytes != DecodeLimits::default().max_output_bytes
            {
                usage.ops = usage.ops.saturating_add(1);
            }
            Ok(usage)
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
                usage: d.usage,
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
            let mut d = self.decode_image(input, limits, false, Some(options.color_transform))?;
            if self.jpeg_option_usage_zero
                && options.color_transform == Some(true)
                && d.value.components == 3
                && d.value.adobe_app14.is_none()
            {
                d.usage.working_bytes = 0;
            }
            Ok(d)
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
            let mut d = self.decode_bitmap(input, limits, key)?;
            if self.jbig2_globals_usage_zero && globals.is_none_or(|g| g.len() < 4) {
                d.usage.working_bytes = 0;
            }
            Ok(d)
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
                let product = if self.ccitt_u32_product {
                    u64::from(params.columns.wrapping_mul(params.rows))
                } else {
                    u64::from(params.columns) * u64::from(params.rows)
                };
                if params.columns > limits.max_image_dimension
                    || (rows_checked
                        && (params.rows > limits.max_image_dimension
                            || product > limits.max_image_pixels))
                {
                    return Err(BackendError::LimitExceeded("CCITT parameters"));
                }
            }
            if self.panic_on_extreme_k && params.k == i32::MIN {
                panic!("K");
            }
            let mut d = self.decode_bitmap(input, limits, ccitt_key(params))?;
            if self.ccitt_option_usage_zero && params.k < 0 && !params.encoded_byte_align {
                d.usage.working_bytes = 0;
            }
            Ok(d)
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

    /// Makes a fixture need heavy work: a trailing "repeat count" byte of 4, which the toy
    /// charges 4 × [`TOY_OPS_PER_UNIT`] = 256 for and otherwise ignores. The lighter input has
    /// the same length with a count of 0, and the claim is the toy's exact difference. Never
    /// use it on a fixture with a salvage cut (the cut would fall in the trailing byte).
    fn add_work(input: &mut Vec<u8>) -> Option<ExtraWork> {
        let mut lighter = input.clone();
        lighter.push(0);
        input.push(4);
        Some(ExtraWork {
            lighter,
            min_extra_ops: 4 * TOY_OPS_PER_UNIT,
        })
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
        let transform = color_transform
            .unwrap_or_else(|| implied_transform(expected.components, expected.adobe_app14));
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
            extra_work: None,
        }
    }

    /// - landscape 4×3 RGB, APP14 1, ColorTransform true (salvage cut);
    /// - portrait 2×5 RGB, APP14 1, ColorTransform false (overrides APP14);
    /// - square 3×3 CMYK, APP14 2 (YCCK), no ColorTransform;
    /// - square 4×4 RGB, no APP14, no ColorTransform (converted by default; heavy work);
    /// - portrait 2×3 CMYK, no APP14, ColorTransform true (explicit, 4 components, overrides
    ///   the implied "no transform");
    /// - portrait 2×5 RGB, APP14 0, ColorTransform true (overrides APP14);
    /// - portrait 2×5 RGB, no APP14, ColorTransform false (overrides the default conversion).
    fn jpeg_fixtures() -> Vec<JpegFixture> {
        let mut heavy = jpeg_fixture((4, 4), 0x03, None, false);
        heavy.extra_work = add_work(&mut heavy.input);
        vec![
            jpeg_fixture((4, 3), 0xb3, Some(true), true),
            jpeg_fixture((2, 5), 0xb3, Some(false), false),
            jpeg_fixture((3, 3), 0xa4, None, false),
            heavy,
            jpeg_fixture((2, 3), 0x04, Some(true), false),
            jpeg_fixture((2, 5), 0x93, Some(true), false),
            jpeg_fixture((2, 5), 0x03, Some(false), false),
        ]
    }

    /// Landscape 4×3 (salvage cut) and portrait 2×5 RGB (heavy work), plus a 4-bit 5×2 grey
    /// image (samples 0–15); lossless.
    fn jpx_fixtures() -> Vec<Jpeg2000Fixture> {
        [
            ((4, 3), 0x03, true),
            ((2, 5), 0x03, false),
            ((5, 2), 0x41, false),
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
                extra_work: None,
            }
        })
        .map(|mut f| {
            if f.expected.height == 5 {
                f.extra_work = add_work(&mut f.input);
            }
            f
        })
        .collect()
    }

    /// Landscape 12×2 and portrait 3×9 (heavy work) pages that use their globals (first byte 0x80, the
    /// toy's XOR key). `key` is what the input is stored with: 0x80, or 0 for pages that
    /// don't depend on their globals.
    fn jbig2_fixtures(key: u8) -> Vec<Jbig2Fixture> {
        [(12, 2), (3, 9)]
            .into_iter()
            .map(|(w, h)| {
                let (input, expected) = toy_bitmap(w, h, key);
                let mut f = Jbig2Fixture {
                    name: format!("toy {w}x{h}"),
                    input,
                    globals: Some(vec![0x80, 2, 3, 4]),
                    expected,
                    extra_work: None,
                };
                if h == 9 {
                    f.extra_work = add_work(&mut f.input);
                }
                f
            })
            .collect()
    }

    /// Landscape 12×2 Group 4 with EncodedByteAlign and Rows set (salvage cut), portrait 3×9
    /// Group 3 1-D with EncodedByteAlign, landscape 10×3 Group 3 2-D (K 2). `keyed`: inputs
    /// stored with the toy's K/alignment key (false for streams that decode the same under
    /// any K).
    fn ccitt_fixtures(keyed: bool) -> Vec<CcittFixture> {
        [
            ((12, 2), -1, true, true),
            ((3, 9), 0, true, false),
            ((10, 3), 2, false, false),
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

    /// Every suite's report for `toy`, with the fixture sets a correct decoder passes.
    fn every_suite(toy: &Toy) -> [Report; 4] {
        [
            jpeg(toy),
            run_jpeg2000(toy, &jpx_fixtures()),
            run_jbig2(toy, &jbig2_fixtures(0x80)),
            run_ccitt(toy, &ccitt_fixtures(true)),
        ]
    }

    #[test]
    fn ignoring_or_misreading_the_work_limits_is_caught() {
        // Ignored: one below and 0 succeed. Inclusive: the limit itself fails.
        for toy in [
            Toy {
                ignore_work_limits: true,
                ..Toy::default()
            },
            Toy {
                work_limits_inclusive: true,
                ..Toy::default()
            },
        ] {
            for report in every_suite(&toy) {
                assert_eq!(
                    report.failed_checks(),
                    vec!["image.ops_limit", "image.working_limit"],
                    "{report}"
                );
            }
        }
    }

    #[test]
    fn usage_that_depends_on_the_limits_is_caught() {
        let toy = Toy {
            usage_from_limits: true,
            ..Toy::default()
        };
        for report in every_suite(&toy) {
            assert_eq!(report.failed_checks(), vec!["image.ops_limit"], "{report}");
        }
    }

    #[test]
    fn working_memory_without_the_output_is_caught() {
        let toy = Toy {
            working_without_output: true,
            ..Toy::default()
        };
        for report in every_suite(&toy) {
            assert!(report.failed("image.usage_valid"), "{report}");
            // Every Ok result is checked too, not only the fixture decode.
            assert!(report.failed("image.truncated"), "{report}");
            assert!(report.failed("image.mutated"), "{report}");
        }
    }

    #[test]
    fn counting_only_output_samples_is_caught() {
        // Exactly one per sample, or a flat 100 per sample: either way the lighter input costs
        // the same, so the difference is 0.
        for toy in [
            Toy {
                ops_output_only: true,
                ..Toy::default()
            },
            Toy {
                flat_ops_per_sample: true,
                ..Toy::default()
            },
            // Same-length inputs: a per-byte rate gives no difference either.
            Toy {
                ops_per_input_byte: true,
                ..Toy::default()
            },
        ] {
            let [jpeg, jpx, jbig2, ccitt] = every_suite(&toy);
            for report in [jpeg, jpx, jbig2] {
                assert_eq!(report.failed_checks(), vec!["image.extra_work"], "{report}");
            }
            // CCITT needs no heavy-work fixture (linear work), so it passes.
            assert!(ccitt.passed(), "{ccitt}");
        }
    }

    #[test]
    fn over_limit_usage_at_the_default_limits_is_caught() {
        let toy = Toy {
            default_usage_over_limit: true,
            ..Toy::default()
        };
        // The fixture decode itself is over its limits (the other checks see it too, through
        // usage that differs from the at-limit probes, but usage_valid names it).
        for report in every_suite(&toy) {
            assert!(report.failed("image.usage_valid"), "{report}");
        }
    }

    #[test]
    fn usage_that_depends_on_the_other_limits_is_caught() {
        let toy = Toy {
            usage_from_other_limits: true,
            ..Toy::default()
        };
        let [jpeg, jpx, jbig2, ccitt] = every_suite(&toy);
        for report in [jpeg, jpx] {
            assert_eq!(
                report.failed_checks(),
                vec![
                    "image.component_limit",
                    "image.dimension_limit",
                    "image.pixel_limit",
                    "image.working_limit"
                ],
                "{report}"
            );
        }
        // Bitmaps have no component probe.
        for report in [jbig2, ccitt] {
            assert_eq!(
                report.failed_checks(),
                vec![
                    "image.dimension_limit",
                    "image.pixel_limit",
                    "image.working_limit"
                ],
                "{report}"
            );
        }
    }

    #[test]
    fn fewer_ops_than_output_samples_are_caught() {
        let toy = Toy {
            ops_below_samples: true,
            ..Toy::default()
        };
        for report in every_suite(&toy) {
            assert!(report.failed("image.usage_valid"), "{report}");
            // Every Ok result is checked too, not only the fixture decode.
            assert!(report.failed("image.truncated"), "{report}");
            assert!(report.failed("image.mutated"), "{report}");
        }
    }

    #[test]
    fn invalid_usage_in_a_jpeg_or_jbig2_option_probe_is_caught() {
        let report = jpeg(&Toy {
            jpeg_option_usage_zero: true,
            ..Toy::default()
        });
        assert!(!report.failed("image.usage_valid"), "{report}");
        assert!(!report.failed("image.fixture_decodes"), "{report}");
        // The implied-value probe of the fixture without ColorTransform has its own message.
        assert!(
            report
                .failures()
                .iter()
                .any(|f| f.check == "image.option_used"
                    && f.detail.contains("no ColorTransform must decode like")),
            "{report}"
        );
        let report = run_jbig2(
            &Toy {
                jbig2_globals_usage_zero: true,
                ..Toy::default()
            },
            &jbig2_fixtures(0x80),
        );
        assert_eq!(
            report.failed_checks(),
            vec!["image.option_used", "image.truncated_globals"],
            "{report}"
        );
    }

    #[test]
    fn usage_that_depends_on_the_output_limit_is_caught() {
        let toy = Toy {
            usage_from_output_limit: true,
            ..Toy::default()
        };
        for report in every_suite(&toy) {
            assert_eq!(
                report.failed_checks(),
                vec!["image.output_limit"],
                "{report}"
            );
        }
    }

    #[test]
    fn invalid_usage_in_an_option_probe_is_caught() {
        let toy = Toy {
            ccitt_option_usage_zero: true,
            ..Toy::default()
        };
        // Only the EncodedByteAlign-false probe of the Group 4 fixture decodes that way.
        let report = run_ccitt(&toy, &ccitt_fixtures(true));
        assert!(report.failed("image.option_used"), "{report}");
        assert!(!report.failed("image.usage_valid"), "{report}");
        assert!(!report.failed("image.fixture_decodes"), "{report}");
    }

    #[test]
    fn over_limit_usage_on_truncated_results_is_caught() {
        let toy = Toy {
            truncated_usage_over_limit: true,
            ..Toy::default()
        };
        for report in every_suite(&toy) {
            assert!(report.failed("image.truncated"), "{report}");
            let failed = report.failed_checks();
            assert!(
                failed
                    .iter()
                    .all(|c| ["image.truncated", "image.salvage"].contains(c)),
                "{report}"
            );
        }
    }

    #[test]
    fn a_heavy_work_fixture_is_required_at_four_times_the_samples() {
        let toy = Toy::default();
        let mut fixtures = jbig2_fixtures(0x80);
        let heavy = fixtures
            .iter()
            .position(|f| f.extra_work.is_some())
            .unwrap();
        let samples = fixtures[heavy].expected.samples();
        // The toy charges 256 for the trailing count, so a claim of 4 × samples is true.
        assert!(HEAVY_WORK_FACTOR * samples <= 4 * TOY_OPS_PER_UNIT);
        let set = |fixtures: &mut Vec<Jbig2Fixture>, n: u64| {
            if let Some(w) = fixtures[heavy].extra_work.as_mut() {
                w.min_extra_ops = n;
            }
        };
        set(&mut fixtures, HEAVY_WORK_FACTOR * samples);
        let report = run_jbig2(&toy, &fixtures);
        assert!(report.passed(), "{report}");
        set(&mut fixtures, HEAVY_WORK_FACTOR * samples - 1);
        let report = run_jbig2(&toy, &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.work_fixture"],
            "{report}"
        );
        fixtures[heavy].extra_work = None;
        let report = run_jbig2(&toy, &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.work_fixture"],
            "{report}"
        );
    }

    #[test]
    fn the_extra_work_claim_is_measured_and_validated() {
        let toy = Toy::default();
        let fixtures = jpx_fixtures();
        let heavy = fixtures
            .iter()
            .position(|f| f.extra_work.is_some())
            .unwrap();
        let with = |change: &dyn Fn(&mut ExtraWork)| {
            let mut fixtures = fixtures.clone();
            if let Some(w) = fixtures[heavy].extra_work.as_mut() {
                change(w);
            }
            run_jpeg2000(&toy, &fixtures)
        };
        // The claim is the toy's exact difference: at it passes, one more fails.
        assert!(with(&|_| {}).passed());
        let report = with(&|w| w.min_extra_ops += 1);
        assert_eq!(report.failed_checks(), vec!["image.extra_work"], "{report}");
        // A lighter input that decodes to another image fails.
        let report = with(&|w| {
            if let Some(b) = w.lighter.get_mut(5) {
                *b ^= 1;
            }
        });
        assert_eq!(report.failed_checks(), vec!["image.extra_work"], "{report}");
        // A lighter input shorter than the input is an invalid fixture.
        let report = with(&|w| {
            w.lighter.pop();
        });
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid", "image.work_fixture"],
            "{report}"
        );
        // One byte longer (padding) is fine: the toy ignores a second count of 0.
        assert!(with(&|w| w.lighter.push(0)).passed());
        // A lighter input equal to the input is an invalid fixture (and then not heavy).
        let input = fixtures[heavy].input.clone();
        let report = with(&|w| w.lighter = input.clone());
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid", "image.work_fixture"],
            "{report}"
        );
        // At the default max_decode_ops the claim is valid but false; one past is invalid.
        let max = DecodeLimits::default().max_ops;
        let report = with(&|w| w.min_extra_ops = max);
        assert_eq!(report.failed_checks(), vec!["image.extra_work"], "{report}");
        let report = with(&|w| w.min_extra_ops = max + 1);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid", "image.work_fixture"],
            "{report}"
        );
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
                "image.salvage_fixture",
                "image.work_fixture"
            ],
            "{report}"
        );
    }

    #[test]
    fn one_orientation_is_not_enough() {
        // The portraits (2×5, 2×3, 2×5, 2×5) turned landscape, with the same options.
        let mut fixtures = jpeg_fixtures();
        fixtures[1] = jpeg_fixture((5, 2), 0xb3, Some(false), false);
        fixtures[4] = jpeg_fixture((3, 2), 0x04, Some(true), false);
        fixtures[5] = jpeg_fixture((5, 2), 0x93, Some(true), false);
        fixtures[6] = jpeg_fixture((5, 2), 0x03, Some(false), false);
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
        assert_eq!(max_lossy_tolerance(12), 2);
        assert_eq!(max_lossy_tolerance(16), 2);
        assert_eq!(max_lossy_tolerance(255), 2);
        // A 12-bit decoder working at 8-bit precision (low 4 bits dropped) is off by up to
        // 15: above the cap.
        let twelve = DecodedImage {
            width: 16,
            height: 1,
            components: 1,
            bits_per_component: 16,
            source_bits: 12,
            adobe_app14: None,
            color: ImageColor::Gray,
            has_alpha: false,
            samples: (0u16..16).flat_map(|i| (i * 257).to_be_bytes()).collect(),
        };
        let eight = DecodedImage {
            samples: (0u16..16)
                .flat_map(|i| ((i * 257) & 0x0ff0).to_be_bytes())
                .collect(),
            ..twelve.clone()
        };
        assert!(twelve.varied());
        assert!(!eight.matches(&twelve, max_lossy_tolerance(12)));
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
        // Every fixture is invalid, so none counts as the heavy-work fixture either.
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid", "image.work_fixture"],
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
        // Every fixture is invalid, so none counts as the heavy-work fixture either.
        let report = with(false, 1);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid", "image.work_fixture"],
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
        for (cut, rows) in [(0, 2), (len, 2), (len - 1, 0), (len - 1, 4)] {
            let mut fixtures = jpeg_fixtures();
            fixtures[0].salvage = Some(Salvage { cut, rows });
            let report = run_jpeg(&Toy::default(), &fixtures);
            assert_eq!(
                report.failed_checks(),
                vec!["image.fixture_valid"],
                "cut {cut} rows {rows}: {report}"
            );
        }
        // The shortest cut that keeps rows 0–1 (12 bytes each) intact passes; one byte less
        // fails the salvage. Row 0 alone is too flat per component, so `rows: 1` is refused.
        let with = |cut, rows| {
            let mut fixtures = jpeg_fixtures();
            fixtures[0].salvage = Some(Salvage { cut, rows });
            run_jpeg(&Toy::default(), &fixtures)
        };
        assert!(with(5 + 24, 2).passed(), "{}", with(5 + 24, 2));
        assert_eq!(with(5 + 23, 2).failed_checks(), vec!["image.salvage"]);
        assert_eq!(with(5 + 24, 1).failed_checks(), vec!["image.fixture_valid"]);
        // A white top row on a fax page: a blank salvage would pass, so it is refused.
        let mut fixtures = ccitt_fixtures(true);
        fixtures[0].expected.data[..2].fill(0);
        let report = run_ccitt(&Toy::default(), &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );

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
        // The fixtures whose explicit ColorTransform overrides the implied one fail outright.
        let report = jpeg(&toy);
        assert_eq!(
            report.failed_checks(),
            vec![
                "image.component_limit",
                "image.dimension_limit",
                "image.fixture_decodes",
                "image.ops_limit",
                "image.option_used",
                "image.output_limit",
                "image.pixel_limit",
                "image.working_limit"
            ],
            "{report}"
        );
        // With only fixtures whose APP14 agrees with the option, the flips still catch it
        // (and the coverage rules refuse such a set).
        let mut fixtures = jpeg_fixtures();
        fixtures.truncate(4);
        fixtures[1] = jpeg_fixture((2, 5), 0x93, Some(false), false);
        let report = run_jpeg(&toy, &fixtures);
        assert_eq!(
            report.failed_checks(),
            vec!["image.option_coverage", "image.option_used"],
            "{report}"
        );
        assert_eq!(
            report
                .failures()
                .iter()
                .filter(|f| f.check == "image.option_used")
                .count(),
            4,
            "{report}"
        );

        // Fixtures that decode the same without their globals or under any K.
        let report = run_jbig2(&toy, &jbig2_fixtures(0));
        assert_eq!(
            report.failed_checks(),
            vec!["image.option_used"],
            "{report}"
        );
        assert_eq!(report.failures().len(), 2, "{report}");
        let report = run_ccitt(&toy, &ccitt_fixtures(false));
        assert_eq!(
            report.failed_checks(),
            vec!["image.option_used"],
            "{report}"
        );
        // Three K swaps and two EncodedByteAlign flips.
        assert_eq!(report.failures().len(), 5, "{report}");
    }

    #[test]
    fn refusing_instead_of_applying_color_transform_is_caught() {
        // The fixture overriding APP14 fails outright, and every flip that contradicts APP14
        // errors instead of decoding.
        let report = jpeg(&Toy {
            err_on_contradiction: true,
            ..Toy::default()
        });
        // The work boundaries need a successful decode to probe at, so they don't run here.
        assert_eq!(
            report.failed_checks(),
            vec![
                "image.component_limit",
                "image.dimension_limit",
                "image.fixture_decodes",
                "image.option_used",
                "image.output_limit",
                "image.pixel_limit"
            ],
            "{report}"
        );
        assert!(
            report
                .failures()
                .iter()
                .filter(|f| f.check == "image.option_used")
                .all(|f| f.detail.contains("must still decode")),
            "{report}"
        );
    }

    #[test]
    fn a_blank_or_invalid_flip_is_caught() {
        // Wraps the toy: on an explicit ColorTransform that contradicts APP14, returns a
        // blank image (or one whose samples don't match its header) instead of applying it.
        struct BadFlip(bool);
        impl JpegDecoder for BadFlip {
            fn decode(
                &self,
                input: &[u8],
                options: &JpegOptions,
                limits: &DecodeLimits,
            ) -> Result<Decoded<DecodedImage>, BackendError> {
                let mut d = JpegDecoder::decode(&Toy::default(), input, options, limits)?;
                let image = &mut d.value;
                if let Some(t) = options.color_transform
                    && image.adobe_app14.is_some()
                    && t != implied_transform(image.components, image.adobe_app14)
                {
                    if self.0 {
                        image.samples.fill(0);
                    } else {
                        image.samples.clear();
                    }
                }
                Ok(d)
            }
        }
        // `BadFlip(true)`: blank. The fixture that overrides APP14 catches it.
        // `BadFlip(false)`: invalid output, also caught by the flips themselves.
        for blank in [true, false] {
            let report = run_jpeg(&BadFlip(blank), &jpeg_fixtures());
            let mut expected = vec![
                "image.component_limit",
                "image.dimension_limit",
                "image.fixture_decodes",
                "image.ops_limit",
                "image.output_limit",
                "image.pixel_limit",
                "image.working_limit",
            ];
            if !blank {
                // The broken output also shows on truncated and mutated input.
                // The fixture decode's own Ok is invalid too (usage_valid checks it whole).
                expected.extend([
                    "image.mutated",
                    "image.option_used",
                    "image.truncated",
                    "image.usage_valid",
                ]);
                expected.sort_unstable();
            }
            assert_eq!(report.failed_checks(), expected, "{report}");
        }
    }

    #[test]
    fn a_missing_color_transform_read_as_false_is_caught() {
        let report = jpeg(&Toy {
            none_as_false: true,
            ..Toy::default()
        });
        // The two fixtures without ColorTransform decode unconverted.
        assert_eq!(
            report.failed_checks(),
            vec![
                "image.component_limit",
                "image.dimension_limit",
                "image.extra_work",
                "image.fixture_decodes",
                "image.ops_limit",
                "image.output_limit",
                "image.pixel_limit",
                "image.working_limit"
            ],
            "{report}"
        );
        assert!(
            report
                .failures()
                .iter()
                .all(|f| f.detail.starts_with("toy 3x3") || f.detail.starts_with("toy 4x4")),
            "{report}"
        );
    }

    #[test]
    fn option_coverage_is_required() {
        let toy = Toy::default();
        // Each JPEG rule in turn: one fixture swapped so that only that rule fails.
        let cases: [(&str, Vec<(usize, JpegFixture)>); 6] = [
            (
                "ColorTransform true and APP14 transform 0",
                vec![(5, jpeg_fixture((2, 5), 0xb3, Some(true), false))],
            ),
            (
                "ColorTransform true and without APP14",
                vec![(4, jpeg_fixture((2, 3), 0xa4, Some(true), false))],
            ),
            (
                "ColorTransform false and APP14 transform 1 or 2",
                vec![(1, jpeg_fixture((2, 5), 0x93, Some(false), false))],
            ),
            (
                "ColorTransform false and without APP14",
                vec![(6, jpeg_fixture((2, 5), 0x93, Some(false), false))],
            ),
            (
                "converted by default",
                vec![(3, jpeg_fixture((4, 4), 0xb3, None, false))],
            ),
            (
                "(YCCK) and without ColorTransform",
                vec![(2, jpeg_fixture((3, 3), 0xa4, Some(true), false))],
            ),
        ];
        for (rule, replacements) in cases {
            let mut fixtures = jpeg_fixtures();
            for (index, mut replacement) in replacements {
                // Keep the heavy-work fixture heavy, so only the option rule is missing.
                if fixtures[index].extra_work.is_some() {
                    replacement.extra_work = add_work(&mut replacement.input);
                }
                fixtures[index] = replacement;
            }
            let report = run_jpeg(&toy, &fixtures);
            assert_eq!(
                report.failed_checks(),
                vec!["image.option_coverage"],
                "{rule}: {report}"
            );
            assert_eq!(report.failures().len(), 1, "{rule}: {report}");
            assert!(
                report.failures()[0].detail.contains(rule),
                "{rule}: {report}"
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
        // Each CCITT scheme in turn replaced: aligned Group 4 by plain 1-D, aligned 1-D by
        // plain Group 4, K 2 by K 1 (every line 1-D, so it doesn't count as 2-D).
        for (i, k) in [(0, 0), (1, -1), (2, 1)] {
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
        // Per fixture: the pixel product one past, both at the dimension limit, and Rows
        // u32::MAX; plus Rows one past the dimension on the portrait fixture.
        let details: Vec<&str> = report
            .failures()
            .iter()
            .map(|f| f.detail.as_str())
            .collect();
        assert_eq!(details.len(), 10, "{report}");
        assert!(details.iter().all(|d| d.contains("Rows")), "{report}");
        assert_eq!(
            details
                .iter()
                .filter(|d| d.contains("Rows one past max_image_dimension"))
                .count(),
            1,
            "{report}"
        );

        let report = run_ccitt(
            &Toy {
                ccitt_u32_product: true,
                ..Toy::default()
            },
            &ccitt_fixtures(true),
        );
        assert_eq!(
            report.failed_checks(),
            vec!["image.ccitt_param_limits"],
            "{report}"
        );
        assert!(
            report
                .failures()
                .iter()
                .all(|f| f.detail.contains("product above max_image_pixels")),
            "{report}"
        );
        assert_eq!(report.failures().len(), 3, "{report}");
    }

    #[test]
    fn flatness_threshold_is_exact_and_per_component() {
        let image = |components: u16, source_bits: u8, samples: Vec<u8>| DecodedImage {
            width: u32::try_from(samples.len()).unwrap() / u32::from(components),
            height: 1,
            components,
            bits_per_component: 8,
            source_bits,
            adobe_app14: None,
            color: ImageColor::Unknown,
            has_alpha: false,
            samples,
        };
        assert!(image(1, 8, vec![0, 127]).varied());
        assert!(!image(1, 8, vec![0, 126]).varied());
        assert!(image(1, 4, vec![3, 10]).varied());
        assert!(!image(1, 4, vec![3, 9]).varied());
        assert!(image(1, 1, vec![0, 1]).varied());
        assert!(!image(1, 1, vec![1, 1]).varied());
        // RGB: the third component flat.
        assert!(!image(3, 8, vec![0, 0, 5, 200, 200, 5]).varied());
        assert!(image(3, 8, vec![0, 0, 5, 200, 200, 255]).varied());
        // Rows: only the first row is checked.
        let two_rows = DecodedImage {
            width: 2,
            height: 2,
            ..image(1, 8, vec![9, 9, 0, 255])
        };
        assert!(two_rows.varied());
        assert!(!two_rows.rows_varied(1));
        assert!(two_rows.rows_varied(2));
        // A flat alpha channel is fine; a flat colour channel is not.
        let gray_alpha = |gray: [u8; 2]| DecodedImage {
            has_alpha: true,
            ..image(2, 8, vec![gray[0], 255, gray[1], 255])
        };
        assert!(gray_alpha([0, 200]).varied());
        assert!(!gray_alpha([100, 120]).varied());
    }

    #[test]
    fn a_salvage_of_every_row_is_accepted() {
        // The toy pads a missing byte with 0, so a fixture whose last sample is 0 salvages
        // all its rows: rows equal to the height is in range.
        let mut fixtures = jpx_fixtures();
        let f = &mut fixtures[0];
        *f.input.last_mut().unwrap() = 0;
        *f.expected.samples.last_mut().unwrap() = 0;
        f.salvage = Some(Salvage {
            cut: f.input.len() - 1,
            rows: f.expected.height,
        });
        let report = run_jpeg2000(&Toy::default(), &fixtures);
        assert!(report.passed(), "{report}");
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
                    "image.ops_limit",
                    "image.output_limit",
                    "image.pixel_limit",
                    "image.working_limit"
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
