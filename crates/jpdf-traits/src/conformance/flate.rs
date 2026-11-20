//! Conformance suite for [`Inflate`] and [`Deflate`] (ISO 32000-2 §7.4.4; RFC 1950, 1951).

use super::vectors::{FLATE, dynamic_plaintext, hex};
use super::{Report, describe, guarded, pseudo_random, sample_offsets};
use crate::{
    BackendError, CompressionLevel, DecodeLimits, DecodeStatus, Decoded, Deflate, FlateFormat,
    Inflate,
};

const SUITE: &str = "flate";
const LEVELS: [CompressionLevel; 3] = [
    CompressionLevel::Fastest,
    CompressionLevel::Default,
    CompressionLevel::Best,
];
const FORMATS: [FlateFormat; 2] = [FlateFormat::Zlib, FlateFormat::Raw];
/// Vectors at least this long must salvage output when their last bytes are cut.
const SALVAGE_MIN_PLAIN: usize = 64;
/// The large bomb: 1 GiB of zeros as fixed-Huffman back-references, about 6.5 MiB encoded.
const LARGE_BOMB_BYTES: usize = 1 << 30;
/// Output cap for mutated inputs: generous for the vectors, small enough to stay fast.
const MUTATION_CAP: usize = 1 << 20;

/// A known compressed stream and its plaintext.
struct Vector {
    name: &'static str,
    plain: Vec<u8>,
    zlib: Vec<u8>,
}

impl Vector {
    /// The bare deflate stream inside the zlib framing.
    fn raw(&self) -> &[u8] {
        let end = self.zlib.len().saturating_sub(4);
        self.zlib.get(2..end).unwrap_or(&[])
    }

    fn encoded(&self, format: FlateFormat) -> &[u8] {
        match format {
            FlateFormat::Zlib => &self.zlib,
            FlateFormat::Raw => self.raw(),
        }
    }
}

/// Runs every check against `inflate` and `deflate` (usually the same adapter).
pub fn run(inflate: &dyn Inflate, deflate: &dyn Deflate) -> Report {
    let mut report = Report::new(SUITE);
    let mut vectors = Vec::new();
    for v in &FLATE {
        let plain = v.plain.map_or_else(dynamic_plaintext, <[u8]>::to_vec);
        let zlib = hex(v.zlib_hex);
        report.check("flate.builtin_vector", zlib.is_some(), || {
            format!("{}: bad hex", v.name)
        });
        if let Some(zlib) = zlib {
            vectors.push(Vector {
                name: v.name,
                plain,
                zlib,
            });
        }
    }
    run_with(&mut report, inflate, deflate, &vectors);
    report
}

