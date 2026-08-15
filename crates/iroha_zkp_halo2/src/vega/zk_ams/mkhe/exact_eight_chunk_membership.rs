//! Sealed exact-eight-chunk T256 membership engine for MKHE relations.
//!
//! The release ring is always represented by eight ordered 16,384-
//! coefficient chunks.  Role marker types fix the coefficient bound, outer
//! wire magic, and every transcript/root domain at compile time.  In
//! particular, persistent-secret, RKG-ephemeral, and CPK-error evidence cannot be converted into
//! one another and no verified membership capability alone establishes a polynomial relation.
use super::ZkAmsMkhePartyIdV1;
use crate::{
    generalized_bulletproof::ProofRandomSource,
    vega::{
        VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar,
        bulletproof_t256::{
            ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1, ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
            ZkAmsT256MembershipBoundV1, ZkAmsT256MembershipErrorV1, ZkAmsT256MembershipProofV1,
            preflight_zk_ams_t256_membership_chunk_wire_v1, prove_zk_ams_t256_membership_chunk_v1,
            verify_zk_ams_t256_membership_chunk_v1, verify_zk_ams_t256_membership_chunk_wire_v1,
            zk_ams_t256_bulletproof_generator_basis_digest_v1,
        },
        sponge::Keccak256,
    },
};
use core::{fmt::Debug, marker::PhantomData};
use thiserror::Error;
const EXACT_MEMBERSHIP_VERSION_V1: u8 = 1;
/// Exact number of ordered proofs for one release-ring polynomial.
pub(super) const ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1: usize = 8;
/// Exact coefficient count covered by one complete membership set.
pub(super) const ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1: usize =
    ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 * ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1;
const MEMBERSHIP_CHUNK_WIRE_HEADER_BYTES_V1: usize = 47;
const BOUND_ONE_PROOF_BYTES_V1: usize = 1_447;
const BOUND_TWO_PROOF_BYTES_V1: usize = 1_513;
const BOUND_ONE_CHUNK_WIRE_BYTES_V1: usize =
    MEMBERSHIP_CHUNK_WIRE_HEADER_BYTES_V1 + BOUND_ONE_PROOF_BYTES_V1;
const BOUND_TWO_CHUNK_WIRE_BYTES_V1: usize =
    MEMBERSHIP_CHUNK_WIRE_HEADER_BYTES_V1 + BOUND_TWO_PROOF_BYTES_V1;
const OFFSET_BOUND_V1: usize = 5;
const OFFSET_CHUNK_COUNT_V1: usize = OFFSET_BOUND_V1 + 1;
const OFFSET_COEFFICIENT_COUNT_V1: usize = OFFSET_CHUNK_COUNT_V1 + 1;
const OFFSET_GENERATOR_BASIS_DIGEST_V1: usize = OFFSET_COEFFICIENT_COUNT_V1 + 4;
const OFFSET_PROFILE_DIGEST_V1: usize = OFFSET_GENERATOR_BASIS_DIGEST_V1 + 32;
const OFFSET_ROSTER_DIGEST_V1: usize = OFFSET_PROFILE_DIGEST_V1 + 32;
const OFFSET_KEY_MATERIAL_DIGEST_V1: usize = OFFSET_ROSTER_DIGEST_V1 + 32;
const OFFSET_EPOCH_V1: usize = OFFSET_KEY_MATERIAL_DIGEST_V1 + 32;
const OFFSET_CPK_TRANSCRIPT_DIGEST_V1: usize = OFFSET_EPOCH_V1 + 8;
const OFFSET_PARTY_V1: usize = OFFSET_CPK_TRANSCRIPT_DIGEST_V1 + 32;
const OFFSET_SHARE_STATEMENT_DIGEST_V1: usize = OFFSET_PARTY_V1 + 32;
const OFFSET_COMMITMENT_SET_DIGEST_V1: usize = OFFSET_SHARE_STATEMENT_DIGEST_V1 + 32;
const OFFSET_PROOF_SET_DIGEST_V1: usize = OFFSET_COMMITMENT_SET_DIGEST_V1 + 32;
const OFFSET_VERIFIER_TRANSCRIPT_DIGEST_V1: usize = OFFSET_PROOF_SET_DIGEST_V1 + 32;
pub(super) const EXACT_MEMBERSHIP_HEADER_BYTES_V1: usize =
    OFFSET_VERIFIER_TRANSCRIPT_DIGEST_V1 + 32;
/// Exact persistent-secret evidence width retained by the first-release wire.
pub(super) const ZK_AMS_MKHE_PERSISTENT_SECRET_MEMBERSHIP_WIRE_BYTES_V1: usize =
    EXACT_MEMBERSHIP_HEADER_BYTES_V1
        + ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 * BOUND_ONE_CHUNK_WIRE_BYTES_V1;
/// Exact RKG-ephemeral evidence width for the distinct bound-one role.
pub(super) const ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1: usize =
    EXACT_MEMBERSHIP_HEADER_BYTES_V1
        + ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 * BOUND_ONE_CHUNK_WIRE_BYTES_V1;
/// Exact CPK-error evidence width for the bound-two role.
pub(super) const ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_WIRE_BYTES_V1: usize =
    EXACT_MEMBERSHIP_HEADER_BYTES_V1
        + ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 * BOUND_TWO_CHUNK_WIRE_BYTES_V1;
/// Exact direct-relation bound-one evidence width.
pub(super) const ZK_AMS_MKHE_DIRECT_BOUND_ONE_MEMBERSHIP_WIRE_BYTES_V1: usize =
    EXACT_MEMBERSHIP_HEADER_BYTES_V1
        + ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 * BOUND_ONE_CHUNK_WIRE_BYTES_V1;
/// Exact direct-relation bound-two evidence width.
pub(super) const ZK_AMS_MKHE_DIRECT_BOUND_TWO_MEMBERSHIP_WIRE_BYTES_V1: usize =
    EXACT_MEMBERSHIP_HEADER_BYTES_V1
        + ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 * BOUND_TWO_CHUNK_WIRE_BYTES_V1;
