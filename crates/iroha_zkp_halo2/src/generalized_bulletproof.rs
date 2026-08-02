//! Reusable generalized-Bulletproof arithmetic and inner-product backend.
//!
//! This module owns the curve-agnostic proof equations. Transcript codecs,
//! concrete curves, generator derivation domains, and entropy providers remain
//! explicit adapters so a protocol can freeze its own consensus bytes.

#![allow(missing_docs)]

use std::{
    fmt::Debug,
    ops::{Add, AddAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign},
};

use thiserror::Error;

/// Stable failure classes emitted by the generalized-Bulletproof backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum GeneralizedBulletproofErrorV1 {
    #[error("generalized-Bulletproof arithmetic invariant failed")]
    ArithmeticInvariant,
    #[error("generalized-Bulletproof prover randomness exhausted its retry bound")]
    ProverRandomnessExhausted,
    #[error("generalized-Bulletproof randomness source is unavailable")]
    RandomnessUnavailable,
    #[error("generalized-Bulletproof transcript challenge exhausted its retry bound")]
    TranscriptChallengeExhausted,
    #[error("generalized-Bulletproof point encoding is invalid")]
    PointEncoding,
    #[error("generalized-Bulletproof point must be non-identity")]
    PointIdentity,
    #[error("generalized-Bulletproof scalar encoding is non-canonical")]
    ScalarEncoding,
    #[error("generalized-Bulletproof prover commitment was the identity")]
    CircuitProverCommitmentIdentity,
    #[error("generalized-Bulletproof inner-product round produced an identity")]
    InnerProductRoundIdentity,
    #[error("generalized-Bulletproof verification equation failed")]
    CircuitEquation,
    #[error("generalized-Bulletproof proof length {actual} does not equal {expected}")]
    ProofLength { actual: usize, expected: usize },
    #[error("generalized-Bulletproof resource bound overflowed")]
    ResourceOverflow,
    #[error("generalized-Bulletproof transcript was not consumed exactly")]
    TranscriptConsumption,
}

/// Fallible cryptographic byte source.
///
/// Implementations must either fill the complete destination or return an
/// error. The backend never falls back to deterministic or partial entropy.
pub trait ProofRandomSource {
    /// Fill all of `destination` or report entropy unavailability.
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), GeneralizedBulletproofErrorV1>;
}

/// Fixed number of canonical-decoding attempts for one prover scalar.
pub const MAX_PROVER_SCALAR_ATTEMPTS_V1: usize = 128;

/// Sample one canonical scalar from a fallible byte source.
///
/// Non-canonical candidates are rejected with a fixed public retry bound;
/// entropy failure is returned immediately and never replaced with fallback
/// bytes.
pub fn random_scalar<F, R>(rng: &mut R) -> Result<F, GeneralizedBulletproofErrorV1>
where
    F: ProofScalar,
    R: ProofRandomSource,
{
    for _ in 0..MAX_PROVER_SCALAR_ATTEMPTS_V1 {
        if let Some(scalar) = F::random(rng)? {
            return Ok(scalar);
        }
    }
    Err(GeneralizedBulletproofErrorV1::ProverRandomnessExhausted)
}

/// Scalar operations required by the shared arithmetic proof.
pub trait ProofScalar:
    Copy
    + Clone
    + Debug
    + Eq
    + Send
    + Sync
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
    /// Number of significant little-endian scalar bits used by MSM.
    const SCALAR_BITS: usize;

    fn from_u64(value: u64) -> Self;
    fn decode(bytes: [u8; 32]) -> Option<Self>;
    fn encode(self) -> [u8; 32];
    fn reduce_wide(bytes: [u8; 64]) -> Self;
    fn invert(self) -> Option<Self>;
    fn sqrt(self) -> Option<Self>;
    fn square(self) -> Self;
    fn double(self) -> Self;
    fn is_zero(self) -> bool;
    fn is_odd(self) -> bool;
    fn clear_secret(&mut self);

    fn bits_le(self) -> [u8; 32] {
        self.encode()
    }

    fn random(
        rng: &mut impl ProofRandomSource,
    ) -> Result<Option<Self>, GeneralizedBulletproofErrorV1> {
        let mut bytes = [0_u8; 32];
        if rng.fill_bytes(&mut bytes).is_err() {
            bytes.fill(0);
            return Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable);
        }
        let value = Self::decode(bytes);
        bytes.fill(0);
        Ok(value)
    }
}

/// Group operations required by the shared arithmetic proof.
///
/// `POINT_BYTES` and `Encoded` make point width a suite property: FCMP uses
/// 32-byte cycle points while the T256 adapter uses 33-byte compressed points.
pub trait ProofPoint:
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
    type Encoded: AsRef<[u8]> + Copy + Clone + Debug + Eq + Send + Sync + 'static;
    const POINT_BYTES: usize;

    fn identity() -> Self;
    fn is_identity(self) -> bool;
    fn double(self) -> Self;
    fn scale(self, scalar: Self::Scalar) -> Self;
    fn encode(self) -> Self::Encoded;
    fn decode(
        bytes: impl AsRef<[u8]>,
        allow_identity: bool,
    ) -> Result<Self, GeneralizedBulletproofErrorV1>;
}