fn run_with(report: &mut Report, inflate: &dyn Inflate, deflate: &dyn Deflate, vectors: &[Vector]) {
    let unlimited = DecodeLimits::default();
    let call = |input: &[u8], format: FlateFormat, limits: &DecodeLimits| {
        guarded(|| inflate.inflate(input, format, limits))
    };

    for format in FORMATS {
        let result = call(&[], format, &unlimited);
        report.check("flate.empty_input", matches!(result, Some(Err(_))), || {
            format!("{format:?}: expected an error, got {}", describe(&result))
        });
    }

    for v in vectors {
        for format in FORMATS {
            let input = v.encoded(format);
            let result = call(input, format, &unlimited);
            report.check(
                "flate.decode_vector",
                is_complete(&result, &v.plain),
                || format!("{} ({format:?}): {}", v.name, describe_decoded(&result)),
            );

            // Truncation: every strict prefix is truncated, never complete.
            for cut in sample_offsets(input.len(), 256) {
                let prefix = input.get(..cut).unwrap_or(&[]);
                let result = call(prefix, format, &unlimited);
                let ok = match &result {
                    Some(Err(_)) => true,
                    Some(Ok(d)) => {
                        d.status == DecodeStatus::Truncated && v.plain.starts_with(&d.value)
                    }
                    None => false,
                };
                report.check("flate.truncated", ok, || {
                    format!(
                        "{} ({format:?}) cut at {cut}/{}: expected an error or a truncated \
                         prefix of the plaintext, got {}",
                        v.name,
                        input.len(),
                        describe_decoded(&result)
                    )
                });
            }

            // Lenient reading (CONSTITUTION 17): the decoded prefix must be salvaged, not
            // dropped. Without only the Adler-32 trailer, all of the plaintext comes back;
            // with the last bytes of a long stream cut, a non-empty prefix does.
            let data_end = match format {
                FlateFormat::Zlib => input.len().saturating_sub(4),
                FlateFormat::Raw => input.len(),
            };
            let mut salvage: Vec<(usize, bool)> = Vec::new();
            if format == FlateFormat::Zlib {
                salvage.extend((data_end.max(2)..input.len()).map(|cut| (cut, true)));
            }
            if v.plain.len() >= SALVAGE_MIN_PLAIN {
                salvage.extend((data_end.saturating_sub(16)..data_end).map(|cut| (cut, false)));
            }
            for (cut, whole) in salvage {
                let prefix = input.get(..cut).unwrap_or(&[]);
                let result = call(prefix, format, &unlimited);
                let ok = match &result {
                    Some(Ok(d)) if d.status == DecodeStatus::Truncated => {
                        if whole {
                            d.value == v.plain
                        } else {
                            !d.value.is_empty() && v.plain.starts_with(&d.value)
                        }
                    }
                    Some(Err(BackendError::Truncated)) => whole && v.plain.is_empty(),
                    _ => false,
                };
                report.check("flate.truncated_salvage", ok, || {
                    format!(
                        "{} ({format:?}) cut at {cut}/{}: expected Ok(Truncated) with {}, got {}",
                        v.name,
                        input.len(),
                        if whole {
                            "the whole plaintext"
                        } else {
                            "a non-empty prefix"
                        },
                        describe_decoded(&result)
                    )
                });
            }

            // Mutations never panic and never break the output cap.
            let limits = DecodeLimits {
                max_output_bytes: MUTATION_CAP,
                ..unlimited
            };
            for at in sample_offsets(input.len(), 256) {
                let mut mutated = input.to_vec();
                if let Some(byte) = mutated.get_mut(at) {
                    *byte ^= 0xff;
                }
                let result = call(&mutated, format, &limits);
                let ok = match &result {
                    Some(Ok(d)) => d.value.len() <= MUTATION_CAP,
                    Some(Err(_)) => true,
                    None => false,
                };
                report.check("flate.mutated", ok, || {
                    format!(
                        "{} ({format:?}) byte {at} flipped: {}",
                        v.name,
                        describe_decoded(&result)
                    )
                });
            }
        }

        // Exact output limit: `len` bytes allowed passes, `len - 1` fails with LimitExceeded.
        if let Some(below) = v.plain.len().checked_sub(1) {
            let at = DecodeLimits {
                max_output_bytes: v.plain.len(),
                ..unlimited
            };
            let result = call(&v.zlib, FlateFormat::Zlib, &at);
            report.check("flate.output_limit", is_complete(&result, &v.plain), || {
                format!(
                    "{}: limit equal to the output length: {}",
                    v.name,
                    describe_decoded(&result)
                )
            });
            let under = DecodeLimits {
                max_output_bytes: below,
                ..unlimited
            };
            let result = call(&v.zlib, FlateFormat::Zlib, &under);
            report.check(
                "flate.output_limit",
                matches!(result, Some(Err(BackendError::LimitExceeded(_)))),
                || {
                    format!(
                        "{}: limit one below the output length: expected LimitExceeded, got {}",
                        v.name,
                        describe_decoded(&result)
                    )
                },
            );
        }

        // A corrupted Adler-32 may be tolerated, but must not change the bytes.
        let mut bad_sum = v.zlib.clone();
        if let Some(last) = bad_sum.last_mut() {
            *last ^= 0x01;
        }
        let result = call(&bad_sum, FlateFormat::Zlib, &unlimited);
        let ok = match &result {
            Some(Ok(d)) => d.value == v.plain,
            Some(Err(_)) => true,
            None => false,
        };
        report.check("flate.checksum_mismatch", ok, || {
            format!("{}: {}", v.name, describe_decoded(&result))
        });
    }

    round_trips(report, inflate, deflate);
    zip_bomb(report, inflate, deflate);
    large_bomb(report, inflate);
}

