//! Sealed adapter from a collective-state lease to public CPK membership data.
//!
//! This module proves only bound-one membership of the exact state opening. It
//! neither proves the native CPK equation nor creates any verified binding,
//! receipt, contribution, admission, or release authority.

use super::super::{
    ZkAmsMkheErrorV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    collective::PersistentDirectOpeningLeaseV1,
    persistent_membership_evidence::{
        ZkAmsMkhePersistentMembershipContextV1, ZkAmsMkhePersistentMembershipErrorV1,
        ZkAmsMkhePersistentMembershipEvidenceV1,
    },
};
use super::{
    ZkAmsMkheCpkPartyBPointerV1, ZkAmsMkheCpkRelationErrorV1, ZkAmsMkheCpkShareStatementV1,
};
use crate::{
    generalized_bulletproof::{GeneralizedBulletproofErrorV1, ProofRandomSource},
    vega::bulletproof_t256::ZkAmsT256MembershipErrorV1,
};

/// Opaque, public-data-only precursor emitted from the exclusive state lease.
///
/// The fields remain sealed because no incomplete CPK prover or verifier is
/// admitted yet.  This type intentionally implements neither `Clone`, `Copy`,
/// a codec, nor a conversion to any verified capability.
pub(in crate::vega::zk_ams::mkhe) struct StateOwnedCpkSecretMembershipPrecursorV1 {
    statement: ZkAmsMkheCpkShareStatementV1,
    public_share_digest: [u8; 32],
    secret_membership: ZkAmsMkhePersistentMembershipEvidenceV1,
}

/// Derive the exact CPK statement/context and consume one unforgeable opening
/// lease into membership-only public material.
pub(in crate::vega::zk_ams::mkhe) fn prove_state_owned_cpk_secret_membership_v1<
    R: ProofRandomSource,
>(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    party_b_pointer: ZkAmsMkheCpkPartyBPointerV1,
    expected_party_b_payload_blake3: [u8; 32],
    lease: PersistentDirectOpeningLeaseV1<'_>,
    random: &mut R,
) -> Result<StateOwnedCpkSecretMembershipPrecursorV1, ZkAmsMkheErrorV1> {
    roster.validate()?;
    if expected_party_b_payload_blake3 == [0; 32]
        || party_b_pointer.payload_blake3() != expected_party_b_payload_blake3
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let statement = ZkAmsMkheCpkShareStatementV1::from_governed_roster(
        roster,
        lease.cpk_transcript_digest(),
        lease.party_index(),
        party_b_pointer,
    )
    .map_err(map_relation_error_v1)?;
    if statement.profile_digest != lease.profile_digest()
        || statement.security_certificate_digest != lease.security_certificate_digest()
        || statement.roster_digest != lease.roster_digest()
        || statement.key_material_digest != lease.key_material_digest()
        || statement.epoch != lease.epoch()
        || statement.cpk_transcript_digest != lease.cpk_transcript_digest()
        || statement.party_index() != lease.party_index()
        || statement.party() != lease.party()
        || statement.party_b_pointer() != party_b_pointer
        || lease.public_share_digest() == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let context = ZkAmsMkhePersistentMembershipContextV1::from_relation_axes(
        statement.profile_digest,
        statement.roster_digest,
        statement.key_material_digest,
        statement.epoch,
        statement.cpk_transcript_digest,
        statement.party,
        statement
            .statement_digest()
            .map_err(map_relation_error_v1)?,
    )
    .map_err(map_membership_error_v1)?;
    let public_share_digest = lease.public_share_digest();
    let secret_membership = lease
        .prove(context, random)
        .map_err(map_membership_error_v1)?;
    if secret_membership.context() != context || secret_membership.commitments().len() != 8 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(StateOwnedCpkSecretMembershipPrecursorV1 {
        statement,
        public_share_digest,
        secret_membership,
    })
}

fn map_relation_error_v1(error: ZkAmsMkheCpkRelationErrorV1) -> ZkAmsMkheErrorV1 {
    match error {
        ZkAmsMkheCpkRelationErrorV1::ResourceCeiling => ZkAmsMkheErrorV1::ResourceCeilingExceeded,
        ZkAmsMkheCpkRelationErrorV1::RandomUnavailable => ZkAmsMkheErrorV1::RandomUnavailable,
        _ => ZkAmsMkheErrorV1::InvalidKeyMaterial,
    }
}

fn map_membership_error_v1(error: ZkAmsMkhePersistentMembershipErrorV1) -> ZkAmsMkheErrorV1 {
    match error {
        ZkAmsMkhePersistentMembershipErrorV1::Membership(ZkAmsT256MembershipErrorV1::Backend(
            GeneralizedBulletproofErrorV1::RandomnessUnavailable
            | GeneralizedBulletproofErrorV1::ProverRandomnessExhausted,
        )) => ZkAmsMkheErrorV1::RandomUnavailable,
        ZkAmsMkhePersistentMembershipErrorV1::Membership(ZkAmsT256MembershipErrorV1::Backend(
            GeneralizedBulletproofErrorV1::ResourceOverflow,
        )) => ZkAmsMkheErrorV1::ResourceCeilingExceeded,
        _ => ZkAmsMkheErrorV1::InvalidKeyMaterial,
    }
}

#[cfg(test)]
#[path = "state_owned_secret_adapter_v1_tests.rs"]
mod tests;
