//! Sealed full-relation adapter; creates no binding, receipt, contribution, admission, or release authority.

use super::super::{
    MaskedRelaxedRandomSourceV1, ZkAmsMkheErrorV1,
    active::ZkAmsMkheGovernedActiveRosterV1,
    collective::{
        PersistentDirectOpeningLeaseV1, ReopenedCpkDirectOpeningLeaseV1,
        ZeroizingT256MembershipCoefficientsV1, ZkAmsMkheCollectivePublicKeyShareV1,
    },
    persistent_membership_evidence::{
        ZkAmsMkhePersistentMembershipContextV1, ZkAmsMkhePersistentMembershipErrorV1,
        ZkAmsMkhePersistentMembershipEvidenceV1,
    },
};
use super::{
    ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_BYTES_V1, ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_BYTES_V1,
    ZkAmsMkheCpkErrorMembershipContextV1, ZkAmsMkheCpkPartyBPointerV1, ZkAmsMkheCpkRelationErrorV1,
    ZkAmsMkheCpkRelationHeaderV1, ZkAmsMkheCpkRelationProofV1, ZkAmsMkheCpkShareStatementV1,
};
use crate::{generalized_bulletproof::ProofRandomSource, vega::VegaT256ScalarV1 as Scalar};

/// Public-data-only precursor sealed against incomplete CPK provers and verifiers.
/// It implements neither `Clone`, `Copy`, a codec, nor conversion to verified authority.
pub(in crate::vega::zk_ams::mkhe) struct StateOwnedCpkSecretMembershipPrecursorV1 {
    statement: ZkAmsMkheCpkShareStatementV1,
    public_share_digest: [u8; 32],
    secret_membership: ZkAmsMkhePersistentMembershipEvidenceV1,
}

