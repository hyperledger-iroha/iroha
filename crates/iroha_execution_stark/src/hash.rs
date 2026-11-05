//! Native BLAKE2b-384 hashing with a fixed suite domain and length-framed items.
//!
//! The native 48-byte digest size is selected in BLAKE2b's parameter block (RFC 7693).
//! Every protocol hash includes the suite prefix and a separately framed domain.

use blake2::{Blake2b, Digest as _, digest::consts::U48};

/// Bytes in every cryptographic commitment and transcript state.
pub const DIGEST_BYTES: usize = 48;
/// A native 384-bit BLAKE2b digest.
pub type Digest = [u8; DIGEST_BYTES];

const SUITE_PREFIX: &[u8] = b"iroha_execution_stark:blake2b-384:v1\0";

/// Incremental hash of a domain and an ordered sequence of complete, length-framed items.
///
/// `update(a); update(b)` deliberately differs from `update(a || b)`, including when an item is
/// empty. Large inputs can be supplied as a sequence of canonical scalar or source-file items
/// without allocating their concatenation. Item boundaries are part of the protocol.
#[derive(Clone, Debug)]
pub struct FramedHash(Blake2b<U48>);

impl FramedHash {
    /// Starts a hash in the supplied application or protocol domain.
    pub fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Blake2b::<U48>::new());
        hasher.0.update(SUITE_PREFIX);
        hasher.update(domain);
        hasher
    }

    /// Absorbs one complete item, prefixed with its byte length as a little-endian `u64`.
    pub fn update(&mut self, item: &[u8]) {
        self.0.update((item.len() as u64).to_le_bytes());
        self.0.update(item);
    }

    /// Finishes the native 48-byte hash.
    pub fn finalize(self) -> Digest {
        self.0.finalize().into()
    }
}

/// Hashes an ordered sequence of complete items in a separately framed domain.
pub fn hash(domain: &[u8], items: &[&[u8]]) -> Digest {
    let mut hasher = FramedHash::new(domain);
    for item in items {
        hasher.update(item);
    }
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        bytes.iter().fold(String::new(), |mut output, byte| {
            write!(output, "{byte:02x}").expect("string write");
            output
        })
    }

    #[test]
    fn native_blake2b_384_known_answers() {
        // RFC 7693 sections 2.5/3 and Appendix A. The 384-bit answers were computed by an
        // independent transcription of the RFC algorithm and cross-checked with Python hashlib,
        // not generated from this implementation. The published 512-bit abc answer also checks
        // that reference. Native 384-bit output is deliberately not truncated 512-bit output.
        let published = blake2::Blake2b512::digest(b"abc");
        assert_eq!(
            hex(&published),
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
        );
        let multi: Vec<u8> = (0..=255).chain(0..44).collect();
        for (input, expected) in [
            (
                &b""[..],
                "b32811423377f52d7862286ee1a72ee540524380fda1724a6f25d7978c6fd3244a6caf0498812673c5e05ef583825100",
            ),
            (
                &b"abc"[..],
                "6f56a82c8e7ef526dfe182eb5212f7db9df1317e57815dbda46083fc30f54ee6c66ba83be64b302d7cba6ce15bb556f4",
            ),
            (
                multi.as_slice(),
                "65c8d6bcbe61c9de29892f921721acaec46f6cd319f6a455549565a82c067b5f7275fb759cd7f5676a2f2c51067a852f",
            ),
        ] {
            assert_eq!(hex(&Blake2b::<U48>::digest(input)), expected);
        }
        assert_ne!(
            &Blake2b::<U48>::digest(b"abc")[..],
            &published[..DIGEST_BYTES]
        );
    }

    #[test]
    fn protocol_framing_matches_independent_vector() {
        let payload: Vec<u8> = (0..=255).collect();
        let expected = hash(b"vector domain", &[b"alpha", b"", &payload]);
        assert_eq!(
            hex(&expected),
            "a930787af960751930a0fc4f84d1c20ddd9988d951290e11c39b67fb226cc5858e3625315fad872f76586691f1fc07a5"
        );
        let mut streamed = FramedHash::new(b"vector domain");
        streamed.update(b"alpha");
        streamed.update(b"");
        streamed.update(&payload);
        assert_eq!(streamed.finalize(), expected);
        assert_ne!(
            hash(b"domain", &[b"ab", b"c"]),
            hash(b"domain", &[b"a", b"bc"])
        );
        assert_ne!(hash(b"domain", &[b"a"]), hash(b"domain", &[b"a", b""]));
        assert_ne!(hash(b"domain", &[]), hash(b"domain", &[b""]));
        assert_ne!(hash(b"domain a", &[b"b"]), hash(b"domain", &[b"ab"]));
    }
}
