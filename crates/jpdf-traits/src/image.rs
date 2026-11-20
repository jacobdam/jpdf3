//! Image decoders for DCTDecode, JPXDecode, JBIG2Decode and CCITTFaxDecode
//! (ISO 32000-2 §7.4.6–§7.4.9).
//!
//! Decoders return pixels exactly as the codec defines them. Mapping them to PDF image
//! samples (Decode arrays, `BlackIs1`, colour-space overrides from the image dictionary) is
//! the job of `jpdf-filters`, so every backend behaves the same way at this boundary.

use crate::{BackendError, DecodeLimits, Decoded};

/// Colour model of a decoded image, as stored in the codestream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageColor {
    /// One component.
    Gray,
    /// Three components, RGB (YCbCr is converted to RGB by the decoder).
    Rgb,
    /// Four components, CMYK (YCCK is converted to CMYK by the decoder). Values are as stored:
    /// no Adobe inversion is applied; [`DecodedImage::adobe_app14`] tells the caller whether
    /// the codestream is an Adobe one.
    Cmyk,
    /// An embedded ICC profile (JPEG 2000 `colr` box, JPEG APP2) describes the colour
    /// components.
    Icc(Vec<u8>),
    /// The codestream does not say; the caller uses the image dictionary.
    Unknown,
}

/// A decoded continuous-tone image.
///
/// Samples are stored row by row, top to bottom, components interleaved per pixel, every
/// component at full resolution. Sample **values are not rescaled**: a 4-bit component holds
/// 0–15, so an `/Indexed` override in the image dictionary still works. The container is one
/// byte per sample when `source_bits` ≤ 8 (`bits_per_component` 8) and two bytes, big-endian,
/// when it is 9–16 (`bits_per_component` 16), like PDF sample data (ISO 32000-2 §8.9.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    /// Width in pixels (≥ 1).
    pub width: u32,
    /// Height in pixels (≥ 1).
    pub height: u32,
    /// Components per pixel, including alpha.
    pub components: u16,
    /// Container size: 8 or 16.
    pub bits_per_component: u8,
    /// Significant bits per sample in the codestream (1–16); values are `0..2^source_bits`.
    /// 8 for baseline JPEG, 12 for 12-bit JPEG.
    pub source_bits: u8,
    /// Colour model of the non-alpha components.
    pub color: ImageColor,
    /// Whether the last component is alpha (JPEG 2000 with an opacity channel; ISO 32000-2
    /// §8.9.5 `SMaskInData`).
    pub has_alpha: bool,
    /// JPEG only: the transform flag of the Adobe APP14 marker (0 = none/CMYK, 1 = YCbCr,
    /// 2 = YCCK) when the codestream has one, else `None`. `jpdf-filters` uses it for the
    /// Adobe CMYK inversion. Always `None` for other codecs.
    pub adobe_app14: Option<u8>,
    /// The samples; exactly [`DecodedImage::expected_len`] bytes.
    pub samples: Vec<u8>,
}

impl DecodedImage {
    /// The sample length the dimensions imply, or `None` if the header is invalid (zero
    /// size, zero components, a container other than the one `source_bits` requires) or the
    /// length overflows `usize`.
    pub fn expected_len(&self) -> Option<usize> {
        if self.width == 0 || self.height == 0 || self.components == 0 {
            return None;
        }
        let bytes_per_sample: usize = match (self.bits_per_component, self.source_bits) {
            (8, 1..=8) => 1,
            (16, 9..=16) => 2,
            _ => return None,
        };
        usize::try_from(self.width)
            .ok()?
            .checked_mul(usize::try_from(self.height).ok()?)?
            .checked_mul(usize::from(self.components))?
            .checked_mul(bytes_per_sample)
    }
}

