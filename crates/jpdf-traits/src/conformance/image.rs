//! Conformance suites for the image decoders (ISO 32000-2 §7.4.6–§7.4.9).
//!
//! jpdf-traits has no encoders for these formats, so the adapter supplies fixtures: small
//! files we generated ourselves (never corpus files, CONSTITUTION 18) with the pixels a
//! reference decoder produced. The suite adds format-independent checks on top: empty and
//! truncated input, mutated bytes, and every limit at its exact boundary.

use super::{Report, describe, guarded, sample_offsets};
use crate::{
    BackendError, Bitmap, CcittDecoder, CcittParams, DecodeLimits, DecodeStatus, Decoded,
    DecodedImage, Jbig2Decoder, Jpeg2000Decoder, JpegDecoder, JpegOptions,
};

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
    /// differently).
    pub tolerance: u16,
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
    /// Largest allowed difference per sample (0 for lossless fixtures).
    pub tolerance: u16,
}

/// A JBIG2 embedded stream and the bitmap it decodes to (exact match).
#[derive(Debug, Clone)]
pub struct Jbig2Fixture {
    /// Name for failure messages.
    pub name: String,
    /// The embedded stream.
    pub input: Vec<u8>,
    /// The decoded `/JBIG2Globals` stream, if any.
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
}

/// Runs every check against a [`JpegDecoder`]. At least one fixture is required.
pub fn run_jpeg(decoder: &dyn JpegDecoder, fixtures: &[JpegFixture]) -> Report {
    let mut report = Report::new("jpeg");
    for f in fixtures {
        let decode =
            |input: &[u8], limits: &DecodeLimits| decoder.decode(input, &f.options, limits);
        run_fixture(
            &mut report,
            &f.name,
            &f.input,
            &f.expected,
            f.tolerance,
            &decode,
        );
    }
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
        run_fixture(
            &mut report,
            &f.name,
            &f.input,
            &f.expected,
            f.tolerance,
            &decode,
        );
    }
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
        run_fixture(&mut report, &f.name, &f.input, &f.expected, 0, &decode);
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
        }
    }
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
        run_fixture(&mut report, &f.name, &f.input, &f.expected, 0, &decode);

        let zero = CcittParams {
            columns: 0,
            ..f.params
        };
        let result = guarded(|| decoder.decode(&f.input, &zero, &DecodeLimits::default()));
        report.check(
            "image.invalid_argument",
            matches!(result, Some(Err(BackendError::InvalidArgument(_)))),
            || {
                format!(
                    "{}: Columns 0: expected InvalidArgument, got {}",
                    f.name,
                    describe(&result)
                )
            },
        );
    }
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

/// What the suite needs to know about a decoded image or bitmap.
trait Raster: std::fmt::Debug {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn components(&self) -> Option<u16>;
    fn byte_len(&self) -> usize;
    fn expected_len(&self) -> Option<usize>;
    /// Every sample is below `2^source_bits` (always true for bitmaps).
    fn samples_in_range(&self) -> bool;
    /// Same header and every sample within `tolerance`.
    fn matches(&self, expected: &Self, tolerance: u16) -> bool;
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
        if self.bits_per_component == 16 {
            let (samples, _) = self.samples.as_chunks::<2>();
            samples
                .iter()
                .all(|s| u32::from(u16::from_be_bytes(*s)) < limit)
        } else {
            self.samples.iter().all(|&s| u32::from(s) < limit)
        }
    }
    fn matches(&self, expected: &Self, tolerance: u16) -> bool {
        let header = self.width == expected.width
            && self.height == expected.height
            && self.components == expected.components
            && self.bits_per_component == expected.bits_per_component
            && self.source_bits == expected.source_bits
            && self.adobe_app14 == expected.adobe_app14
            && self.color == expected.color
            && self.has_alpha == expected.has_alpha
            && self.samples.len() == expected.samples.len();
        if !header {
            return false;
        }
        let close = |a: u16, b: u16| a.abs_diff(b) <= tolerance;
        if self.bits_per_component == 16 {
            let (ours, _) = self.samples.as_chunks::<2>();
            let (theirs, _) = expected.samples.as_chunks::<2>();
            ours.iter()
                .zip(theirs)
                .all(|(a, b)| close(u16::from_be_bytes(*a), u16::from_be_bytes(*b)))
        } else {
            self.samples
                .iter()
                .zip(&expected.samples)
                .all(|(&a, &b)| close(u16::from(a), u16::from(b)))
        }
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
        true
    }
    fn matches(&self, expected: &Self, _tolerance: u16) -> bool {
        // Padding bits are specified as 0, so whole-byte equality is exact.
        self == expected
    }
}

