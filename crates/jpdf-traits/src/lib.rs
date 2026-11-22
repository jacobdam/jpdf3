//! jpdf-traits: the replaceable-backend boundary (PLAN.md §3) and the shared [`Limits`].
//!
//! Every third-party capability is used only through a trait defined here (CONSTITUTION 11);
//! default adapters live in `jpdf-backends`. This crate has no implementations and no
//! dependencies. Rules every trait follows (specs/core/backend-traits.md):
//!
//! - every call that produces data takes [`DecodeLimits`] and checks it before allocating;
//! - every call returns [`BackendError`] instead of panicking, whatever the input;
//! - decoders report truncated input with [`DecodeStatus::Truncated`] rather than guessing;
//! - every trait has a suite in [`conformance`] that each adapter must pass.
#![forbid(unsafe_code)]

mod codec;
pub mod conformance;
mod crypto;
mod error;
mod font;
mod image;
mod limits;
mod raster;

pub use codec::{
    CompressionLevel, DecodeStatus, DecodeUsage, Decoded, Deflate, FlateFormat, Inflate,
};
pub use crypto::{Cipher, Direction, HashAlgorithm, HashState, Hasher, Padding};
pub use error::BackendError;
pub use font::{
    BuiltinEncoding, CmapId, FaceMetrics, FontEngine, FontFace, FontProgram, GlyphId, Outline,
    OutlineFormat, PathSegment, Point, Type1Parser,
};
pub use image::{
    Bitmap, CcittDecoder, CcittParams, DecodedImage, ImageColor, Jbig2Decoder, Jpeg2000Decoder,
    JpegDecoder, JpegOptions,
};
pub use limits::{DecodeLimits, FontLimits, Limits, RasterLimits};
pub use raster::{
    BlendMode, Color, DrawCommand, FillRule, LineCap, LineJoin, Paint, Pixmap, Rasterizer,
    RgbaImage, SceneStats, StrokeStyle, Transform, check_scene, path_is_valid,
};
