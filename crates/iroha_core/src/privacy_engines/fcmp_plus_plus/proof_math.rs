//! FCMP++ adapters for the shared generalized-Bulletproof backend.
//!
//! Selene/Helios arithmetic and the exact Blake2b transcript remain here so
//! the IFC1 consensus codec stays byte-for-byte pinned. The proof equations,
//! IPA, generators container, and MSM implementation live in
//! `iroha_zkp_halo2::generalized_bulletproof`.

use std::{
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
    sync::OnceLock,
};

use blake2::{Blake2b512, Digest as _};
use iroha_zkp_halo2::generalized_bulletproof::{
    GeneralizedBulletproofErrorV1, ProofRandomSource, ProverTranscript as SharedProverTranscript,
    VerifierTranscript as SharedVerifierTranscript,
};
pub(super) use iroha_zkp_halo2::generalized_bulletproof::{
    ProofGeneratorView, ProofGenerators, ProofPoint, ProofScalar, ProofSuite, multiexp,
};
use p256::elliptic_curve::bigint::U256;
use rand_core_06::{CryptoRng, RngCore};
use zeroize::Zeroize;

use super::{
    FcmpNativeErrorV1,
    field::{
        Field25519, HeliosPoint, HelioseleneField, SelenePoint, decode_field25519,
        decode_helioselene, encode_field25519, encode_helioselene, field25519_from_u64,
        field25519_is_odd, field25519_is_zero, hash_bytes_to_helios, hash_bytes_to_selene,
        helioselene_is_odd, helioselene_is_zero, invert_field25519, invert_helioselene,
        monero_varint, sqrt_field25519, sqrt_helioselene,
    },
};

const SCALAR_TAG: u8 = 0;
const POINT_TAG: u8 = 1;
const CHALLENGE_TAG: u8 = 2;
const CHALLENGE_RETRY_DOMAIN_V1: &[u8] = b"iroha:privacy:fcmp-plus-plus:nonzero-challenge-retry:v1";
const MAX_TRANSCRIPT_CHALLENGE_ATTEMPTS_V1: usize = 128;
const SELENE_BP_GENERATOR_COUNT: usize = 4_096;
const HELIOS_BP_GENERATOR_COUNT: usize = 2_048;

impl From<GeneralizedBulletproofErrorV1> for FcmpNativeErrorV1 {
    fn from(error: GeneralizedBulletproofErrorV1) -> Self {
        match error {
            GeneralizedBulletproofErrorV1::ArithmeticInvariant => Self::ArithmeticInvariant,
            GeneralizedBulletproofErrorV1::ProverRandomnessExhausted => {
                Self::ProverRandomnessExhausted
            }
            GeneralizedBulletproofErrorV1::RandomnessUnavailable => Self::RandomnessUnavailable,
            GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted => {
                Self::TranscriptChallengeExhausted
            }
            GeneralizedBulletproofErrorV1::PointEncoding => Self::CyclePointEncoding,
            GeneralizedBulletproofErrorV1::PointIdentity => Self::CyclePointIdentity,
            GeneralizedBulletproofErrorV1::ScalarEncoding => Self::ScalarEncoding,
            GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity => {
                Self::CircuitProverCommitmentIdentity
            }
            GeneralizedBulletproofErrorV1::InnerProductRoundIdentity => {
                Self::InnerProductRoundIdentity
            }
            GeneralizedBulletproofErrorV1::CircuitEquation => Self::CircuitEquation,
            GeneralizedBulletproofErrorV1::ProofLength { actual, expected } => {
                Self::ProofLength { actual, expected }
            }
            GeneralizedBulletproofErrorV1::ResourceOverflow => Self::TreeFull,
            GeneralizedBulletproofErrorV1::TranscriptConsumption => Self::TranscriptConsumption,
        }
    }
}

