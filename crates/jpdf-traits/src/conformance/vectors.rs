//! Built-in test vectors. Flate vectors were produced once with CPython's `zlib.compress`
//! (zlib 1.3) and checked by decompressing them; hash and cipher values are the published
//! examples of RFC 1321, FIPS 180-4, SP 800-38A (F.2.1, F.2.2, F.2.5, F.2.6) and the
//! well-known RC4 test vectors, each re-checked with `openssl`/`hashlib` when added.

/// Decodes a lowercase hex string; `None` on odd length or a non-hex character.
pub(crate) fn hex(s: &str) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let (pairs, _) = bytes.as_chunks::<2>();
    pairs
        .iter()
        .map(|&[hi, lo]| {
            let hi = char::from(hi).to_digit(16)?;
            let lo = char::from(lo).to_digit(16)?;
            u8::try_from(hi * 16 + lo).ok()
        })
        .collect()
}

/// The plaintext of the dynamic-Huffman vector: `jpdf 0; jpdf 1; … jpdf 199; `.
pub(crate) fn dynamic_plaintext() -> Vec<u8> {
    (0..200)
        .flat_map(|i| format!("jpdf {i}; ").into_bytes())
        .collect()
}

/// A zlib vector: name, plaintext (`None` = [`dynamic_plaintext`]), compressed hex.
pub(crate) struct FlateVector {
    pub name: &'static str,
    pub plain: Option<&'static [u8]>,
    pub zlib_hex: &'static str,
}

pub(crate) const FLATE: [FlateVector; 4] = [
    FlateVector {
        name: "empty, fixed Huffman",
        plain: Some(b""),
        zlib_hex: "78da030000000001",
    },
    FlateVector {
        name: "short, fixed Huffman",
        plain: Some(b"hello, hello, hello"),
        zlib_hex: "78dacb48cdc9c9d751c840a200442806d5",
    },
    FlateVector {
        name: "stored block",
        plain: Some(b"stored block"),
        zlib_hex: "7801010c00f3ff73746f72656420626c6f636b1f8004bd",
    },
    FlateVector {
        name: "dynamic Huffman",
        plain: None,
        zlib_hex: concat!(
            "78da4dd5bb6dc4301405d156b6045efe09b76338d8c8fd47b601cdd0d1cbe609120ff5fefefc7a958fd7fb6f",
            "e699f599ed99fd99e399f399eb99fb99878e418a21199a211aaa211bba211cca9572f5592957ca9572a55c29",
            "57ca9572a5dc2837cacdd740b9516e941be546b9516e943be54eb953eebe61ca9d72a7dc2977ca9df2a03c28",
            "0fca83f2f0e3511e9407e54179509e9427e54979529e94a7e782f2a43c294fca8bf2a2bc282fca8bf2a2bc3c",
            "729417e5457953de9437e54d7953de9437e5ed69a6bc291fca87f2a17c281fca87f2a17c281fa15c2952295a",
            "2962296a2972297a2982298a299229eef8c7d11d17e41579495e9317e5557959ea32c24cbde6dda1cd8833ea",
            "8c3ca3cf08340a8d44a3d1b47bb1b843a6d169841aa546aad16ac41ab546aee9f7f672876223d96836a28d6a",
            "23dbe836c28d7233ee15e90ef146bd916ff41b0147c19170341c1167de7bd81d3a8e90a3e448395a8e98a3e6",
            "c8397aceba97bd3b241d4d47d45175641d5d47d85176a49d7dff28ee5077e41d7d47e05178241e8d47e45179",
            "cefd6dfdeef80148920ba3",
        ),
    },
];

