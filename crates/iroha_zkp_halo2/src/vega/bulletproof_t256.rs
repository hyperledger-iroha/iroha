//! Canonical T256 adapter for the reusable generalized-Bulletproof backend.
//!
//! This module freezes the T256 generator domains and typed transcript used by
//! exact ZK-AMS coefficient-membership arguments.  It deliberately does not
//! reuse FCMP's 32-byte cycle transcript: T256 proof points occupy 33 bytes and
//! its scalar field uses the canonical Vega little-endian proof encoding.

#![allow(dead_code)]

use core::{
    marker::PhantomData,
    ops::{AddAssign, Neg, SubAssign},
};
use std::sync::OnceLock;

use halo2curves::ff::Field as _;

use crate::generalized_bulletproof::{
    GeneralizedBulletproofErrorV1, ProofGenerators, ProofPoint, ProofScalar, ProofSuite,
    ProverTranscript, VerifierTranscript,
};

use super::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar, derive_t256_generators_v1,
    sponge::{Keccak256, keccak256},
};

const T256_BP_MAX_GATES_V1: usize = 65_536;
const T256_BP_TRANSCRIPT_DOMAIN_V1: &[u8] = b"iroha.generalized-bulletproof.t256.transcript.v1";
const T256_BP_CHALLENGE_DOMAIN_V1: &[u8] = b"iroha.generalized-bulletproof.t256.challenge.v1";
const T256_BP_GENERATOR_BASIS_DOMAIN_V1: &[u8] =
    b"iroha.generalized-bulletproof.t256.generator-basis.v1";
const T256_BP_G_LABEL_V1: &[u8] = b"iroha.generalized-bulletproof.t256.g.v1";
const T256_BP_H_LABEL_V1: &[u8] = b"iroha.generalized-bulletproof.t256.h.v1";
const T256_BP_G_BOLD_LABEL_V1: &[u8] = b"iroha.generalized-bulletproof.t256.G.v1";
const T256_BP_H_BOLD_LABEL_V1: &[u8] = b"iroha.generalized-bulletproof.t256.H.v1";
const T256_BP_SCALAR_TAG_V1: u8 = 0;
const T256_BP_POINT_TAG_V1: u8 = 1;
const T256_BP_CHALLENGE_TAG_V1: u8 = 2;
const T256_BP_MAX_CHALLENGE_ATTEMPTS_V1: usize = 128;

impl ProofScalar for Scalar {
    const ZERO: Self = Self::zero();
    const ONE: Self = Self::one();
    const SCALAR_BITS: usize = 256;

    fn from_u64(value: u64) -> Self {
        Self::from_u64(value)
    }

    fn decode(bytes: [u8; 32]) -> Option<Self> {
        Self::from_le_bytes_exact(bytes).ok()
    }

    fn encode(self) -> [u8; 32] {
        self.to_le_bytes()
    }

    fn reduce_wide(bytes: [u8; 64]) -> Self {
        Self::from_uniform_le_bytes(bytes)
    }

    fn invert(self) -> Option<Self> {
        self.inverse().ok()
    }

    fn sqrt(self) -> Option<Self> {
        Option::from(self.0.sqrt()).map(Self)
    }

    fn square(self) -> Self {
        self.square()
    }

    fn double(self) -> Self {
        self + self
    }

    fn is_zero(self) -> bool {
        self.is_zero()
    }

    fn is_odd(self) -> bool {
        self.to_le_bytes()[0] & 1 == 1
    }

    fn clear_secret(&mut self) {
        *self = Self::zero();
    }
}

