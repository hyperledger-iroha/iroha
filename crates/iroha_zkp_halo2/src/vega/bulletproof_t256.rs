//! Canonical T256 adapter for the reusable generalized-Bulletproof backend.
//!
//! This module freezes the T256 generator domains and typed transcript used by
//! exact ZK-AMS coefficient-membership arguments.  It deliberately does not
//! reuse FCMP's 32-byte cycle transcript: T256 proof points occupy 33 bytes and
//! its scalar field uses the canonical Vega little-endian proof encoding.
#![allow(dead_code)]
use super::{
    VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar, derive_t256_generators_v1,
    sponge::{Keccak256, keccak256},
};
use crate::generalized_bulletproof::{
    ArithmeticCircuitStatement, ArithmeticCircuitWitness, GeneralizedBulletproofErrorV1, LinComb,
    ProofGenerators, ProofPoint, ProofRandomSource, ProofScalar, ProofSuite, ProverTranscript,
    SecretMultiexpBuilder, Variable, VectorCommitmentOpening, VerifierTranscript,
};
use core::{
    marker::PhantomData,
    ops::{AddAssign, Neg, SubAssign},
};
use halo2curves::ff::Field as _;
use std::sync::OnceLock;
use thiserror::Error;
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
pub(super) const ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1: [u8; 32] = [
    0xbf, 0x81, 0xc8, 0x30, 0x91, 0xa4, 0x26, 0xbb, 0xcb, 0x2f, 0x75, 0x18, 0xad, 0x37, 0x16, 0x39,
    0x18, 0x10, 0xe5, 0x0b, 0x84, 0x8b, 0x38, 0xd0, 0xc2, 0xb3, 0xcd, 0x96, 0xaf, 0xf9, 0xa3, 0xf8,
];
pub(super) const ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1: usize = 16_384;
pub(super) const ZK_AMS_MEMBERSHIP_MAX_CHUNK_ORDINAL_V1: u16 = 47;
const ZK_AMS_MEMBERSHIP_WIRE_MAGIC_V1: [u8; 4] = *b"ZMBP";
const ZK_AMS_MEMBERSHIP_WIRE_VERSION_V1: u8 = 1;
const ZK_AMS_MEMBERSHIP_WIRE_HEADER_BYTES_V1: usize = 4 + 1 + 1 + 2 + 4 + 33 + 2;
const ZK_AMS_MEMBERSHIP_FIXED_PROOF_POINTS_V1: usize = 9;
const ZK_AMS_MEMBERSHIP_FIXED_PROOF_SCALARS_V1: usize = 5;
const ZK_AMS_MEMBERSHIP_PRE_IPA_SCALARS_V1: usize = 3;
const ZK_AMS_MEMBERSHIP_FINAL_IPA_SCALARS_V1: usize = 2;
const _: () = {
    assert!(
        ZK_AMS_MEMBERSHIP_PRE_IPA_SCALARS_V1 + ZK_AMS_MEMBERSHIP_FINAL_IPA_SCALARS_V1
            == ZK_AMS_MEMBERSHIP_FIXED_PROOF_SCALARS_V1
    );
};
fn clear_t256_scalar_encoding_bytes_v1(bytes: &mut [u8; 32]) {
    bytes.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *bytes);
    #[cfg(test)]
    T256_SCALAR_ENCODING_CLEARS_V1.with(|clears| clears.set(clears.get().saturating_add(1)));
}
fn clear_t256_point_encoding_bytes_v1(bytes: &mut [u8; 33]) {
    bytes.fill(0);
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    let _ = core::hint::black_box(&mut *bytes);
    #[cfg(test)]
    T256_POINT_ENCODING_CLEARS_V1.with(|clears| clears.set(clears.get().saturating_add(1)));
}
struct SecretT256ScalarEncodingV1([u8; 32]);
impl SecretT256ScalarEncodingV1 {
    fn new(value: &Scalar) -> Self {
        let mut owned = Self([0_u8; 32]);
        value.write_le_bytes_ref(&mut owned.0);
        owned
    }
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}
impl Drop for SecretT256ScalarEncodingV1 {
    fn drop(&mut self) {
        clear_t256_scalar_encoding_bytes_v1(&mut self.0);
    }
}
/// Lend one borrowed T256 scalar's owned canonical encoding to an explicit
/// publication boundary and erase the bytes after success, error, or unwind.
pub(super) fn with_borrowed_t256_scalar_encoding_v1<T>(
    value: &Scalar,
    use_encoding: impl FnOnce(&[u8; 32]) -> T,
) -> T {
    let encoded = SecretT256ScalarEncodingV1::new(value);
    let result = use_encoding(encoded.as_ref());
    drop(encoded);
    result
}
/// Owner for one borrowed private T256 point's canonical proof encoding.
///
/// Construction starts with an owned zero buffer so even identity/error exits
/// run the same byte eraser. The owner stays live across all transcript and
/// proof writes and is also erased if one of those writes unwinds.
pub(super) struct SecretT256PointEncodingV1([u8; 33]);
impl SecretT256PointEncodingV1 {
    pub(super) fn new(point: &Point) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let mut owned = Self([0_u8; 33]);
        point
            .write_non_identity_wire_bytes_ref(&mut owned.0)
            .map_err(|_| GeneralizedBulletproofErrorV1::PointIdentity)?;
        Ok(owned)
    }
    /// Encode the generalized-proof identity sentinel or one borrowed
    /// non-identity point without copying the point by value.
    pub(super) fn new_allow_identity(point: &Point) -> Result<Self, GeneralizedBulletproofErrorV1> {
        let mut owned = Self([0_u8; 33]);
        if point == &Point::identity() {
            owned.0[0] = 0x40;
        } else {
            point
                .write_non_identity_wire_bytes_ref(&mut owned.0)
                .map_err(|_| GeneralizedBulletproofErrorV1::PointIdentity)?;
        }
        Ok(owned)
    }
    pub(super) fn as_ref(&self) -> &[u8; 33] {
        &self.0
    }
}
impl Drop for SecretT256PointEncodingV1 {
    fn drop(&mut self) {
        clear_t256_point_encoding_bytes_v1(&mut self.0);
    }
}
#[cfg(test)]
std::thread_local! {
    static T256_SCALAR_ENCODING_CLEARS_V1: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    static T256_POINT_ENCODING_CLEARS_V1: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}
/// Best-effort erased named copy of a T256 prover secret.
///
/// The public scalar type is intentionally `Copy` for field arithmetic, so
/// every function which accepts a secret blinding by value wraps and clears
/// its own stack instance.  Owned witness vectors are cleared independently by
/// the generalized-Bulletproof RAII containers.
pub(super) struct ZeroizingT256ScalarCopyV1(Scalar);
struct BorrowedT256ScalarCopyV1<'a>(&'a mut Scalar);
impl BorrowedT256ScalarCopyV1<'_> {
    fn get(&self) -> Scalar {
        *self.0
    }
}
impl Drop for BorrowedT256ScalarCopyV1<'_> {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}
impl ZeroizingT256ScalarCopyV1 {
    pub(super) fn new(mut value: Scalar) -> Self {
        let incoming = BorrowedT256ScalarCopyV1(&mut value);
        let owned = Self(incoming.get());
        drop(incoming);
        owned
    }
    /// Transfer a named scalar slot into this owner and clear the source slot
    /// on normal return or unwind.
    pub(super) fn take(value: &mut Scalar) -> Self {
        let incoming = BorrowedT256ScalarCopyV1(value);
        let owned = Self(incoming.get());
        drop(incoming);
        owned
    }
    pub(super) fn get(&self) -> Scalar {
        self.0
    }
    pub(super) fn as_ref(&self) -> &Scalar {
        &self.0
    }
    /// Accumulate one borrowed product without materializing either operand or
    /// the running secret in a caller-owned `Copy` slot.
    pub(super) fn add_product_assign(&mut self, left: &Scalar, right: &Scalar) {
        self.0 += *left * *right;
    }
}
impl Drop for ZeroizingT256ScalarCopyV1 {
    fn drop(&mut self) {
        self.0.clear_secret();
    }
}
/// Audited RAII owner for a vector of secret T256 scalars.
///
/// This is crate-private so other native T256 protocols reuse the same
/// clearing boundary instead of growing protocol-local best-effort erasers.
pub(super) struct ZeroizingT256ScalarVecV1(Vec<Scalar>);
impl ZeroizingT256ScalarVecV1 {
    #[cfg(test)]
    pub(super) fn new(values: Vec<Scalar>) -> Self {
        Self(values)
    }
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }
    pub(super) fn push(&mut self, mut value: Scalar) {
        let incoming = BorrowedT256ScalarCopyV1(&mut value);
        self.0.push(incoming.get());
        // `Scalar` is `Copy`, so the retained vector element does not consume
        // this callee-owned parameter slot. Clear it on success; the guard
        // performs the same erasure if allocation unwinds inside `Vec::push`.
        drop(incoming);
    }
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn as_slice(&self) -> &[Scalar] {
        &self.0
    }
    pub(super) fn as_mut_slice(&mut self) -> &mut [Scalar] {
        &mut self.0
    }
    /// Clear the removed suffix before reducing the logical vector length.
    pub(super) fn clear_and_truncate(&mut self, len: usize) {
        if len < self.0.len() {
            for scalar in &mut self.0[len..] {
                scalar.clear_secret();
            }
            self.0.truncate(len);
        }
    }
    pub(super) fn take(&mut self) -> Vec<Scalar> {
        core::mem::take(&mut self.0)
    }
}
impl core::fmt::Debug for ZeroizingT256ScalarVecV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZeroizingT256ScalarVecV1([REDACTED])")
    }
}
impl Drop for ZeroizingT256ScalarVecV1 {
    fn drop(&mut self) {
        for scalar in &mut self.0 {
            scalar.clear_secret();
        }
        #[cfg(test)]
        if self.0.iter().all(|scalar| scalar.is_zero()) {
            ZEROIZING_T256_SCALAR_VEC_DROPS_V1.with(|drops| {
                drops.set(drops.get().saturating_add(1));
            });
        }
    }
}
#[cfg(test)]
std::thread_local! {
    static ZEROIZING_T256_SCALAR_VEC_DROPS_V1: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}