/// Hash vectors: algorithm, message, digest hex.
pub(crate) const HASH: [(crate::HashAlgorithm, &[u8], &str); 10] = {
    use crate::HashAlgorithm::{Md5, Sha1, Sha256, Sha384, Sha512};
    [
        (Md5, b"", "d41d8cd98f00b204e9800998ecf8427e"),
        (Md5, b"abc", "900150983cd24fb0d6963f7d28e17f72"),
        (Sha1, b"", "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
        (Sha1, b"abc", "a9993e364706816aba3e25717850c26c9cd0d89d"),
        (
            Sha256,
            b"",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            Sha256,
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            Sha384,
            b"",
            concat!(
                "38b060a751ac96384cd9327eb1b1e36a21fdb71114be0743",
                "4c0cc7bf63f6e1da274edebfe76f65fbd51ad2f14898b95b",
            ),
        ),
        (
            Sha384,
            b"abc",
            concat!(
                "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded163",
                "1a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7",
            ),
        ),
        (
            Sha512,
            b"",
            concat!(
                "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce",
                "47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
            ),
        ),
        (
            Sha512,
            b"abc",
            concat!(
                "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a",
                "2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
            ),
        ),
    ]
};

/// SHA-256 of one million `a` bytes (FIPS 180-2 example), to exercise many blocks.
pub(crate) const SHA256_MILLION_A: &str =
    "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0";

/// RC4 vectors: key, plaintext, ciphertext hex.
pub(crate) const RC4: [(&[u8], &[u8], &str); 3] = [
    (b"Key", b"Plaintext", "bbf316e8d940af0ad3"),
    (b"Wiki", b"pedia", "1021bf0420"),
    (b"Secret", b"Attack at dawn", "45a01f645fc35b383552544b9bf5"),
];

/// SP 800-38A F.2 plaintext (four blocks) and IV.
pub(crate) const AES_PLAIN: &str = concat!(
    "6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51",
    "30c81c46a35ce411e5fbc1191a0a52eff69f2445df4f9b17ad2b417be66c3710",
);
pub(crate) const AES_IV: [u8; 16] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
];

/// AES-CBC vectors (SP 800-38A F.2.1/F.2.2 and F.2.5/F.2.6): key hex, ciphertext hex.
pub(crate) const AES_CBC: [(&str, &str); 2] = [
    (
        "2b7e151628aed2a6abf7158809cf4f3c",
        concat!(
            "7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b2",
            "73bed6b8e3c1743b7116e69e222295163ff1caa1681fac09120eca307586e1a7",
        ),
    ),
    (
        "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4",
        concat!(
            "f58c4c04d6e5f1ba779eabfb5f7bfbd69cfc4e967edb808d679f777bc6702c7d",
            "39f23369a9d9bacfa530e26304231461b2eb05e2c39be9fcda6c19078c6a9d1b",
        ),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decodes_and_rejects_bad_input() {
        assert_eq!(hex("00ff7a"), Some(vec![0, 255, 0x7a]));
        assert_eq!(hex(""), Some(vec![]));
        assert_eq!(hex("0"), None);
        assert_eq!(hex("zz"), None);
    }

    #[test]
    fn every_vector_is_valid_hex_of_the_right_length() {
        for v in &FLATE {
            assert!(hex(v.zlib_hex).is_some(), "{}", v.name);
        }
        for (alg, _, digest) in &HASH {
            assert_eq!(hex(digest).map(|d| d.len()), Some(alg.output_len()));
        }
        assert_eq!(hex(SHA256_MILLION_A).map(|d| d.len()), Some(32));
        for (_, plain, cipher) in &RC4 {
            assert_eq!(hex(cipher).map(|c| c.len()), Some(plain.len()));
        }
        assert_eq!(hex(AES_PLAIN).map(|p| p.len()), Some(64));
        for (key, cipher) in &AES_CBC {
            assert!(matches!(hex(key).map(|k| k.len()), Some(16 | 32)));
            assert_eq!(hex(cipher).map(|c| c.len()), Some(64));
        }
    }

    /// The stored-block vector can be checked by hand: header, one final stored block
    /// (LEN/NLEN), the plaintext, Adler-32.
    #[test]
    fn stored_vector_is_what_rfc_1950_says() {
        let v = FLATE.iter().find(|v| v.name == "stored block").unwrap();
        let bytes = hex(v.zlib_hex).unwrap();
        let plain = v.plain.unwrap();
        assert_eq!(&bytes[..2], &[0x78, 0x01]);
        assert_eq!((u16::from(bytes[0]) << 8 | u16::from(bytes[1])) % 31, 0);
        assert_eq!(bytes[2], 0x01, "BFINAL=1, BTYPE=00");
        let len = u16::from_le_bytes([bytes[3], bytes[4]]);
        let nlen = u16::from_le_bytes([bytes[5], bytes[6]]);
        assert_eq!(usize::from(len), plain.len());
        assert_eq!(len, !nlen);
        assert_eq!(&bytes[7..7 + plain.len()], plain);
        assert_eq!(
            &bytes[7 + plain.len()..],
            &super::super::flate::adler32(plain).to_be_bytes()
        );
    }
}
