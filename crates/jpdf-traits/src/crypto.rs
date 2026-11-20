//! Hash functions and symmetric ciphers used by the standard security handler
//! (ISO 32000-2 §7.6.3, §7.6.4) and by signatures (§12.8).

use crate::{BackendError, DecodeLimits};

/// Hash algorithms PDF needs: MD5 and RC4-era key derivation (§7.6.4.3, revisions 2–4),
/// SHA-256/384/512 for revision 6 (Algorithm 2.B) and signatures, SHA-1 to verify old
/// signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    /// MD5 (RFC 1321), 16 bytes.
    Md5,
    /// SHA-1 (FIPS 180-4), 20 bytes.
    Sha1,
    /// SHA-256 (FIPS 180-4), 32 bytes.
    Sha256,
    /// SHA-384 (FIPS 180-4), 48 bytes.
    Sha384,
    /// SHA-512 (FIPS 180-4), 64 bytes.
    Sha512,
}

impl HashAlgorithm {
    /// Every algorithm, for tests and conformance suites.
    pub const ALL: [Self; 5] = [
        Self::Md5,
        Self::Sha1,
        Self::Sha256,
        Self::Sha384,
        Self::Sha512,
    ];

    /// Digest length in bytes.
    pub fn output_len(self) -> usize {
        match self {
            Self::Md5 => 16,
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }
}

/// An incremental hash computation. The digest is fixed-size, so no output limit applies.
pub trait HashState {
    /// Feeds more data.
    fn update(&mut self, data: &[u8]);
    /// Returns the digest; exactly `output_len()` bytes of the algorithm it was started with.
    fn finish(self: Box<Self>) -> Vec<u8>;
}

/// Hash functions.
pub trait Hasher {
    /// Starts a hash computation. Every [`HashAlgorithm`] must be supported.
    fn start(&self, algorithm: HashAlgorithm) -> Result<Box<dyn HashState>, BackendError>;

    /// One-shot digest of `data`.
    fn digest(&self, algorithm: HashAlgorithm, data: &[u8]) -> Result<Vec<u8>, BackendError> {
        let mut state = self.start(algorithm)?;
        state.update(data);
        Ok(state.finish())
    }
}

/// Cipher direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Encrypt.
    Encrypt,
    /// Decrypt.
    Decrypt,
}

/// Block padding for AES-CBC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Padding {
    /// PKCS #7 (the "PKCS#5" padding of ISO 32000-2 §7.6.3): encryption always adds 1–16
    /// bytes; decryption checks and removes them.
    Pkcs7,
    /// No padding: the data length must be a multiple of 16. Used by Algorithm 2.B and the
    /// revision 6 key unwrapping, and by readers that strip broken padding themselves.
    None,
}

/// Symmetric ciphers. Keys are secrets: backends must not log them or keep copies after a
/// call returns.
pub trait Cipher {
    /// RC4 (ISO 32000-2 §7.6.3, revisions 2–4). `key` must be 1–256 bytes (PDF uses 5–16),
    /// otherwise [`BackendError::InvalidArgument`]. Output is as long as `data`; longer than
    /// `limits.max_output_bytes` fails with [`BackendError::LimitExceeded`].
    fn rc4(&self, key: &[u8], data: &[u8], limits: &DecodeLimits) -> Result<Vec<u8>, BackendError>;

    /// AES in CBC mode (FIPS 197, SP 800-38A) with a 16- or 32-byte key (AESV2, AESV3);
    /// other key lengths fail with [`BackendError::InvalidArgument`].
    ///
    /// - Decrypting data whose length is not a multiple of 16, or whose PKCS #7 padding is
    ///   invalid, fails with [`BackendError::Malformed`].
    /// - Encrypting with [`Padding::None`] data whose length is not a multiple of 16 fails
    ///   with [`BackendError::InvalidArgument`].
    /// - Output longer than `limits.max_output_bytes` fails with
    ///   [`BackendError::LimitExceeded`].
    fn aes_cbc(
        &self,
        direction: Direction,
        key: &[u8],
        iv: &[u8; 16],
        data: &[u8],
        padding: Padding,
        limits: &DecodeLimits,
    ) -> Result<Vec<u8>, BackendError>;
}
