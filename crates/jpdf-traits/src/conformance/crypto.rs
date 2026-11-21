//! Conformance suites for [`Hasher`] and [`Cipher`] (ISO 32000-2 §7.6.3, §7.6.4): published
//! known-answer vectors plus the structural rules of the trait contracts.

use super::vectors::{AES_CBC, AES_IV, AES_PLAIN, HASH, RC4, RC4_LONG, SHA256_MILLION_A, hex};
use super::{Boundary, Report, describe, guarded, narrow, pseudo_random};
use crate::{BackendError, Cipher, DecodeLimits, Direction, HashAlgorithm, Hasher, Padding};

/// Runs every check against a [`Hasher`].
pub fn run_hasher(hasher: &dyn Hasher) -> Report {
    let mut report = Report::new("hasher");
    for (algorithm, input, digest) in &HASH {
        let message = input.bytes();
        let result = guarded(|| hasher.digest(*algorithm, &message));
        let expected = hex(digest);
        report.check(
            "crypto.hash_kat",
            matches!((&result, &expected), (Some(Ok(got)), Some(want)) if got == want),
            || format!("{algorithm:?}({input:?}): {}", describe(&result)),
        );
    }

    let data = pseudo_random(1000, 3);
    for algorithm in HashAlgorithm::ALL {
        let one_shot = guarded(|| hasher.digest(algorithm, &data));
        let Some(Ok(one_shot)) = one_shot else {
            report.check("crypto.hash_streaming", false, || {
                format!("{algorithm:?}: {}", describe(&one_shot))
            });
            continue;
        };
        report.check(
            "crypto.hash_output_len",
            one_shot.len() == algorithm.output_len(),
            || format!("{algorithm:?}: {} bytes", one_shot.len()),
        );
        for chunk in [1, 7, 64, 999] {
            let streamed = guarded(|| -> Result<Vec<u8>, BackendError> {
                let mut state = hasher.start(algorithm)?;
                for part in data.chunks(chunk) {
                    state.update(part);
                }
                Ok(state.finish())
            });
            report.check(
                "crypto.hash_streaming",
                matches!(&streamed, Some(Ok(d)) if *d == one_shot),
                || {
                    format!(
                        "{algorithm:?} in {chunk}-byte updates: {}",
                        describe(&streamed)
                    )
                },
            );
        }
        // Uneven two-part splits: a buffered implementation must carry a partial block
        // across updates (3 + 997, 63 + 937 and 127 + 873 bytes: one short of a 64-byte
        // block for MD5/SHA-1/SHA-256 and of a 128-byte block for SHA-384/512).
        for split in [3, 63, 127] {
            let streamed = guarded(|| -> Result<Vec<u8>, BackendError> {
                let mut state = hasher.start(algorithm)?;
                let (head, tail) = data.split_at(split);
                state.update(head);
                state.update(tail);
                Ok(state.finish())
            });
            report.check(
                "crypto.hash_streaming",
                matches!(&streamed, Some(Ok(d)) if *d == one_shot),
                || {
                    format!(
                        "{algorithm:?} as {split} + {} bytes: {}",
                        data.len().saturating_sub(split),
                        describe(&streamed)
                    )
                },
            );
        }
    }

    let million = guarded(|| -> Result<Vec<u8>, BackendError> {
        let mut state = hasher.start(HashAlgorithm::Sha256)?;
        let block = [b'a'; 1000];
        for _ in 0..1000 {
            state.update(&block);
        }
        Ok(state.finish())
    });
    report.check(
        "crypto.hash_kat",
        matches!((&million, hex(SHA256_MILLION_A)), (Some(Ok(got)), Some(want)) if *got == want),
        || format!("SHA-256 of a million 'a': {}", describe(&million)),
    );
    report
}

/// Runs every check against a [`Cipher`].
pub fn run_cipher(cipher: &dyn Cipher) -> Report {
    let mut report = Report::new("cipher");
    rc4_checks(&mut report, cipher);
    aes_checks(&mut report, cipher);
    report
}