fn shared_error(error: FcmpNativeErrorV1) -> GeneralizedBulletproofErrorV1 {
    match error {
        FcmpNativeErrorV1::ArithmeticInvariant => {
            GeneralizedBulletproofErrorV1::ArithmeticInvariant
        }
        FcmpNativeErrorV1::ProverRandomnessExhausted => {
            GeneralizedBulletproofErrorV1::ProverRandomnessExhausted
        }
        FcmpNativeErrorV1::RandomnessUnavailable => {
            GeneralizedBulletproofErrorV1::RandomnessUnavailable
        }
        FcmpNativeErrorV1::TranscriptChallengeExhausted => {
            GeneralizedBulletproofErrorV1::TranscriptChallengeExhausted
        }
        FcmpNativeErrorV1::CyclePointEncoding => GeneralizedBulletproofErrorV1::PointEncoding,
        FcmpNativeErrorV1::CyclePointIdentity => GeneralizedBulletproofErrorV1::PointIdentity,
        FcmpNativeErrorV1::ScalarEncoding => GeneralizedBulletproofErrorV1::ScalarEncoding,
        FcmpNativeErrorV1::CircuitProverCommitmentIdentity => {
            GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity
        }
        FcmpNativeErrorV1::InnerProductRoundIdentity => {
            GeneralizedBulletproofErrorV1::InnerProductRoundIdentity
        }
        FcmpNativeErrorV1::CircuitEquation => GeneralizedBulletproofErrorV1::CircuitEquation,
        FcmpNativeErrorV1::ProofLength { actual, expected } => {
            GeneralizedBulletproofErrorV1::ProofLength { actual, expected }
        }
        FcmpNativeErrorV1::TreeFull => GeneralizedBulletproofErrorV1::ResourceOverflow,
        FcmpNativeErrorV1::TranscriptConsumption => {
            GeneralizedBulletproofErrorV1::TranscriptConsumption
        }
        _ => GeneralizedBulletproofErrorV1::ArithmeticInvariant,
    }
}

macro_rules! impl_proof_scalar {
    (
        $field:ty,
        $decode:ident,
        $encode:ident,
        $zero:ident,
        $odd:ident,
        $sqrt:ident,
        $invert:expr,
        $from_u64:expr
    ) => {
        impl ProofScalar for $field {
            const ZERO: Self = <$field>::ZERO;
            const ONE: Self = <$field>::ONE;
            const SCALAR_BITS: usize = 255;

            fn from_u64(value: u64) -> Self {
                ($from_u64)(value)
            }

            fn decode(bytes: [u8; 32]) -> Option<Self> {
                $decode(bytes)
            }

            fn encode(self) -> [u8; 32] {
                $encode(self)
            }

            fn reduce_wide(bytes: [u8; 64]) -> Self {
                // Preserve the pinned little-endian reduction exactly.
                let radix = Self::from_u64(256);
                bytes.iter().rev().fold(Self::ZERO, |accumulator, byte| {
                    (accumulator * radix) + Self::from_u64(u64::from(*byte))
                })
            }

            fn invert(self) -> Option<Self> {
                ($invert)(self)
            }

            fn sqrt(self) -> Option<Self> {
                $sqrt(self)
            }

            fn square(self) -> Self {
                <$field>::square(&self)
            }

            fn double(self) -> Self {
                self + self
            }

            fn is_zero(self) -> bool {
                $zero(self)
            }

            fn is_odd(self) -> bool {
                $odd(self)
            }

            fn clear_secret(&mut self) {
                Zeroize::zeroize(self);
            }
        }
    };
}

impl_proof_scalar!(
    Field25519,
    decode_field25519,
    encode_field25519,
    field25519_is_zero,
    field25519_is_odd,
    sqrt_field25519,
    invert_field25519,
    field25519_from_u64
);
impl_proof_scalar!(
    HelioseleneField,
    decode_helioselene,
    encode_helioselene,
    helioselene_is_zero,
    helioselene_is_odd,
    sqrt_helioselene,
    invert_helioselene,
    |value: u64| HelioseleneField::new(&U256::from(value))
);