/// Curve suite binding one scalar field, one group, and one generator basis.
pub trait ProofSuite: Copy + Clone + Debug + Eq + Send + Sync + 'static {
    type Scalar: ProofScalar;
    type Point: ProofPoint<Scalar = Self::Scalar>;

    fn generators() -> &'static ProofGenerators<Self>;
}

/// Transcript writes required by the prover.
pub trait ProverTranscript<S: ProofSuite> {
    fn push_scalar(&mut self, scalar: S::Scalar) -> Result<(), GeneralizedBulletproofErrorV1>;
    fn push_point(&mut self, point: S::Point) -> Result<(), GeneralizedBulletproofErrorV1>;
    fn challenge(&mut self) -> Result<S::Scalar, GeneralizedBulletproofErrorV1>;
}

/// Transcript reads required by the verifier.
pub trait VerifierTranscript<S: ProofSuite> {
    fn read_scalar(&mut self) -> Result<S::Scalar, GeneralizedBulletproofErrorV1>;
    fn read_point(&mut self) -> Result<S::Point, GeneralizedBulletproofErrorV1>;
    fn challenge(&mut self) -> Result<S::Scalar, GeneralizedBulletproofErrorV1>;
}

/// Full generator basis for one proof suite.
#[derive(Clone, Debug)]
pub struct ProofGenerators<S: ProofSuite> {
    pub g: S::Point,
    pub h: S::Point,
    pub g_bold: Vec<S::Point>,
    pub h_bold: Vec<S::Point>,
    pub h_sum: Vec<S::Point>,
}

impl<S: ProofSuite> ProofGenerators<S> {
    /// Construct a checked basis and its power-of-two H-prefix sums.
    pub fn new(
        g: S::Point,
        h: S::Point,
        g_bold: Vec<S::Point>,
        h_bold: Vec<S::Point>,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        if g.is_identity() || h.is_identity() || g_bold.is_empty() || g_bold.len() != h_bold.len() {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let mut h_sum = Vec::new();
        let mut running = S::Point::identity();
        let mut next_power = 1_usize;
        for (index, point) in h_bold.iter().copied().enumerate() {
            running += point;
            if index + 1 == next_power {
                h_sum.push(running);
                next_power = next_power
                    .checked_mul(2)
                    .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
            }
        }
        Ok(Self {
            g,
            h,
            g_bold,
            h_bold,
            h_sum,
        })
    }

    pub fn reduce(
        &self,
        count: usize,
    ) -> Result<ProofGeneratorView<'_, S>, GeneralizedBulletproofErrorV1> {
        if count == 0
            || !count.is_power_of_two()
            || count > self.g_bold.len()
            || count > self.h_bold.len()
        {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        Ok(ProofGeneratorView {
            g: self.g,
            h: self.h,
            g_bold: &self.g_bold[..count],
            h_bold: &self.h_bold[..count],
        })
    }
}

/// Borrowed power-of-two generator prefix used by one proof.
#[derive(Clone, Copy, Debug)]
pub struct ProofGeneratorView<'a, S: ProofSuite> {
    pub g: S::Point,
    pub h: S::Point,
    pub g_bold: &'a [S::Point],
    pub h_bold: &'a [S::Point],
}