fn rc4_checks(report: &mut Report, cipher: &dyn Cipher) {
    let unlimited = DecodeLimits::default();
    for (key, plain, expected) in &RC4 {
        let result = guarded(|| cipher.rc4(key, plain, &unlimited));
        report.check(
            "crypto.rc4_kat",
            matches!((&result, hex(expected)), (Some(Ok(got)), Some(want)) if *got == want),
            || {
                format!(
                    "key {:?}: {}",
                    String::from_utf8_lossy(key),
                    describe(&result)
                )
            },
        );
    }

    // RFC 6229 at offsets up to 4080: the keystream must run on, not restart or drift.
    for (key, offsets) in &RC4_LONG {
        let zeros = [0u8; 4096];
        let result = guarded(|| cipher.rc4(key, &zeros, &unlimited));
        for (offset, expected) in offsets {
            let got = match &result {
                Some(Ok(stream)) => stream.get(*offset..offset.saturating_add(16)),
                _ => None,
            };
            report.check(
                "crypto.rc4_kat",
                matches!((got, hex(expected)), (Some(got), Some(want)) if *got == want),
                || {
                    format!(
                        "{}-bit key, keystream offset {offset}: {}",
                        key.len().saturating_mul(8),
                        describe(&result)
                    )
                },
            );
        }
    }

    let data = pseudo_random(300, 5);
    for key_len in [1, 5, 16, 256] {
        let key = pseudo_random(key_len, 9);
        let round = guarded(|| -> Result<Vec<u8>, BackendError> {
            let once = cipher.rc4(&key, &data, &unlimited)?;
            cipher.rc4(&key, &once, &unlimited)
        });
        report.check(
            "crypto.rc4_round_trip",
            matches!(&round, Some(Ok(back)) if *back == data),
            || format!("{key_len}-byte key: {}", describe(&round)),
        );
    }

    for key_len in [0, 257] {
        let key = vec![1; key_len];
        let result = guarded(|| cipher.rc4(&key, &data, &unlimited));
        report.check(
            "crypto.key_length",
            matches!(result, Some(Err(BackendError::InvalidArgument(_)))),
            || {
                format!(
                    "RC4 {key_len}-byte key: expected InvalidArgument, got {}",
                    describe(&result)
                )
            },
        );
    }

    output_limit(report, "RC4 of 300 bytes", data.len(), |limits| {
        cipher.rc4(b"key", &data, limits)
    });
}