#[cfg(test)]
pub(super) fn zeroizing_t256_scalar_vec_drop_count_v1() -> usize {
    ZEROIZING_T256_SCALAR_VEC_DROPS_V1.with(core::cell::Cell::get)
}
/// Exact small-coefficient set certified by one membership proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum ZkAmsT256MembershipBoundV1 {
    /// Coefficients are members of `{-1, 0, 1}`.
    One = 1,
    /// Coefficients are members of `{-2, -1, 0, 1, 2}`.
    Two = 2,
}
impl ZkAmsT256MembershipBoundV1 {
    fn gates_per_coefficient(self) -> usize {
        match self {
            Self::One => 2,
            Self::Two => 3,
        }
    }
    fn constraints_per_coefficient(self) -> usize {
        match self {
            Self::One => 5,
            Self::Two => 7,
        }
    }
    fn contains(self, coefficient: i8) -> bool {
        coefficient.unsigned_abs() <= self as u8
    }
}
impl TryFrom<u8> for ZkAmsT256MembershipBoundV1 {
    type Error = ZkAmsT256MembershipErrorV1;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            _ => Err(ZkAmsT256MembershipErrorV1::WireEncoding),
        }
    }
}
/// Stable failure classes for exact T256 coefficient-membership proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum ZkAmsT256MembershipErrorV1 {
    #[error("ZK-AMS T256 membership context must be non-zero")]
    Context,
    #[error("ZK-AMS T256 membership chunk ordinal is outside the governed release shape")]
    ChunkOrdinal,
    #[error("ZK-AMS T256 membership coefficient count is invalid")]
    CoefficientCount,
    #[error("ZK-AMS T256 membership coefficient {index} is outside the claimed set")]
    CoefficientOutOfRange { index: usize },
    #[error("ZK-AMS T256 membership commitment blinding must be non-zero")]
    Blinding,
    #[error("ZK-AMS T256 membership commitment is the identity")]
    CommitmentIdentity,
    #[error("ZK-AMS T256 membership proof statement does not match the expected axes")]
    StatementMismatch,
    #[error("ZK-AMS T256 membership wire encoding is invalid")]
    WireEncoding,
    #[error("ZK-AMS T256 membership proof length is invalid")]
    ProofLength,
    #[error(transparent)]
    Backend(#[from] GeneralizedBulletproofErrorV1),
}
/// Canonical public evidence for one coefficient chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsT256MembershipProofV1 {
    bound: ZkAmsT256MembershipBoundV1,
    chunk_ordinal: u16,
    coefficient_count: u32,
    commitment: Point,
    proof: Vec<u8>,
}
struct BorrowedZkAmsT256MembershipProofWireV1<'a> {
    bound: ZkAmsT256MembershipBoundV1,
    chunk_ordinal: u16,
    coefficient_count: u32,
    commitment: Point,
    proof: &'a [u8],
    padded_gates: usize,
}
fn borrow_zk_ams_t256_membership_proof_wire_exact_v1(
    bytes: &[u8],
) -> Result<BorrowedZkAmsT256MembershipProofWireV1<'_>, ZkAmsT256MembershipErrorV1> {
    if bytes.len() < ZK_AMS_MEMBERSHIP_WIRE_HEADER_BYTES_V1
        || bytes[..4] != ZK_AMS_MEMBERSHIP_WIRE_MAGIC_V1
        || bytes[4] != ZK_AMS_MEMBERSHIP_WIRE_VERSION_V1
    {
        return Err(ZkAmsT256MembershipErrorV1::WireEncoding);
    }
    let bound = ZkAmsT256MembershipBoundV1::try_from(bytes[5])?;
    let chunk_ordinal = u16::from_be_bytes(
        bytes[6..8]
            .try_into()
            .map_err(|_| ZkAmsT256MembershipErrorV1::WireEncoding)?,
    );
    if chunk_ordinal > ZK_AMS_MEMBERSHIP_MAX_CHUNK_ORDINAL_V1 {
        return Err(ZkAmsT256MembershipErrorV1::ChunkOrdinal);
    }
    let coefficient_count = u32::from_be_bytes(
        bytes[8..12]
            .try_into()
            .map_err(|_| ZkAmsT256MembershipErrorV1::WireEncoding)?,
    );
    let coefficient_count_usize = usize::try_from(coefficient_count)
        .map_err(|_| ZkAmsT256MembershipErrorV1::CoefficientCount)?;
    let (_, padded_gates, _) = membership_shape(coefficient_count_usize, bound)?;
    let expected_proof_len = membership_proof_len(padded_gates)?;
    let commitment = Point::from_non_identity_wire_bytes_exact(&bytes[12..45])
        .map_err(|_| ZkAmsT256MembershipErrorV1::WireEncoding)?;
    let encoded_proof_len = usize::from(u16::from_be_bytes(
        bytes[45..47]
            .try_into()
            .map_err(|_| ZkAmsT256MembershipErrorV1::WireEncoding)?,
    ));
    let expected_wire_len = ZK_AMS_MEMBERSHIP_WIRE_HEADER_BYTES_V1
        .checked_add(encoded_proof_len)
        .ok_or(ZkAmsT256MembershipErrorV1::WireEncoding)?;
    if encoded_proof_len != expected_proof_len || bytes.len() != expected_wire_len {
        return Err(ZkAmsT256MembershipErrorV1::ProofLength);
    }
    Ok(BorrowedZkAmsT256MembershipProofWireV1 {
        bound,
        chunk_ordinal,
        coefficient_count,
        commitment,
        proof: &bytes[ZK_AMS_MEMBERSHIP_WIRE_HEADER_BYTES_V1..],
        padded_gates,
    })
}
struct CanonicalMembershipProofSyntaxCursorV1<'a> {
    proof: &'a [u8],
    cursor: usize,
}
impl CanonicalMembershipProofSyntaxCursorV1<'_> {
    fn read_point(&mut self) -> Result<(), GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(Point::POINT_BYTES)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let bytes =
            self.proof
                .get(self.cursor..end)
                .ok_or(GeneralizedBulletproofErrorV1::ProofLength {
                    actual: self.proof.len(),
                    expected: end,
                })?;
        Point::from_non_identity_wire_bytes_exact(bytes).map_err(|error| {
            if matches!(error, super::VegaCurveError::IdentityPoint) {
                GeneralizedBulletproofErrorV1::PointIdentity
            } else {
                GeneralizedBulletproofErrorV1::PointEncoding
            }
        })?;
        self.cursor = end;
        Ok(())
    }
    fn read_scalar(&mut self) -> Result<(), GeneralizedBulletproofErrorV1> {
        let end = self
            .cursor
            .checked_add(32)
            .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
        let bytes = self
            .proof
            .get(self.cursor..end)
            .ok_or(GeneralizedBulletproofErrorV1::ProofLength {
                actual: self.proof.len(),
                expected: end,
            })?
            .try_into()
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        Scalar::from_le_bytes_exact(bytes)
            .map_err(|_| GeneralizedBulletproofErrorV1::ScalarEncoding)?;
        self.cursor = end;
        Ok(())
    }
    fn finish(self) -> Result<(), GeneralizedBulletproofErrorV1> {
        if self.cursor != self.proof.len() {
            return Err(GeneralizedBulletproofErrorV1::TranscriptConsumption);
        }
        Ok(())
    }
}
fn preflight_generalized_membership_proof_syntax_v1(
    proof: &[u8],
    padded_gates: usize,
) -> Result<(), ZkAmsT256MembershipErrorV1> {
    if padded_gates == 0 || !padded_gates.is_power_of_two() {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientCount);
    }
    let ipa_rounds = usize::try_from(padded_gates.ilog2())
        .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    let mut cursor = CanonicalMembershipProofSyntaxCursorV1 { proof, cursor: 0 };
    for _ in 0..ZK_AMS_MEMBERSHIP_FIXED_PROOF_POINTS_V1 {
        cursor.read_point()?;
    }
    for _ in 0..ZK_AMS_MEMBERSHIP_PRE_IPA_SCALARS_V1 {
        cursor.read_scalar()?;
    }
    for _ in 0..ipa_rounds
        .checked_mul(2)
        .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?
    {
        cursor.read_point()?;
    }
    for _ in 0..ZK_AMS_MEMBERSHIP_FINAL_IPA_SCALARS_V1 {
        cursor.read_scalar()?;
    }
    cursor.finish()?;
    Ok(())
}
/// Allocation-free canonical syntax preflight for one exact membership chunk.
///
/// This validates the fixed wrapper, outer commitment, all generalized-
/// Bulletproof points and scalars, and exact terminal consumption. It does not
/// evaluate the proof equations or allocate an owned proof buffer.
pub(super) fn preflight_zk_ams_t256_membership_chunk_wire_v1(
    bytes: &[u8],
    expected_chunk_ordinal: u16,
    expected_bound: ZkAmsT256MembershipBoundV1,
    expected_coefficient_count: usize,
) -> Result<Point, ZkAmsT256MembershipErrorV1> {
    let borrowed = borrow_zk_ams_t256_membership_proof_wire_exact_v1(bytes)?;
    if borrowed.bound != expected_bound
        || borrowed.chunk_ordinal != expected_chunk_ordinal
        || usize::try_from(borrowed.coefficient_count).ok() != Some(expected_coefficient_count)
    {
        return Err(ZkAmsT256MembershipErrorV1::StatementMismatch);
    }
    preflight_generalized_membership_proof_syntax_v1(borrowed.proof, borrowed.padded_gates)?;
    Ok(borrowed.commitment)
}
impl ZkAmsT256MembershipProofV1 {
    pub(super) fn bound(&self) -> ZkAmsT256MembershipBoundV1 {
        self.bound
    }
    pub(super) fn chunk_ordinal(&self) -> u16 {
        self.chunk_ordinal
    }
    pub(super) fn coefficient_count(&self) -> u32 {
        self.coefficient_count
    }
    pub(super) fn commitment(&self) -> Point {
        self.commitment
    }
    pub(super) fn proof_bytes(&self) -> &[u8] {
        &self.proof
    }
    pub(super) fn to_wire_bytes(&self) -> Vec<u8> {
        let proof_len = u16::try_from(self.proof.len())
            .expect("governed T256 membership proof length fits u16");
        let mut bytes =
            Vec::with_capacity(ZK_AMS_MEMBERSHIP_WIRE_HEADER_BYTES_V1 + self.proof.len());
        bytes.extend_from_slice(&ZK_AMS_MEMBERSHIP_WIRE_MAGIC_V1);
        bytes.push(ZK_AMS_MEMBERSHIP_WIRE_VERSION_V1);
        bytes.push(self.bound as u8);
        bytes.extend_from_slice(&self.chunk_ordinal.to_be_bytes());
        bytes.extend_from_slice(&self.coefficient_count.to_be_bytes());
        bytes.extend_from_slice(
            &self
                .commitment
                .to_non_identity_wire_bytes()
                .expect("membership evidence excludes identity commitments"),
        );
        bytes.extend_from_slice(&proof_len.to_be_bytes());
        bytes.extend_from_slice(&self.proof);
        bytes
    }
    pub(super) fn from_wire_bytes_exact(bytes: &[u8]) -> Result<Self, ZkAmsT256MembershipErrorV1> {
        let borrowed = borrow_zk_ams_t256_membership_proof_wire_exact_v1(bytes)?;
        Ok(Self {
            bound: borrowed.bound,
            chunk_ordinal: borrowed.chunk_ordinal,
            coefficient_count: borrowed.coefficient_count,
            commitment: borrowed.commitment,
            proof: borrowed.proof.to_vec(),
        })
    }
}
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
        Scalar::clear_secret(self);
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
    fn conditional_select(a: &Self, b: &Self, choice: u8) -> Self {
        Point::conditional_select(a, b, choice)
    }
    fn clear_secret(&mut self) {
        Point::clear_secret(self);
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
fn membership_shape(
    coefficient_count: usize,
    bound: ZkAmsT256MembershipBoundV1,
) -> Result<(usize, usize, usize), ZkAmsT256MembershipErrorV1> {
    if coefficient_count == 0 || coefficient_count > ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientCount);
    }
    let actual_gates = coefficient_count
        .checked_mul(bound.gates_per_coefficient())
        .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    let padded_gates = actual_gates
        .checked_next_power_of_two()
        .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    // The committed opening is padded to the circuit dimension.  Every
    // coordinate outside the public coefficient prefix must therefore be
    // constrained to zero; otherwise a prover can commit to a different,
    // longer vector while claiming this `coefficient_count`.
    let visible_constraint_count = coefficient_count
        .checked_mul(bound.constraints_per_coefficient())
        .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    let padded_tail_constraint_count = padded_gates
        .checked_sub(coefficient_count)
        .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
    let constraint_count = visible_constraint_count
        .checked_add(padded_tail_constraint_count)
        .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    if padded_gates > T256_BP_MAX_GATES_V1 {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientCount);
    }
    Ok((actual_gates, padded_gates, constraint_count))
}
fn membership_proof_len(padded_gates: usize) -> Result<usize, ZkAmsT256MembershipErrorV1> {
    if padded_gates == 0 || !padded_gates.is_power_of_two() {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientCount);
    }
    let ipa_rounds = usize::try_from(padded_gates.ilog2())
        .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    let points = ZK_AMS_MEMBERSHIP_FIXED_PROOF_POINTS_V1
        .checked_add(
            ipa_rounds
                .checked_mul(2)
                .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?,
        )
        .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    points
        .checked_mul(Point::POINT_BYTES)
        .and_then(|bytes| {
            ZK_AMS_MEMBERSHIP_FIXED_PROOF_SCALARS_V1
                .checked_mul(32)
                .and_then(|scalar_bytes| bytes.checked_add(scalar_bytes))
        })
        .ok_or(GeneralizedBulletproofErrorV1::ResourceOverflow.into())
}
fn boolean_constraints(gate: usize) -> [LinComb<Scalar>; 2] {
    [
        LinComb::empty()
            .term(Scalar::ONE, Variable::aL(gate))
            .term(-Scalar::ONE, Variable::aR(gate)),
        LinComb::empty()
            .term(Scalar::ONE, Variable::aO(gate))
            .term(-Scalar::ONE, Variable::aL(gate)),
    ]
}
fn membership_constraints(
    coefficient_count: usize,
    bound: ZkAmsT256MembershipBoundV1,
) -> Result<(usize, Vec<LinComb<Scalar>>), ZkAmsT256MembershipErrorV1> {
    let (_, padded_gates, constraint_count) = membership_shape(coefficient_count, bound)?;
    let mut constraints = Vec::new();
    constraints
        .try_reserve_exact(constraint_count)
        .map_err(|_| GeneralizedBulletproofErrorV1::ResourceOverflow)?;
    for coefficient_index in 0..coefficient_count {
        let first_gate = coefficient_index * bound.gates_per_coefficient();
        constraints.extend(boolean_constraints(first_gate));
        constraints.extend(boolean_constraints(first_gate + 1));
        match bound {
            ZkAmsT256MembershipBoundV1::One => {
                constraints.push(
                    LinComb::empty()
                        .term(Scalar::ONE, Variable::aL(first_gate))
                        .term(-Scalar::ONE, Variable::aL(first_gate + 1))
                        .term(
                            -Scalar::ONE,
                            Variable::CG {
                                commitment: 0,
                                index: coefficient_index,
                            },
                        ),
                );
            }
            ZkAmsT256MembershipBoundV1::Two => {
                constraints.extend(boolean_constraints(first_gate + 2));
                constraints.push(
                    LinComb::empty()
                        .term(Scalar::ONE, Variable::aL(first_gate))
                        .term(Scalar::ONE, Variable::aL(first_gate + 1))
                        .term(-Scalar::from_u64(2), Variable::aL(first_gate + 2))
                        .term(
                            -Scalar::ONE,
                            Variable::CG {
                                commitment: 0,
                                index: coefficient_index,
                            },
                        ),
                );
            }
        }
    }
    for padded_index in coefficient_count..padded_gates {
        constraints.push(LinComb::empty().term(
            Scalar::ONE,
            Variable::CG {
                commitment: 0,
                index: padded_index,
            },
        ));
    }
    debug_assert_eq!(constraints.len(), constraint_count);
    Ok((padded_gates, constraints))
}
fn signed_scalar(coefficient: i8) -> Scalar {
    // Obtain the absolute value and sign with arithmetic masks, then select
    // the field sign algebraically. Valid membership coefficients are in
    // -2..=2, but the expression is total over i8 and never indexes memory or
    // branches on the witness.
    let signed = i16::from(coefficient);
    let sign_mask = signed >> 15;
    let magnitude = ((signed ^ sign_mask) - sign_mask) as u64;
    let sign = u64::from((coefficient as u8) >> 7);
    let magnitude = Scalar::from_u64(magnitude);
    magnitude - (Scalar::from_u64(sign) * (magnitude + magnitude))
}
fn append_boolean_witness(
    a_l: &mut ZeroizingT256ScalarVecV1,
    a_r: &mut ZeroizingT256ScalarVecV1,
    bit: bool,
) {
    let bit = ZeroizingT256ScalarCopyV1::new(Scalar::from_u64(u64::from(bit)));
    a_l.push(bit.get());
    a_r.push(bit.get());
}
fn membership_commitment_for_suite<S>(
    coefficients: &[i8],
    bound: ZkAmsT256MembershipBoundV1,
    blinding: &Scalar,
) -> Result<
    (
        crate::generalized_bulletproof::SecretPoint<Point>,
        ZeroizingT256ScalarVecV1,
        usize,
    ),
    ZkAmsT256MembershipErrorV1,
