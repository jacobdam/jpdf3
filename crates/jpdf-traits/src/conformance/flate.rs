//! Conformance suite for [`Inflate`] and [`Deflate`] (ISO 32000-2 §7.4.4; RFC 1950, 1951).

use super::vectors::{FLATE, dynamic_plaintext, hex};
use super::{Boundary, Report, describe, guarded, narrow, pseudo_random, sample_offsets};
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
    vectors.extend(generated_vectors());
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
                // An empty salvage is `Err(Truncated)`, never `Ok(Truncated)` with no bytes.
                let ok = match &result {
                    Some(Err(_)) => true,
                    Some(Ok(d)) => {
                        d.status == DecodeStatus::Truncated
                            && !d.value.is_empty()
                            && v.plain.starts_with(&d.value)
                    }
                    None => false,
                };
                report.check("flate.truncated", ok, || {
                    format!(
                        "{} ({format:?}) cut at {cut}/{}: expected an error or a non-empty \
                         truncated prefix of the plaintext, got {}",
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

        // Exact output limit in both formats, and a cap of 0 (which an empty plaintext
        // must pass).
        for format in FORMATS {
            let name = format!("{} ({format:?})", v.name);
            Boundary {
                check: "flate.output_limit",
                name: &name,
                at: u64::try_from(v.plain.len()).unwrap_or(u64::MAX),
            }
            .probe(
                report,
                cap,
                |limits| call(v.encoded(format), format, limits),
                |d| d.status == DecodeStatus::Complete && d.value == v.plain,
                describe_decoded,
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

// RFC 1951 §3.2.5 length and distance tables.
const LEN_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// The suite's own vectors, built bit by bit so they reach what short compressor output
/// never does: distances up to 32 768 (codes 22–29), output past 64 KiB (a window that wraps
/// twice), every length code including 284 with all 5 extra bits set (length 258, which
/// zlib accepts), overlapping copies, an empty stored block, and stored, fixed and dynamic
/// blocks in one stream.
fn generated_vectors() -> Vec<Vector> {
    vec![long_distances(), mixed_blocks()]
}

/// Stored, empty stored and fixed-Huffman blocks only (no dynamic tables), more than 64 KiB.
fn long_distances() -> Vector {
    let mut s = StreamBuilder::default();
    s.stored(false, &pseudo_random(40_000, 21));
    s.stored(false, &[]);
    s.huffman(false, &Codes::fixed());
    let lit = |s: &mut StreamBuilder, text: &[u8]| text.iter().for_each(|&b| s.literal(b));
    lit(&mut s, b"jpdf");
    // Every distance code at the bottom and the top of its range, each with a different
    // length code (cycling through all 29, extra bits at their maximum on odd steps).
    let mut step = 0usize;
    for (&base, &extra) in DIST_BASE.iter().zip(&DIST_EXTRA) {
        let top = base.saturating_add((1usize << extra).saturating_sub(1));
        for dist in [base, top] {
            let len_code = step % LEN_BASE.len();
            let len_extra = if step % 2 == 1 {
                (1u32 << LEN_EXTRA.get(len_code).copied().unwrap_or(0)).saturating_sub(1)
            } else {
                0
            };
            s.copy_coded(len_code, len_extra, dist);
            step = step.saturating_add(1);
        }
    }
    // Length 258 as code 284 with extra bits 31 (RFC 1951 §3.2.5 lists 284 for 227–257;
    // consensus decoders read 31 as 258), then overlapping copies (distance below length).
    s.copy_coded(27, 31, 1000);
    s.copy(258, 3);
    s.copy(100, 1);
    lit(&mut s, b"wrap");
    s.end_block();
    s.stored(false, &pseudo_random(30_000, 22));
    s.huffman(true, &Codes::fixed());
    // Past 64 KiB of output: the longest distances again, so a 32 KiB ring buffer has wrapped.
    for dist in [32_768, 24_577, 32_768, 16_385, 1] {
        s.copy(258, dist);
    }
    s.end_block();
    s.finish("generated: long distances, > 64 KiB")
}

/// Stored, fixed and dynamic blocks in one stream, ending with an empty final stored block.
/// The dynamic block uses its own complete code (not the fixed lengths, which zlib would
/// reject as an incomplete set).
fn mixed_blocks() -> Vector {
    let mut s = StreamBuilder::default();
    s.stored(false, &pseudo_random(300, 23));
    s.huffman(false, &Codes::fixed());
    for &b in b"fixed block; " {
        s.literal(b);
    }
    s.copy(40, 313);
    s.end_block();
    // Literal/length: 0–255 at 9 bits, 256–257 at 5, 258–285 at 6 (sum of 2^-len is 1);
    // distance: codes 0–1 at 4 bits, 2–29 at 5.
    let mut litlen = vec![9u8; 256];
    litlen.extend([5, 5]);
    litlen.extend([6; 28]);
    let mut dist = vec![4u8, 4];
    dist.extend([5; 28]);
    s.dynamic(false, &litlen, &dist);
    for &b in b"dynamic block \x00\xff; " {
        s.literal(b);
    }
    s.copy(258, 1);
    s.copy(3, 300);
    s.copy(17, 2);
    s.end_block();
    s.stored(true, &[]);
    s.finish("dynamic Huffman, mixed with stored and fixed blocks")
}

/// Huffman codes for one block: per symbol, (code, bit length); length 0 = unused.
struct Codes {
    litlen: Vec<(u32, u32)>,
    dist: Vec<(u32, u32)>,
}

impl Codes {
    /// RFC 1951 §3.2.6 fixed codes.
    fn fixed() -> Self {
        let mut litlen = vec![8u8; 144];
        litlen.extend([9; 112]);
        litlen.extend([7; 24]);
        litlen.extend([8; 8]);
        Self {
            litlen: canonical(&litlen),
            dist: canonical(&[5; 30]),
        }
    }
}

/// Canonical Huffman codes from bit lengths (RFC 1951 §3.2.2).
fn canonical(lengths: &[u8]) -> Vec<(u32, u32)> {
    let mut count = [0u32; 16];
    for &len in lengths {
        if let Some(c) = count.get_mut(usize::from(len)) {
            *c = c.saturating_add(1);
        }
    }
    count[0] = 0;
    // next[bits] = (next[bits - 1] + count[bits - 1]) << 1, with next[0] = 0.
    let mut next = [0u32; 16];
    let mut code = 0u32;
    for (n, &c) in next.iter_mut().skip(1).zip(&count) {
        code = code.saturating_add(c) << 1;
        *n = code;
    }
    lengths
        .iter()
        .map(|&len| {
            let len = usize::from(len);
            match next.get_mut(len) {
                Some(n) if len > 0 => {
                    let code = *n;
                    *n = n.saturating_add(1);
                    (code, u32::try_from(len).unwrap_or(0))
                }
                _ => (0, 0),
            }
        })
        .collect()
}

/// Writes a raw deflate stream and its plaintext side by side.
#[derive(Default)]
struct StreamBuilder {
    bits: BitWriter,
    plain: Vec<u8>,
    codes: Option<Codes>,
}

impl StreamBuilder {
    /// A stored block (RFC 1951 §3.2.4): header bits, pad to a byte, LEN, NLEN, the bytes.
    fn stored(&mut self, last: bool, data: &[u8]) {
        self.bits.lsb_first(u32::from(last), 1);
        self.bits.lsb_first(0, 2);
        self.bits.align();
        let len = u16::try_from(data.len()).unwrap_or(u16::MAX);
        self.bits.lsb_first(u32::from(len), 16);
        self.bits.lsb_first(u32::from(!len), 16);
        let data = data.get(..usize::from(len)).unwrap_or(&[]);
        for &b in data {
            self.bits.lsb_first(u32::from(b), 8);
        }
        self.plain.extend_from_slice(data);
    }

    /// Starts a fixed-Huffman block (BTYPE 01) coded with `codes` (normally
    /// [`Codes::fixed`]).
    fn huffman(&mut self, last: bool, codes: &Codes) {
        self.bits.lsb_first(u32::from(last), 1);
        self.bits.lsb_first(1, 2);
        self.codes = Some(Codes {
            litlen: codes.litlen.clone(),
            dist: codes.dist.clone(),
        });
    }

    /// Starts a dynamic-Huffman block (BTYPE 10, RFC 1951 §3.2.7) with the given complete
    /// literal/length (286) and distance (30) code lengths, each sent as a code-length
    /// symbol (no run-length codes 16–18).
    fn dynamic(&mut self, last: bool, litlen: &[u8], dist: &[u8]) {
        const ORDER: [usize; 19] = [
            16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
        ];
        self.bits.lsb_first(u32::from(last), 1);
        self.bits.lsb_first(2, 2);
        self.bits.lsb_first(
            u32::try_from(litlen.len().saturating_sub(257)).unwrap_or(0),
            5,
        );
        self.bits
            .lsb_first(u32::try_from(dist.len().saturating_sub(1)).unwrap_or(0), 5);
        // Code-length code: 2 bits for each length in use (at most 4 distinct, so complete).
        let mut used: Vec<u8> = litlen.iter().chain(dist).copied().collect();
        used.sort_unstable();
        used.dedup();
        let mut cl_lengths = [0u8; 19];
        for &len in &used {
            if let Some(l) = cl_lengths.get_mut(usize::from(len)) {
                *l = 2;
            }
        }
        let hclen = ORDER
            .iter()
            .rposition(|&sym| cl_lengths.get(sym).is_some_and(|&l| l > 0))
            .map_or(4, |i| i.saturating_add(1).max(4));
        self.bits
            .lsb_first(u32::try_from(hclen.saturating_sub(4)).unwrap_or(0), 4);
        for &sym in ORDER.iter().take(hclen) {
            self.bits
                .lsb_first(u32::from(cl_lengths.get(sym).copied().unwrap_or(0)), 3);
        }
        let cl_codes = canonical(&cl_lengths);
        for &len in litlen.iter().chain(dist) {
            let (code, bits) = cl_codes.get(usize::from(len)).copied().unwrap_or((0, 0));
            self.bits.code(code, bits);
        }
        self.codes = Some(Codes {
            litlen: canonical(litlen),
            dist: canonical(dist),
        });
    }

    fn symbol(&mut self, sym: usize) {
        let (code, bits) = self
            .codes
            .as_ref()
            .and_then(|c| c.litlen.get(sym))
            .copied()
            .unwrap_or((0, 0));
        self.bits.code(code, bits);
    }

    fn literal(&mut self, byte: u8) {
        self.symbol(usize::from(byte));
        self.plain.push(byte);
    }

    /// A copy of `len` bytes from `dist` back, coded the usual way.
    fn copy(&mut self, len: usize, dist: usize) {
        let code = if len == 258 {
            LEN_BASE.len().saturating_sub(1)
        } else {
            LEN_BASE.iter().rposition(|&b| b <= len).unwrap_or(0)
        };
        let extra = LEN_BASE.get(code).map_or(0, |&b| len.saturating_sub(b));
        self.copy_coded(code, u32::try_from(extra).unwrap_or(0), dist);
    }

    /// A copy with an explicit length code (index into [`LEN_BASE`]) and extra-bits value.
    fn copy_coded(&mut self, len_code: usize, len_extra: u32, dist: usize) {
        self.symbol(len_code.saturating_add(257));
        self.bits
            .lsb_first(len_extra, LEN_EXTRA.get(len_code).copied().unwrap_or(0));
        let dist_code = DIST_BASE.iter().rposition(|&b| b <= dist).unwrap_or(0);
        let (code, bits) = self
            .codes
            .as_ref()
            .and_then(|c| c.dist.get(dist_code))
            .copied()
            .unwrap_or((0, 0));
        self.bits.code(code, bits);
        let dist_extra = DIST_BASE
            .get(dist_code)
            .map_or(0, |&b| dist.saturating_sub(b));
        self.bits.lsb_first(
            u32::try_from(dist_extra).unwrap_or(0),
            DIST_EXTRA.get(dist_code).copied().unwrap_or(0),
        );
        let len = LEN_BASE.get(len_code).map_or(0, |&b| {
            b.saturating_add(usize::try_from(len_extra).unwrap_or(0))
        });
        for _ in 0..len {
            let byte = self
                .plain
                .len()
                .checked_sub(dist)
                .and_then(|i| self.plain.get(i))
                .copied()
                .unwrap_or(0);
            self.plain.push(byte);
        }
    }

    fn end_block(&mut self) {
        self.symbol(256);
    }

    /// Wraps the raw stream in zlib framing (`78 01`, Adler-32).
    fn finish(self, name: &'static str) -> Vector {
        let mut zlib = vec![0x78, 0x01];
        zlib.extend(self.bits.finish());
        zlib.extend_from_slice(&adler32(&self.plain).to_be_bytes());
        Vector {
            name,
            plain: self.plain,
            zlib,
        }
    }
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

    /// Pads with zero bits to the next byte boundary.
    fn align(&mut self) {
        while self.used > 0 {
            self.bit(0);
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

    // Deflate's output limit at its exact boundary: the cap equal to the adapter's own
    // output length for this input passes (compression must be deterministic), one below and
    // 0 fail. Random input does not compress, so the output is about 64 KiB.
    let input = pseudo_random(65_536, 11);
    for format in FORMATS {
        let level = CompressionLevel::Default;
        let reference = guarded(|| deflate.deflate(&input, format, level, &unlimited));
        let Some(Ok(reference)) = reference else {
            report.check("flate.deflate_output_limit", false, || {
                format!("{format:?}: deflate {}", describe(&reference))
            });
            continue;
        };
        let name = format!("64 KiB random ({format:?})");
        Boundary {
            check: "flate.deflate_output_limit",
            name: &name,
            at: u64::try_from(reference.len()).unwrap_or(u64::MAX),
        }
        .probe(
            report,
            cap,
            |limits| guarded(|| deflate.deflate(&input, format, level, limits)),
            |out| *out == reference,
            describe,
        );
    }
}

/// Generous limits with the output cap at `n`.
fn cap(n: u64) -> DecodeLimits {
    DecodeLimits {
        max_output_bytes: narrow(n),
        ..DecodeLimits::default()
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
        /// Rejects distances over 2048 as Malformed (a decoder with a small window).
        small_window: bool,
        /// Treats a cap of 0 as "no limit" in both directions.
        zero_unlimited: bool,
        /// Returns `Ok(Truncated)` with no bytes instead of `Err(Truncated)`.
        empty_truncated_ok: bool,
        deflate_calls: Cell<usize>,
    }

    const SAFETY_NET: usize = 32 << 20;
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
        fn cap(&self, limits: &DecodeLimits) -> usize {
            if self.zero_unlimited && limits.max_output_bytes == 0 {
                usize::MAX
            } else {
                limits.max_output_bytes
            }
        }

        fn over(&self, len: usize, limits: &DecodeLimits) -> bool {
            len > if self.ignore_limit {
                SAFETY_NET
            } else {
                self.cap(limits)
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
            } else if out.is_empty() && !self.empty_truncated_ok {
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
                                if dist > out.len() || (self.small_window && dist > 2048) {
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
                if out.len() > self.cap(limits) {
                    return Err(BackendError::LimitExceeded("output"));
                }
            }
            if format == FlateFormat::Zlib {
                out.extend_from_slice(&adler32(input).to_be_bytes());
                if out.len() > self.cap(limits) {
                    return Err(BackendError::LimitExceeded("output"));
                }
            }
            Ok(out)
        }
    }

    /// Every built-in and generated vector without a dynamic-Huffman block.
    fn mini_vectors() -> Vec<Vector> {
        FLATE
            .iter()
            .filter(|v| v.name != "dynamic Huffman")
            .map(|v| Vector {
                name: v.name,
                plain: v.plain.map_or_else(dynamic_plaintext, <[u8]>::to_vec),
                zlib: hex(v.zlib_hex).unwrap(),
            })
            .chain([long_distances()])
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

    /// The generated vectors were checked against CPython's zlib (`decompressobj` reaches
    /// `eof` with no unused data and returns the plaintext, in both framings) when written.
    /// The fingerprints make any change to the generator visible, so it gets re-checked.
    #[test]
    fn generated_vectors_are_pinned() {
        let v = long_distances();
        assert_eq!((v.zlib.len(), v.plain.len()), (70_205, 75_617));
        assert!(v.plain.len() > 65_536);
        assert_eq!(adler32(&v.zlib), 0x0a19_3681);
        let v = mixed_blocks();
        assert_eq!((v.zlib.len(), v.plain.len()), (444, 649));
        assert_eq!(adler32(&v.zlib), 0xc7a5_fa9c);
        // The stream starts with a non-final stored block (BFINAL 0, BTYPE 00).
        assert_eq!(v.zlib[2] & 0b111, 0b000);
    }

    #[test]
    fn canonical_codes_match_rfc_1951_example() {
        // RFC 1951 §3.2.2: lengths (3, 3, 3, 3, 3, 2, 4, 4) give these codes.
        let codes = canonical(&[3, 3, 3, 3, 3, 2, 4, 4]);
        let want = [
            (0b010, 3),
            (0b011, 3),
            (0b100, 3),
            (0b101, 3),
            (0b110, 3),
            (0b00, 2),
            (0b1110, 4),
            (0b1111, 4),
        ];
        assert_eq!(codes, want);
        // Fixed codes (§3.2.6): literal 0 is 00110000, 144 is 110010000, 256 is 0000000,
        // 280 is 11000000.
        let fixed = Codes::fixed();
        assert_eq!(fixed.litlen[0], (0x30, 8));
        assert_eq!(fixed.litlen[144], (0x190, 9));
        assert_eq!(fixed.litlen[256], (0, 7));
        assert_eq!(fixed.litlen[280], (0xc0, 8));
        assert_eq!(fixed.dist[29], (29, 5));
    }

    #[test]
    fn a_small_window_is_caught() {
        let report = run_mini(&Mini {
            small_window: true,
            ..Mini::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec![
                "flate.decode_vector",
                "flate.output_limit",
                "flate.truncated_salvage"
            ],
            "{report}"
        );
        assert!(
            report
                .failures()
                .iter()
                .all(|f| f.detail.starts_with("generated: long distances")),
            "{report}"
        );
    }

    #[test]
    fn reading_a_zero_cap_as_unlimited_is_caught() {
        let report = run_mini(&Mini {
            zero_unlimited: true,
            ..Mini::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["flate.deflate_output_limit", "flate.output_limit"],
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
    fn an_empty_ok_truncated_is_caught() {
        let report = run_mini(&Mini {
            empty_truncated_ok: true,
            ..Mini::default()
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
