//! Reusable generalized-Bulletproof arithmetic and inner-product backend.
//!
//! This module owns the curve-agnostic proof equations. Transcript codecs,
//! concrete curves, generator derivation domains, and entropy providers remain
//! explicit adapters so a protocol can freeze its own consensus bytes.
pub(crate) mod exact_small_coefficient_source_v1;
use exact_small_coefficient_source_v1::{
    ExactSmallCoefficientAggregatesV1, ExactSmallCoefficientConstraintSourceV1,
};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::{
    fmt::Debug,
    ops::{Add, AddAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign},
};
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
/// Sample one canonical scalar from a fallible byte source into a zeroizing
/// owner.
///
/// Non-canonical candidates are rejected with a fixed public retry bound;
/// entropy failure is returned immediately and never replaced with fallback
/// bytes.
pub fn random_scalar<F, R>(rng: &mut R) -> Result<SecretScalar<F>, GeneralizedBulletproofErrorV1>
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
    /// Sample one canonical scalar from the supplied entropy source into a
    /// zeroizing owner.
    fn random(
        rng: &mut impl ProofRandomSource,
    ) -> Result<Option<SecretScalar<Self>>, GeneralizedBulletproofErrorV1> {
        let mut bytes = SecretBytes([0_u8; 32]);
        if rng.fill_bytes(&mut bytes.0).is_err() {
            return Err(GeneralizedBulletproofErrorV1::RandomnessUnavailable);
        }
        if let Some(mut scalar) = Self::decode(bytes.0) {
            Ok(Some(SecretScalar::take(&mut scalar)))
        } else {
            Ok(None)
        }
    }
}
/// One owned secret scalar whose storage is cleared on every exit path.
///
/// `ProofScalar` is necessarily `Copy`, so arithmetic can still create
/// compiler temporaries and register copies. This guard covers the named stack
/// slot owned by the prover and makes every intentional copy explicit.
pub struct SecretScalar<F: ProofScalar>(F);
/// Erases one callee-owned `Copy` scalar parameter on every exit path.
struct BorrowedSecretScalarSlot<'a, F: ProofScalar>(&'a mut F);
impl<F: ProofScalar> BorrowedSecretScalarSlot<'_, F> {
    fn expose_copy(&self) -> F {
        *self.0
    }
}
impl<F: ProofScalar> Drop for BorrowedSecretScalarSlot<'_, F> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}
impl<F: ProofScalar> SecretScalar<F> {
    fn new(mut value: F) -> Self {
        let incoming = BorrowedSecretScalarSlot(&mut value);
        let owned = Self(incoming.expose_copy());
        drop(incoming);
        owned
    }
    fn take(value: &mut F) -> Self {
        let incoming = BorrowedSecretScalarSlot(value);
        let owned = Self(incoming.expose_copy());
        drop(incoming);
        owned
    }
    fn is_zero(&self) -> bool {
        self.0.eq(&F::ZERO)
    }
    /// Borrow the scalar while this owner retains responsibility for clearing
    /// its storage.
    pub fn expose_ref(&self) -> &F {
        &self.0
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
    fn push_scalar(&mut self, scalar: &S::Scalar) -> Result<(), GeneralizedBulletproofErrorV1>;
    /// Append one canonical non-identity point to the proof transcript.
    fn push_point(&mut self, point: &S::Point) -> Result<(), GeneralizedBulletproofErrorV1>;
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
impl<S: ProofSuite> SecretMsmTerm<S> {
    fn copy_from_borrowed(scalar: &S::Scalar, point: &S::Point) -> Self {
        Self {
            scalar: *scalar,
            point: *point,
        }
    }
}
impl<S: ProofSuite> Drop for SecretMsmTerm<S> {
    fn drop(&mut self) {
        self.scalar.clear_secret();
        self.point.clear_secret();
    }
}
/// Unwind-safe handoff guard for private by-value `push_copy` parameter slots.
///
/// The complete retained term is initialized before either source slot moves.
/// Each vacated source receives zero or the identity and is then cleared after
/// a successful push or on every error and unwind path.
struct BorrowedSecretMsmTerm<'a, S: ProofSuite> {
    scalar: &'a mut S::Scalar,
    point: &'a mut S::Point,
}
impl<'a, S: ProofSuite> BorrowedSecretMsmTerm<'a, S> {
    fn new(scalar: &'a mut S::Scalar, point: &'a mut S::Point) -> Self {
        Self { scalar, point }
    }
    fn take_term(&mut self) -> SecretMsmTerm<S> {
        let mut retained = SecretMsmTerm {
            scalar: S::Scalar::ZERO,
            point: S::Point::identity(),
        };
        core::mem::swap(&mut retained.scalar, &mut *self.scalar);
        core::mem::swap(&mut retained.point, &mut *self.point);
        retained
    }
}
impl<S: ProofSuite> Drop for BorrowedSecretMsmTerm<'_, S> {
    fn drop(&mut self) {
        self.scalar.clear_secret();
        self.point.clear_secret();
    }
}
/// Exact-capacity owner for prover-secret multiscalar-multiplication terms.
///
/// Construction reserves all storage before a secret is accepted. `push`
/// borrows caller-owned values and copies them directly into a retained term
/// only after the fixed-capacity preflight succeeds. Private computed-value
/// insertions hand both by-value parameter slots directly into a retained term
/// and clear the vacated slots on success, error, or unwind. Evaluation requires
/// exactly the declared term count and clears all retained scalar and point
/// copies on success, error, or unwind.
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
    /// Copy one borrowed scalar/point pair into the fixed allocation.
    ///
    /// Caller-owned slots remain under their existing RAII owner. If the
    /// declared count is already full, this returns
    /// [`GeneralizedBulletproofErrorV1::ResourceOverflow`] before making a
    /// copy. Otherwise both copies are created directly inside the zeroizing
    /// retained-term owner.
    pub fn push(
        &mut self,
        scalar: &S::Scalar,
        point: &S::Point,
    ) -> Result<(), GeneralizedBulletproofErrorV1> {
        if self.terms.len() >= self.exact_capacity {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        debug_assert_eq!(self.terms.capacity(), self.allocation_capacity);
        self.terms
            .push(SecretMsmTerm::<S>::copy_from_borrowed(scalar, point));
        debug_assert_eq!(self.terms.capacity(), self.allocation_capacity);
        Ok(())
    }
    fn push_copy(
        &mut self,
        mut scalar: S::Scalar,
        mut point: S::Point,
    ) -> Result<(), GeneralizedBulletproofErrorV1> {
        let mut incoming = BorrowedSecretMsmTerm::<S>::new(&mut scalar, &mut point);
        if self.terms.len() >= self.exact_capacity {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        debug_assert_eq!(self.terms.capacity(), self.allocation_capacity);
        let retained = incoming.take_term();
        self.terms.push(retained);
        // Clear the now-zero/identity private parameter slots before any later
        // assertion or return. The retained term owns the moved values through
        // every success, error, and unwind path after the handoff.
        drop(incoming);
        debug_assert_eq!(self.terms.capacity(), self.allocation_capacity);
        Ok(())
    }
    /// Evaluate exactly the declared number of terms in constant time with
    /// respect to all scalar values.
    pub fn evaluate(self) -> Result<SecretPoint<S::Point>, GeneralizedBulletproofErrorV1> {
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
    fn fold_in_order(self) -> Result<SecretPoint<S::Point>, GeneralizedBulletproofErrorV1> {
        let mut result = SecretPoint::new(S::Point::identity());
        for chunk in self.values {
            let chunk = chunk?;
            result.add_assign_secret(chunk);
        }
        Ok(result)
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
/// Move-only owner for one point derived from prover-secret MSM scalars.
///
/// The point can be inspected only by borrow and is erased when this owner is
/// dropped. It deliberately implements neither `Copy` nor `Clone`, so callers
/// must keep the owner live through comparison, transcript publication, and
/// any controlled in-place update.
pub struct SecretPoint<P: ProofPoint>(P);
impl<P: ProofPoint> SecretPoint<P> {
    fn new(mut point: P) -> Self {
        let incoming = BorrowedSecretPoint::new(&mut point);
        let owned = Self(incoming.expose_copy());
        drop(incoming);
        owned
    }
    /// Borrow the retained point without transferring ownership.
    pub fn expose_ref(&self) -> &P {
        &self.0
    }
    /// Move this retained point into caller-owned erasing storage and clear
    /// both the previous destination and intermediate source slots.
    pub fn move_into(mut self, destination: &mut P) {
        destination.clear_secret();
        *destination = self.0;
        self.0.clear_secret();
    }
    /// Return whether the retained point is the group identity.
    pub fn is_identity(&self) -> bool {
        self.0.eq(&P::identity())
    }
    /// Compare the retained point with one borrowed point.
    pub fn equals(&self, point: &P) -> bool {
        self.0.eq(point)
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
    fn add_assign_secret(&mut self, rhs: Self) {
        let mut sum = self.0 + rhs.0;
        drop(rhs);
        self.replace(&mut sum);
    }
    fn add_scaled_pair_assign(
        &mut self,
        left: Self,
        left_scalar: P::Scalar,
        right: Self,
        right_scalar: P::Scalar,
    ) {
        let mut updated = left.0.scale(left_scalar) + self.0 + right.0.scale(right_scalar);
        drop(right);
        drop(left);
        self.replace(&mut updated);
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
fn ct_eq_window_nibble(encoded_byte: &u8, shift: usize, candidate: u8) -> u8 {
    let difference =
        u16::from(((*encoded_byte >> shift) & (SECRET_MSM_TABLE_ENTRIES_V1 as u8 - 1)) ^ candidate);
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
        let mut encoding = SecretBytes(term.scalar.bits_le());
        core::mem::swap(&mut encodings.0[index], &mut encoding.0);
        drop(encoding);
    }
    let mut accumulator = SecretPoint::new(S::Point::identity());
    for window in (0..SECRET_MSM_WINDOWS_V1).rev() {
        for _ in 0..SECRET_MSM_WINDOW_BITS_V1 {
            accumulator.double_assign();
        }
        let byte_index = window / 2;
        let shift = (window % 2) * SECRET_MSM_WINDOW_BITS_V1;
        for index in 0..terms.len() {
            let mut selected = SecretPoint::new(S::Point::identity());
            for candidate in 0..SECRET_MSM_TABLE_ENTRIES_V1 {
                selected.select_assign(
                    &tables.0[index][candidate],
                    ct_eq_window_nibble(&encodings.0[index][byte_index], shift, candidate as u8),
                );
            }
            accumulator.add_assign_secret(selected);
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
    /// Compute one elementwise product from borrowed inputs into a fixed
    /// zeroizing allocation.
    fn product_from_borrowed(
        left: &Self,
        right: &Self,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        if left.len() != right.len() {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let exact_len = left.len();
        let mut product = Self(Vec::new());
        product
            .0
            .try_reserve_exact(exact_len)
            .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let allocation_capacity = product.0.capacity();
        if allocation_capacity < exact_len {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        let allocation_pointer = product.0.as_ptr();
        for _ in 0..exact_len {
            debug_assert!(product.0.len() < allocation_capacity);
            product.0.push(F::ZERO);
        }
        debug_assert_eq!(product.0.len(), exact_len);
        debug_assert_eq!(product.0.capacity(), allocation_capacity);
        debug_assert_eq!(product.0.as_ptr(), allocation_pointer);
        for ((output, left), right) in product.0.iter_mut().zip(&left.0).zip(&right.0) {
            *output = *left;
            *output *= *right;
        }
        debug_assert_eq!(product.0.len(), exact_len);
        debug_assert_eq!(product.0.capacity(), allocation_capacity);
        debug_assert_eq!(product.0.as_ptr(), allocation_pointer);
        Ok(product)
    }
    /// Add one borrowed vector multiplied by one borrowed scalar in place.
    fn add_scaled_assign(&mut self, coefficient: &Self, scalar: &F) {
        assert_eq!(self.len(), coefficient.len());
        for (result, coefficient) in self.0.iter_mut().zip(&coefficient.0) {
            *result += *coefficient * *scalar;
        }
    }
    /// Compute an inner product with the corresponding prefix of an iterator
    /// and retain the result in a zeroizing owner.
    ///
    /// # Panics
    ///
    /// Panics when the iterator yields fewer elements than this vector.
    pub fn inner_product<'a>(&self, vector: impl Iterator<Item = &'a F>) -> SecretScalar<F>
    where
        F: 'a,
    {
        let mut count = 0;
        let mut result = SecretScalar::new(F::ZERO);
        for (left, right) in self.0.iter().zip(vector) {
            *result.expose_mut() += *left * *right;
            count += 1;
        }
        assert_eq!(count, self.len());
        result
    }
    fn pad_with_zeroes(&mut self, len: usize) -> Result<(), GeneralizedBulletproofErrorV1> {
        let source_len = self.len();
        if source_len > len {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        if source_len == len {
            return Ok(());
        }
        let source_pointer = self.0.as_ptr();
        let source_capacity = self.0.capacity();
        // Establish the complete final-size zeroizing destination before
        // moving any private source slot. Allocation failure therefore leaves
        // the original owner unchanged, and insertion cannot grow later.
        let mut padded = Self(Vec::new());
        padded
            .0
            .try_reserve_exact(len)
            .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let allocation_capacity = padded.0.capacity();
        if allocation_capacity < len {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        let allocation_pointer = padded.0.as_ptr();
        for _ in 0..len {
            debug_assert!(padded.0.len() < allocation_capacity);
            padded.0.push(F::ZERO);
        }
        debug_assert_eq!(padded.0.len(), len);
        debug_assert_eq!(padded.0.capacity(), allocation_capacity);
        debug_assert_eq!(padded.0.as_ptr(), allocation_pointer);
        // Move the initialized prefix directly between owner slots. Clear
        // each now-zero source through the scalar erasure boundary before its
        // length is shortened and its allocation is released.
        for (source, destination) in self.0.iter_mut().zip(&mut padded.0[..source_len]) {
            core::mem::swap(source, destination);
            source.clear_secret();
        }
        debug_assert_eq!(self.0.len(), source_len);
        debug_assert_eq!(self.0.capacity(), source_capacity);
        debug_assert_eq!(self.0.as_ptr(), source_pointer);
        debug_assert_eq!(padded.0.len(), len);
        debug_assert_eq!(padded.0.capacity(), allocation_capacity);
        debug_assert_eq!(padded.0.as_ptr(), allocation_pointer);
        self.0.truncate(0);
        debug_assert_eq!(self.0.capacity(), source_capacity);
        debug_assert_eq!(self.0.as_ptr(), source_pointer);
        core::mem::swap(&mut self.0, &mut padded.0);
        debug_assert_eq!(self.0.len(), len);
        debug_assert_eq!(self.0.capacity(), allocation_capacity);
        debug_assert_eq!(self.0.as_ptr(), allocation_pointer);
        debug_assert!(padded.0.is_empty());
        debug_assert_eq!(padded.0.capacity(), source_capacity);
        debug_assert_eq!(padded.0.as_ptr(), source_pointer);
        Ok(())
    }
    fn split(mut self) -> Result<(Self, Self), GeneralizedBulletproofErrorV1> {
        if self.len() <= 1 || !self.len().is_multiple_of(2) {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let half = self.len() / 2;
        // Establish the complete zeroizing destination before moving any
        // private suffix slot. A fallible exact reserve keeps allocation
        // failure ahead of the first handoff and prevents later growth.
        let mut right = Self(Vec::new());
        right
            .0
            .try_reserve_exact(half)
            .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let allocation_capacity = right.0.capacity();
        if allocation_capacity < half {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        let allocation_pointer = right.0.as_ptr();
        for _ in 0..half {
            debug_assert!(right.0.len() < allocation_capacity);
            right.0.push(F::ZERO);
        }
        debug_assert_eq!(right.0.len(), half);
        debug_assert_eq!(right.0.capacity(), allocation_capacity);
        debug_assert_eq!(right.0.as_ptr(), allocation_pointer);
        // Swap each suffix value directly between initialized owner slots.
        // Clear the zeroed source slot through the scalar's erasure boundary
        // before truncation moves it beyond this owner's reachable length.
        for (source, destination) in self.0[half..].iter_mut().zip(&mut right.0) {
            core::mem::swap(source, destination);
            source.clear_secret();
        }
        debug_assert_eq!(right.0.len(), half);
        debug_assert_eq!(right.0.capacity(), allocation_capacity);
        debug_assert_eq!(right.0.as_ptr(), allocation_pointer);
        self.0.truncate(half);
        Ok((self, right))
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
    // Establish the complete retained zeroizing allocation before accepting
    // any entropy, so neither allocation nor insertion can fail after a
    // secret has been sampled.
    let mut result = ScalarVector(Vec::new());
    result
        .0
        .try_reserve_exact(len)
        .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    let allocation_capacity = result.0.capacity();
    if allocation_capacity < len {
        return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
    }
    let allocation_pointer = result.0.as_ptr();
    for _ in 0..len {
        debug_assert!(result.0.len() < allocation_capacity);
        result.0.push(F::ZERO);
    }
    debug_assert_eq!(result.0.len(), len);
    debug_assert_eq!(result.0.capacity(), allocation_capacity);
    debug_assert_eq!(result.0.as_ptr(), allocation_pointer);
    // Swap each sampled owner directly into its preinitialized destination.
    // The source receives zero and is cleared immediately before the next
    // entropy request.
    for destination in &mut result.0 {
        let mut sampled = random_scalar::<F, _>(rng)?;
        core::mem::swap(destination, sampled.expose_mut());
        drop(sampled);
    }
    debug_assert_eq!(result.0.len(), len);
    debug_assert_eq!(result.0.capacity(), allocation_capacity);
    debug_assert_eq!(result.0.as_ptr(), allocation_pointer);
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
    /// Construct an opening by moving a caller-owned mask slot into its final
    /// zeroizing owner. The source slot is replaced with zero before return.
    pub fn take_mask_from_slot(values: Vec<F>, mask: &mut F) -> Self {
        let mut opening = Self {
            values: ScalarVector(values),
            mask: F::ZERO,
        };
        core::mem::swap(&mut opening.mask, mask);
        opening
    }
    /// Move the committed values into their next zeroizing owner while
    /// retaining this opening's mask for the later polynomial response.
    fn take_values(&mut self) -> ScalarVector<F> {
        core::mem::replace(&mut self.values, ScalarVector(Vec::new()))
    }
    /// Construct an opening from committed values and its blinding scalar.
    pub fn new(values: Vec<F>, mut mask: F) -> Self {
        let incoming = BorrowedSecretScalarSlot(&mut mask);
        let owned = Self {
            values: ScalarVector(values),
            mask: incoming.expose_copy(),
        };
        drop(incoming);
        owned
    }
}
/// Owned opening of one scalar Pedersen commitment.
///
/// The type stays private so only the bounded witness constructor can create
/// it. Both named secret slots are cleared on success, error, and unwind.
struct ScalarCommitmentOpening<F: ProofScalar> {
    value: F,
    mask: F,
}
impl<F: ProofScalar> ScalarCommitmentOpening<F> {
    fn new(mut value: F, mut mask: F) -> Self {
        let incoming_value = BorrowedSecretScalarSlot(&mut value);
        let incoming_mask = BorrowedSecretScalarSlot(&mut mask);
        let owned = Self {
            value: incoming_value.expose_copy(),
            mask: incoming_mask.expose_copy(),
        };
        drop((incoming_value, incoming_mask));
        owned
    }
}
/// Guard for the tuple allocation accepted at the crate-private boundary.
struct ScalarCommitmentOpeningInputs<F: ProofScalar>(Vec<(F, F)>);
impl<F: ProofScalar> Drop for ScalarCommitmentOpeningInputs<F> {
    fn drop(&mut self) {
        for (value, mask) in &mut self.0 {
            value.clear_secret();
            mask.clear_secret();
        }
    }
}
impl<F: ProofScalar> Drop for ScalarCommitmentOpening<F> {
    fn drop(&mut self) {
        self.value.clear_secret();
        self.mask.clear_secret();
    }
}
/// Witness for a concrete generalized-Bulletproof arithmetic circuit.
pub struct ArithmeticCircuitWitness<S: ProofSuite> {
    a_l: ScalarVector<S::Scalar>,
    a_r: ScalarVector<S::Scalar>,
    a_o: ScalarVector<S::Scalar>,
    vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
    scalar_commitments: Vec<ScalarCommitmentOpening<S::Scalar>>,
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
        Self::new_with_scalar_commitments(a_l, a_r, vector_commitments, Vec::new())
    }
    /// Construct a witness which also opens the statement's scalar Pedersen
    /// commitments in statement order.
    pub(crate) fn new_with_scalar_commitments(
        a_l: Vec<S::Scalar>,
        a_r: Vec<S::Scalar>,
        vector_commitments: Vec<VectorCommitmentOpening<S::Scalar>>,
        scalar_commitments: Vec<(S::Scalar, S::Scalar)>,
    ) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let a_l = ScalarVector(a_l);
        let a_r = ScalarVector(a_r);
        let scalar_commitment_inputs = ScalarCommitmentOpeningInputs(scalar_commitments);
        let mut scalar_commitments = Vec::new();
        scalar_commitments
            .try_reserve_exact(scalar_commitment_inputs.0.len())
            .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        for (value, mask) in &scalar_commitment_inputs.0 {
            scalar_commitments.push(ScalarCommitmentOpening::new(*value, *mask));
        }
        drop(scalar_commitment_inputs);
        if a_l.len() != a_r.len() {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        let a_o = ScalarVector::product_from_borrowed(&a_l, &a_r)?;
        Ok(Self {
            a_l,
            a_r,
            a_o,
            vector_commitments,
            scalar_commitments,
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
enum VerifierConstraintSourceV1 {
    Materialized,
    ExactSmallCoefficient(ExactSmallCoefficientConstraintSourceV1),
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
            || witness.scalar_commitments.len() != self.scalar_commitments.len()
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
            for (scalar, point) in opening.values.0.iter().zip(self.generators.g_bold) {
                terms.push(scalar, point)?;
            }
            terms.push(&opening.mask, &self.generators.h)?;
            if !terms.evaluate()?.equals(commitment) {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
        }
        for (commitment, opening) in self
            .scalar_commitments
            .iter()
            .zip(&witness.scalar_commitments)
        {
            let mut terms = SecretMultiexpBuilder::<S>::new(2)?;
            terms.push(&opening.value, &self.generators.g)?;
            terms.push(&opening.mask, &self.generators.h)?;
            if !terms.evaluate()?.equals(commitment) {
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
            for (commitment, weight) in &constraint.wv {
                *evaluation.expose_mut() += witness.scalar_commitments[*commitment].value * *weight;
            }
            if !evaluation.is_zero() {
                return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
            }
        }
        let alpha = random_scalar::<S::Scalar, _>(rng)?;
        let beta = random_scalar::<S::Scalar, _>(rng)?;
        let rho = random_scalar::<S::Scalar, _>(rng)?;
        let ai = {
            let mut terms = SecretMultiexpBuilder::<S>::new((2 * n) + 1)?;
            for (scalar, point) in witness.a_l.0.iter().zip(self.generators.g_bold) {
                terms.push(scalar, point)?;
            }
            for (scalar, point) in witness.a_r.0.iter().zip(self.generators.h_bold) {
                terms.push(scalar, point)?;
            }
            terms.push(alpha.expose_ref(), &self.generators.h)?;
            terms.evaluate()?
        };
        let ao = {
            let mut terms = SecretMultiexpBuilder::<S>::new(n + 1)?;
            for (scalar, point) in witness.a_o.0.iter().zip(self.generators.g_bold) {
                terms.push(scalar, point)?;
            }
            terms.push(beta.expose_ref(), &self.generators.h)?;
            terms.evaluate()?
        };
        let s_l = random_scalar_vector::<S::Scalar, _>(rng, n)?;
        let s_r = random_scalar_vector::<S::Scalar, _>(rng, n)?;
        let s_point = {
            let mut terms = SecretMultiexpBuilder::<S>::new((2 * n) + 1)?;
            for (scalar, point) in s_l.0.iter().zip(self.generators.g_bold) {
                terms.push(scalar, point)?;
            }
            for (scalar, point) in s_r.0.iter().zip(self.generators.h_bold) {
                terms.push(scalar, point)?;
            }
            terms.push(rho.expose_ref(), &self.generators.h)?;
            terms.evaluate()?
        };
        if ai.is_identity() || ao.is_identity() || s_point.is_identity() {
            return Err(GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity);
        }
        transcript.push_point(ai.expose_ref())?;
        transcript.push_point(ao.expose_ref())?;
        transcript.push_point(s_point.expose_ref())?;
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
        let scaled_r_weights = r_weights * &y_inverse;
        let a_l = core::mem::replace(&mut witness.a_l, ScalarVector(Vec::new()));
        l[ilr] = a_l + &scaled_r_weights;
        drop(scaled_r_weights);
        l[io] = core::mem::replace(&mut witness.a_o, ScalarVector(Vec::new()));
        l[is] = s_l;
        let a_r = core::mem::replace(&mut witness.a_r, ScalarVector(Vec::new()));
        r[jlr] = l_weights + &(a_r * &y_powers);
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
        // The omitted center coefficient commits each scalar opening with
        // q_i = -sum_j z_j w_{j,i}. Its value is already fixed by the native
        // constraint precheck; its Pedersen mask is carried by tau_x below.
        let mut scalar_commitment_weights = ScalarVector::zero(self.scalar_commitments.len());
        for (constraint, z) in self.constraints.iter().zip(&z.0) {
            accumulate(&mut scalar_commitment_weights, &constraint.wv, -*z);
        }
        for (mut index, (opening, weights)) in witness
            .vector_commitments
            .iter_mut()
            .zip(cg_weights)
            .enumerate()
        {
            if index >= ilr {
                index += 1;
            }
            let reverse = ni - index;
            l[index] = opening.take_values();
            r[reverse] = weights;
        }
        let t_poly_len = 1 + (2 * (l.len() - 1));
        let mut t = ScalarVector::zero(t_poly_len);
        for (left_index, left) in l.iter().enumerate() {
            for (right_index, right) in r.iter().enumerate() {
                let product = left.inner_product(right.0.iter());
                t[left_index + right_index] += *product.expose_ref();
                drop(product);
            }
        }
        let tau_before = random_scalar_vector::<S::Scalar, _>(rng, ni)?;
        let tau_after = random_scalar_vector::<S::Scalar, _>(rng, t_poly_len - ni - 1)?;
        for (coefficient, mask) in t.0[..ni].iter().zip(&tau_before.0) {
            let mut terms = SecretMultiexpBuilder::<S>::new(2)?;
            terms.push(coefficient, &self.generators.g)?;
            terms.push(mask, &self.generators.h)?;
            let commitment = terms.evaluate()?;
            if commitment.is_identity() {
                return Err(GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity);
            }
            transcript.push_point(commitment.expose_ref())?;
        }
        for (coefficient, mask) in t.0[ni + 1..].iter().zip(&tau_after.0) {
            let mut terms = SecretMultiexpBuilder::<S>::new(2)?;
            terms.push(coefficient, &self.generators.g)?;
            terms.push(mask, &self.generators.h)?;
            let commitment = terms.evaluate()?;
            if commitment.is_identity() {
                return Err(GeneralizedBulletproofErrorV1::CircuitProverCommitmentIdentity);
            }
            transcript.push_point(commitment.expose_ref())?;
        }
        drop(t);
        let x = ScalarVector::powers(transcript.challenge()?, t_poly_len);
        let evaluate = |polynomial: &[ScalarVector<S::Scalar>]| {
            let mut result = ScalarVector::zero(n);
            for (index, coefficient) in polynomial.iter().enumerate() {
                result.add_scaled_assign(coefficient, &x[index]);
            }
            result
        };
        let l_eval = evaluate(&l);
        let r_eval = evaluate(&r);
        drop(l);
        drop(r);
        let t_caret = l_eval.inner_product(r_eval.0.iter());
        let mut tau_ni = SecretScalar::new(S::Scalar::ZERO);
        for (weight, opening) in scalar_commitment_weights
            .0
            .iter()
            .zip(&witness.scalar_commitments)
        {
            *tau_ni.expose_mut() += *weight * opening.mask;
        }
        drop(scalar_commitment_weights);
        // The omitted t[ni] commitment is reconstructed from the scalar
        // statement commitments. Vector-commitment masks instead contribute
        // to `u` below.
        let mut tau_x = SecretScalar::new(S::Scalar::ZERO);
        for (index, coefficient) in tau_before.0.iter().enumerate() {
            *tau_x.expose_mut() += *coefficient * x[index];
        }
        *tau_x.expose_mut() += *tau_ni.expose_ref() * x[ni];
        for (index, coefficient) in tau_after.0.iter().enumerate() {
            *tau_x.expose_mut() += *coefficient * x[ni + 1 + index];
        }
        drop(tau_before);
        drop(tau_after);
        drop(tau_ni);
        let mut u = SecretScalar::new(*alpha.expose_ref() * x[ilr]);
        *u.expose_mut() += *beta.expose_ref() * x[io];
        *u.expose_mut() += *rho.expose_ref() * x[is];
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
            p_terms.push(left, &self.generators.g_bold[index])?;
            p_terms.push_copy(y_inverse[index] * *right, self.generators.h_bold[index])?;
        }
        transcript.push_scalar(tau_x.expose_ref())?;
        transcript.push_scalar(u.expose_ref())?;
        transcript.push_scalar(t_caret.expose_ref())?;
        let ip_x = transcript.challenge()?;
        p_terms.push_copy(ip_x * *t_caret.expose_ref(), self.generators.g)?;
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
        self.verify_with_constraint_source(VerifierConstraintSourceV1::Materialized, transcript)
    }
    fn verify_with_constraint_source<T>(
        self,
        constraint_source: VerifierConstraintSourceV1,
        transcript: &mut T,
    ) -> Result<(), GeneralizedBulletproofErrorV1>
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
        let (y_inverse, z, exact_aggregates) = match constraint_source {
            VerifierConstraintSourceV1::Materialized => {
                let (y_inverse, z) = self.yz_challenges(y, z_one)?;
                (y_inverse, Some(z), None)
            }
            VerifierConstraintSourceV1::ExactSmallCoefficient(source) => {
                let y_inverse = y
                    .invert()
                    .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
                let y_inverse = ScalarVector::powers(y_inverse, n);
                (y_inverse, None, Some(source.aggregate(z_one)?))
            }
        };
        let (
            l_weights,
            r_weights,
            o_weights,
            exact_cg_weights,
            exact_v_weights,
            exact_constraint_product,
        ) = match exact_aggregates {
            Some(ExactSmallCoefficientAggregatesV1 {
                l_weights,
                r_weights,
                o_weights,
                vector_commitment_weights,
                scalar_commitment_weights,
                constraint_product,
            }) => (
                l_weights,
                r_weights,
                o_weights,
                Some(vector_commitment_weights),
                Some(scalar_commitment_weights),
                Some(constraint_product),
            ),
            None => {
                let z = z
                    .as_ref()
                    .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
                let mut l_weights = ScalarVector::zero(n);
                let mut r_weights = ScalarVector::zero(n);
                let mut o_weights = ScalarVector::zero(n);
                for (constraint, z) in self.constraints.iter().zip(&z.0) {
                    accumulate(&mut l_weights, &constraint.wl, *z);
                    accumulate(&mut r_weights, &constraint.wr, *z);
                    accumulate(&mut o_weights, &constraint.wo, *z);
                }
                (l_weights, r_weights, o_weights, None, None, None)
            }
        };
        let r_weights = r_weights * &y_inverse;
        let delta = r_weights.inner_product(l_weights.0.iter());
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
        let (mut v_weights, constraint_product) = match (exact_v_weights, exact_constraint_product)
        {
            (Some(v_weights), Some(constraint_product)) => (v_weights, constraint_product),
            (None, None) => {
                let z = z
                    .as_ref()
                    .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
                let mut v_weights = ScalarVector::zero(self.scalar_commitments.len());
                for (constraint, z) in self.constraints.iter().zip(&z.0) {
                    accumulate(&mut v_weights, &constraint.wv, -*z);
                }
                let constraint_product =
                    z.inner_product(self.constraints.iter().map(|constraint| &constraint.c));
                (v_weights, constraint_product)
            }
            _ => return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant),
        };
        v_weights = v_weights * x[ni];
        polynomial.g -= x[ni] * (*delta.expose_ref() - *constraint_product.expose_ref());
        drop((delta, constraint_product));
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
        let cg_weights = if let Some(weights) = exact_cg_weights {
            vec![weights]
        } else {
            let z = z
                .as_ref()
                .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
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
            cg_weights
        };
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
    mut p: SecretPoint<S::Point>,
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
        for (scalar, point) in a.0.iter().zip(generators.g_bold) {
            opening_terms.push(scalar, point)?;
        }
        for ((scalar, weight), point) in b.0.iter().zip(&h_bold_weights.0).zip(generators.h_bold) {
            opening_terms.push_copy(*scalar * *weight, *point)?;
        }
        let opening_product = a.inner_product(b.0.iter());
        opening_terms.push_copy(*opening_product.expose_ref() * u_scalar, generators.g)?;
        if !opening_terms.evaluate()?.equals(p.expose_ref()) {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        drop(opening_product);
    }
    // A one-element IPA has no challenge with which to fold the symbolic H
    // weight. Finish directly against the original fixed generators.
    if n == 1 {
        let mut folded_terms = SecretMultiexpBuilder::<S>::new(3)?;
        folded_terms.push(&a[0], &generators.g_bold[0])?;
        folded_terms.push_copy(b[0] * h_bold_weights[0], generators.h_bold[0])?;
        folded_terms.push_copy(a[0] * b[0] * u_scalar, generators.g)?;
        let folded = folded_terms.evaluate()?;
        if !folded.equals(p.expose_ref()) {
            return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
        }
        transcript.push_scalar(&a[0])?;
        transcript.push_scalar(&b[0])?;
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
    let c_left = a_left.inner_product(b_right.0.iter());
    let c_right = a_right.inner_product(b_left.0.iter());
    let left = {
        let mut terms = SecretMultiexpBuilder::<S>::new((2 * half) + 1)?;
        for (scalar, point) in a_left.0.iter().zip(g_right) {
            terms.push(scalar, point)?;
        }
        for ((scalar, weight), point) in b_right.0.iter().zip(h_weight_left).zip(h_left) {
            terms.push_copy(*scalar * *weight, *point)?;
        }
        terms.push_copy(*c_left.expose_ref() * u_scalar, generators.g)?;
        terms.evaluate()?
    };
    let right = {
        let mut terms = SecretMultiexpBuilder::<S>::new((2 * half) + 1)?;
        for (scalar, point) in a_right.0.iter().zip(g_left) {
            terms.push(scalar, point)?;
        }
        for ((scalar, weight), point) in b_left.0.iter().zip(h_weight_right).zip(h_right) {
            terms.push_copy(*scalar * *weight, *point)?;
        }
        terms.push_copy(*c_right.expose_ref() * u_scalar, generators.g)?;
        terms.evaluate()?
    };
    drop(c_left);
    drop(c_right);
    if left.is_identity() || right.is_identity() {
        return Err(GeneralizedBulletproofErrorV1::InnerProductRoundIdentity);
    }
    transcript.push_point(left.expose_ref())?;
    transcript.push_point(right.expose_ref())?;
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
    p.add_scaled_pair_assign(left, challenge.square(), right, inverse.square());
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
        let c_left = a_left.inner_product(b_right.0.iter());
        let c_right = a_right.inner_product(b_left.0.iter());
        let left = {
            let mut terms = SecretMultiexpBuilder::<S>::new((2 * half) + 1)?;
            for (scalar, point) in a_left.0.iter().zip(&g_right) {
                terms.push(scalar, point)?;
            }
            for (scalar, point) in b_right.0.iter().zip(&h_left) {
                terms.push(scalar, point)?;
            }
            terms.push_copy(*c_left.expose_ref() * u_scalar, generators.g)?;
            terms.evaluate()?
        };
        let right = {
            let mut terms = SecretMultiexpBuilder::<S>::new((2 * half) + 1)?;
            for (scalar, point) in a_right.0.iter().zip(&g_left) {
                terms.push(scalar, point)?;
            }
            for (scalar, point) in b_left.0.iter().zip(&h_right) {
                terms.push(scalar, point)?;
            }
            terms.push_copy(*c_right.expose_ref() * u_scalar, generators.g)?;
            terms.evaluate()?
        };
        drop(c_left);
        drop(c_right);
        if left.is_identity() || right.is_identity() {
            return Err(GeneralizedBulletproofErrorV1::InnerProductRoundIdentity);
        }
        transcript.push_point(left.expose_ref())?;
        transcript.push_point(right.expose_ref())?;
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
        p.add_scaled_pair_assign(left, challenge.square(), right, inverse.square());
        a = (a_left * challenge) + &(a_right * inverse);
        b = (b_left * inverse) + &(b_right * challenge);
    }
    if g_bold.len() != 1 || h_bold.len() != 1 || a.len() != 1 || b.len() != 1 {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }
    let mut folded_terms = SecretMultiexpBuilder::<S>::new(3)?;
    folded_terms.push(&a[0], &g_bold[0])?;
    folded_terms.push(&b[0], &h_bold[0])?;
    folded_terms.push_copy(a[0] * b[0] * u_scalar, generators.g)?;
    let folded = folded_terms.evaluate()?;
    if !folded.equals(p.expose_ref()) {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }
    transcript.push_scalar(&a[0])?;
    transcript.push_scalar(&b[0])?;
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
    include!("generalized_bulletproof_secret_cleanup_tests.rs");
    include!("generalized_bulletproof_secret_cleanup_more_tests.rs");
    include!("generalized_bulletproof_streaming_constraint_tests.rs");
}
