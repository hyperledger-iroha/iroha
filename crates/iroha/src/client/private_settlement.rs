//! Fail-closed Rust client surface for atomic private settlement Torii routes.
//!
//! Account-authenticated operations use the client's transaction authority.
//! Committee and auditor operations require an explicitly supplied role key so
//! consensus, auditor, and ordinary operator identities never share implicit
//! client state.

use super::*;
use iroha_data_model::{
    isi::private_settlement::FinalizeAtomicPrivateSettlementV1,
    nexus::{
        ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, AtomicPrivateSettlementV1,
        PRIVATE_SETTLEMENT_BLS_BYTES_V1, PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1,
        PrivateSettlementAvailabilityShareV1, PrivateSettlementCommitteeAuthorityV1,
        PrivateSettlementDeltaV1, PrivateSettlementPhaseBodyV1,
        PrivateSettlementPhaseCertificateV1, PrivateSettlementPhaseV1,
        PrivateSettlementPhaseVoteV1, PrivateSettlementPrepareBarrierV1,
        PrivateSettlementProvisionalLegMaterialV1, PrivateSettlementSidecarAvailabilityBodyV1,
        PrivateSettlementSidecarAvailabilityV1, private_settlement_proof_digest_v1,
    },
    transaction::Executable,
};
use iroha_torii_shared::private_settlement_api::{
    PrivateSettlementAuditApprovalRequestV1, PrivateSettlementAuditApprovalResponseV1,
    PrivateSettlementAuditorCapsuleResponseV1, PrivateSettlementAvailabilityShareRequestV1,
    PrivateSettlementAvailabilityShareResponseV1, PrivateSettlementBundleReceiptResponseV1,
    PrivateSettlementBundleStatusResponseV1, PrivateSettlementBundleSubmitRequestV1,
    PrivateSettlementBundleSubmitResponseV1, PrivateSettlementCommitVoteRequestV1,
    PrivateSettlementCommitteeProofResponseV1, PrivateSettlementLegStatusResponseV1,
    PrivateSettlementLegUploadRequestV1, PrivateSettlementLegUploadResponseV1,
    PrivateSettlementLifecycleDtoV1, PrivateSettlementPhaseCertificateRequestV1,
    PrivateSettlementPhaseCertificateResponseV1, PrivateSettlementPhaseVoteResponseV1,
    PrivateSettlementPrepareVoteRequestV1,
};
use std::collections::BTreeMap;

const PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1: usize = 32 * 1024 * 1024;

fn private_settlement_resource_path_v1(kind: &str, identifier: &Hash, suffix: &str) -> String {
    format!("v1/nexus/private-settlements/{kind}/{identifier}{suffix}")
}

fn private_settlement_leg_ordinal_for_payload_v1(
    manifest: &AtomicPrivateSettlementV1,
    payload_digest: Hash,
) -> Result<u8> {
    let mut matches = manifest
        .legs
        .iter()
        .filter(|leg| leg.payload_digest == payload_digest);
    let ordinal = matches
        .next()
        .map(|leg| leg.ordinal)
        .ok_or_else(|| eyre!("private-settlement payload is not in the manifest"))?;
    if matches.next().is_some() {
        return Err(eyre!("private-settlement payload digest is duplicated"));
    }
    Ok(ordinal)
}

fn exact_private_settlement_carrier_v1(
    transaction: &SignedTransaction,
) -> Result<&FinalizeAtomicPrivateSettlementV1> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(eyre!(
            "private-settlement bundle submission requires one direct carrier"
        ));
    };
    if instructions.len() != 1 {
        return Err(eyre!(
            "private-settlement bundle submission requires one direct carrier"
        ));
    }
    instructions[0]
        .as_any()
        .downcast_ref::<FinalizeAtomicPrivateSettlementV1>()
        .ok_or_else(|| eyre!("private-settlement bundle submission requires one direct carrier"))
}

fn validate_availability_certificate_v1(
    certificate: &PrivateSettlementSidecarAvailabilityV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
) -> Result<()> {
    certificate
        .validate_shape()
        .map_err(|_| eyre!("private-settlement availability certificate is invalid"))?;
    authority
        .validate()
        .map_err(|_| eyre!("private-settlement availability certificate is invalid"))?;
    let authority_digest = authority
        .digest()
        .map_err(|_| eyre!("private-settlement availability certificate is invalid"))?;
    if certificate.body.route != authority.route
        || certificate.body.authority_digest != authority_digest
    {
        return Err(eyre!(
            "private-settlement availability certificate is invalid"
        ));
    }
    let mut signer_keys = Vec::with_capacity(3);
    let mut signer_pops = Vec::with_capacity(3);
    for (index, (validator, pop)) in authority
        .validators
        .iter()
        .zip(&authority.validator_pops)
        .enumerate()
    {
        if validator.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
            || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
        {
            return Err(eyre!(
                "private-settlement availability certificate is invalid"
            ));
        }
        if certificate.signers_bitmap & (1_u8 << index) != 0 {
            signer_keys.push(validator.public_key());
            signer_pops.push(pop.as_slice());
        }
    }
    let preimage = certificate
        .signature_preimage()
        .map_err(|_| eyre!("private-settlement availability certificate is invalid"))?;
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &certificate.aggregate_signature,
        &signer_keys,
        &signer_pops,
    )
    .map_err(|_| eyre!("private-settlement availability certificate is invalid"))
}

fn validate_availability_share_v1(
    share: &PrivateSettlementAvailabilityShareV1,
    body: &PrivateSettlementSidecarAvailabilityBodyV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
) -> Result<usize> {
    share
        .validate_shape()
        .map_err(|_| eyre!("private-settlement availability share is invalid"))?;
    authority
        .validate()
        .map_err(|_| eyre!("private-settlement availability share is invalid"))?;
    let authority_digest = authority
        .digest()
        .map_err(|_| eyre!("private-settlement availability share is invalid"))?;
    let index = authority
        .validators
        .iter()
        .position(|validator| validator == &share.signer)
        .ok_or_else(|| eyre!("private-settlement availability share is invalid"))?;
    let pop = authority
        .validator_pops
        .get(index)
        .ok_or_else(|| eyre!("private-settlement availability share is invalid"))?;
    if &share.body != body
        || body.route != authority.route
        || body.authority_digest != authority_digest
        || share.signer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
        || iroha_crypto::bls_normal_pop_verify(share.signer.public_key(), pop).is_err()
    {
        return Err(eyre!("private-settlement availability share is invalid"));
    }
    let signature = Signature::try_from_bytes(&share.signature)
        .map_err(|_| eyre!("private-settlement availability share is invalid"))?;
    signature
        .verify(
            share.signer.public_key(),
            &body
                .signature_preimage()
                .map_err(|_| eyre!("private-settlement availability share is invalid"))?,
        )
        .map_err(|_| eyre!("private-settlement availability share is invalid"))?;
    Ok(index)
}

