//! Reusable generalized-Bulletproof arithmetic and inner-product backend.
//!
//! This module owns the curve-agnostic proof equations. Transcript codecs,
//! concrete curves, generator derivation domains, and entropy providers remain
//! explicit adapters so a protocol can freeze its own consensus bytes.

use std::{
    fmt::Debug,
    ops::{Add, AddAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign},
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;
use thiserror::Error;

/// Stable failure classes emitted by the generalized-Bulletproof backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum GeneralizedBulletproofErrorV1 {
    /// A checked scalar, point, vector, or circuit invariant did not hold.
    #[error("generalized-Bulletproof arithmetic invariant failed")]
    ArithmeticInvariant,
    /// Canonical scalar sampling exhausted its public retry limit.
    #[error("generalized-Bulletproof prover randomness exhausted its retry bound")]
    ProverRandomnessExhausted,
    /// The configured entropy source could not fill a requested buffer.
    #[error("generalized-Bulletproof randomness source is unavailable")]
    RandomnessUnavailable,
    /// Transcript challenge derivation exhausted its public retry limit.
    #[error("generalized-Bulletproof transcript challenge exhausted its retry bound")]
    TranscriptChallengeExhausted,
    /// A compressed point failed the suite's canonical decoder.
    #[error("generalized-Bulletproof point encoding is invalid")]
    PointEncoding,
    /// A protocol point was the identity where a non-identity point is required.
    #[error("generalized-Bulletproof point must be non-identity")]
    PointIdentity,
    /// A scalar byte string was not a canonical field encoding.
    #[error("generalized-Bulletproof scalar encoding is non-canonical")]
    ScalarEncoding,
    /// A prover-generated circuit commitment was the identity.
    #[error("generalized-Bulletproof prover commitment was the identity")]
    CircuitProverCommitmentIdentity,
    /// An inner-product round produced an inadmissible identity point.
    #[error("generalized-Bulletproof inner-product round produced an identity")]
    InnerProductRoundIdentity,
    /// A verifier equation did not evaluate to the group identity.
    #[error("generalized-Bulletproof verification equation failed")]
    CircuitEquation,
    /// A proof payload did not contain the exact number of bytes required.
    #[error("generalized-Bulletproof proof length {actual} does not equal {expected}")]
    ProofLength {
        /// Number of proof bytes supplied by the caller.
        actual: usize,
        /// Number of proof bytes required by the statement.
        expected: usize,
    },
    /// A checked resource-size computation overflowed.
    #[error("generalized-Bulletproof resource bound overflowed")]
    ResourceOverflow,
    /// Verification left unread transcript data or requested data past its end.
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

/// Best-effort clearing for secret byte encodings without introducing unsafe
/// code into this crate.
///
/// The volatile implementation normally supplied by a zeroization crate is
/// unavailable at this abstraction boundary. Passing the destination through
/// `black_box` before and after the overwrite, followed by a compiler fence,
/// prevents ordinary dead-store elimination while preserving this crate's
/// `deny(unsafe_code)` contract.
fn clear_secret_bytes(bytes: &mut [u8]) {
    let bytes = core::hint::black_box(bytes);
    bytes.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *bytes);
    #[cfg(test)]
    SECRET_BYTE_CLEAR_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
static SECRET_BYTE_CLEAR_CALLS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Fixed-size secret byte buffer cleared on normal return and unwind.
struct SecretBytes<const N: usize>([u8; N]);

impl<const N: usize> Drop for SecretBytes<N> {
    fn drop(&mut self) {
        clear_secret_bytes(&mut self.0);
    }
}

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
            let scalar = SecretScalar::new(scalar);
            return Ok(scalar.expose_copy());
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
    /// Additive identity of the scalar field.
    const ZERO: Self;
    /// Multiplicative identity of the scalar field.
    const ONE: Self;
    /// Number of significant little-endian scalar bits used by MSM.
    const SCALAR_BITS: usize;

    /// Convert a small unsigned integer into this scalar field.
    fn from_u64(value: u64) -> Self;
    /// Decode a canonical 32-byte scalar, rejecting non-canonical encodings.
    fn decode(bytes: [u8; 32]) -> Option<Self>;
    /// Encode this scalar canonically as 32 bytes.
    fn encode(self) -> [u8; 32];
    /// Reduce a 64-byte little-endian integer into this scalar field.
    fn reduce_wide(bytes: [u8; 64]) -> Self;
    /// Return the multiplicative inverse, or `None` for zero.
    fn invert(self) -> Option<Self>;
    /// Return a square root when one exists in the scalar field.
    fn sqrt(self) -> Option<Self>;
    /// Square this scalar.
    fn square(self) -> Self;
    /// Double this scalar.
    fn double(self) -> Self;
    /// Return whether this scalar is zero.
    fn is_zero(self) -> bool;
    /// Return the canonical parity bit of this scalar.
    fn is_odd(self) -> bool;
    /// Clear the complete scalar value without panicking.
    ///
    /// Implementations must be idempotent, overwrite every field of the value,
    /// and must not allocate or panic: this method is called from `Drop`,
    /// including during unwinding, and nested owners may clear the same slot
    /// before its field destructor does. This is best-effort erasure. Rust does
    /// not promise that compiler-created copies or register temporaries are
    /// erased, and no destructor runs if the process aborts.
    fn clear_secret(&mut self);

    /// Return the canonical little-endian bytes consumed by MSM bit extraction.
    fn bits_le(self) -> [u8; 32] {
        self.encode()
    }

    /// Sample one canonical scalar from the supplied entropy source.
    fn random(
        rng: &mut impl ProofRandomSource,
    ) -> Result<Option<Self>, GeneralizedBulletproofErrorV1> {
        let mut bytes = SecretBytes([0_u8; 32]);
        if rng.fill_bytes(&mut bytes.0).is_err() {
            return Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable);
        }
        Ok(Self::decode(bytes.0))
    }
}

/// One owned secret scalar whose storage is cleared on every exit path.
///
/// `ProofScalar` is necessarily `Copy`, so arithmetic can still create
/// compiler temporaries and register copies. This guard covers the named stack
/// slot owned by the prover and makes every intentional copy explicit.
struct SecretScalar<F: ProofScalar>(F);

impl<F: ProofScalar> SecretScalar<F> {
    fn new(value: F) -> Self {
        Self(value)
    }

    fn expose_copy(&self) -> F {
        self.0
    }

    fn expose_mut(&mut self) -> &mut F {
        &mut self.0
    }
}

impl<F: ProofScalar> Drop for SecretScalar<F> {
    fn drop(&mut self) {
        self.0.clear_secret();
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
    /// Scalar field used to multiply points in this group.
    type Scalar: ProofScalar;
    /// Canonical fixed-width compressed point encoding.
    type Encoded: AsRef<[u8]> + Copy + Clone + Debug + Eq + Send + Sync + 'static;
    /// Exact number of bytes in one canonical point encoding.
    const POINT_BYTES: usize;

    /// Return the group identity.
    fn identity() -> Self;
    /// Return whether this point is the group identity.
    fn is_identity(self) -> bool;
    /// Double this point.
    fn double(self) -> Self;
    /// Multiply this point by a scalar.
    fn scale(self, scalar: Self::Scalar) -> Self;
    /// Select `a` for `choice == 0` and `b` for `choice == 1` in constant time.
    ///
    /// The caller always supplies exactly zero or one. Implementations must
    /// not branch on `choice` or use it for a secret-indexed memory access.
    fn conditional_select(a: &Self, b: &Self, choice: u8) -> Self;
    /// Clear the complete point value without panicking.
    ///
    /// As with [`ProofScalar::clear_secret`], this must overwrite the full
    /// named value, must be idempotent, must not allocate or panic, cannot
    /// erase compiler-created copies, and is not run after an abort.
    fn clear_secret(&mut self);
    /// Encode this point canonically.
    fn encode(self) -> Self::Encoded;
    /// Decode one canonical point, optionally permitting the identity.
    fn decode(
        bytes: impl AsRef<[u8]>,
        allow_identity: bool,
    ) -> Result<Self, GeneralizedBulletproofErrorV1>;
}

/// Curve suite binding one scalar field, one group, and one generator basis.
pub trait ProofSuite: Copy + Clone + Debug + Eq + Send + Sync + 'static {
    /// Scalar field used by the suite.
    type Scalar: ProofScalar;
    /// Prime-order group used by the suite.
    type Point: ProofPoint<Scalar = Self::Scalar>;

    /// Return the suite's immutable generator basis.
    fn generators() -> &'static ProofGenerators<Self>;
}

/// Transcript writes required by the prover.
pub trait ProverTranscript<S: ProofSuite> {
    /// Append one canonical scalar to the proof transcript.
    fn push_scalar(&mut self, scalar: S::Scalar) -> Result<(), GeneralizedBulletproofErrorV1>;
    /// Append one canonical non-identity point to the proof transcript.
    fn push_point(&mut self, point: S::Point) -> Result<(), GeneralizedBulletproofErrorV1>;
    /// Derive the next non-zero Fiat-Shamir challenge.
    fn challenge(&mut self) -> Result<S::Scalar, GeneralizedBulletproofErrorV1>;
}

/// Transcript reads required by the verifier.
pub trait VerifierTranscript<S: ProofSuite> {
    /// Read the next canonical scalar from the proof transcript.
    fn read_scalar(&mut self) -> Result<S::Scalar, GeneralizedBulletproofErrorV1>;
    /// Read the next canonical non-identity point from the proof transcript.
    fn read_point(&mut self) -> Result<S::Point, GeneralizedBulletproofErrorV1>;
    /// Derive the next non-zero Fiat-Shamir challenge.
    fn challenge(&mut self) -> Result<S::Scalar, GeneralizedBulletproofErrorV1>;
}

