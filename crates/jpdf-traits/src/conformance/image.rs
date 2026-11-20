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
    common(&mut report, fixtures.len(), &|input, limits| {
        decoder.decode(input, &options, limits)
    });
    report
}

/// Runs every check against a [`Jpeg2000Decoder`]. At least one fixture is required.
pub fn run_jpeg2000(decoder: &dyn Jpeg2000Decoder, fixtures: &[Jpeg2000Fixture]) -> Report {
    let mut report = Report::new("jpeg2000");
    let decode = |input: &[u8], limits: &DecodeLimits| decoder.decode(input, limits);
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
    common(&mut report, fixtures.len(), &decode);
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
    common(&mut report, fixtures.len(), &|input, limits| {
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
    common(&mut report, fixtures.len(), &|input, limits| {
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
    fn matches(&self, expected: &Self, tolerance: u16) -> bool {
        let header = self.width == expected.width
            && self.height == expected.height
            && self.components == expected.components
            && self.bits_per_component == expected.bits_per_component
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
    fn matches(&self, expected: &Self, _tolerance: u16) -> bool {
        // Padding bits are specified as 0, so whole-byte equality is exact.
        self == expected
    }
}

type DecodeFn<'a, R> = dyn Fn(&[u8], &DecodeLimits) -> Result<Decoded<R>, BackendError> + 'a;
type Outcome<R> = Option<Result<Decoded<R>, BackendError>>;

/// Checks that need no fixture.
fn common<R: Raster>(report: &mut Report, fixture_count: usize, decode: &DecodeFn<'_, R>) {
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
        .filter(|&l| l == expected.byte_len())
    else {
        report.check("image.fixture_valid", false, || {
            format!("{name}: the expected output's length does not match its header")
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
            let components = u16::from(header[4]);
            if width == 0 || height == 0 || components == 0 {
                return Err(BackendError::Malformed("header"));
            }
            if !self.ignore_limits
                && (width.max(height) > limits.max_image_dimension
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

    fn toy_image() -> (Vec<u8>, DecodedImage) {
        let samples: Vec<u8> = (0..4 * 3 * 3)
            .map(|i| u8::try_from(i * 7).unwrap())
            .collect();
        let mut input = vec![0, 4, 0, 3, 3];
        input.extend_from_slice(&samples);
        let image = DecodedImage {
            width: 4,
            height: 3,
            components: 3,
            bits_per_component: 8,
            color: ImageColor::Unknown,
            has_alpha: false,
            samples,
        };
        (input, image)
    }

    /// A 12×2 bitmap: stride 2, so 4 data bytes after the toy header.
    fn toy_bitmap() -> (Vec<u8>, Bitmap) {
        let data = vec![0b1010_1010, 0b1111_0000, 0b0101_0101, 0b0000_0000];
        let mut input = vec![0, 12, 0, 2, 1];
        input.extend_from_slice(&data);
        (
            input,
            Bitmap {
                width: 12,
                height: 2,
                data,
            },
        )
    }

    fn jpeg(toy: &Toy) -> Report {
        let (input, expected) = toy_image();
        run_jpeg(
            toy,
            &[JpegFixture {
                name: "toy".into(),
                input,
                options: JpegOptions::default(),
                expected,
                tolerance: 0,
            }],
        )
    }

    #[test]
    fn a_correct_decoder_passes_every_suite() {
        let toy = Toy::default();
        let report = jpeg(&toy);
        assert!(report.passed(), "{report}");

        let (input, expected) = toy_image();
        let report = run_jpeg2000(
            &toy,
            &[Jpeg2000Fixture {
                name: "toy".into(),
                input,
                expected,
                tolerance: 0,
            }],
        );
        assert!(report.passed(), "{report}");

        let (input, expected) = toy_bitmap();
        let report = run_jbig2(
            &toy,
            &[Jbig2Fixture {
                name: "toy".into(),
                input: input.clone(),
                globals: Some(vec![1, 2, 3, 4]),
                expected: expected.clone(),
            }],
        );
        assert!(report.passed(), "{report}");
        assert!(report.checks_run > 25, "{report}");

        let report = run_ccitt(
            &toy,
            &[CcittFixture {
                name: "toy".into(),
                input,
                params: CcittParams {
                    columns: 12,
                    ..CcittParams::default()
                },
                expected,
            }],
        );
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn no_fixtures_fails() {
        let report = run_jpeg(&Toy::default(), &[]);
        assert_eq!(
            report.failed_checks(),
            vec!["image.has_fixtures"],
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
        let (input, mut expected) = toy_image();
        expected.samples[5] = expected.samples[5].wrapping_add(2);
        let fixture = |tolerance| JpegFixture {
            name: "toy".into(),
            input: input.clone(),
            options: JpegOptions::default(),
            expected: expected.clone(),
            tolerance,
        };
        let report = run_jpeg(&Toy::default(), &[fixture(1)]);
        assert!(report.failed("image.fixture_decodes"), "{report}");
        let report = run_jpeg(&Toy::default(), &[fixture(2)]);
        assert!(report.passed(), "{report}");
    }

    #[test]
    fn sixteen_bit_tolerance_compares_whole_samples() {
        let base = DecodedImage {
            width: 1,
            height: 1,
            components: 1,
            bits_per_component: 16,
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
        let (input, mut expected) = toy_image();
        expected.samples.pop();
        let report = run_jpeg(
            &Toy::default(),
            &[JpegFixture {
                name: "toy".into(),
                input,
                options: JpegOptions::default(),
                expected,
                tolerance: 0,
            }],
        );
        assert_eq!(
            report.failed_checks(),
            vec!["image.fixture_valid"],
            "{report}"
        );
    }
}
