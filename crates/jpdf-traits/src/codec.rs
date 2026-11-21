//! Flate compression (ISO 32000-2 §7.4.4).

use crate::{BackendError, DecodeLimits};

/// Whether a decoder consumed a complete, well-formed stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeStatus {
    /// The stream ended where its format says it ends (for zlib: final block and Adler-32
    /// trailer present).
    Complete,
    /// The input ended early; the output is everything decodable before that point. Real-world
    /// PDFs often have truncated streams and consensus engines show what they can
    /// (CONSTITUTION 17: read leniently).
    Truncated,
}

/// Decoder output, whether the stream was complete, and what the call charged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded<T> {
    /// The decoded value.
    pub value: T,
    /// Whether the input was complete.
    pub status: DecodeStatus,
    /// What the call charged against [`DecodeLimits::max_working_bytes`] and
    /// [`DecodeLimits::max_ops`]. Image decoders report it; Inflate, Deflate and Cipher don't
    /// charge and report [`DecodeUsage::default`] (specs/core/decode-work-limits.md).
    pub usage: DecodeUsage,
}

/// What one image decode charged (specs/core/decode-work-limits.md). It depends only on the
/// input and the options, never on the limits, and an `Ok` result's usage is within its
/// limits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DecodeUsage {
    /// The most bytes the call held at once, by the backend's nominal accounting, output
    /// buffer included (so at least the output's byte length).
    pub working_bytes: u64,
    /// Operations performed, in a backend-chosen unit: at least one per output sample, and
    /// work that produces no output (tiles, layers, scans, symbol instances) counts too.
    pub ops: u64,
}

/// Framing of a Flate stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlateFormat {
    /// RFC 1950 zlib framing (2-byte header, Adler-32 trailer). FlateDecode streams use this
    /// (ISO 32000-2 §7.4.4.1).
    Zlib,
    /// A bare RFC 1951 deflate stream. Not valid PDF; readers fall back to it for broken
    /// files whose zlib header is missing.
    Raw,
}

/// Compression effort for [`Deflate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    /// Fastest compression.
    Fastest,
    /// The backend's balanced default.
    Default,
    /// Smallest output.
    Best,
}

/// Flate decoding (RFC 1950/1951).
pub trait Inflate {
    /// Decompresses `input`.
    ///
    /// - Output longer than `limits.max_output_bytes` fails with
    ///   [`BackendError::LimitExceeded`] as soon as the backend would exceed it: the output
    ///   buffer never grows beyond the limit, and working memory (window, tables) is bounded
    ///   by a constant.
    /// - Input that ends early returns the bytes decoded so far with
    ///   [`DecodeStatus::Truncated`] (or [`BackendError::Truncated`] if nothing was decodable).
    ///   For [`FlateFormat::Zlib`], a missing Adler-32 trailer counts as truncated.
    /// - An Adler-32 mismatch may be ignored (consensus engines do) but must not change the
    ///   decoded bytes.
    fn inflate(
        &self,
        input: &[u8],
        format: FlateFormat,
        limits: &DecodeLimits,
    ) -> Result<Decoded<Vec<u8>>, BackendError>;
}

/// Flate encoding (RFC 1950/1951). Output must be spec-conformant (CONSTITUTION 17: write
/// strictly): for [`FlateFormat::Zlib`], a valid header without a preset dictionary and a
/// correct Adler-32 trailer.
pub trait Deflate {
    /// Compresses `input`. Output longer than `limits.max_output_bytes` fails with
    /// [`BackendError::LimitExceeded`]. For a given input, format and level the output is
    /// deterministic and does not depend on `limits` when the cap is not hit (the
    /// conformance suite probes the cap at the exact output length).
    fn deflate(
        &self,
        input: &[u8],
        format: FlateFormat,
        level: CompressionLevel,
        limits: &DecodeLimits,
    ) -> Result<Vec<u8>, BackendError>;
}
