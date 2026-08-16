//! Allocation-closed T256 transcript and exact public-proof owner.
//!
//! The raw transcript and challenge-domain-prefixed transcript are retained as
//! two zeroizing Keccak owners. Forking those owners reproduces
//! `Keccak256(challenge_domain || transcript || ordinal || attempt || suffix)`
//! without materializing or cloning transcript bytes. Only the eventual public
//! proof body owns heap storage, and that storage is exact-capacity or rejected.

use super::*;
use crate::generalized_bulletproof::{ProverTranscript, VerifierTranscript};
use core::marker::PhantomData;

const T256_BP_TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.generalized-bulletproof.t256.transcript.v1";
const T256_BP_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.generalized-bulletproof.t256.challenge.v1";
const T256_BP_SCALAR_TAG_V1: u8 = 0;
const T256_BP_POINT_TAG_V1: u8 = 1;
const T256_BP_CHALLENGE_TAG_V1: u8 = 2;
const T256_BP_MAX_CHALLENGE_ATTEMPTS_V1: usize = 128;
#[cfg(test)]
std::thread_local! {
    static T256_PARTIAL_PROOF_BUFFER_DROPS_V1: core::cell::Cell<usize> = const {
        core::cell::Cell::new(0)
    };
}

#[cfg(test)]
pub(super) fn reset_partial_proof_buffer_drops_v1() {
    T256_PARTIAL_PROOF_BUFFER_DROPS_V1.with(|drops| drops.set(0));
}

#[cfg(test)]
pub(super) fn partial_proof_buffer_drops_v1() -> usize {
    T256_PARTIAL_PROOF_BUFFER_DROPS_V1.with(core::cell::Cell::get)
}

/// Zeroizing owner for a partial secret-derived proof body.
pub(super) struct ExactT256ProofBufferV1 {
    bytes: Vec<u8>,
    expected_len: usize,
}

impl ExactT256ProofBufferV1 {
    pub(super) fn new(expected_len: usize) -> Result<Self, GeneralizedBulletproofErrorV1> {
        if expected_len == 0 {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        Ok(Self {
            bytes: try_exact_capacity_vec_v1(expected_len)?,
            expected_len,
        })
    }

    fn append(&mut self, encoded: &[u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        let end = self
            .bytes
            .len()
            .checked_add(encoded.len())
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        if end > self.expected_len || self.bytes.capacity() != self.expected_len {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        self.bytes.extend_from_slice(encoded);
        if self.bytes.len() != end || self.bytes.capacity() != self.expected_len {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        Ok(())
    }

    fn try_take_complete(&mut self) -> Result<Vec<u8>, GeneralizedBulletproofErrorV1> {
        if self.bytes.len() != self.expected_len || self.bytes.capacity() != self.expected_len {
            return Err(GeneralizedBulletproofErrorV1::TranscriptConsumption);
        }
        Ok(core::mem::take(&mut self.bytes))
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.bytes.len()
    }

    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.bytes.capacity()
    }
}

impl Drop for ExactT256ProofBufferV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(self.bytes.as_mut_slice());
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *bytes);
        #[cfg(test)]
        T256_PARTIAL_PROOF_BUFFER_DROPS_V1.with(|drops| drops.set(drops.get().saturating_add(1)));
    }
}

struct T256TranscriptStateV1 {
    raw: Keccak256,
    challenge_prefixed: Keccak256,
}

impl T256TranscriptStateV1 {
    fn new(
        context_digest: [u8; 32],
        generator_basis_digest: [u8; 32],
        chunk_ordinal: u16,
        coefficient_bound: u8,
        commitment: &Point,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        if context_digest == [0; 32]
            || generator_basis_digest == [0; 32]
            || !matches!(coefficient_bound, 1 | 2)
            || commitment == &Point::identity()
        {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let mut state = Self {
            raw: Keccak256::new(),
            challenge_prefixed: Keccak256::new(),
        };
        state.challenge_prefixed.update(T256_BP_CHALLENGE_DOMAIN_V1);
        state.append(T256_BP_TRANSCRIPT_DOMAIN_V1);
        state.append(&context_digest);
        state.append(&generator_basis_digest);
        state.append(&chunk_ordinal.to_be_bytes());
        state.append(&[coefficient_bound]);
        let encoded = SecretT256PointEncodingV1::new(commitment)?;
        state.append(encoded.as_ref());
        drop(encoded);
        Ok(state)
    }

    fn append(&mut self, bytes: &[u8]) {
        self.raw.update(bytes);
        self.challenge_prefixed.update(bytes);
    }

    fn append_scalar(&mut self, scalar: &Scalar) {
        let encoded = SecretT256ScalarEncodingV1::new(scalar);
        self.append(&[T256_BP_SCALAR_TAG_V1]);
        self.append(encoded.as_ref());
        drop(encoded);
    }

    fn append_point(&mut self, point: &Point) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = SecretT256PointEncodingV1::new(point)?;
        self.append(&[T256_BP_POINT_TAG_V1]);
        self.append(encoded.as_ref());
        drop(encoded);
        Ok(())
    }

    fn challenge(&mut self, ordinal: &mut u32) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let ordinal_bytes = ordinal.to_be_bytes();
        for attempt in 0..T256_BP_MAX_CHALLENGE_ATTEMPTS_V1 {
            let attempt = u8::try_from(attempt)
                .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
            let mut high = self.challenge_prefixed.fork_v1();
            high.update(&ordinal_bytes);
            high.update(&[attempt]);
            let mut low = high.fork_v1();
            low.update(&[0]);
            high.update(&[1]);
            let mut wide = [0_u8; 64];
            wide[..32].copy_from_slice(&low.finalize());
            wide[32..].copy_from_slice(&high.finalize());
            let challenge = Scalar::from_uniform_le_bytes(wide);
            wide.fill(0);
            if !challenge.is_zero() {
                let next_ordinal = ordinal
                    .checked_add(1)
                    .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
                self.append(&[T256_BP_CHALLENGE_TAG_V1]);
                self.append(&ordinal_bytes);
                self.append(&[attempt]);
                self.append(&challenge.to_le_bytes());
                *ordinal = next_ordinal;
                return Ok(challenge);
            }
        }
        Err(GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted)
    }

    fn digest(self) -> [u8; 32] {
        self.raw.finalize()
    }
}