>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    let blinding = ZeroizingT256ScalarCopyV1::new(*blinding);
    if blinding.get().is_zero() {
        return Err(ZkAmsT256MembershipErrorV1::Blinding);
    }
    for (index, coefficient) in coefficients.iter().copied().enumerate() {
        if !bound.contains(coefficient) {
            return Err(ZkAmsT256MembershipErrorV1::CoefficientOutOfRange { index });
        }
    }
    let (actual_gates, padded_gates, _) = membership_shape(coefficients.len(), bound)?;
    let generators = S::generators().reduce(padded_gates)?;
    let mut values = ZeroizingT256ScalarVecV1::with_capacity(coefficients.len());
    for coefficient in coefficients.iter().copied() {
        values.push(signed_scalar(coefficient));
    }
    let mut commitment_terms = SecretMultiexpBuilder::<S>::new(values.0.len() + 1)?;
    for (scalar, point) in values.0.iter().zip(generators.g_bold) {
        commitment_terms.push(scalar, point)?;
    }
    commitment_terms.push(blinding.as_ref(), &generators.h)?;
    let commitment = commitment_terms.evaluate()?;
    if commitment.is_identity() {
        return Err(ZkAmsT256MembershipErrorV1::CommitmentIdentity);
    }
    Ok((commitment, values, actual_gates))
}
fn membership_witness<S>(
    coefficients: &[i8],
    bound: ZkAmsT256MembershipBoundV1,
    blinding: &Scalar,
) -> Result<
    (
        crate::generalized_bulletproof::SecretPoint<Point>,
        ArithmeticCircuitWitness<S>,
    ),
    ZkAmsT256MembershipErrorV1,
