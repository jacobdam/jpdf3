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
    /// no Adobe inversion is applied.
    Cmyk,
    /// An embedded ICC profile (JPEG 2000 `colr` box, JPEG APP2) describes the colour
    /// components.
    Icc(Vec<u8>),
    /// The codestream does not say; the caller uses the image dictionary.
    Unknown,
}

/// A decoded continuous-tone image.
///
/// Samples are stored row by row, top to bottom, components interleaved per pixel. With
/// `bits_per_component` 8 each sample is one byte; with 16 each is two bytes, big-endian (the
/// PDF sample layout, ISO 32000-2 §8.9.3). No other depth is allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    /// Width in pixels (≥ 1).
    pub width: u32,
    /// Height in pixels (≥ 1).
    pub height: u32,
    /// Components per pixel, including alpha.
    pub components: u16,
    /// 8 or 16.
    pub bits_per_component: u8,
    /// Colour model of the non-alpha components.
    pub color: ImageColor,
    /// Whether the last component is alpha (JPEG 2000 with an opacity channel; ISO 32000-2
    /// §8.9.5 `SMaskInData`).
    pub has_alpha: bool,
    /// The samples; exactly [`DecodedImage::expected_len`] bytes.
    pub samples: Vec<u8>,
}

impl DecodedImage {
    /// The sample length the dimensions imply, or `None` if the header is invalid (zero
    /// size, zero components, depth other than 8/16) or the length overflows `usize`.
    pub fn expected_len(&self) -> Option<usize> {
        if self.width == 0 || self.height == 0 || self.components == 0 {
            return None;
        }
        let bytes_per_sample: usize = match self.bits_per_component {
            8 => 1,
            16 => 2,
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
    /// `/ColorTransform`: `Some(true)` converts YCbCr/YCCK, `Some(false)` returns components
    /// as stored, `None` uses the codestream's own markers (Adobe APP14, JFIF) and otherwise
    /// converts 3-component images only.
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
    /// Decodes the full-resolution image. Components deeper than 8 bits are returned as 16-bit
    /// samples scaled to the full 16-bit range; shallower ones as 8-bit samples scaled to
    /// 0–255.
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

    fn image(width: u32, height: u32, components: u16, bits: u8) -> DecodedImage {
        DecodedImage {
            width,
            height,
            components,
            bits_per_component: bits,
            color: ImageColor::Unknown,
            has_alpha: false,
            samples: Vec::new(),
        }
    }

    #[test]
    fn image_expected_len() {
        assert_eq!(image(4, 3, 3, 8).expected_len(), Some(36));
        assert_eq!(image(4, 3, 3, 16).expected_len(), Some(72));
        assert_eq!(image(0, 3, 3, 8).expected_len(), None);
        assert_eq!(image(4, 0, 3, 8).expected_len(), None);
        assert_eq!(image(4, 3, 0, 8).expected_len(), None);
        assert_eq!(image(4, 3, 3, 4).expected_len(), None);
        assert_eq!(image(4, 3, 3, 1).expected_len(), None);
    }

    #[test]
    fn image_expected_len_overflow_is_none() {
        let huge = image(u32::MAX, u32::MAX, u16::MAX, 16);
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