/// 1 GiB of zeros under a 64 KiB cap must stop with LimitExceeded. The suite cannot see
/// allocations, but an adapter that decodes everything and checks afterwards needs over
/// 1 GiB here, which shows up as an out-of-memory failure or a very slow test.
fn large_bomb(report: &mut Report, inflate: &dyn Inflate) {
    let bomb = fixed_huffman_zeros(LARGE_BOMB_BYTES);
    let capped = DecodeLimits {
        max_output_bytes: 64 << 10,
        ..DecodeLimits::default()
    };
    let result = guarded(|| inflate.inflate(&bomb, FlateFormat::Raw, &capped));
    report.check(
        "flate.large_bomb",
        matches!(result, Some(Err(BackendError::LimitExceeded(_)))),
        || format!("expected LimitExceeded, got {}", describe_decoded(&result)),
    );
}

/// A raw deflate stream (RFC 1951 §3.2.6, one final fixed-Huffman block) that decodes to at
/// least `len` zero bytes: literal 0, then length-258/distance-1 copies, then end of block.
pub(crate) fn fixed_huffman_zeros(len: usize) -> Vec<u8> {
    let mut bits = BitWriter::default();
    bits.lsb_first(1, 1); // BFINAL
    bits.lsb_first(1, 2); // BTYPE = 01, fixed Huffman
    bits.code(0x30, 8); // literal 0: codes 00110000..10111111 are literals 0..143
    for _ in 0..len.saturating_sub(1).div_ceil(258) {
        bits.code(0xc5, 8); // symbol 285 (length 258): 11000000 + (285 - 280)
        bits.code(0, 5); // distance code 0 (distance 1), no extra bits
    }
    bits.code(0, 7); // end of block (256): 0000000
    bits.finish()
}

/// Packs bits LSB-first into bytes, as RFC 1951 §3.1.1 requires.
#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    used: u32,
}

impl BitWriter {
    /// Writes `count` bits of `value`, least significant first (header fields, extra bits).
    fn lsb_first(&mut self, value: u32, count: u32) {
        for i in 0..count {
            self.bit(value >> i & 1);
        }
    }

    /// Writes a Huffman code of `count` bits, most significant first.
    fn code(&mut self, value: u32, count: u32) {
        for i in (0..count).rev() {
            self.bit(value >> i & 1);
        }
    }