fn aggregate_availability_shares_v1(
    body: PrivateSettlementSidecarAvailabilityBodyV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
    shares: &[PrivateSettlementAvailabilityShareV1],
) -> Result<PrivateSettlementSidecarAvailabilityV1> {
    if shares.len() != usize::from(PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1) {
        return Err(eyre!("private-settlement availability quorum is invalid"));
    }
    let mut indexed = BTreeMap::new();
    for share in shares {
        let index = validate_availability_share_v1(share, &body, authority)?;
        if indexed.insert(index, share.signature.clone()).is_some() {
            return Err(eyre!("private-settlement availability share is invalid"));
        }
    }
    let mut signers_bitmap = 0_u8;
    let signatures = indexed
        .into_iter()
        .map(|(index, signature)| {
            signers_bitmap |= 1_u8 << index;
            signature
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let certificate = PrivateSettlementSidecarAvailabilityV1 {
        body,
        signers_bitmap,
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .map_err(|_| eyre!("private-settlement availability share is invalid"))?,
    };
    validate_availability_certificate_v1(&certificate, authority)?;
    Ok(certificate)
}

fn canonical_availability_share_quorum_v1(
    shares: &[PrivateSettlementAvailabilityShareV1],
) -> Result<&[PrivateSettlementAvailabilityShareV1]> {
    shares
        .get(..usize::from(PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1))
        .ok_or_else(|| eyre!("private-settlement availability quorum is unavailable"))
}

fn private_settlement_reserved_prepared_digest_v1() -> Hash {
    Hash::prehashed([0; Hash::LENGTH])
}

fn phase_signature_preimage_v1(body: &PrivateSettlementPhaseBodyV1) -> Result<Vec<u8>> {
    body.signature_preimage()
        .map_err(|_| eyre!("private-settlement phase body encoding failed"))
}

fn prepared_bundle_digest_v1(
    manifest: &AtomicPrivateSettlementV1,
    authority_catalog: &[PrivateSettlementCommitteeAuthorityV1],
    deltas: &[PrivateSettlementDeltaV1],
    prepare_certificates: &[PrivateSettlementPhaseCertificateV1],
) -> Result<Hash> {
    if authority_catalog.len() != manifest.legs.len()
        || deltas.len() != manifest.legs.len()
        || prepare_certificates.len() != manifest.legs.len()
    {
        return Err(eyre!("private-settlement Prepare barrier is incomplete"));
    }
    PrivateSettlementPrepareBarrierV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        manifest: manifest.clone(),
        authority_catalog: authority_catalog.to_vec(),
        deltas: deltas.to_vec(),
        prepare_certificates: prepare_certificates.to_vec(),
        prepared_bundle_digest: private_settlement_reserved_prepared_digest_v1(),
    }
    .computed_prepared_bundle_digest()
    .map_err(|_| eyre!("private-settlement Prepare barrier encoding failed"))
}

fn expected_phase_body_v1(
    manifest: &AtomicPrivateSettlementV1,
    leg_ordinal: u8,
    authority: &PrivateSettlementCommitteeAuthorityV1,
    phase: PrivateSettlementPhaseV1,
    prepared_bundle_digest: Hash,
) -> Result<PrivateSettlementPhaseBodyV1> {
    manifest
        .validate()
        .map_err(|_| eyre!("private-settlement phase manifest is invalid"))?;
    authority
        .validate()
        .map_err(|_| eyre!("private-settlement phase authority is invalid"))?;
    let leg = manifest
        .legs
        .get(usize::from(leg_ordinal))
        .ok_or_else(|| eyre!("private-settlement phase leg is invalid"))?;
    let reserved = private_settlement_reserved_prepared_digest_v1();
    if authority.route != leg.route
        || match phase {
            PrivateSettlementPhaseV1::Prepare => prepared_bundle_digest != reserved,
            PrivateSettlementPhaseV1::Commit => prepared_bundle_digest == reserved,
        }
    {
        return Err(eyre!("private-settlement phase binding is invalid"));
    }
    Ok(PrivateSettlementPhaseBodyV1 {
        network_id: manifest.network_id,
        bundle_id: manifest.bundle_id,
        manifest_digest: manifest
            .manifest_digest()
            .map_err(|_| eyre!("private-settlement phase manifest encoding failed"))?,
        leg_ordinal,
        route: leg.route,
        delta_digest: leg.delta_digest,
        authority_digest: authority
            .digest()
            .map_err(|_| eyre!("private-settlement phase authority encoding failed"))?,
        prepared_bundle_digest,
        phase,
        authority_context_height: manifest.authority_context_height,
        expiry_height: manifest.expiry_height,
    })
}

fn validate_phase_vote_v1(
    vote: &PrivateSettlementPhaseVoteV1,
    expected_body: &PrivateSettlementPhaseBodyV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
) -> Result<usize> {
    vote.validate_shape()
        .map_err(|_| eyre!("private-settlement phase vote is invalid"))?;
    authority
        .validate()
        .map_err(|_| eyre!("private-settlement phase vote is invalid"))?;
    let index = authority
        .validators
        .iter()
        .position(|validator| validator == &vote.signer)
        .ok_or_else(|| eyre!("private-settlement phase vote is invalid"))?;
    let pop = authority
        .validator_pops
        .get(index)
        .ok_or_else(|| eyre!("private-settlement phase vote is invalid"))?;
    if &vote.body != expected_body
        || vote.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1
        || vote.signature.len() != PRIVATE_SETTLEMENT_BLS_BYTES_V1
        || vote.signer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
        || iroha_crypto::bls_normal_pop_verify(vote.signer.public_key(), pop).is_err()
    {
        return Err(eyre!("private-settlement phase vote is invalid"));
    }
    let signature = Signature::try_from_bytes(&vote.signature)
        .map_err(|_| eyre!("private-settlement phase vote is invalid"))?;
    signature
        .verify(
            vote.signer.public_key(),
            &phase_signature_preimage_v1(&vote.body)?,
        )
        .map_err(|_| eyre!("private-settlement phase vote is invalid"))?;
    Ok(index)
}

fn validate_phase_certificate_v1(
    certificate: &PrivateSettlementPhaseCertificateV1,
    expected_body: &PrivateSettlementPhaseBodyV1,
    authority_catalog_index: u8,
    authority: &PrivateSettlementCommitteeAuthorityV1,
) -> Result<()> {
    certificate
        .validate_shape()
        .map_err(|_| eyre!("private-settlement phase certificate is invalid"))?;
    if &certificate.body != expected_body
        || certificate.authority_catalog_index != authority_catalog_index
        || expected_body.authority_digest
            != authority
                .digest()
                .map_err(|_| eyre!("private-settlement phase certificate is invalid"))?
    {
        return Err(eyre!("private-settlement phase certificate is invalid"));
    }
    let mut signer_keys = Vec::with_capacity(usize::from(PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1));
    let mut signer_pops = Vec::with_capacity(usize::from(PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1));
    for (index, (validator, pop)) in authority
        .validators
        .iter()
        .zip(&authority.validator_pops)
        .enumerate()
    {
        if validator.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
            || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
        {
            return Err(eyre!("private-settlement phase certificate is invalid"));
        }
        if certificate.signers_bitmap & (1_u8 << index) != 0 {
            signer_keys.push(validator.public_key());
            signer_pops.push(pop.as_slice());
        }
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &phase_signature_preimage_v1(expected_body)?,
        &certificate.aggregate_signature,
        &signer_keys,
        &signer_pops,
    )
    .map_err(|_| eyre!("private-settlement phase certificate is invalid"))
}

fn aggregate_phase_votes_v1(
    body: PrivateSettlementPhaseBodyV1,
    authority_catalog_index: u8,
    authority: &PrivateSettlementCommitteeAuthorityV1,
    votes: &[PrivateSettlementPhaseVoteV1],
) -> Result<PrivateSettlementPhaseCertificateV1> {
    if votes.len() != usize::from(PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1) {
        return Err(eyre!("private-settlement phase quorum is invalid"));
    }
    let mut indexed = BTreeMap::new();
    for vote in votes {
        let index = validate_phase_vote_v1(vote, &body, authority)?;
        if indexed.insert(index, vote.signature.clone()).is_some() {
            return Err(eyre!("private-settlement phase vote is duplicated"));
        }
    }
    let mut signers_bitmap = 0_u8;
    let signatures = indexed
        .into_iter()
        .map(|(index, signature)| {
            signers_bitmap |= 1_u8 << index;
            signature
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let certificate = PrivateSettlementPhaseCertificateV1 {
        body,
        authority_catalog_index,
        signers_bitmap,
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .map_err(|_| eyre!("private-settlement phase aggregation failed"))?,
    };
    validate_phase_certificate_v1(&certificate, &body, authority_catalog_index, authority)?;
    Ok(certificate)
}

fn canonical_phase_vote_quorum_v1(
    votes: &[PrivateSettlementPhaseVoteV1],
) -> Result<&[PrivateSettlementPhaseVoteV1]> {
    votes
        .get(..usize::from(PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1))
        .ok_or_else(|| eyre!("private-settlement phase quorum is unavailable"))
}

fn validate_prepare_barrier_v1(barrier: &PrivateSettlementPrepareBarrierV1) -> Result<()> {
    barrier
        .validate_shape()
        .map_err(|_| eyre!("private-settlement Prepare barrier is invalid"))?;
    for (index, ((authority, certificate), _delta)) in barrier
        .authority_catalog
        .iter()
        .zip(&barrier.prepare_certificates)
        .zip(&barrier.deltas)
        .enumerate()
    {
        let ordinal = u8::try_from(index)
            .map_err(|_| eyre!("private-settlement Prepare barrier is invalid"))?;
        let expected = expected_phase_body_v1(
            &barrier.manifest,
            ordinal,
            authority,
            PrivateSettlementPhaseV1::Prepare,
            private_settlement_reserved_prepared_digest_v1(),
        )?;
        validate_phase_certificate_v1(certificate, &expected, ordinal, authority)?;
        if barrier.deltas[index]
            .digest()
            .map_err(|_| eyre!("private-settlement Prepare barrier is invalid"))?
            != expected.delta_digest
        {
            return Err(eyre!("private-settlement Prepare barrier is invalid"));
        }
    }
    if barrier.prepared_bundle_digest
        != prepared_bundle_digest_v1(
            &barrier.manifest,
            &barrier.authority_catalog,
            &barrier.deltas,
            &barrier.prepare_certificates,
        )?
    {
        return Err(eyre!("private-settlement Prepare barrier is invalid"));
    }
    Ok(())
}

fn validate_leg_status_response_v1(
    requested: Hash,
    response: &PrivateSettlementLegStatusResponseV1,
) -> Result<()> {
    if response.payload_digest != requested
        || response.route.dataspace_id == iroha_data_model::nexus::DataSpaceId::UNIVERSAL
        || response.stored_at_height == 0
        || response.lifecycle_height < response.stored_at_height
        || response.expiry_height <= response.stored_at_height
    {
        return Err(eyre!(
            "private-settlement leg status response is invalid or substituted"
        ));
    }
    Ok(())
}

fn validate_restricted_proof_response_v1(
    requested: Hash,
    response: &PrivateSettlementCommitteeProofResponseV1,
) -> Result<()> {
    response
        .manifest
        .validate()
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    response
        .audit_policy
        .validate()
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    response
        .committee_authority
        .validate()
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    response
        .statement
        .validate()
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    response
        .delta
        .validate_against(&response.statement)
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    validate_availability_certificate_v1(&response.availability, &response.committee_authority)
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    let statement_digest = response
        .statement
        .digest()
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    let proof_digest = private_settlement_proof_digest_v1(&response.proof);
    let delta_digest = response
        .delta
        .digest()
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    let authority_digest = response
        .committee_authority
        .digest()
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    let availability_digest = response
        .availability
        .digest()
        .map_err(|_| eyre!("private-settlement committee response is invalid or substituted"))?;
    let ordinal = usize::from(response.statement.leg_ordinal);
    let leg =
        response.manifest.legs.get(ordinal).ok_or_else(|| {
            eyre!("private-settlement committee response is invalid or substituted")
        })?;
    let availability = &response.availability.body;
    if availability.payload_digest != requested
        || leg.payload_digest != requested
        || leg.delta_digest != delta_digest
        || leg.availability_certificate_digest != availability_digest
        || leg.route != response.statement.route
        || leg.pool_id != response.statement.pool_id
        || leg.asset_binding_commitment != response.statement.asset_binding_commitment
        || leg.audit_policy_digest != response.audit_policy.policy_digest
        || response.audit_policy.body.dataspace_id != response.statement.route.dataspace_id
        || response.committee_authority.route != response.statement.route
        || availability.network_id != response.manifest.network_id
        || availability.bundle_id != response.manifest.bundle_id
        || availability.leg_ordinal != response.statement.leg_ordinal
        || availability.route != response.statement.route
        || availability.authority_digest != authority_digest
        || availability.authority_context_height != response.manifest.authority_context_height
        || availability.retention_until_height < response.manifest.expiry_height
        || response.manifest.bundle_id != response.statement.bundle_id
        || response.manifest.bundle_id != response.delta.bundle_id
        || response.statement.leg_ordinal != response.delta.leg_ordinal
        || response.statement.route != response.delta.route
        || response.statement.pool_id != response.delta.pool_id
        || response.statement.asset_binding_commitment != response.delta.asset_binding_commitment
        || response.statement.audit_policy_digest != response.delta.audit_policy_digest
        || response.statement.audit_key_epoch != response.delta.audit_key_epoch
        || response.statement.audit_capsule_digest != response.audit_capsule_digest
        || response.delta.capsule_digest != response.audit_capsule_digest
        || response.delta.statement_digest != statement_digest
        || response.delta.proof_digest != proof_digest
    {
        return Err(eyre!(
            "private-settlement committee response is invalid or substituted"
        ));
    }
    if response.audit_approvals.len() < usize::from(response.audit_policy.body.min_approvals) {
        return Err(eyre!(
            "private-settlement committee response is invalid or substituted"
        ));
    }
    let mut previous_auditor = None;
    for approval in &response.audit_approvals {
        approval
            .verify(
                &response.audit_policy,
                response.manifest.authority_context_height,
            )
            .map_err(|_| {
                eyre!("private-settlement committee response is invalid or substituted")
            })?;
        if previous_auditor
            .as_ref()
            .is_some_and(|previous| previous >= &approval.body.auditor_id)
            || approval.body.network_id != response.statement.network_id
            || approval.body.bundle_id != response.statement.bundle_id
            || approval.body.leg_ordinal != response.statement.leg_ordinal
            || approval.body.dataspace_id != response.statement.route.dataspace_id
            || approval.body.audit_policy_digest != response.audit_policy.policy_digest
            || approval.body.audit_key_epoch != response.audit_policy.body.key_epoch
            || approval.body.proof_digest != proof_digest
            || approval.body.capsule_digest != response.audit_capsule_digest
            || approval.body.delta_digest != delta_digest
            || approval.body.old_root != response.delta.old_root
            || approval.body.new_root != response.delta.new_root
            || approval.body.expiry_height != response.statement.expiry_height
        {
            return Err(eyre!(
                "private-settlement committee response is invalid or substituted"
            ));
        }
        previous_auditor = Some(approval.body.auditor_id.clone());
    }
    Ok(())
}

fn validate_auditor_capsule_response_v1(
    requested: Hash,
    response: &PrivateSettlementAuditorCapsuleResponseV1,
) -> Result<()> {
    response
        .manifest
        .validate()
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    response
        .audit_policy
        .validate()
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    response
        .committee_authority
        .validate()
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    response
        .statement
        .validate()
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    response
        .delta
        .validate_against(&response.statement)
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    response
        .audit_capsule
        .validate_against(&response.audit_policy)
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    validate_availability_certificate_v1(&response.availability, &response.committee_authority)
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    let capsule_digest = response
        .audit_capsule
        .digest()
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    let delta_digest = response
        .delta
        .digest()
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    let authority_digest = response
        .committee_authority
        .digest()
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    let availability_digest = response
        .availability
        .digest()
        .map_err(|_| eyre!("private-settlement auditor response is invalid or substituted"))?;
    let ordinal = usize::from(response.statement.leg_ordinal);
    let leg =
        response.manifest.legs.get(ordinal).ok_or_else(|| {
            eyre!("private-settlement auditor response is invalid or substituted")
        })?;
    let availability = &response.availability.body;
    if availability.payload_digest != requested
        || leg.payload_digest != requested
        || leg.delta_digest != delta_digest
        || leg.availability_certificate_digest != availability_digest
        || leg.route != response.statement.route
        || leg.pool_id != response.statement.pool_id
        || leg.asset_binding_commitment != response.statement.asset_binding_commitment
        || leg.audit_policy_digest != response.audit_policy.policy_digest
        || response.audit_policy.body.dataspace_id != response.statement.route.dataspace_id
        || response.committee_authority.route != response.statement.route
        || availability.network_id != response.manifest.network_id
        || availability.bundle_id != response.manifest.bundle_id
        || availability.leg_ordinal != response.statement.leg_ordinal
        || availability.route != response.statement.route
        || availability.authority_digest != authority_digest
        || availability.authority_context_height != response.manifest.authority_context_height
        || availability.retention_until_height < response.manifest.expiry_height
        || response.manifest.bundle_id != response.statement.bundle_id
        || response.manifest.bundle_id != response.delta.bundle_id
        || response.statement.leg_ordinal != response.delta.leg_ordinal
        || response.statement.route != response.delta.route
        || response.statement.pool_id != response.delta.pool_id
        || response.statement.audit_capsule_digest != capsule_digest
        || response.delta.capsule_digest != capsule_digest
    {
        return Err(eyre!(
            "private-settlement auditor response is invalid or substituted"
        ));
    }
    Ok(())
}

fn validate_bundle_status_response_v1(
    requested: Hash,
    response: &PrivateSettlementBundleStatusResponseV1,
) -> Result<()> {
    if let Some(manifest) = &response.manifest {
        manifest
            .validate()
            .map_err(|_| eyre!("private-settlement bundle status is invalid or substituted"))?;
        if manifest.bundle_id != requested {
            return Err(eyre!(
                "private-settlement bundle status is invalid or substituted"
            ));
        }
    }
    match response.lifecycle {
        PrivateSettlementLifecycleDtoV1::Finalized
        | PrivateSettlementLifecycleDtoV1::Aborted
        | PrivateSettlementLifecycleDtoV1::Expired => {
            if response.finalized_height.is_none() {
                return Err(eyre!(
                    "private-settlement bundle status is invalid or substituted"
                ));
            }
        }
        _ if response.finalized_height.is_some() => {
            return Err(eyre!(
                "private-settlement bundle status is invalid or substituted"
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_bundle_receipt_response_v1(
    requested: Hash,
    response: &PrivateSettlementBundleReceiptResponseV1,
) -> Result<()> {
    match response {
        PrivateSettlementBundleReceiptResponseV1::Pending { bundle_id, .. } => {
            if *bundle_id != requested {
                return Err(eyre!(
                    "private-settlement receipt is invalid or substituted"
                ));
            }
        }
        PrivateSettlementBundleReceiptResponseV1::Finalized(receipt) => {
            receipt
                .validate_shape()
                .map_err(|_| eyre!("private-settlement receipt is invalid or substituted"))?;
            if receipt.manifest.bundle_id != requested {
                return Err(eyre!(
                    "private-settlement receipt is invalid or substituted"
                ));
            }
        }
        PrivateSettlementBundleReceiptResponseV1::Aborted(receipt) => {
            receipt
                .validate()
                .map_err(|_| eyre!("private-settlement receipt is invalid or substituted"))?;
            if receipt.bundle_id != requested {
                return Err(eyre!(
                    "private-settlement receipt is invalid or substituted"
                ));
            }
        }
    }
    Ok(())
}

impl Client {
    fn decode_private_settlement_response_v1<T>(
        response: Response<Vec<u8>>,
        context: &'static str,
    ) -> Result<T>
    where
        T: norito::json::JsonDeserializeOwned,
    {
        if !matches!(response.status(), StatusCode::OK | StatusCode::ACCEPTED) {
            return Err(ResponseReport::with_msg(context, &response)
                .unwrap_or_else(core::convert::identity)
                .into());
        }
        let content_type = Self::response_content_type(&response);
        if !Self::is_json_content_type(content_type) {
            return Err(eyre!(
                "{context}: invalid content-type `{content_type}` (expected application/json)"
            ));
        }
        norito::json::from_slice(response.body())
            .map_err(|error| eyre!("{context}: invalid JSON response: {error}"))
    }

    fn send_private_settlement_builder_v1(
        &self,
        builder: DefaultRequestBuilder,
    ) -> Result<Response<Vec<u8>>> {
        self.send_builder(builder.max_response_bytes(PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1))
    }

    /// Ask one committee endpoint to durably persist provisional material and sign it.
    ///
    /// The request is authenticated as the manifest sponsor. The returned
    /// share is verified locally against the exact request body and authority.
    ///
    /// # Errors
    ///
    /// Fails on local binding errors, request signing/transport failure, Torii
    /// rejection, or a malformed/substituted share.
    pub fn request_private_settlement_availability_share_v1(
        &self,
        endpoint: &Url,
        material: &PrivateSettlementProvisionalLegMaterialV1,
    ) -> Result<PrivateSettlementAvailabilityShareResponseV1> {
        material
            .validate()
            .map_err(|_| eyre!("private-settlement provisional leg is invalid"))?;
        if material.manifest.sponsor != self.account {
            return Err(eyre!(
                "private-settlement availability request requires the manifest sponsor"
            ));
        }
        let request = PrivateSettlementAvailabilityShareRequestV1 {
            material: material.clone(),
        };
        let body = norito::json::to_vec(&request)
            .wrap_err("failed to encode private-settlement availability request")?;
        let url = join_torii_url(
            endpoint,
            "v1/nexus/private-settlements/legs/availability-shares",
        );
        let response = self.send_private_settlement_builder_v1(
            self.account_signed_request(HttpMethod::POST, url, body)?
                .header("Content-Type", APPLICATION_JSON)
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded: PrivateSettlementAvailabilityShareResponseV1 =
            Self::decode_private_settlement_response_v1(
                response,
                "private-settlement availability share failed",
            )?;
        validate_availability_share_v1(
            &decoded.share,
            &material.availability_body,
            &material.committee_authority,
        )?;
        if decoded.bundle_id != material.manifest.bundle_id
            || decoded.payload_digest != material.availability_body.payload_digest
            || decoded.leg_ordinal != material.statement.leg_ordinal
        {
            return Err(eyre!(
                "private-settlement availability share response is substituted"
            ));
        }
        Ok(decoded)
    }

    /// Collect and aggregate exactly three shares from four roster-aligned endpoints.
    ///
    /// Endpoints must be supplied in the same order as the authority roster.
    /// Unavailable endpoints are skipped; every accepted response must be
    /// signed by the validator at its endpoint's roster index.
    ///
    /// # Errors
    ///
    /// Fails if endpoint ordering is invalid or fewer than three exact shares
    /// can be verified.
    pub fn certify_private_settlement_leg_availability_v1(
        &self,
        committee_endpoints: &[Url],
        material: &PrivateSettlementProvisionalLegMaterialV1,
    ) -> Result<PrivateSettlementSidecarAvailabilityV1> {
        material
            .validate()
            .map_err(|_| eyre!("private-settlement provisional leg is invalid"))?;
        if committee_endpoints.len() != material.committee_authority.validators.len() {
            return Err(eyre!(
                "private-settlement availability endpoints must match the four-validator roster"
            ));
        }
        let mut shares = Vec::with_capacity(material.committee_authority.validators.len());
        for (index, endpoint) in committee_endpoints.iter().enumerate() {
            let Ok(response) =
                self.request_private_settlement_availability_share_v1(endpoint, material)
            else {
                continue;
            };
            if response.share.signer != material.committee_authority.validators[index] {
                return Err(eyre!(
                    "private-settlement availability endpoint identity is substituted"
                ));
            }
            shares.push(response.share);
        }
        let selected = canonical_availability_share_quorum_v1(&shares)?;
        aggregate_availability_shares_v1(
            material.availability_body,
            &material.committee_authority,
            selected,
        )
    }

    /// Ask one participant endpoint to independently verify, stage, and vote Prepare.
    ///
    /// # Errors
    ///
    /// Fails on sponsor/binding errors, transport rejection, or a malformed or
    /// substituted node vote.
    pub fn request_private_settlement_prepare_vote_v1(
        &self,
        endpoint: &Url,
        manifest: &AtomicPrivateSettlementV1,
        payload_digest: Hash,
        authority: &PrivateSettlementCommitteeAuthorityV1,
    ) -> Result<PrivateSettlementPhaseVoteResponseV1> {
        manifest
            .validate()
            .map_err(|_| eyre!("private-settlement Prepare manifest is invalid"))?;
        if manifest.sponsor != self.account {
            return Err(eyre!(
                "private-settlement Prepare requires the manifest sponsor"
            ));
        }
        let ordinal = private_settlement_leg_ordinal_for_payload_v1(manifest, payload_digest)?;
        let expected = expected_phase_body_v1(
            manifest,
            ordinal,
            authority,
            PrivateSettlementPhaseV1::Prepare,
            private_settlement_reserved_prepared_digest_v1(),
        )?;
        let request = PrivateSettlementPrepareVoteRequestV1 {
            manifest: manifest.clone(),
            payload_digest,
        };
        let body = norito::json::to_vec(&request)
            .wrap_err("failed to encode private-settlement Prepare request")?;
        let url = join_torii_url(
            endpoint,
            "v1/nexus/private-settlements/phases/prepare-votes",
        );
        let response = self.send_private_settlement_builder_v1(
            self.account_signed_request(HttpMethod::POST, url, body)?
                .header("Content-Type", APPLICATION_JSON)
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded: PrivateSettlementPhaseVoteResponseV1 =
            Self::decode_private_settlement_response_v1(
                response,
                "private-settlement Prepare vote failed",
            )?;
        validate_phase_vote_v1(&decoded.vote, &expected, authority)?;
        if decoded.bundle_id != manifest.bundle_id
            || decoded.payload_digest != payload_digest
            || decoded.leg_ordinal != ordinal
        {
            return Err(eyre!("private-settlement Prepare response is substituted"));
        }
        Ok(decoded)
    }

    /// Fsync one exact aggregate participant certificate on a selected signer endpoint.
    ///
    /// # Errors
    ///
    /// Fails on sponsor/binding errors, transport rejection, or a substituted acknowledgement.
    pub fn persist_private_settlement_phase_certificate_v1(
        &self,
        endpoint: &Url,
        manifest: &AtomicPrivateSettlementV1,
        payload_digest: Hash,
        certificate: &PrivateSettlementPhaseCertificateV1,
    ) -> Result<PrivateSettlementPhaseCertificateResponseV1> {
        if manifest.sponsor != self.account {
            return Err(eyre!(
                "private-settlement phase persistence requires the manifest sponsor"
            ));
        }
        let request = PrivateSettlementPhaseCertificateRequestV1 {
            manifest: manifest.clone(),
            payload_digest,
            certificate: certificate.clone(),
        };
        let body = norito::json::to_vec(&request)
            .wrap_err("failed to encode private-settlement phase certificate")?;
        let url = join_torii_url(endpoint, "v1/nexus/private-settlements/phases/certificates");
        let response = self.send_private_settlement_builder_v1(
            self.account_signed_request(HttpMethod::POST, url, body)?
                .header("Content-Type", APPLICATION_JSON)
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded: PrivateSettlementPhaseCertificateResponseV1 =
            Self::decode_private_settlement_response_v1(
                response,
                "private-settlement phase certificate persistence failed",
            )?;
        let expected_lifecycle = match certificate.body.phase {
            PrivateSettlementPhaseV1::Prepare => PrivateSettlementLifecycleDtoV1::Prepared,
            PrivateSettlementPhaseV1::Commit => PrivateSettlementLifecycleDtoV1::CommitCertified,
        };
        if decoded.bundle_id != manifest.bundle_id
            || decoded.payload_digest != payload_digest
            || decoded.leg_ordinal != certificate.body.leg_ordinal
            || decoded.phase != certificate.body.phase
            || decoded.lifecycle != expected_lifecycle
        {
            return Err(eyre!(
                "private-settlement phase certificate acknowledgement is substituted"
            ));
        }
        Ok(decoded)
    }

    /// Fan out to all four validators, select exactly three votes canonically,
    /// and durably hand the QC back to every successfully staged node.
    ///
    /// Endpoint order must exactly match the authority roster. One unavailable
    /// endpoint is tolerated; malformed or identity-substituted responses are not.
    ///
    /// # Errors
    ///
    /// Fails for a non-four-node roster, fewer than three valid votes, or any
    /// failed durable certificate handoff.
    pub fn certify_private_settlement_prepare_v1(
        &self,
        committee_endpoints: &[Url],
        manifest: &AtomicPrivateSettlementV1,
        payload_digest: Hash,
        authority: &PrivateSettlementCommitteeAuthorityV1,
    ) -> Result<PrivateSettlementPhaseCertificateV1> {
        if committee_endpoints.len() != authority.validators.len() {
            return Err(eyre!(
                "private-settlement Prepare endpoints must match the four-validator roster"
            ));
        }
        let ordinal = private_settlement_leg_ordinal_for_payload_v1(manifest, payload_digest)?;
        let body = expected_phase_body_v1(
            manifest,
            ordinal,
            authority,
            PrivateSettlementPhaseV1::Prepare,
            private_settlement_reserved_prepared_digest_v1(),
        )?;
        let mut votes = Vec::with_capacity(authority.validators.len());
        let mut responders = Vec::with_capacity(authority.validators.len());
        for (index, endpoint) in committee_endpoints.iter().enumerate() {
            let Ok(response) = self.request_private_settlement_prepare_vote_v1(
                endpoint,
                manifest,
                payload_digest,
                authority,
            ) else {
                continue;
            };
            if response.vote.signer != authority.validators[index] {
                return Err(eyre!(
                    "private-settlement Prepare endpoint identity is substituted"
                ));
            }
            votes.push(response.vote);
            responders.push(index);
        }
        let selected = canonical_phase_vote_quorum_v1(&votes)?;
        let certificate = aggregate_phase_votes_v1(body, ordinal, authority, selected)?;
        for index in responders {
            let endpoint = &committee_endpoints[index];
            self.persist_private_settlement_phase_certificate_v1(
                endpoint,
                manifest,
                payload_digest,
                &certificate,
            )?;
        }
        Ok(certificate)
    }

    /// Build the exact complete all-Prepare barrier used by every Commit vote.
    ///
    /// # Errors
    ///
    /// Rejects incomplete, reordered, substituted, or cryptographically invalid material.
    pub fn build_private_settlement_prepare_barrier_v1(
        manifest: AtomicPrivateSettlementV1,
        authority_catalog: Vec<PrivateSettlementCommitteeAuthorityV1>,
        deltas: Vec<PrivateSettlementDeltaV1>,
        prepare_certificates: Vec<PrivateSettlementPhaseCertificateV1>,
    ) -> Result<PrivateSettlementPrepareBarrierV1> {
        let prepared_bundle_digest = prepared_bundle_digest_v1(
            &manifest,
            &authority_catalog,
            &deltas,
            &prepare_certificates,
        )?;
        let barrier = PrivateSettlementPrepareBarrierV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            manifest,
            authority_catalog,
            deltas,
            prepare_certificates,
            prepared_bundle_digest,
        };
        validate_prepare_barrier_v1(&barrier)?;
        Ok(barrier)
    }

    /// Prepare every canonical participant leg and construct the complete barrier.
    ///
    /// # Errors
    ///
    /// Fails on incomplete endpoint/authority/delta matrices or any per-leg
    /// verification, quorum, or certificate-persistence failure.
    pub fn prepare_private_settlement_bundle_v1(
        &self,
        committee_endpoints: &[Vec<Url>],
        manifest: &AtomicPrivateSettlementV1,
        authority_catalog: &[PrivateSettlementCommitteeAuthorityV1],
        deltas: &[PrivateSettlementDeltaV1],
    ) -> Result<PrivateSettlementPrepareBarrierV1> {
        if committee_endpoints.len() != manifest.legs.len()
            || authority_catalog.len() != manifest.legs.len()
            || deltas.len() != manifest.legs.len()
        {
            return Err(eyre!("private-settlement Prepare matrix is incomplete"));
        }
        let mut certificates = Vec::with_capacity(manifest.legs.len());
        for (index, ((endpoints, authority), delta)) in committee_endpoints
            .iter()
            .zip(authority_catalog)
            .zip(deltas)
            .enumerate()
        {
            if usize::from(delta.leg_ordinal) != index
                || delta
                    .digest()
                    .map_err(|_| eyre!("private-settlement delta encoding failed"))?
                    != manifest.legs[index].delta_digest
            {
                return Err(eyre!("private-settlement Prepare matrix is substituted"));
            }
            certificates.push(self.certify_private_settlement_prepare_v1(
                endpoints,
                manifest,
                manifest.legs[index].payload_digest,
                authority,
            )?);
        }
        Self::build_private_settlement_prepare_barrier_v1(
            manifest.clone(),
            authority_catalog.to_vec(),
            deltas.to_vec(),
            certificates,
        )
    }

    /// Ask one participant endpoint for a Commit vote over the exact complete barrier.
    ///
    /// # Errors
    ///
    /// Fails on invalid barrier material, transport rejection, or a malformed
    /// or substituted node vote.
    pub fn request_private_settlement_commit_vote_v1(
        &self,
        endpoint: &Url,
        payload_digest: Hash,
        barrier: &PrivateSettlementPrepareBarrierV1,
        authority: &PrivateSettlementCommitteeAuthorityV1,
    ) -> Result<PrivateSettlementPhaseVoteResponseV1> {
        validate_prepare_barrier_v1(barrier)?;
        if barrier.manifest.sponsor != self.account {
            return Err(eyre!(
                "private-settlement Commit requires the manifest sponsor"
            ));
        }
        let ordinal =
            private_settlement_leg_ordinal_for_payload_v1(&barrier.manifest, payload_digest)?;
        let expected = expected_phase_body_v1(
            &barrier.manifest,
            ordinal,
            authority,
            PrivateSettlementPhaseV1::Commit,
            barrier.prepared_bundle_digest,
        )?;
        let request = PrivateSettlementCommitVoteRequestV1 {
            payload_digest,
            barrier: barrier.clone(),
        };
        let body = norito::json::to_vec(&request)
            .wrap_err("failed to encode private-settlement Commit request")?;
        let url = join_torii_url(endpoint, "v1/nexus/private-settlements/phases/commit-votes");
        let response = self.send_private_settlement_builder_v1(
            self.account_signed_request(HttpMethod::POST, url, body)?
                .header("Content-Type", APPLICATION_JSON)
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded: PrivateSettlementPhaseVoteResponseV1 =
            Self::decode_private_settlement_response_v1(
                response,
                "private-settlement Commit vote failed",
            )?;
        validate_phase_vote_v1(&decoded.vote, &expected, authority)?;
        if decoded.bundle_id != barrier.manifest.bundle_id
            || decoded.payload_digest != payload_digest
            || decoded.leg_ordinal != ordinal
        {
            return Err(eyre!("private-settlement Commit response is substituted"));
        }
        Ok(decoded)
    }

    /// Fan out to all four validators, select exactly three Commit votes
    /// canonically, and durably hand the QC back to every successful responder.
    ///
    /// # Errors
    ///
    /// Fails for incomplete local Prepare persistence, fewer than three exact
    /// votes, identity substitution, or any failed Commit-QC handoff.
    pub fn certify_private_settlement_commit_v1(
        &self,
        committee_endpoints: &[Url],
        payload_digest: Hash,
        barrier: &PrivateSettlementPrepareBarrierV1,
        authority: &PrivateSettlementCommitteeAuthorityV1,
    ) -> Result<PrivateSettlementPhaseCertificateV1> {
        if committee_endpoints.len() != authority.validators.len() {
            return Err(eyre!(
                "private-settlement Commit endpoints must match the four-validator roster"
            ));
        }
        let ordinal =
            private_settlement_leg_ordinal_for_payload_v1(&barrier.manifest, payload_digest)?;
        let body = expected_phase_body_v1(
            &barrier.manifest,
            ordinal,
            authority,
            PrivateSettlementPhaseV1::Commit,
            barrier.prepared_bundle_digest,
        )?;
        let mut votes = Vec::with_capacity(authority.validators.len());
        let mut responders = Vec::with_capacity(authority.validators.len());
        for (index, endpoint) in committee_endpoints.iter().enumerate() {
            let Ok(response) = self.request_private_settlement_commit_vote_v1(
                endpoint,
                payload_digest,
                barrier,
                authority,
            ) else {
                continue;
            };
            if response.vote.signer != authority.validators[index] {
                return Err(eyre!(
                    "private-settlement Commit endpoint identity is substituted"
                ));
            }
            votes.push(response.vote);
            responders.push(index);
        }
        let selected = canonical_phase_vote_quorum_v1(&votes)?;
        let certificate = aggregate_phase_votes_v1(body, ordinal, authority, selected)?;
        for index in responders {
            let endpoint = &committee_endpoints[index];
            self.persist_private_settlement_phase_certificate_v1(
                endpoint,
                &barrier.manifest,
                payload_digest,
                &certificate,
            )?;
        }
        Ok(certificate)
    }

    /// Commit every participant leg against one exact complete Prepare barrier.
    ///
    /// # Errors
    ///
    /// Fails on an incomplete endpoint matrix or any per-leg Commit quorum or persistence error.
    pub fn commit_private_settlement_bundle_v1(
        &self,
        committee_endpoints: &[Vec<Url>],
        barrier: &PrivateSettlementPrepareBarrierV1,
    ) -> Result<Vec<PrivateSettlementPhaseCertificateV1>> {
        validate_prepare_barrier_v1(barrier)?;
        if committee_endpoints.len() != barrier.manifest.legs.len() {
            return Err(eyre!("private-settlement Commit matrix is incomplete"));
        }
        committee_endpoints
            .iter()
            .zip(&barrier.authority_catalog)
            .enumerate()
            .map(|(index, (endpoints, authority))| {
                self.certify_private_settlement_commit_v1(
                    endpoints,
                    barrier.manifest.legs[index].payload_digest,
                    barrier,
                    authority,
                )
            })
            .collect()
    }

    /// Upload one encrypted, fixed-shape settlement leg with sponsor authentication.
    ///
    /// # Errors
    ///
    /// Fails on local structural inconsistency, request signing/transport failure,
    /// Torii rejection, or a substituted response identity.
    pub fn upload_private_settlement_leg_v1(
        &self,
        request: &PrivateSettlementLegUploadRequestV1,
    ) -> Result<PrivateSettlementLegUploadResponseV1> {
        self.upload_private_settlement_leg_to_v1(&self.torii_url, request)
    }

    /// Upload one certified leg to a selected participant endpoint.
    ///
    /// # Errors
    ///
    /// Fails under the same conditions as [`Self::upload_private_settlement_leg_v1`].
    pub fn upload_private_settlement_leg_to_v1(
        &self,
        endpoint: &Url,
        request: &PrivateSettlementLegUploadRequestV1,
    ) -> Result<PrivateSettlementLegUploadResponseV1> {
        request
            .manifest
            .validate()
            .map_err(|_| eyre!("private-settlement leg upload is invalid"))?;
        request
            .payload
            .validate_against(&request.manifest, &request.audit_policy)
            .map_err(|_| eyre!("private-settlement leg upload is invalid"))?;
        validate_availability_certificate_v1(
            &request.payload.availability,
            &request.committee_authority,
        )
        .map_err(|_| eyre!("private-settlement leg upload is invalid"))?;
        if request.manifest.sponsor != self.account {
            return Err(eyre!(
                "private-settlement leg upload requires the manifest sponsor"
            ));
        }
        let body = norito::json::to_vec(request)
            .wrap_err("failed to encode private-settlement leg upload")?;
        let url = join_torii_url(endpoint, "v1/nexus/private-settlements/legs");
        let response = self.send_private_settlement_builder_v1(
            self.account_signed_request(HttpMethod::POST, url, body)?
                .header("Content-Type", APPLICATION_JSON)
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded: PrivateSettlementLegUploadResponseV1 =
            Self::decode_private_settlement_response_v1(
                response,
                "private-settlement leg upload failed",
            )?;
        if decoded.bundle_id != request.manifest.bundle_id
            || decoded.payload_digest != request.payload.availability.body.payload_digest
            || decoded.leg_ordinal != request.payload.statement.leg_ordinal
        {
            return Err(eyre!(
                "private-settlement leg upload response is substituted"
            ));
        }
        Ok(decoded)
    }

    /// Certify every leg, finalize one common manifest, and promote each leg on its signers.
    ///
    /// `committee_endpoints[leg][validator]` is aligned with the canonical leg
    /// ordinal and that leg's four-validator authority roster. Exactly three
    /// shares per leg are aggregated after all four endpoints have been tried;
    /// the completed sidecar is then submitted to every available responder.
    ///
    /// # Errors
    ///
    /// Fails on inconsistent provisional manifests, endpoint matrices,
    /// insufficient shares, final manifest validation, or any promotion error.
    pub fn certify_and_upload_private_settlement_legs_v1(
        &self,
        materials: &[PrivateSettlementProvisionalLegMaterialV1],
        committee_endpoints: &[Vec<Url>],
    ) -> Result<Vec<PrivateSettlementLegUploadResponseV1>> {
        let first = materials
            .first()
            .ok_or_else(|| eyre!("private-settlement provisional leg set is empty"))?;
        if materials.len() != first.manifest.legs.len()
            || committee_endpoints.len() != materials.len()
        {
            return Err(eyre!(
                "private-settlement provisional leg set is incomplete"
            ));
        }
        first
            .manifest
            .validate_provisional()
            .map_err(|_| eyre!("private-settlement provisional manifest is invalid"))?;
        let mut certificates = Vec::with_capacity(materials.len());
        for (index, (material, endpoints)) in materials.iter().zip(committee_endpoints).enumerate()
        {
            if material.manifest != first.manifest
                || usize::from(material.statement.leg_ordinal) != index
            {
                return Err(eyre!(
                    "private-settlement provisional leg set is inconsistent"
                ));
            }
            certificates
                .push(self.certify_private_settlement_leg_availability_v1(endpoints, material)?);
        }
        let mut final_manifest = first.manifest.clone();
        for (index, certificate) in certificates.iter().enumerate() {
            final_manifest.legs[index].availability_certificate_digest = certificate
                .digest()
                .map_err(|_| eyre!("private-settlement certificate encoding failed"))?;
        }
        final_manifest
            .validate()
            .map_err(|_| eyre!("private-settlement final manifest is invalid"))?;

        let mut responses =
            Vec::with_capacity(materials.len() * first.committee_authority.validators.len());
        for (index, ((material, certificate), endpoints)) in materials
            .iter()
            .zip(&certificates)
            .zip(committee_endpoints)
            .enumerate()
        {
            let payload = material.payload_with_certificate(certificate.clone());
            let request = PrivateSettlementLegUploadRequestV1 {
                manifest: final_manifest.clone(),
                audit_policy: material.audit_policy.clone(),
                committee_authority: material.committee_authority.clone(),
                payload,
            };
            for (validator_index, endpoint) in endpoints.iter().enumerate() {
                match self.upload_private_settlement_leg_to_v1(endpoint, &request) {
                    Ok(response) => {
                        if usize::from(response.leg_ordinal) != index {
                            return Err(eyre!(
                                "private-settlement promotion response is substituted"
                            ));
                        }
                        responses.push(response);
                    }
                    Err(error) if certificate.signers_bitmap & (1_u8 << validator_index) != 0 => {
                        return Err(error).wrap_err(
                            "private-settlement availability signer did not durably promote",
                        );
                    }
                    Err(_) => {}
                }
            }
        }
        Ok(responses)
    }

    /// Read one account-authenticated, redacted leg lifecycle projection.
    ///
    /// # Errors
    ///
    /// Fails on request signing/transport failure, Torii rejection, or a
    /// malformed/substituted status response.
    pub fn private_settlement_leg_status_v1(
        &self,
        payload_digest: Hash,
    ) -> Result<PrivateSettlementLegStatusResponseV1> {
        let path = private_settlement_resource_path_v1("legs", &payload_digest, "/status");
        let url = join_torii_url(&self.torii_url, &path);
        let response = self.send_private_settlement_builder_v1(
            self.account_signed_request(HttpMethod::GET, url, Vec::new())?
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded = Self::decode_private_settlement_response_v1(
            response,
            "private-settlement leg status failed",
        )?;
        validate_leg_status_response_v1(payload_digest, &decoded)?;
        Ok(decoded)
    }

    /// Fetch proof material as one exact committee validator identity.
    ///
    /// Encrypted capsule bytes are never returned by this route.
    ///
    /// # Errors
    ///
    /// Fails on identity signing/transport failure, authorization denial, or a
    /// malformed/substituted proof view.
    pub fn private_settlement_committee_proof_v1(
        &self,
        payload_digest: Hash,
        validator_key: &KeyPair,
    ) -> Result<PrivateSettlementCommitteeProofResponseV1> {
        let path = private_settlement_resource_path_v1("legs", &payload_digest, "/committee-proof");
        let url = join_torii_url(&self.torii_url, &path);
        let response = self.send_private_settlement_builder_v1(
            self.identity_signed_request(validator_key, HttpMethod::GET, url, Vec::new())?
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded = Self::decode_private_settlement_response_v1(
            response,
            "private-settlement committee proof fetch failed",
        )?;
        validate_restricted_proof_response_v1(payload_digest, &decoded)?;
        Ok(decoded)
    }

    /// Fetch one padded encrypted capsule as an exact governed auditor identity.
    ///
    /// # Errors
    ///
    /// Fails on identity signing/transport failure, authorization denial, or a
    /// malformed/substituted auditor view.
    pub fn private_settlement_auditor_capsule_v1(
        &self,
        payload_digest: Hash,
        auditor_signing_key: &KeyPair,
    ) -> Result<PrivateSettlementAuditorCapsuleResponseV1> {
        let path = private_settlement_resource_path_v1("legs", &payload_digest, "/audit-capsule");
        let url = join_torii_url(&self.torii_url, &path);
        let response = self.send_private_settlement_builder_v1(
            self.identity_signed_request(auditor_signing_key, HttpMethod::GET, url, Vec::new())?
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded = Self::decode_private_settlement_response_v1(
            response,
            "private-settlement auditor capsule fetch failed",
        )?;
        validate_auditor_capsule_response_v1(payload_digest, &decoded)?;
        Ok(decoded)
    }

    /// Submit one auditor approval under the exact purpose-separated signing identity.
    ///
    /// # Errors
    ///
    /// Fails on identity mismatch, request signing/transport failure, Torii
    /// rejection, or a malformed/substituted acknowledgement.
    pub fn submit_private_settlement_audit_approval_v1(
        &self,
        payload_digest: Hash,
        auditor_signing_key: &KeyPair,
        request: &PrivateSettlementAuditApprovalRequestV1,
    ) -> Result<PrivateSettlementAuditApprovalResponseV1> {
        request
            .approval
            .signature
            .verify(auditor_signing_key.public_key(), &request.approval.body)
            .map_err(|_| eyre!("private-settlement auditor approval identity is invalid"))?;
        let body = norito::json::to_vec(request)
            .wrap_err("failed to encode private-settlement auditor approval")?;
        let path = private_settlement_resource_path_v1("legs", &payload_digest, "/audit-approvals");
        let url = join_torii_url(&self.torii_url, &path);
        let response = self.send_private_settlement_builder_v1(
            self.identity_signed_request(auditor_signing_key, HttpMethod::POST, url, body)?
                .header("Content-Type", APPLICATION_JSON)
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded: PrivateSettlementAuditApprovalResponseV1 =
            Self::decode_private_settlement_response_v1(
                response,
                "private-settlement auditor approval failed",
            )?;
        if decoded.payload_digest != payload_digest
            || decoded.bundle_id != request.approval.body.bundle_id
            || decoded.leg_ordinal != request.approval.body.leg_ordinal
            || decoded.required == 0
            || decoded.collected > decoded.required
        {
            return Err(eyre!(
                "private-settlement auditor approval response is substituted"
            ));
        }
        Ok(decoded)
    }

    /// Submit one exact sponsor-signed global finalization carrier.
    ///
    /// # Errors
    ///
    /// Fails before dispatch if the transaction is not the exact direct carrier
    /// for this client sponsor, or after dispatch if Torii returns a substituted
    /// carrier identity.
    pub fn submit_private_settlement_bundle_v1(
        &self,
        request: &PrivateSettlementBundleSubmitRequestV1,
    ) -> Result<PrivateSettlementBundleSubmitResponseV1> {
        let carrier = exact_private_settlement_carrier_v1(&request.transaction)?;
        if carrier.commit_bundle.manifest.sponsor != self.account
            || request.transaction.authority() != &self.account
            || request.transaction.fee_payment_intent()
                != &carrier.commit_bundle.manifest.public_fee_intent
        {
            return Err(eyre!(
                "private-settlement bundle carrier sponsor binding is invalid"
            ));
        }
        let expected_bundle = carrier.commit_bundle.manifest.bundle_id;
        let expected_carrier = Hash::from(request.transaction.hash());
        let body = norito::json::to_vec(request)
            .wrap_err("failed to encode private-settlement bundle carrier")?;
        let url = join_torii_url(&self.torii_url, "v1/nexus/private-settlements/bundles");
        let response = self.send_private_settlement_builder_v1(
            self.account_signed_request(HttpMethod::POST, url, body)?
                .header("Content-Type", APPLICATION_JSON)
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded: PrivateSettlementBundleSubmitResponseV1 =
            Self::decode_private_settlement_response_v1(
                response,
                "private-settlement bundle submission failed",
            )?;
        if decoded.bundle_id != expected_bundle || decoded.carrier_id != expected_carrier {
            return Err(eyre!("private-settlement bundle response is substituted"));
        }
        Ok(decoded)
    }

    /// Read the public allowlisted lifecycle for one bundle.
    ///
    /// # Errors
    ///
    /// Fails on transport/Torii error or a malformed/substituted status.
    pub fn private_settlement_bundle_status_v1(
        &self,
        bundle_id: Hash,
    ) -> Result<PrivateSettlementBundleStatusResponseV1> {
        let path = private_settlement_resource_path_v1("bundles", &bundle_id, "");
        let url = join_torii_url(&self.torii_url, &path);
        let response = self.send_private_settlement_builder_v1(
            self.request_without_iroha_identity_auth(HttpMethod::GET, url)
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded = Self::decode_private_settlement_response_v1(
            response,
            "private-settlement bundle status failed",
        )?;
        validate_bundle_status_response_v1(bundle_id, &decoded)?;
        Ok(decoded)
    }

    /// Read the public terminal receipt or pending marker for one bundle.
    ///
    /// # Errors
    ///
    /// Fails on transport/Torii error or a malformed/substituted receipt.
    pub fn private_settlement_bundle_receipt_v1(
        &self,
        bundle_id: Hash,
    ) -> Result<PrivateSettlementBundleReceiptResponseV1> {
        let path = private_settlement_resource_path_v1("bundles", &bundle_id, "/receipt");
        let url = join_torii_url(&self.torii_url, &path);
        let response = self.send_private_settlement_builder_v1(
            self.request_without_iroha_identity_auth(HttpMethod::GET, url)
                .header("Accept", APPLICATION_JSON),
        )?;
        let decoded = Self::decode_private_settlement_response_v1(
            response,
            "private-settlement bundle receipt failed",
        )?;
        validate_bundle_receipt_response_v1(bundle_id, &decoded)?;
        Ok(decoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::evidence_http_tests::{
        SnapshotStore, base_url, client_with_base_url, respond_with, with_mock_http,
    };
    use std::sync::{Arc, Mutex};

    fn phase_fixture_v1() -> (
        PrivateSettlementCommitteeAuthorityV1,
        Vec<iroha_crypto::KeyPair>,
        PrivateSettlementPhaseBodyV1,
    ) {
        let route = iroha_data_model::nexus::PrivateSettlementRouteV1 {
            dataspace_id: iroha_data_model::nexus::DataSpaceId::new(31),
            lane_id: iroha_data_model::nexus::LaneId::new(7),
            lane_incarnation: Hash::new(b"client-phase-incarnation"),
        };
        let keys = (0_u8..4)
            .map(|index| {
                iroha_crypto::KeyPair::from_seed(
                    vec![0x81_u8.saturating_add(index); 32],
                    Algorithm::BlsNormal,
                )
            })
            .collect::<Vec<_>>();
        let validators = keys
            .iter()
            .map(|key| iroha_data_model::peer::PeerId::from(key.public_key().clone()))
            .collect::<Vec<_>>();
        let authority = PrivateSettlementCommitteeAuthorityV1 {
            route,
            validator_set_hash: iroha_crypto::HashOf::new(&validators),
            validators,
            validator_pops: keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP")
                })
                .collect(),
        };
        let body = PrivateSettlementPhaseBodyV1 {
            network_id: iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                iroha_data_model::block::BlockHeader,
            >::from_untyped_unchecked(
                Hash::new(b"client-phase-network"),
            )),
            bundle_id: Hash::new(b"client-phase-bundle"),
            manifest_digest: Hash::new(b"client-phase-manifest"),
            leg_ordinal: 0,
            route,
            delta_digest: Hash::new(b"client-phase-delta"),
            authority_digest: authority.digest().expect("authority digest"),
            prepared_bundle_digest: private_settlement_reserved_prepared_digest_v1(),
            phase: PrivateSettlementPhaseV1::Prepare,
            authority_context_height: 10,
            expiry_height: 100,
        };
        (authority, keys, body)
    }

    fn phase_votes_v1(
        authority: &PrivateSettlementCommitteeAuthorityV1,
        keys: &[iroha_crypto::KeyPair],
        body: PrivateSettlementPhaseBodyV1,
        indexes: &[usize],
    ) -> Vec<PrivateSettlementPhaseVoteV1> {
        let preimage = phase_signature_preimage_v1(&body).expect("phase preimage");
        indexes
            .iter()
            .map(|index| PrivateSettlementPhaseVoteV1 {
                version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                body,
                signer: authority.validators[*index].clone(),
                signature: Signature::try_new(keys[*index].private_key(), &preimage)
                    .expect("phase signature")
                    .payload()
                    .to_vec(),
            })
            .collect()
    }

    fn availability_shares_v1(
        authority: &PrivateSettlementCommitteeAuthorityV1,
        keys: &[iroha_crypto::KeyPair],
    ) -> Vec<PrivateSettlementAvailabilityShareV1> {
        let body = PrivateSettlementSidecarAvailabilityBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                iroha_data_model::block::BlockHeader,
            >::from_untyped_unchecked(
                Hash::new(b"client-availability-network"),
            )),
            bundle_id: Hash::new(b"client-availability-bundle"),
            leg_ordinal: 0,
            route: authority.route,
            authority_digest: authority.digest().expect("authority digest"),
            authority_context_height: 10,
            payload_digest: Hash::new(b"client-availability-payload"),
            payload_bytes: 128,
            retention_until_height: 100,
        };
        let preimage = body.signature_preimage().expect("availability preimage");
        keys.iter()
            .enumerate()
            .map(|(index, key)| PrivateSettlementAvailabilityShareV1 {
                version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
                body,
                signer: authority.validators[index].clone(),
                signature: Signature::try_new(key.private_key(), &preimage)
                    .expect("availability signature")
                    .payload()
                    .to_vec(),
            })
            .collect()
    }

    #[test]
    fn resource_paths_are_exact_and_query_free() {
        let digest = Hash::new(b"private-settlement-client-path");
        let encoded = digest.to_string();
        assert_eq!(
            private_settlement_resource_path_v1("legs", &digest, "/status"),
            format!("v1/nexus/private-settlements/legs/{encoded}/status")
        );
        assert_eq!(
            private_settlement_resource_path_v1("bundles", &digest, "/receipt"),
            format!("v1/nexus/private-settlements/bundles/{encoded}/receipt")
        );
    }

    #[test]
    fn client_phase_aggregation_requires_exact_three_distinct_valid_votes() {
        let (authority, keys, body) = phase_fixture_v1();
        let exact = phase_votes_v1(&authority, &keys, body, &[0, 1, 3]);
        let certificate =
            aggregate_phase_votes_v1(body, 0, &authority, &exact).expect("exact quorum");
        assert_eq!(certificate.signers_bitmap, 0b1011);
        assert!(validate_phase_certificate_v1(&certificate, &body, 0, &authority).is_ok());

        assert!(
            aggregate_phase_votes_v1(
                body,
                0,
                &authority,
                &phase_votes_v1(&authority, &keys, body, &[0, 1]),
            )
            .is_err()
        );
        assert!(
            aggregate_phase_votes_v1(
                body,
                0,
                &authority,
                &phase_votes_v1(&authority, &keys, body, &[0, 1, 2, 3]),
            )
            .is_err()
        );

        let mut duplicate = phase_votes_v1(&authority, &keys, body, &[0, 1, 2]);
        duplicate[1] = duplicate[0].clone();
        assert!(aggregate_phase_votes_v1(body, 0, &authority, &duplicate).is_err());
        let mut malformed = phase_votes_v1(&authority, &keys, body, &[0, 1, 2]);
        malformed[2].signature[0] ^= 1;
        assert!(aggregate_phase_votes_v1(body, 0, &authority, &malformed).is_err());

        let all_four = phase_votes_v1(&authority, &keys, body, &[0, 1, 2, 3]);
        let selected = canonical_phase_vote_quorum_v1(&all_four)
            .expect("four-endpoint fanout deterministically selects a quorum");
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].signer, authority.validators[0]);
        assert_eq!(selected[1].signer, authority.validators[1]);
        assert_eq!(selected[2].signer, authority.validators[2]);
        assert!(
            canonical_phase_vote_quorum_v1(&phase_votes_v1(&authority, &keys, body, &[0, 1],))
                .is_err()
        );
    }

    #[test]
    fn availability_fanout_selects_the_canonical_three_of_four() {
        let (authority, keys, _) = phase_fixture_v1();
        let all_four = availability_shares_v1(&authority, &keys);
        let selected = canonical_availability_share_quorum_v1(&all_four)
            .expect("four-endpoint availability fanout has a quorum");
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].signer, authority.validators[0]);
        assert_eq!(selected[1].signer, authority.validators[1]);
        assert_eq!(selected[2].signer, authority.validators[2]);
        let certificate = aggregate_availability_shares_v1(all_four[0].body, &authority, selected)
            .expect("canonical availability quorum");
        assert_eq!(certificate.signers_bitmap, 0b0111);
        assert!(canonical_availability_share_quorum_v1(&all_four[..2]).is_err());
    }

    #[test]
    fn pending_receipt_rejects_identifier_substitution() {
        let requested = Hash::new(b"requested-private-settlement-bundle");
        let substituted = Hash::new(b"substituted-private-settlement-bundle");
        let response = PrivateSettlementBundleReceiptResponseV1::Pending {
            bundle_id: substituted,
            lifecycle: PrivateSettlementLifecycleDtoV1::Collecting,
        };
        assert!(validate_bundle_receipt_response_v1(requested, &response).is_err());
    }

    #[test]
    fn nonterminal_status_rejects_a_finalized_height() {
        let bundle_id = Hash::new(b"private-settlement-status-bundle");
        let response = PrivateSettlementBundleStatusResponseV1 {
            manifest: None,
            lifecycle: PrivateSettlementLifecycleDtoV1::Prepared,
            finalized_height: Some(42),
        };
        assert!(validate_bundle_status_response_v1(bundle_id, &response).is_err());
    }

    #[test]
    fn public_bundle_status_strips_iroha_identity_headers_but_retains_gateway_auth() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let mut client = client_with_base_url(base_url());
        client
            .headers
            .insert("Authorization".to_owned(), "Basic gateway".to_owned());
        for header in [
            HEADER_ACCOUNT,
            HEADER_SIGNATURE,
            HEADER_TIMESTAMP_MS,
            HEADER_NONCE,
            HEADER_OPERATOR_PUBLIC_KEY,
            HEADER_OPERATOR_TIMESTAMP_MS,
            HEADER_OPERATOR_NONCE,
            HEADER_OPERATOR_SIGNATURE,
        ] {
            client
                .headers
                .insert(header.to_owned(), "must-not-leak".to_owned());
        }
        let bundle_id = Hash::new(b"public-private-settlement-status");
        let _ = with_mock_http(
            respond_with(
                &snapshots,
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .expect("response build"),
            ),
            || client.private_settlement_bundle_status_v1(bundle_id),
        )
        .expect_err("mocked missing bundle fails");
        let snapshots = snapshots.lock().expect("lock snapshots");
        assert_eq!(snapshots.len(), 1);
        let request = &snapshots[0];
        assert_eq!(request.method, HttpMethod::GET);
        assert!(request.url.query().is_none());
        assert!(request.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("authorization") && value == "Basic gateway"
        }));
        for header in [
            HEADER_ACCOUNT,
            HEADER_SIGNATURE,
            HEADER_TIMESTAMP_MS,
            HEADER_NONCE,
            HEADER_OPERATOR_PUBLIC_KEY,
            HEADER_OPERATOR_TIMESTAMP_MS,
            HEADER_OPERATOR_NONCE,
            HEADER_OPERATOR_SIGNATURE,
        ] {
            assert!(
                !request
                    .headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case(header)),
                "public query leaked {header}"
            );
        }
    }

    #[test]
    fn committee_fetch_uses_the_explicit_identity_key() {
        let snapshots: SnapshotStore = Arc::new(Mutex::new(Vec::new()));
        let client = client_with_base_url(base_url());
        let validator = checked_random_keypair();
        let payload_digest = Hash::new(b"committee-private-settlement-proof");
        let _ = with_mock_http(
            respond_with(
                &snapshots,
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .expect("response build"),
            ),
            || client.private_settlement_committee_proof_v1(payload_digest, &validator),
        )
        .expect_err("mocked unauthorized committee read fails");
        let snapshots = snapshots.lock().expect("lock snapshots");
        assert_eq!(snapshots.len(), 1);
        let request = &snapshots[0];
        assert_eq!(request.method, HttpMethod::GET);
        assert!(request.url.path().ends_with("/committee-proof"));
        assert!(request.body.is_empty());
        let expected_key = validator
            .public_key()
            .try_to_multihash_string()
            .expect("encode validator key");
        assert!(request.headers.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case(HEADER_OPERATOR_PUBLIC_KEY) && value == &expected_key
        }));
    }
}
