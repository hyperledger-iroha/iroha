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
    state: [u8; SHA3_256_SIZE],
}

impl Transcript {
    /// Creates a new transcript initialized with the crate DST and the
    /// provided `label` for personalization.
    pub fn new(label: &str) -> Self {
        let mut buf = Vec::with_capacity(DST.len() + label.len() + 16);
        buf.extend_from_slice(DST.as_bytes());
        buf.push(0u8);
        buf.extend_from_slice(&(label.len() as u64).to_le_bytes());
        buf.extend_from_slice(label.as_bytes());
        Self {
            state: sha3_256(&buf),
        }
    }

    /// Absorbs arbitrary bytes under a scope label to maintain structure.
    pub fn absorb(&mut self, scope: &str, data: &[u8]) {
        let mut buf =
            Vec::with_capacity(DST.len() + self.state.len() + scope.len() + data.len() + 24);
        buf.extend_from_slice(DST.as_bytes());
        buf.push(1u8);
        buf.extend_from_slice(&self.state);
        buf.extend_from_slice(&(scope.len() as u64).to_le_bytes());
        buf.extend_from_slice(scope.as_bytes());
        buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
        buf.extend_from_slice(data);
        self.state = sha3_256(&buf);
    }

    /// Derives a scalar challenge from the transcript with an explicit label.
    pub fn challenge_scalar<S>(&mut self, label: &str) -> S
    where
        S: IpaScalar,
    {
        let mut buf = Vec::with_capacity(DST.len() + self.state.len() + label.len() + 16);
        buf.extend_from_slice(DST.as_bytes());
        buf.push(2u8);
        buf.extend_from_slice(&self.state);
        buf.extend_from_slice(&(label.len() as u64).to_le_bytes());
        buf.extend_from_slice(label.as_bytes());
        let out = sha3_512(&buf);

        let mut next =
            Vec::with_capacity(DST.len() + self.state.len() + label.len() + out.len() + 16);
        next.extend_from_slice(DST.as_bytes());
        next.push(3u8);
        next.extend_from_slice(&self.state);
        next.extend_from_slice(&(label.len() as u64).to_le_bytes());
        next.extend_from_slice(label.as_bytes());
        next.extend_from_slice(&out);
        self.state = sha3_256(&next);

        let mut wide = [0u8; SHA3_512_SIZE];
        wide.copy_from_slice(&out);
        S::from_uniform(&wide)
    }

    /// Returns the current transcript digest without altering state.
    pub fn cur_digest(&self) -> [u8; SHA3_256_SIZE] {
        self.state
    }
}
