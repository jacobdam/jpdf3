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

    /// A Flate implementation that only handles stored blocks (BTYPE 00): enough to exercise
    /// every suite check without a real compressor.
    #[derive(Default)]
    struct Stored {
        ignore_limit: bool,
        panic_on_empty: bool,
        complete_on_truncation: bool,
        deflate_calls: Cell<usize>,
    }

    impl Inflate for Stored {
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
            let mut pos = 0;
            let truncated = |out: Vec<u8>| {
                if self.complete_on_truncation {
                    Ok(Decoded {
                        value: out,
                        status: DecodeStatus::Complete,
                    })
                } else if out.is_empty() {
                    Err(BackendError::Truncated)
                } else {
                    Ok(Decoded {
                        value: out,
                        status: DecodeStatus::Truncated,
                    })
                }
            };
            loop {
                let Some(&head) = body.get(pos) else {
                    return truncated(out);
                };
                if head & 0b110 != 0 {
                    return Err(BackendError::Unsupported("compressed block"));
                }
                let Some(lens) = body.get(pos + 1..pos + 5) else {
                    return truncated(out);
                };
                let len = usize::from(u16::from_le_bytes([lens[0], lens[1]]));
                if u16::from_le_bytes([lens[0], lens[1]]) != !u16::from_le_bytes([lens[2], lens[3]])
                {
                    return Err(BackendError::Malformed("stored LEN/NLEN"));
                }
                let start = pos + 5;
                let avail = body.len().saturating_sub(start).min(len);
                if !self.ignore_limit && out.len() + avail > limits.max_output_bytes {
                    return Err(BackendError::LimitExceeded("output"));
                }
                out.extend_from_slice(&body[start..start + avail]);
                if avail < len {
                    return truncated(out);
                }
                pos = start + len;
                if head & 1 == 1 {
                    break;
                }
            }
            if format == FlateFormat::Zlib && body.get(pos..pos + 4).is_none() {
                return truncated(out);
            }
            Ok(Decoded {
                value: out,
                status: DecodeStatus::Complete,
            })
        }
    }

    impl Deflate for Stored {
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

    fn stored_vectors() -> Vec<Vector> {
        FLATE
            .iter()
            .filter(|v| v.name == "stored block")
            .map(|v| Vector {
                name: v.name,
                plain: v.plain.unwrap().to_vec(),
                zlib: hex(v.zlib_hex).unwrap(),
            })
            .collect()
    }

    fn run_stored(imp: &Stored) -> Report {
        let mut report = Report::new(SUITE);
        run_with(&mut report, imp, imp, &stored_vectors());
        report
    }

    #[test]
    fn a_correct_implementation_passes() {
        let imp = Stored::default();
        let report = run_stored(&imp);
        assert!(report.passed(), "{report}");
        assert!(report.checks_run > 100, "{report}");
        assert!(imp.deflate_calls.get() >= 30);
    }

    #[test]
    fn the_full_suite_rejects_a_stored_only_decoder() {
        // The built-in vectors include Huffman blocks, so `run` must not pass a decoder that
        // only knows stored blocks.
        let imp = Stored::default();
        let report = run(&imp, &imp);
        assert!(report.failed("flate.decode_vector"), "{report}");
    }

    #[test]
    fn ignoring_the_output_limit_is_caught() {
        let report = run_stored(&Stored {
            ignore_limit: true,
            ..Stored::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["flate.output_limit", "flate.zip_bomb"],
            "{report}"
        );
    }

    #[test]
    fn a_panic_is_caught_and_reported() {
        let report = run_stored(&Stored {
            panic_on_empty: true,
            ..Stored::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["flate.empty_input", "flate.truncated"],
            "{report}"
        );
    }

    #[test]
    fn claiming_completeness_for_truncated_input_is_caught() {
        let report = run_stored(&Stored {
            complete_on_truncation: true,
            ..Stored::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["flate.empty_input", "flate.truncated"],
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