impl Neg for Point {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self.negate()
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl SubAssign for Point {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl ProofPoint for Point {
    type Scalar = Scalar;
    type Encoded = [u8; 33];
    const POINT_BYTES: usize = 33;

    fn identity() -> Self {
        Self::identity()
    }

    fn is_identity(self) -> bool {
        self.is_identity()
    }

    fn double(self) -> Self {
        self + self
    }

    fn scale(self, scalar: Self::Scalar) -> Self {
        self.mul_scalar(scalar)
    }

    fn encode(self) -> Self::Encoded {
        if self.is_identity() {
            let mut identity = [0_u8; 33];
            identity[0] = 0x40;
            identity
        } else {
            self.to_non_identity_wire_bytes()
                .expect("non-identity T256 point has a canonical encoding")
        }
    }

    fn decode(
        bytes: impl AsRef<[u8]>,
        allow_identity: bool,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let bytes = bytes.as_ref();
        if allow_identity && bytes.len() == Self::POINT_BYTES {
            let mut identity = [0_u8; 33];
            identity[0] = 0x40;
            if bytes == identity {
                return Ok(Self::identity());
            }
        }
        Self::from_non_identity_wire_bytes_exact(bytes).map_err(|error| {
            if matches!(error, super::VegaCurveError::IdentityPoint) {
                GeneralizedBulletproofErrorV1::PointIdentity
            } else {
                GeneralizedBulletproofErrorV1::PointEncoding
            }
        })
    }
}

/// The full T256 basis needed by the largest exact membership circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsT256BulletproofSuiteV1;

impl ProofSuite for ZkAmsT256BulletproofSuiteV1 {
    type Scalar = Scalar;
    type Point = Point;

    fn generators() -> &'static ProofGenerators<Self> {
        static GENERATORS: OnceLock<ProofGenerators<ZkAmsT256BulletproofSuiteV1>> = OnceLock::new();
        GENERATORS.get_or_init(|| {
            build_t256_generators::<Self>(
                T256_BP_G_LABEL_V1,
                T256_BP_H_LABEL_V1,
                T256_BP_G_BOLD_LABEL_V1,
                T256_BP_H_BOLD_LABEL_V1,
                T256_BP_MAX_GATES_V1,
            )
        })
    }
}

fn one_generator(label: &[u8]) -> Point {
    derive_t256_generators_v1(label, 1)
        .expect("fixed T256 Bulletproof generator label is valid")
        .pop()
        .expect("one T256 Bulletproof generator was requested")
}

fn build_t256_generators<S>(
    g_label: &[u8],
    h_label: &[u8],
    g_bold_label: &[u8],
    h_bold_label: &[u8],
    count: usize,
) -> ProofGenerators<S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    let g = one_generator(g_label);
    let h = one_generator(h_label);
    let g_bold = derive_t256_generators_v1(g_bold_label, count)
        .expect("fixed T256 Bulletproof G basis is valid");
    let h_bold = derive_t256_generators_v1(h_bold_label, count)
        .expect("fixed T256 Bulletproof H basis is valid");
    ProofGenerators::new(g, h, g_bold, h_bold)
        .expect("fixed T256 Bulletproof basis has canonical shape")
}

/// Digest of every point in the full, ordered T256 generator basis.
pub(super) fn zk_ams_t256_bulletproof_generator_basis_digest_v1() -> [u8; 32] {
    static DIGEST: OnceLock<[u8; 32]> = OnceLock::new();
    *DIGEST.get_or_init(|| {
        let generators = ZkAmsT256BulletproofSuiteV1::generators();
        let mut hash = Keccak256::new();
        hash.update(T256_BP_GENERATOR_BASIS_DOMAIN_V1);
        hash.update(
            &u32::try_from(generators.g_bold.len())
                .expect("fixed T256 basis length fits u32")
                .to_be_bytes(),
        );
        for point in core::iter::once(generators.g)
            .chain(core::iter::once(generators.h))
            .chain(generators.g_bold.iter().copied())
            .chain(generators.h_bold.iter().copied())
        {
            hash.update(
                &point
                    .to_non_identity_wire_bytes()
                    .expect("fixed T256 basis excludes identity"),
            );
        }
        hash.finalize()
    })
}

fn initialize_transcript_state(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    chunk_ordinal: u16,
    coefficient_bound: u8,
    commitment: Point,
) -> Result<Vec<u8>, GeneralizedBulletproofErrorV1> {
    if context_digest == [0; 32]
        || generator_basis_digest == [0; 32]
        || !matches!(coefficient_bound, 1 | 2)
        || commitment.is_identity()
    {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }
    let mut state = Vec::with_capacity(T256_BP_TRANSCRIPT_DOMAIN_V1.len() + 32 + 32 + 2 + 1 + 33);
    state.extend_from_slice(T256_BP_TRANSCRIPT_DOMAIN_V1);
    state.extend_from_slice(&context_digest);
    state.extend_from_slice(&generator_basis_digest);
    state.extend_from_slice(&chunk_ordinal.to_be_bytes());
    state.push(coefficient_bound);
    state.extend_from_slice(
        &commitment
            .to_non_identity_wire_bytes()
            .map_err(|_| GeneralizedBulletproofErrorV1::PointIdentity)?,
    );
    Ok(state)
}

