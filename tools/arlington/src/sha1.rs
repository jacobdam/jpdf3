//! SHA-1 (FIPS 180-4 §6.1) and git object ids, for checking the vendored Arlington data
//! against its pinned git ids offline (specs/tooling/arlington.md, "Vendored data"; ADR 0007).
//!
//! This is an integrity check against hand edits and incomplete copies, not a defence against
//! crafted collisions: the data is test input that reaches the repository only through a
//! reviewed PR, and git's own ids are SHA-1. Written from the FIPS text; std only.

/// The SHA-1 digest of `data`.
pub fn digest(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    // §5.1.1 padding: 0x80, zeros to 56 mod 64, then the bit length as a 64-bit big-endian.
    let bit_len = u64::try_from(data.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    let (blocks, rest) = data.as_chunks::<64>();
    let mut tail = Vec::with_capacity(128);
    tail.extend_from_slice(rest);
    tail.push(0x80);
    while tail.len() % 64 != 56 {
        tail.push(0);
    }
    tail.extend_from_slice(&bit_len.to_be_bytes());
    // `tail` is 64 or 128 bytes, so its remainder is empty.
    for block in blocks.iter().chain(tail.as_chunks::<64>().0) {
        compress(&mut h, block);
    }
    let mut out = [0u8; 20];
    for (dst, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        *dst = word.to_be_bytes();
    }
    out
}

/// §6.1.2: one 512-bit block.
fn compress(h: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for (wt, bytes) in w.iter_mut().zip(block.as_chunks::<4>().0) {
        *wt = u32::from_be_bytes(*bytes);
    }
    for t in 16..80usize {
        // `t` is 16..80, so every index is in 0..80 and `get` never misses.
        let at = |i: usize| w.get(i).copied().unwrap_or_default();
        let back = |k: usize| at(t.saturating_sub(k));
        let x = back(3) ^ back(8) ^ back(14) ^ back(16);
        if let Some(wt) = w.get_mut(t) {
            *wt = x.rotate_left(1);
        }
    }
    let [mut a, mut b, mut c, mut d, mut e] = *h;
    for (t, wt) in w.iter().enumerate() {
        let (f, k) = match t {
            0..20 => ((b & c) | (!b & d), 0x5A82_7999),
            20..40 => (b ^ c ^ d, 0x6ED9_EBA1),
            40..60 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
            _ => (b ^ c ^ d, 0xCA62_C1D6),
        };
        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(*wt);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }
    for (hi, v) in h.iter_mut().zip([a, b, c, d, e]) {
        *hi = hi.wrapping_add(v);
    }
}

/// Lowercase hex of a digest.
pub fn hex(digest: &[u8; 20]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// The git object id of a loose object: SHA-1 of `"<kind> <len>\0" + body`.
pub fn object_id(kind: &str, body: &[u8]) -> [u8; 20] {
    let mut buf = format!("{kind} {}\0", body.len()).into_bytes();
    buf.extend_from_slice(body);
    digest(&buf)
}

/// The git id of a tree holding only regular, non-executable files (mode `100644`).
/// `files` must be sorted by name in byte order, which is git's order for blob entries.
pub fn tree_id(files: &[(String, Vec<u8>)]) -> [u8; 20] {
    let mut body = Vec::new();
    for (name, bytes) in files {
        body.extend_from_slice(b"100644 ");
        body.extend_from_slice(name.as_bytes());
        body.push(0);
        body.extend_from_slice(&object_id("blob", bytes));
    }
    object_id("tree", &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(data: &[u8]) -> String {
        hex(&digest(data))
    }

    /// FIPS 180-4 / NIST CSRC example vectors.
    #[test]
    fn known_vectors() {
        assert_eq!(h(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(h(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            h(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            h(&vec![b'a'; 1_000_000]),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }

    /// Well-known git ids: the empty blob and the empty tree.
    #[test]
    fn git_ids() {
        assert_eq!(
            hex(&object_id("blob", b"")),
            "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
        );
        assert_eq!(
            hex(&tree_id(&[])),
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
        );
    }
}