type DecodeFn<'a, R> = dyn Fn(&[u8], &DecodeLimits) -> Result<Decoded<R>, BackendError> + 'a;
type Outcome<R> = Option<Result<Decoded<R>, BackendError>>;

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

fn run_fixture<R: Raster>(
    report: &mut Report,
    name: &str,
    input: &[u8],
    expected: &R,
    tolerance: u16,
    decode: &DecodeFn<'_, R>,
) {
    let unlimited = DecodeLimits::default();
    let call = |input: &[u8], limits: &DecodeLimits| guarded(|| decode(input, limits));
    let Some(len) = expected
        .expected_len()
        .filter(|&l| l == expected.byte_len() && expected.samples_in_range())
    else {
        report.check("image.fixture_valid", false, || {
            format!(
                "{name}: the expected output's length does not match its header, or a sample \
                 is not below 2^source_bits"
            )
        });
        return;
    };

    let result = call(input, &unlimited);
    let ok = matches!(&result, Some(Ok(d))
        if d.status == DecodeStatus::Complete && d.value.matches(expected, tolerance));
    report.check("image.fixture_decodes", ok, || {
        format!(
            "{name}: expected {expected:?}, got {}",
            describe_raster(&result)
        )
    });

    // Exact limits: each at the fixture's size passes, one below fails with LimitExceeded.
    let pixels = u64::from(expected.width()).saturating_mul(u64::from(expected.height()));
    let side = expected.width().max(expected.height());
    let mut boundaries: Vec<(&'static str, DecodeLimits, DecodeLimits)> = vec![
        (
            "image.output_limit",
            DecodeLimits {
                max_output_bytes: len,
                ..unlimited
            },
            DecodeLimits {
                max_output_bytes: len.saturating_sub(1),
                ..unlimited
            },
        ),
        (
            "image.pixel_limit",
            DecodeLimits {
                max_image_pixels: pixels,
                ..unlimited
            },
            DecodeLimits {
                max_image_pixels: pixels.saturating_sub(1),
                ..unlimited
            },
        ),
        (
            "image.dimension_limit",
            DecodeLimits {
                max_image_dimension: side,
                ..unlimited
            },
            DecodeLimits {
                max_image_dimension: side.saturating_sub(1),
                ..unlimited
            },
        ),
    ];
    if let Some(components) = expected.components() {
        boundaries.push((
            "image.component_limit",
            DecodeLimits {
                max_image_components: components,
                ..unlimited
            },
            DecodeLimits {
                max_image_components: components.saturating_sub(1),
                ..unlimited
            },
        ));
    }
    for (check, at, below) in boundaries {
        let result = call(input, &at);
        report.check(
            check,
            matches!(&result, Some(Ok(d)) if d.value.matches(expected, tolerance)),
            || {
                format!(
                    "{name}: limit equal to the fixture: {}",
                    describe_raster(&result)
                )
            },
        );
        let result = call(input, &below);
        report.check(
            check,
            matches!(result, Some(Err(BackendError::LimitExceeded(_)))),
            || {
                format!(
                    "{name}: limit one below the fixture: expected LimitExceeded, got {}",
                    describe_raster(&result)
                )
            },
        );
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
    use crate::ImageColor;

    /// A toy codec standing in for every format: `width u16 BE, height u16 BE, components u8,
    /// then samples`. The suite is format-independent, so this exercises every check.
    #[derive(Default)]
    struct Toy {
        ignore_limits: bool,
        width_only_dimension_check: bool,
        /// Rescales sub-8-bit samples to 0–255 (the bug `source_bits` exists to prevent).
        scale_low_depth: bool,
        panic_on_mutation: bool,
        lie_about_length: bool,
    }

    impl Toy {
        /// `packed`: one bit per pixel, rows padded to bytes (bitmaps).
        fn decode_image(
            &self,
            input: &[u8],
            limits: &DecodeLimits,
            packed: bool,
        ) -> Result<Decoded<DecodedImage>, BackendError> {
            let header = input.get(..5).ok_or(BackendError::Truncated)?;
            if self.panic_on_mutation && header[4] > 4 {
                panic!("bad components");
            }
            let width = u32::from(u16::from_be_bytes([header[0], header[1]]));
            let height = u32::from(u16::from_be_bytes([header[2], header[3]]));
            // Low nibble: components; high nibble: source bits (0 means 8).
            let components = u16::from(header[4] & 0x0f);
            let source_bits = match header[4] >> 4 {
                0 => 8,
                b @ 1..=8 => b,
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
            if !self.ignore_limits && len > limits.max_output_bytes {
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
            let status = if samples.len() < len {
                if samples.is_empty() {
                    return Err(BackendError::Truncated);
                }
                if !self.lie_about_length {
                    samples.resize(len, 0);
                }
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
                    adobe_app14: None,
                    color: if components == 1 {
                        ImageColor::Gray
                    } else {
                        ImageColor::Unknown
                    },
                    has_alpha: false,
                    samples,
                },
                status,
            })
        }

        fn decode_bitmap(
            &self,
            input: &[u8],
            limits: &DecodeLimits,
        ) -> Result<Decoded<Bitmap>, BackendError> {
            let d = self.decode_image(input, limits, true)?;
            Ok(Decoded {
                value: Bitmap {
                    width: d.value.width,
                    height: d.value.height,
                    data: d.value.samples,
                },
                status: d.status,
            })
        }
    }

    impl JpegDecoder for Toy {
        fn decode(
            &self,
            input: &[u8],
            _options: &JpegOptions,
            limits: &DecodeLimits,
        ) -> Result<Decoded<DecodedImage>, BackendError> {
            self.decode_image(input, limits, false)
        }
    }

    impl Jpeg2000Decoder for Toy {
        fn decode(
            &self,
            input: &[u8],
            limits: &DecodeLimits,
        ) -> Result<Decoded<DecodedImage>, BackendError> {
            self.decode_image(input, limits, false)
        }
    }

    impl Jbig2Decoder for Toy {
        fn decode(
            &self,
            input: &[u8],
            globals: Option<&[u8]>,
            limits: &DecodeLimits,
        ) -> Result<Decoded<Bitmap>, BackendError> {
            if globals.is_some_and(|g| g.len() < 2) {
                return Err(BackendError::Malformed("globals"));
            }
            self.decode_bitmap(input, limits)
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
            self.decode_bitmap(input, limits)
        }
    }

    fn toy_image(width: u16, height: u16) -> (Vec<u8>, DecodedImage) {
        let len = usize::from(width) * usize::from(height) * 3;
        let samples: Vec<u8> = (0..len)
            .map(|i| u8::try_from(i * 7 % 256).unwrap())
            .collect();
        let [w0, w1] = width.to_be_bytes();
        let [h0, h1] = height.to_be_bytes();
        let mut input = vec![w0, w1, h0, h1, 3];
        input.extend_from_slice(&samples);
        let image = DecodedImage {
            width: u32::from(width),
            height: u32::from(height),
            components: 3,
            bits_per_component: 8,
            source_bits: 8,
            adobe_app14: None,
            color: ImageColor::Unknown,
            has_alpha: false,
            samples,
        };
        (input, image)
    }

    /// A packed bitmap after the toy header (one "component").
    fn toy_bitmap(width: u16, height: u16) -> (Vec<u8>, Bitmap) {
        let len = usize::from(width.div_ceil(8)) * usize::from(height);
        let data: Vec<u8> = (0..len)
            .map(|i| u8::try_from(i * 37 % 256).unwrap())
            .collect();
        let [w0, w1] = width.to_be_bytes();
        let [h0, h1] = height.to_be_bytes();
        let mut input = vec![w0, w1, h0, h1, 1];
        input.extend_from_slice(&data);
        let bitmap = Bitmap {
            width: u32::from(width),
            height: u32::from(height),
            data,
        };
        (input, bitmap)
    }

    /// Landscape 4×3 and portrait 2×5.
    fn jpeg_fixtures() -> Vec<JpegFixture> {
        [(4, 3), (2, 5)]
            .into_iter()
            .map(|(w, h)| {
                let (input, expected) = toy_image(w, h);
                JpegFixture {
                    name: format!("toy {w}x{h}"),
                    input,
                    options: JpegOptions::default(),
                    expected,
                    tolerance: 0,
                }
            })
            .collect()
    }

    /// The JPEG fixtures plus a 4-bit 3×2 image.
    fn jpx_fixtures() -> Vec<Jpeg2000Fixture> {
        let mut fixtures: Vec<Jpeg2000Fixture> = jpeg_fixtures()
            .into_iter()
            .map(|f| Jpeg2000Fixture {
                name: f.name,
                input: f.input,
                expected: f.expected,
                tolerance: 0,
            })
            .collect();
        let (mut input, mut expected) = toy_image(3, 2);
        input[4] = 0x43;
        for (i, v) in input[5..].iter_mut().enumerate() {
            *v = u8::try_from(i % 16).unwrap();
        }
        expected.samples = input[5..].to_vec();
        expected.source_bits = 4;
        fixtures.push(Jpeg2000Fixture {
            name: "toy 4-bit".into(),
            input,
            expected,
            tolerance: 0,
        });
        fixtures
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

        let bitmaps = [toy_bitmap(12, 2), toy_bitmap(3, 9)];
        let fixtures: Vec<Jbig2Fixture> = bitmaps
            .iter()
            .map(|(input, expected)| Jbig2Fixture {
                name: "toy".into(),
                input: input.clone(),
                globals: Some(vec![1, 2, 3, 4]),
                expected: expected.clone(),
            })
            .collect();
        let report = run_jbig2(&toy, &fixtures);
        assert!(report.passed(), "{report}");
        assert!(report.checks_run > 50, "{report}");

        let fixtures: Vec<CcittFixture> = bitmaps
            .iter()
            .map(|(input, expected)| CcittFixture {
                name: "toy".into(),
                input: input.clone(),
                params: CcittParams {
                    columns: expected.width,
                    ..CcittParams::default()
                },
                expected: expected.clone(),
            })
            .collect();
        let report = run_ccitt(&toy, &fixtures);
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn no_fixtures_fails() {
        let report = run_jpeg(&Toy::default(), &[]);
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_orientations", "image.has_fixtures"],
            "{report}"
        );
    }

    #[test]
    fn one_orientation_is_not_enough() {
        let mut fixtures = jpeg_fixtures();
        fixtures.pop();
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
        assert_eq!(report.failed_checks(), vec!["image.truncated"], "{report}");
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
        let base = DecodedImage {
            width: 1,
            height: 1,
            components: 1,
            bits_per_component: 16,
            source_bits: 16,
            adobe_app14: None,
            color: ImageColor::Gray,
            has_alpha: false,
            samples: vec![0x01, 0xff],
        };
        let next = DecodedImage {
            samples: vec![0x02, 0x00],
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