    fn bit(&mut self, bit: u32) {
        self.acc |= bit << self.used;
        self.used += 1;
        if self.used == 8 {
            self.out
                .push(self.acc.to_le_bytes().first().copied().unwrap_or(0));
            self.acc = 0;
            self.used = 0;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.used > 0 {
            self.out
                .push(self.acc.to_le_bytes().first().copied().unwrap_or(0));
        }
        self.out
    }
}

fn round_trips(report: &mut Report, inflate: &dyn Inflate, deflate: &dyn Deflate) {
    let unlimited = DecodeLimits::default();
    let inputs: [(&str, Vec<u8>); 5] = [
        ("empty", Vec::new()),
        ("one byte", vec![0x42]),
        ("64 KiB + 3 random", pseudo_random(65_539, 7)),
        ("1 MiB zeros", vec![0; 1 << 20]),
        ("text", dynamic_plaintext()),
    ];
    for (name, input) in &inputs {
        for level in LEVELS {
            for format in FORMATS {
                let compressed = guarded(|| deflate.deflate(input, format, level, &unlimited));
                let Some(Ok(compressed)) = compressed else {
                    report.check("flate.round_trip", false, || {
                        format!(
                            "{name} {level:?} {format:?}: deflate {}",
                            describe(&compressed)
                        )
                    });
                    continue;
                };
                if format == FlateFormat::Zlib {
                    report.check(
                        "flate.deflate_zlib_framing",
                        zlib_framing_ok(&compressed, input),
                        || format!("{name} {level:?}: bad zlib header or Adler-32 trailer"),
                    );
                }
                let result = guarded(|| inflate.inflate(&compressed, format, &unlimited));
                report.check("flate.round_trip", is_complete(&result, input), || {
                    format!("{name} {level:?} {format:?}: {}", describe_decoded(&result))
                });
            }
        }
    }

    let input = pseudo_random(65_536, 11);
    let tiny = DecodeLimits {
        max_output_bytes: 100,
        ..unlimited
    };
    for format in FORMATS {
        let result = guarded(|| deflate.deflate(&input, format, CompressionLevel::Default, &tiny));
        report.check(
            "flate.deflate_output_limit",
            matches!(result, Some(Err(BackendError::LimitExceeded(_)))),
            || {
                format!(
                    "{format:?}: expected LimitExceeded, got {}",
                    describe(&result)
                )
            },
        );
    }
}

/// 16 MiB of zeros compresses to a few KiB; inflating it under a 64 KiB cap must stop with
/// LimitExceeded.
fn zip_bomb(report: &mut Report, inflate: &dyn Inflate, deflate: &dyn Deflate) {
    let unlimited = DecodeLimits::default();
    let zeros = vec![0u8; 16 << 20];
    let bomb = guarded(|| {
        deflate.deflate(
            &zeros,
            FlateFormat::Zlib,
            CompressionLevel::Best,
            &unlimited,
        )
    });
    let Some(Ok(bomb)) = bomb else {
        report.check("flate.zip_bomb", false, || {
            format!("deflate of the bomb input: {}", describe(&bomb))
        });
        return;
    };
    let capped = DecodeLimits {
        max_output_bytes: 64 << 10,
        ..unlimited
    };
    let result = guarded(|| inflate.inflate(&bomb, FlateFormat::Zlib, &capped));
    report.check(
        "flate.zip_bomb",
        matches!(result, Some(Err(BackendError::LimitExceeded(_)))),
        || format!("expected LimitExceeded, got {}", describe_decoded(&result)),
    );
}

fn is_complete(result: &Option<Result<Decoded<Vec<u8>>, BackendError>>, expected: &[u8]) -> bool {
    matches!(result, Some(Ok(d)) if d.status == DecodeStatus::Complete && d.value == expected)
}

fn describe_decoded(result: &Option<Result<Decoded<Vec<u8>>, BackendError>>) -> String {
    match result {
        Some(Ok(d)) => format!("Ok({:?}, {} bytes)", d.status, d.value.len()),
        other => describe(other),
    }
}

/// RFC 1950 §2.2: CM = 8, CINFO ≤ 7, FCHECK makes the header a multiple of 31, no FDICT,
/// and the trailer is the big-endian Adler-32 of the uncompressed data.
fn zlib_framing_ok(stream: &[u8], plain: &[u8]) -> bool {
    let (Some(&cmf), Some(&flg)) = (stream.first(), stream.get(1)) else {
        return false;
    };
    let header_ok = cmf & 0x0f == 8
        && cmf >> 4 <= 7
        && (u16::from(cmf) << 8 | u16::from(flg)) % 31 == 0
        && flg & 0x20 == 0;
    let trailer = stream
        .len()
        .checked_sub(4)
        .filter(|&start| start >= 2)
        .and_then(|start| stream.get(start..));
    header_ok && trailer == Some(&adler32(plain).to_be_bytes()[..])
}

/// Adler-32 (RFC 1950 §8.2).
pub(crate) fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let (mut a, mut b) = (1u32, 0u32);
    // 5552 is the largest block for which `b` cannot overflow before the reduction.
    for block in data.chunks(5552) {
        for &byte in block {
            a = a.wrapping_add(u32::from(byte));
            b = b.wrapping_add(a);
        }
        a %= MOD;
        b %= MOD;
    }
    b << 16 | a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conformance::vectors::FLATE;
    use std::cell::Cell;