macro_rules! impl_point_operators {
    ($point:ty, $scalar:ty) => {
        impl Add for $point {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                <$point>::add(self, rhs)
            }
        }
        impl AddAssign for $point {
            fn add_assign(&mut self, rhs: Self) {
                *self = <$point>::add(*self, rhs);
            }
        }
        impl Sub for $point {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                <$point>::add(self, -rhs)
            }
        }
        impl SubAssign for $point {
            fn sub_assign(&mut self, rhs: Self) {
                *self = <$point>::add(*self, -rhs);
            }
        }
        impl Neg for $point {
            type Output = Self;
            fn neg(self) -> Self {
                <$point>::negate(self)
            }
        }
        impl Mul<$scalar> for $point {
            type Output = Self;
            fn mul(self, rhs: $scalar) -> Self {
                <$point>::mul(self, rhs.retrieve())
            }
        }
        impl MulAssign<$scalar> for $point {
            fn mul_assign(&mut self, rhs: $scalar) {
                *self = <$point>::mul(*self, rhs.retrieve());
            }
        }
        impl ProofPoint for $point {
            type Scalar = $scalar;
            type Encoded = [u8; 32];
            const POINT_BYTES: usize = 32;

            fn identity() -> Self {
                <$point>::identity()
            }
            fn is_identity(self) -> bool {
                <$point>::is_identity(self)
            }
            fn double(self) -> Self {
                <$point>::double(self)
            }
            fn scale(self, scalar: Self::Scalar) -> Self {
                <$point>::mul(self, scalar.retrieve())
            }
            fn encode(self) -> Self::Encoded {
                <$point>::encode(self)
            }
            fn decode(
                bytes: impl AsRef<[u8]>,
                allow_identity: bool,
            ) -> Result<Self, GeneralizedBulletproofErrorV1> {
                let bytes: [u8; 32] = bytes
                    .as_ref()
                    .try_into()
                    .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?;
                <$point>::decode(bytes, allow_identity).map_err(shared_error)
            }
        }
    };
}

impl_point_operators!(SelenePoint, Field25519);
impl_point_operators!(HeliosPoint, HelioseleneField);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SeleneSuite;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HeliosSuite;

impl ProofSuite for SeleneSuite {
    type Scalar = Field25519;
    type Point = SelenePoint;

    fn generators() -> &'static ProofGenerators<Self> {
        selene_bp_generators()
    }
}

impl ProofSuite for HeliosSuite {
    type Scalar = HelioseleneField;
    type Point = HeliosPoint;

    fn generators() -> &'static ProofGenerators<Self> {
        helios_bp_generators()
    }
}

fn build_generators<S: ProofSuite>(
    curve_name: &[u8],
    count: usize,
    hash_point: fn(&[u8]) -> S::Point,
) -> ProofGenerators<S> {
    let mut g_domain = b"Monero ".to_vec();
    g_domain.extend_from_slice(curve_name);
    g_domain.extend_from_slice(b" G");
    let g = hash_point(&g_domain);
    let mut h_domain = b"Monero ".to_vec();
    h_domain.extend_from_slice(curve_name);
    h_domain.extend_from_slice(b" H");
    let h = hash_point(&h_domain);

    let mut g_bold = Vec::with_capacity(count);
    let mut h_bold = Vec::with_capacity(count);
    for index in 0..count {
        let index = u32::try_from(index).expect("compiled generator count fits u32");
        let mut g_indexed = g_domain.clone();
        g_indexed.push(b' ');
        g_indexed.extend(monero_varint(index));
        g_bold.push(hash_point(&g_indexed));

        let mut h_indexed = h_domain.clone();
        h_indexed.push(b' ');
        h_indexed.extend(monero_varint(index));
        h_bold.push(hash_point(&h_indexed));
    }
    ProofGenerators::new(g, h, g_bold, h_bold)
        .expect("the frozen FCMP generator basis is non-empty and shape-valid")
}

pub(super) fn selene_bp_generators() -> &'static ProofGenerators<SeleneSuite> {
    static CELL: OnceLock<ProofGenerators<SeleneSuite>> = OnceLock::new();
    CELL.get_or_init(|| {
        build_generators::<SeleneSuite>(b"Selene", SELENE_BP_GENERATOR_COUNT, hash_bytes_to_selene)
    })
}

pub(super) fn helios_bp_generators() -> &'static ProofGenerators<HeliosSuite> {
    static CELL: OnceLock<ProofGenerators<HeliosSuite>> = OnceLock::new();
    CELL.get_or_init(|| {
        build_generators::<HeliosSuite>(b"Helios", HELIOS_BP_GENERATOR_COUNT, hash_bytes_to_helios)
    })
}