fn aes_checks(report: &mut Report, cipher: &dyn Cipher) {
    let unlimited = DecodeLimits::default();
    let aes = |direction, key: &[u8], iv: &[u8; 16], data: &[u8], padding| {
        guarded(|| cipher.aes_cbc(direction, key, iv, data, padding, &unlimited))
    };

    let plain = hex(AES_PLAIN);
    for (key_hex, cipher_hex) in &AES_CBC {
        let (Some(plain), Some(key), Some(expected)) = (&plain, hex(key_hex), hex(cipher_hex))
        else {
            report.check("crypto.builtin_vector", false, || {
                "bad AES vector hex".into()
            });
            continue;
        };
        let bits = key.len() * 8;
        let result = aes(Direction::Encrypt, &key, &AES_IV, plain, Padding::None);
        report.check(
            "crypto.aes_kat",
            matches!(&result, Some(Ok(got)) if *got == expected),
            || format!("AES-{bits}-CBC encrypt: {}", describe(&result)),
        );
        let result = aes(Direction::Decrypt, &key, &AES_IV, &expected, Padding::None);
        report.check(
            "crypto.aes_kat",
            matches!(&result, Some(Ok(got)) if got == plain),
            || format!("AES-{bits}-CBC decrypt: {}", describe(&result)),
        );
    }

    for key_len in [16, 32] {
        let key = pseudo_random(key_len, 13);
        let iv = [7u8; 16];
        for len in 0..=33usize {
            let data = pseudo_random(len, 17);
            let round = guarded(|| -> Result<(usize, Vec<u8>), BackendError> {
                let sealed = cipher.aes_cbc(
                    Direction::Encrypt,
                    &key,
                    &iv,
                    &data,
                    Padding::Pkcs7,
                    &unlimited,
                )?;
                let opened = cipher.aes_cbc(
                    Direction::Decrypt,
                    &key,
                    &iv,
                    &sealed,
                    Padding::Pkcs7,
                    &unlimited,
                )?;
                Ok((sealed.len(), opened))
            });
            let padded_len = (len / 16 + 1) * 16;
            report.check(
                "crypto.aes_padding",
                matches!(&round, Some(Ok((sealed, opened))) if *sealed == padded_len && *opened == data),
                || format!("AES-{} PKCS#7 round trip of {len} bytes: {}", key_len * 8, describe(&round)),
            );
        }

        // Known answers for the padding itself: PKCS #7 encryption equals unpadded encryption
        // of the data with its padding written out (n bytes of value n), and decryption of
        // that ciphertext removes exactly those bytes. A round trip alone accepts any
        // self-consistent padding (ANSI X.923's zeros, ISO 10126's random bytes).
        for len in [0usize, 1, 15, 16, 17, 31, 32, 33] {
            let data = pseudo_random(len, 19);
            let pad = 16usize.saturating_sub(len % 16);
            let mut padded = data.clone();
            padded.extend(std::iter::repeat_n(u8::try_from(pad).unwrap_or(0), pad));
            let reference = aes(Direction::Encrypt, &key, &iv, &padded, Padding::None);
            let Some(Ok(reference)) = reference else {
                report.check("crypto.aes_padding", false, || {
                    format!(
                        "encrypting {len} explicitly padded bytes: {}",
                        describe(&reference)
                    )
                });
                continue;
            };
            let sealed = aes(Direction::Encrypt, &key, &iv, &data, Padding::Pkcs7);
            report.check(
                "crypto.aes_padding",
                matches!(&sealed, Some(Ok(c)) if *c == reference),
                || format!("AES-{} PKCS#7 encryption of {len} bytes differs from the explicitly padded data: {}", key_len * 8, describe(&sealed)),
            );
            let opened = aes(Direction::Decrypt, &key, &iv, &reference, Padding::Pkcs7);
            report.check(
                "crypto.aes_padding",
                matches!(&opened, Some(Ok(p)) if *p == data),
                || {
                    format!(
                        "AES-{} PKCS#7 decryption of {len} explicitly padded bytes: {}",
                        key_len * 8,
                        describe(&opened)
                    )
                },
            );
        }

        // Invalid padding: a final block ending in 0x00, in 0x11 (> 16), or in a valid-looking
        // last byte whose earlier padding bytes are wrong (…, 3, 2, 3), (…, 0, 0, 2: valid
        // ANSI X.923) and (…, 1, 2).
        let wrong = |tail: &[u8]| {
            let mut block = [0x41u8; 16];
            let start = 16usize.saturating_sub(tail.len());
            if let Some(end) = block.get_mut(start..) {
                end.copy_from_slice(tail);
            }
            block
        };
        for probe in [
            [0x00u8; 16],
            [0x11; 16],
            wrong(&[3, 2, 3]),
            wrong(&[0, 0, 2]),
            wrong(&[1, 2]),
        ] {
            // Each probe once as a single block and once after a full first block, so a
            // check that only runs on one-block ciphertexts is caught.
            for lead in [0usize, 16] {
                let mut plain = vec![0x41u8; lead];
                plain.extend_from_slice(&probe);
                let sealed = aes(Direction::Encrypt, &key, &iv, &plain, Padding::None);
                let Some(Ok(sealed)) = sealed else {
                    report.check("crypto.aes_bad_padding", false, || {
                        format!("encrypting the probe block: {}", describe(&sealed))
                    });
                    continue;
                };
                let result = aes(Direction::Decrypt, &key, &iv, &sealed, Padding::Pkcs7);
                report.check(
                    "crypto.aes_bad_padding",
                    matches!(result, Some(Err(BackendError::Malformed(_)))),
                    || {
                        format!(
                            "final block {probe:02x?} after {lead} bytes: expected Malformed, got {}",
                            describe(&result)
                        )
                    },
                );
            }
        }

        // The IV must be used: two IVs give different first blocks.
        let first = aes(Direction::Encrypt, &key, &[0; 16], &[0; 16], Padding::None);
        let second = aes(Direction::Encrypt, &key, &[1; 16], &[0; 16], Padding::None);
        report.check(
            "crypto.aes_iv",
            matches!((&first, &second), (Some(Ok(a)), Some(Ok(b))) if a != b),
            || format!("{} vs {}", describe(&first), describe(&second)),
        );

        let lengths: [LengthCase; 4] = [
            (
                Direction::Decrypt,
                15,
                Padding::None,
                is_malformed,
                "Malformed",
            ),
            (
                Direction::Decrypt,
                17,
                Padding::Pkcs7,
                is_malformed,
                "Malformed",
            ),
            (
                Direction::Decrypt,
                0,
                Padding::Pkcs7,
                is_malformed,
                "Malformed",
            ),
            (
                Direction::Encrypt,
                15,
                Padding::None,
                is_invalid_argument,
                "InvalidArgument",
            ),
        ];
        for (direction, len, padding, expected, want) in lengths {
            let result = aes(direction, &key, &iv, &vec![0; len], padding);
            report.check(
                "crypto.aes_length",
                matches!(&result, Some(Err(e)) if expected(e)),
                || {
                    format!(
                        "{direction:?} {len} bytes {padding:?}: expected {want}, got {}",
                        describe(&result)
                    )
                },
            );
        }
        let result = aes(Direction::Decrypt, &key, &iv, &[], Padding::None);
        report.check(
            "crypto.aes_length",
            matches!(&result, Some(Ok(v)) if v.is_empty()),
            || format!("decrypting 0 bytes without padding: {}", describe(&result)),
        );
    }

    for key_len in [0, 15, 24, 33] {
        let key = vec![3; key_len];
        for direction in [Direction::Encrypt, Direction::Decrypt] {
            let result = aes(direction, &key, &AES_IV, &[0; 16], Padding::None);
            report.check(
                "crypto.key_length",
                matches!(result, Some(Err(BackendError::InvalidArgument(_)))),
                || format!("AES {direction:?} with a {key_len}-byte key: expected InvalidArgument, got {}", describe(&result)),
            );
        }
    }

    let key = [9u8; 16];
    // Decryption: the cap applies to the ciphertext length, which is known before
    // allocating (48 bytes of ciphertext holding 32 bytes of plaintext).
    if let Some(Ok(sealed)) = aes(Direction::Encrypt, &key, &AES_IV, &[5; 32], Padding::Pkcs7) {
        Boundary {
            check: "crypto.output_limit",
            name: "AES decryption of 48 bytes (32 of plaintext)",
            at: 48,
        }
        .probe(
            report,
            cap,
            |limits| {
                guarded(|| {
                    cipher.aes_cbc(
                        Direction::Decrypt,
                        &key,
                        &AES_IV,
                        &sealed,
                        Padding::Pkcs7,
                        limits,
                    )
                })
            },
            |v| *v == [5; 32],
            describe,
        );
    } else {
        report.check("crypto.output_limit", false, || {
            "encrypting the probe failed".into()
        });
    }
    output_limit(report, "AES encryption of 32 bytes", 48, |limits| {
        cipher.aes_cbc(
            Direction::Encrypt,
            &key,
            &AES_IV,
            &[0; 32],
            Padding::Pkcs7,
            limits,
        )
    });
}