    #[test]
    fn adler32_matches_rfc_example() {
        // Wikipedia/RFC 1950 example: "Wikipedia" -> 0x11E60398.
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        assert_eq!(adler32(b""), 1);
        // Long input crosses the 5552-byte reduction boundary.
        let long = vec![0xff; 100_000];
        let naive = long.iter().fold((1u64, 0u64), |(a, b), &x| {
            let a = (a + u64::from(x)) % 65_521;
            (a, (b + a) % 65_521)
        });
        assert_eq!(u64::from(adler32(&long)), naive.1 << 16 | naive.0);
    }

    /// A Flate implementation that handles stored blocks (BTYPE 00) and fixed-Huffman blocks
    /// (BTYPE 01) but not dynamic ones: enough to exercise every suite check, including the
    /// large bomb, without a real compressor. Variants break one rule each.
    #[derive(Default)]
    struct Mini {
        /// Ignores the cap (stops only at a 32 MiB safety net, returning what it has, so the
        /// test stays fast).
        ignore_limit: bool,
        panic_on_empty: bool,
        complete_on_truncation: bool,
        /// Drops partial output: any truncation is an error.
        strict_truncation: bool,
        deflate_calls: Cell<usize>,
    }

    const SAFETY_NET: usize = 32 << 20;
    // RFC 1951 §3.2.5 length and distance tables.
    const LEN_BASE: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LEN_EXTRA: [u32; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u32; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    struct Bits<'a> {
        data: &'a [u8],
        pos: usize,
    }