pub(in crate::vega::zk_ams::mkhe) struct StateOwnedCpkSealedAbortSessionV1 {
    statement: ZkAmsMkheCpkShareStatementV1,
    public_share_digest: [u8; 32],
    secret_wire: Box<[u8; ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_BYTES_V1]>,
    error_wire: Box<[u8; ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_BYTES_V1]>,
    header: ZkAmsMkheCpkRelationHeaderV1,
}
pub(in crate::vega::zk_ams::mkhe) struct ReopenedStateOwnedCpkRelationPrecursorV1<'a> {
    opening: ReopenedCpkDirectOpeningLeaseV1<'a>,
    session: StateOwnedCpkSealedAbortSessionV1,
}
pub(in crate::vega::zk_ams::mkhe) struct StateOwnedCpkProvedPublicV1 {
    pub(in crate::vega::zk_ams::mkhe) statement: ZkAmsMkheCpkShareStatementV1,
    pub(in crate::vega::zk_ams::mkhe) secret_wire:
        Box<[u8; ZK_AMS_MKHE_CPK_SECRET_MEMBERSHIP_BYTES_V1]>,
    pub(in crate::vega::zk_ams::mkhe) error_wire:
        Box<[u8; ZK_AMS_MKHE_CPK_ERROR_MEMBERSHIP_BYTES_V1]>,
    pub(in crate::vega::zk_ams::mkhe) header: ZkAmsMkheCpkRelationHeaderV1,
    pub(in crate::vega::zk_ams::mkhe) proof: ZkAmsMkheCpkRelationProofV1,
}
fn into_exact_wire_box_v1<const N: usize>(
    bytes: Vec<u8>,
) -> Result<Box<[u8; N]>, ZkAmsMkheErrorV1> {
    if bytes.len() != N {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    if bytes.capacity() != N {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let allocation = bytes.as_ptr();
    let boxed = bytes.into_boxed_slice();
    if boxed.as_ptr() != allocation {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    boxed
        .try_into()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)
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

#[allow(clippy::too_many_arguments)]
pub(in crate::vega::zk_ams::mkhe) fn reopen_state_owned_cpk_relation_precursor_v1<'a, R>(
    precursor: StateOwnedCpkSecretMembershipPrecursorV1,
    statement: ZkAmsMkheCpkShareStatementV1,
    public_share_digest: [u8; 32],
    opening: PersistentDirectOpeningLeaseV1<'a>,
    error_coefficients: ZeroizingT256MembershipCoefficientsV1,
    random: &mut R,
) -> Result<ReopenedStateOwnedCpkRelationPrecursorV1<'a>, ZkAmsMkheErrorV1>
where
    R: ProofRandomSource + MaskedRelaxedRandomSourceV1,
{
    if precursor.statement != statement
        || precursor.public_share_digest != public_share_digest
        || public_share_digest == [0; 32]
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    opening
        .validate_secret_membership_v1(&precursor.secret_membership)
        .map_err(map_membership_error_v1)?;
    let opening = opening.into_reopened_v1(error_coefficients, random)?;
    let error = opening
        .prove_error_membership_v1(
            ZkAmsMkheCpkErrorMembershipContextV1::from_share_statement(statement)
                .map_err(map_relation_error_v1)?,
            random,
        )
        .map_err(map_relation_error_v1)?;
    let secret_wire = into_exact_wire_box_v1(
        precursor
            .secret_membership
            .into_wire_bytes()
            .map_err(map_membership_error_v1)?,
    )?;
    let error_wire =
        into_exact_wire_box_v1(error.into_wire_bytes().map_err(map_relation_error_v1)?)?;
    let header = ZkAmsMkheCpkRelationHeaderV1::new(statement, &*secret_wire, &*error_wire)
        .map_err(map_relation_error_v1)?;
    Ok(ReopenedStateOwnedCpkRelationPrecursorV1 {
        opening,
        session: StateOwnedCpkSealedAbortSessionV1 {
            statement,
            public_share_digest,
            secret_wire,
            error_wire,
            header,
        },
    })
}

impl ReopenedStateOwnedCpkRelationPrecursorV1<'_> {
    pub(in crate::vega::zk_ams::mkhe) fn into_proved_public_v1<R: MaskedRelaxedRandomSourceV1>(
        self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        random: &mut R,
    ) -> Result<StateOwnedCpkProvedPublicV1, ZkAmsMkheErrorV1> {
        self.opening
            .consume_sealed_cpk_abort_session_v1(self.session, roster, share, random)
            .map_err(map_relation_error_v1)
    }
}

impl StateOwnedCpkSealedAbortSessionV1 {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::vega::zk_ams::mkhe) fn prove_with_opening_v1<R: MaskedRelaxedRandomSourceV1>(
        self,
        roster: &ZkAmsMkheGovernedActiveRosterV1,
        share: &ZkAmsMkheCollectivePublicKeyShareV1,
        secret: &[i8],
        error: &[i8],
        secret_blindings: &[Scalar],
        error_blindings: &[Scalar],
        random: &mut R,
    ) -> Result<StateOwnedCpkProvedPublicV1, ZkAmsMkheCpkRelationErrorV1> {
        if self.public_share_digest != share.digest() {
            return Err(ZkAmsMkheCpkRelationErrorV1::GovernedContext);
        }
        let (header, proof) = super::state_owned_creator_v1::prove_state_owned_opening_v1(
            roster,
            self.statement.cpk_transcript_digest,
            share.public_a().residues(),
            self.statement,
            &*self.secret_wire,
            &*self.error_wire,
            secret,
            error,
            secret_blindings,
            error_blindings,
            random,
        )?;
        if header != self.header {
            return Err(ZkAmsMkheCpkRelationErrorV1::RelationHeader);
        }
        Ok(StateOwnedCpkProvedPublicV1 {
            statement: self.statement,
            secret_wire: self.secret_wire,
            error_wire: self.error_wire,
            header,
            proof,
        })
    }
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
        ZkAmsMkhePersistentMembershipErrorV1::Membership(error) => {
            map_relation_error_v1(super::map_membership_prover_error_v1(error))
        }
        _ => ZkAmsMkheErrorV1::InvalidKeyMaterial,
    }
}

#[cfg(test)]
#[path = "state_owned_secret_adapter_v1_tests.rs"]
mod tests;
