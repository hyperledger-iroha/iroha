//! Exact public membership evidence for direct RKG ephemerals.
//!
//! One party-local `u_i` is committed in eight canonical T256 chunks under a
//! role which is disjoint from persistent CPK secrets and CPK public errors.
//! The public 12,291-byte evidence is bound to the complete direct ceremony
//! context and the same party's already verified secret-lineage identity.
//! This module creates no verified binding or retained secret owner. The
//! party-local collective state owns the opening, and only a future complete
//! direct-relation verifier may mint binding authority from this evidence.

use super::{
    MKHE_VERSION_V1, ZkAmsMkheErrorV1, ZkAmsMkhePartyIdV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::VerifiedPersistentWitnessBindingSetV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
    exact_eight_chunk_membership::{
        ExactEightChunkMembershipContextV1, ExactEightChunkMembershipErrorV1,
        ExactEightChunkMembershipEvidenceV1, RkgEphemeralMembershipRoleV1,
        ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1, ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1,
        ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1,
    },
    manifest::{ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1},
};
#[cfg(test)]
use crate::vega::bulletproof_t256::ZkAmsT256MembershipProofV1;
use crate::{
    generalized_bulletproof::ProofRandomSource,
    vega::{VegaT256PointV1 as Point, VegaT256ScalarV1 as Scalar, sponge::Keccak256},
};
use thiserror::Error;
const RKG_EPHEMERAL_STATEMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-rkg-ephemeral-membership.statement";
const _: () = {
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1 == 8);
    assert!(ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 == 131_072);
    assert!(ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1 == 12_291);
};
/// Stable wrapper failures before the active binding capability is minted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum ZkAmsMkheDirectRkgEphemeralMembershipErrorV1 {
    /// The governed/direct context is missing, stale, or inconsistent.
    #[error("invalid direct RKG-ephemeral membership context")]
    Context,
    /// The exact role-separated membership engine rejected the evidence.
    #[error(transparent)]
    ExactMembership(#[from] ExactEightChunkMembershipErrorV1),
}
/// Canonical wrapper context for one party-local RKG `u_i` source.
///
/// The exact membership frame carries the common profile/roster/key/epoch,
/// CPK-transcript, and party axes directly. Its statement digest binds the
/// remaining direct-context, ordinal, digit, record, and secret-lineage axes
/// in the fixed order implemented by [`rkg_ephemeral_statement_digest_v1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkheDirectRkgEphemeralMembershipContextV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    cpk_transcript_digest: [u8; 32],
    direct_context_digest: [u8; 32],
    party_index: u8,
    party: ZkAmsMkhePartyIdV1,
    evaluated_key_ordinal: u8,
    digit_index: u8,
    record_index: u32,
    secret_lineage_identity_digest: [u8; 32],
    statement_digest: [u8; 32],
}
impl ZkAmsMkheDirectRkgEphemeralMembershipContextV1 {
    /// Derive every axis from the governed roster, opaque secret-binding set,
    /// and validated direct relinearization context.
    pub(super) fn from_verified_binding_set(
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        bindings: &VerifiedPersistentWitnessBindingSetV1,
        direct_context: &ZkAmsMkheDirectCeremonyContextV1,
        party_index: usize,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        roster.validate()?;
        direct_context.validate_rkg_ephemeral_membership_axes(roster, bindings)?;
        let participant = roster
            .participants()
            .get(party_index)
            .ok_or(ZkAmsMkheErrorV1::InvalidPartySet)?;
        let record_index = canonical_rkg_ephemeral_record_index_v1(
            direct_context.evaluated_key_ordinal(),
            direct_context.digit_index(),
            party_index,
        )
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut context = Self {
            profile_digest: direct_context.profile_digest(),
            roster_digest: direct_context.roster_digest(),
            key_material_digest: direct_context.key_material_digest(),
            epoch: direct_context.epoch(),
            cpk_transcript_digest: direct_context.transcript_digest(),
            direct_context_digest: direct_context.digest(),
            party_index: u8::try_from(party_index)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
            party: participant.party(),
            evaluated_key_ordinal: direct_context.evaluated_key_ordinal(),
            digit_index: direct_context.digit_index(),
            record_index,
            secret_lineage_identity_digest: bindings.identity_digests()[party_index],
            statement_digest: [0; 32],
        };
        context.statement_digest = rkg_ephemeral_statement_digest_v1(context);
        context
            .validate()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        Ok(context)
    }
    fn validate(self) -> Result<(), ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        if self.profile_digest == [0; 32]
            || self.roster_digest == [0; 32]
            || self.key_material_digest == [0; 32]
            || self.epoch == 0
            || self.cpk_transcript_digest == [0; 32]
            || self.direct_context_digest == [0; 32]
            || self.party.to_bytes() == [0; 32]
            || self.evaluated_key_ordinal != 0
            || Some(self.record_index)
                != canonical_rkg_ephemeral_record_index_v1(
                    self.evaluated_key_ordinal,
                    self.digit_index,
                    usize::from(self.party_index),
                )
            || self.secret_lineage_identity_digest == [0; 32]
            || self.statement_digest == [0; 32]
            || self.statement_digest != rkg_ephemeral_statement_digest_v1(self)
        {
            return Err(ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context);
        }
        Ok(())
    }
    fn to_exact(
        self,
    ) -> Result<
        ExactEightChunkMembershipContextV1<RkgEphemeralMembershipRoleV1>,
        ZkAmsMkheDirectRkgEphemeralMembershipErrorV1,
    > {
        self.validate()?;
        ExactEightChunkMembershipContextV1::new(
            self.profile_digest,
            self.roster_digest,
            self.key_material_digest,
            self.epoch,
            self.cpk_transcript_digest,
            self.party,
            self.statement_digest,
        )
        .map_err(Into::into)
    }
    pub(super) const fn statement_digest(self) -> [u8; 32] {
        self.statement_digest
    }
    pub(super) const fn direct_context_digest(self) -> [u8; 32] {
        self.direct_context_digest
    }
    pub(super) const fn party_index(self) -> usize {
        self.party_index as usize
    }
    pub(super) const fn record_index(self) -> u32 {
        self.record_index
    }
    pub(super) const fn digit_index(self) -> u8 {
        self.digit_index
    }
}
fn canonical_rkg_ephemeral_record_index_v1(
    evaluated_key_ordinal: u8,
    digit_index: u8,
    party_index: usize,
) -> Option<u32> {
    let profile = release_profile_v1();
    if evaluated_key_ordinal != 0
        || usize::from(digit_index) >= profile.gadget_digits
        || party_index >= ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
    {
        return None;
    }
    usize::from(evaluated_key_ordinal)
        .checked_mul(profile.gadget_digits)
        .and_then(|base| base.checked_add(usize::from(digit_index)))
        .and_then(|coordinate| coordinate.checked_mul(ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1))
        .and_then(|base| base.checked_add(party_index))
        .and_then(|zero_based| zero_based.checked_add(1))
        .and_then(|record_index| u32::try_from(record_index).ok())
}
fn rkg_ephemeral_statement_digest_v1(
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(RKG_EPHEMERAL_STATEMENT_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1]);
    hash.update(&context.profile_digest);
    hash.update(&context.roster_digest);
    hash.update(&context.key_material_digest);
    hash.update(&context.epoch.to_be_bytes());
    hash.update(&context.cpk_transcript_digest);
    hash.update(&context.direct_context_digest);
    hash.update(&[context.party_index]);
    hash.update(&context.party.to_bytes());
    hash.update(&[context.evaluated_key_ordinal]);
    hash.update(&[context.digit_index]);
    hash.update(&context.record_index.to_be_bytes());
    hash.update(&context.secret_lineage_identity_digest);
    hash.finalize()
}
/// Canonical public evidence for one direct RKG-ephemeral opening.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1 {
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    inner: ExactEightChunkMembershipEvidenceV1<RkgEphemeralMembershipRoleV1>,
}
impl ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1 {
    /// Prove and locally verify all eight bound-one chunks.
    pub(super) fn prove<Random: ProofRandomSource>(
        context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
        coefficients: &[i8],
        blindings: &[Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
        random: &mut Random,
    ) -> Result<Self, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        context.validate()?;
        let inner = ExactEightChunkMembershipEvidenceV1::prove(
            context.to_exact()?,
            coefficients,
            blindings,
            random,
        )?;
        Ok(Self { context, inner })
    }
    /// Decode exactly 12,291 bytes at a verifier-derived wrapper context.
    ///
    /// There is no context-free decoder: the omitted direct axes are carried
    /// by the expected statement digest and must be known before parsing.
    pub(super) fn from_wire_bytes_exact(
        expected_context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
        bytes: &[u8],
    ) -> Result<Self, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        expected_context.validate()?;
        if bytes.len() != ZK_AMS_MKHE_RKG_EPHEMERAL_MEMBERSHIP_WIRE_BYTES_V1 {
            return Err(ExactEightChunkMembershipErrorV1::WireEncoding.into());
        }
        let inner = ExactEightChunkMembershipEvidenceV1::from_wire_bytes_exact(bytes)?;
        if inner.context() != expected_context.to_exact()? {
            return Err(ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context);
        }
        Ok(Self {
            context: expected_context,
            inner,
        })
    }
    /// Encode the unique role-separated exact-membership wire.
    pub(super) fn to_wire_bytes(
        &self,
    ) -> Result<Vec<u8>, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        self.context.validate()?;
        if self.inner.context() != self.context.to_exact()? {
            return Err(ZkAmsMkheDirectRkgEphemeralMembershipErrorV1::Context);
        }
        self.inner.to_wire_bytes().map_err(Into::into)
    }
    pub(super) const fn context(&self) -> ZkAmsMkheDirectRkgEphemeralMembershipContextV1 {
        self.context
    }
    pub(super) fn commitments(&self) -> [Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] {
        self.inner.commitments()
    }
    #[cfg(test)]
    pub(super) fn assemble_for_test(
        context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
        chunks: [ZkAmsT256MembershipProofV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
        transcript_digests: [[u8; 32]; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    ) -> Result<Self, ZkAmsMkheDirectRkgEphemeralMembershipErrorV1> {
        let inner = ExactEightChunkMembershipEvidenceV1::assemble_for_test(
            context.to_exact()?,
            chunks,
            transcript_digests,
        )?;
        Ok(Self { context, inner })
    }
}
#[cfg(test)]
#[path = "direct_rkg_ephemeral_membership_tests.rs"]
pub(super) mod tests;
