//! SHA-1 (FIPS 180-4 §6.1) for git blob ids (specs/tooling/corpus.md, item 2).
//!
//! Only used to check downloads against ids recorded from pinned git commits; it is not a
//! security primitive of the engine (that goes through `jpdf-traits`).

/// Streaming SHA-1.
#[derive(Debug, Clone)]
pub struct Sha1 {
    state: [u32; 5],
    block: [u8; 64],
    /// Bytes in `block` (always < 64 between calls).
    fill: usize,
    /// Total message length in bytes.
    len: u64,
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    pub fn new() -> Self {
        Self {
            state: [
                0x6745_2301,
                0xEFCD_AB89,
                0x98BA_DCFE,
                0x1032_5476,
                0xC3D2_E1F0,
            ],
            block: [0; 64],
            fill: 0,
            len: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.len = self
            .len
            .wrapping_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
        while !data.is_empty() {
            // `fill` stays in 0..64, so none of these saturates.
            let take = 64usize.saturating_sub(self.fill).min(data.len());
            let (head, rest) = data.split_at(take);
            let end = self.fill.saturating_add(take);
            if let Some(dst) = self.block.get_mut(self.fill..end) {
                dst.copy_from_slice(head);
            }
            self.fill = end;
            data = rest;
            if self.fill == 64 {
                let block = self.block;
                self.compress(&block);
                self.fill = 0;
            }
        }
    }

    pub fn finish(mut self) -> [u8; 20] {
        let bits = self.len.wrapping_mul(8);
        self.update(&[0x80]);
        while self.fill != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        let mut out = [0u8; 20];
        for (chunk, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 80];
        for (word, bytes) in w.iter_mut().zip(block.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        // W[t] = ROTL1(W[t-3] ^ W[t-8] ^ W[t-14] ^ W[t-16]) for 16 ≤ t < 80.
        let at = |w: &[u32; 80], i: usize| w.get(i).copied().unwrap_or(0);
        for t in 16..80usize {
            let back = |k: usize| at(&w, t.saturating_sub(k));
            let v = (back(3) ^ back(8) ^ back(14) ^ back(16)).rotate_left(1);
            if let Some(slot) = w.get_mut(t) {
                *slot = v;
            }
        }
        let [mut a, mut b, mut c, mut d, mut e] = self.state;
        for (t, wt) in w.iter().enumerate() {
            let (f, k) = match t {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wt);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        for (s, v) in self.state.iter_mut().zip([a, b, c, d, e]) {
            *s = s.wrapping_add(v);
        }
    }
}

/// Lowercase hex of a digest.
pub fn hex(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// The git blob id of `data`: SHA-1 of `"blob <len>\0" + data`, in lowercase hex.
pub fn git_blob_id(data: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(format!("blob {}\0", data.len()).as_bytes());
    h.update(data);
    hex(&h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha1_hex(data: &[u8]) -> String {
        let mut h = Sha1::new();
        h.update(data);
        hex(&h.finish())
    }

    #[test]
    fn fips_180_vectors() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha1_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            sha1_hex(&vec![b'a'; 1_000_000]),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }

    #[test]
    fn padding_and_block_boundaries() {
        // `printf 'a%.0s' $(seq N) | shasum` for each length.
        for (n, want) in [
            (55, "c1c8bbdc22796e28c0e15163d20899b65621d65a"),
            (56, "c2db330f6083854c99d4b5bfb6e8f29f201be699"),
            (63, "03f09f5b158a7a8cdad920bddc29b81c18a551f5"),
            (64, "0098ba824b5c16427bd7a1122a5a442a25ec644d"),
            (65, "11655326c708d70319be2610e8a57d9a5b959d3b"),
        ] {
            assert_eq!(sha1_hex(&vec![b'a'; n]), want, "length {n}");
        }
    }

    #[test]
    fn split_updates_match_one_update() {
        let data: Vec<u8> = (0..300u32).map(|i| (i * 7).to_le_bytes()[0]).collect();
        for cut in [0, 1, 63, 64, 65, 128, 299] {
            let mut h = Sha1::new();
            h.update(&data[..cut]);
            h.update(&data[cut..]);
            assert_eq!(hex(&h.finish()), sha1_hex(&data), "cut {cut}");
        }
    }

    #[test]
    fn git_blob_ids() {
        // `git hash-object --stdin`.
        assert_eq!(git_blob_id(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
        assert_eq!(
            git_blob_id(b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
    }
}