/// A decoded bilevel image (JBIG2, CCITT).
///
/// **1 bits are black** (foreground), the JBIG2 and CCITT convention. Rows run top to bottom;
/// each row is [`Bitmap::stride`] bytes, most significant bit first, padded with 0 bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    /// Width in pixels (≥ 1).
    pub width: u32,
    /// Height in pixels (≥ 1).
    pub height: u32,
    /// Packed rows; exactly [`Bitmap::expected_len`] bytes.
    pub data: Vec<u8>,
}

impl Bitmap {
    /// Bytes per row: `ceil(width / 8)`.
    pub fn stride(&self) -> usize {
        usize::try_from(self.width.div_ceil(8)).unwrap_or(usize::MAX)
    }

    /// The data length the dimensions imply, or `None` for a zero size or overflow.
    pub fn expected_len(&self) -> Option<usize> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        self.stride()
            .checked_mul(usize::try_from(self.height).ok()?)
    }
}

/// Options for [`JpegDecoder`] taken from the DCTDecode parameters (ISO 32000-2 §7.4.8,
/// Table 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JpegOptions {
    /// `/ColorTransform`:
    /// - `Some(true)`: the encoder transformed RGB→YCbCr or CMYK→YCCK; the decoder converts
    ///   back.
    /// - `Some(false)`: the encoder did not transform; components are returned as stored and
    ///   *are* RGB or CMYK.
    /// - `None`: the codestream decides (Adobe APP14 transform flag; otherwise 3-component
    ///   images are converted and 4-component ones are not).
    ///
    /// In every case 1 component gives [`ImageColor::Gray`], 3 give [`ImageColor::Rgb`], 4
    /// give [`ImageColor::Cmyk`], unless an ICC profile (APP2) gives [`ImageColor::Icc`].
    pub color_transform: Option<bool>,
}

/// Baseline and progressive JPEG decoding (DCTDecode, ISO 32000-2 §7.4.8).
pub trait JpegDecoder {
    /// Decodes one JPEG codestream. Fails with [`BackendError::LimitExceeded`] when the frame
    /// header exceeds a [`DecodeLimits`] value, before allocating the image.
    fn decode(
        &self,
        input: &[u8],
        options: &JpegOptions,
        limits: &DecodeLimits,
    ) -> Result<Decoded<DecodedImage>, BackendError>;
}

/// JPEG 2000 decoding (JPXDecode, ISO 32000-2 §7.4.9): a JP2 file or a raw codestream.
pub trait Jpeg2000Decoder {
    /// Decodes the full-resolution image, unscaled (see [`DecodedImage`]).
    /// - Components with different bit depths fail with [`BackendError::Unsupported`].
    /// - Subsampled components are upsampled to full resolution by pixel replication.
    /// - Signed components are made unsigned by adding `2^(source_bits - 1)`.
    fn decode(
        &self,
        input: &[u8],
        limits: &DecodeLimits,
    ) -> Result<Decoded<DecodedImage>, BackendError>;
}

/// JBIG2 decoding (JBIG2Decode, ISO 32000-2 §7.4.7). The input is the embedded stream
/// format: segments without the JBIG2 file header, for a single page.
pub trait Jbig2Decoder {
    /// Decodes the page. `globals` is the decoded `/JBIG2Globals` stream, if any.
    fn decode(
        &self,
        input: &[u8],
        globals: Option<&[u8]>,
        limits: &DecodeLimits,
    ) -> Result<Decoded<Bitmap>, BackendError>;
}