fn append_scalar(state: &mut Vec<u8>, scalar: Scalar) {
    state.push(T256_BP_SCALAR_TAG_V1);
    state.extend_from_slice(&scalar.to_le_bytes());
}

fn append_point(
    state: &mut Vec<u8>,
    point: Point,
) -> Result<[u8; 33], GeneralizedBulletproofErrorV1> {
    let encoded = point
        .to_non_identity_wire_bytes()
        .map_err(|_| GeneralizedBulletproofErrorV1::PointIdentity)?;
    state.push(T256_BP_POINT_TAG_V1);
    state.extend_from_slice(&encoded);
    Ok(encoded)
}

fn derive_nonzero_challenge(
    state: &mut Vec<u8>,
    ordinal: &mut u32,
) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
    for attempt in 0..T256_BP_MAX_CHALLENGE_ATTEMPTS_V1 {
        let attempt =
            u8::try_from(attempt).map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let mut input =
            Vec::with_capacity(T256_BP_CHALLENGE_DOMAIN_V1.len() + state.len() + 4 + 1 + 1);
        input.extend_from_slice(T256_BP_CHALLENGE_DOMAIN_V1);
        input.extend_from_slice(state);
        input.extend_from_slice(&ordinal.to_be_bytes());
        input.push(attempt);
        let mut low = input.clone();
        low.push(0);
        input.push(1);
        let mut wide = [0_u8; 64];
        wide[..32].copy_from_slice(&keccak256(&low));
        wide[32..].copy_from_slice(&keccak256(&input));
        let challenge = Scalar::from_uniform_le_bytes(wide);
        wide.fill(0);
        if !challenge.is_zero() {
            state.push(T256_BP_CHALLENGE_TAG_V1);
            state.extend_from_slice(&ordinal.to_be_bytes());
            state.push(attempt);
            state.extend_from_slice(&challenge.to_le_bytes());
            *ordinal = ordinal
                .checked_add(1)
                .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
            return Ok(challenge);
        }
    }
    Err(GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted)
}

/// Typed prover transcript for one T256 membership-proof chunk.
pub(super) struct T256BulletproofProverTranscriptV1<S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: Vec<u8>,
    proof: Vec<u8>,
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
        commitment: Point,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        Ok(Self {
            state: initialize_transcript_state(
                context_digest,
                generator_basis_digest,
                chunk_ordinal,
                coefficient_bound,
                commitment,
            )?,
            proof: Vec::new(),
            challenge_ordinal: 0,
            suite: PhantomData,
        })
    }

    pub(super) fn complete(self) -> (Vec<u8>, [u8; 32]) {
        (self.proof, keccak256(&self.state))
    }
}

impl<S> ProverTranscript<S> for T256BulletproofProverTranscriptV1<S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    fn push_scalar(&mut self, scalar: Scalar) -> Result<(), GeneralizedBulletproofErrorV1> {
        append_scalar(&mut self.state, scalar);
        self.proof.extend_from_slice(&scalar.to_le_bytes());
        Ok(())
    }

    fn push_point(&mut self, point: Point) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = append_point(&mut self.state, point)?;
        self.proof.extend_from_slice(&encoded);
        Ok(())
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        derive_nonzero_challenge(&mut self.state, &mut self.challenge_ordinal)
    }
}

