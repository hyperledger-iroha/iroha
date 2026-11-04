//! BLAKE2b-384 Fiat–Shamir transcript with domain-separated counter expansion.
//!
//! Every absorbed item is framed with its label and length. A squeeze first absorbs its label, then
//! reads challenge bytes from separately framed 384-bit hash blocks of the new state and counter,
//! so challenges depend on everything absorbed before them and never on later prover messages.

use rayon::prelude::*;

use crate::field::{Fp, Fp4, MODULUS};
use crate::hash::{DIGEST_BYTES, Digest, hash};

const STATE_CONTEXT: &[u8] = b"transcript state";
const CHALLENGE_CONTEXT: &[u8] = b"transcript challenge block";
const GRIND_CONTEXT: &[u8] = b"proof of work";

struct ChallengeReader {
    state: Digest,
    counter: u64,
    block: Digest,
    offset: usize,
}

impl ChallengeReader {
    fn new(state: Digest) -> Self {
        Self {
            state,
            counter: 0,
            block: [0; DIGEST_BYTES],
            offset: DIGEST_BYTES,
        }
    }

    fn fill(&mut self, mut output: &mut [u8]) {
        while !output.is_empty() {
            if self.offset == DIGEST_BYTES {
                self.block = hash(
                    CHALLENGE_CONTEXT,
                    &[&self.state, &self.counter.to_le_bytes()],
                );
                self.counter = self
                    .counter
                    .checked_add(1)
                    .expect("challenge stream exhausted");
                self.offset = 0;
            }
            let count = output.len().min(DIGEST_BYTES - self.offset);
            output[..count].copy_from_slice(&self.block[self.offset..self.offset + count]);
            self.offset += count;
            output = &mut output[count..];
        }
    }
}

/// The Fiat–Shamir state shared by prover and verifier.
///
/// Deliberately not `Copy`: an implicit copy would silently fork the transcript.
#[allow(missing_copy_implementations)]
#[derive(Clone, Debug)]
pub struct Transcript {
    state: Digest,
}

impl Transcript {
    /// Starts a transcript for one protocol label.
    pub fn new(protocol: &[u8]) -> Self {
        let mut transcript = Self {
            state: [0; DIGEST_BYTES],
        };
        transcript.absorb(b"protocol", protocol);
        transcript
    }

    /// Absorbs labelled bytes.
    pub fn absorb(&mut self, label: &[u8], data: &[u8]) {
        self.state = hash(STATE_CONTEXT, &[&self.state, label, data]);
    }

    /// Absorbs a digest.
    pub fn absorb_digest(&mut self, label: &[u8], digest: &Digest) {
        self.absorb(label, digest);
    }

    /// Absorbs base-field elements.
    pub fn absorb_fps(&mut self, label: &[u8], values: &[Fp]) {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.absorb(label, &bytes);
    }

    /// Absorbs extension-field elements.
    pub fn absorb_fp4s(&mut self, label: &[u8], values: &[Fp4]) {
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.absorb(label, &bytes);
    }

    fn reader(&mut self, label: &[u8]) -> ChallengeReader {
        self.absorb(b"squeeze", label);
        ChallengeReader::new(self.state)
    }

    /// Squeezes uniformly distributed extension elements (rejection-sampled coefficients).
    pub fn challenge_fp4s(&mut self, label: &[u8], count: usize) -> Vec<Fp4> {
        let mut reader = self.reader(label);
        let mut next = || loop {
            let mut bytes = [0u8; 8];
            reader.fill(&mut bytes);
            let value = u64::from_le_bytes(bytes);
            if value < MODULUS {
                return Fp::new(value);
            }
        };
        (0..count)
            .map(|_| Fp4([next(), next(), next(), next()]))
            .collect()
    }

    /// Squeezes one extension element.
    pub fn challenge_fp4(&mut self, label: &[u8]) -> Fp4 {
        self.challenge_fp4s(label, 1)[0]
    }

    /// Squeezes `count` distinct uniformly random indices below `2^log_size`.
    ///
    /// # Panics
    /// Panics if `count` exceeds the domain size.
    pub fn challenge_unique_indices(
        &mut self,
        label: &[u8],
        count: usize,
        log_size: u32,
    ) -> Vec<usize> {
        let size = 1usize << log_size;
        assert!(count <= size, "too many queries for the domain");
        let mut reader = self.reader(label);
        let mut chosen = Vec::with_capacity(count);
        let mut seen = vec![false; size];
        while chosen.len() < count {
            let mut bytes = [0u8; 8];
            reader.fill(&mut bytes);
            let index = (u64::from_le_bytes(bytes) & ((size as u64) - 1)) as usize;
            if !seen[index] {
                seen[index] = true;
                chosen.push(index);
            }
        }
        chosen
    }

    fn pow_digest(&self, nonce: u64) -> Digest {
        hash(GRIND_CONTEXT, &[&self.state, &nonce.to_le_bytes()])
    }

    fn pow_ok(digest: &Digest, bits: u32) -> bool {
        let leading = u64::from_le_bytes(digest[..8].try_into().expect("8 bytes")).trailing_zeros();
        leading >= bits
    }