/// CCITTFaxDecode parameters (ISO 32000-2 §7.4.6, Table 11) that affect decoding.
/// `BlackIs1` is not here: the decoder always returns 1 = black and `jpdf-filters` inverts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CcittParams {
    /// `/K`: < 0 pure two-dimensional (Group 4), 0 pure one-dimensional (Group 3 1-D),
    /// > 0 mixed (Group 3 2-D).
    pub k: i32,
    /// `/EndOfLine`: end-of-line bit patterns are required.
    pub end_of_line: bool,
    /// `/EncodedByteAlign`: rows (or EOLs) start on byte boundaries.
    pub encoded_byte_align: bool,
    /// `/Columns`: width in pixels; must be ≥ 1.
    pub columns: u32,
    /// `/Rows`: height in pixels, or 0 when unknown (decode until end of data or EOB).
    pub rows: u32,
    /// `/EndOfBlock`: an end-of-block pattern is expected.
    pub end_of_block: bool,
    /// `/DamagedRowsBeforeError`: damaged rows tolerated (only meaningful with
    /// `end_of_line`).
    pub damaged_rows_before_error: u32,
}

impl Default for CcittParams {
    /// The Table 11 defaults.
    fn default() -> Self {
        Self {
            k: 0,
            end_of_line: false,
            encoded_byte_align: false,
            columns: 1728,
            rows: 0,
            end_of_block: true,
            damaged_rows_before_error: 0,
        }
    }
}

/// CCITT Group 3/4 fax decoding (CCITTFaxDecode, ISO 32000-2 §7.4.6).
pub trait CcittDecoder {
    /// Decodes the image. `columns == 0` fails with [`BackendError::InvalidArgument`];
    /// `columns` or `rows` above `limits.max_image_dimension` fails with
    /// [`BackendError::LimitExceeded`] before decoding.
    fn decode(
        &self,
        input: &[u8],
        params: &CcittParams,
        limits: &DecodeLimits,
    ) -> Result<Decoded<Bitmap>, BackendError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32, components: u16, container: u8, bits: u8) -> DecodedImage {
        DecodedImage {
            width,
            height,
            components,
            bits_per_component: container,
            source_bits: bits,
            color: ImageColor::Unknown,
            has_alpha: false,
            adobe_app14: None,
            samples: Vec::new(),
        }
    }

    #[test]
    fn image_expected_len() {
        assert_eq!(image(4, 3, 3, 8, 8).expected_len(), Some(36));
        assert_eq!(image(4, 3, 3, 8, 1).expected_len(), Some(36));
        assert_eq!(image(4, 3, 3, 16, 12).expected_len(), Some(72));
        assert_eq!(image(4, 3, 3, 16, 16).expected_len(), Some(72));
        assert_eq!(image(0, 3, 3, 8, 8).expected_len(), None);
        assert_eq!(image(4, 0, 3, 8, 8).expected_len(), None);
        assert_eq!(image(4, 3, 0, 8, 8).expected_len(), None);
    }

    #[test]
    fn image_container_must_fit_source_bits() {
        assert_eq!(image(4, 3, 3, 8, 0).expected_len(), None);
        assert_eq!(image(4, 3, 3, 8, 9).expected_len(), None);
        assert_eq!(image(4, 3, 3, 16, 8).expected_len(), None);
        assert_eq!(image(4, 3, 3, 16, 17).expected_len(), None);
        assert_eq!(image(4, 3, 3, 4, 4).expected_len(), None);
        assert_eq!(image(4, 3, 3, 1, 1).expected_len(), None);
    }

    #[test]
    fn image_expected_len_overflow_is_none() {
        let huge = image(u32::MAX, u32::MAX, u16::MAX, 16, 16);
        assert_eq!(huge.expected_len(), None);
    }

    #[test]
    fn bitmap_stride_and_len() {
        let bitmap = |width, height| Bitmap {
            width,
            height,
            data: Vec::new(),
        };
        assert_eq!(bitmap(1, 1).stride(), 1);
        assert_eq!(bitmap(8, 1).stride(), 1);
        assert_eq!(bitmap(9, 2).stride(), 2);
        assert_eq!(bitmap(9, 2).expected_len(), Some(4));
        assert_eq!(bitmap(0, 2).expected_len(), None);
        assert_eq!(bitmap(9, 0).expected_len(), None);
        assert_eq!(bitmap(u32::MAX, 1).stride(), 536_870_912);
    }
}
