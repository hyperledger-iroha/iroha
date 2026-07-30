//! Concrete proof algebra shared by the two FCMP++ Bulletproof systems.
//!
//! The abstractions in this file are intentionally private and support only
//! the Selene/Helios cycle. They preserve the pinned upstream transcript and
//! generator derivation without adding a generic consensus dependency.

use std::{
    fmt::Debug,
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
    sync::OnceLock,
};

use blake2::{Blake2b512, Digest as _};
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

pub(super) trait ProofScalar:
    Copy
    + Clone
    + Debug
    + Eq
    + Send
    + Sync
    + Zeroize
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + 'static
{
    const ZERO: Self;
    const ONE: Self;

    fn from_u64(value: u64) -> Self;
    fn random(rng: &mut (impl RngCore + CryptoRng)) -> Result<Option<Self>, FcmpNativeErrorV1>;
    fn decode(bytes: [u8; 32]) -> Option<Self>;
    fn encode(self) -> [u8; 32];
    fn reduce_wide(bytes: [u8; 64]) -> Self;
    fn invert(self) -> Option<Self>;
    fn sqrt(self) -> Option<Self>;
    fn square(self) -> Self;
    fn double(self) -> Self;
    fn is_zero(self) -> bool;
    fn is_odd(self) -> bool;
    fn bits_le(self) -> [u8; 32] {
        self.encode()
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

            fn from_u64(value: u64) -> Self {
                ($from_u64)(value)
            }

            fn random(
                rng: &mut (impl RngCore + CryptoRng),
            ) -> Result<Option<Self>, FcmpNativeErrorV1> {
                let mut bytes = [0_u8; 32];
                if rng.try_fill_bytes(&mut bytes).is_err() {
                    bytes.fill(0);
                    return Err(FcmpNativeErrorV1::RandomnessUnavailable);
                }
                let value = $decode(bytes);
                bytes.fill(0);
                Ok(value)
            }

            fn decode(bytes: [u8; 32]) -> Option<Self> {
                $decode(bytes)
            }

            fn encode(self) -> [u8; 32] {
                $encode(self)
            }

            fn reduce_wide(bytes: [u8; 64]) -> Self {
                // `reduce_512` interprets the digest as a little-endian integer.
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

pub(super) trait ProofPoint:
    Copy
    + Clone
    + Debug
    + Eq
    + Send
    + Sync
    + Add<Output = Self>
    + Sub<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + 'static
{
    type Scalar: ProofScalar;

    fn identity() -> Self;
    fn is_identity(self) -> bool;
    fn double(self) -> Self;
    fn scale(self, scalar: Self::Scalar) -> Self;
    fn encode(self) -> [u8; 32];
    fn decode(bytes: [u8; 32], allow_identity: bool) -> Result<Self, FcmpNativeErrorV1>;
}

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
            fn encode(self) -> [u8; 32] {
                <$point>::encode(self)
            }
            fn decode(bytes: [u8; 32], allow_identity: bool) -> Result<Self, FcmpNativeErrorV1> {
                <$point>::decode(bytes, allow_identity)
            }
        }
    };
}

impl_point_operators!(SelenePoint, Field25519);
impl_point_operators!(HeliosPoint, HelioseleneField);

pub(super) trait ProofSuite: Copy + Clone + Debug + Eq + Send + Sync + 'static {
    type Scalar: ProofScalar;
    type Point: ProofPoint<Scalar = Self::Scalar>;
    fn generators() -> &'static ProofGenerators<Self>;
}

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

#[derive(Clone, Debug)]
pub(super) struct ProofGenerators<S: ProofSuite> {
    pub(super) g: S::Point,
    pub(super) h: S::Point,
    pub(super) g_bold: Vec<S::Point>,
    pub(super) h_bold: Vec<S::Point>,
    pub(super) h_sum: Vec<S::Point>,
}