>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    let blinding = ZeroizingT256ScalarCopyV1::new(*blinding);
    let (secret_commitment, mut values, actual_gates) =
        membership_commitment_for_suite::<S>(coefficients, bound, blinding.as_ref())?;
    let mut a_l = ZeroizingT256ScalarVecV1::with_capacity(actual_gates);
    let mut a_r = ZeroizingT256ScalarVecV1::with_capacity(actual_gates);
    for coefficient in coefficients.iter().copied() {
        match bound {
            ZkAmsT256MembershipBoundV1::One => {
                append_boolean_witness(&mut a_l, &mut a_r, coefficient == 1);
                append_boolean_witness(&mut a_l, &mut a_r, coefficient == -1);
            }
            ZkAmsT256MembershipBoundV1::Two => {
                append_boolean_witness(
                    &mut a_l,
                    &mut a_r,
                    (coefficient == -1) | (coefficient >= 1),
                );
                append_boolean_witness(&mut a_l, &mut a_r, coefficient == 2);
                append_boolean_witness(&mut a_l, &mut a_r, coefficient < 0);
            }
        }
    }
    // The generalized prover expands this visible prefix to `padded_gates`.
    // `membership_constraints` fixes that complete added tail to zero; the
    // separately padded arithmetic-gate wires carry no public vector claim.
    let openings = vec![VectorCommitmentOpening::new(values.take(), blinding.get())];
    let witness = ArithmeticCircuitWitness::<S>::new(a_l.take(), a_r.take(), openings)?;
    Ok((secret_commitment, witness))
}
fn prove_membership_chunk_for_suite<S, R>(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    chunk_ordinal: u16,
    bound: ZkAmsT256MembershipBoundV1,
    coefficients: &[i8],
    blinding: &Scalar,
    rng: &mut R,
) -> Result<(ZkAmsT256MembershipProofV1, [u8; 32]), ZkAmsT256MembershipErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
    R: ProofRandomSource,
{
    let blinding = ZeroizingT256ScalarCopyV1::new(*blinding);
    if context_digest == [0; 32] || generator_basis_digest == [0; 32] {
        return Err(ZkAmsT256MembershipErrorV1::Context);
    }
    if chunk_ordinal > ZK_AMS_MEMBERSHIP_MAX_CHUNK_ORDINAL_V1 {
        return Err(ZkAmsT256MembershipErrorV1::ChunkOrdinal);
    }
    let (padded_gates, constraints) = membership_constraints(coefficients.len(), bound)?;
    let expected_proof_len = membership_proof_len(padded_gates)?;
    let (secret_commitment, witness) =
        membership_witness::<S>(coefficients, bound, blinding.as_ref())?;
    let mut transcript = T256BulletproofProverTranscriptV1::<S>::new(
        context_digest,
        generator_basis_digest,
        chunk_ordinal,
        bound as u8,
        secret_commitment.expose_ref(),
    )?;
    ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        constraints,
        vec![*secret_commitment.expose_ref()],
        Vec::new(),
    )?
    .prove(rng, &mut transcript, witness)?;
    let (proof, transcript_digest) = transcript.complete();
    if proof.len() != expected_proof_len {
        return Err(ZkAmsT256MembershipErrorV1::ProofLength);
    }
    let commitment = *secret_commitment.expose_ref();
    Ok((
        ZkAmsT256MembershipProofV1 {
            bound,
            chunk_ordinal,
            coefficient_count: u32::try_from(coefficients.len())
                .map_err(|_| ZkAmsT256MembershipErrorV1::CoefficientCount)?,
            commitment,
            proof,
        },
        transcript_digest,
    ))
}
fn verify_membership_chunk_for_suite<S>(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    expected_chunk_ordinal: u16,
    expected_bound: ZkAmsT256MembershipBoundV1,
    expected_coefficient_count: usize,
    evidence: &ZkAmsT256MembershipProofV1,
) -> Result<[u8; 32], ZkAmsT256MembershipErrorV1>
where
    S: ProofSuite<Scalar = Scalar, Point = Point>,
{
    if context_digest == [0; 32] || generator_basis_digest == [0; 32] {
        return Err(ZkAmsT256MembershipErrorV1::Context);
    }
    if expected_chunk_ordinal > ZK_AMS_MEMBERSHIP_MAX_CHUNK_ORDINAL_V1 {
        return Err(ZkAmsT256MembershipErrorV1::ChunkOrdinal);
    }
    if evidence.bound != expected_bound
        || evidence.chunk_ordinal != expected_chunk_ordinal
        || usize::try_from(evidence.coefficient_count).ok() != Some(expected_coefficient_count)
    {
        return Err(ZkAmsT256MembershipErrorV1::StatementMismatch);
    }
    let (padded_gates, constraints) =
        membership_constraints(expected_coefficient_count, expected_bound)?;
    if evidence.proof.len() != membership_proof_len(padded_gates)? {
        return Err(ZkAmsT256MembershipErrorV1::ProofLength);
    }
    let mut transcript = T256BulletproofVerifierTranscriptV1::<S>::new(
        context_digest,
        generator_basis_digest,
        expected_chunk_ordinal,
        expected_bound as u8,
        evidence.commitment,
        &evidence.proof,
    )?;
    ArithmeticCircuitStatement::new(
        S::generators().reduce(padded_gates)?,
        constraints,
        vec![evidence.commitment],
        Vec::new(),
    )?
    .verify(&mut transcript)?;
    Ok(transcript.finish()?)
}
/// Prove exact membership for one release-shape 16,384-coefficient chunk.
pub(super) fn prove_zk_ams_t256_membership_chunk_v1<R: ProofRandomSource>(
    context_digest: [u8; 32],
    chunk_ordinal: u16,
    bound: ZkAmsT256MembershipBoundV1,
    coefficients: &[i8],
    blinding: &Scalar,
    rng: &mut R,
) -> Result<(ZkAmsT256MembershipProofV1, [u8; 32]), ZkAmsT256MembershipErrorV1> {
    let blinding = ZeroizingT256ScalarCopyV1::new(*blinding);
    if coefficients.len() != ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientCount);
    }
    prove_membership_chunk_for_suite::<ZkAmsT256BulletproofSuiteV1, _>(
        context_digest,
        zk_ams_t256_bulletproof_generator_basis_digest_v1(),
        chunk_ordinal,
        bound,
        coefficients,
        blinding.as_ref(),
        rng,
    )
}
/// Commit one exact release-shape membership chunk under the canonical T256 basis.
///
/// This is the shared commitment primitive for state-owned opening checks and
/// membership proof construction. It emits no proof and accepts neither a
/// partial chunk nor an unchecked coefficient or blinding.
pub(super) fn commit_zk_ams_t256_membership_chunk_v1(
    bound: ZkAmsT256MembershipBoundV1,
    coefficients: &[i8],
    blinding: &Scalar,
) -> Result<Point, ZkAmsT256MembershipErrorV1> {
    let blinding = ZeroizingT256ScalarCopyV1::new(*blinding);
    if coefficients.len() != ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 {
        return Err(ZkAmsT256MembershipErrorV1::CoefficientCount);
    }
    membership_commitment_for_suite::<ZkAmsT256BulletproofSuiteV1>(
        coefficients,
        bound,
        blinding.as_ref(),
    )
    .map(|(commitment, _values, _actual_gates)| *commitment.expose_ref())
}
/// Verify exact membership for one release-shape 16,384-coefficient chunk.
pub(super) fn verify_zk_ams_t256_membership_chunk_v1(
    context_digest: [u8; 32],
    expected_chunk_ordinal: u16,
    expected_bound: ZkAmsT256MembershipBoundV1,
    evidence: &ZkAmsT256MembershipProofV1,
) -> Result<[u8; 32], ZkAmsT256MembershipErrorV1> {
    verify_membership_chunk_for_suite::<ZkAmsT256BulletproofSuiteV1>(
        context_digest,
        zk_ams_t256_bulletproof_generator_basis_digest_v1(),
        expected_chunk_ordinal,
        expected_bound,
        ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1,
        evidence,
    )
}
fn initialize_transcript_state(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    chunk_ordinal: u16,
    coefficient_bound: u8,
    commitment: &Point,
) -> Result<Vec<u8>, GeneralizedBulletproofErrorV1> {
    if context_digest == [0; 32]
        || generator_basis_digest == [0; 32]
        || !matches!(coefficient_bound, 1 | 2)
        || commitment == &Point::identity()
    {
        return Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant);
    }
    let mut state = Vec::with_capacity(T256_BP_TRANSCRIPT_DOMAIN_V1.len() + 32 + 32 + 2 + 1 + 33);
    state.extend_from_slice(T256_BP_TRANSCRIPT_DOMAIN_V1);
    state.extend_from_slice(&context_digest);
    state.extend_from_slice(&generator_basis_digest);
    state.extend_from_slice(&chunk_ordinal.to_be_bytes());
    state.push(coefficient_bound);
    let encoded = SecretT256PointEncodingV1::new(commitment)?;
    state.extend_from_slice(encoded.as_ref());
    drop(encoded);
    Ok(state)
}
fn append_scalar(state: &mut Vec<u8>, scalar: &Scalar) {
    state.push(T256_BP_SCALAR_TAG_V1);
    with_borrowed_t256_scalar_encoding_v1(scalar, |encoded| {
        state.extend_from_slice(encoded);
    });
}
fn append_point(state: &mut Vec<u8>, point: &Point) -> Result<(), GeneralizedBulletproofErrorV1> {
    let encoded = SecretT256PointEncodingV1::new(point)?;
    state.push(T256_BP_POINT_TAG_V1);
    state.extend_from_slice(encoded.as_ref());
    drop(encoded);
    Ok(())
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
        commitment: &Point,
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
    fn push_scalar(&mut self, scalar: &Scalar) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = SecretT256ScalarEncodingV1::new(scalar);
        self.state.push(T256_BP_SCALAR_TAG_V1);
        self.state.extend_from_slice(encoded.as_ref());
        self.proof.extend_from_slice(encoded.as_ref());
        drop(encoded);
        Ok(())
    }
    fn push_point(&mut self, point: &Point) -> Result<(), GeneralizedBulletproofErrorV1> {
        let encoded = SecretT256PointEncodingV1::new(point)?;
        self.state.push(T256_BP_POINT_TAG_V1);
        self.state.extend_from_slice(encoded.as_ref());
        self.proof.extend_from_slice(encoded.as_ref());
        drop(encoded);
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
        append_scalar(&mut self.state, &scalar);
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
        append_point(&mut self.state, &point)?;
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
            SecretMultiexpBuilder, Variable, VectorCommitmentOpening,
        },
        vega::VEGA_T256_SCALAR_MODULUS_BE_V1,
    };
    #[test]
    fn scalar_copy_owner_take_clears_named_source_and_guards_by_value_boundary() {
        let expected = Scalar::from_u64(41);
        let mut source_scalar = expected;
        let owned = ZeroizingT256ScalarCopyV1::take(&mut source_scalar);
        assert!(source_scalar.is_zero());
        assert_eq!(owned.get(), expected);
        let source = include_str!("bulletproof_t256.rs");
        assert!(source.contains("pub(super) fn new(mut value: Scalar) -> Self"));
        assert!(source.contains("let incoming = BorrowedT256ScalarCopyV1(&mut value);"));
        assert!(source.contains("pub(super) fn take(value: &mut Scalar) -> Self"));
        assert!(source.contains("impl Drop for BorrowedT256ScalarCopyV1<'_>"));
        let vector_owner = source
            .split_once("impl ZeroizingT256ScalarVecV1 {")
            .expect("scalar-vector owner")
            .1
            .split_once("impl core::fmt::Debug for ZeroizingT256ScalarVecV1")
            .expect("scalar-vector owner boundary")
            .0;
        assert!(vector_owner.contains("pub(super) fn push(&mut self, mut value: Scalar)"));
        assert!(vector_owner.contains("let incoming = BorrowedT256ScalarCopyV1(&mut value);"));
        assert!(vector_owner.contains("self.0.push(incoming.get());"));
        assert!(vector_owner.contains("drop(incoming);"));
        let boolean_witness = source
            .split_once("fn append_boolean_witness(")
            .expect("Boolean witness helper")
            .1
            .split_once("\nfn membership_commitment_for_suite")
            .expect("Boolean witness helper boundary")
            .0;
        assert!(boolean_witness.contains("let bit = ZeroizingT256ScalarCopyV1::new("));
        assert_eq!(boolean_witness.matches("push(bit.get())").count(), 2);
    }
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
                    16,
                )
            })
        }
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct CancellationT256Suite;
    impl ProofSuite for CancellationT256Suite {
        type Scalar = Scalar;
        type Point = Point;
        fn generators() -> &'static ProofGenerators<Self> {
            static GENERATORS: OnceLock<ProofGenerators<CancellationT256Suite>> = OnceLock::new();
            GENERATORS.get_or_init(|| {
                let source = TinyT256Suite::generators();
                let commitment_generator = source.g_bold[0];
                ProofGenerators::new(
                    source.g,
                    commitment_generator,
                    source.g_bold.clone(),
                    source.h_bold.clone(),
                )
                .expect("deliberately dependent test basis has a valid public shape")
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
    type TinyFixture = (
        [u8; 32],
        [u8; 32],
        Point,
        Vec<LinComb<Scalar>>,
        ArithmeticCircuitWitness<TinyT256Suite>,
    );
    fn fixture() -> TinyFixture {
        let generators = TinyT256Suite::generators().reduce(4).expect("tiny basis");
        let values = vec![
            Scalar::from_u64(7),
            Scalar::from_u64(8),
            Scalar::from_u64(9),
            Scalar::from_u64(10),
        ];
        let mask = Scalar::from_u64(13);
        let mut commitment_terms = SecretMultiexpBuilder::<TinyT256Suite>::new(values.len() + 1)
            .expect("fixed fixture commitment capacity");
        for (scalar, point) in values.iter().zip(generators.g_bold) {
            commitment_terms
                .push(scalar, point)
                .expect("fixture term fits fixed commitment capacity");
        }
        commitment_terms
            .push(&mask, &generators.h)
            .expect("fixture mask fits fixed commitment capacity");
        let commitment = *commitment_terms
            .evaluate()
            .expect("complete fixture commitment")
            .expose_ref();
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
    fn scalar_commitment(value: Scalar, mask: Scalar) -> Point {
        let generators = TinyT256Suite::generators();
        let mut terms = SecretMultiexpBuilder::<TinyT256Suite>::new(2)
            .expect("fixed scalar commitment capacity");
        terms
            .push(&value, &generators.g)
            .expect("scalar value term fits exact capacity");
        terms
            .push(&mask, &generators.h)
            .expect("scalar mask term fits exact capacity");
        *terms
            .evaluate()
            .expect("complete scalar commitment")
            .expose_ref()
    }
    fn scalar_openings() -> Vec<(Scalar, Scalar)> {
        vec![
            (Scalar::from_u64(7), Scalar::from_u64(11)),
            (Scalar::from_u64(13), Scalar::from_u64(17)),
        ]
    }
    fn scalar_commitments() -> Vec<Point> {
        scalar_openings()
            .into_iter()
            .map(|(value, mask)| scalar_commitment(value, mask))
            .collect()
    }
    fn scalar_constraints() -> Vec<LinComb<Scalar>> {
        vec![
            LinComb::empty()
                .term(Scalar::ONE, Variable::aO(0))
                .term(-Scalar::ONE, Variable::V(0))
                .constant(-Scalar::from_u64(5)),
            LinComb::empty()
                .term(Scalar::ONE, Variable::aO(1))
                .term(Scalar::from_u64(2), Variable::V(1))
                .constant(-Scalar::from_u64(56)),
        ]
    }
    fn scalar_fixture_context() -> [u8; 32] {
        keccak256(b"t256-bp-scalar-opening-context")
    }
    fn scalar_fixture_basis() -> [u8; 32] {
        keccak256(b"t256-bp-scalar-opening-basis")
    }
    fn prove_scalar_fixture(
        constraints: Vec<LinComb<Scalar>>,
        vector_commitments: Vec<Point>,
        vector_openings: Vec<VectorCommitmentOpening<Scalar>>,
        scalar_commitments: Vec<Point>,
        scalar_openings: Vec<(Scalar, Scalar)>,
        rng_label: &[u8],
    ) -> Result<(Vec<u8>, [u8; 32]), GeneralizedBulletproofErrorV1> {
        let statement_axis = scalar_commitments
            .first()
            .copied()
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        let witness = ArithmeticCircuitWitness::<TinyT256Suite>::new_with_scalar_commitments(
            vec![Scalar::from_u64(3), Scalar::from_u64(5)],
            vec![Scalar::from_u64(4), Scalar::from_u64(6)],
            vector_openings,
            scalar_openings,
        )?;
        let mut transcript = T256BulletproofProverTranscriptV1::<TinyT256Suite>::new(
            scalar_fixture_context(),
            scalar_fixture_basis(),
            11,
            2,
            &statement_axis,
        )?;
        ArithmeticCircuitStatement::new(
            TinyT256Suite::generators().reduce(4)?,
            constraints,
            vector_commitments,
            scalar_commitments,
        )?
        .prove(&mut KatRandom::new(rng_label), &mut transcript, witness)?;
        Ok(transcript.complete())
    }
    fn verify_scalar_fixture(
        constraints: Vec<LinComb<Scalar>>,
        vector_commitments: Vec<Point>,
        scalar_commitments: Vec<Point>,
        proof: &[u8],
    ) -> Result<[u8; 32], GeneralizedBulletproofErrorV1> {
        let statement_axis = scalar_commitments
            .first()
            .copied()
            .ok_or(GeneralizedBulletproofErrorV1::ArithmeticInvariant)?;
        let mut transcript = T256BulletproofVerifierTranscriptV1::<TinyT256Suite>::new(
            scalar_fixture_context(),
            scalar_fixture_basis(),
            11,
            2,
            statement_axis,
            proof,
        )?;
        ArithmeticCircuitStatement::new(
            TinyT256Suite::generators().reduce(4)?,
            constraints,
            vector_commitments,
            scalar_commitments,
        )?
        .verify(&mut transcript)?;
        transcript.finish()
    }
    fn mixed_vector_opening() -> (Point, VectorCommitmentOpening<Scalar>) {
        let generators = TinyT256Suite::generators().reduce(4).expect("tiny basis");
        let values = vec![
            Scalar::from_u64(19),
            Scalar::from_u64(23),
            Scalar::from_u64(29),
            Scalar::from_u64(31),
        ];
        let mask = Scalar::from_u64(37);
        let mut terms = SecretMultiexpBuilder::<TinyT256Suite>::new(values.len() + 1)
            .expect("fixed mixed commitment capacity");
        for (value, generator) in values.iter().zip(generators.g_bold) {
            terms
                .push(value, generator)
                .expect("mixed value fits exact capacity");
        }
        terms
            .push(&mask, &generators.h)
            .expect("mixed mask fits exact capacity");
        (
            *terms
                .evaluate()
                .expect("complete mixed commitment")
                .expose_ref(),
            VectorCommitmentOpening::new(values, mask),
        )
    }
    fn verify_fixture_with_axes(
        context: [u8; 32],
        basis: [u8; 32],
        chunk_ordinal: u16,
        coefficient_bound: u8,
        commitment: Point,
        proof: &[u8],
    ) -> Result<[u8; 32], GeneralizedBulletproofErrorV1> {
        let (_, _, _, constraints, _) = fixture();
        let generators = TinyT256Suite::generators().reduce(4)?;
        let mut transcript = T256BulletproofVerifierTranscriptV1::<TinyT256Suite>::new(
            context,
            basis,
            chunk_ordinal,
            coefficient_bound,
            commitment,
            proof,
        )?;
        ArithmeticCircuitStatement::new(generators, constraints, vec![commitment], Vec::new())?
            .verify(&mut transcript)?;
        transcript.finish()
    }
    fn verify_fixture(
        context: [u8; 32],
        basis: [u8; 32],
        commitment: Point,
        proof: &[u8],
    ) -> Result<[u8; 32], GeneralizedBulletproofErrorV1> {
        verify_fixture_with_axes(context, basis, 7, 2, commitment, proof)
    }
    fn padded_tail_membership_witness(
        bound: ZkAmsT256MembershipBoundV1,
        coefficients: &[i8],
        tail: Scalar,
        mask: Scalar,
    ) -> (usize, Point, ArithmeticCircuitWitness<TinyT256Suite>) {
        assert!(!tail.is_zero());
        assert!(!mask.is_zero());
        assert!(
            coefficients
                .iter()
                .copied()
                .all(|value| bound.contains(value))
        );
        let (actual_gates, padded_gates, _) =
            membership_shape(coefficients.len(), bound).expect("valid tiny membership shape");
        let generators = TinyT256Suite::generators()
            .reduce(padded_gates)
            .expect("tiny padded basis");
        let mut values = coefficients
            .iter()
            .copied()
            .map(signed_scalar)
            .collect::<Vec<_>>();
        values.resize(padded_gates, Scalar::ZERO);
        values[coefficients.len()] = tail;
        let mut commitment_terms = SecretMultiexpBuilder::<TinyT256Suite>::new(padded_gates + 1)
            .expect("padded commitment capacity");
        for (scalar, point) in values.iter().zip(generators.g_bold) {
            commitment_terms
                .push(scalar, point)
                .expect("padded commitment term fits capacity");
        }
        commitment_terms
            .push(&mask, &generators.h)
            .expect("padded commitment mask fits capacity");
        let commitment = *commitment_terms
            .evaluate()
            .expect("complete padded commitment")
            .expose_ref();
        assert!(!commitment.is_identity());
        let mut a_l = Vec::with_capacity(actual_gates);
        let mut a_r = Vec::with_capacity(actual_gates);
        for coefficient in coefficients.iter().copied() {
            let bits = match bound {
                ZkAmsT256MembershipBoundV1::One => [coefficient == 1, coefficient == -1, false],
                ZkAmsT256MembershipBoundV1::Two => [
                    (coefficient == -1) | (coefficient >= 1),
                    coefficient == 2,
                    coefficient < 0,
                ],
            };
            for bit in bits.into_iter().take(bound.gates_per_coefficient()) {
                let bit = Scalar::from_u64(u64::from(bit));
                a_l.push(bit);
                a_r.push(bit);
            }
        }
        assert_eq!(a_l.len(), actual_gates);
        let witness = ArithmeticCircuitWitness::<TinyT256Suite>::new(
            a_l,
            a_r,
            vec![VectorCommitmentOpening::new(values, mask)],
        )
        .expect("shape-valid padded-tail witness");
        (padded_gates, commitment, witness)
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
    fn borrowed_t256_scalar_publication_encoding_clears_on_success_error_and_unwind() {
        let scalar = Scalar::from_u64(0x0102);
        T256_SCALAR_ENCODING_CLEARS_V1.with(|clears| clears.set(0));
        let published = with_borrowed_t256_scalar_encoding_v1(&scalar, |encoded| *encoded);
        assert_eq!(published, scalar.to_le_bytes());
        assert_eq!(scalar, Scalar::from_u64(0x0102));
        assert_eq!(
            T256_SCALAR_ENCODING_CLEARS_V1.with(core::cell::Cell::get),
            1
        );

        T256_SCALAR_ENCODING_CLEARS_V1.with(|clears| clears.set(0));
        let error: Result<(), &'static str> =
            with_borrowed_t256_scalar_encoding_v1(&scalar, |_| Err("injected sink error"));
        assert_eq!(error, Err("injected sink error"));
        assert_eq!(
            T256_SCALAR_ENCODING_CLEARS_V1.with(core::cell::Cell::get),
            1
        );

        T256_SCALAR_ENCODING_CLEARS_V1.with(|clears| clears.set(0));
        let unwind = std::panic::catch_unwind(|| {
            with_borrowed_t256_scalar_encoding_v1(&scalar, |_| {
                panic!("exercise borrowed T256 scalar-publication unwind")
            });
        });
        assert!(unwind.is_err());
        assert_eq!(scalar, Scalar::from_u64(0x0102));
        assert_eq!(
            T256_SCALAR_ENCODING_CLEARS_V1.with(core::cell::Cell::get),
            1
        );

        let production = include_str!("bulletproof_t256.rs")
            .split_once("#[cfg(test)]\nmod tests")
            .expect("production source boundary")
            .0;
        let transcript = production
            .split_once("impl<S> ProverTranscript<S> for T256BulletproofProverTranscriptV1<S>")
            .expect("T256 prover transcript")
            .1
            .split_once("/// Exact, allocation-bounded verifier transcript")
            .expect("T256 prover transcript boundary")
            .0;
        assert!(transcript.contains("fn push_scalar(&mut self, scalar: &Scalar)"));
        assert!(transcript.contains("let encoded = SecretT256ScalarEncodingV1::new(scalar);"));
        assert!(transcript.contains("drop(encoded);"));
        assert!(!transcript.contains("scalar.to_le_bytes()"));
    }
    #[test]
    fn borrowed_t256_point_publication_encoding_clears_on_success_identity_and_unwind() {
        let point = TinyT256Suite::generators().g;
        let expected = point
            .to_non_identity_wire_bytes()
            .expect("nonidentity fixture point");
        T256_POINT_ENCODING_CLEARS_V1.with(|clears| clears.set(0));
        let encoded = SecretT256PointEncodingV1::new(&point).expect("borrowed point encoding");
        assert_eq!(encoded.as_ref(), &expected);
        assert_eq!(T256_POINT_ENCODING_CLEARS_V1.with(core::cell::Cell::get), 0);
        drop(encoded);
        assert_eq!(T256_POINT_ENCODING_CLEARS_V1.with(core::cell::Cell::get), 1);

        T256_POINT_ENCODING_CLEARS_V1.with(|clears| clears.set(0));
        let encoded_identity = SecretT256PointEncodingV1::new_allow_identity(&Point::identity())
            .expect("canonical identity encoding");
        let mut expected_identity = [0_u8; 33];
        expected_identity[0] = 0x40;
        assert_eq!(encoded_identity.as_ref(), &expected_identity);
        drop(encoded_identity);
        assert_eq!(T256_POINT_ENCODING_CLEARS_V1.with(core::cell::Cell::get), 1);

        T256_POINT_ENCODING_CLEARS_V1.with(|clears| clears.set(0));
        assert!(matches!(
            SecretT256PointEncodingV1::new(&Point::identity()),
            Err(GeneralizedBulletproofErrorV1::PointIdentity)
        ));
        assert_eq!(T256_POINT_ENCODING_CLEARS_V1.with(core::cell::Cell::get), 1);

        T256_POINT_ENCODING_CLEARS_V1.with(|clears| clears.set(0));
        let unwind = std::panic::catch_unwind(|| {
            let encoded =
                SecretT256PointEncodingV1::new(&point).expect("point owner before unwind");
            let _ = core::hint::black_box(encoded.as_ref());
            panic!("exercise borrowed T256 point-publication unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(T256_POINT_ENCODING_CLEARS_V1.with(core::cell::Cell::get), 1);

        let production = include_str!("bulletproof_t256.rs")
            .split_once("#[cfg(test)]\nmod tests")
            .expect("production source boundary")
            .0;
        let transcript = production
            .split_once("impl<S> ProverTranscript<S> for T256BulletproofProverTranscriptV1<S>")
            .expect("T256 prover transcript")
            .1
            .split_once("/// Exact, allocation-bounded verifier transcript")
            .expect("T256 prover transcript boundary")
            .0;
        assert!(transcript.contains("fn push_point(&mut self, point: &Point)"));
        assert!(transcript.contains("let encoded = SecretT256PointEncodingV1::new(point)?;"));
        assert!(transcript.contains("self.state.extend_from_slice(encoded.as_ref());"));
        assert!(transcript.contains("self.proof.extend_from_slice(encoded.as_ref());"));
        assert!(transcript.contains("drop(encoded);"));
        assert!(!transcript.contains("point.to_non_identity_wire_bytes()"));
        assert!(!transcript.contains("fn push_point(&mut self, point: Point)"));
        let membership_witness = production
            .split_once("fn membership_witness<S>(")
            .expect("membership witness")
            .1
            .split_once("fn prove_membership_chunk_for_suite<S, R>(")
            .expect("membership witness boundary")
            .0;
        assert!(membership_witness.contains("SecretPoint<Point>"));
        assert!(membership_witness.contains("Ok((secret_commitment, witness))"));
        let membership_prover = production
            .split_once("fn prove_membership_chunk_for_suite<S, R>(")
            .expect("membership prover")
            .1
            .split_once("fn verify_membership_chunk_for_suite<S>(")
            .expect("membership prover boundary")
            .0;
        let transcript_borrow = membership_prover
            .find("secret_commitment.expose_ref(),")
            .expect("borrowed statement-axis publication");
        let statement_copy = membership_prover
            .find("vec![*secret_commitment.expose_ref()]")
            .expect("published statement point copy");
        let output_copy = membership_prover
            .find("let commitment = *secret_commitment.expose_ref();")
            .expect("public proof commitment copy");
        assert!(transcript_borrow < statement_copy && statement_copy < output_copy);
    }
    #[test]
    fn release_generator_basis_digest_is_pinned() {
        assert_eq!(
            zk_ams_t256_bulletproof_generator_basis_digest_v1(),
            ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1
        );
    }
    #[test]
    fn branchless_signed_membership_scalar_covers_release_coefficients() {
        assert_eq!(signed_scalar(-2), -Scalar::from_u64(2));
        assert_eq!(signed_scalar(-1), -Scalar::ONE);
        assert_eq!(signed_scalar(0), Scalar::ZERO);
        assert_eq!(signed_scalar(1), Scalar::ONE);
        assert_eq!(signed_scalar(2), Scalar::from_u64(2));
    }
    #[test]
    fn factored_membership_commitment_matches_embedded_proof_commitment() {
        let context = keccak256(b"t256-factored-membership-commitment-context");
        let basis = keccak256(b"t256-factored-membership-commitment-basis");
        let coefficients = [-1, 0, 1];
        let blinding = Scalar::from_u64(17);
        let (direct, _opening, _actual_gates) = membership_commitment_for_suite::<TinyT256Suite>(
            &coefficients,
            ZkAmsT256MembershipBoundV1::One,
            &blinding,
        )
        .expect("valid direct tiny commitment");
        let (evidence, _) = prove_membership_chunk_for_suite::<TinyT256Suite, _>(
            context,
            basis,
            3,
            ZkAmsT256MembershipBoundV1::One,
            &coefficients,
            &blinding,
            &mut KatRandom::new(b"t256-factored-membership-commitment-rng"),
        )
        .expect("valid tiny membership proof");
        assert!(direct.equals(&evidence.commitment()));
    }
    #[test]
    fn release_membership_commitment_is_exact_and_binds_every_opening_input() {
        let bound = ZkAmsT256MembershipBoundV1::One;
        let blinding = Scalar::from_u64(29);
        let mut coefficients = vec![0_i8; ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1];
        assert_eq!(
            commit_zk_ams_t256_membership_chunk_v1(
                bound,
                &coefficients[..coefficients.len() - 1],
                &blinding,
            ),
            Err(ZkAmsT256MembershipErrorV1::CoefficientCount)
        );
        let excessive_coefficients = vec![0_i8; ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 + 1];
        assert_eq!(
            commit_zk_ams_t256_membership_chunk_v1(bound, &excessive_coefficients, &blinding),
            Err(ZkAmsT256MembershipErrorV1::CoefficientCount)
        );
        let changed_index = coefficients.len() / 2;
        coefficients[changed_index] = 2;
        assert_eq!(
            commit_zk_ams_t256_membership_chunk_v1(bound, &coefficients, &blinding),
            Err(ZkAmsT256MembershipErrorV1::CoefficientOutOfRange {
                index: changed_index,
            })
        );
        coefficients[changed_index] = 0;
        assert_eq!(
            commit_zk_ams_t256_membership_chunk_v1(bound, &coefficients, &Scalar::ZERO),
            Err(ZkAmsT256MembershipErrorV1::Blinding)
        );
        let commitment = commit_zk_ams_t256_membership_chunk_v1(bound, &coefficients, &blinding)
            .expect("valid exact release commitment");
        assert!(!commitment.is_identity());
        coefficients[changed_index] = 1;
        let changed_coefficient =
            commit_zk_ams_t256_membership_chunk_v1(bound, &coefficients, &blinding)
                .expect("mutated coefficient remains in range");
        assert_ne!(commitment, changed_coefficient);
        coefficients[changed_index] = 0;
        let changed_blinding =
            commit_zk_ams_t256_membership_chunk_v1(bound, &coefficients, &Scalar::from_u64(31))
                .expect("mutated blinding remains nonzero");
        assert_ne!(commitment, changed_blinding);
    }
    #[test]
    fn membership_commitment_rejects_constructible_identity() {
        let result = membership_commitment_for_suite::<CancellationT256Suite>(
            &[1],
            ZkAmsT256MembershipBoundV1::One,
            &-Scalar::ONE,
        );
        assert!(matches!(
            result,
            Err(ZkAmsT256MembershipErrorV1::CommitmentIdentity)
        ));
    }
    #[test]
    fn membership_verifier_rejects_legacy_nonzero_padded_opening_tails_for_both_bounds() {
        type PaddedTailCase<'a> = (ZkAmsT256MembershipBoundV1, &'a [i8], u64, u64, &'a [u8]);
        let context = keccak256(b"t256-membership-padded-tail-context");
        let basis = keccak256(b"t256-membership-padded-tail-basis");
        let cases: [PaddedTailCase<'_>; 2] = [
            (
                ZkAmsT256MembershipBoundV1::One,
                &[-1, 0, 1],
                23,
                29,
                b"t256-membership-padded-tail-bound-one",
            ),
            (
                ZkAmsT256MembershipBoundV1::Two,
                &[-2, 0, 2],
                31,
                37,
                b"t256-membership-padded-tail-bound-two",
            ),
        ];
        for (case_index, (bound, coefficients, tail, mask, rng_label)) in
            cases.into_iter().enumerate()
        {
            let tail = Scalar::from_u64(tail);
            let mask = Scalar::from_u64(mask);
            let coefficient_count = coefficients.len();
            let (padded_gates, commitment, witness) =
                padded_tail_membership_witness(bound, coefficients, tail, mask);
            let (statement_gates, hardened_constraints) =
                membership_constraints(coefficient_count, bound)
                    .expect("valid hardened membership statement");
            assert_eq!(statement_gates, padded_gates);
            let visible_constraint_count = coefficient_count * bound.constraints_per_coefficient();
            assert_eq!(
                hardened_constraints.len(),
                visible_constraint_count + (padded_gates - coefficient_count)
            );
            for (offset, constraint) in hardened_constraints[visible_constraint_count..]
                .iter()
                .enumerate()
            {
                assert_eq!(
                    constraint,
                    &LinComb::empty().term(
                        Scalar::ONE,
                        Variable::CG {
                            commitment: 0,
                            index: coefficient_count + offset,
                        },
                    ),
                    "bound {bound:?} padded coordinate {} was not fixed to zero",
                    coefficient_count + offset
                );
            }
            // Reconstruct the formerly accepted statement by dropping the
            // new tail constraints.  Its valid proof demonstrates that the
            // nonzero coordinate is a real legacy forgery, not malformed
            // proof bytes; the hardened production verifier must reject it.
            let mut legacy_constraints = hardened_constraints;
            legacy_constraints.truncate(visible_constraint_count);
            let chunk_ordinal = 10 + u16::try_from(case_index).expect("two fixed cases");
            let mut prover_transcript = T256BulletproofProverTranscriptV1::<TinyT256Suite>::new(
                context,
                basis,
                chunk_ordinal,
                bound as u8,
                &commitment,
            )
            .expect("valid padded-tail transcript axes");
            ArithmeticCircuitStatement::new(
                TinyT256Suite::generators()
                    .reduce(padded_gates)
                    .expect("tiny padded basis"),
                legacy_constraints.clone(),
                vec![commitment],
                Vec::new(),
            )
            .expect("legacy statement shape")
            .prove(
                &mut KatRandom::new(rng_label),
                &mut prover_transcript,
                witness,
            )
            .expect("legacy statement admits the nonzero padded tail");
            let (proof, prover_digest) = prover_transcript.complete();
            let mut legacy_verifier = T256BulletproofVerifierTranscriptV1::<TinyT256Suite>::new(
                context,
                basis,
                chunk_ordinal,
                bound as u8,
                commitment,
                &proof,
            )
            .expect("valid legacy verifier transcript");
            ArithmeticCircuitStatement::new(
                TinyT256Suite::generators()
                    .reduce(padded_gates)
                    .expect("tiny padded basis"),
                legacy_constraints,
                vec![commitment],
                Vec::new(),
            )
            .expect("legacy verifier statement shape")
            .verify(&mut legacy_verifier)
            .expect("legacy verifier accepts its forged statement");
            assert_eq!(legacy_verifier.finish(), Ok(prover_digest));
            let forged_evidence = ZkAmsT256MembershipProofV1 {
                bound,
                chunk_ordinal,
                coefficient_count: u32::try_from(coefficient_count)
                    .expect("tiny coefficient count"),
                commitment,
                proof,
            };
            assert!(
                verify_membership_chunk_for_suite::<TinyT256Suite>(
                    context,
                    basis,
                    chunk_ordinal,
                    bound,
                    coefficient_count,
                    &forged_evidence,
                )
                .is_err(),
                "hardened verifier accepted bound {bound:?} with a nonzero padded tail"
            );
        }
    }
    #[test]
    fn exact_membership_circuits_cover_both_sets_and_reject_adversarial_evidence() {
        let context = keccak256(b"t256-membership-test-context");
        let basis = keccak256(b"t256-membership-test-basis");
        let prove_bound_one = || {
            prove_membership_chunk_for_suite::<TinyT256Suite, _>(
                context,
                basis,
                6,
                ZkAmsT256MembershipBoundV1::One,
                &[-1, 0, 1],
                &Scalar::from_u64(17),
                &mut KatRandom::new(b"t256-membership-bound-one-rng"),
            )
            .expect("all bound-one members prove")
        };
        #[cfg(feature = "parallel")]
        let single_worker = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("single-worker Rayon pool")
            .install(&prove_bound_one);
        #[cfg(not(feature = "parallel"))]
        let single_worker = prove_bound_one();
        #[cfg(feature = "parallel")]
        {
            let four_workers = rayon::ThreadPoolBuilder::new()
                .num_threads(4)
                .build()
                .expect("four-worker Rayon pool")
                .install(&prove_bound_one);
            assert_eq!(
                single_worker.0.proof_bytes(),
                four_workers.0.proof_bytes(),
                "membership proof bytes must be worker-count independent"
            );
            assert_eq!(
                single_worker.1, four_workers.1,
                "membership transcript digest must be worker-count independent"
            );
        }
        let (bound_one, bound_one_digest) = single_worker;
        assert_eq!(bound_one.proof_bytes().len(), 655);
        assert_eq!(bound_one.to_wire_bytes().len(), 702);
        assert_eq!(
            bound_one_digest,
            [
                0x41, 0x48, 0xe5, 0x5f, 0xbd, 0xea, 0x41, 0x2b, 0x5b, 0xb5, 0xee, 0x3a, 0xc2, 0x77,
                0x6c, 0x8a, 0x66, 0xff, 0xa6, 0x73, 0x4e, 0xe7, 0xa4, 0xd4, 0x5c, 0x1d, 0xde, 0x5d,
                0xbb, 0xb3, 0x93, 0xf6,
            ]
        );
        assert_eq!(
            verify_membership_chunk_for_suite::<TinyT256Suite>(
                context,
                basis,
                6,
                ZkAmsT256MembershipBoundV1::One,
                3,
                &bound_one,
            ),
            Ok(bound_one_digest)
        );
        let (bound_two, bound_two_digest) = prove_membership_chunk_for_suite::<TinyT256Suite, _>(
            context,
            basis,
            7,
            ZkAmsT256MembershipBoundV1::Two,
            &[-2, -1, 0, 1, 2],
            &Scalar::from_u64(19),
            &mut KatRandom::new(b"t256-membership-bound-two-rng"),
        )
        .expect("all bound-two members prove");
        assert_eq!(bound_two.proof_bytes().len(), 721);
        assert_eq!(bound_two.to_wire_bytes().len(), 768);
        assert_eq!(
            bound_two_digest,
            [
                0x45, 0x58, 0x07, 0xed, 0xde, 0xc5, 0xcb, 0xc7, 0x81, 0x78, 0x26, 0xd0, 0x5f, 0x65,
                0x14, 0xeb, 0x19, 0xa6, 0x7b, 0x49, 0xff, 0x0c, 0x2b, 0x5f, 0x12, 0x55, 0x5c, 0x73,
                0xab, 0x2c, 0xd1, 0xcb,
            ]
        );
        assert_eq!(
            membership_shape(3, ZkAmsT256MembershipBoundV1::One),
            Ok((6, 8, 20))
        );
        assert_eq!(
            membership_shape(5, ZkAmsT256MembershipBoundV1::Two),
            Ok((15, 16, 46))
        );
        assert_eq!(
            membership_shape(
                ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1,
                ZkAmsT256MembershipBoundV1::One,
            ),
            Ok((32_768, 32_768, 98_304))
        );
        assert_eq!(membership_proof_len(32_768), Ok(1_447));
        assert_eq!(
            membership_shape(
                ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1,
                ZkAmsT256MembershipBoundV1::Two,
            ),
            Ok((49_152, 65_536, 163_840))
        );
        assert_eq!(membership_proof_len(65_536), Ok(1_513));
        assert_eq!(
            verify_membership_chunk_for_suite::<TinyT256Suite>(
                context,
                basis,
                7,
                ZkAmsT256MembershipBoundV1::Two,
                5,
                &bound_two,
            ),
            Ok(bound_two_digest)
        );
        for index in 0..bound_two.proof.len() {
            let mut changed = bound_two.clone();
            changed.proof[index] ^= 1;
            assert!(
                verify_membership_chunk_for_suite::<TinyT256Suite>(
                    context,
                    basis,
                    7,
                    ZkAmsT256MembershipBoundV1::Two,
                    5,
                    &changed,
                )
                .is_err(),
                "changed membership proof byte {index} was accepted"
            );
        }
        let wire = bound_two.to_wire_bytes();
        assert_eq!(
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wire),
            Ok(bound_two.clone())
        );
        for end in 0..wire.len() {
            assert!(ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wire[..end]).is_err());
        }
        let mut trailing = wire.clone();
        trailing.push(0);
        assert_eq!(
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&trailing),
            Err(ZkAmsT256MembershipErrorV1::ProofLength)
        );
        for index in 12..45 {
            let mut changed = wire.clone();
            changed[index] ^= 1;
            if let Ok(changed) = ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&changed) {
                assert!(
                    verify_membership_chunk_for_suite::<TinyT256Suite>(
                        context,
                        basis,
                        7,
                        ZkAmsT256MembershipBoundV1::Two,
                        5,
                        &changed,
                    )
                    .is_err(),
                    "changed commitment byte {index} was accepted"
                );
            }
        }
        let mut wrong_magic = wire.clone();
        wrong_magic[0] ^= 1;
        assert_eq!(
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wrong_magic),
            Err(ZkAmsT256MembershipErrorV1::WireEncoding)
        );
        let mut wrong_version = wire.clone();
        wrong_version[4] = 2;
        assert_eq!(
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wrong_version),
            Err(ZkAmsT256MembershipErrorV1::WireEncoding)
        );
        let mut wrong_bound = wire.clone();
        wrong_bound[5] = 3;
        assert_eq!(
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wrong_bound),
            Err(ZkAmsT256MembershipErrorV1::WireEncoding)
        );
        let mut excessive_ordinal = wire.clone();
        excessive_ordinal[6..8]
            .copy_from_slice(&(ZK_AMS_MEMBERSHIP_MAX_CHUNK_ORDINAL_V1 + 1).to_be_bytes());
        assert_eq!(
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&excessive_ordinal),
            Err(ZkAmsT256MembershipErrorV1::ChunkOrdinal)
        );
        let mut wrong_proof_len = wire.clone();
        wrong_proof_len[45..47].copy_from_slice(&720_u16.to_be_bytes());
        assert_eq!(
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wrong_proof_len),
            Err(ZkAmsT256MembershipErrorV1::ProofLength)
        );
        let mut same_shape_wrong_count = wire.clone();
        same_shape_wrong_count[8..12].copy_from_slice(&4_u32.to_be_bytes());
        let same_shape_wrong_count =
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&same_shape_wrong_count)
                .expect("four and five bound-two coefficients share the same padded shape");
        assert_eq!(
            verify_membership_chunk_for_suite::<TinyT256Suite>(
                context,
                basis,
                7,
                ZkAmsT256MembershipBoundV1::Two,
                5,
                &same_shape_wrong_count,
            ),
            Err(ZkAmsT256MembershipErrorV1::StatementMismatch)
        );
        assert_eq!(
            prove_membership_chunk_for_suite::<TinyT256Suite, _>(
                context,
                basis,
                7,
                ZkAmsT256MembershipBoundV1::One,
                &[0, 2],
                &Scalar::from_u64(1),
                &mut KatRandom::new(b"out-of-range-one"),
            ),
            Err(ZkAmsT256MembershipErrorV1::CoefficientOutOfRange { index: 1 })
        );
        assert_eq!(
            prove_membership_chunk_for_suite::<TinyT256Suite, _>(
                context,
                basis,
                7,
                ZkAmsT256MembershipBoundV1::Two,
                &[0, -3],
                &Scalar::from_u64(1),
                &mut KatRandom::new(b"out-of-range-two"),
            ),
            Err(ZkAmsT256MembershipErrorV1::CoefficientOutOfRange { index: 1 })
        );
        assert_eq!(
            prove_membership_chunk_for_suite::<TinyT256Suite, _>(
                context,
                basis,
                7,
                ZkAmsT256MembershipBoundV1::Two,
                &[0],
                &Scalar::ZERO,
                &mut KatRandom::new(b"zero-blinding"),
            ),
            Err(ZkAmsT256MembershipErrorV1::Blinding)
        );
        assert!(
            verify_membership_chunk_for_suite::<TinyT256Suite>(
                keccak256(b"wrong-membership-context"),
                basis,
                7,
                ZkAmsT256MembershipBoundV1::Two,
                5,
                &bound_two,
            )
            .is_err()
        );
        assert!(
            verify_membership_chunk_for_suite::<TinyT256Suite>(
                context,
                keccak256(b"wrong-membership-basis"),
                7,
                ZkAmsT256MembershipBoundV1::Two,
                5,
                &bound_two,
            )
            .is_err()
        );
    }
    #[test]
    fn scalar_pedersen_openings_roundtrip_for_one_and_mixed_commitments() {
        let mut one_constraints = scalar_constraints();
        one_constraints.truncate(1);
        let one_commitments = vec![scalar_commitments()[0]];
        let one_openings = vec![scalar_openings()[0]];
        let (one_proof, one_digest) = prove_scalar_fixture(
            one_constraints,
            Vec::new(),
            Vec::new(),
            one_commitments,
            one_openings,
            b"t256-bp-one-scalar-opening-rng",
        )
        .expect("one scalar commitment proves");
        let mut one_constraints = scalar_constraints();
        one_constraints.truncate(1);
        assert_eq!(
            verify_scalar_fixture(
                one_constraints,
                Vec::new(),
                vec![scalar_commitments()[0]],
                &one_proof,
            ),
            Ok(one_digest)
        );
        assert_eq!(one_proof.len(), 589);
        let (vector_commitment, vector_opening) = mixed_vector_opening();
        let (mixed_proof, mixed_digest) = prove_scalar_fixture(
            scalar_constraints(),
            vec![vector_commitment],
            vec![vector_opening],
            scalar_commitments(),
            scalar_openings(),
            b"t256-bp-mixed-scalar-opening-rng",
        )
        .expect("mixed vector and scalar commitments prove");
        assert_eq!(
            verify_scalar_fixture(
                scalar_constraints(),
                vec![vector_commitment],
                scalar_commitments(),
                &mixed_proof,
            ),
            Ok(mixed_digest)
        );
        assert_eq!(mixed_proof.len(), 589);
        // Scalar openings use the omitted center coefficient and therefore
        // add no transcript elements at the governed lookup dimensions.
        assert_eq!(membership_proof_len(1_024), Ok(1_117));
        assert_eq!(membership_proof_len(16_384), Ok(1_381));
        assert_eq!(membership_proof_len(32_768), Ok(1_447));
    }
    #[test]
    fn scalar_pedersen_openings_reject_wrong_openings_order_count_weights_and_sign() {
        let canonical_commitments = scalar_commitments();
        let canonical_openings = scalar_openings();
        let (proof, digest) = prove_scalar_fixture(
            scalar_constraints(),
            Vec::new(),
            Vec::new(),
            canonical_commitments.clone(),
            canonical_openings.clone(),
            b"t256-bp-two-scalar-opening-rng",
        )
        .expect("canonical two-scalar fixture proves");
        assert_eq!(
            verify_scalar_fixture(
                scalar_constraints(),
                Vec::new(),
                canonical_commitments.clone(),
                &proof,
            ),
            Ok(digest)
        );
        let mut wrong_value = canonical_openings.clone();
        wrong_value[0].0 += Scalar::ONE;
        let mut wrong_mask = canonical_openings.clone();
        wrong_mask[0].1 += Scalar::ONE;
        let mut wrong_order = canonical_openings.clone();
        wrong_order.swap(0, 1);
        let mut too_many = canonical_openings.clone();
        too_many.push((Scalar::from_u64(19), Scalar::from_u64(23)));
        for openings in [
            wrong_value,
            wrong_mask,
            wrong_order,
            canonical_openings[..1].to_vec(),
            too_many,
        ] {
            assert!(matches!(
                prove_scalar_fixture(
                    scalar_constraints(),
                    Vec::new(),
                    Vec::new(),
                    canonical_commitments.clone(),
                    openings,
                    b"t256-bp-invalid-scalar-opening-rng",
                ),
                Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
            ));
        }
        let mut prover_wrong_sign = scalar_constraints();
        prover_wrong_sign[0].wv[0].1 = Scalar::ONE;
        assert!(matches!(
            prove_scalar_fixture(
                prover_wrong_sign,
                Vec::new(),
                Vec::new(),
                canonical_commitments.clone(),
                canonical_openings.clone(),
                b"t256-bp-wrong-scalar-sign-rng",
            ),
            Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
        ));
        let mut prover_wrong_weight = scalar_constraints();
        prover_wrong_weight[1].wv[0].1 = Scalar::from_u64(3);
        assert!(matches!(
            prove_scalar_fixture(
                prover_wrong_weight,
                Vec::new(),
                Vec::new(),
                canonical_commitments.clone(),
                canonical_openings,
                b"t256-bp-wrong-scalar-weight-rng",
            ),
            Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
        ));
        let mut reordered_commitments = canonical_commitments.clone();
        reordered_commitments.swap(0, 1);
        assert!(
            verify_scalar_fixture(
                scalar_constraints(),
                Vec::new(),
                reordered_commitments,
                &proof,
            )
            .is_err()
        );
        let mut wrong_sign = scalar_constraints();
        wrong_sign[0].wv[0].1 = Scalar::ONE;
        assert!(
            verify_scalar_fixture(
                wrong_sign,
                Vec::new(),
                canonical_commitments.clone(),
                &proof,
            )
            .is_err()
        );
        let mut wrong_weight = scalar_constraints();
        wrong_weight[1].wv[0].1 = Scalar::from_u64(3);
        assert!(
            verify_scalar_fixture(wrong_weight, Vec::new(), canonical_commitments, &proof,)
                .is_err()
        );
    }
    #[test]
    fn t256_generalized_bulletproof_roundtrip_binds_every_byte_and_axis() {
        let (context, basis, commitment, constraints, witness) = fixture();
        let generators = TinyT256Suite::generators().reduce(4).expect("tiny basis");
        let mut transcript = T256BulletproofProverTranscriptV1::<TinyT256Suite>::new(
            context,
            basis,
            7,
            2,
            &commitment,
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
            verify_fixture_with_axes(context, basis, 8, 2, commitment, &proof),
            verify_fixture_with_axes(context, basis, 7, 1, commitment, &proof),
            verify_fixture(context, basis, other_commitment, &proof),
        ] {
            assert!(result.is_err());
        }
    }
}