/// Deterministic variable-time multiscalar multiplication over public terms.
pub fn multiexp<S: ProofSuite>(terms: &[(S::Scalar, S::Point)]) -> S::Point {
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
    let windows = S::Scalar::SCALAR_BITS.div_ceil(window);
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
struct BatchVerifier<S: ProofSuite> {
    g: S::Scalar,
    h: S::Scalar,
    g_bold: Vec<S::Scalar>,
    h_bold: Vec<S::Scalar>,
    h_sum: Vec<S::Scalar>,
    additional: Vec<(S::Scalar, S::Point)>,
}

impl<S: ProofSuite> BatchVerifier<S> {
    fn new() -> Self {
        Self {
            g: S::Scalar::ZERO,
            h: S::Scalar::ZERO,
            g_bold: Vec::new(),
            h_bold: Vec::new(),
            h_sum: Vec::new(),
            additional: Vec::new(),
        }
    }

    fn ensure_len(&mut self, len: usize) {
        if self.g_bold.len() < len {
            self.g_bold.resize(len, S::Scalar::ZERO);
            self.h_bold.resize(len, S::Scalar::ZERO);
            self.h_sum.resize(len, S::Scalar::ZERO);
        }
    }

    fn verify(self) -> bool {
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

#[derive(Clone, PartialEq, Eq)]
pub struct ScalarVector<F: ProofScalar>(pub Vec<F>);

impl<F: ProofScalar> Drop for ScalarVector<F> {
    fn drop(&mut self) {
        for value in &mut self.0 {
            value.clear_secret();
        }
    }
}

impl<F: ProofScalar> Index<usize> for ScalarVector<F> {
    type Output = F;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<F: ProofScalar> IndexMut<usize> for ScalarVector<F> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl<F: ProofScalar> Add<F> for ScalarVector<F> {
    type Output = Self;
    fn add(mut self, scalar: F) -> Self {
        for value in &mut self.0 {
            *value += scalar;
        }
        self
    }
}

impl<F: ProofScalar> Sub<F> for ScalarVector<F> {
    type Output = Self;
    fn sub(mut self, scalar: F) -> Self {
        for value in &mut self.0 {
            *value -= scalar;
        }
        self
    }
}

impl<F: ProofScalar> Mul<F> for ScalarVector<F> {
    type Output = Self;
    fn mul(mut self, scalar: F) -> Self {
        for value in &mut self.0 {
            *value *= scalar;
        }
        self
    }
}

impl<F: ProofScalar> Add<&Self> for ScalarVector<F> {
    type Output = Self;
    fn add(mut self, other: &Self) -> Self {
        assert_eq!(self.len(), other.len());
        for (value, other) in self.0.iter_mut().zip(&other.0) {
            *value += *other;
        }
        self
    }
}

impl<F: ProofScalar> Sub<&Self> for ScalarVector<F> {
    type Output = Self;
    fn sub(mut self, other: &Self) -> Self {
        assert_eq!(self.len(), other.len());
        for (value, other) in self.0.iter_mut().zip(&other.0) {
            *value -= *other;
        }
        self
    }
}

impl<F: ProofScalar> Mul<&Self> for ScalarVector<F> {
    type Output = Self;
    fn mul(mut self, other: &Self) -> Self {
        assert_eq!(self.len(), other.len());
        for (value, other) in self.0.iter_mut().zip(&other.0) {
            *value *= *other;
        }
        self
    }
}

impl<F: ProofScalar> ScalarVector<F> {
    pub fn zero(len: usize) -> Self {
        Self(vec![F::ZERO; len])
    }

    pub fn powers(value: F, len: usize) -> Self {
        assert!(len != 0);
        let mut result = Vec::with_capacity(len);
        result.push(F::ONE);
        if len > 1 {
            result.push(value);
        }
        for index in 2..len {
            result.push(result[index - 1] * value);
        }
        Self(result)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn inner_product(&self, vector: impl Iterator<Item = F>) -> F {
        let mut count = 0;
        let mut result = F::ZERO;
        for (left, right) in self.0.iter().zip(vector) {
            result += *left * right;
            count += 1;
        }
        assert_eq!(count, self.len());
        result
    }

    fn split(mut self) -> Result<(Self, Self), GeneralizedBulletproofErrorV1> {
        if self.len() <= 1 || self.len() % 2 != 0 {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let right = self.0.split_off(self.len() / 2);
        Ok((self, Self(right)))
    }
}

/// Opening of one Pedersen vector commitment used by the FCMP circuit.
#[derive(Clone)]
pub struct VectorCommitmentOpening<F: ProofScalar> {
    pub values: ScalarVector<F>,
    pub mask: F,
}

impl<F: ProofScalar> Drop for VectorCommitmentOpening<F> {
    fn drop(&mut self) {
        for value in &mut self.values.0 {
            value.clear_secret();
        }
        self.mask.clear_secret();
    }
}

impl<F: ProofScalar> VectorCommitmentOpening<F> {
    pub fn new(values: Vec<F>, mask: F) -> Self {
        Self {
            values: ScalarVector(values),
            mask,
        }
    }
}

/// Witness for a concrete generalized-Bulletproof arithmetic circuit.
#[derive(Clone)]
pub struct ArithmeticCircuitWitness<S: ProofSuite> {
    a_l: ScalarVector<S::Scalar>,
    a_r: ScalarVector<S::Scalar>,
    a_o: ScalarVector<S::Scalar>,
    vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
}

impl<S: ProofSuite> ArithmeticCircuitWitness<S> {
    pub fn new(
        a_l: Vec<S::Scalar>,
        a_r: Vec<S::Scalar>,
        vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let a_l = ScalarVector(a_l);
        let a_r = ScalarVector(a_r);
        if a_l.len() != a_r.len() {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let a_o = a_l.clone() * &a_r;
        Ok(Self {
            a_l,
            a_r,
            a_o,
            vector_commitments,
        })
    }
}

/// One constrainable circuit variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Variable {
    aL(usize),
    aR(usize),
    aO(usize),
    CG {
        commitment: usize,
        index: usize,
    },
    #[cfg_attr(not(test), allow(dead_code))]
    V(usize),
}

/// Sparse generalized-Bulletproof linear combination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinComb<F: ProofScalar> {
    pub highest_a_index: Option<usize>,
    pub highest_c_index: Option<usize>,
    pub highest_v_index: Option<usize>,
    pub wl: Vec<(usize, F)>,
    pub wr: Vec<(usize, F)>,
    pub wo: Vec<(usize, F)>,
    pub wcg: Vec<Vec<(usize, F)>>,
    pub wv: Vec<(usize, F)>,
    pub c: F,
}

impl<F: ProofScalar> From<Variable> for LinComb<F> {
    fn from(variable: Variable) -> Self {
        Self::empty().term(F::ONE, variable)
    }
}

impl<F: ProofScalar> Add<&Self> for LinComb<F> {
    type Output = Self;
    fn add(mut self, other: &Self) -> Self {
        self.highest_a_index = self.highest_a_index.max(other.highest_a_index);
        self.highest_c_index = self.highest_c_index.max(other.highest_c_index);
        self.highest_v_index = self.highest_v_index.max(other.highest_v_index);
        self.wl.extend(&other.wl);
        self.wr.extend(&other.wr);
        self.wo.extend(&other.wo);
        self.wcg
            .resize_with(self.wcg.len().max(other.wcg.len()), Vec::new);
        for (ours, theirs) in self.wcg.iter_mut().zip(&other.wcg) {
            ours.extend(theirs);
        }
        self.wv.extend(&other.wv);
        self.c += other.c;
        self
    }
}

impl<F: ProofScalar> Sub<&Self> for LinComb<F> {
    type Output = Self;
    fn sub(mut self, other: &Self) -> Self {
        self.highest_a_index = self.highest_a_index.max(other.highest_a_index);
        self.highest_c_index = self.highest_c_index.max(other.highest_c_index);
        self.highest_v_index = self.highest_v_index.max(other.highest_v_index);
        self.wl
            .extend(other.wl.iter().map(|(index, weight)| (*index, -*weight)));
        self.wr
            .extend(other.wr.iter().map(|(index, weight)| (*index, -*weight)));
        self.wo
            .extend(other.wo.iter().map(|(index, weight)| (*index, -*weight)));
        self.wcg
            .resize_with(self.wcg.len().max(other.wcg.len()), Vec::new);
        for (ours, theirs) in self.wcg.iter_mut().zip(&other.wcg) {
            ours.extend(theirs.iter().map(|(index, weight)| (*index, -*weight)));
        }
        self.wv
            .extend(other.wv.iter().map(|(index, weight)| (*index, -*weight)));
        self.c -= other.c;
        self
    }
}

impl<F: ProofScalar> Mul<F> for LinComb<F> {
    type Output = Self;
    fn mul(mut self, scalar: F) -> Self {
        for (_, weight) in &mut self.wl {
            *weight *= scalar;
        }
        for (_, weight) in &mut self.wr {
            *weight *= scalar;
        }
        for (_, weight) in &mut self.wo {
            *weight *= scalar;
        }
        for commitment in &mut self.wcg {
            for (_, weight) in commitment {
                *weight *= scalar;
            }
        }
        for (_, weight) in &mut self.wv {
            *weight *= scalar;
        }
        self.c *= scalar;
        self
    }
}

impl<F: ProofScalar> LinComb<F> {
    pub fn empty() -> Self {
        Self {
            highest_a_index: None,
            highest_c_index: None,
            highest_v_index: None,
            wl: Vec::new(),
            wr: Vec::new(),
            wo: Vec::new(),
            wcg: Vec::new(),
            wv: Vec::new(),
            c: F::ZERO,
        }
    }

    pub fn term(mut self, scalar: F, variable: Variable) -> Self {
        match variable {
            Variable::aL(index) => {
                self.highest_a_index = self.highest_a_index.max(Some(index));
                self.wl.push((index, scalar));
            }
            Variable::aR(index) => {
                self.highest_a_index = self.highest_a_index.max(Some(index));
                self.wr.push((index, scalar));
            }
            Variable::aO(index) => {
                self.highest_a_index = self.highest_a_index.max(Some(index));
                self.wo.push((index, scalar));
            }
            Variable::CG { commitment, index } => {
                self.highest_c_index = self.highest_c_index.max(Some(commitment));
                self.highest_a_index = self.highest_a_index.max(Some(index));
                if self.wcg.len() <= commitment {
                    self.wcg.resize_with(commitment + 1, Vec::new);
                }
                self.wcg[commitment].push((index, scalar));
            }
            Variable::V(index) => {
                self.highest_v_index = self.highest_v_index.max(Some(index));
                self.wv.push((index, scalar));
            }
        }
        self
    }

    pub fn constant(mut self, scalar: F) -> Self {
        self.c += scalar;
        self
    }
}

fn accumulate<F: ProofScalar>(accumulator: &mut ScalarVector<F>, values: &[(usize, F)], weight: F) {
    for (index, coefficient) in values {
        accumulator[*index] += *coefficient * weight;
    }
}

#[derive(Clone, Debug)]
pub struct ArithmeticCircuitStatement<'a, S: ProofSuite> {
    generators: ProofGeneratorView<'a, S>,
    constraints: Vec<LinComb<S::Scalar>>,
    vector_commitments: Vec<S::Point>,
    scalar_commitments: Vec<S::Point>,
}

impl<'a, S: ProofSuite> ArithmeticCircuitStatement<'a, S> {
    pub fn new(
        generators: ProofGeneratorView<'a, S>,
        constraints: Vec<LinComb<S::Scalar>>,
        vector_commitments: Vec<S::Point>,
        scalar_commitments: Vec<S::Point>,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        for constraint in &constraints {
            if Some(generators.g_bold.len()) <= constraint.highest_a_index
                || Some(vector_commitments.len()) <= constraint.highest_c_index
                || Some(scalar_commitments.len()) <= constraint.highest_v_index
            {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
        }
        Ok(Self {
            generators,
            constraints,
            vector_commitments,
            scalar_commitments,
        })
    }

    fn yz_challenges(
        &self,
        y: S::Scalar,
        z_one: S::Scalar,
    ) -> Result<(ScalarVector<S::Scalar>, ScalarVector<S::Scalar>), GeneralizedBulletproofErrorV1>
    {
        let y_inverse = y
            .invert()
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        let y_inverse = ScalarVector::powers(y_inverse, self.generators.g_bold.len());
        let mut z = Vec::with_capacity(self.constraints.len());
        if !self.constraints.is_empty() {
            z.push(z_one);
            for index in 1..self.constraints.len() {
                z.push(z[index - 1] * z_one);
            }
        }
        Ok((y_inverse, ScalarVector(z)))
    }

    pub fn prove<R, T>(
        self,
        rng: &mut R,
        transcript: &mut T,
        mut witness: ArithmeticCircuitWitness<S>,
    ) -> Result<(), GeneralizedBulletproofErrorV1>
    where
        R: ProofRandomSource,
        T: ProverTranscript<S>,
    {
        let n = self.generators.g_bold.len();
        let commitment_count = self.vector_commitments.len();
        if witness.a_l.len() > n
            || witness.a_l.len() != witness.a_r.len()
            || witness.vector_commitments.len() != commitment_count
            || !self.scalar_commitments.is_empty()
        {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        while witness.a_l.len() < n {
            witness.a_l.0.push(S::Scalar::ZERO);
            witness.a_r.0.push(S::Scalar::ZERO);
            witness.a_o.0.push(S::Scalar::ZERO);
        }
        for opening in &mut witness.vector_commitments {
            if opening.values.len() > n {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
            opening.values.0.resize(n, S::Scalar::ZERO);
        }

        // Validate every opening and every circuit constraint before emitting
        // any proof bytes. A malformed native witness is an API error, never a
        // source of a knowingly-invalid proof.
        for (commitment, opening) in self
            .vector_commitments
            .iter()
            .zip(&witness.vector_commitments)
        {
            let mut terms = opening
                .values
                .0
                .iter()
                .copied()
                .zip(self.generators.g_bold.iter().copied())
                .collect::<Vec<_>>();
            terms.push((opening.mask, self.generators.h));
            if multiexp::<S>(&terms) != *commitment {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
        }
        for constraint in &self.constraints {
            let mut evaluation = constraint.c;
            for (index, weight) in &constraint.wl {
                evaluation += witness.a_l[*index] * *weight;
            }
            for (index, weight) in &constraint.wr {
                evaluation += witness.a_r[*index] * *weight;
            }
            for (index, weight) in &constraint.wo {
                evaluation += witness.a_o[*index] * *weight;
            }
            for (commitment, weights) in constraint.wcg.iter().enumerate() {
                for (index, weight) in weights {
                    evaluation += witness.vector_commitments[commitment].values[*index] * *weight;
                }
            }
            if !constraint.wv.is_empty() || !evaluation.is_zero() {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
        }

        let alpha = random_scalar::<S::Scalar, _>(rng)?;
        let beta = random_scalar::<S::Scalar, _>(rng)?;
        let rho = random_scalar::<S::Scalar, _>(rng)?;
        let ai = {
            let mut terms = Vec::with_capacity((2 * n) + 1);
            terms.extend(
                witness
                    .a_l
                    .0
                    .iter()
                    .copied()
                    .zip(self.generators.g_bold.iter().copied()),
            );
            terms.extend(
                witness
                    .a_r
                    .0
                    .iter()
                    .copied()
                    .zip(self.generators.h_bold.iter().copied()),
            );
            terms.push((alpha, self.generators.h));
            multiexp::<S>(&terms)
        };
        let ao = {
            let mut terms = witness
                .a_o
                .0
                .iter()
                .copied()
                .zip(self.generators.g_bold.iter().copied())
                .collect::<Vec<_>>();
            terms.push((beta, self.generators.h));
            multiexp::<S>(&terms)
        };
        let s_l = ScalarVector(
            (0..n)
                .map(|_| random_scalar::<S::Scalar, _>(rng))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let s_r = ScalarVector(
            (0..n)
                .map(|_| random_scalar::<S::Scalar, _>(rng))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let s_point = {
            let mut terms = Vec::with_capacity((2 * n) + 1);
            terms.extend(
                s_l.0
                    .iter()
                    .copied()
                    .zip(self.generators.g_bold.iter().copied()),
            );
            terms.extend(
                s_r.0
                    .iter()
                    .copied()
                    .zip(self.generators.h_bold.iter().copied()),
            );
            terms.push((rho, self.generators.h));
            multiexp::<S>(&terms)
        };
        if ai.is_identity() || ao.is_identity() || s_point.is_identity() {
            return Err(GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity);
        }
        transcript.push_point(ai)?;
        transcript.push_point(ao)?;
        transcript.push_point(s_point)?;
        let y = transcript.challenge()?;
        let z_one = transcript.challenge()?;
        let (y_inverse, z) = self.yz_challenges(y, z_one)?;
        let y_powers = ScalarVector::powers(y, n);

        let ni = 2 + (2 * (commitment_count / 2));
        let ilr = ni / 2;
        let io = ni;
        let is = ni + 1;
        let jlr = ni / 2;
        let jo = 0;
        let js = ni + 1;
        let mut l = vec![ScalarVector(Vec::new()); is + 1];
        let mut r = vec![ScalarVector(Vec::new()); is + 1];

        let mut l_weights = ScalarVector::zero(n);
        let mut r_weights = ScalarVector::zero(n);
        let mut o_weights = ScalarVector::zero(n);
        for (constraint, z) in self.constraints.iter().zip(&z.0) {
            accumulate(&mut l_weights, &constraint.wl, *z);
            accumulate(&mut r_weights, &constraint.wr, *z);
            accumulate(&mut o_weights, &constraint.wo, *z);
        }
        l[ilr] = (r_weights * &y_inverse) + &witness.a_l;
        l[io] = witness.a_o.clone();
        l[is] = s_l;
        r[jlr] = l_weights + &(witness.a_r.clone() * &y_powers);
        r[jo] = o_weights - &y_powers;
        r[js] = s_r * &y_powers;
        for coefficient in &mut l {
            if coefficient.0.is_empty() {
                *coefficient = ScalarVector::zero(n);
            } else if coefficient.len() != n {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
        }
        for coefficient in &mut r {
            if coefficient.0.is_empty() {
                *coefficient = ScalarVector::zero(n);
            } else if coefficient.len() != n {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
        }

        let mut cg_weights = Vec::with_capacity(commitment_count);
        for commitment in 0..commitment_count {
            let mut weights = ScalarVector::zero(n);
            for (constraint, z) in self.constraints.iter().zip(&z.0) {
                if let Some(values) = constraint.wcg.get(commitment) {
                    accumulate(&mut weights, values, *z);
                }
            }
            cg_weights.push(weights);
        }
        for (mut index, (opening, weights)) in witness
            .vector_commitments
            .iter()
            .zip(cg_weights)
            .enumerate()
        {
            if index >= ilr {
                index += 1;
            }
            let reverse = ni - index;
            l[index] = opening.values.clone();
            r[reverse] = weights;
        }

        let t_poly_len = 1 + (2 * (l.len() - 1));
        let mut t = ScalarVector::zero(t_poly_len);
        for (left_index, left) in l.iter().enumerate() {
            for (right_index, right) in r.iter().enumerate() {
                t[left_index + right_index] += left.inner_product(right.0.iter().copied());
            }
        }
        let tau_before = (0..ni)
            .map(|_| random_scalar::<S::Scalar, _>(rng))
            .collect::<Result<Vec<_>, _>>()?;
        let tau_after = (0..t_poly_len - ni - 1)
            .map(|_| random_scalar::<S::Scalar, _>(rng))
            .collect::<Result<Vec<_>, _>>()?;
        for (coefficient, mask) in t.0[..ni].iter().zip(&tau_before) {
            let commitment = multiexp::<S>(&[
                (*coefficient, self.generators.g),
                (*mask, self.generators.h),
            ]);
            if commitment.is_identity() {
                return Err(GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity);
            }
            transcript.push_point(commitment)?;
        }
        for (coefficient, mask) in t.0[ni + 1..].iter().zip(&tau_after) {
            let commitment = multiexp::<S>(&[
                (*coefficient, self.generators.g),
                (*mask, self.generators.h),
            ]);
            if commitment.is_identity() {
                return Err(GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity);
            }
            transcript.push_point(commitment)?;
        }
        let x = ScalarVector::powers(transcript.challenge()?, t_poly_len);
        let evaluate = |polynomial: &[ScalarVector<S::Scalar>]| {
            let mut result = ScalarVector::zero(n);
            for (index, coefficient) in polynomial.iter().enumerate() {
                result = result + &(coefficient.clone() * x[index]);
            }
            result
        };
        let l_eval = evaluate(&l);
        let r_eval = evaluate(&r);
        let t_caret = l_eval.inner_product(r_eval.0.iter().copied());

        // FCMP does not use scalar commitments, so the omitted t[ni] mask is
        // zero. Vector-commitment masks instead contribute to `u` below.
        let mut tau_x_poly = tau_before;
        tau_x_poly.push(S::Scalar::ZERO);
        tau_x_poly.extend(tau_after);
        let tau_x = tau_x_poly
            .into_iter()
            .enumerate()
            .fold(S::Scalar::ZERO, |sum, (index, coefficient)| {
                sum + (coefficient * x[index])
            });
        let mut u = (alpha * x[ilr]) + (beta * x[io]) + (rho * x[is]);
        for (mut index, opening) in witness.vector_commitments.iter().enumerate() {
            if index >= ilr {
                index += 1;
            }
            u += x[index] * opening.mask;
        }

        let mut p_terms = Vec::with_capacity(1 + (2 * n));
        for (index, (left, right)) in l_eval.0.iter().zip(&r_eval.0).enumerate() {
            p_terms.push((*left, self.generators.g_bold[index]));
            p_terms.push((y_inverse[index] * *right, self.generators.h_bold[index]));
        }
        transcript.push_scalar(tau_x)?;
        transcript.push_scalar(u)?;
        transcript.push_scalar(t_caret)?;
        let ip_x = transcript.challenge()?;
        p_terms.push((ip_x * t_caret, self.generators.g));
        let p = multiexp::<S>(&p_terms);
        prove_inner_product::<S, _>(
            self.generators,
            y_inverse,
            ip_x,
            p,
            l_eval,
            r_eval,
            transcript,
        )
    }

    pub fn verify<T>(self, transcript: &mut T) -> Result<(), GeneralizedBulletproofErrorV1>
    where
        T: VerifierTranscript<S>,
    {
        let n = self.generators.g_bold.len();
        let commitment_count = self.vector_commitments.len();
        let ni = 2 + (2 * (commitment_count / 2));
        let ilr = ni / 2;
        let io = ni;
        let is = ni + 1;
        let jlr = ni / 2;
        let l_r_poly_len = ni + 2;
        let t_poly_len = (2 * l_r_poly_len) - 1;

        let ai = transcript.read_point()?;
        let ao = transcript.read_point()?;
        let s_point = transcript.read_point()?;
        let y = transcript.challenge()?;
        let z_one = transcript.challenge()?;
        let (y_inverse, z) = self.yz_challenges(y, z_one)?;

        let mut l_weights = ScalarVector::zero(n);
        let mut r_weights = ScalarVector::zero(n);
        let mut o_weights = ScalarVector::zero(n);
        for (constraint, z) in self.constraints.iter().zip(&z.0) {
            accumulate(&mut l_weights, &constraint.wl, *z);
            accumulate(&mut r_weights, &constraint.wr, *z);
            accumulate(&mut o_weights, &constraint.wo, *z);
        }
        let r_weights = r_weights * &y_inverse;
        let delta = r_weights.inner_product(l_weights.0.iter().copied());

        let mut t_before = Vec::with_capacity(ni);
        for _ in 0..ni {
            t_before.push(transcript.read_point()?);
        }
        let mut t_after = Vec::with_capacity(t_poly_len - ni - 1);
        for _ in 0..(t_poly_len - ni - 1) {
            t_after.push(transcript.read_point()?);
        }
        let x = ScalarVector::powers(transcript.challenge()?, t_poly_len);
        let tau_x = transcript.read_scalar()?;
        let u = transcript.read_scalar()?;
        let t_caret = transcript.read_scalar()?;

        // Check the polynomial commitment equation independently.
        let mut polynomial = BatchVerifier::<S>::new();
        polynomial.g += t_caret;
        polynomial.h += tau_x;
        let mut v_weights = ScalarVector::zero(self.scalar_commitments.len());
        for (constraint, z) in self.constraints.iter().zip(&z.0) {
            accumulate(&mut v_weights, &constraint.wv, -*z);
        }
        v_weights = v_weights * x[ni];
        polynomial.g -= x[ni]
            * (delta - z.inner_product(self.constraints.iter().map(|constraint| constraint.c)));
        polynomial.additional.extend(
            v_weights
                .0
                .iter()
                .copied()
                .zip(self.scalar_commitments.iter().copied())
                .map(|(weight, point)| (-weight, point)),
        );
        polynomial.additional.extend(
            t_before
                .into_iter()
                .enumerate()
                .map(|(index, point)| (-x[index], point)),
        );
        polynomial.additional.extend(
            t_after
                .into_iter()
                .enumerate()
                .map(|(index, point)| (-x[ni + 1 + index], point)),
        );
        if !polynomial.verify() {
            return Err(GeneralizedBulletproofErrorV1::CircuitEquation);
        }

        // Build P and verify the inner-product equation independently.
        let mut ipa = BatchVerifier::<S>::new();
        ipa.ensure_len(n);
        ipa.additional.push((x[ilr], ai));
        ipa.additional.push((x[io], ao));
        let log_n = n.trailing_zeros() as usize;
        if !n.is_power_of_two() || log_n >= ipa.h_sum.len() {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        ipa.h_sum[log_n] -= S::Scalar::ONE;
        ipa.additional.push((x[is], s_point));

        let mut h_bold_scalars = l_weights * x[jlr];
        for (index, weight) in (r_weights * x[jlr]).0.iter().copied().enumerate() {
            ipa.g_bold[index] += weight;
        }
        h_bold_scalars = h_bold_scalars + &(o_weights * S::Scalar::ONE);

        let mut cg_weights = Vec::with_capacity(self.vector_commitments.len());
        for commitment in 0..self.vector_commitments.len() {
            let mut weights = ScalarVector::zero(n);
            for (constraint, z) in self.constraints.iter().zip(&z.0) {
                if let Some(values) = constraint.wcg.get(commitment) {
                    accumulate(&mut weights, values, *z);
                }
            }
            cg_weights.push(weights);
        }
        for (mut index, (commitment, weights)) in self
            .vector_commitments
            .iter()
            .copied()
            .zip(cg_weights)
            .enumerate()
        {
            if index >= ni / 2 {
                index += 1;
            }
            let reverse = ni - index;
            ipa.additional.push((x[index], commitment));
            h_bold_scalars = h_bold_scalars + &(weights * x[reverse]);
        }
        h_bold_scalars = h_bold_scalars * &y_inverse;
        for (index, scalar) in h_bold_scalars.0.iter().copied().enumerate() {
            ipa.h_bold[index] += scalar;
        }
        ipa.h -= u;

        let ip_x = transcript.challenge()?;
        ipa.g += ip_x * t_caret;
        verify_inner_product::<S, _>(self.generators, y_inverse, ip_x, &mut ipa, transcript)?;
        if !ipa.verify() {
            return Err(GeneralizedBulletproofErrorV1::CircuitEquation);
        }
        Ok(())
    }
}

fn prove_inner_product<S, T>(
    generators: ProofGeneratorView<'_, S>,
    h_bold_weights: ScalarVector<S::Scalar>,
    u_scalar: S::Scalar,
    mut p: S::Point,
    mut a: ScalarVector<S::Scalar>,
    mut b: ScalarVector<S::Scalar>,
    transcript: &mut T,
) -> Result<(), GeneralizedBulletproofErrorV1>
where
    S: ProofSuite,
    T: ProverTranscript<S>,
{
    let n = generators.g_bold.len();
    if n == 0
        || !n.is_power_of_two()
        || generators.h_bold.len() != n
        || h_bold_weights.len() != n
        || a.len() != n
        || b.len() != n
    {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }

    let mut g_bold = generators.g_bold.to_vec();
    let mut h_bold = generators
        .h_bold
        .iter()
        .copied()
        .zip(h_bold_weights.0.iter().copied())
        .map(|(point, weight)| point.scale(weight))
        .collect::<Vec<_>>();
    let u = generators.g.scale(u_scalar);

    // The inner-product protocol itself does not otherwise use `P` while
    // proving. Check the opening unconditionally so release builds cannot
    // serialize a proof for an inconsistent caller-supplied statement.
    let mut opening_terms = Vec::with_capacity((2 * n) + 1);
    opening_terms.extend(a.0.iter().copied().zip(g_bold.iter().copied()));
    opening_terms.extend(b.0.iter().copied().zip(h_bold.iter().copied()));
    opening_terms.push((a.inner_product(b.0.iter().copied()), u));
    if multiexp::<S>(&opening_terms) != p {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }

    while g_bold.len() > 1 {
        let (a_left, a_right) = a.split()?;
        let (b_left, b_right) = b.split()?;
        let half = g_bold.len() / 2;
        let g_right = g_bold.split_off(half);
        let g_left = g_bold;
        let h_right = h_bold.split_off(half);
        let h_left = h_bold;
        if a_left.len() != half
            || a_right.len() != half
            || b_left.len() != half
            || b_right.len() != half
            || h_left.len() != half
            || h_right.len() != half
        {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }

        let c_left = a_left.inner_product(b_right.0.iter().copied());
        let c_right = a_right.inner_product(b_left.0.iter().copied());
        let left = {
            let mut terms = Vec::with_capacity((2 * half) + 1);
            terms.extend(a_left.0.iter().copied().zip(g_right.iter().copied()));
            terms.extend(b_right.0.iter().copied().zip(h_left.iter().copied()));
            terms.push((c_left, u));
            multiexp::<S>(&terms)
        };
        let right = {
            let mut terms = Vec::with_capacity((2 * half) + 1);
            terms.extend(a_right.0.iter().copied().zip(g_left.iter().copied()));
            terms.extend(b_left.0.iter().copied().zip(h_right.iter().copied()));
            terms.push((c_right, u));
            multiexp::<S>(&terms)
        };
        if left.is_identity() || right.is_identity() {
            return Err(GeneralizedBulletproofErrorV1::InnerProductRoundIdentity);
        }

        transcript.push_point(left)?;
        transcript.push_point(right)?;
        let challenge = transcript.challenge()?;
        let inverse = challenge
            .invert()
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;

        g_bold = g_left
            .into_iter()
            .zip(g_right)
            .map(|(left, right)| multiexp::<S>(&[(inverse, left), (challenge, right)]))
            .collect();
        h_bold = h_left
            .into_iter()
            .zip(h_right)
            .map(|(left, right)| multiexp::<S>(&[(challenge, left), (inverse, right)]))
            .collect();
        p = left.scale(challenge.square()) + p + right.scale(inverse.square());
        a = (a_left * challenge) + &(a_right * inverse);
        b = (b_left * inverse) + &(b_right * challenge);
    }

    if g_bold.len() != 1 || h_bold.len() != 1 || a.len() != 1 || b.len() != 1 {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }
    let folded = multiexp::<S>(&[(a[0], g_bold[0]), (b[0], h_bold[0]), (a[0] * b[0], u)]);
    if folded != p {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }
    transcript.push_scalar(a[0])?;
    transcript.push_scalar(b[0])?;
    Ok(())
}

fn challenge_products<F: ProofScalar>(challenges: &[(F, F)]) -> Vec<F> {
    let mut products = vec![F::ONE; 1 << challenges.len()];
    if !challenges.is_empty() {
        products[0] = challenges[0].1;
        products[1] = challenges[0].0;
        for (column, challenge) in challenges.iter().enumerate().skip(1) {
            let mut slots = (1 << (column + 1)) - 1;
            while slots > 0 {
                products[slots] = products[slots / 2] * challenge.0;
                products[slots - 1] = products[slots / 2] * challenge.1;
                slots = slots.saturating_sub(2);
            }
        }
    }
    products
}

fn verify_inner_product<S, T>(
    generators: ProofGeneratorView<'_, S>,
    h_bold_weights: ScalarVector<S::Scalar>,
    u: S::Scalar,
    verifier: &mut BatchVerifier<S>,
    transcript: &mut T,
) -> Result<(), GeneralizedBulletproofErrorV1>
where
    S: ProofSuite,
    T: VerifierTranscript<S>,
{
    if generators.h_bold.len() != h_bold_weights.len() {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }
    verifier.ensure_len(generators.g_bold.len());
    let mut lr_len = 0;
    while (1 << lr_len) < generators.g_bold.len() {
        lr_len += 1;
    }
    let mut challenges = Vec::with_capacity(lr_len);
    for _ in 0..lr_len {
        let left = transcript.read_point()?;
        let right = transcript.read_point()?;
        let x = transcript.challenge()?;
        let inverse = x
            .invert()
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        verifier.additional.push((x.square(), left));
        verifier.additional.push((inverse.square(), right));
        challenges.push((x, inverse));
    }
    let products = challenge_products(&challenges);
    let a = transcript.read_scalar()?;
    let b = transcript.read_scalar()?;
    for index in 0..generators.g_bold.len() {
        verifier.g_bold[index] -= products[index] * a;
        verifier.h_bold[index] -= products[products.len() - 1 - index] * b * h_bold_weights[index];
    }
    verifier.g -= a * b * u;
    Ok(())
}