const _: () = {
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 == 8);
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 == 131_072);
    assert!(BOUND_ONE_PROOF_BYTES_V1 == 1_447);
    assert!(BOUND_TWO_PROOF_BYTES_V1 == 1_513);
    assert!(BOUND_ONE_CHUNK_WIRE_BYTES_V1 == 1_494);
    assert!(BOUND_TWO_CHUNK_WIRE_BYTES_V1 == 1_560);
    assert!(EXACT_MEMBERSHIP_HEADER_BYTES_V1 == 339);
    assert!(ZK_AMS_MKHE_PERSISTENT_SECRET_MEMBERSHIP_WIRE_BYTES_V1 == 12_291);
    assert!(ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1 == 12_291);
    assert!(ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_WIRE_BYTES_V1 == 12_819);
    assert!(ZK_AMS_MKHE_DIRECT_BOUND_ONE_MEMBERSHIP_WIRE_BYTES_V1 == 12_291);
    assert!(ZK_AMS_MKHE_DIRECT_BOUND_TWO_MEMBERSHIP_WIRE_BYTES_V1 == 12_819);
};
mod sealed {
    pub trait Sealed {}
}
/// Compile-time persistent-secret role.  It has no constructible values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PersistentSecretMembershipRoleV1 {}
/// Compile-time RKG-ephemeral role. It has no constructible values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RkgEphemeralMembershipRoleV1 {}
/// Compile-time CPK public-error role.  It has no constructible values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CpkErrorMembershipRoleV1 {}
/// Compile-time direct-relation bound-one role. It has no constructible values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DirectRelationBoundOneMembershipRoleV1 {}
/// Compile-time direct-relation bound-two role. It has no constructible values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DirectRelationBoundTwoMembershipRoleV1 {}
impl sealed::Sealed for PersistentSecretMembershipRoleV1 {}
impl sealed::Sealed for RkgEphemeralMembershipRoleV1 {}
impl sealed::Sealed for CpkErrorMembershipRoleV1 {}
impl sealed::Sealed for DirectRelationBoundOneMembershipRoleV1 {}
impl sealed::Sealed for DirectRelationBoundTwoMembershipRoleV1 {}
pub(super) trait ExactEightChunkMembershipRoleV1:
    sealed::Sealed + Clone + Copy + Debug + PartialEq + Eq
{
    const MAGIC: [u8; 4];
    const BOUND: ZkAmsT256MembershipBoundV1;
    const PROOF_BYTES: usize;
    const CHUNK_WIRE_BYTES: usize;
    const WIRE_BYTES: usize;
    const CONTEXT_DIGEST_DOMAIN: &'static [u8];
    const COMMITMENT_SET_DIGEST_DOMAIN: &'static [u8];
    const PROOF_SET_DIGEST_DOMAIN: &'static [u8];
    const VERIFIER_TRANSCRIPT_DIGEST_DOMAIN: &'static [u8];
}
impl ExactEightChunkMembershipRoleV1 for PersistentSecretMembershipRoleV1 {
    const MAGIC: [u8; 4] = *b"ZPME";
    const BOUND: ZkAmsT256MembershipBoundV1 = ZkAmsT256MembershipBoundV1::One;
    const PROOF_BYTES: usize = BOUND_ONE_PROOF_BYTES_V1;
    const CHUNK_WIRE_BYTES: usize = BOUND_ONE_CHUNK_WIRE_BYTES_V1;
    const WIRE_BYTES: usize = ZK_AMS_MKHE_PERSISTENT_SECRET_MEMBERSHIP_WIRE_BYTES_V1;
    const CONTEXT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.persistent-membership.context";
    const COMMITMENT_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.active-exact-small-binding.persistent-commitment-set";
    const PROOF_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.persistent-membership.proof-set";
    const VERIFIER_TRANSCRIPT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.persistent-membership.verifier-transcript-set";
}
impl ExactEightChunkMembershipRoleV1 for RkgEphemeralMembershipRoleV1 {
    const MAGIC: [u8; 4] = *b"ZRME";
    const BOUND: ZkAmsT256MembershipBoundV1 = ZkAmsT256MembershipBoundV1::One;
    const PROOF_BYTES: usize = BOUND_ONE_PROOF_BYTES_V1;
    const CHUNK_WIRE_BYTES: usize = BOUND_ONE_CHUNK_WIRE_BYTES_V1;
    const WIRE_BYTES: usize = ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1;
    const CONTEXT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.rkg-ephemeral-membership.context";
    const COMMITMENT_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.rkg-ephemeral-membership.commitment-set";
    const PROOF_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.rkg-ephemeral-membership.proof-set";
    const VERIFIER_TRANSCRIPT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.rkg-ephemeral-membership.verifier-transcript-set";
}
impl ExactEightChunkMembershipRoleV1 for CpkErrorMembershipRoleV1 {
    const MAGIC: [u8; 4] = *b"ZCEM";
    const BOUND: ZkAmsT256MembershipBoundV1 = ZkAmsT256MembershipBoundV1::Two;
    const PROOF_BYTES: usize = BOUND_TWO_PROOF_BYTES_V1;
    const CHUNK_WIRE_BYTES: usize = BOUND_TWO_CHUNK_WIRE_BYTES_V1;
    const WIRE_BYTES: usize = ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_WIRE_BYTES_V1;
    const CONTEXT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.cpk-error-membership.context";
    const COMMITMENT_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.cpk-error-membership.commitment-set";
    const PROOF_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.cpk-error-membership.proof-set";
    const VERIFIER_TRANSCRIPT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.cpk-error-membership.verifier-transcript-set";
}
impl ExactEightChunkMembershipRoleV1 for DirectRelationBoundOneMembershipRoleV1 {
    const MAGIC: [u8; 4] = *b"ZDB1";
    const BOUND: ZkAmsT256MembershipBoundV1 = ZkAmsT256MembershipBoundV1::One;
    const PROOF_BYTES: usize = BOUND_ONE_PROOF_BYTES_V1;
    const CHUNK_WIRE_BYTES: usize = BOUND_ONE_CHUNK_WIRE_BYTES_V1;
    const WIRE_BYTES: usize = ZK_AMS_MKHE_DIRECT_BOUND_ONE_MEMBERSHIP_WIRE_BYTES_V1;
    const CONTEXT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.direct-relation-membership.bound-one.context";
    const COMMITMENT_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.direct-relation-membership.bound-one.commitment-set";
    const PROOF_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.direct-relation-membership.bound-one.proof-set";
    const VERIFIER_TRANSCRIPT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.direct-relation-membership.bound-one.verifier-transcript-set";
}
impl ExactEightChunkMembershipRoleV1 for DirectRelationBoundTwoMembershipRoleV1 {
    const MAGIC: [u8; 4] = *b"ZDB2";
    const BOUND: ZkAmsT256MembershipBoundV1 = ZkAmsT256MembershipBoundV1::Two;
    const PROOF_BYTES: usize = BOUND_TWO_PROOF_BYTES_V1;
    const CHUNK_WIRE_BYTES: usize = BOUND_TWO_CHUNK_WIRE_BYTES_V1;
    const WIRE_BYTES: usize = ZK_AMS_MKHE_DIRECT_BOUND_TWO_MEMBERSHIP_WIRE_BYTES_V1;
    const CONTEXT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.direct-relation-membership.bound-two.context";
    const COMMITMENT_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.direct-relation-membership.bound-two.commitment-set";
    const PROOF_SET_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.direct-relation-membership.bound-two.proof-set";
    const VERIFIER_TRANSCRIPT_DIGEST_DOMAIN: &'static [u8] =
        b"iroha.zk-ams.v1.mkhe.direct-relation-membership.bound-two.verifier-transcript-set";
}
/// Stable failures shared by the three sealed exact-eight-chunk roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum ExactEightChunkMembershipErrorV1 {
    /// One or more source-context axes are zero.
    #[error("invalid ZK-AMS exact-membership context")]
    Context,
    /// The outer or inner proof-set shape is not the exact release shape.
    #[error("invalid ZK-AMS exact-membership proof-set shape")]
    Shape,
    /// The wire is truncated, extended, malformed, or non-canonical.
    #[error("invalid canonical ZK-AMS exact-membership wire encoding")]
    WireEncoding,
    /// A stored ordered-set digest does not recompute from the evidence.
    #[error("invalid ZK-AMS exact-membership ordered-set digest")]
    DigestMismatch,
    /// The encoded generator basis is not the pinned production basis.
    #[error("invalid ZK-AMS exact-membership generator basis")]
    GeneratorBasis,
    /// One T256 coefficient-membership proof failed.
    #[error(transparent)]
    Membership(#[from] ZkAmsT256MembershipErrorV1),
}
/// Complete public context absorbed by every chunk and ordered-set root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExactEightChunkMembershipContextV1<R> {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    cpk_transcript_digest: [u8; 32],
    party: ZkAmsMkhePartyIdV1,
    share_statement_digest: [u8; 32],
    role: PhantomData<fn() -> R>,
}
impl<R: ExactEightChunkMembershipRoleV1> ExactEightChunkMembershipContextV1<R> {
    /// Construct an exact nonzero seven-axis context for one sealed role.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        profile_digest: [u8; 32],
        roster_digest: [u8; 32],
        key_material_digest: [u8; 32],
        epoch: u64,
        cpk_transcript_digest: [u8; 32],
        party: ZkAmsMkhePartyIdV1,
        share_statement_digest: [u8; 32],
    ) -> Result<Self, ExactEightChunkMembershipErrorV1> {
        let context = Self {
            profile_digest,
            roster_digest,
            key_material_digest,
            epoch,
            cpk_transcript_digest,
            party,
            share_statement_digest,
            role: PhantomData,
        };
        context.validate()?;
        Ok(context)
    }
    pub(super) fn validate(self) -> Result<(), ExactEightChunkMembershipErrorV1> {
        if self.profile_digest == [0; 32]
            || self.roster_digest == [0; 32]
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.cpk_transcript_digest == [0; 32]
            || self.party.to_bytes() == [0; 32]
            || self.share_statement_digest == [0; 32]
        {
            return Err(ExactEightChunkMembershipErrorV1::Context);
        }
        Ok(())
    }
    pub(super) const fn profile_digest(self) -> [u8; 32] {
        self.profile_digest
    }
    pub(super) const fn roster_digest(self) -> [u8; 32] {
        self.roster_digest
    }
    pub(super) const fn key_material_digest(self) -> [u8; 32] {
        self.key_material_digest
    }
    pub(super) const fn epoch(self) -> u64 {
        self.epoch
    }
    pub(super) const fn cpk_transcript_digest(self) -> [u8; 32] {
        self.cpk_transcript_digest
    }
    pub(super) const fn party(self) -> ZkAmsMkhePartyIdV1 {
        self.party
    }
    pub(super) const fn share_statement_digest(self) -> [u8; 32] {
        self.share_statement_digest
    }
    /// Role-separated digest absorbed by every chunk proof and ordered root.
    pub(super) fn context_digest(self) -> [u8; 32] {
        let mut hash = Keccak256::new();
        hash.update(R::CONTEXT_DIGEST_DOMAIN);
        hash.update(&[EXACT_MEMBERSHIP_VERSION_V1]);
        hash.update(&self.profile_digest);
        hash.update(&self.roster_digest);
        hash.update(&self.key_material_digest);
        hash.update(&self.epoch.to_be_bytes());
        hash.update(&self.cpk_transcript_digest);
        hash.update(&self.party.to_bytes());
        hash.update(&self.share_statement_digest);
        hash.finalize()
    }
}
/// Allocation-free, role-typed view of one canonically preflighted evidence frame.
///
/// Construction scans every wrapper, point, scalar, digest root, and terminal
/// offset. Owned proof buffers can only be materialized by consuming this view.
pub(super) struct PreflightedExactEightChunkMembershipWireV1<'a, R> {
    bytes: &'a [u8],
    context: ExactEightChunkMembershipContextV1<R>,
    commitments: [Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    commitment_set_digest: [u8; 32],
    proof_set_digest: [u8; 32],
    verifier_transcript_digest: [u8; 32],
}
impl<'a, R: ExactEightChunkMembershipRoleV1> PreflightedExactEightChunkMembershipWireV1<'a, R> {
    pub(super) fn preflight(bytes: &'a [u8]) -> Result<Self, ExactEightChunkMembershipErrorV1> {
        if bytes.len() != R::WIRE_BYTES
            || bytes[..4] != R::MAGIC
            || bytes[4] != EXACT_MEMBERSHIP_VERSION_V1
            || bytes[OFFSET_BOUND_V1] != R::BOUND as u8
            || usize::from(bytes[OFFSET_CHUNK_COUNT_V1]) != ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1
            || u32::from_be_bytes(array_at::<4>(bytes, OFFSET_COEFFICIENT_COUNT_V1)?)
                != u32::try_from(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
                    .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?
        {
            return Err(ExactEightChunkMembershipErrorV1::WireEncoding);
        }
        let generator_basis_digest = array_at::<32>(bytes, OFFSET_GENERATOR_BASIS_DIGEST_V1)?;
        if generator_basis_digest != ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1 {
            return Err(ExactEightChunkMembershipErrorV1::GeneratorBasis);
        }
        let context = ExactEightChunkMembershipContextV1::new(
            array_at::<32>(bytes, OFFSET_PROFILE_DIGEST_V1)?,
            array_at::<32>(bytes, OFFSET_ROSTER_DIGEST_V1)?,
            array_at::<32>(bytes, OFFSET_KEY_MATERIAL_DIGEST_V1)?,
            u64::from_be_bytes(array_at::<8>(bytes, OFFSET_EPOCH_V1)?),
            array_at::<32>(bytes, OFFSET_CPK_TRANSCRIPT_DIGEST_V1)?,
            ZkAmsMkhePartyIdV1::new(array_at::<32>(bytes, OFFSET_PARTY_V1)?)
                .map_err(|_| ExactEightChunkMembershipErrorV1::Context)?,
            array_at::<32>(bytes, OFFSET_SHARE_STATEMENT_DIGEST_V1)?,
        )?;
        let commitment_set_digest = array_at::<32>(bytes, OFFSET_COMMITMENT_SET_DIGEST_V1)?;
        let proof_set_digest = array_at::<32>(bytes, OFFSET_PROOF_SET_DIGEST_V1)?;
        let verifier_transcript_digest =
            array_at::<32>(bytes, OFFSET_VERIFIER_TRANSCRIPT_DIGEST_V1)?;
        let mut commitments = [Point::identity(); ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1];
        let mut terminal = EXACT_MEMBERSHIP_HEADER_BYTES_V1;
        for (index, commitment) in commitments.iter_mut().enumerate() {
            let start = EXACT_MEMBERSHIP_HEADER_BYTES_V1
                .checked_add(
                    index
                        .checked_mul(R::CHUNK_WIRE_BYTES)
                        .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?,
                )
                .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
            let end = start
                .checked_add(R::CHUNK_WIRE_BYTES)
                .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
            let chunk = bytes
                .get(start..end)
                .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
            *commitment = preflight_zk_ams_t256_membership_chunk_wire_v1(
                chunk,
                u16::try_from(index).map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?,
                R::BOUND,
                ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1,
            )?;
            terminal = end;
        }
        if terminal != bytes.len() {
            return Err(ExactEightChunkMembershipErrorV1::WireEncoding);
        }
        let context_digest = context.context_digest();
        if context_digest == [0; 32]
            || commitment_set_digest == [0; 32]
            || proof_set_digest == [0; 32]
            || verifier_transcript_digest == [0; 32]
            || commitment_set_digest_from_points::<R>(generator_basis_digest, &commitments)?
                != commitment_set_digest
            || proof_set_digest_from_borrowed_wire::<R>(
                context_digest,
                generator_basis_digest,
                bytes,
            )? != proof_set_digest
        {
            return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
        }
        Ok(Self {
            bytes,
            context,
            commitments,
            commitment_set_digest,
            proof_set_digest,
            verifier_transcript_digest,
        })
    }
    #[cfg(test)]
    pub(super) fn materialize(
        self,
    ) -> Result<ExactEightChunkMembershipEvidenceV1<R>, ExactEightChunkMembershipErrorV1> {
        ExactEightChunkMembershipEvidenceV1::from_wire_bytes_exact(self.bytes)
    }
    /// Replay all eight borrowed chunk wires without constructing owned proofs.
    ///
    /// The preflighted frame remains reusable. This returns no capability or receipt and retains
    /// only the eight transcript digests needed to recheck the exact ordered transcript-set root.
    pub(super) fn verify_replayable(&self) -> Result<(), ExactEightChunkMembershipErrorV1> {
        ensure_canonical_generator_basis()?;
        self.verify_replayable_with(|context_digest, ordinal, wire| {
            verify_zk_ams_t256_membership_chunk_wire_v1(context_digest, ordinal, R::BOUND, wire)
                .map_err(Into::into)
        })
    }
    fn verify_replayable_with<F>(
        &self,
        mut verify_chunk: F,
    ) -> Result<(), ExactEightChunkMembershipErrorV1>
    where
        F: FnMut([u8; 32], u16, &[u8]) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1>,
    {
        let context_digest = self.context.context_digest();
        let mut transcript_digests = [[0_u8; 32]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1];
        for (index, transcript_digest) in transcript_digests.iter_mut().enumerate() {
            let start = EXACT_MEMBERSHIP_HEADER_BYTES_V1
                .checked_add(
                    index
                        .checked_mul(R::CHUNK_WIRE_BYTES)
                        .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?,
                )
                .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
            let end = start
                .checked_add(R::CHUNK_WIRE_BYTES)
                .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
            *transcript_digest = verify_chunk(
                context_digest,
                u16::try_from(index).map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?,
                self.bytes
                    .get(start..end)
                    .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?,
            )?;
            if *transcript_digest == [0; 32] {
                return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
            }
        }
        let recomputed = verifier_transcript_set_digest::<R>(
            context_digest,
            ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1,
            &transcript_digests,
        );
        if self.verifier_transcript_digest == [0; 32]
            || recomputed != self.verifier_transcript_digest
        {
            return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn verify_replayable_with_for_test<F>(
        &self,
        verify_chunk: F,
    ) -> Result<(), ExactEightChunkMembershipErrorV1>
    where
        F: FnMut([u8; 32], u16, &[u8]) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1>,
    {
        self.verify_replayable_with(verify_chunk)
    }
    pub(super) const fn context(&self) -> ExactEightChunkMembershipContextV1<R> {
        self.context
    }
    pub(super) const fn commitments(&self) -> &[Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] {
        &self.commitments
    }
    pub(super) const fn commitment_set_digest(&self) -> [u8; 32] {
        self.commitment_set_digest
    }
    pub(super) const fn proof_set_digest(&self) -> [u8; 32] {
        self.proof_set_digest
    }
    pub(super) const fn verifier_transcript_digest(&self) -> [u8; 32] {
        self.verifier_transcript_digest
    }
}
/// Canonical public evidence for one compile-time-selected membership role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ExactEightChunkMembershipEvidenceV1<R> {
    context: ExactEightChunkMembershipContextV1<R>,
    generator_basis_digest: [u8; 32],
    chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    commitment_set_digest: [u8; 32],
    proof_set_digest: [u8; 32],
    verifier_transcript_digest: [u8; 32],
}
impl<R: ExactEightChunkMembershipRoleV1> ExactEightChunkMembershipEvidenceV1<R> {
    /// Prove and locally verify all eight production-shape chunks.
    pub(super) fn prove<Random: ProofRandomSource>(
        context: ExactEightChunkMembershipContextV1<R>,
        coefficients: &[i8],
        blindings: &[Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
        random: &mut Random,
    ) -> Result<Self, ExactEightChunkMembershipErrorV1> {
        context.validate()?;
        if coefficients.len() != ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 {
            return Err(ExactEightChunkMembershipErrorV1::Shape);
        }
        ensure_canonical_generator_basis()?;
        let context_digest = context.context_digest();
        if context_digest == [0; 32] {
            return Err(ExactEightChunkMembershipErrorV1::Context);
        }
        let mut chunks = Vec::with_capacity(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1);
        let mut prover_transcripts = [[0_u8; 32]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1];
        for (index, coefficients) in coefficients
            .chunks_exact(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
            .enumerate()
        {
            let ordinal =
                u16::try_from(index).map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?;
            let (proof, transcript_digest) = prove_zk_ams_t256_membership_chunk_v1(
                context_digest,
                ordinal,
                R::BOUND,
                coefficients,
                &blindings[index],
                random,
            )?;
            chunks.push(proof);
            prover_transcripts[index] = transcript_digest;
        }
        let chunks = chunks
            .try_into()
            .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?;
        let evidence = Self::from_proof_chunks_verified(context, chunks)?;
        if verifier_transcript_set_digest::<R>(
            context_digest,
            evidence.generator_basis_digest,
            &prover_transcripts,
        ) != evidence.verifier_transcript_digest
        {
            return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
        }
        Ok(evidence)
    }
    /// Verify and assemble an exact ordered set of externally supplied chunks.
    pub(super) fn from_proof_chunks_verified(
        context: ExactEightChunkMembershipContextV1<R>,
        chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    ) -> Result<Self, ExactEightChunkMembershipErrorV1> {
        context.validate()?;
        validate_chunk_shape::<R>(&chunks)?;
        ensure_canonical_generator_basis()?;
        let context_digest = context.context_digest();
        let mut transcript_digests = [[0_u8; 32]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1];
        for (index, chunk) in chunks.iter().enumerate() {
            transcript_digests[index] = verify_zk_ams_t256_membership_chunk_v1(
                context_digest,
                u16::try_from(index).map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?,
                R::BOUND,
                chunk,
            )?;
        }
        Self::assemble(context, chunks, transcript_digests)
    }
    fn assemble(
        context: ExactEightChunkMembershipContextV1<R>,
        chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
        transcript_digests: [[u8; 32]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    ) -> Result<Self, ExactEightChunkMembershipErrorV1> {
        context.validate()?;
        validate_chunk_shape::<R>(&chunks)?;
        if transcript_digests.contains(&[0; 32]) {
            return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
        }
        let generator_basis_digest = ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1;
        let context_digest = context.context_digest();
        let evidence = Self {
            context,
            generator_basis_digest,
            commitment_set_digest: commitment_set_digest::<R>(generator_basis_digest, &chunks)?,
            proof_set_digest: proof_set_digest::<R>(
                context_digest,
                generator_basis_digest,
                &chunks,
            )?,
            verifier_transcript_digest: verifier_transcript_set_digest::<R>(
                context_digest,
                generator_basis_digest,
                &transcript_digests,
            ),
            chunks,
        };
        evidence.validate_structural_digests()?;
        Ok(evidence)
    }
    /// Reconstruct a decoded public container and recheck its structural roots.
    ///
    /// This does not verify any chunk proof and therefore cannot return a
    /// verified capability.  It exists for role-specific facade types which
    /// retain their established field/API layout.
    pub(super) fn from_structural_parts(
        context: ExactEightChunkMembershipContextV1<R>,
        generator_basis_digest: [u8; 32],
        chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
        commitment_set_digest: [u8; 32],
        proof_set_digest: [u8; 32],
        verifier_transcript_digest: [u8; 32],
    ) -> Result<Self, ExactEightChunkMembershipErrorV1> {
        let evidence = Self {
            context,
            generator_basis_digest,
            chunks,
            commitment_set_digest,
            proof_set_digest,
            verifier_transcript_digest,
        };
        evidence.validate_structural_digests()?;
        Ok(evidence)
    }
    /// Strictly decode one exact role-specific canonical evidence frame.
    pub(super) fn from_wire_bytes_exact(
        bytes: &[u8],
    ) -> Result<Self, ExactEightChunkMembershipErrorV1> {
        if bytes.len() != R::WIRE_BYTES
            || bytes[..4] != R::MAGIC
            || bytes[4] != EXACT_MEMBERSHIP_VERSION_V1
            || bytes[OFFSET_BOUND_V1] != R::BOUND as u8
            || usize::from(bytes[OFFSET_CHUNK_COUNT_V1]) != ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1
            || u32::from_be_bytes(array_at::<4>(bytes, OFFSET_COEFFICIENT_COUNT_V1)?)
                != u32::try_from(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
                    .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?
        {
            return Err(ExactEightChunkMembershipErrorV1::WireEncoding);
        }
        let generator_basis_digest = array_at::<32>(bytes, OFFSET_GENERATOR_BASIS_DIGEST_V1)?;
        if generator_basis_digest != ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1 {
            return Err(ExactEightChunkMembershipErrorV1::GeneratorBasis);
        }
        let context = ExactEightChunkMembershipContextV1::new(
            array_at::<32>(bytes, OFFSET_PROFILE_DIGEST_V1)?,
            array_at::<32>(bytes, OFFSET_ROSTER_DIGEST_V1)?,
            array_at::<32>(bytes, OFFSET_KEY_MATERIAL_DIGEST_V1)?,
            u64::from_be_bytes(array_at::<8>(bytes, OFFSET_EPOCH_V1)?),
            array_at::<32>(bytes, OFFSET_CPK_TRANSCRIPT_DIGEST_V1)?,
            ZkAmsMkhePartyIdV1::new(array_at::<32>(bytes, OFFSET_PARTY_V1)?)
                .map_err(|_| ExactEightChunkMembershipErrorV1::Context)?,
            array_at::<32>(bytes, OFFSET_SHARE_STATEMENT_DIGEST_V1)?,
        )?;
        let commitment_set_digest = array_at::<32>(bytes, OFFSET_COMMITMENT_SET_DIGEST_V1)?;
        let proof_set_digest = array_at::<32>(bytes, OFFSET_PROOF_SET_DIGEST_V1)?;
        let verifier_transcript_digest =
            array_at::<32>(bytes, OFFSET_VERIFIER_TRANSCRIPT_DIGEST_V1)?;
        let mut chunks = Vec::with_capacity(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1);
        for index in 0..ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 {
            let start = EXACT_MEMBERSHIP_HEADER_BYTES_V1
                .checked_add(
                    index
                        .checked_mul(R::CHUNK_WIRE_BYTES)
                        .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?,
                )
                .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
            let end = start
                .checked_add(R::CHUNK_WIRE_BYTES)
                .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
            chunks.push(ZkAmsT256MembershipProofV1::from_wire_bytes_exact(
                bytes
                    .get(start..end)
                    .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?,
            )?);
        }
        let evidence = Self {
            context,
            generator_basis_digest,
            chunks: chunks
                .try_into()
                .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?,
            commitment_set_digest,
            proof_set_digest,
            verifier_transcript_digest,
        };
        evidence.validate_structural_digests()?;
        Ok(evidence)
    }
    /// Encode the fixed-layout role-specific representation after rechecking roots.
    pub(super) fn to_wire_bytes(&self) -> Result<Vec<u8>, ExactEightChunkMembershipErrorV1> {
        self.validate_structural_digests()?;
        let coefficient_count = u32::try_from(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
            .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?;
        let mut bytes = Vec::with_capacity(R::WIRE_BYTES);
        bytes.extend_from_slice(&R::MAGIC);
        bytes.push(EXACT_MEMBERSHIP_VERSION_V1);
        bytes.push(R::BOUND as u8);
        bytes.push(
            u8::try_from(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1)
                .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?,
        );
        bytes.extend_from_slice(&coefficient_count.to_be_bytes());
        bytes.extend_from_slice(&self.generator_basis_digest);
        bytes.extend_from_slice(&self.context.profile_digest);
        bytes.extend_from_slice(&self.context.roster_digest);
        bytes.extend_from_slice(&self.context.key_material_digest);
        bytes.extend_from_slice(&self.context.epoch.to_be_bytes());
        bytes.extend_from_slice(&self.context.cpk_transcript_digest);
        bytes.extend_from_slice(&self.context.party.to_bytes());
        bytes.extend_from_slice(&self.context.share_statement_digest);
        bytes.extend_from_slice(&self.commitment_set_digest);
        bytes.extend_from_slice(&self.proof_set_digest);
        bytes.extend_from_slice(&self.verifier_transcript_digest);
        for chunk in &self.chunks {
            let chunk_wire = chunk.to_wire_bytes();
            if chunk_wire.len() != R::CHUNK_WIRE_BYTES {
                return Err(ExactEightChunkMembershipErrorV1::Shape);
            }
            bytes.extend_from_slice(&chunk_wire);
        }
        if bytes.len() != R::WIRE_BYTES {
            return Err(ExactEightChunkMembershipErrorV1::WireEncoding);
        }
        Ok(bytes)
    }
    /// Replay all eight proofs and return a move-only membership capability.
    pub(super) fn into_verified(
        self,
    ) -> Result<VerifiedExactEightChunkMembershipV1<R>, ExactEightChunkMembershipErrorV1> {
        ensure_canonical_generator_basis()?;
        self.verify_with(|context_digest, ordinal, chunk| {
            verify_zk_ams_t256_membership_chunk_v1(context_digest, ordinal, R::BOUND, chunk)
                .map_err(Into::into)
        })
    }
    /// Replay all proofs without retaining the resulting typed capability.
    pub(super) fn verify(&self) -> Result<(), ExactEightChunkMembershipErrorV1> {
        ensure_canonical_generator_basis()?;
        self.verify_with_ref(|context_digest, ordinal, chunk| {
            verify_zk_ams_t256_membership_chunk_v1(context_digest, ordinal, R::BOUND, chunk)
                .map_err(Into::into)
        })
        .map(|_| ())
    }
    fn verify_with<F>(
        self,
        verify_chunk: F,
    ) -> Result<VerifiedExactEightChunkMembershipV1<R>, ExactEightChunkMembershipErrorV1>
    where
        F: FnMut(
            [u8; 32],
            u16,
            &ZkAmsT256MembershipProofV1,
        ) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1>,
    {
        self.verify_with_ref(verify_chunk)?;
        Ok(VerifiedExactEightChunkMembershipV1 {
            context: self.context,
            generator_basis_digest: self.generator_basis_digest,
            commitments: self.commitments(),
            commitment_set_digest: self.commitment_set_digest,
            proof_set_digest: self.proof_set_digest,
            verifier_transcript_digest: self.verifier_transcript_digest,
            role: PhantomData,
        })
    }
    fn verify_with_ref<F>(
        &self,
        mut verify_chunk: F,
    ) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1>
    where
        F: FnMut(
            [u8; 32],
            u16,
            &ZkAmsT256MembershipProofV1,
        ) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1>,
    {
        self.validate_structural_digests()?;
        let context_digest = self.context.context_digest();
        let mut transcript_digests = [[0_u8; 32]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1];
        for (index, chunk) in self.chunks.iter().enumerate() {
            transcript_digests[index] = verify_chunk(
                context_digest,
                u16::try_from(index).map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?,
                chunk,
            )?;
            if transcript_digests[index] == [0; 32] {
                return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
            }
        }
        let recomputed = verifier_transcript_set_digest::<R>(
            context_digest,
            self.generator_basis_digest,
            &transcript_digests,
        );
        if self.verifier_transcript_digest == [0; 32]
            || recomputed != self.verifier_transcript_digest
        {
            return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
        }
        Ok(recomputed)
    }
    fn validate_structural_digests(&self) -> Result<(), ExactEightChunkMembershipErrorV1> {
        self.context.validate()?;
        if self.generator_basis_digest != ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1 {
            return Err(ExactEightChunkMembershipErrorV1::GeneratorBasis);
        }
        validate_chunk_shape::<R>(&self.chunks)?;
        let context_digest = self.context.context_digest();
        if context_digest == [0; 32]
            || self.commitment_set_digest == [0; 32]
            || self.proof_set_digest == [0; 32]
            || self.verifier_transcript_digest == [0; 32]
            || commitment_set_digest::<R>(self.generator_basis_digest, &self.chunks)?
                != self.commitment_set_digest
            || proof_set_digest::<R>(context_digest, self.generator_basis_digest, &self.chunks)?
                != self.proof_set_digest
        {
            return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
        }
        Ok(())
    }
    pub(super) const fn context(&self) -> ExactEightChunkMembershipContextV1<R> {
        self.context
    }
    pub(super) const fn chunks(
        &self,
    ) -> &[ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] {
        &self.chunks
    }
    pub(super) fn commitments(&self) -> [Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] {
        core::array::from_fn(|index| self.chunks[index].commitment())
    }
    pub(super) const fn generator_basis_digest(&self) -> [u8; 32] {
        self.generator_basis_digest
    }
    pub(super) const fn commitment_set_digest(&self) -> [u8; 32] {
        self.commitment_set_digest
    }
    pub(super) const fn proof_set_digest(&self) -> [u8; 32] {
        self.proof_set_digest
    }
    pub(super) const fn verifier_transcript_digest(&self) -> [u8; 32] {
        self.verifier_transcript_digest
    }
    #[cfg(test)]
    pub(super) fn assemble_for_test(
        context: ExactEightChunkMembershipContextV1<R>,
        chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
        transcript_digests: [[u8; 32]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    ) -> Result<Self, ExactEightChunkMembershipErrorV1> {
        Self::assemble(context, chunks, transcript_digests)
    }
    #[cfg(test)]
    pub(super) fn into_verified_with_for_test<F>(
        self,
        verify_chunk: F,
    ) -> Result<VerifiedExactEightChunkMembershipV1<R>, ExactEightChunkMembershipErrorV1>
    where
        F: FnMut(
            [u8; 32],
            u16,
            &ZkAmsT256MembershipProofV1,
        ) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1>,
    {
        self.verify_with(verify_chunk)
    }
    #[cfg(test)]
    pub(super) fn verify_with_for_test<F>(
        &self,
        verify_chunk: F,
    ) -> Result<(), ExactEightChunkMembershipErrorV1>
    where
        F: FnMut(
            [u8; 32],
            u16,
            &ZkAmsT256MembershipProofV1,
        ) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1>,
    {
        self.verify_with_ref(verify_chunk).map(|_| ())
    }
}
/// Move-only proof-verified membership capability for exactly one sealed role.
///
/// This capability deliberately has no relation or active-binding conversion. It records membership
/// provenance only; a complete native CPK equation and authentication verifier must consume it
/// together with the other relation objects before minting any reusable witness lineage.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct VerifiedExactEightChunkMembershipV1<R> {
    context: ExactEightChunkMembershipContextV1<R>,
    generator_basis_digest: [u8; 32],
    commitments: [Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    commitment_set_digest: [u8; 32],
    proof_set_digest: [u8; 32],
    verifier_transcript_digest: [u8; 32],
    role: PhantomData<fn() -> R>,
}
impl<R: ExactEightChunkMembershipRoleV1> VerifiedExactEightChunkMembershipV1<R> {
    pub(super) const fn context(&self) -> ExactEightChunkMembershipContextV1<R> {
        self.context
    }
    pub(super) const fn commitments(&self) -> &[Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] {
        &self.commitments
    }
    pub(super) const fn generator_basis_digest(&self) -> [u8; 32] {
        self.generator_basis_digest
    }
    pub(super) const fn commitment_set_digest(&self) -> [u8; 32] {
        self.commitment_set_digest
    }
    pub(super) const fn proof_set_digest(&self) -> [u8; 32] {
        self.proof_set_digest
    }
    pub(super) const fn verifier_transcript_digest(&self) -> [u8; 32] {
        self.verifier_transcript_digest
    }
}
fn ensure_canonical_generator_basis() -> Result<(), ExactEightChunkMembershipErrorV1> {
    if zk_ams_t256_bulletproof_generator_basis_digest_v1()
        != ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1
    {
        return Err(ExactEightChunkMembershipErrorV1::GeneratorBasis);
    }
    Ok(())
}
fn validate_chunk_shape<R: ExactEightChunkMembershipRoleV1>(
    chunks: &[ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) -> Result<(), ExactEightChunkMembershipErrorV1> {
    for (index, chunk) in chunks.iter().enumerate() {
        if chunk.bound() != R::BOUND
            || usize::from(chunk.chunk_ordinal()) != index
            || usize::try_from(chunk.coefficient_count()).ok()
                != Some(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
            || chunk.proof_bytes().len() != R::PROOF_BYTES
            || chunk.commitment().is_identity()
            || chunk.to_wire_bytes().len() != R::CHUNK_WIRE_BYTES
        {
            return Err(ExactEightChunkMembershipErrorV1::Shape);
        }
    }
    Ok(())
}
fn digest_shape_prefix<R: ExactEightChunkMembershipRoleV1>(
    hash: &mut Keccak256,
    domain: &[u8],
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
) {
    hash.update(domain);
    hash.update(&[EXACT_MEMBERSHIP_VERSION_V1]);
    hash.update(&context_digest);
    hash.update(&generator_basis_digest);
    hash.update(&[R::BOUND as u8]);
    hash.update(
        &u32::try_from(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
            .expect("fixed membership coefficient count fits u32")
            .to_be_bytes(),
    );
    hash.update(&[ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 as u8]);
}
pub(super) fn commitment_set_digest<R: ExactEightChunkMembershipRoleV1>(
    generator_basis_digest: [u8; 32],
    chunks: &[ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1> {
    commitment_set_digest_from_points::<R>(
        generator_basis_digest,
        &core::array::from_fn(|index| chunks[index].commitment()),
    )
}
fn commitment_set_digest_from_points<R: ExactEightChunkMembershipRoleV1>(
    generator_basis_digest: [u8; 32],
    commitments: &[Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(R::COMMITMENT_SET_DIGEST_DOMAIN);
    hash.update(&generator_basis_digest);
    hash.update(
        &u32::try_from(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
            .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?
            .to_be_bytes(),
    );
    hash.update(&[ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 as u8]);
    for (index, commitment) in commitments.iter().enumerate() {
        hash.update(
            &u32::try_from(index)
                .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?
                .to_be_bytes(),
        );
        hash.update(
            &commitment
                .to_non_identity_wire_bytes()
                .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?,
        );
    }
    Ok(hash.finalize())
}
fn proof_set_digest_from_borrowed_wire<R: ExactEightChunkMembershipRoleV1>(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    bytes: &[u8],
) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1> {
    if bytes.len() != R::WIRE_BYTES {
        return Err(ExactEightChunkMembershipErrorV1::WireEncoding);
    }
    let mut hash = Keccak256::new();
    digest_shape_prefix::<R>(
        &mut hash,
        R::PROOF_SET_DIGEST_DOMAIN,
        context_digest,
        generator_basis_digest,
    );
    let mut terminal = EXACT_MEMBERSHIP_HEADER_BYTES_V1;
    for index in 0..ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 {
        let start = EXACT_MEMBERSHIP_HEADER_BYTES_V1
            .checked_add(
                index
                    .checked_mul(R::CHUNK_WIRE_BYTES)
                    .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?,
            )
            .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
        let end = start
            .checked_add(R::CHUNK_WIRE_BYTES)
            .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
        let wire = bytes
            .get(start..end)
            .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
        hash.update(
            &u16::try_from(index)
                .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?
                .to_be_bytes(),
        );
        hash.update(
            &u16::try_from(wire.len())
                .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?
                .to_be_bytes(),
        );
        hash.update(wire);
        terminal = end;
    }
    if terminal != bytes.len() {
        return Err(ExactEightChunkMembershipErrorV1::WireEncoding);
    }
    Ok(hash.finalize())
}
pub(super) fn proof_set_digest<R: ExactEightChunkMembershipRoleV1>(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    chunks: &[ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1> {
    let mut hash = Keccak256::new();
    digest_shape_prefix::<R>(
        &mut hash,
        R::PROOF_SET_DIGEST_DOMAIN,
        context_digest,
        generator_basis_digest,
    );
    for (index, chunk) in chunks.iter().enumerate() {
        let wire = chunk.to_wire_bytes();
        if wire.len() != R::CHUNK_WIRE_BYTES {
            return Err(ExactEightChunkMembershipErrorV1::Shape);
        }
        hash.update(
            &u16::try_from(index)
                .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?
                .to_be_bytes(),
        );
        hash.update(
            &u16::try_from(wire.len())
                .map_err(|_| ExactEightChunkMembershipErrorV1::Shape)?
                .to_be_bytes(),
        );
        hash.update(&wire);
    }
    Ok(hash.finalize())
}
pub(super) fn verifier_transcript_set_digest<R: ExactEightChunkMembershipRoleV1>(
    context_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    transcript_digests: &[[u8; 32]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    digest_shape_prefix::<R>(
        &mut hash,
        R::VERIFIER_TRANSCRIPT_DIGEST_DOMAIN,
        context_digest,
        generator_basis_digest,
    );
    for (index, digest) in transcript_digests.iter().enumerate() {
        hash.update(
            &u16::try_from(index)
                .expect("fixed exact-membership chunk index fits u16")
                .to_be_bytes(),
        );
        hash.update(digest);
    }
    hash.finalize()
}
fn array_at<const N: usize>(
    bytes: &[u8],
    offset: usize,
) -> Result<[u8; N], ExactEightChunkMembershipErrorV1> {
    let end = offset
        .checked_add(N)
        .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?;
    bytes
        .get(offset..end)
        .ok_or(ExactEightChunkMembershipErrorV1::WireEncoding)?
        .try_into()
        .map_err(|_| ExactEightChunkMembershipErrorV1::WireEncoding)
}
#[cfg(test)]
pub(super) fn canonical_membership_syntax_wire_fixture_for_test<
    R: ExactEightChunkMembershipRoleV1,
>(
    seed: &[u8],
    point_offset: usize,
) -> Vec<u8> {
    let digest = |label: &[u8]| {
        let mut hash = Keccak256::new();
        hash.update(seed);
        hash.update(label);
        hash.finalize()
    };
    let context: ExactEightChunkMembershipContextV1<R> = ExactEightChunkMembershipContextV1::new(
        digest(b"profile"),
        digest(b"roster"),
        digest(b"key-material"),
        7,
        digest(b"cpk-transcript"),
        ZkAmsMkhePartyIdV1::new(digest(b"party")).expect("fixture party"),
        digest(b"share-statement"),
    )
    .expect("fixture context");
    let points = crate::vega::derive_t256_generators_v1(
        b"iroha.zk-ams.v1.mkhe.direct-membership.syntax-fixture",
        point_offset + 1 + ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1,
    )
    .expect("fixture points");
    let proof_point = points[point_offset]
        .to_non_identity_wire_bytes()
        .expect("fixture proof point");
    let ipa_point_count = R::PROOF_BYTES
        .checked_sub(9 * core::mem::size_of::<[u8; 33]>() + 5 * core::mem::size_of::<[u8; 32]>())
        .expect("fixture proof shape")
        / core::mem::size_of::<[u8; 33]>();
    assert_eq!(ipa_point_count % 2, 0);
    let chunks = core::array::from_fn(|index| {
        let scalar = Scalar::from_u64(index as u64 + 1).to_le_bytes();
        let mut proof = Vec::with_capacity(R::PROOF_BYTES);
        for _ in 0..9 {
            proof.extend_from_slice(&proof_point);
        }
        for _ in 0..3 {
            proof.extend_from_slice(&scalar);
        }
        for _ in 0..ipa_point_count {
            proof.extend_from_slice(&proof_point);
        }
        for _ in 0..2 {
            proof.extend_from_slice(&scalar);
        }
        assert_eq!(proof.len(), R::PROOF_BYTES);
        let mut wire = Vec::with_capacity(R::CHUNK_WIRE_BYTES);
        wire.extend_from_slice(b"ZMBP");
        wire.push(1);
        wire.push(R::BOUND as u8);
        wire.extend_from_slice(&(index as u16).to_be_bytes());
        wire.extend_from_slice(
            &u32::try_from(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
                .expect("fixed fixture count")
                .to_be_bytes(),
        );
        wire.extend_from_slice(
            &points[point_offset + 1 + index]
                .to_non_identity_wire_bytes()
                .expect("fixture commitment"),
        );
        wire.extend_from_slice(
            &u16::try_from(proof.len())
                .expect("fixed fixture proof length")
                .to_be_bytes(),
        );
        wire.extend_from_slice(&proof);
        ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wire).expect("fixture chunk")
    });
    let transcript_digests = core::array::from_fn(|index| {
        let mut hash = Keccak256::new();
        hash.update(b"iroha.zk-ams.v1.mkhe.direct-membership.syntax-fixture-transcript");
        hash.update(&context.context_digest());
        hash.update(&(index as u16).to_be_bytes());
        hash.update(&chunks[index].to_wire_bytes());
        hash.finalize()
    });
    ExactEightChunkMembershipEvidenceV1::assemble_for_test(context, chunks, transcript_digests)
        .expect("fixture evidence")
        .to_wire_bytes()
        .expect("fixture wire")
}
#[cfg(test)]
pub(super) fn repair_membership_proof_set_digest_for_test<R: ExactEightChunkMembershipRoleV1>(
    bytes: &mut [u8],
) {
    assert_eq!(bytes.len(), R::WIRE_BYTES);
    let context = ExactEightChunkMembershipContextV1::<R>::new(
        array_at::<32>(bytes, OFFSET_PROFILE_DIGEST_V1).expect("fixture profile"),
        array_at::<32>(bytes, OFFSET_ROSTER_DIGEST_V1).expect("fixture roster"),
        array_at::<32>(bytes, OFFSET_KEY_MATERIAL_DIGEST_V1).expect("fixture key material"),
        u64::from_be_bytes(array_at::<8>(bytes, OFFSET_EPOCH_V1).expect("fixture epoch")),
        array_at::<32>(bytes, OFFSET_CPK_TRANSCRIPT_DIGEST_V1).expect("fixture transcript"),
        ZkAmsMkhePartyIdV1::new(
            array_at::<32>(bytes, OFFSET_PARTY_V1).expect("fixture party bytes"),
        )
        .expect("fixture party"),
        array_at::<32>(bytes, OFFSET_SHARE_STATEMENT_DIGEST_V1).expect("fixture statement"),
    )
    .expect("fixture context");
    let generator_basis_digest =
        array_at::<32>(bytes, OFFSET_GENERATOR_BASIS_DIGEST_V1).expect("fixture basis");
    let repaired = proof_set_digest_from_borrowed_wire::<R>(
        context.context_digest(),
        generator_basis_digest,
        bytes,
    )
    .expect("fixture proof-set digest");
    bytes[OFFSET_PROOF_SET_DIGEST_V1..OFFSET_PROOF_SET_DIGEST_V1 + 32].copy_from_slice(&repaired);
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::{derive_t256_generators_v1, sponge::keccak256};
    const INNER_COMMITMENT_OFFSET_V1: usize = 12;
    const INNER_PROOF_OFFSET_V1: usize = 47;
    fn context<R: ExactEightChunkMembershipRoleV1>(
        seed: &[u8],
    ) -> ExactEightChunkMembershipContextV1<R> {
        let digest = |label: &[u8]| {
            let mut hash = Keccak256::new();
            hash.update(seed);
            hash.update(label);
            hash.finalize()
        };
        ExactEightChunkMembershipContextV1::new(
            digest(b"profile"),
            digest(b"roster"),
            digest(b"key-material"),
            7,
            digest(b"cpk-transcript"),
            ZkAmsMkhePartyIdV1::new(digest(b"party")).expect("nonzero party"),
            digest(b"share-statement"),
        )
        .expect("canonical context")
    }
    fn fake_chunks<R: ExactEightChunkMembershipRoleV1>(
        context: ExactEightChunkMembershipContextV1<R>,
        point_offset: usize,
    ) -> [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] {
        let points = derive_t256_generators_v1(
            b"iroha.zk-ams.v1.mkhe.exact-membership.test-points",
            point_offset + ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1,
        )
        .expect("test points");
        let context_digest = context.context_digest();
        core::array::from_fn(|index| {
            let mut proof = vec![index as u8; R::PROOF_BYTES];
            proof[..32].copy_from_slice(&context_digest);
            proof[32..34].copy_from_slice(&(index as u16).to_be_bytes());
            let mut wire = Vec::with_capacity(R::CHUNK_WIRE_BYTES);
            wire.extend_from_slice(b"ZMBP");
            wire.push(1);
            wire.push(R::BOUND as u8);
            wire.extend_from_slice(&(index as u16).to_be_bytes());
            wire.extend_from_slice(
                &u32::try_from(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
                    .expect("fixed count")
                    .to_be_bytes(),
            );
            wire.extend_from_slice(
                &points[point_offset + index]
                    .to_non_identity_wire_bytes()
                    .expect("test point"),
            );
            wire.extend_from_slice(
                &u16::try_from(proof.len())
                    .expect("fixed proof length")
                    .to_be_bytes(),
            );
            wire.extend_from_slice(&proof);
            ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wire).expect("synthetic chunk")
        })
    }
    fn fake_verify<R: ExactEightChunkMembershipRoleV1>(
        context_digest: [u8; 32],
        ordinal: u16,
        chunk: &ZkAmsT256MembershipProofV1,
    ) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1> {
        if chunk.bound() != R::BOUND
            || chunk.proof_bytes().get(..32) != Some(context_digest.as_slice())
            || chunk.proof_bytes().get(32..34) != Some(ordinal.to_be_bytes().as_slice())
        {
            return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
        }
        let mut hash = Keccak256::new();
        hash.update(b"iroha.zk-ams.v1.mkhe.exact-membership.test-transcript");
        hash.update(&R::MAGIC);
        hash.update(&context_digest);
        hash.update(&ordinal.to_be_bytes());
        hash.update(&chunk.to_wire_bytes());
        Ok(hash.finalize())
    }
    fn syntax_fixture_transcript_digest(
        context_digest: [u8; 32],
        ordinal: u16,
        wire: &[u8],
    ) -> [u8; 32] {
        let mut hash = Keccak256::new();
        hash.update(b"iroha.zk-ams.v1.mkhe.direct-membership.syntax-fixture-transcript");
        hash.update(&context_digest);
        hash.update(&ordinal.to_be_bytes());
        hash.update(wire);
        hash.finalize()
    }
    fn fake_evidence<R: ExactEightChunkMembershipRoleV1>(
        seed: &[u8],
        point_offset: usize,
    ) -> ExactEightChunkMembershipEvidenceV1<R> {
        let context = context::<R>(seed);
        let chunks = fake_chunks::<R>(context, point_offset);
        let transcripts = core::array::from_fn(|index| {
            fake_verify::<R>(context.context_digest(), index as u16, &chunks[index])
                .expect("fake transcript")
        });
        ExactEightChunkMembershipEvidenceV1::assemble_for_test(context, chunks, transcripts)
            .expect("synthetic evidence")
    }
    fn assert_role_roundtrip<R: ExactEightChunkMembershipRoleV1>() {
        let evidence = fake_evidence::<R>(b"role-roundtrip", 0);
        let wire = evidence.to_wire_bytes().expect("wire");
        assert_eq!(wire.len(), R::WIRE_BYTES);
        assert_eq!(&wire[..4], &R::MAGIC);
        assert_eq!(wire[OFFSET_BOUND_V1], R::BOUND as u8);
        assert_eq!(evidence.chunks().len(), 8);
        assert!(
            evidence
                .chunks()
                .iter()
                .all(|chunk| chunk.proof_bytes().len() == R::PROOF_BYTES)
        );
        let decoded =
            ExactEightChunkMembershipEvidenceV1::<R>::from_wire_bytes_exact(&wire).expect("decode");
        assert_eq!(decoded.to_wire_bytes().expect("re-encode"), wire);
        let verified = decoded
            .into_verified_with_for_test(fake_verify::<R>)
            .expect("fake verification");
        assert_eq!(verified.context(), evidence.context());
        assert_eq!(verified.commitments(), &evidence.commitments());
        assert_eq!(
            verified.generator_basis_digest(),
            ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1
        );
        assert_eq!(
            verified.commitment_set_digest(),
            evidence.commitment_set_digest()
        );
        assert_eq!(verified.proof_set_digest(), evidence.proof_set_digest());
        assert_eq!(
            verified.verifier_transcript_digest(),
            evidence.verifier_transcript_digest()
        );
    }

    #[test]
    fn borrowed_replay_visits_zero_through_seven_and_stops_on_the_first_failure() {
        let wire = canonical_membership_syntax_wire_fixture_for_test::<
            DirectRelationBoundOneMembershipRoleV1,
        >(b"borrowed-replay-order", 0);
        let view = PreflightedExactEightChunkMembershipWireV1::<
            DirectRelationBoundOneMembershipRoleV1,
        >::preflight(&wire)
        .expect("preflighted fixture");
        let mut visited = Vec::new();
        view.verify_replayable_with_for_test(|context_digest, ordinal, chunk_wire| {
            visited.push(ordinal);
            Ok(syntax_fixture_transcript_digest(
                context_digest,
                ordinal,
                chunk_wire,
            ))
        })
        .expect("borrowed replay");
        assert_eq!(visited, (0_u16..8).collect::<Vec<_>>());

        visited.clear();
        let error = view
            .verify_replayable_with_for_test(|context_digest, ordinal, chunk_wire| {
                visited.push(ordinal);
                if ordinal == 3 {
                    return Err(ExactEightChunkMembershipErrorV1::Membership(
                        ZkAmsT256MembershipErrorV1::StatementMismatch,
                    ));
                }
                Ok(syntax_fixture_transcript_digest(
                    context_digest,
                    ordinal,
                    chunk_wire,
                ))
            })
            .unwrap_err();
        assert_eq!(
            error,
            ExactEightChunkMembershipErrorV1::Membership(
                ZkAmsT256MembershipErrorV1::StatementMismatch
            )
        );
        assert_eq!(visited, [0, 1, 2, 3]);
    }

    #[test]
    fn borrowed_replay_rejects_zero_and_mismatched_transcript_roots() {
        let wire = canonical_membership_syntax_wire_fixture_for_test::<
            DirectRelationBoundTwoMembershipRoleV1,
        >(b"borrowed-replay-root", 0);
        let view = PreflightedExactEightChunkMembershipWireV1::<
            DirectRelationBoundTwoMembershipRoleV1,
        >::preflight(&wire)
        .expect("preflighted fixture");
        let mut calls = 0;
        let zero = view.verify_replayable_with_for_test(|context, ordinal, chunk_wire| {
            calls += 1;
            if ordinal == 2 {
                Ok([0; 32])
            } else {
                Ok(syntax_fixture_transcript_digest(
                    context, ordinal, chunk_wire,
                ))
            }
        });
        assert_eq!(zero, Err(ExactEightChunkMembershipErrorV1::DigestMismatch));
        assert_eq!(calls, 3);

        calls = 0;
        let mismatch = view.verify_replayable_with_for_test(|context, ordinal, chunk_wire| {
            calls += 1;
            let mut digest = syntax_fixture_transcript_digest(context, ordinal, chunk_wire);
            if ordinal == 7 {
                digest[0] ^= 1;
            }
            Ok(digest)
        });
        assert_eq!(
            mismatch,
            Err(ExactEightChunkMembershipErrorV1::DigestMismatch)
        );
        assert_eq!(calls, 8);
    }

    #[test]
    fn borrowed_replay_surface_is_conversion_and_authority_free() {
        let source = include_str!("exact_eight_chunk_membership.rs");
        let replay = source
            .split_once("    pub(super) fn verify_replayable(&self)")
            .and_then(|(_, tail)| tail.split_once("    pub(super) const fn context("))
            .map(|(body, _)| body)
            .expect("bounded borrowed replay surface");
        assert_eq!(
            replay
                .matches("verify_zk_ams_t256_membership_chunk_wire_v1(")
                .count(),
            1
        );
        assert!(replay.contains("let mut transcript_digests = [[0_u8; 32];"));
        assert!(replay.contains("verifier_transcript_set_digest::<R>("));
        for forbidden in [
            "materialize",
            "from_wire_bytes_exact",
            "to_vec",
            "Vec<",
            "ZkAmsT256MembershipProofV1",
            "VerifiedExactEightChunkMembershipV1",
            "capability",
            "receipt",
            "into_verified",
        ] {
            assert!(!replay.contains(forbidden));
        }
    }
    #[test]
    fn all_roles_have_exact_release_sizes_and_move_only_verified_outputs() {
        assert_eq!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1, 8);
        assert_eq!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1, 131_072);
        assert_eq!(PersistentSecretMembershipRoleV1::PROOF_BYTES, 1_447);
        assert_eq!(PersistentSecretMembershipRoleV1::CHUNK_WIRE_BYTES, 1_494);
        assert_eq!(PersistentSecretMembershipRoleV1::WIRE_BYTES, 12_291);
        assert_eq!(RkgEphemeralMembershipRoleV1::PROOF_BYTES, 1_447);
        assert_eq!(RkgEphemeralMembershipRoleV1::CHUNK_WIRE_BYTES, 1_494);
        assert_eq!(RkgEphemeralMembershipRoleV1::WIRE_BYTES, 12_291);
        assert_eq!(CpkErrorMembershipRoleV1::PROOF_BYTES, 1_513);
        assert_eq!(CpkErrorMembershipRoleV1::CHUNK_WIRE_BYTES, 1_560);
        assert_eq!(CpkErrorMembershipRoleV1::WIRE_BYTES, 12_819);
        assert_eq!(DirectRelationBoundOneMembershipRoleV1::MAGIC, *b"ZDB1");
        assert_eq!(DirectRelationBoundOneMembershipRoleV1::WIRE_BYTES, 12_291);
        assert_eq!(DirectRelationBoundTwoMembershipRoleV1::MAGIC, *b"ZDB2");
        assert_eq!(DirectRelationBoundTwoMembershipRoleV1::WIRE_BYTES, 12_819);
        assert_role_roundtrip::<PersistentSecretMembershipRoleV1>();
        assert_role_roundtrip::<RkgEphemeralMembershipRoleV1>();
        assert_role_roundtrip::<CpkErrorMembershipRoleV1>();
        assert_role_roundtrip::<DirectRelationBoundOneMembershipRoleV1>();
        assert_role_roundtrip::<DirectRelationBoundTwoMembershipRoleV1>();
        fn consume_persistent(
            _: VerifiedExactEightChunkMembershipV1<PersistentSecretMembershipRoleV1>,
        ) {
        }
        fn consume_ephemeral(_: VerifiedExactEightChunkMembershipV1<RkgEphemeralMembershipRoleV1>) {
        }
        fn consume_error(_: VerifiedExactEightChunkMembershipV1<CpkErrorMembershipRoleV1>) {}
        let persistent = fake_evidence::<PersistentSecretMembershipRoleV1>(b"move-only-p", 0)
            .into_verified_with_for_test(fake_verify::<PersistentSecretMembershipRoleV1>)
            .expect("persistent verified");
        let ephemeral = fake_evidence::<RkgEphemeralMembershipRoleV1>(b"move-only-u", 8)
            .into_verified_with_for_test(fake_verify::<RkgEphemeralMembershipRoleV1>)
            .expect("ephemeral verified");
        let error = fake_evidence::<CpkErrorMembershipRoleV1>(b"move-only-e", 16)
            .into_verified_with_for_test(fake_verify::<CpkErrorMembershipRoleV1>)
            .expect("error verified");
        consume_persistent(persistent);
        consume_ephemeral(ephemeral);
        consume_error(error);
    }
    fn assert_exact_length_rejection<R: ExactEightChunkMembershipRoleV1>() {
        let wire = fake_evidence::<R>(b"length-adversary", 0)
            .to_wire_bytes()
            .expect("wire");
        for end in 0..wire.len() {
            assert_eq!(
                ExactEightChunkMembershipEvidenceV1::<R>::from_wire_bytes_exact(&wire[..end]),
                Err(ExactEightChunkMembershipErrorV1::WireEncoding),
                "truncation at {end} was accepted"
            );
        }
        for trailing_len in [1, 2, 32, 1_560] {
            let mut trailing = wire.clone();
            trailing.resize(wire.len() + trailing_len, 0);
            assert_eq!(
                ExactEightChunkMembershipEvidenceV1::<R>::from_wire_bytes_exact(&trailing),
                Err(ExactEightChunkMembershipErrorV1::WireEncoding)
            );
        }
    }
    #[test]
    fn all_roles_reject_every_truncation_and_representative_trailing_bytes() {
        assert_exact_length_rejection::<PersistentSecretMembershipRoleV1>();
        assert_exact_length_rejection::<RkgEphemeralMembershipRoleV1>();
        assert_exact_length_rejection::<CpkErrorMembershipRoleV1>();
        assert_exact_length_rejection::<DirectRelationBoundOneMembershipRoleV1>();
        assert_exact_length_rejection::<DirectRelationBoundTwoMembershipRoleV1>();
    }
    #[test]
    fn role_substitution_is_rejected_even_after_length_and_prefix_reframing() {
        let persistent = fake_evidence::<PersistentSecretMembershipRoleV1>(b"cross-role", 0)
            .to_wire_bytes()
            .expect("persistent wire");
        let ephemeral = fake_evidence::<RkgEphemeralMembershipRoleV1>(b"cross-role", 8)
            .to_wire_bytes()
            .expect("ephemeral wire");
        let error = fake_evidence::<CpkErrorMembershipRoleV1>(b"cross-role", 16)
            .to_wire_bytes()
            .expect("error wire");
        assert!(
            ExactEightChunkMembershipEvidenceV1::<CpkErrorMembershipRoleV1>::from_wire_bytes_exact(
                &persistent
            )
            .is_err()
        );
        assert!(
            ExactEightChunkMembershipEvidenceV1::<PersistentSecretMembershipRoleV1>::from_wire_bytes_exact(
                &error
            )
            .is_err()
        );
        assert!(
            ExactEightChunkMembershipEvidenceV1::<RkgEphemeralMembershipRoleV1>::from_wire_bytes_exact(
                &persistent
            )
            .is_err()
        );
        assert!(
            ExactEightChunkMembershipEvidenceV1::<PersistentSecretMembershipRoleV1>::from_wire_bytes_exact(
                &ephemeral
            )
            .is_err()
        );
        let mut padded_persistent = persistent;
        padded_persistent.resize(CpkErrorMembershipRoleV1::WIRE_BYTES, 0);
        padded_persistent[..4].copy_from_slice(&CpkErrorMembershipRoleV1::MAGIC);
        assert!(
            ExactEightChunkMembershipEvidenceV1::<CpkErrorMembershipRoleV1>::from_wire_bytes_exact(
                &padded_persistent
            )
            .is_err()
        );
        let mut truncated_error = error;
        truncated_error.truncate(PersistentSecretMembershipRoleV1::WIRE_BYTES);
        truncated_error[..4].copy_from_slice(&PersistentSecretMembershipRoleV1::MAGIC);
        assert!(
            ExactEightChunkMembershipEvidenceV1::<PersistentSecretMembershipRoleV1>::from_wire_bytes_exact(
                &truncated_error
            )
            .is_err()
        );
    }
    fn changed_context<R: ExactEightChunkMembershipRoleV1>(
        context: ExactEightChunkMembershipContextV1<R>,
        axis: usize,
    ) -> ExactEightChunkMembershipContextV1<R> {
        let mut profile = context.profile_digest();
        let mut roster = context.roster_digest();
        let mut key_material = context.key_material_digest();
        let mut epoch = context.epoch();
        let mut cpk = context.cpk_transcript_digest();
        let mut party = context.party().to_bytes();
        let mut statement = context.share_statement_digest();
        match axis {
            0 => profile[0] ^= 1,
            1 => roster[0] ^= 1,
            2 => key_material[0] ^= 1,
            3 => epoch += 1,
            4 => cpk[0] ^= 1,
            5 => party[0] ^= 1,
            6 => statement[0] ^= 1,
            _ => unreachable!(),
        }
        ExactEightChunkMembershipContextV1::new(
            profile,
            roster,
            key_material,
            epoch,
            cpk,
            ZkAmsMkhePartyIdV1::new(party).expect("changed nonzero party"),
            statement,
        )
        .expect("changed nonzero context")
    }
    fn assert_all_context_axes_bound<R: ExactEightChunkMembershipRoleV1>() {
        let evidence = fake_evidence::<R>(b"axis-binding", 0);
        for axis in 0..7 {
            let changed = changed_context(evidence.context(), axis);
            assert_ne!(
                changed.context_digest(),
                evidence.context().context_digest()
            );
            let chunks = fake_chunks::<R>(changed, 8);
            let transcripts = core::array::from_fn(|index| {
                fake_verify::<R>(changed.context_digest(), index as u16, &chunks[index])
                    .expect("changed transcript")
            });
            let changed_evidence = ExactEightChunkMembershipEvidenceV1::<R>::assemble_for_test(
                changed,
                chunks,
                transcripts,
            )
            .expect("changed evidence");
            assert_ne!(
                changed_evidence.proof_set_digest(),
                evidence.proof_set_digest(),
                "axis {axis} was not proof-root bound"
            );
            assert_ne!(
                changed_evidence.verifier_transcript_digest(),
                evidence.verifier_transcript_digest(),
                "axis {axis} was not transcript-root bound"
            );
        }
    }
    #[test]
    fn every_context_axis_is_bound_for_all_roles_and_domains_are_disjoint() {
        assert_all_context_axes_bound::<PersistentSecretMembershipRoleV1>();
        assert_all_context_axes_bound::<RkgEphemeralMembershipRoleV1>();
        assert_all_context_axes_bound::<CpkErrorMembershipRoleV1>();
        assert_all_context_axes_bound::<DirectRelationBoundOneMembershipRoleV1>();
        assert_all_context_axes_bound::<DirectRelationBoundTwoMembershipRoleV1>();
        let persistent = context::<PersistentSecretMembershipRoleV1>(b"same-axes");
        let ephemeral = ExactEightChunkMembershipContextV1::<RkgEphemeralMembershipRoleV1>::new(
            persistent.profile_digest(),
            persistent.roster_digest(),
            persistent.key_material_digest(),
            persistent.epoch(),
            persistent.cpk_transcript_digest(),
            persistent.party(),
            persistent.share_statement_digest(),
        )
        .expect("same axes under RKG-ephemeral role");
        let error = ExactEightChunkMembershipContextV1::<CpkErrorMembershipRoleV1>::new(
            persistent.profile_digest(),
            persistent.roster_digest(),
            persistent.key_material_digest(),
            persistent.epoch(),
            persistent.cpk_transcript_digest(),
            persistent.party(),
            persistent.share_statement_digest(),
        )
        .expect("same axes under error role");
        assert_ne!(persistent.context_digest(), ephemeral.context_digest());
        assert_ne!(persistent.context_digest(), error.context_digest());
        assert_ne!(ephemeral.context_digest(), error.context_digest());
        assert_ne!(
            PersistentSecretMembershipRoleV1::COMMITMENT_SET_DIGEST_DOMAIN,
            RkgEphemeralMembershipRoleV1::COMMITMENT_SET_DIGEST_DOMAIN
        );
        assert_ne!(
            PersistentSecretMembershipRoleV1::COMMITMENT_SET_DIGEST_DOMAIN,
            CpkErrorMembershipRoleV1::COMMITMENT_SET_DIGEST_DOMAIN
        );
        assert_ne!(
            PersistentSecretMembershipRoleV1::PROOF_SET_DIGEST_DOMAIN,
            CpkErrorMembershipRoleV1::PROOF_SET_DIGEST_DOMAIN
        );
        assert_ne!(
            PersistentSecretMembershipRoleV1::VERIFIER_TRANSCRIPT_DIGEST_DOMAIN,
            CpkErrorMembershipRoleV1::VERIFIER_TRANSCRIPT_DIGEST_DOMAIN
        );
    }
    fn assert_mutations_rejected<R: ExactEightChunkMembershipRoleV1>() {
        let evidence = fake_evidence::<R>(b"mutation-adversary", 0);
        let wire = evidence.to_wire_bytes().expect("wire");
        for offset in [
            0,
            4,
            OFFSET_BOUND_V1,
            OFFSET_CHUNK_COUNT_V1,
            OFFSET_COEFFICIENT_COUNT_V1,
            OFFSET_GENERATOR_BASIS_DIGEST_V1,
            OFFSET_PROFILE_DIGEST_V1,
            OFFSET_ROSTER_DIGEST_V1,
            OFFSET_KEY_MATERIAL_DIGEST_V1,
            OFFSET_EPOCH_V1,
            OFFSET_CPK_TRANSCRIPT_DIGEST_V1,
            OFFSET_PARTY_V1,
            OFFSET_SHARE_STATEMENT_DIGEST_V1,
            OFFSET_COMMITMENT_SET_DIGEST_V1,
            OFFSET_PROOF_SET_DIGEST_V1,
        ] {
            let mut changed = wire.clone();
            changed[offset] ^= 1;
            assert!(
                ExactEightChunkMembershipEvidenceV1::<R>::from_wire_bytes_exact(&changed).is_err(),
                "outer mutation at {offset} was accepted"
            );
        }
        let first_chunk = EXACT_MEMBERSHIP_HEADER_BYTES_V1;
        for inner in [
            0,
            4,
            5,
            6,
            8,
            INNER_COMMITMENT_OFFSET_V1,
            45,
            INNER_PROOF_OFFSET_V1,
            R::CHUNK_WIRE_BYTES - 1,
        ] {
            let mut changed = wire.clone();
            changed[first_chunk + inner] ^= 1;
            assert!(
                ExactEightChunkMembershipEvidenceV1::<R>::from_wire_bytes_exact(&changed).is_err(),
                "inner mutation at {inner} was accepted"
            );
        }
        let mut transcript_root = wire;
        transcript_root[OFFSET_VERIFIER_TRANSCRIPT_DIGEST_V1] ^= 1;
        let decoded =
            ExactEightChunkMembershipEvidenceV1::<R>::from_wire_bytes_exact(&transcript_root)
                .expect("transcript root requires replay");
        assert_eq!(
            decoded.into_verified_with_for_test(fake_verify::<R>),
            Err(ExactEightChunkMembershipErrorV1::DigestMismatch)
        );
    }
    #[test]
    fn outer_inner_and_digest_mutations_fail_closed_for_all_roles() {
        assert_mutations_rejected::<PersistentSecretMembershipRoleV1>();
        assert_mutations_rejected::<RkgEphemeralMembershipRoleV1>();
        assert_mutations_rejected::<CpkErrorMembershipRoleV1>();
    }
    #[test]
    fn zero_context_axes_are_rejected_for_all_roles() {
        fn assert_role<R: ExactEightChunkMembershipRoleV1>() {
            let valid = context::<R>(b"zero-axis");
            for axis in 0..7 {
                let result = ExactEightChunkMembershipContextV1::<R>::new(
                    if axis == 0 {
                        [0; 32]
                    } else {
                        valid.profile_digest()
                    },
                    if axis == 1 {
                        [0; 32]
                    } else {
                        valid.roster_digest()
                    },
                    if axis == 2 {
                        [0; 32]
                    } else {
                        valid.key_material_digest()
                    },
                    if axis == 3 { 0 } else { valid.epoch() },
                    if axis == 4 {
                        [0; 32]
                    } else {
                        valid.cpk_transcript_digest()
                    },
                    if axis == 5 {
                        ZkAmsMkhePartyIdV1([0; 32])
                    } else {
                        valid.party()
                    },
                    if axis == 6 {
                        [0; 32]
                    } else {
                        valid.share_statement_digest()
                    },
                );
                assert_eq!(
                    result,
                    Err(ExactEightChunkMembershipErrorV1::Context),
                    "zero axis {axis} was accepted"
                );
            }
        }
        assert_role::<PersistentSecretMembershipRoleV1>();
        assert_role::<RkgEphemeralMembershipRoleV1>();
        assert_role::<CpkErrorMembershipRoleV1>();
    }
    #[test]
    fn rkg_ephemeral_role_has_distinct_frozen_wire_and_domains() {
        assert_eq!(RkgEphemeralMembershipRoleV1::MAGIC, *b"ZRME");
        assert_eq!(
            RkgEphemeralMembershipRoleV1::BOUND,
            ZkAmsT256MembershipBoundV1::One
        );
        assert_eq!(RkgEphemeralMembershipRoleV1::WIRE_BYTES, 12_291);
        assert_eq!(
            RkgEphemeralMembershipRoleV1::CONTEXT_DIGEST_DOMAIN,
            b"iroha.zk-ams.v1.mkhe.rkg-ephemeral-membership.context"
        );
        assert_ne!(
            RkgEphemeralMembershipRoleV1::CONTEXT_DIGEST_DOMAIN,
            PersistentSecretMembershipRoleV1::CONTEXT_DIGEST_DOMAIN
        );
        assert_ne!(
            RkgEphemeralMembershipRoleV1::PROOF_SET_DIGEST_DOMAIN,
            CpkErrorMembershipRoleV1::PROOF_SET_DIGEST_DOMAIN
        );
    }
    #[test]
    fn direct_relation_roles_have_frozen_disjoint_wires_and_domains() {
        assert_eq!(DirectRelationBoundOneMembershipRoleV1::MAGIC, *b"ZDB1");
        assert_eq!(DirectRelationBoundOneMembershipRoleV1::WIRE_BYTES, 12_291);
        assert_eq!(DirectRelationBoundTwoMembershipRoleV1::MAGIC, *b"ZDB2");
        assert_eq!(DirectRelationBoundTwoMembershipRoleV1::WIRE_BYTES, 12_819);
        assert_ne!(
            DirectRelationBoundOneMembershipRoleV1::CONTEXT_DIGEST_DOMAIN,
            DirectRelationBoundTwoMembershipRoleV1::CONTEXT_DIGEST_DOMAIN
        );
        assert_ne!(
            DirectRelationBoundOneMembershipRoleV1::COMMITMENT_SET_DIGEST_DOMAIN,
            DirectRelationBoundTwoMembershipRoleV1::COMMITMENT_SET_DIGEST_DOMAIN
        );
        assert_ne!(
            DirectRelationBoundOneMembershipRoleV1::PROOF_SET_DIGEST_DOMAIN,
            DirectRelationBoundTwoMembershipRoleV1::PROOF_SET_DIGEST_DOMAIN
        );
        assert_ne!(
            DirectRelationBoundOneMembershipRoleV1::VERIFIER_TRANSCRIPT_DIGEST_DOMAIN,
            DirectRelationBoundTwoMembershipRoleV1::VERIFIER_TRANSCRIPT_DIGEST_DOMAIN
        );
        assert_ne!(
            DirectRelationBoundOneMembershipRoleV1::CONTEXT_DIGEST_DOMAIN,
            PersistentSecretMembershipRoleV1::CONTEXT_DIGEST_DOMAIN
        );
        assert_ne!(
            DirectRelationBoundTwoMembershipRoleV1::CONTEXT_DIGEST_DOMAIN,
            CpkErrorMembershipRoleV1::CONTEXT_DIGEST_DOMAIN
        );
    }
    #[test]
    fn persistent_role_keeps_the_frozen_wire_and_digest_domains() {
        assert_eq!(PersistentSecretMembershipRoleV1::MAGIC, *b"ZPME");
        assert_eq!(
            PersistentSecretMembershipRoleV1::CONTEXT_DIGEST_DOMAIN,
            b"iroha.zk-ams.v1.mkhe.persistent-membership.context"
        );
        assert_eq!(
            PersistentSecretMembershipRoleV1::COMMITMENT_SET_DIGEST_DOMAIN,
            b"iroha.zk-ams.v1.mkhe.active-exact-small-binding.persistent-commitment-set"
        );
        assert_eq!(
            PersistentSecretMembershipRoleV1::PROOF_SET_DIGEST_DOMAIN,
            b"iroha.zk-ams.v1.mkhe.persistent-membership.proof-set"
        );
        assert_eq!(
            PersistentSecretMembershipRoleV1::VERIFIER_TRANSCRIPT_DIGEST_DOMAIN,
            b"iroha.zk-ams.v1.mkhe.persistent-membership.verifier-transcript-set"
        );
        let evidence = fake_evidence::<PersistentSecretMembershipRoleV1>(b"legacy-wire", 0);
        let wire = evidence.to_wire_bytes().expect("wire");
        assert_eq!(wire.len(), 12_291);
        assert_eq!(&wire[..4], b"ZPME");
        assert_eq!(wire[4], 1);
        assert_eq!(wire[5], ZkAmsT256MembershipBoundV1::One as u8);
        assert_eq!(wire[6], 8);
        assert_eq!(
            u32::from_be_bytes(wire[7..11].try_into().expect("count")),
            16_384
        );
        assert_eq!(&wire[11..43], &ZK_AMS_T256_BP_GENERATOR_BASIS_DIGEST_V1);
        assert_eq!(
            keccak256(PersistentSecretMembershipRoleV1::CONTEXT_DIGEST_DOMAIN),
            keccak256(b"iroha.zk-ams.v1.mkhe.persistent-membership.context")
        );
    }
}