/// Exact, allocation-bounded verifier transcript over attacker proof bytes.
pub(super) struct T256BulletproofVerifierTranscriptV1<'a, S>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    state: Vec<u8>,
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
            state: initialize_transcript_state(
                context_digest,
                generator_basis_digest,
                chunk_ordinal,
                coefficient_bound,
                commitment,
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
        Ok(keccak256(&self.state))
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
        append_scalar(&mut self.state, scalar);
        Ok(scalar)
    }

    fn read_point(&mut self) -> Result<Point, GeneralizedBulletproofErrorV1> {
        let bytes: [u8; 33] = self
            .take(33)?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
        let point = Point::from_non_identity_wire_bytes_exact(&bytes).map_err(|error| {
            if matches!(error, super::VegaCurveError::IdentityPoint) {
                GeneralizedBulletproofErrorV1::PointIdentity
            } else {
                GeneralizedBulletproofErrorV1::PointEncoding
            }
        })?;
        append_point(&mut self.state, point)?;
        Ok(point)
    }

    fn challenge(&mut self) -> Result<Scalar, GeneralizedBulletproofErrorV1> {
        derive_nonzero_challenge(&mut self.state, &mut self.challenge_ordinal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        generalized_bulletproof::{
            ArithmeticCircuitStatement, ArithmeticCircuitWitness, LinComb, ProofRandomSource,
            Variable, VectorCommitmentOpening, multiexp,
        },
        vega::VEGA_T256_SCALAR_MODULUS_BE_V1,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TinyT256Suite;

    impl ProofSuite for TinyT256Suite {
        type Scalar = Scalar;
        type Point = Point;

        fn generators() -> &'static ProofGenerators<Self> {
            static GENERATORS: OnceLock<ProofGenerators<TinyT256Suite>> = OnceLock::new();
            GENERATORS.get_or_init(|| {
                build_t256_generators::<Self>(
                    b"iroha.generalized-bulletproof.t256.test.g.v1",
                    b"iroha.generalized-bulletproof.t256.test.h.v1",
                    b"iroha.generalized-bulletproof.t256.test.G.v1",
                    b"iroha.generalized-bulletproof.t256.test.H.v1",
                    4,
                )
            })
        }
    }

    struct KatRandom {
        seed: [u8; 32],
        counter: u64,
    }

    impl KatRandom {
        fn new(label: &[u8]) -> Self {
            Self {
                seed: keccak256(label),
                counter: 0,
            }
        }
    }

    impl ProofRandomSource for KatRandom {
        fn fill_bytes(
            &mut self,
            destination: &mut [u8],
        ) -> Result<(), GeneralizedBulletproofErrorV1> {
            let mut written = 0;
            while written < destination.len() {
                let mut input = Vec::with_capacity(40);
                input.extend_from_slice(&self.seed);
                input.extend_from_slice(&self.counter.to_be_bytes());
                let block = keccak256(&input);
                self.counter = self
                    .counter
                    .checked_add(1)
                    .ok_or(GeneralizedBulletproofErrorV1::RandomnessUnavailable)?;
                let take = (destination.len() - written).min(block.len());
                destination[written..written + take].copy_from_slice(&block[..take]);
                written += take;
            }
            Ok(())
        }
    }

    fn fixture() -> (
        [u8; 32],
        [u8; 32],
        Point,
        Vec<LinComb<Scalar>>,
        ArithmeticCircuitWitness<TinyT256Suite>,
    ) {
        let generators = TinyT256Suite::generators().reduce(4).expect("tiny basis");
        let values = vec![
            Scalar::from_u64(7),
            Scalar::from_u64(8),
            Scalar::from_u64(9),
            Scalar::from_u64(10),
        ];
        let mask = Scalar::from_u64(13);
        let mut commitment_terms = values
            .iter()
            .copied()
            .zip(generators.g_bold.iter().copied())
            .collect::<Vec<_>>();
        commitment_terms.push((mask, generators.h));
        let commitment = multiexp::<TinyT256Suite>(&commitment_terms);
        let constraints = vec![
            LinComb::empty()
                .term(Scalar::ONE, Variable::aO(0))
                .constant(-Scalar::from_u64(12)),
            LinComb::empty()
                .term(Scalar::ONE, Variable::aO(1))
                .constant(-Scalar::from_u64(30)),
            LinComb::empty()
                .term(Scalar::ONE, Variable::aL(0))
                .term(
                    Scalar::ONE,
                    Variable::CG {
                        commitment: 0,
                        index: 0,
                    },
                )
                .constant(-Scalar::from_u64(10)),
            LinComb::empty()
                .term(Scalar::ONE, Variable::aR(1))
                .term(
                    Scalar::ONE,
                    Variable::CG {
                        commitment: 0,
                        index: 3,
                    },
                )
                .constant(-Scalar::from_u64(16)),
        ];
        let witness = ArithmeticCircuitWitness::<TinyT256Suite>::new(
            vec![Scalar::from_u64(3), Scalar::from_u64(5)],
            vec![Scalar::from_u64(4), Scalar::from_u64(6)],
            vec![VectorCommitmentOpening::new(values, mask)],
        )
        .expect("shape-valid witness");
        (
            keccak256(b"t256-bp-test-context"),
            keccak256(b"t256-bp-test-basis"),
            commitment,
            constraints,
            witness,
        )
    }

    fn verify_fixture(
        context: [u8; 32],
        basis: [u8; 32],
        commitment: Point,
        proof: &[u8],
    ) -> Result<[u8; 32], GeneralizedBulletproofErrorV1> {
        let (_, _, _, constraints, _) = fixture();
        let generators = TinyT256Suite::generators().reduce(4)?;
        let mut transcript = T256BulletproofVerifierTranscriptV1::<TinyT256Suite>::new(
            context, basis, 7, 2, commitment, proof,
        )?;
        ArithmeticCircuitStatement::new(generators, constraints, vec![commitment], Vec::new())?
            .verify(&mut transcript)?;
        transcript.finish()
    }

    #[test]
    fn t256_scalar_and_point_adapters_are_exact_and_non_malleable() {
        let scalar = Scalar::from_u64(0x0102);
        assert_eq!(
            <Scalar as ProofScalar>::decode(scalar.to_le_bytes()),
            Some(scalar)
        );
        let mut noncanonical = VEGA_T256_SCALAR_MODULUS_BE_V1;
        noncanonical.reverse();
        assert_eq!(<Scalar as ProofScalar>::decode(noncanonical), None);
        assert_eq!(<Scalar as ProofScalar>::SCALAR_BITS, 256);
        assert_eq!(
            <Scalar as ProofScalar>::sqrt(scalar.square()).map(|root| root.square()),
            Some(scalar.square())
        );

        let point = TinyT256Suite::generators().g;
        let encoded = <Point as ProofPoint>::encode(point);
        assert_eq!(encoded.len(), 33);
        assert_eq!(<Point as ProofPoint>::decode(encoded, false), Ok(point));
        let mut identity = [0_u8; 33];
        identity[0] = 0x40;
        assert_eq!(
            <Point as ProofPoint>::decode(identity, false),
            Err(GeneralizedBulletproofErrorV1::PointIdentity)
        );
        assert!(
            <Point as ProofPoint>::decode(identity, true)
                .expect("explicitly admitted identity")
                .is_identity()
        );
        let mut bad_flag = encoded;
        bad_flag[0] = 0x20;
        assert_eq!(
            <Point as ProofPoint>::decode(bad_flag, false),
            Err(GeneralizedBulletproofErrorV1::PointEncoding)
        );
        assert_eq!(
            <Point as ProofPoint>::decode([0_u8; 34], false),
            Err(GeneralizedBulletproofErrorV1::PointEncoding)
        );
    }

    #[test]
    fn t256_generalized_bulletproof_roundtrip_binds_every_byte_and_axis() {
        let (context, basis, commitment, constraints, witness) = fixture();
        let generators = TinyT256Suite::generators().reduce(4).expect("tiny basis");
        let mut transcript = T256BulletproofProverTranscriptV1::<TinyT256Suite>::new(
            context, basis, 7, 2, commitment,
        )
        .expect("canonical context");
        ArithmeticCircuitStatement::new(generators, constraints, vec![commitment], Vec::new())
            .expect("statement")
            .prove(
                &mut KatRandom::new(b"t256-bp-test-rng"),
                &mut transcript,
                witness,
            )
            .expect("proof");
        let (proof, prover_digest) = transcript.complete();
        assert_eq!(proof.len(), 589);
        assert_eq!(
            verify_fixture(context, basis, commitment, &proof),
            Ok(prover_digest)
        );

        for index in 0..proof.len() {
            let mut changed = proof.clone();
            changed[index] ^= 1;
            assert!(
                verify_fixture(context, basis, commitment, &changed).is_err(),
                "changed proof byte {index} was accepted"
            );
        }
        for end in 0..proof.len() {
            assert!(verify_fixture(context, basis, commitment, &proof[..end]).is_err());
        }
        let mut trailing = proof.clone();
        trailing.push(0);
        assert_eq!(
            verify_fixture(context, basis, commitment, &trailing),
            Err(GeneralizedBulletproofErrorV1::TranscriptConsumption)
        );
        let other_commitment = commitment + TinyT256Suite::generators().g;
        for result in [
            verify_fixture(keccak256(b"wrong-context"), basis, commitment, &proof),
            verify_fixture(context, keccak256(b"wrong-basis"), commitment, &proof),
            verify_fixture(context, basis, other_commitment, &proof),
        ] {
            assert!(result.is_err());
        }
    }
}