    impl Bits<'_> {
        fn bit(&mut self) -> Option<u32> {
            let byte = *self.data.get(self.pos / 8)?;
            let bit = u32::from(byte >> (self.pos % 8) & 1);
            self.pos += 1;
            Some(bit)
        }
        /// `n` bits, least significant first.
        fn lsb(&mut self, n: u32) -> Option<u32> {
            (0..n).try_fold(0, |acc, i| Some(acc | self.bit()? << i))
        }
        /// A Huffman code of `n` bits, most significant first.
        fn msb(&mut self, n: u32) -> Option<u32> {
            (0..n).try_fold(0, |acc, _| Some(acc << 1 | self.bit()?))
        }
        /// A fixed literal/length symbol (RFC 1951 §3.2.6).
        fn fixed_symbol(&mut self) -> Option<u32> {
            let c7 = self.msb(7)?;
            if c7 <= 0b001_0111 {
                return Some(256 + c7);
            }
            let c8 = c7 << 1 | self.bit()?;
            match c8 {
                0x30..=0xbf => Some(c8 - 0x30),
                0xc0..=0xc7 => Some(280 + c8 - 0xc0),
                _ => Some(144 + (c8 << 1 | self.bit()?) - 0x190),
            }
        }
    }

    impl Mini {
        fn over(&self, len: usize, limits: &DecodeLimits) -> bool {
            len > if self.ignore_limit {
                SAFETY_NET
            } else {
                limits.max_output_bytes
            }
        }

        /// At the cap: an error, or (ignoring the cap) the output so far, as a decoder that
        /// silently stops would return it.
        fn limit(&self, out: Vec<u8>) -> Result<Decoded<Vec<u8>>, BackendError> {
            if self.ignore_limit {
                Ok(Decoded {
                    value: out,
                    status: DecodeStatus::Truncated,
                })
            } else {
                Err(BackendError::LimitExceeded("output"))
            }
        }

        fn truncated(&self, out: Vec<u8>) -> Result<Decoded<Vec<u8>>, BackendError> {
            if self.complete_on_truncation {
                Ok(Decoded {
                    value: out,
                    status: DecodeStatus::Complete,
                })
            } else if self.strict_truncation {
                Err(BackendError::Malformed("truncated"))
            } else if out.is_empty() {
                Err(BackendError::Truncated)
            } else {
                Ok(Decoded {
                    value: out,
                    status: DecodeStatus::Truncated,
                })
            }
        }
    }

    impl Inflate for Mini {
        fn inflate(
            &self,
            input: &[u8],
            format: FlateFormat,
            limits: &DecodeLimits,
        ) -> Result<Decoded<Vec<u8>>, BackendError> {
            if input.is_empty() && self.panic_on_empty {
                panic!("empty");
            }
            let body = match format {
                FlateFormat::Zlib => {
                    let header = input.get(..2).ok_or(BackendError::Truncated)?;
                    if (u16::from(header[0]) << 8 | u16::from(header[1])) % 31 != 0
                        || header[0] & 0x0f != 8
                    {
                        return Err(BackendError::Malformed("zlib header"));
                    }
                    &input[2..]
                }
                FlateFormat::Raw => input,
            };
            let mut out = Vec::new();
            let mut r = Bits { data: body, pos: 0 };
            loop {
                let (Some(last), Some(btype)) = (r.lsb(1), r.lsb(2)) else {
                    return self.truncated(out);
                };
                match btype {
                    0 => {
                        let start = r.pos.div_ceil(8);
                        let Some(lens) = body.get(start..start + 4) else {
                            return self.truncated(out);
                        };
                        let len = u16::from_le_bytes([lens[0], lens[1]]);
                        if len != !u16::from_le_bytes([lens[2], lens[3]]) {
                            return Err(BackendError::Malformed("stored LEN/NLEN"));
                        }
                        let len = usize::from(len);
                        let avail = body.len().saturating_sub(start + 4).min(len);
                        if self.over(out.len() + avail, limits) {
                            return self.limit(out);
                        }
                        out.extend_from_slice(&body[start + 4..start + 4 + avail]);
                        if avail < len {
                            return self.truncated(out);
                        }
                        r.pos = (start + 4 + len) * 8;
                    }
                    1 => loop {
                        let Some(sym) = r.fixed_symbol() else {
                            return self.truncated(out);
                        };
                        match sym {
                            0..=255 => {
                                if self.over(out.len() + 1, limits) {
                                    return self.limit(out);
                                }
                                out.push(u8::try_from(sym).unwrap());
                            }
                            256 => break,
                            257..=285 => {
                                let i = usize::try_from(sym - 257).unwrap();
                                let Some(extra) = r.lsb(LEN_EXTRA[i]) else {
                                    return self.truncated(out);
                                };
                                let len = LEN_BASE[i] + usize::try_from(extra).unwrap();
                                let Some(code) = r.msb(5) else {
                                    return self.truncated(out);
                                };
                                let d = usize::try_from(code).unwrap();
                                if d >= 30 {
                                    return Err(BackendError::Malformed("distance code"));
                                }
                                let Some(extra) = r.lsb(DIST_EXTRA[d]) else {
                                    return self.truncated(out);
                                };
                                let dist = DIST_BASE[d] + usize::try_from(extra).unwrap();
                                if dist > out.len() {
                                    return Err(BackendError::Malformed("distance"));
                                }
                                if self.over(out.len() + len, limits) {
                                    return self.limit(out);
                                }
                                for _ in 0..len {
                                    out.push(out[out.len() - dist]);
                                }
                            }
                            _ => return Err(BackendError::Malformed("symbol")),
                        }
                    },
                    _ => return Err(BackendError::Unsupported("dynamic Huffman")),
                }
                if last == 1 {
                    break;
                }
            }
            let end = r.pos.div_ceil(8);
            if format == FlateFormat::Zlib && body.get(end..end + 4).is_none() {
                return self.truncated(out);
            }
            Ok(Decoded {
                value: out,
                status: DecodeStatus::Complete,
            })
        }
    }

    impl Deflate for Mini {
        fn deflate(
            &self,
            input: &[u8],
            format: FlateFormat,
            _level: CompressionLevel,
            limits: &DecodeLimits,
        ) -> Result<Vec<u8>, BackendError> {
            self.deflate_calls.set(self.deflate_calls.get() + 1);
            let mut out = Vec::new();
            if format == FlateFormat::Zlib {
                out.extend_from_slice(&[0x78, 0x01]);
            }
            let mut chunks: Vec<&[u8]> = input.chunks(65_535).collect();
            if chunks.is_empty() {
                chunks.push(&[]);
            }
            let last = chunks.len() - 1;
            for (i, chunk) in chunks.into_iter().enumerate() {
                let len = u16::try_from(chunk.len()).unwrap();
                out.push(u8::from(i == last));
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(&(!len).to_le_bytes());
                out.extend_from_slice(chunk);
                if out.len() > limits.max_output_bytes {
                    return Err(BackendError::LimitExceeded("output"));
                }
            }
            if format == FlateFormat::Zlib {
                out.extend_from_slice(&adler32(input).to_be_bytes());
            }
            Ok(out)
        }
    }

    /// Every built-in vector except the dynamic-Huffman one.
    fn mini_vectors() -> Vec<Vector> {
        FLATE
            .iter()
            .filter(|v| v.name != "dynamic Huffman")
            .map(|v| Vector {
                name: v.name,
                plain: v.plain.map_or_else(dynamic_plaintext, <[u8]>::to_vec),
                zlib: hex(v.zlib_hex).unwrap(),
            })
            .collect()
    }

    fn run_mini(imp: &Mini) -> Report {
        let mut report = Report::new(SUITE);
        run_with(&mut report, imp, imp, &mini_vectors());
        report
    }

    #[test]
    fn a_correct_implementation_passes() {
        let imp = Mini::default();
        let report = run_mini(&imp);
        assert!(report.passed(), "{report}");
        assert!(report.checks_run > 1000, "{report}");
        assert!(imp.deflate_calls.get() >= 30);
    }

    #[test]
    fn the_full_suite_rejects_a_decoder_without_dynamic_huffman() {
        let imp = Mini::default();
        let report = run(&imp, &imp);
        assert!(report.failed("flate.decode_vector"), "{report}");
        assert!(
            report
                .failures()
                .iter()
                .all(|f| f.detail.starts_with("dynamic Huffman")),
            "{report}"
        );
    }

    #[test]
    fn the_large_bomb_decodes_to_zeros() {
        let bomb = fixed_huffman_zeros(1000);
        let imp = Mini::default();
        let decoded = imp
            .inflate(&bomb, FlateFormat::Raw, &DecodeLimits::default())
            .unwrap();
        assert_eq!(decoded.status, DecodeStatus::Complete);
        assert_eq!(decoded.value.len(), 1 + 4 * 258);
        assert!(decoded.value.iter().all(|&b| b == 0));
        assert_eq!(fixed_huffman_zeros(LARGE_BOMB_BYTES).len(), 6_762_913);
    }

    #[test]
    fn dropping_partial_output_is_caught() {
        let report = run_mini(&Mini {
            strict_truncation: true,
            ..Mini::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["flate.truncated_salvage"],
            "{report}"
        );
    }

    #[test]
    fn ignoring_the_output_limit_is_caught() {
        let report = run_mini(&Mini {
            ignore_limit: true,
            ..Mini::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["flate.large_bomb", "flate.output_limit", "flate.zip_bomb"],
            "{report}"
        );
    }

    #[test]
    fn a_panic_is_caught_and_reported() {
        let report = run_mini(&Mini {
            panic_on_empty: true,
            ..Mini::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["flate.empty_input", "flate.truncated"],
            "{report}"
        );
    }

    #[test]
    fn claiming_completeness_for_truncated_input_is_caught() {
        let report = run_mini(&Mini {
            complete_on_truncation: true,
            ..Mini::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec![
                "flate.empty_input",
                "flate.truncated",
                "flate.truncated_salvage"
            ],
            "{report}"
        );
    }

    #[test]
    fn zlib_framing_check() {
        let plain = b"stored block";
        let good = hex(FLATE[2].zlib_hex).unwrap();
        assert!(zlib_framing_ok(&good, plain));
        let mut bad_header = good.clone();
        bad_header[1] ^= 1;
        assert!(!zlib_framing_ok(&bad_header, plain));
        let mut fdict = good.clone();
        fdict[1] |= 0x20;
        assert!(!zlib_framing_ok(&fdict, plain));
        assert!(!zlib_framing_ok(&good, b"other"));
        assert!(!zlib_framing_ok(&good[..5], plain));
        assert!(!zlib_framing_ok(&[], plain));
    }
}