/// Typed prover transcript for one T256 membership-proof chunk.
pub(super) struct T256BulletproofProverTranscriptV1<S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: T256TranscriptStateV1,
    proof: ExactT256ProofBufferV1,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<S> T256BulletproofProverTranscriptV1<S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    pub(super) fn new(
        context_digest: [u8; 32],
        generator_basis_digest: [u8; 32],
        chunk_ordinal: u16,
        coefficient_bound: u8,
        commitment: &Point,
        expected_proof_len: usize,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        Self::new_with_exact_proof_buffer(
            context_digest,
            generator_basis_digest,
            chunk_ordinal,
            coefficient_bound,
            commitment,
            ExactT256ProofBufferV1::new(expected_proof_len)?,
        )
    }

    pub(super) fn new_with_exact_proof_buffer(
        context_digest: [u8; 32],
        generator_basis_digest: [u8; 32],
        chunk_ordinal: u16,
        coefficient_bound: u8,
        commitment: &Point,
        proof: ExactT256ProofBufferV1,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        Ok(Self {
            state: T256TranscriptStateV1::new(
                context_digest,
                generator_basis_digest,
                chunk_ordinal,
                coefficient_bound,
                commitment,
            )?,
            proof,
            challenge_ordinal: 0,
            suite: PhantomData,
        })
    }

    pub(super) fn complete(self) -> Result<(Vec<u8>, [u8; 32]), GeneralizedBulletproofErrorV1> {
        let Self {
            state, mut proof, ..
        } = self;
        let digest = state.digest();
        let proof = proof.try_take_complete()?;
        Ok((proof, digest))
    }

    #[cfg(test)]
    pub(super) fn partial_proof_len(&self) -> usize {
        self.proof.len()
    }

    #[cfg(test)]
    pub(super) fn proof_capacity(&self) -> usize {
        self.proof.capacity()
    }
}

impl<S> ProverTranscript<S> for T256BulletproofProverTranscriptV1<S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    fn push_scalar(&mut self, scalar: &Scalar) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = SecretT256ScalarEncodingV1::new(scalar);
        self.proof.append(encoded.as_ref())?;
        self.state.append(&[T256_BP_SCALAR_TAG_V1]);
        self.state.append(encoded.as_ref());
        drop(encoded);
        Ok(())
    }

    fn push_point(&mut self, point: &Point) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = SecretT256PointEncodingV1::new(point)?;
        self.proof.append(encoded.as_ref())?;
        self.state.append(&[T256_BP_POINT_TAG_V1]);
        self.state.append(encoded.as_ref());
        drop(encoded);
        Ok(())
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        self.state.challenge(&mut self.challenge_ordinal)
    }
}

/// Exact, allocation-bounded verifier transcript over attacker proof bytes.
pub(super) struct T256BulletproofVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: T256TranscriptStateV1,
    proof: &'a [u8],
    cursor: usize,
    challenge_ordinal: u32,
    suite: PhantomData<S>,
}

impl<'a, S> T256BulletproofVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    pub(super) fn new(
        context_digest: [u8; 32],
        generator_basis_digest: [u8; 32],
        chunk_ordinal: u16,
        coefficient_bound: u8,
        commitment: Point,
        proof: &'a [u8],
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        Ok(Self {
            state: T256TranscriptStateV1::new(
                context_digest,
                generator_basis_digest,
                chunk_ordinal,
                coefficient_bound,
                &commitment,
            )?,
            proof,
            cursor: 0,
            challenge_ordinal: 0,
            suite: PhantomData,
        })
    }

    fn take(&mut self, bytes: usize) -> Result<&'a [u8], GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(bytes)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let value =
            self.proof
                .get(self.cursor..end)
                .ok_or(GeneralizedBulletproofErrorV1::ProofLength {
                    actual: self.proof.len(),
                    expected: end,
                })?;
        self.cursor = end;
        Ok(value)
    }

    pub(super) fn finish(self) -> Result<[u8; 32], GeneralizedBulletproofErrorV1> {
        if self.cursor != self.proof.len() {
            return Err(GeneralizedBulletproofErrorV1::TranscriptConsumption);
        }
        Ok(self.state.digest())
    }
}

impl<S> VerifierTranscript<S> for T256BulletproofVerifierTranscriptV1<'_, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    fn read_scalar(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        let bytes: [u8; 32] = self
            .take(32)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        let scalar = Scalar::from_le_bytes_exact(bytes)
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        self.state.append_scalar(&scalar);
        Ok(scalar)
    }

    fn read_point(&mut self) -> Result<Point, GeneralizedBulletproofErrorV1> {
        let bytes: [u8; 33] = self
            .take(33)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        let point = Point::from_non_identity_wire_bytes_exact(&bytes).map_err(|error| {
            if matches!(error, super::super::VegaCurveError::IdentityPoint) {
                GeneralizedBulletproofErrorV1::PointIdentity
            } else {
                GeneralizedBulletproofErrorV1::PointEncoding
            }
        })?;
        self.state.append_point(&point)?;
        Ok(point)
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        self.state.challenge(&mut self.challenge_ordinal)
    }
}