/// Full generator basis for one proof suite.
#[derive(Clone, Debug)]
pub struct ProofGenerators<S: ProofSuite> {
    /// Base generator used for value and inner-product terms.
    pub g: S::Point,
    /// Blinding generator used by Pedersen commitments.
    pub h: S::Point,
    /// Left generator vector at the suite's maximum supported width.
    pub g_bold: Vec<S::Point>,
    /// Right generator vector at the suite's maximum supported width.
    pub h_bold: Vec<S::Point>,
    /// Power-of-two prefix sums of `h_bold` used by batched verification.
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
        if g.is_identity()
            || h.is_identity()
            || g_bold.is_empty()
            || g_bold.len() != h_bold.len()
            || !(1..=256).contains(&S::Scalar::SCALAR_BITS)
            || g_bold.iter().copied().any(S::Point::is_identity)
            || h_bold.iter().copied().any(S::Point::is_identity)
        {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let mut h_sum = Vec::new();
        let mut running = S::Point::identity();
        let mut next_power = 1_usize;
        for (index, point) in h_bold.iter().copied().enumerate() {
            running += point;
            if index + 1 == next_power {
                if running.is_identity() {
                    return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
                }
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

    /// Borrow a checked power-of-two prefix of this generator basis.
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
    /// Base generator used for value and inner-product terms.
    pub g: S::Point,
    /// Blinding generator used by Pedersen commitments.
    pub h: S::Point,
    /// Borrowed left-generator prefix.
    pub g_bold: &'a [S::Point],
    /// Borrowed right-generator prefix.
    pub h_bold: &'a [S::Point],
}

const SECRET_MSM_WINDOW_BITS_V1: usize = 4;
const SECRET_MSM_TABLE_ENTRIES_V1: usize = 1 << SECRET_MSM_WINDOW_BITS_V1;
const SECRET_MSM_CHUNK_TERMS_V1: usize = 256;
const SECRET_MSM_WINDOWS_V1: usize = 256 / SECRET_MSM_WINDOW_BITS_V1;

struct SecretMsmTerm<S: ProofSuite> {
    scalar: S::Scalar,
    point: S::Point,
}

impl<S: ProofSuite> Drop for SecretMsmTerm<S> {
    fn drop(&mut self) {
        self.scalar.clear_secret();
        self.point.clear_secret();
    }
}

/// Exact-capacity owner for prover-secret multiscalar-multiplication terms.
///
/// Construction reserves all storage before a secret is accepted. `push`
/// therefore never reallocates; an over-capacity scalar is cleared before the
/// error is returned. Evaluation requires exactly the declared term count and
/// clears all owned scalar and point copies on success, error, or unwind.
///
/// The fixed four-bit Straus evaluator has secret-independent control flow and
/// memory access. It processes independent 256-point chunks in parallel when
/// the `parallel` feature is enabled and sequentially otherwise, scans every
/// one of the 16 precomputed table entries for every digit, and always executes
/// 64 windows. Chunk results are collected into a preallocated public-size
/// buffer and folded in input order, so scheduling cannot affect the result.
/// This owner cannot erase copies retained by its caller or generated by the
/// compiler, and destructors do not run after process abort.
pub struct SecretMultiexpBuilder<S: ProofSuite> {
    terms: Vec<SecretMsmTerm<S>>,
    exact_capacity: usize,
    allocation_capacity: usize,
}

impl<S: ProofSuite> SecretMultiexpBuilder<S> {
    /// Reserve storage for exactly `exact_capacity` subsequently supplied
    /// terms before accepting any prover secret.
    pub fn new(exact_capacity: usize) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let mut terms = Vec::new();
        terms
            .try_reserve_exact(exact_capacity)
            .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let allocation_capacity = terms.capacity();
        if allocation_capacity < exact_capacity {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        Ok(Self {
            terms,
            exact_capacity,
            allocation_capacity,
        })
    }

    /// Return the exact public term count declared at construction.
    pub const fn capacity(&self) -> usize {
        self.exact_capacity
    }

    /// Return the number of terms accepted so far.
    pub fn len(&self) -> usize {
        self.terms.len()
    }

    /// Return whether no terms have been accepted yet.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Transfer one scalar/point copy into the fixed allocation.
    ///
    /// If the declared count is already full, both incoming named copies are
    /// cleared before [`GeneralizedBulletproofErrorV1::ResourceOverflow`] is
    /// returned.
    pub fn push(
        &mut self,
        mut scalar: S::Scalar,
        mut point: S::Point,
    ) -> Result<(), GeneralizedBulletproofErrorV1> {
        if self.terms.len() >= self.exact_capacity {
            scalar.clear_secret();
            point.clear_secret();
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        debug_assert_eq!(self.terms.capacity(), self.allocation_capacity);
        self.terms.push(SecretMsmTerm { scalar, point });
        debug_assert_eq!(self.terms.capacity(), self.allocation_capacity);
        Ok(())
    }

    /// Evaluate exactly the declared number of terms in constant time with
    /// respect to all scalar values.
    pub fn evaluate(self) -> Result<S::Point, GeneralizedBulletproofErrorV1> {
        if self.terms.len() != self.exact_capacity {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let exact_chunk_count = self.terms.len().div_ceil(SECRET_MSM_CHUNK_TERMS_V1);
        let mut chunks = SecretMsmChunkResults::<S>::new(exact_chunk_count)?;
        chunks.collect(&self.terms)?;
        chunks.fold_in_order()
    }
}

/// Exact-capacity collection of independently evaluated secret MSM chunks.
///
/// Storing `Result` as the item deliberately avoids short-circuiting on the
/// first error: every successful `SecretPoint` is then owned by this vector and
/// cleared if any peer chunk fails. Parallel collection also drops initialized
/// items when collection unwinds, so worker panics cannot strand completed
/// secret-derived points.
struct SecretMsmChunkResults<S: ProofSuite> {
    values: Vec<Result<SecretPoint<S::Point>, GeneralizedBulletproofErrorV1>>,
    exact_len: usize,
    allocation_capacity: usize,
}

impl<S: ProofSuite> SecretMsmChunkResults<S> {
    fn new(exact_len: usize) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(exact_len)
            .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let allocation_capacity = values.capacity();
        if allocation_capacity < exact_len {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        Ok(Self {
            values,
            exact_len,
            allocation_capacity,
        })
    }

    fn collect(&mut self, terms: &[SecretMsmTerm<S>]) -> Result<(), GeneralizedBulletproofErrorV1> {
        if !self.values.is_empty()
            || terms.len().div_ceil(SECRET_MSM_CHUNK_TERMS_V1) != self.exact_len
        {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let allocation = self.values.as_ptr();
        #[cfg(feature = "parallel")]
        terms
            .par_chunks(SECRET_MSM_CHUNK_TERMS_V1)
            .map(secret_straus_chunk::<S>)
            .collect_into_vec(&mut self.values);
        #[cfg(not(feature = "parallel"))]
        self.values.extend(
            terms
                .chunks(SECRET_MSM_CHUNK_TERMS_V1)
                .map(secret_straus_chunk::<S>),
        );
        debug_assert_eq!(self.values.len(), self.exact_len);
        debug_assert_eq!(self.values.capacity(), self.allocation_capacity);
        debug_assert_eq!(self.values.as_ptr(), allocation);
        Ok(())
    }

    fn fold_in_order(self) -> Result<S::Point, GeneralizedBulletproofErrorV1> {
        let mut result = SecretPoint::new(S::Point::identity());
        for chunk in self.values {
            let chunk = chunk?;
            result.add_assign(chunk.expose_copy());
        }
        Ok(result.expose_copy())
    }
}

struct SecretScalarEncodings([[u8; 32]; SECRET_MSM_CHUNK_TERMS_V1]);

impl Drop for SecretScalarEncodings {
    fn drop(&mut self) {
        for encoding in &mut self.0 {
            clear_secret_bytes(encoding);
        }
    }
}

struct SecretDigits([u8; SECRET_MSM_CHUNK_TERMS_V1]);

impl Drop for SecretDigits {
    fn drop(&mut self) {
        clear_secret_bytes(&mut self.0);
    }
}

struct SecretPoint<P: ProofPoint>(P);

impl<P: ProofPoint> SecretPoint<P> {
    fn new(point: P) -> Self {
        Self(point)
    }

    fn expose_copy(&self) -> P {
        self.0
    }

    fn replace(&mut self, point: &mut P) {
        self.0.clear_secret();
        self.0 = *point;
        point.clear_secret();
    }

    fn double_assign(&mut self) {
        let mut doubled = self.0.double();
        self.replace(&mut doubled);
    }

    fn add_assign(&mut self, mut rhs: P) {
        let rhs_slot = BorrowedSecretPoint::new(&mut rhs);
        let mut sum = self.0 + rhs_slot.expose_copy();
        // This explicit drop clears the named by-value `rhs` slot after the
        // addition. The same guard clears it during unwind if point addition
        // panics before reaching this line.
        drop(rhs_slot);
        self.replace(&mut sum);
    }

    fn select_assign(&mut self, candidate: &P, choice: u8) {
        let mut selected = P::conditional_select(&self.0, candidate, choice);
        self.replace(&mut selected);
    }
}

impl<P: ProofPoint> Drop for SecretPoint<P> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

struct BorrowedSecretPoint<'a, P: ProofPoint>(&'a mut P);

impl<'a, P: ProofPoint> BorrowedSecretPoint<'a, P> {
    fn new(point: &'a mut P) -> Self {
        Self(point)
    }

    fn expose_copy(&self) -> P {
        *self.0
    }
}

impl<P: ProofPoint> Drop for BorrowedSecretPoint<'_, P> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}

struct SecretPointTable<P: ProofPoint>(Vec<[P; SECRET_MSM_TABLE_ENTRIES_V1]>);

impl<P: ProofPoint> Drop for SecretPointTable<P> {
    fn drop(&mut self) {
        for row in &mut self.0 {
            for point in row {
                point.clear_secret();
            }
        }
    }
}

struct SecretPointTableRow<P: ProofPoint>([P; SECRET_MSM_TABLE_ENTRIES_V1]);

impl<P: ProofPoint> Drop for SecretPointTableRow<P> {
    fn drop(&mut self) {
        for point in &mut self.0 {
            point.clear_secret();
        }
    }
}

fn ct_eq_u8(left: u8, right: u8) -> u8 {
    let difference = u16::from(left ^ right);
    ((difference.wrapping_sub(1) >> 8) & 1) as u8
}

fn secret_straus_chunk<S: ProofSuite>(
    terms: &[SecretMsmTerm<S>],
) -> Result<SecretPoint<S::Point>, GeneralizedBulletproofErrorV1> {
    if terms.is_empty() || terms.len() > SECRET_MSM_CHUNK_TERMS_V1 {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }

    // Allocate the public-point table before scalar encodings or digits are
    // materialized. Rows and their temporary source copies are still cleared
    // on every exit path because callers may conservatively treat points as
    // secret-derived.
    let mut tables = SecretPointTable(Vec::new());
    tables
        .0
        .try_reserve_exact(terms.len())
        .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    for term in terms {
        let mut row = SecretPointTableRow([S::Point::identity(); SECRET_MSM_TABLE_ENTRIES_V1]);
        for index in 1..SECRET_MSM_TABLE_ENTRIES_V1 {
            row.0[index] = row.0[index - 1] + term.point;
        }
        tables.0.push(row.0);
    }

    let mut encodings = SecretScalarEncodings([[0_u8; 32]; SECRET_MSM_CHUNK_TERMS_V1]);
    for (index, term) in terms.iter().enumerate() {
        let encoding = SecretBytes(term.scalar.bits_le());
        encodings.0[index] = encoding.0;
    }
    let mut digits = SecretDigits([0_u8; SECRET_MSM_CHUNK_TERMS_V1]);
    let mut accumulator = SecretPoint::new(S::Point::identity());
    for window in (0..SECRET_MSM_WINDOWS_V1).rev() {
        for _ in 0..SECRET_MSM_WINDOW_BITS_V1 {
            accumulator.double_assign();
        }
        let byte_index = window / 2;
        let shift = (window % 2) * SECRET_MSM_WINDOW_BITS_V1;
        for index in 0..terms.len() {
            digits.0[index] =
                (encodings.0[index][byte_index] >> shift) & (SECRET_MSM_TABLE_ENTRIES_V1 as u8 - 1);
        }
        for index in 0..terms.len() {
            let mut selected = SecretPoint::new(S::Point::identity());
            for candidate in 0..SECRET_MSM_TABLE_ENTRIES_V1 {
                selected.select_assign(
                    &tables.0[index][candidate],
                    ct_eq_u8(digits.0[index], candidate as u8),
                );
            }
            accumulator.add_assign(selected.expose_copy());
        }
    }
    Ok(accumulator)
}

/// Encoded scalar material cached by Pippenger while extracting windows.
struct SecretScalarBytes(Vec<[u8; 32]>);

impl Drop for SecretScalarBytes {
    fn drop(&mut self) {
        for bytes in &mut self.0 {
            clear_secret_bytes(bytes);
        }
    }
}

/// Deterministic variable-time multiscalar multiplication over public terms.
///
/// This function is verifier/public-input only. Its window selection, bucket
/// access, and zero-digit branches reveal scalar values. Provers must submit
/// secret terms through [`SecretMultiexpBuilder`].
pub fn multiexp<S: ProofSuite>(terms: &[(S::Scalar, S::Point)]) -> S::Point {
    if terms.is_empty() {
        return S::Point::identity();
    }
    if terms.len() == 1 {
        return terms[0].1.scale(terms[0].0);
    }
    if terms.len() == 2 {
        return public_two_term_multiexp::<S>(terms);
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

/// Simultaneous multiplication for two public-scalar terms.
///
/// Inner-product proving repeatedly folds adjacent public generator points by
/// transcript challenges. Sending each pair through bucket Pippenger allocates
/// and reduces sixteen buckets in every four-bit window. This two-bit
/// Shamir/Straus specialization instead uses one fixed 16-entry stack table and
/// halves the number of accumulator additions and loop iterations compared
/// with a bit-at-a-time joint scan. The table index and the zero-digit branch
/// depend only on transcript-public scalars; callers must continue to route
/// secret scalars through [`SecretMultiexpBuilder`].
fn public_two_term_multiexp<S: ProofSuite>(terms: &[(S::Scalar, S::Point)]) -> S::Point {
    debug_assert_eq!(terms.len(), 2);
    debug_assert!((1..=256).contains(&S::Scalar::SCALAR_BITS));
    let left = SecretBytes(terms[0].0.bits_le());
    let right = SecretBytes(terms[1].0.bits_le());
    let identity = S::Point::identity();
    let left_two = terms[0].1.double();
    let left_multiples = [identity, terms[0].1, left_two, left_two + terms[0].1];
    let right_two = terms[1].1.double();
    let right_multiples = [identity, terms[1].1, right_two, right_two + terms[1].1];
    let mut table = [identity; 16];
    for digit in 1..4 {
        table[digit] = left_multiples[digit];
        table[digit << 2] = right_multiples[digit];
    }
    for left_digit in 1..4 {
        for right_digit in 1..4 {
            table[left_digit | (right_digit << 2)] =
                left_multiples[left_digit] + right_multiples[right_digit];
        }
    }

    let mut result = S::Point::identity();
    let windows = S::Scalar::SCALAR_BITS.div_ceil(2);
    for window in (0..windows).rev() {
        result = result.double().double();
        let bit = window * 2;
        let byte = bit / 8;
        let offset = bit % 8;
        let mask = if bit + 1 < S::Scalar::SCALAR_BITS {
            0b11
        } else {
            0b01
        };
        let left_digit = usize::from((left.0[byte] >> offset) & mask);
        let right_digit = usize::from((right.0[byte] >> offset) & mask);
        let digit = left_digit | (right_digit << 2);
        if digit != 0 {
            result += table[digit];
        }
    }
    result
}

fn pippenger<S: ProofSuite>(terms: &[(S::Scalar, S::Point)], window: usize) -> S::Point {
    let windows = S::Scalar::SCALAR_BITS.div_ceil(window);
    let mask = (1_u16 << window) - 1;
    let mut scalar_bytes = SecretScalarBytes(Vec::with_capacity(terms.len()));
    for (scalar, _) in terms {
        let bytes = SecretBytes(scalar.bits_le());
        scalar_bytes.0.push(bytes.0);
    }
    let mut result = S::Point::identity();
    for window_index in (0..windows).rev() {
        if window_index + 1 != windows {
            for _ in 0..window {
                result = result.double();
            }
        }
        let mut buckets = vec![S::Point::identity(); 1 << window];
        let bit_offset = window_index * window;
        for ((_, point), bytes) in terms.iter().zip(&scalar_bytes.0) {
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

/// Owned scalar vector whose elements are cleared when the vector is dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct ScalarVector<F: ProofScalar>(
    /// Scalar elements in deterministic protocol order.
    pub Vec<F>,
);

type ScalarVectorPair<F> = (ScalarVector<F>, ScalarVector<F>);

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
    /// Construct a vector containing `len` zero scalars.
    pub fn zero(len: usize) -> Self {
        Self(vec![F::ZERO; len])
    }

    /// Construct the first `len` powers of `value`, beginning with one.
    ///
    /// # Panics
    ///
    /// Panics when `len` is zero.
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

    /// Return the number of scalar elements.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return whether this vector has no scalar elements.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Compute an inner product with the corresponding prefix of an iterator.
    ///
    /// # Panics
    ///
    /// Panics when the iterator yields fewer elements than this vector.
    pub fn inner_product(&self, vector: impl Iterator<Item = F>) -> F {
        let mut count = 0;
        let mut result = SecretScalar::new(F::ZERO);
        for (left, right) in self.0.iter().zip(vector) {
            *result.expose_mut() += *left * right;
            count += 1;
        }
        assert_eq!(count, self.len());
        result.expose_copy()
    }

    fn pad_with_zeroes(&mut self, len: usize) -> Result<(), GeneralizedBulletproofErrorV1> {
        if self.len() > len {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        if self.len() != len {
            // Allocate first and copy into the final-size allocation. Replacing
            // `self` then clears the complete initialized portion of the old
            // allocation before it is released; `Vec::resize` could otherwise
            // reallocate and leave an unwiped copy behind.
            let mut padded = Self::zero(len);
            padded.0[..self.len()].copy_from_slice(&self.0);
            *self = padded;
        }
        Ok(())
    }

    fn split(mut self) -> Result<(Self, Self), GeneralizedBulletproofErrorV1> {
        if self.len() <= 1 || !self.len().is_multiple_of(2) {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let half = self.len() / 2;
        let mut right = Vec::with_capacity(half);
        right.extend_from_slice(&self.0[half..]);
        // `Vec::split_off` copies `Copy` elements into a new allocation but
        // leaves their bytes beyond the shortened old length. Clear those
        // stale source slots before truncating them out of Drop's reach.
        for value in &mut self.0[half..] {
            value.clear_secret();
        }
        self.0.truncate(half);
        Ok((self, Self(right)))
    }
}

/// Sample a secret vector incrementally so successfully sampled prefixes are
/// cleared if a later entropy request or canonical decode fails.
fn random_scalar_vector<F, R>(
    rng: &mut R,
    len: usize,
) -> Result<ScalarVector<F>, GeneralizedBulletproofErrorV1>
where
    F: ProofScalar,
    R: ProofRandomSource,
{
    let mut result = ScalarVector(Vec::with_capacity(len));
    for _ in 0..len {
        result.0.push(random_scalar::<F, _>(rng)?);
    }
    Ok(result)
}

/// Opening of one Pedersen vector commitment used by the FCMP circuit.
pub struct VectorCommitmentOpening<F: ProofScalar> {
    /// Committed vector values in generator order.
    pub values: ScalarVector<F>,
    /// Pedersen blinding scalar for the commitment.
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
    /// Construct an opening from committed values and its blinding scalar.
    pub fn new(values: Vec<F>, mask: F) -> Self {
        Self {
            values: ScalarVector(values),
            mask,
        }
    }
}

/// Witness for a concrete generalized-Bulletproof arithmetic circuit.
pub struct ArithmeticCircuitWitness<S: ProofSuite> {
    a_l: ScalarVector<S::Scalar>,
    a_r: ScalarVector<S::Scalar>,
    a_o: ScalarVector<S::Scalar>,
    vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
}

impl<S: ProofSuite> ArithmeticCircuitWitness<S> {
    /// Construct a witness and derive each multiplication-gate output.
    ///
    /// Returns an error unless the left and right gate vectors have equal
    /// lengths.
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
    /// Left input wire at the contained multiplication-gate index.
    aL(
        /// Multiplication-gate index containing the wire.
        usize,
    ),
    /// Right input wire at the contained multiplication-gate index.
    aR(
        /// Multiplication-gate index containing the wire.
        usize,
    ),
    /// Output wire at the contained multiplication-gate index.
    aO(
        /// Multiplication-gate index containing the wire.
        usize,
    ),
    /// Vector-commitment coordinate addressed by commitment and element index.
    CG {
        /// Index of the vector commitment in the statement.
        commitment: usize,
        /// Index of the scalar within that vector commitment.
        index: usize,
    },
    /// Scalar commitment at the contained statement index.
    #[cfg_attr(not(test), allow(dead_code))]
    V(
        /// Scalar-commitment index in the statement.
        usize,
    ),
}

/// Sparse generalized-Bulletproof linear combination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinComb<F: ProofScalar> {
    /// Highest multiplication-gate index referenced by any wire term.
    pub highest_a_index: Option<usize>,
    /// Highest vector-commitment index referenced by any coordinate term.
    pub highest_c_index: Option<usize>,
    /// Highest scalar-commitment index referenced by any value term.
    pub highest_v_index: Option<usize>,
    /// Coefficients for left multiplication-gate wires.
    pub wl: Vec<(usize, F)>,
    /// Coefficients for right multiplication-gate wires.
    pub wr: Vec<(usize, F)>,
    /// Coefficients for multiplication-gate output wires.
    pub wo: Vec<(usize, F)>,
    /// Per-commitment coefficients for vector coordinates.
    pub wcg: Vec<Vec<(usize, F)>>,
    /// Coefficients for scalar commitments.
    pub wv: Vec<(usize, F)>,
    /// Constant term of the linear combination.
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
    /// Construct an empty linear combination equal to zero.
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

    /// Append one weighted variable term.
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

    /// Add a constant scalar term.
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

/// Public circuit statement, constraints, and commitment points to verify.
#[derive(Clone, Debug)]
pub struct ArithmeticCircuitStatement<'a, S: ProofSuite> {
    generators: ProofGeneratorView<'a, S>,
    constraints: Vec<LinComb<S::Scalar>>,
    vector_commitments: Vec<S::Point>,
    scalar_commitments: Vec<S::Point>,
}

impl<'a, S: ProofSuite> ArithmeticCircuitStatement<'a, S> {
    /// Construct and validate a statement against its suite-bound generators.
    ///
    /// Returns an error for malformed generator views, out-of-range constraint
    /// references, or inconsistent cached highest-index metadata.
    pub fn new(
        generators: ProofGeneratorView<'a, S>,
        constraints: Vec<LinComb<S::Scalar>>,
        vector_commitments: Vec<S::Point>,
        scalar_commitments: Vec<S::Point>,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let generator_count = generators.g_bold.len();
        let suite_generators = S::generators();
        if generator_count == 0
            || !generator_count.is_power_of_two()
            || generators.h_bold.len() != generator_count
            || !(1..=256).contains(&S::Scalar::SCALAR_BITS)
            || generators.g.is_identity()
            || generators.h.is_identity()
            || generators
                .g_bold
                .iter()
                .copied()
                .any(S::Point::is_identity)
            || generators
                .h_bold
                .iter()
                .copied()
                .any(S::Point::is_identity)
            // A suite statically binds its basis. Accepting an unrelated
            // public view here would make the statement equations use one
            // basis while the batch verifier resolves weights against
            // `S::generators()`.
            || generators.g != suite_generators.g
            || generators.h != suite_generators.h
            || generator_count > suite_generators.g_bold.len()
            || generator_count > suite_generators.h_bold.len()
            || generators.g_bold != &suite_generators.g_bold[..generator_count]
            || generators.h_bold != &suite_generators.h_bold[..generator_count]
        {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }

        for constraint in &constraints {
            let actual_highest_a_index = constraint
                .wl
                .iter()
                .chain(&constraint.wr)
                .chain(&constraint.wo)
                .map(|(index, _)| *index)
                .chain(
                    constraint
                        .wcg
                        .iter()
                        .flat_map(|weights| weights.iter().map(|(index, _)| *index)),
                )
                .max();
            let actual_highest_c_index = constraint
                .wcg
                .iter()
                .enumerate()
                .filter_map(|(commitment, weights)| (!weights.is_empty()).then_some(commitment))
                .max();
            let actual_highest_v_index = constraint.wv.iter().map(|(index, _)| *index).max();

            if constraint.highest_a_index != actual_highest_a_index
                || constraint.highest_c_index != actual_highest_c_index
                || constraint.highest_v_index != actual_highest_v_index
                || actual_highest_a_index.is_some_and(|index| index >= generator_count)
                || actual_highest_c_index
                    .is_some_and(|commitment| commitment >= vector_commitments.len())
                || actual_highest_v_index.is_some_and(|index| index >= scalar_commitments.len())
                || constraint.wcg.len() > vector_commitments.len()
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
    ) -> Result<ScalarVectorPair<S::Scalar>, GeneralizedBulletproofErrorV1> {
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

    /// Create a proof for this statement and witness in `transcript`.
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
        witness.a_l.pad_with_zeroes(n)?;
        witness.a_r.pad_with_zeroes(n)?;
        witness.a_o.pad_with_zeroes(n)?;
        for opening in &mut witness.vector_commitments {
            opening.values.pad_with_zeroes(n)?;
        }

        // Validate every opening and every circuit constraint before emitting
        // any proof bytes. A malformed native witness is an API error, never a
        // source of a knowingly-invalid proof.
        for (commitment, opening) in self
            .vector_commitments
            .iter()
            .zip(&witness.vector_commitments)
        {
            let mut terms = SecretMultiexpBuilder::<S>::new(n + 1)?;
            for (scalar, point) in opening
                .values
                .0
                .iter()
                .copied()
                .zip(self.generators.g_bold.iter().copied())
            {
                terms.push(scalar, point)?;
            }
            terms.push(opening.mask, self.generators.h)?;
            if terms.evaluate()? != *commitment {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
        }
        for constraint in &self.constraints {
            let mut evaluation = SecretScalar::new(constraint.c);
            for (index, weight) in &constraint.wl {
                *evaluation.expose_mut() += witness.a_l[*index] * *weight;
            }
            for (index, weight) in &constraint.wr {
                *evaluation.expose_mut() += witness.a_r[*index] * *weight;
            }
            for (index, weight) in &constraint.wo {
                *evaluation.expose_mut() += witness.a_o[*index] * *weight;
            }
            for (commitment, weights) in constraint.wcg.iter().enumerate() {
                for (index, weight) in weights {
                    *evaluation.expose_mut() +=
                        witness.vector_commitments[commitment].values[*index] * *weight;
                }
            }
            if !constraint.wv.is_empty() || !evaluation.expose_copy().is_zero() {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
        }

        let alpha = SecretScalar::new(random_scalar::<S::Scalar, _>(rng)?);
        let beta = SecretScalar::new(random_scalar::<S::Scalar, _>(rng)?);
        let rho = SecretScalar::new(random_scalar::<S::Scalar, _>(rng)?);
        let ai = {
            let mut terms = SecretMultiexpBuilder::<S>::new((2 * n) + 1)?;
            for (scalar, point) in witness
                .a_l
                .0
                .iter()
                .copied()
                .zip(self.generators.g_bold.iter().copied())
            {
                terms.push(scalar, point)?;
            }
            for (scalar, point) in witness
                .a_r
                .0
                .iter()
                .copied()
                .zip(self.generators.h_bold.iter().copied())
            {
                terms.push(scalar, point)?;
            }
            terms.push(alpha.expose_copy(), self.generators.h)?;
            terms.evaluate()?
        };
        let ao = {
            let mut terms = SecretMultiexpBuilder::<S>::new(n + 1)?;
            for (scalar, point) in witness
                .a_o
                .0
                .iter()
                .copied()
                .zip(self.generators.g_bold.iter().copied())
            {
                terms.push(scalar, point)?;
            }
            terms.push(beta.expose_copy(), self.generators.h)?;
            terms.evaluate()?
        };
        let s_l = random_scalar_vector::<S::Scalar, _>(rng, n)?;
        let s_r = random_scalar_vector::<S::Scalar, _>(rng, n)?;
        let s_point = {
            let mut terms = SecretMultiexpBuilder::<S>::new((2 * n) + 1)?;
            for (scalar, point) in s_l
                .0
                .iter()
                .copied()
                .zip(self.generators.g_bold.iter().copied())
            {
                terms.push(scalar, point)?;
            }
            for (scalar, point) in s_r
                .0
                .iter()
                .copied()
                .zip(self.generators.h_bold.iter().copied())
            {
                terms.push(scalar, point)?;
            }
            terms.push(rho.expose_copy(), self.generators.h)?;
            terms.evaluate()?
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
        drop(y_powers);
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
        let tau_before = random_scalar_vector::<S::Scalar, _>(rng, ni)?;
        let tau_after = random_scalar_vector::<S::Scalar, _>(rng, t_poly_len - ni - 1)?;
        for (coefficient, mask) in t.0[..ni].iter().zip(&tau_before.0) {
            let mut terms = SecretMultiexpBuilder::<S>::new(2)?;
            terms.push(*coefficient, self.generators.g)?;
            terms.push(*mask, self.generators.h)?;
            let commitment = terms.evaluate()?;
            if commitment.is_identity() {
                return Err(GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity);
            }
            transcript.push_point(commitment)?;
        }
        for (coefficient, mask) in t.0[ni + 1..].iter().zip(&tau_after.0) {
            let mut terms = SecretMultiexpBuilder::<S>::new(2)?;
            terms.push(*coefficient, self.generators.g)?;
            terms.push(*mask, self.generators.h)?;
            let commitment = terms.evaluate()?;
            if commitment.is_identity() {
                return Err(GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity);
            }
            transcript.push_point(commitment)?;
        }
        drop(t);
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
        drop(l);
        drop(r);
        let t_caret = SecretScalar::new(l_eval.inner_product(r_eval.0.iter().copied()));

        // FCMP does not use scalar commitments, so the omitted t[ni] mask is
        // zero. Vector-commitment masks instead contribute to `u` below.
        let mut tau_x_poly = ScalarVector(Vec::with_capacity(t_poly_len));
        tau_x_poly.0.extend(tau_before.0.iter().copied());
        tau_x_poly.0.push(S::Scalar::ZERO);
        tau_x_poly.0.extend(tau_after.0.iter().copied());
        let mut tau_x = SecretScalar::new(S::Scalar::ZERO);
        for (index, coefficient) in tau_x_poly.0.iter().copied().enumerate() {
            *tau_x.expose_mut() += coefficient * x[index];
        }
        drop(tau_before);
        drop(tau_after);
        drop(tau_x_poly);
        let mut u = SecretScalar::new(
            (alpha.expose_copy() * x[ilr])
                + (beta.expose_copy() * x[io])
                + (rho.expose_copy() * x[is]),
        );
        for (mut index, opening) in witness.vector_commitments.iter().enumerate() {
            if index >= ilr {
                index += 1;
            }
            *u.expose_mut() += x[index] * opening.mask;
        }
        drop(alpha);
        drop(beta);
        drop(rho);
        drop(witness);

        let mut p_terms = SecretMultiexpBuilder::<S>::new(1 + (2 * n))?;
        for (index, (left, right)) in l_eval.0.iter().zip(&r_eval.0).enumerate() {
            p_terms.push(*left, self.generators.g_bold[index])?;
            p_terms.push(y_inverse[index] * *right, self.generators.h_bold[index])?;
        }
        transcript.push_scalar(tau_x.expose_copy())?;
        transcript.push_scalar(u.expose_copy())?;
        transcript.push_scalar(t_caret.expose_copy())?;
        let ip_x = transcript.challenge()?;
        p_terms.push(ip_x * t_caret.expose_copy(), self.generators.g)?;
        let p = p_terms.evaluate()?;
        drop(tau_x);
        drop(u);
        drop(t_caret);
        drop(x);
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

    /// Consume and verify one proof transcript for this statement.
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

    // The inner-product protocol itself does not otherwise use `P` while
    // proving. Check the opening unconditionally so release builds cannot
    // serialize a proof for an inconsistent caller-supplied statement. Keep
    // the initial H weights and U scalar symbolic: multiplying a secret scalar
    // by their public weight before the constant-time MSM is exactly the same
    // group element and avoids one full scalar multiplication per H generator.
    {
        let mut opening_terms = SecretMultiexpBuilder::<S>::new((2 * n) + 1)?;
        for (scalar, point) in a.0.iter().copied().zip(generators.g_bold.iter().copied()) {
            opening_terms.push(scalar, point)?;
        }
        for ((scalar, weight), point) in
            b.0.iter()
                .copied()
                .zip(h_bold_weights.0.iter().copied())
                .zip(generators.h_bold.iter().copied())
        {
            opening_terms.push(scalar * weight, point)?;
        }
        let opening_product = SecretScalar::new(a.inner_product(b.0.iter().copied()));
        opening_terms.push(opening_product.expose_copy() * u_scalar, generators.g)?;
        if opening_terms.evaluate()? != p {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
    }

    // A one-element IPA has no challenge with which to fold the symbolic H
    // weight. Finish directly against the original fixed generators.
    if n == 1 {
        let mut folded_terms = SecretMultiexpBuilder::<S>::new(3)?;
        folded_terms.push(a[0], generators.g_bold[0])?;
        folded_terms.push(b[0] * h_bold_weights[0], generators.h_bold[0])?;
        folded_terms.push(a[0] * b[0] * u_scalar, generators.g)?;
        let folded = folded_terms.evaluate()?;
        if folded != p {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        transcript.push_scalar(a[0])?;
        transcript.push_scalar(b[0])?;
        return Ok(());
    }

    // Round zero consumes the original fixed basis. Apply each public H weight
    // to the corresponding secret coefficient inside the constant-time MSM.
    // Only after the transcript fixes x do we materialize n/2 folded H points,
    // directly as (x w_L) H_L + (x^-1 w_R) H_R. From then on the ordinary IPA
    // representation and equations are unchanged.
    let (a_left, a_right) = a.split()?;
    let (b_left, b_right) = b.split()?;
    let half = n / 2;
    let (g_left, g_right) = generators.g_bold.split_at(half);
    let (h_left, h_right) = generators.h_bold.split_at(half);
    let (h_weight_left, h_weight_right) = h_bold_weights.0.split_at(half);
    if a_left.len() != half
        || a_right.len() != half
        || b_left.len() != half
        || b_right.len() != half
        || g_left.len() != half
        || g_right.len() != half
        || h_left.len() != half
        || h_right.len() != half
        || h_weight_left.len() != half
        || h_weight_right.len() != half
    {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }

    let c_left = SecretScalar::new(a_left.inner_product(b_right.0.iter().copied()));
    let c_right = SecretScalar::new(a_right.inner_product(b_left.0.iter().copied()));
    let left = {
        let mut terms = SecretMultiexpBuilder::<S>::new((2 * half) + 1)?;
        for (scalar, point) in a_left.0.iter().copied().zip(g_right.iter().copied()) {
            terms.push(scalar, point)?;
        }
        for ((scalar, weight), point) in b_right
            .0
            .iter()
            .copied()
            .zip(h_weight_left.iter().copied())
            .zip(h_left.iter().copied())
        {
            terms.push(scalar * weight, point)?;
        }
        terms.push(c_left.expose_copy() * u_scalar, generators.g)?;
        terms.evaluate()?
    };
    let right = {
        let mut terms = SecretMultiexpBuilder::<S>::new((2 * half) + 1)?;
        for (scalar, point) in a_right.0.iter().copied().zip(g_left.iter().copied()) {
            terms.push(scalar, point)?;
        }
        for ((scalar, weight), point) in b_left
            .0
            .iter()
            .copied()
            .zip(h_weight_right.iter().copied())
            .zip(h_right.iter().copied())
        {
            terms.push(scalar * weight, point)?;
        }
        terms.push(c_right.expose_copy() * u_scalar, generators.g)?;
        terms.evaluate()?
    };
    drop(c_left);
    drop(c_right);
    if left.is_identity() || right.is_identity() {
        return Err(GeneralizedBulletproofErrorV1::InnerProductRoundIdentity);
    }

    transcript.push_point(left)?;
    transcript.push_point(right)?;
    let challenge = transcript.challenge()?;
    let inverse = challenge
        .invert()
        .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
    let (mut g_bold, mut h_bold): (Vec<S::Point>, Vec<S::Point>) = {
        #[cfg(feature = "parallel")]
        {
            rayon::join(
                || {
                    g_left
                        .par_iter()
                        .copied()
                        .zip(g_right.par_iter().copied())
                        .map(|(left, right)| multiexp::<S>(&[(inverse, left), (challenge, right)]))
                        .collect()
                },
                || {
                    h_left
                        .par_iter()
                        .copied()
                        .zip(h_right.par_iter().copied())
                        .zip(h_weight_left.par_iter().copied())
                        .zip(h_weight_right.par_iter().copied())
                        .map(|(((left, right), left_weight), right_weight)| {
                            multiexp::<S>(&[
                                (challenge * left_weight, left),
                                (inverse * right_weight, right),
                            ])
                        })
                        .collect()
                },
            )
        }
        #[cfg(not(feature = "parallel"))]
        {
            (
                g_left
                    .iter()
                    .copied()
                    .zip(g_right.iter().copied())
                    .map(|(left, right)| multiexp::<S>(&[(inverse, left), (challenge, right)]))
                    .collect(),
                h_left
                    .iter()
                    .copied()
                    .zip(h_right.iter().copied())
                    .zip(h_weight_left.iter().copied())
                    .zip(h_weight_right.iter().copied())
                    .map(|(((left, right), left_weight), right_weight)| {
                        multiexp::<S>(&[
                            (challenge * left_weight, left),
                            (inverse * right_weight, right),
                        ])
                    })
                    .collect(),
            )
        }
    };
    p = left.scale(challenge.square()) + p + right.scale(inverse.square());
    a = (a_left * challenge) + &(a_right * inverse);
    b = (b_left * inverse) + &(b_right * challenge);
    drop(h_bold_weights);

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

        let c_left = SecretScalar::new(a_left.inner_product(b_right.0.iter().copied()));
        let c_right = SecretScalar::new(a_right.inner_product(b_left.0.iter().copied()));
        let left = {
            let mut terms = SecretMultiexpBuilder::<S>::new((2 * half) + 1)?;
            for (scalar, point) in a_left.0.iter().copied().zip(g_right.iter().copied()) {
                terms.push(scalar, point)?;
            }
            for (scalar, point) in b_right.0.iter().copied().zip(h_left.iter().copied()) {
                terms.push(scalar, point)?;
            }
            terms.push(c_left.expose_copy() * u_scalar, generators.g)?;
            terms.evaluate()?
        };
        let right = {
            let mut terms = SecretMultiexpBuilder::<S>::new((2 * half) + 1)?;
            for (scalar, point) in a_right.0.iter().copied().zip(g_left.iter().copied()) {
                terms.push(scalar, point)?;
            }
            for (scalar, point) in b_left.0.iter().copied().zip(h_right.iter().copied()) {
                terms.push(scalar, point)?;
            }
            terms.push(c_right.expose_copy() * u_scalar, generators.g)?;
            terms.evaluate()?
        };
        drop(c_left);
        drop(c_right);
        if left.is_identity() || right.is_identity() {
            return Err(GeneralizedBulletproofErrorV1::InnerProductRoundIdentity);
        }

        transcript.push_point(left)?;
        transcript.push_point(right)?;
        let challenge = transcript.challenge()?;
        let inverse = challenge
            .invert()
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;

        // Every fold scalar is transcript-public. Evaluate the independent G
        // and H halves in indexed order. The `parallel` feature uses the
        // ambient deterministic Rayon pool; the fallback performs the same
        // ordered maps sequentially. The two-term specialization avoids a
        // fresh Pippenger bucket arena per pair.
        (g_bold, h_bold) = {
            #[cfg(feature = "parallel")]
            {
                rayon::join(
                    || {
                        g_left
                            .into_par_iter()
                            .zip(g_right.into_par_iter())
                            .map(|(left, right)| {
                                multiexp::<S>(&[(inverse, left), (challenge, right)])
                            })
                            .collect()
                    },
                    || {
                        h_left
                            .into_par_iter()
                            .zip(h_right.into_par_iter())
                            .map(|(left, right)| {
                                multiexp::<S>(&[(challenge, left), (inverse, right)])
                            })
                            .collect()
                    },
                )
            }
            #[cfg(not(feature = "parallel"))]
            {
                (
                    g_left
                        .into_iter()
                        .zip(g_right)
                        .map(|(left, right)| multiexp::<S>(&[(inverse, left), (challenge, right)]))
                        .collect(),
                    h_left
                        .into_iter()
                        .zip(h_right)
                        .map(|(left, right)| multiexp::<S>(&[(challenge, left), (inverse, right)]))
                        .collect(),
                )
            }
        };
        p = left.scale(challenge.square()) + p + right.scale(inverse.square());
        a = (a_left * challenge) + &(a_right * inverse);
        b = (b_left * inverse) + &(b_right * challenge);
    }

    if g_bold.len() != 1 || h_bold.len() != 1 || a.len() != 1 || b.len() != 1 {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }
    let mut folded_terms = SecretMultiexpBuilder::<S>::new(3)?;
    folded_terms.push(a[0], g_bold[0])?;
    folded_terms.push(b[0], h_bold[0])?;
    folded_terms.push(a[0] * b[0] * u_scalar, generators.g)?;
    let folded = folded_terms.evaluate()?;
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

#[cfg(test)]
mod secret_cleanup_tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());
    static CLEAR_CALLS: AtomicUsize = AtomicUsize::new(0);
    static POINT_CLEAR_CALLS: AtomicUsize = AtomicUsize::new(0);
    static POINT_ADD_CALLS: AtomicUsize = AtomicUsize::new(0);
    static PANIC_ON_POINT_ADD: AtomicUsize = AtomicUsize::new(usize::MAX);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TrackingScalar(u64);

    impl Add for TrackingScalar {
        type Output = Self;

        fn add(self, rhs: Self) -> Self::Output {
            Self(self.0.wrapping_add(rhs.0))
        }
    }

    impl Sub for TrackingScalar {
        type Output = Self;

        fn sub(self, rhs: Self) -> Self::Output {
            Self(self.0.wrapping_sub(rhs.0))
        }
    }

    impl Mul for TrackingScalar {
        type Output = Self;

        fn mul(self, rhs: Self) -> Self::Output {
            Self(self.0.wrapping_mul(rhs.0))
        }
    }

    impl Neg for TrackingScalar {
        type Output = Self;

        fn neg(self) -> Self::Output {
            Self(self.0.wrapping_neg())
        }
    }

    impl AddAssign for TrackingScalar {
        fn add_assign(&mut self, rhs: Self) {
            *self = *self + rhs;
        }
    }

    impl SubAssign for TrackingScalar {
        fn sub_assign(&mut self, rhs: Self) {
            *self = *self - rhs;
        }
    }

    impl MulAssign for TrackingScalar {
        fn mul_assign(&mut self, rhs: Self) {
            *self = *self * rhs;
        }
    }

    impl ProofScalar for TrackingScalar {
        const ZERO: Self = Self(0);
        const ONE: Self = Self(1);
        const SCALAR_BITS: usize = 64;

        fn from_u64(value: u64) -> Self {
            Self(value)
        }

        fn decode(bytes: [u8; 32]) -> Option<Self> {
            Some(Self(u64::from_le_bytes(
                bytes[..8]
                    .try_into()
                    .expect("eight-byte tracking scalar encoding"),
            )))
        }

        fn encode(self) -> [u8; 32] {
            let mut bytes = [0_u8; 32];
            bytes[..8].copy_from_slice(&self.0.to_le_bytes());
            bytes
        }

        fn reduce_wide(bytes: [u8; 64]) -> Self {
            Self(u64::from_le_bytes(
                bytes[..8]
                    .try_into()
                    .expect("eight-byte tracking scalar reduction"),
            ))
        }

        fn invert(self) -> Option<Self> {
            (!self.is_zero()).then_some(Self::ONE)
        }

        fn sqrt(self) -> Option<Self> {
            Some(self)
        }

        fn square(self) -> Self {
            self * self
        }

        fn double(self) -> Self {
            self + self
        }

        fn is_zero(self) -> bool {
            self == Self::ZERO
        }

        fn is_odd(self) -> bool {
            self.0 & 1 == 1
        }

        fn clear_secret(&mut self) {
            self.0 = 0;
            CLEAR_CALLS.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TrackingPoint(u64);

    impl Add for TrackingPoint {
        type Output = Self;

        fn add(self, rhs: Self) -> Self::Output {
            let call = POINT_ADD_CALLS.fetch_add(1, Ordering::SeqCst) + 1;
            assert_ne!(
                PANIC_ON_POINT_ADD.load(Ordering::SeqCst),
                call,
                "deliberate secret-MSM point-operation panic"
            );
            Self(self.0.wrapping_add(rhs.0))
        }
    }

    impl Sub for TrackingPoint {
        type Output = Self;

        fn sub(self, rhs: Self) -> Self::Output {
            Self(self.0.wrapping_sub(rhs.0))
        }
    }

    impl Neg for TrackingPoint {
        type Output = Self;

        fn neg(self) -> Self::Output {
            Self(self.0.wrapping_neg())
        }
    }

    impl AddAssign for TrackingPoint {
        fn add_assign(&mut self, rhs: Self) {
            *self = *self + rhs;
        }
    }

    impl SubAssign for TrackingPoint {
        fn sub_assign(&mut self, rhs: Self) {
            *self = *self - rhs;
        }
    }

    impl ProofPoint for TrackingPoint {
        type Scalar = TrackingScalar;
        type Encoded = [u8; 32];
        const POINT_BYTES: usize = 32;

        fn identity() -> Self {
            Self(0)
        }

        fn is_identity(self) -> bool {
            self.0 == 0
        }

        fn double(self) -> Self {
            Self(self.0.wrapping_mul(2))
        }

        fn scale(self, scalar: Self::Scalar) -> Self {
            Self(self.0.wrapping_mul(scalar.0))
        }

        fn conditional_select(a: &Self, b: &Self, choice: u8) -> Self {
            let mask = 0_u64.wrapping_sub(u64::from(choice & 1));
            Self((a.0 & !mask) | (b.0 & mask))
        }

        fn clear_secret(&mut self) {
            self.0 = 0;
            POINT_CLEAR_CALLS.fetch_add(1, Ordering::SeqCst);
        }

        fn encode(self) -> Self::Encoded {
            let mut bytes = [0_u8; 32];
            bytes[..8].copy_from_slice(&self.0.to_le_bytes());
            bytes
        }

        fn decode(
            bytes: impl AsRef<[u8]>,
            allow_identity: bool,
        ) -> Result<Self, GeneralizedBulletproofErrorV1> {
            let bytes = bytes.as_ref();
            if bytes.len() != 32 || bytes[8..].iter().any(|byte| *byte != 0) {
                return Err(GeneralizedBulletproofErrorV1::PointEncoding);
            }
            let point = Self(u64::from_le_bytes(
                bytes[..8]
                    .try_into()
                    .map_err(|_| GeneralizedBulletproofErrorV1::PointEncoding)?,
            ));
            if !allow_identity && point.is_identity() {
                return Err(GeneralizedBulletproofErrorV1::PointIdentity);
            }
            Ok(point)
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TrackingSuite;

    impl ProofSuite for TrackingSuite {
        type Scalar = TrackingScalar;
        type Point = TrackingPoint;

        fn generators() -> &'static ProofGenerators<Self> {
            static GENERATORS: std::sync::OnceLock<ProofGenerators<TrackingSuite>> =
                std::sync::OnceLock::new();
            GENERATORS.get_or_init(|| {
                ProofGenerators::new(
                    TrackingPoint(1),
                    TrackingPoint(2),
                    vec![TrackingPoint(3)],
                    vec![TrackingPoint(4)],
                )
                .expect("tracking generator basis")
            })
        }
    }

    struct ScriptedRandom {
        requests: usize,
        fail_at: Option<usize>,
    }

    impl ProofRandomSource for ScriptedRandom {
        fn fill_bytes(
            &mut self,
            destination: &mut [u8],
        ) -> Result<(), GeneralizedBulletproofErrorV1> {
            let request = self.requests;
            self.requests += 1;
            if self.fail_at == Some(request) {
                return Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable);
            }
            destination.fill((request + 1) as u8);
            Ok(())
        }
    }

    #[test]
    fn scoped_guards_clear_named_scalars_and_msm_copies() {
        let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
        CLEAR_CALLS.store(0, Ordering::SeqCst);
        {
            let _scalar = SecretScalar::new(TrackingScalar(7));
            let mut terms =
                SecretMultiexpBuilder::<TrackingSuite>::new(2).expect("fixed tracking capacity");
            terms
                .push(TrackingScalar(11), TrackingPoint(17))
                .expect("first term");
            terms
                .push(TrackingScalar(13), TrackingPoint(19))
                .expect("second term");
        }
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn secret_builder_rejects_overflow_without_reallocation_and_wipes_terms() {
        let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
        CLEAR_CALLS.store(0, Ordering::SeqCst);
        POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
        let mut terms =
            SecretMultiexpBuilder::<TrackingSuite>::new(2).expect("fixed tracking capacity");
        terms
            .push(TrackingScalar(3), TrackingPoint(5))
            .expect("first term");
        terms
            .push(TrackingScalar(7), TrackingPoint(11))
            .expect("second term");
        let allocation = terms.terms.as_ptr();
        let allocation_capacity = terms.terms.capacity();
        assert_eq!(
            terms.push(TrackingScalar(13), TrackingPoint(17)),
            Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
        );
        assert_eq!(terms.terms.as_ptr(), allocation);
        assert_eq!(terms.terms.capacity(), allocation_capacity);
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
        drop(terms);
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 3);
        assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 3);

        CLEAR_CALLS.store(0, Ordering::SeqCst);
        let mut incomplete =
            SecretMultiexpBuilder::<TrackingSuite>::new(2).expect("fixed tracking capacity");
        incomplete
            .push(TrackingScalar(19), TrackingPoint(23))
            .expect("partial term");
        assert_eq!(
            incomplete.evaluate(),
            Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
        );
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn secret_builder_matches_public_and_naive_msm_across_chunks() {
        let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
        PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
        POINT_ADD_CALLS.store(0, Ordering::SeqCst);
        let mut public_terms = Vec::with_capacity(260);
        let edges = [0_u64, 1, 2, u64::MAX];
        for index in 0..260_u64 {
            let scalar = if index < edges.len() as u64 {
                edges[index as usize]
            } else {
                index
                    .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                    .rotate_left((index % 64) as u32)
            };
            public_terms.push((
                TrackingScalar(scalar),
                TrackingPoint(index.wrapping_mul(17).wrapping_add(3)),
            ));
        }
        let expected = public_terms
            .iter()
            .fold(TrackingPoint::identity(), |sum, term| {
                TrackingPoint(sum.0.wrapping_add(term.0.0.wrapping_mul(term.1.0)))
            });
        assert_eq!(multiexp::<TrackingSuite>(&public_terms), expected);

        let evaluate_secret = || {
            let mut secret = SecretMultiexpBuilder::<TrackingSuite>::new(public_terms.len())
                .expect("fixed cross-chunk capacity");
            for (scalar, point) in public_terms.iter().copied() {
                secret
                    .push(scalar, point)
                    .expect("term fits exact capacity");
            }
            secret.evaluate().expect("complete secret MSM")
        };
        #[cfg(feature = "parallel")]
        let single_thread = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("single-thread Rayon pool")
            .install(&evaluate_secret);
        #[cfg(not(feature = "parallel"))]
        let single_thread = evaluate_secret();
        assert_eq!(single_thread, expected);
        #[cfg(feature = "parallel")]
        {
            let four_threads = rayon::ThreadPoolBuilder::new()
                .num_threads(4)
                .build()
                .expect("four-thread Rayon pool")
                .install(&evaluate_secret);
            assert_eq!(four_threads, expected);
            assert_eq!(single_thread.encode(), four_threads.encode());
        }
    }

    #[test]
    fn public_two_term_straus_matches_independent_scaling_at_scalar_edges() {
        let scalars = [
            0_u64,
            1,
            2,
            3,
            4,
            0x5555_5555_5555_5555,
            0xaaaa_aaaa_aaaa_aaaa,
            0x8000_0000_0000_0001,
            u64::MAX,
        ];
        for left in scalars {
            for right in scalars {
                let terms = [
                    (TrackingScalar(left), TrackingPoint(0x1234_5678)),
                    (TrackingScalar(right), TrackingPoint(0x9abc_def0)),
                ];
                let expected = terms[0].1.scale(terms[0].0) + terms[1].1.scale(terms[1].0);
                assert_eq!(
                    multiexp::<TrackingSuite>(&terms),
                    expected,
                    "two-term public fold diverged for ({left:#x}, {right:#x})"
                );
            }
        }
    }

    fn tracking_msm(
        terms: impl IntoIterator<Item = (TrackingScalar, TrackingPoint)>,
    ) -> TrackingPoint {
        terms
            .into_iter()
            .fold(TrackingPoint::identity(), |sum, (scalar, point)| {
                sum + point.scale(scalar)
            })
    }

    fn tracking_inner_product(left: &[TrackingScalar], right: &[TrackingScalar]) -> TrackingScalar {
        left.iter()
            .copied()
            .zip(right.iter().copied())
            .fold(TrackingScalar::ZERO, |sum, (left, right)| {
                sum + (left * right)
            })
    }

    #[test]
    fn symbolic_initial_h_matches_eager_materialization_at_small_powers_of_two() {
        let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
        PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
        for n in [1_usize, 2, 4, 8] {
            let g_bold = (0..n)
                .map(|index| TrackingPoint(17 + (index as u64 * 6)))
                .collect::<Vec<_>>();
            let h_bold = (0..n)
                .map(|index| TrackingPoint(71 + (index as u64 * 10)))
                .collect::<Vec<_>>();
            let a = (0..n)
                .map(|index| TrackingScalar(3 + index as u64))
                .collect::<Vec<_>>();
            let b = (0..n)
                .map(|index| TrackingScalar(19 + (index as u64 * 3)))
                .collect::<Vec<_>>();
            let weight_edges = [
                0_u64,
                1,
                u64::MAX,
                0x8000_0000_0000_0001,
                2,
                3,
                0x5555_5555_5555_5555,
                0xaaaa_aaaa_aaaa_aaaa,
            ];
            let weights = weight_edges[..n]
                .iter()
                .copied()
                .map(TrackingScalar)
                .collect::<Vec<_>>();
            let eager_h = h_bold
                .iter()
                .copied()
                .zip(weights.iter().copied())
                .map(|(point, weight)| point.scale(weight))
                .collect::<Vec<_>>();
            let g = TrackingPoint(211);
            let u_scalar = TrackingScalar(13);
            let u = g.scale(u_scalar);
            let product = tracking_inner_product(&a, &b);

            let eager_opening = tracking_msm(
                a.iter()
                    .copied()
                    .zip(g_bold.iter().copied())
                    .chain(b.iter().copied().zip(eager_h.iter().copied()))
                    .chain(core::iter::once((product, u))),
            );
            let symbolic_opening = tracking_msm(
                a.iter()
                    .copied()
                    .zip(g_bold.iter().copied())
                    .chain(
                        b.iter()
                            .copied()
                            .zip(weights.iter().copied())
                            .map(|(scalar, weight)| scalar * weight)
                            .zip(h_bold.iter().copied()),
                    )
                    .chain(core::iter::once((product * u_scalar, g))),
            );
            assert_eq!(symbolic_opening, eager_opening, "opening diverged at n={n}");

            if n == 1 {
                continue;
            }
            let half = n / 2;
            let (a_left, a_right) = a.split_at(half);
            let (b_left, b_right) = b.split_at(half);
            let (g_left, g_right) = g_bold.split_at(half);
            let (h_left, h_right) = h_bold.split_at(half);
            let (eager_h_left, eager_h_right) = eager_h.split_at(half);
            let (weight_left, weight_right) = weights.split_at(half);
            let c_left = tracking_inner_product(a_left, b_right);
            let c_right = tracking_inner_product(a_right, b_left);

            let eager_left = tracking_msm(
                a_left
                    .iter()
                    .copied()
                    .zip(g_right.iter().copied())
                    .chain(b_right.iter().copied().zip(eager_h_left.iter().copied()))
                    .chain(core::iter::once((c_left, u))),
            );
            let symbolic_left = tracking_msm(
                a_left
                    .iter()
                    .copied()
                    .zip(g_right.iter().copied())
                    .chain(
                        b_right
                            .iter()
                            .copied()
                            .zip(weight_left.iter().copied())
                            .map(|(scalar, weight)| scalar * weight)
                            .zip(h_left.iter().copied()),
                    )
                    .chain(core::iter::once((c_left * u_scalar, g))),
            );
            assert_eq!(symbolic_left, eager_left, "L0 diverged at n={n}");

            let eager_right = tracking_msm(
                a_right
                    .iter()
                    .copied()
                    .zip(g_left.iter().copied())
                    .chain(b_left.iter().copied().zip(eager_h_right.iter().copied()))
                    .chain(core::iter::once((c_right, u))),
            );
            let symbolic_right = tracking_msm(
                a_right
                    .iter()
                    .copied()
                    .zip(g_left.iter().copied())
                    .chain(
                        b_left
                            .iter()
                            .copied()
                            .zip(weight_right.iter().copied())
                            .map(|(scalar, weight)| scalar * weight)
                            .zip(h_right.iter().copied()),
                    )
                    .chain(core::iter::once((c_right * u_scalar, g))),
            );
            assert_eq!(symbolic_right, eager_right, "R0 diverged at n={n}");

            let challenge = TrackingScalar(7);
            let inverse = TrackingScalar(11);
            for index in 0..half {
                let eager_fold =
                    eager_h_left[index].scale(challenge) + eager_h_right[index].scale(inverse);
                let symbolic_fold = h_left[index].scale(challenge * weight_left[index])
                    + h_right[index].scale(inverse * weight_right[index]);
                assert_eq!(
                    symbolic_fold, eager_fold,
                    "first H fold diverged at n={n}, index={index}"
                );
            }
        }
    }

    #[derive(Default)]
    struct RecordingProverTranscript(Vec<u8>);

    impl ProverTranscript<TrackingSuite> for RecordingProverTranscript {
        fn push_scalar(
            &mut self,
            scalar: TrackingScalar,
        ) -> Result<(), GeneralizedBulletproofErrorV1> {
            self.0.push(0);
            self.0.extend_from_slice(&scalar.encode());
            Ok(())
        }

        fn push_point(
            &mut self,
            point: TrackingPoint,
        ) -> Result<(), GeneralizedBulletproofErrorV1> {
            self.0.push(1);
            self.0.extend_from_slice(&point.encode());
            Ok(())
        }

        fn challenge(&mut self) -> Result<TrackingScalar, GeneralizedBulletproofErrorV1> {
            Ok(TrackingScalar::ONE)
        }
    }

    #[test]
    fn symbolic_h_proof_bytes_are_worker_count_independent() {
        let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
        PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
        let g_bold = [
            TrackingPoint(17),
            TrackingPoint(19),
            TrackingPoint(23),
            TrackingPoint(29),
        ];
        let h_bold = [
            TrackingPoint(31),
            TrackingPoint(37),
            TrackingPoint(41),
            TrackingPoint(43),
        ];
        let generators = ProofGeneratorView::<TrackingSuite> {
            g: TrackingPoint(47),
            h: TrackingPoint(53),
            g_bold: &g_bold,
            h_bold: &h_bold,
        };
        let a = [
            TrackingScalar(2),
            TrackingScalar(3),
            TrackingScalar(4),
            TrackingScalar(5),
        ];
        let b = [
            TrackingScalar(6),
            TrackingScalar(7),
            TrackingScalar(8),
            TrackingScalar(9),
        ];
        let weights = [
            TrackingScalar(10),
            TrackingScalar(11),
            TrackingScalar(12),
            TrackingScalar(13),
        ];
        let u_scalar = TrackingScalar(3);
        let product = tracking_inner_product(&a, &b);
        let p = tracking_msm(
            a.iter()
                .copied()
                .zip(g_bold.iter().copied())
                .chain(
                    b.iter()
                        .copied()
                        .zip(weights.iter().copied())
                        .map(|(scalar, weight)| scalar * weight)
                        .zip(h_bold.iter().copied()),
                )
                .chain(core::iter::once((product * u_scalar, generators.g))),
        );
        let prove = || {
            let mut transcript = RecordingProverTranscript::default();
            prove_inner_product::<TrackingSuite, _>(
                generators,
                ScalarVector(weights.to_vec()),
                u_scalar,
                p,
                ScalarVector(a.to_vec()),
                ScalarVector(b.to_vec()),
                &mut transcript,
            )
            .expect("symbolic-H tracking proof");
            transcript.0
        };
        #[cfg(feature = "parallel")]
        let single_thread = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("single-thread Rayon pool")
            .install(&prove);
        #[cfg(not(feature = "parallel"))]
        let single_thread = prove();
        assert!(!single_thread.is_empty());
        #[cfg(feature = "parallel")]
        {
            let four_threads = rayon::ThreadPoolBuilder::new()
                .num_threads(4)
                .build()
                .expect("four-thread Rayon pool")
                .install(&prove);
            assert_eq!(single_thread, four_threads);
        }
    }

    #[test]
    fn secret_chunk_fold_clears_successes_after_peer_error() {
        let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
        POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
        PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
        let mut chunks =
            SecretMsmChunkResults::<TrackingSuite>::new(3).expect("fixed chunk capacity");
        let allocation = chunks.values.as_ptr();
        let allocation_capacity = chunks.values.capacity();
        chunks.values.push(Ok(SecretPoint::new(TrackingPoint(11))));
        chunks
            .values
            .push(Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant));
        chunks.values.push(Ok(SecretPoint::new(TrackingPoint(13))));
        assert_eq!(chunks.values.as_ptr(), allocation);
        assert_eq!(chunks.values.capacity(), allocation_capacity);
        assert_eq!(
            chunks.fold_in_order(),
            Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
        );
        // The accumulator, consumed first result, its named intermediates, and
        // the still-buffered successful result are all cleared on early exit.
        assert_eq!(POINT_CLEAR_CALLS.load(Ordering::SeqCst), 6);
    }

    #[test]
    fn secret_builder_unwind_wipes_terms_digits_tables_and_named_points() {
        let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
        CLEAR_CALLS.store(0, Ordering::SeqCst);
        POINT_CLEAR_CALLS.store(0, Ordering::SeqCst);
        POINT_ADD_CALLS.store(0, Ordering::SeqCst);
        SECRET_BYTE_CLEAR_CALLS.store(0, Ordering::SeqCst);
        // Two 16-entry tables require 30 additions. Panic on the first
        // scalar-dependent accumulator addition after digits were extracted.
        PANIC_ON_POINT_ADD.store(31, Ordering::SeqCst);
        let mut secret =
            SecretMultiexpBuilder::<TrackingSuite>::new(2).expect("fixed tracking capacity");
        secret
            .push(TrackingScalar(0x1234), TrackingPoint(5))
            .expect("first term");
        secret
            .push(TrackingScalar(0xabcd), TrackingPoint(7))
            .expect("second term");
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = secret.evaluate();
        }));
        PANIC_ON_POINT_ADD.store(usize::MAX, Ordering::SeqCst);
        assert!(unwind.is_err());
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
        assert!(SECRET_BYTE_CLEAR_CALLS.load(Ordering::SeqCst) >= 259);
        assert!(POINT_CLEAR_CALLS.load(Ordering::SeqCst) > 40);
    }

    #[test]
    fn vector_padding_and_split_clear_replaced_allocations() {
        let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
        CLEAR_CALLS.store(0, Ordering::SeqCst);
        let mut padded = ScalarVector(vec![TrackingScalar(1), TrackingScalar(2)]);
        padded
            .pad_with_zeroes(4)
            .expect("tracking vector pads to final length");
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
        drop(padded);
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);

        CLEAR_CALLS.store(0, Ordering::SeqCst);
        let values = ScalarVector(vec![
            TrackingScalar(1),
            TrackingScalar(2),
            TrackingScalar(3),
            TrackingScalar(4),
        ]);
        let (left, right) = values.split().expect("tracking vector splits evenly");
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 2);
        drop(left);
        drop(right);
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);
    }

    #[test]
    fn random_vector_clears_success_and_partial_failure() {
        let _lock = TEST_LOCK.lock().expect("secret cleanup test lock");
        CLEAR_CALLS.store(0, Ordering::SeqCst);
        let mut success = ScriptedRandom {
            requests: 0,
            fail_at: None,
        };
        let values = random_scalar_vector::<TrackingScalar, _>(&mut success, 4)
            .expect("scripted random succeeds");
        // Each named return slot in `random_scalar` is cleared after its copy
        // enters the owned vector.
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 4);
        drop(values);
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 8);

        CLEAR_CALLS.store(0, Ordering::SeqCst);
        let mut failure = ScriptedRandom {
            requests: 0,
            fail_at: Some(3),
        };
        assert!(matches!(
            random_scalar_vector::<TrackingScalar, _>(&mut failure, 5),
            Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable)
        ));
        assert_eq!(CLEAR_CALLS.load(Ordering::SeqCst), 6);
    }
}