impl<S: ProofSuite> ProofGenerators<S> {
    pub(super) fn reduce(
        &self,
        count: usize,
    ) -> Result<ProofGeneratorView<'_, S>, FcmpNativeErrorV1> {
        if count == 0 || !count.is_power_of_two() || count > self.g_bold.len() {
            return Err(FcmpNativeErrorV1::ArithmeticInvariant);
        }
        Ok(ProofGeneratorView {
            g: self.g,
            h: self.h,
            g_bold: &self.g_bold[..count],
            h_bold: &self.h_bold[..count],
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ProofGeneratorView<'a, S: ProofSuite> {
    pub(super) g: S::Point,
    pub(super) h: S::Point,
    pub(super) g_bold: &'a [S::Point],
    pub(super) h_bold: &'a [S::Point],
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
    let mut h_sum = Vec::new();
    let mut running = S::Point::identity();
    let mut next_power = 1;
    for (index, point) in h_bold.iter().copied().enumerate() {
        running += point;
        if index + 1 == next_power {
            h_sum.push(running);
            next_power *= 2;
        }
    }
    ProofGenerators {
        g,
        h,
        g_bold,
        h_bold,
        h_sum,
    }
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

pub(super) fn multiexp<S: ProofSuite>(terms: &[(S::Scalar, S::Point)]) -> S::Point {
    if terms.is_empty() {
        return S::Point::identity();
    }
    if terms.len() == 1 {
        return terms[0].1.scale(terms[0].0);
    }
    let window = match terms.len() {
        0..=124 => 4,
        125..=274 => 5,
        275..=474 => 6,
        475..=874 => 7,
        _ => 8,
    };
    pippenger::<S>(terms, window)
}

fn pippenger<S: ProofSuite>(terms: &[(S::Scalar, S::Point)], window: usize) -> S::Point {
    let windows = 255_usize.div_ceil(window);
    let mask = (1_u16 << window) - 1;
    let scalar_bytes = terms
        .iter()
        .map(|(scalar, _)| scalar.bits_le())
        .collect::<Vec<_>>();
    let mut result = S::Point::identity();
    for window_index in (0..windows).rev() {
        if window_index + 1 != windows {
            for _ in 0..window {
                result = result.double();
            }
        }
        let mut buckets = vec![S::Point::identity(); 1 << window];
        let bit_offset = window_index * window;
        for ((_, point), bytes) in terms.iter().zip(&scalar_bytes) {
            let byte_index = bit_offset / 8;
            let shift = bit_offset % 8;
            let mut word = u16::from(bytes[byte_index]) >> shift;
            if shift + window > 8 && byte_index + 1 < bytes.len() {
                word |= u16::from(bytes[byte_index + 1]) << (8 - shift);
            }
            let digit = usize::from(word & mask);
            if digit != 0 {
                buckets[digit] += *point;
            }
        }
        let mut running = S::Point::identity();
        for bucket in buckets.into_iter().skip(1).rev() {
            running += bucket;
            result += running;
        }
    }
    result
}

#[derive(Clone, Debug)]
pub(super) struct BatchVerifier<S: ProofSuite> {
    pub(super) g: S::Scalar,
    pub(super) h: S::Scalar,
    pub(super) g_bold: Vec<S::Scalar>,
    pub(super) h_bold: Vec<S::Scalar>,
    pub(super) h_sum: Vec<S::Scalar>,
    pub(super) additional: Vec<(S::Scalar, S::Point)>,
}

impl<S: ProofSuite> BatchVerifier<S> {
    pub(super) fn new() -> Self {
        Self {
            g: S::Scalar::ZERO,
            h: S::Scalar::ZERO,
            g_bold: Vec::new(),
            h_bold: Vec::new(),
            h_sum: Vec::new(),
            additional: Vec::new(),
        }
    }

    pub(super) fn ensure_len(&mut self, len: usize) {
        if self.g_bold.len() < len {
            self.g_bold.resize(len, S::Scalar::ZERO);
            self.h_bold.resize(len, S::Scalar::ZERO);
            self.h_sum.resize(len, S::Scalar::ZERO);
        }
    }

    pub(super) fn verify(self) -> bool {
        let generators = S::generators();
        let mut terms = Vec::with_capacity(
            2 + self.g_bold.len() + self.h_bold.len() + self.h_sum.len() + self.additional.len(),
        );
        terms.push((self.g, generators.g));
        terms.push((self.h, generators.h));
        terms.extend(
            self.g_bold
                .into_iter()
                .zip(generators.g_bold.iter().copied()),
        );
        terms.extend(
            self.h_bold
                .into_iter()
                .zip(generators.h_bold.iter().copied()),
        );
        terms.extend(self.h_sum.into_iter().zip(generators.h_sum.iter().copied()));
        terms.extend(self.additional);
        multiexp::<S>(&terms).is_identity()
    }
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
        self.digest.update(bytes);
        self.proof.extend_from_slice(&bytes);
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
        P::decode(bytes, false)
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
