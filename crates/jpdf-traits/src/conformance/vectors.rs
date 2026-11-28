//! Built-in test vectors. Flate vectors were produced once with CPython's `zlib.compress`
//! (zlib 1.3; the long fixed-Huffman one with `Z_FIXED`) and checked by decompressing them; hash and cipher values are the published
//! examples of RFC 1321, FIPS 180-4, SP 800-38A (F.2.1, F.2.2, F.2.5, F.2.6) and the
//! well-known RC4 test vectors and RFC 6229, each re-checked with `openssl`/`hashlib` when added.

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
            u8::try_from(hi.checked_mul(16)?.checked_add(lo)?).ok()
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

pub(crate) const FLATE: [FlateVector; 5] = [
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
    FlateVector {
        name: "long, fixed Huffman",
        plain: None,
        zlib_hex: concat!(
            "7801cb2a48495330b056c802d18650da084a1b436913286d0aa5cda0b43994b680d2963073e006c24c348419",
            "690833d31066a821cc544398b18630730d61061bc24c3682996c04772bcc642398c94630938d60261bc14c36",
            "82996c0433d90866b231cc646398c9c6f06080996c0c33d91866b231cc646398c9c630938d61269bc04c3681",
            "996c0233d9041ec230934d60269bc04c3681996c0233d90466b229cc645398c9a630934d61269bc2230f66b2",
            "29cc645398c9a630934d61269bc14c3683996c0633d90c66b219cc643378ba80996c0633d90c66b219cc6473",
            "98c9e63093cd61269bc34c3687996c0e33d91c9ee460269bc34c3687996c0133d90266b205cc640b98c91630",
            "932d60265bc04cb680a76698c91630932d61265bc24cb684996c0933d91266b225cc644b98c99630932de119",
            "059153e059c5009e570ce099c5009e5b0ce0d9c5009e5f0ce019c6009e630ce059c6006e07527684db81c890",
            "881c89c892883c89c894885c89c896f07c6908cf988646883c0fb7039e370de199d3109e3b0de1d9d3109e3f",
            "0de119d4109e430de159d4109e470d8d11050bdc0e78363584e7534378463584e7544378563584e755437866",
            "3584e756437876353441945e703be039d6109e650de179d6109e690de1b9d6109e6d0de1f9d6109e710de139",
            "d7d0145144c2ed80675e4378ee3584675f4378fe3584676043780e3684676143781e36846762433344390cb7",
            "039e8f0de119d9109e930de159d9109e970de199d9109e9b0de1d9d9109e9f0dcd11853ddc0e78963684e769",
            "4378a63684e76a4378b63684e76b4378c63684e76c4378d636b440d428703be0b9db109ebd0de1f9db109ec1",
            "0de139dc109ec50de179dc109ec90de1b9dcd012516d01ed000048920ba3",
        ),
    },
];