pub(super) struct FcmpProofRandomSource<'a, R> {
    rng: &'a mut R,
}

impl<'a, R> FcmpProofRandomSource<'a, R> {
    pub(super) fn new(rng: &'a mut R) -> Self {
        Self { rng }
    }
}

impl<R: RngCore + CryptoRng> ProofRandomSource for FcmpProofRandomSource<'_, R> {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1> {
        self.rng
            .try_fill_bytes(destination)
            .map_err(|_| GeneralizedBulletproofErrorV1::RandomnessUnavailable)
    }
}

pub(super) fn random_scalar_from_fcmp_rng<F, R>(rng: &mut R) -> Result<Option<F>, FcmpNativeErrorV1>
where
    F: ProofScalar,
    R: RngCore + CryptoRng,
{
    F::random(&mut FcmpProofRandomSource::new(rng)).map_err(Into::into)
}

pub(super) struct ProverTranscript {
    digest: Blake2b512,
    proof: Vec<u8>,
}

impl ProverTranscript {
    pub(super) fn new(context: [u8; 32]) -> Self {
        let mut digest = Blake2b512::new();
        digest.update(context);
        Self {
            digest,
            proof: Vec::new(),
        }
    }

    pub(super) fn push_scalar<F: ProofScalar>(&mut self, scalar: F) {
        self.digest.update([SCALAR_TAG]);
        let bytes = scalar.encode();
        self.digest.update(bytes);
        self.proof.extend_from_slice(&bytes);
    }

    pub(super) fn push_point<P: ProofPoint>(&mut self, point: P) {
        self.digest.update([POINT_TAG]);
        let bytes = point.encode();
        assert_eq!(P::POINT_BYTES, bytes.as_ref().len());
        self.digest.update(bytes.as_ref());
        self.proof.extend_from_slice(bytes.as_ref());
    }

    pub(super) fn challenge<S: ProofSuite>(&mut self) -> Result<S::Scalar, FcmpNativeErrorV1> {
        challenge::<S>(&mut self.digest)
    }

    pub(super) fn write_commitments<S: ProofSuite>(
        &mut self,
        vector: Vec<S::Point>,
        scalar: Vec<S::Point>,
    ) -> (Vec<S::Point>, Vec<S::Point>) {
        self.digest.update(
            u32::try_from(vector.len())
                .expect("bounded FCMP commitment count fits u32")
                .to_le_bytes(),
        );
        for commitment in &vector {
            self.push_point(*commitment);
        }
        self.digest.update(
            u32::try_from(scalar.len())
                .expect("bounded FCMP commitment count fits u32")
                .to_le_bytes(),
        );
        for commitment in &scalar {
            self.push_point(*commitment);
        }
        (vector, scalar)
    }

    pub(super) fn challenge_bytes(&mut self) -> [u8; 64] {
        self.digest.update([CHALLENGE_TAG]);
        self.digest.clone().finalize().into()
    }

    pub(super) fn complete(self) -> Vec<u8> {
        self.proof
    }
}

pub(super) struct VerifierTranscript<'a> {
    digest: Blake2b512,
    proof: &'a [u8],
    cursor: usize,
}

impl<'a> VerifierTranscript<'a> {
    pub(super) fn new(context: [u8; 32], proof: &'a [u8]) -> Self {
        let mut digest = Blake2b512::new();
        digest.update(context);
        Self {
            digest,
            proof,
            cursor: 0,
        }
    }

    pub(super) fn read_scalar<F: ProofScalar>(&mut self) -> Result<F, FcmpNativeErrorV1> {
        let bytes = self.take()?;
        self.digest.update([SCALAR_TAG]);
        self.digest.update(bytes);
        F::decode(bytes).ok_or(FcmpNativeErrorV1::ScalarEncoding)
    }

    pub(super) fn read_point<P: ProofPoint>(&mut self) -> Result<P, FcmpNativeErrorV1> {
        let bytes = self.take()?;
        self.digest.update([POINT_TAG]);
        self.digest.update(bytes);
        P::decode(bytes, false).map_err(Into::into)
    }