/// A length probe: direction, data length, padding, expected error test, its name.
type LengthCase = (
    Direction,
    usize,
    Padding,
    fn(&BackendError) -> bool,
    &'static str,
);

fn is_malformed(e: &BackendError) -> bool {
    matches!(e, BackendError::Malformed(_))
}

fn is_invalid_argument(e: &BackendError) -> bool {
    matches!(e, BackendError::InvalidArgument(_))
}

/// Generous limits with the output cap at `n`.
fn cap(n: u64) -> DecodeLimits {
    DecodeLimits {
        max_output_bytes: narrow(n),
        ..DecodeLimits::default()
    }
}

/// `call` produces `len` bytes: the output limit is probed at its exact boundary and at 0.
fn output_limit(
    report: &mut Report,
    name: &str,
    len: usize,
    call: impl Fn(&DecodeLimits) -> Result<Vec<u8>, BackendError>,
) {
    Boundary {
        check: "crypto.output_limit",
        name,
        at: u64::try_from(len).unwrap_or(u64::MAX),
    }
    .probe(
        report,
        cap,
        |limits| guarded(|| call(limits)),
        |v| v.len() == len,
        describe,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HashState;

    /// A hasher that knows the published answers and otherwise computes FNV-1a stretched to
    /// the digest length: enough to exercise every check's wiring.
    struct TableHasher {
        knows_answers: bool,
        short_digest: bool,
    }

    struct TableState {
        algorithm: HashAlgorithm,
        data: Vec<u8>,
        knows_answers: bool,
        short_digest: bool,
    }

    impl HashState for TableState {
        fn update(&mut self, data: &[u8]) {
            self.data.extend_from_slice(data);
        }
        fn finish(self: Box<Self>) -> Vec<u8> {
            if self.knows_answers {
                if let Some((_, _, d)) = HASH
                    .iter()
                    .find(|(a, m, _)| *a == self.algorithm && m.bytes() == self.data)
                {
                    return hex(d).unwrap();
                }
                if self.algorithm == HashAlgorithm::Sha256 && self.data == vec![b'a'; 1_000_000] {
                    return hex(SHA256_MILLION_A).unwrap();
                }
            }
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for &b in &self.data {
                h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
            }
            let len = self.algorithm.output_len() - usize::from(self.short_digest);
            h.to_be_bytes().iter().copied().cycle().take(len).collect()
        }
    }

    impl Hasher for TableHasher {
        fn start(&self, algorithm: HashAlgorithm) -> Result<Box<dyn HashState>, BackendError> {
            Ok(Box::new(TableState {
                algorithm,
                data: Vec::new(),
                knows_answers: self.knows_answers,
                short_digest: self.short_digest,
            }))
        }
    }

    #[test]
    fn hasher_suite_wiring() {
        let report = run_hasher(&TableHasher {
            knows_answers: true,
            short_digest: false,
        });
        assert!(report.passed(), "{report}");

        let report = run_hasher(&TableHasher {
            knows_answers: false,
            short_digest: false,
        });
        assert_eq!(report.failed_checks(), vec!["crypto.hash_kat"], "{report}");
        assert_eq!(report.failures().len(), HASH.len() + 1, "{report}");

        let report = run_hasher(&TableHasher {
            knows_answers: true,
            short_digest: true,
        });
        assert!(report.failed("crypto.hash_output_len"), "{report}");
    }

    /// Real RC4 and a fake 16-byte block cipher in CBC mode with real PKCS#7 handling: every
    /// check except the AES known answers must pass.
    #[derive(Default)]
    struct FakeCipher {
        ignore_iv: bool,
        lenient_padding: bool,
        last_byte_only_padding: bool,
        decrypt_cap_on_plaintext: bool,
        /// ANSI X.923 padding (zeros, then the count) in both directions.
        x923_padding: bool,
        /// Treats a cap of 0 as "no limit", as some libraries do.
        zero_unlimited: bool,
        /// Restarts the RC4 keystream every 16 bytes.
        rc4_restart_16: bool,
    }

    impl FakeCipher {
        fn cap(&self, limits: &DecodeLimits) -> usize {
            if self.zero_unlimited && limits.max_output_bytes == 0 {
                usize::MAX
            } else {
                limits.max_output_bytes
            }
        }
    }

    fn rc4(key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut s: Vec<u8> = (0..=255).collect();
        let mut j = 0u8;
        for i in 0..256 {
            j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
            s.swap(i, usize::from(j));
        }
        let (mut i, mut j) = (0u8, 0u8);
        data.iter()
            .map(|&b| {
                i = i.wrapping_add(1);
                j = j.wrapping_add(s[usize::from(i)]);
                s.swap(usize::from(i), usize::from(j));
                b ^ s[usize::from(s[usize::from(i)].wrapping_add(s[usize::from(j)]))]
            })
            .collect()
    }

    impl Cipher for FakeCipher {
        fn rc4(
            &self,
            key: &[u8],
            data: &[u8],
            limits: &DecodeLimits,
        ) -> Result<Vec<u8>, BackendError> {
            if key.is_empty() || key.len() > 256 {
                return Err(BackendError::InvalidArgument("RC4 key length"));
            }
            if data.len() > self.cap(limits) {
                return Err(BackendError::LimitExceeded("output"));
            }
            if self.rc4_restart_16 {
                return Ok(data.chunks(16).flat_map(|c| rc4(key, c)).collect());
            }
            Ok(rc4(key, data))
        }

        fn aes_cbc(
            &self,
            direction: Direction,
            key: &[u8],
            iv: &[u8; 16],
            data: &[u8],
            padding: Padding,
            limits: &DecodeLimits,
        ) -> Result<Vec<u8>, BackendError> {
            if key.len() != 16 && key.len() != 32 {
                return Err(BackendError::InvalidArgument("AES key length"));
            }
            let k: Vec<u8> = (0..16).map(|i| key[i] ^ key[key.len() - 16 + i]).collect();
            let iv = if self.ignore_iv { [0; 16] } else { *iv };
            match direction {
                Direction::Encrypt => {
                    let mut input = data.to_vec();
                    match padding {
                        Padding::Pkcs7 => {
                            let pad = 16 - data.len() % 16;
                            let fill = if self.x923_padding { 0 } else { pad };
                            input.extend(std::iter::repeat_n(u8::try_from(fill).unwrap(), pad - 1));
                            input.push(u8::try_from(pad).unwrap());
                        }
                        Padding::None if !data.len().is_multiple_of(16) => {
                            return Err(BackendError::InvalidArgument("length"));
                        }
                        Padding::None => {}
                    }
                    if input.len() > self.cap(limits) {
                        return Err(BackendError::LimitExceeded("output"));
                    }
                    let mut prev = iv.to_vec();
                    let mut out = Vec::new();
                    for block in input.chunks(16) {
                        let mut c: Vec<u8> = (0..16).map(|i| block[i] ^ prev[i] ^ k[i]).collect();
                        c.rotate_left(1);
                        out.extend_from_slice(&c);
                        prev = c;
                    }
                    Ok(out)
                }
                Direction::Decrypt => {
                    if !data.len().is_multiple_of(16) {
                        return Err(BackendError::Malformed("length"));
                    }
                    if !self.decrypt_cap_on_plaintext && data.len() > self.cap(limits) {
                        return Err(BackendError::LimitExceeded("output"));
                    }
                    let mut prev = iv.to_vec();
                    let mut out = Vec::new();
                    for block in data.chunks(16) {
                        let mut d = block.to_vec();
                        d.rotate_right(1);
                        out.extend((0..16).map(|i| d[i] ^ prev[i] ^ k[i]));
                        prev = block.to_vec();
                    }
                    if padding == Padding::Pkcs7 {
                        let pad = usize::from(*out.last().ok_or(BackendError::Malformed("empty"))?);
                        let fill = if self.x923_padding { 0 } else { pad };
                        let ok = (1..=16).contains(&pad)
                            && (self.last_byte_only_padding
                                || out[out.len() - pad..out.len() - 1]
                                    .iter()
                                    .all(|&b| usize::from(b) == fill));
                        if !ok && !self.lenient_padding {
                            return Err(BackendError::Malformed("padding"));
                        }
                        out.truncate(out.len() - pad.min(out.len()));
                    }
                    if out.len() > self.cap(limits) {
                        return Err(BackendError::LimitExceeded("output"));
                    }
                    Ok(out)
                }
            }
        }
    }

    #[test]
    fn cipher_suite_wiring() {
        let report = run_cipher(&FakeCipher::default());
        assert_eq!(report.failed_checks(), vec!["crypto.aes_kat"], "{report}");
        assert_eq!(report.failures().len(), 4, "{report}");

        let report = run_cipher(&FakeCipher {
            ignore_iv: true,
            ..FakeCipher::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["crypto.aes_iv", "crypto.aes_kat"],
            "{report}"
        );

        let report = run_cipher(&FakeCipher {
            lenient_padding: true,
            ..FakeCipher::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["crypto.aes_bad_padding", "crypto.aes_kat"],
            "{report}"
        );

        let report = run_cipher(&FakeCipher {
            last_byte_only_padding: true,
            ..FakeCipher::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["crypto.aes_bad_padding", "crypto.aes_kat"],
            "{report}"
        );

        let report = run_cipher(&FakeCipher {
            decrypt_cap_on_plaintext: true,
            ..FakeCipher::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["crypto.aes_kat", "crypto.output_limit"],
            "{report}"
        );

        // Self-consistent X.923 padding round-trips; the explicit-padding answers and the
        // `…, 0, 0, 2` probe catch it.
        let report = run_cipher(&FakeCipher {
            x923_padding: true,
            ..FakeCipher::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec![
                "crypto.aes_bad_padding",
                "crypto.aes_kat",
                "crypto.aes_padding"
            ],
            "{report}"
        );
        assert!(
            report
                .failures()
                .iter()
                .any(|f| f.check == "crypto.aes_bad_padding" && f.detail.contains("00, 00, 02]")),
            "{report}"
        );

        // A cap of 0 read as "unlimited" passes every at/one-below probe; only cap 0 fails.
        let report = run_cipher(&FakeCipher {
            zero_unlimited: true,
            ..FakeCipher::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["crypto.aes_kat", "crypto.output_limit"],
            "{report}"
        );
        assert!(
            report
                .failures()
                .iter()
                .filter(|f| f.check == "crypto.output_limit")
                .all(|f| f.detail.contains("limit 0 (zero)")),
            "{report}"
        );

        // A keystream that restarts every 16 bytes matches every 16-byte known answer and
        // round-trips; only the long RFC 6229 offsets catch it.
        let report = run_cipher(&FakeCipher {
            rc4_restart_16: true,
            ..FakeCipher::default()
        });
        assert_eq!(
            report.failed_checks(),
            vec!["crypto.aes_kat", "crypto.rc4_kat"],
            "{report}"
        );
        assert_eq!(
            report
                .failures()
                .iter()
                .filter(|f| f.check == "crypto.rc4_kat")
                .count(),
            16,
            "{report}"
        );
    }
}