/// A hash test message.
#[derive(Debug, Clone, Copy)]
pub(crate) enum HashInput {
    /// These bytes.
    Bytes(&'static [u8]),
    /// `n` bytes where byte `i` is `i % 251`: lengths around the padding boundaries
    /// (55/56 and 63/64 for 64-byte blocks, 111/112 and 127/128 for 128-byte blocks).
    Pattern(usize),
}

impl HashInput {
    pub(crate) fn bytes(self) -> Vec<u8> {
        match self {
            Self::Bytes(b) => b.to_vec(),
            Self::Pattern(n) => (0..n).map(|i| u8::try_from(i % 251).unwrap_or(0)).collect(),
        }
    }
}

/// FIPS 180-4 two-block example message (448 bits) for SHA-1/SHA-256.
pub(crate) const TWO_BLOCK: &[u8] = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
/// FIPS 180-4 example message (896 bits) for SHA-384/SHA-512.
pub(crate) const LONG_896: &[u8] = b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu";

/// Hash vectors: algorithm, message, digest hex. Empty, "abc" and the two FIPS messages
/// are published examples (RFC 1321 for MD5); the `Pattern` digests were computed with
/// CPython `hashlib` (OpenSSL) when added.
pub(crate) const HASH: [(crate::HashAlgorithm, HashInput, &str); 65] = {
    use crate::HashAlgorithm::{Md5, Sha1, Sha256, Sha384, Sha512};
    use HashInput::{Bytes, Pattern};
    [
        (Md5, Bytes(b""), "d41d8cd98f00b204e9800998ecf8427e"),
        (Md5, Bytes(b"abc"), "900150983cd24fb0d6963f7d28e17f72"),
        (Md5, Bytes(TWO_BLOCK), "8215ef0796a20bcaaae116d3876c664a"),
        (Md5, Bytes(LONG_896), "03dd8807a93175fb062dfb55dc7d359c"),
        (Md5, Pattern(55), "6912ee65fff2d9f9ce2508cddf8bcda0"),
        (Md5, Pattern(56), "51fdd1acda72405dfdfa03fcb85896d7"),
        (Md5, Pattern(63), "48a6295221902e8e0938f773a7185e72"),
        (Md5, Pattern(64), "b2d3f56bc197fd985d5965079b5e7148"),
        (Md5, Pattern(111), "4fad3ab7d8546851ec1bb63ea7e6f5a8"),
        (Md5, Pattern(112), "d1fec2ac3715e791ca5f489f300381b3"),
        (Md5, Pattern(127), "8402b21e7bc7906493bae0dac017f1f9"),
        (Md5, Pattern(128), "37eff01866ba3f538421b30b7cbefcac"),
        (Md5, Pattern(1000), "a24f1e3ef66950e1327f210e3997ba2c"),
        (Sha1, Bytes(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
        (
            Sha1,
            Bytes(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d",
        ),
        (
            Sha1,
            Bytes(TWO_BLOCK),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1",
        ),
        (
            Sha1,
            Bytes(LONG_896),
            "a49b2446a02c645bf419f995b67091253a04a259",
        ),
        (
            Sha1,
            Pattern(55),
            "8ae2d46729cfe68ff927af5eec9c7d1b66d65ac2",
        ),
        (
            Sha1,
            Pattern(56),
            "636e2ec698dac903498e648bd2f3af641d3c88cb",
        ),
        (
            Sha1,
            Pattern(63),
            "6d942da0c4392b123528f2905c713a3ce28364bd",
        ),
        (
            Sha1,
            Pattern(64),
            "c6138d514ffa2135bfce0ed0b8fac65669917ec7",
        ),
        (
            Sha1,
            Pattern(111),
            "bc544e24573d592290fdaff8ecf3f7f2b00cd483",
        ),
        (
            Sha1,
            Pattern(112),
            "e4ce142d09a84a8645338dd6535cbfaaf800d320",
        ),
        (
            Sha1,
            Pattern(127),
            "89d7312a903f65cd2b3e34a975e55dbea9033353",
        ),
        (
            Sha1,
            Pattern(128),
            "e6434bc401f98603d7eda504790c98c67385d535",
        ),
        (
            Sha1,
            Pattern(1000),
            "c9c960a0b925474fab83942cc27d504fc24ac37b",
        ),
        (
            Sha256,
            Bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            Sha256,
            Bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            Sha256,
            Bytes(TWO_BLOCK),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        ),
        (
            Sha256,
            Bytes(LONG_896),
            "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
        ),
        (
            Sha256,
            Pattern(55),
            "463eb28e72f82e0a96c0a4cc53690c571281131f672aa229e0d45ae59b598b59",
        ),
        (
            Sha256,
            Pattern(56),
            "da2ae4d6b36748f2a318f23e7ab1dfdf45acdc9d049bd80e59de82a60895f562",
        ),
        (
            Sha256,
            Pattern(63),
            "29af2686fd53374a36b0846694cc342177e428d1647515f078784d69cdb9e488",
        ),
        (
            Sha256,
            Pattern(64),
            "fdeab9acf3710362bd2658cdc9a29e8f9c757fcf9811603a8c447cd1d9151108",
        ),
        (
            Sha256,
            Pattern(111),
            "60780e9451bdc43cf4530ffc95cbb0c4eb24dae2c39f55f334d679e076c08065",
        ),
        (
            Sha256,
            Pattern(112),
            "09373f127d34e61dbbaa8bc4499c87074f2ddb10e1b465f506d7d70a15011979",
        ),
        (
            Sha256,
            Pattern(127),
            "92ca0fa6651ee2f97b884b7246a562fa71250fedefe5ebf270d31c546bfea976",
        ),
        (
            Sha256,
            Pattern(128),
            "471fb943aa23c511f6f72f8d1652d9c880cfa392ad80503120547703e56a2be5",
        ),
        (
            Sha256,
            Pattern(1000),
            "4e4c294b331f7a2099a379bec34b9f9fc03dc46ab465d998f4d683da53487e6d",
        ),
        (
            Sha384,
            Bytes(b""),
            concat!(
                "38b060a751ac96384cd9327eb1b1e36a21fdb71114be07434c0cc7bf63f6e1da",
                "274edebfe76f65fbd51ad2f14898b95b"
            ),
        ),
        (
            Sha384,
            Bytes(b"abc"),
            concat!(
                "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed",
                "8086072ba1e7cc2358baeca134c825a7"
            ),
        ),
        (
            Sha384,
            Bytes(TWO_BLOCK),
            concat!(
                "3391fdddfc8dc7393707a65b1b4709397cf8b1d162af05abfe8f450de5f36bc6",
                "b0455a8520bc4e6f5fe95b1fe3c8452b"
            ),
        ),
        (
            Sha384,
            Bytes(LONG_896),
            concat!(
                "09330c33f71147e83d192fc782cd1b4753111b173b3b05d22fa08086e3b0f712",
                "fcc7c71a557e2db966c3e9fa91746039"
            ),
        ),
        (
            Sha384,
            Pattern(55),
            concat!(
                "dcedb6b590edb4efa849c801e6b6490657a5c1e64f69269f5f63c9267f6223de",
                "24cea7aaa6b267d9bcecc15147b6c875"
            ),
        ),
        (
            Sha384,
            Pattern(56),
            concat!(
                "7b9132d597b8873ad55bbc30f18ed3f2c9f340e7de69fb5774056c71a06d9bc2",
                "b14137e9e1c68b6b645fed28b188249d"
            ),
        ),
        (
            Sha384,
            Pattern(63),
            concat!(
                "dd66b519f51a925814407a449c60b34c553d7652d41783ee903a810a4c9f833b",
                "8181c91c7f12283eacd6a5f8a2639ddf"
            ),
        ),
        (
            Sha384,
            Pattern(64),
            concat!(
                "9f2c9eb7116b3d7a4ba84a74a4d4eff8a5efcf54b6d7b662693c38577914c73a",
                "214766f0a175339bb0895a863824fc0a"
            ),
        ),
        (
            Sha384,
            Pattern(111),
            concat!(
                "f5f9fe110d809d34029de262a01b208356caec6e054c7f926b2591f6c9780579",
                "d4b59f5578c6f531a84f158a33660cef"
            ),
        ),
        (
            Sha384,
            Pattern(112),
            concat!(
                "33ba080ec0ccb378e4e95fed3b26c23aa1a280476e007519ee47f60cd9c5c8a6",
                "5d627259a9aa2fd33ca06d3c14ee5548"
            ),
        ),
        (
            Sha384,
            Pattern(127),
            concat!(
                "d5fcfe2fcf6b3ef375ede37c8123d9b78065fecc1d55197e2f7721e6e9a93d0b",
                "a4d7fd15f9b96dea2744df24141ba2ef"
            ),
        ),
        (
            Sha384,
            Pattern(128),
            concat!(
                "ca2385773319124534111a36d0581fc3f00815e907034b90cff9c3a861e126a7",
                "41d5dfcff65a417b6d7296863ac0ec17"
            ),
        ),
        (
            Sha384,
            Pattern(1000),
            concat!(
                "7a2f8c7f12344964a13cb9260492b845e56615d6152b9eb9e54b580fc88405e6",
                "4f31813bfda10de2a642fdf1676c61b4"
            ),
        ),
        (
            Sha512,
            Bytes(b""),
            concat!(
                "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce",
                "47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
            ),
        ),
        (
            Sha512,
            Bytes(b"abc"),
            concat!(
                "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a",
                "2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
            ),
        ),
        (
            Sha512,
            Bytes(TWO_BLOCK),
            concat!(
                "204a8fc6dda82f0a0ced7beb8e08a41657c16ef468b228a8279be331a703c335",
                "96fd15c13b1b07f9aa1d3bea57789ca031ad85c7a71dd70354ec631238ca3445"
            ),
        ),
        (
            Sha512,
            Bytes(LONG_896),
            concat!(
                "8e959b75dae313da8cf4f72814fc143f8f7779c6eb9f7fa17299aeadb6889018",
                "501d289e4900f7e4331b99dec4b5433ac7d329eeb6dd26545e96e55b874be909"
            ),
        ),
        (
            Sha512,
            Pattern(55),
            concat!(
                "6856647f269c2ee3d8128f0b25427659d880641ef343300dd3cd4679168f58d6",
                "527fda70b4ebc854e2065e172b7d58c1536992c0810599259ba84a2b40c65414"
            ),
        ),
        (
            Sha512,
            Pattern(56),
            concat!(
                "8b12b2f6fe400a51d29656e2b8c42a1bbfe6fcf3e425da430db05d1a2dda1479",
                "0dee20fa8b22d8762afffe4988a5c98a4430d22a17e41e23d90fa61ab75671a9"
            ),
        ),
        (
            Sha512,
            Pattern(63),
            concat!(
                "9dc9c5598e55dc42955695320839788e353f1d7f6ba74df74c80a8a52f463c06",
                "97f57f68835d1418f4ce9b6530cd79bd0f4c6f7e13c93feb1218c0b65c2c0561"
            ),
        ),
        (
            Sha512,
            Pattern(64),
            concat!(
                "ee4320ebaf3fdb4f2c832b137200c08e235e0fa7bbd0eb1740c7063ba8a0d151",
                "da77e003398e1714a955d475b05e3e950b639503b452ec185de4229bc4873949"
            ),
        ),
        (
            Sha512,
            Pattern(111),
            concat!(
                "a1a111449b198d9b1f538bad7f3fc1022b3a5b1a5e90a0bc860de8512746cbc3",
                "1599e6c834de3a3235327af0b51ff57bf7acf1974a73014d9c3953812edc7c8d"
            ),
        ),
        (
            Sha512,
            Pattern(112),
            concat!(
                "c5fbd731d19d2ae1180f001be72c2c1aaba1d7b094b3748880e24593b8e117a7",
                "50e11c1bd867cc2f96dace8c8b74abd2d5c4f236be444e77d30d1916174070b9"
            ),
        ),
        (
            Sha512,
            Pattern(127),
            concat!(
                "eab89674feaa34e27aebeeff3c0a4d70070bb872d5e9f186cf1dbbdee517b6e3",
                "5724d629ff025a5b07185e911ada7e3c8acf830aa0e4f71777bd2d44f504f7f0"
            ),
        ),
        (
            Sha512,
            Pattern(128),
            concat!(
                "1dffd5e3adb71d45d2245939665521ae001a317a03720a45732ba1900ca3b835",
                "1fc5c9b4ca513eba6f80bc7b1d1fdad4abd13491cb824d61b08d8c0e1561b3f7"
            ),
        ),
        (
            Sha512,
            Pattern(1000),
            concat!(
                "5096498d96f50f9a137c4db5b8b0cd38383ad55350fb5a98805fedc31fa1262f",
                "1f0cf4d6f12d7ecd8dedd933a4c9126344fe22e937a8ad35fdeae1e876ae698b"
            ),
        ),
    ]
};

/// SHA-256 of one million `a` bytes (FIPS 180-2 example), to exercise many blocks.
pub(crate) const SHA256_MILLION_A: &str =
    "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0";

/// RC4 vectors: key, plaintext, ciphertext hex.
pub(crate) const RC4: [(&[u8], &[u8], &str); 5] = [
    // RFC 6229 offset 0 for the 40-bit and 128-bit keys PDF uses (plaintext zero, so the
    // ciphertext is the keystream).
    (
        &[1, 2, 3, 4, 5],
        &[0; 16],
        "b2396305f03dc027ccc3524a0a1118a8",
    ),
    (
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        &[0; 16],
        "9ac7cc9a609d1ef7b2932899cde41b97",
    ),
    (b"Key", b"Plaintext", "bbf316e8d940af0ad3"),
    (b"Wiki", b"pedia", "1021bf0420"),
    (b"Secret", b"Attack at dawn", "45a01f645fc35b383552544b9bf5"),
];

/// An RC4 key and (offset, 16 bytes of keystream hex) pairs.
pub(crate) type Rc4Long = (&'static [u8], [(usize, &'static str); 8]);

/// RFC 6229 keystream offsets past the first 16 bytes, for the 40-bit key `01 02 03 04 05`
/// and the 128-bit key `01 02 … 10` (the key lengths PDF uses): key, then (offset, 16 bytes
/// of keystream) pairs. They catch a keystream that restarts or drifts after its first block.
pub(crate) const RC4_LONG: [Rc4Long; 2] = [
    (
        &[1, 2, 3, 4, 5],
        [
            (240, "28cb1132c96ce286421dcaadb8b69eae"),
            (496, "42b7d0cdd918a8a33dd51781c81f4041"),
            (752, "ec10327bde2beefd18f9277680457e22"),
            (1008, "45129048e6a0ed0b56b490338f078da5"),
            (1520, "3294f744d8f9790507e70f62e5bbceea"),
            (2032, "1eb14a0c13b3bf47fa2a0ba93ad45b8b"),
            (3056, "f2e30f9bd102ecbf75aaade9bc35c43c"),
            (4080, "068326a2118416d21f9d04b2cd1ca050"),
        ],
    ),
    (
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        [
            (240, "065902e4b620f6cc36c8589f66432f2b"),
            (496, "b6d1e6c4a5e4771cad79538df295fb11"),
            (752, "c5ecf88de897fd57fed301701b82a259"),
            (1008, "e7a72574f8782ae26aabcf9ebcd66065"),
            (1520, "b40110c4190b5622a96116b0017ed297"),
            (2032, "d03d1bc03cd33d70dff9fa5d71963ebd"),
            (3056, "fabeb76028ade2d0e48722e46c4615a3"),
            (4080, "ff38265c1642c1abe8d3c2fe5e572bf8"),
        ],
    ),
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
        for (_, offsets) in &RC4_LONG {
            for (_, keystream) in offsets {
                assert_eq!(hex(keystream).map(|k| k.len()), Some(16));
            }
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