    fn take(&mut self) -> Result<[u8; 32], FcmpNativeErrorV1> {
        let end = self
            .cursor
            .checked_add(32)
            .ok_or(FcmpNativeErrorV1::TreeFull)?;
        let bytes = self
            .proof
            .get(self.cursor..end)
            .ok_or(FcmpNativeErrorV1::ProofLength {
                actual: self.proof.len(),
                expected: end,
            })?;
        let mut element = [0_u8; 32];
        element.copy_from_slice(bytes);
        self.cursor = end;
        Ok(element)
    }

    pub(super) fn challenge<S: ProofSuite>(&mut self) -> Result<S::Scalar, FcmpNativeErrorV1> {
        challenge::<S>(&mut self.digest)
    }

    pub(super) fn read_commitments<S: ProofSuite>(
        &mut self,
        vector_count: usize,
        scalar_count: usize,
    ) -> Result<(Vec<S::Point>, Vec<S::Point>), FcmpNativeErrorV1> {
        self.digest.update(
            u32::try_from(vector_count)
                .map_err(|_| FcmpNativeErrorV1::TreeFull)?
                .to_le_bytes(),
        );
        let mut vector = Vec::with_capacity(vector_count);
        for _ in 0..vector_count {
            vector.push(self.read_point::<S::Point>()?);
        }
        self.digest.update(
            u32::try_from(scalar_count)
                .map_err(|_| FcmpNativeErrorV1::TreeFull)?
                .to_le_bytes(),
        );
        let mut scalar = Vec::with_capacity(scalar_count);
        for _ in 0..scalar_count {
            scalar.push(self.read_point::<S::Point>()?);
        }
        Ok((vector, scalar))
    }

    pub(super) fn challenge_bytes(&mut self) -> [u8; 64] {
        self.digest.update([CHALLENGE_TAG]);
        self.digest.clone().finalize().into()
    }

    pub(super) fn consumed(&self) -> usize {
        self.cursor
    }
}

macro_rules! impl_shared_transcript {
    ($suite:ty) => {
        impl SharedProverTranscript<$suite> for ProverTranscript {
            fn push_scalar(
                &mut self,
                scalar: <$suite as ProofSuite>::Scalar,
            ) -> Result<(), GeneralizedBulletproofErrorV1> {
                ProverTranscript::push_scalar(self, scalar);
                Ok(())
            }

            fn push_point(
                &mut self,
                point: <$suite as ProofSuite>::Point,
            ) -> Result<(), GeneralizedBulletproofErrorV1> {
                if <<$suite as ProofSuite>::Point as ProofPoint>::POINT_BYTES != 32 {
                    return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
                }
                ProverTranscript::push_point(self, point);
                Ok(())
            }

            fn challenge(
                &mut self,
            ) -> Result<<$suite as ProofSuite>::Scalar, GeneralizedBulletproofErrorV1> {
                ProverTranscript::challenge::<$suite>(self).map_err(shared_error)
            }
        }

        impl SharedVerifierTranscript<$suite> for VerifierTranscript<'_> {
            fn read_scalar(
                &mut self,
            ) -> Result<<$suite as ProofSuite>::Scalar, GeneralizedBulletproofErrorV1> {
                VerifierTranscript::read_scalar(self).map_err(shared_error)
            }

            fn read_point(
                &mut self,
            ) -> Result<<$suite as ProofSuite>::Point, GeneralizedBulletproofErrorV1> {
                VerifierTranscript::read_point(self).map_err(shared_error)
            }

            fn challenge(
                &mut self,
            ) -> Result<<$suite as ProofSuite>::Scalar, GeneralizedBulletproofErrorV1> {
                VerifierTranscript::challenge::<$suite>(self).map_err(shared_error)
            }
        }
    };
}

impl_shared_transcript!(SeleneSuite);
impl_shared_transcript!(HeliosSuite);

fn nonzero_challenge_from_scalar_stream<F: ProofScalar>(
    mut scalar_for_attempt: impl FnMut(usize) -> F,
) -> Result<F, FcmpNativeErrorV1> {
    for attempt in 0..MAX_TRANSCRIPT_CHALLENGE_ATTEMPTS_V1 {
        let candidate = scalar_for_attempt(attempt);
        if !candidate.is_zero() {
            return Ok(candidate);
        }
    }
    Err(FcmpNativeErrorV1::TranscriptChallengeExhausted)
}

