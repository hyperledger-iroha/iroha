//! Deterministic transcript based on SHA3-256 with explicit domain separation.

use crate::{
    backend::IpaScalar,
    constants::DST,
    hash::{SHA3_256_SIZE, SHA3_512_SIZE, sha3_256, sha3_512},
};

/// A deterministic Fiat–Shamir transcript (SHA3-256 based).
///
/// The transcript maintains a running hash value seeded with a domain
/// separation tag (DST) and explicit labels. Challenges are derived by
/// hashing the current state plus a label.
pub struct Transcript {
    frames: Vec<u8>,
}

impl Transcript {
    /// Creates a new transcript initialized with the crate DST and the
    /// provided `label` for personalization.
    pub fn new(label: &str) -> Self {
        let mut frames = Vec::with_capacity(DST.len() + label.len() + 8);
        frames.extend_from_slice(DST.as_bytes());
        frames.push(0u8);
        frames.extend_from_slice(label.as_bytes());
        Self { frames }
    }

    /// Absorbs arbitrary bytes under a scope label to maintain structure.
    pub fn absorb(&mut self, scope: &str, data: &[u8]) {
        self.frames.push(1u8);
        self.frames.extend_from_slice(scope.as_bytes());
        self.frames.push(0u8);
        self.frames
            .extend_from_slice(&(data.len() as u64).to_le_bytes());
        self.frames.extend_from_slice(data);
    }

    /// Derives a scalar challenge from the transcript with an explicit label.
    pub fn challenge_scalar<S>(&mut self, label: &str) -> S
    where
        S: IpaScalar,
    {
        // domain separation for challenge
        let mut buf = Vec::with_capacity(self.frames.len() + DST.len() + label.len() + 2);
        buf.extend_from_slice(DST.as_bytes());
        buf.push(2u8);
        buf.extend_from_slice(label.as_bytes());
        buf.push(0u8);
        buf.extend_from_slice(&self.frames);
        let out = sha3_512(&buf);
        let mut wide = [0u8; SHA3_512_SIZE];
        wide.copy_from_slice(&out);
        S::from_uniform(&wide)
    }

    /// Returns the current transcript digest without altering state.
    pub fn cur_digest(&self) -> [u8; SHA3_256_SIZE] {
        sha3_256(&self.frames)
    }
}