    /// Finds the smallest proof-of-work nonce with `bits` zero bits and absorbs it.
    pub fn grind(&mut self, bits: u32) -> u64 {
        if bits == 0 {
            self.absorb(b"pow-nonce", &0u64.to_le_bytes());
            return 0;
        }
        let chunk: u64 = 1 << 14;
        let mut start = 0u64;
        let nonce = loop {
            let found = (start..start + chunk)
                .into_par_iter()
                .find_first(|nonce| Self::pow_ok(&self.pow_digest(*nonce), bits));
            if let Some(nonce) = found {
                break nonce;
            }
            start += chunk;
        };
        self.absorb(b"pow-nonce", &nonce.to_le_bytes());
        nonce
    }

    /// Checks and absorbs a proof-of-work nonce.
    pub fn check_grind(&mut self, bits: u32, nonce: u64) -> bool {
        let ok = bits == 0 && nonce == 0 || bits > 0 && Self::pow_ok(&self.pow_digest(nonce), bits);
        self.absorb(b"pow-nonce", &nonce.to_le_bytes());
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_and_counter_stream_match_independent_vectors() {
        let hex = |bytes: &[u8]| -> String {
            use std::fmt::Write as _;
            bytes.iter().fold(String::new(), |mut output, byte| {
                write!(output, "{byte:02x}").expect("string write");
                output
            })
        };
        let mut transcript = Transcript::new(b"vector protocol");
        transcript.absorb(b"payload", &(0..=255).collect::<Vec<u8>>());
        assert_eq!(
            hex(&transcript.state),
            "239253661543a4d360152147e5567d7e3347f0b22de120a592630428bfdfe466c86bde84b8fdaa8f9c0c9b04db38e8e1"
        );
        assert_eq!(
            hex(&transcript.pow_digest(7)),
            "3589319f9c620637a02ba4f2d2b4911c694656973e7542ec1af29eafd2b11ad471b90c96f96a53ee45a48da9335f07e0"
        );
        let mut same = transcript.clone();
        let mut reader = transcript.reader(b"vector challenge");
        let mut output = [0u8; 3 * DIGEST_BYTES];
        reader.fill(&mut output);
        assert_eq!(
            hex(&output),
            "b815da31d0399121f4b99d78855b402f7528e8d3d13e78ede3f4a4ee043fe3c6def5b8575d028380a28bdf781585584d408911c80835b7c8911995e5220afc1c6d7e0fd40f0f4d142bdd4109746299d2d047615734a75c51295ec7df6ceaa890eb1e15d52d8f4d0d08493d72b7db06fb3964ecebc5a68ef9757739235e19a896addc66763377b629711610069d3d7b40"
        );
        let mut chunked_reader = same.reader(b"vector challenge");
        let mut chunked = [0u8; 3 * DIGEST_BYTES];
        chunked_reader.fill(&mut chunked[..17]);
        chunked_reader.fill(&mut chunked[17..63]);
        chunked_reader.fill(&mut chunked[63..]);
        assert_eq!(chunked, output);
        // The next squeeze absorbs a new frame even when its label is repeated.
        let mut next = [0u8; DIGEST_BYTES];
        transcript.reader(b"vector challenge").fill(&mut next);
        assert_ne!(next.as_slice(), &output[..DIGEST_BYTES]);
    }

    #[test]
    #[allow(clippy::many_single_char_names)]
    fn transcripts_are_deterministic_and_order_sensitive() {
        let mut a = Transcript::new(b"test");
        let mut b = Transcript::new(b"test");
        a.absorb(b"x", b"1");
        b.absorb(b"x", b"1");
        assert_eq!(a.challenge_fp4(b"c"), b.challenge_fp4(b"c"));
        let mut c = Transcript::new(b"test");
        c.absorb(b"x", b"2");
        let mut d = Transcript::new(b"test");
        d.absorb(b"x", b"1");
        assert_ne!(c.challenge_fp4(b"c"), d.challenge_fp4(b"c"));
        // Label framing: ("ab","c") differs from ("a","bc").
        let mut e = Transcript::new(b"test");
        e.absorb(b"ab", b"c");
        let mut f = Transcript::new(b"test");
        f.absorb(b"a", b"bc");
        assert_ne!(e.challenge_fp4(b"c"), f.challenge_fp4(b"c"));
    }

    #[test]
    fn unique_indices_and_grinding() {
        let mut t = Transcript::new(b"test");
        let indices = t.challenge_unique_indices(b"q", 50, 8);
        let mut sorted = indices.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 50);
        assert!(indices.iter().all(|i| *i < 256));
        let mut prover = Transcript::new(b"pow");
        let mut verifier = prover.clone();
        let nonce = prover.grind(8);
        assert!(verifier.check_grind(8, nonce));
        assert_eq!(prover.challenge_fp4(b"z"), verifier.challenge_fp4(b"z"));
        let mut cheater = Transcript::new(b"pow");
        let wrong = (0..u64::MAX)
            .find(|n| !Transcript::pow_ok(&cheater.pow_digest(*n), 8))
            .unwrap();
        assert!(!cheater.check_grind(8, wrong));
    }
}