fn challenge<S: ProofSuite>(digest: &mut Blake2b512) -> Result<S::Scalar, FcmpNativeErrorV1> {
    digest.update([CHALLENGE_TAG]);
    let challenge_state = digest.clone();
    nonzero_challenge_from_scalar_stream(|attempt| {
        let wide = if attempt == 0 {
            challenge_state.clone().finalize().into()
        } else {
            let mut retry = challenge_state.clone();
            retry.update(
                u64::try_from(CHALLENGE_RETRY_DOMAIN_V1.len())
                    .expect("fixed challenge-retry domain length fits u64")
                    .to_be_bytes(),
            );
            retry.update(CHALLENGE_RETRY_DOMAIN_V1);
            retry.update(
                u64::try_from(attempt)
                    .expect("bounded challenge attempt fits u64")
                    .to_be_bytes(),
            );
            retry.finalize().into()
        };
        S::Scalar::reduce_wide(wide)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_bulletproof_generator_domains_extend_tree_generators() {
        let selene = selene_bp_generators();
        let helios = helios_bp_generators();
        assert_eq!(selene.g_bold.len(), SELENE_BP_GENERATOR_COUNT);
        assert_eq!(helios.g_bold.len(), HELIOS_BP_GENERATOR_COUNT);
        assert_eq!(
            selene.g_bold[0].encode(),
            super::super::field::selene_generators()[0].encode()
        );
        assert_eq!(
            helios.g_bold[0].encode(),
            super::super::field::helios_generators()[0].encode()
        );
        assert_ne!(selene.g.encode(), selene.h.encode());
        assert_ne!(helios.g.encode(), helios.h.encode());
    }

    #[test]
    fn pippenger_matches_naive_multiexponentiation() {
        let generators = selene_bp_generators();
        let terms = (0..129)
            .map(|index| {
                (
                    Field25519::from_u64(u64::try_from(index + 1).expect("small index")),
                    generators.g_bold[index],
                )
            })
            .collect::<Vec<_>>();
        let expected = terms
            .iter()
            .fold(SelenePoint::identity(), |sum, (scalar, point)| {
                sum + point.scale(*scalar)
            });
        assert_eq!(multiexp::<SeleneSuite>(&terms), expected);
    }

    #[test]
    fn transcript_tags_and_strict_consumption_are_deterministic() {
        let context = [7_u8; 32];
        let scalar = Field25519::from_u64(9);
        let point = selene_bp_generators().g;
        let mut prover = ProverTranscript::new(context);
        prover.push_scalar(scalar);
        prover.push_point(point);
        let challenge_p = prover.challenge::<SeleneSuite>();
        let bytes = prover.complete();

        let mut verifier = VerifierTranscript::new(context, &bytes);
        assert_eq!(
            verifier.read_scalar::<Field25519>().expect("scalar"),
            scalar
        );
        assert_eq!(verifier.read_point::<SelenePoint>().expect("point"), point);
        assert_eq!(verifier.challenge::<SeleneSuite>(), challenge_p);
        assert_eq!(verifier.consumed(), bytes.len());
    }

    #[test]
    fn transcript_challenge_retries_zero_and_exhausts_at_a_fixed_bound() {
        let mut attempts = Vec::new();
        let challenge = nonzero_challenge_from_scalar_stream::<Field25519>(|attempt| {
            attempts.push(attempt);
            if attempt < 2 {
                Field25519::ZERO
            } else {
                Field25519::from_u64(7)
            }
        })
        .expect("third challenge is non-zero");
        assert_eq!(challenge, Field25519::from_u64(7));
        assert_eq!(attempts, vec![0, 1, 2]);

        let mut exhaustion_attempts = 0;
        assert_eq!(
            nonzero_challenge_from_scalar_stream::<Field25519>(|_| {
                exhaustion_attempts += 1;
                Field25519::ZERO
            }),
            Err(FcmpNativeErrorV1::TranscriptChallengeExhausted)
        );
        assert_eq!(exhaustion_attempts, MAX_TRANSCRIPT_CHALLENGE_ATTEMPTS_V1);
        assert_eq!(MAX_TRANSCRIPT_CHALLENGE_ATTEMPTS_V1, 128);
    }
}
