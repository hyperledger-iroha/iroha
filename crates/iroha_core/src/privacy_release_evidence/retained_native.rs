//! Release stages for retained native note protocols.
//!
//! IVM private notes and PQ-MASP share the parent module's canonical native
//! statement binding and deterministic evidence infrastructure.
// This is a private continuation of the parent release-evidence module.
use super::*;
/// Bind the exact genesis already selected when the release fixture was built.
///
/// The fixture context has already entered note and accumulator commitments;
/// deriving the binding must not replace that context with an unrelated network.
fn native_release_fixture_consensus_binding_v1(
    context: &PrivacyStatementContextV1,
    consensus_limits: &PrivacyConsensusLimitsV1,
) -> Result<PrivacyNativeConsensusBindingV1, PrivacyReleaseEvidenceErrorClassV1> {
    PrivacyNativeConsensusBindingV1::new(context, *context.network_id.as_bytes(), consensus_limits)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
}
fn redigest_ivm_release_statement_v1(
    statement: &mut iroha_data_model::privacy::IrohaIvmPrivateNoteStarkStatementV1,
) -> Result<(), PrivacyReleaseEvidenceErrorClassV1> {
    statement.action_digest = iroha_data_model::privacy::PrivacyActionDigestV1::new([0; 32]);
    statement.action_digest = statement
        .computed_action_digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    Ok(())
}
fn native_bound_statement_material_v1(
    domain: &[u8],
    statement: &PrivacyStatementV1,
    consensus_binding: &PrivacyNativeConsensusBindingV1,
) -> Result<Vec<u8>, PrivacyReleaseEvidenceErrorClassV1> {
    let statement_bytes = norito::encode_canonical(statement)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let statement_len = u64::try_from(statement_bytes.len())
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let binding_digest = consensus_binding
        .digest()
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
    let mut material = Vec::new();
    material.extend_from_slice(domain);
    material.extend_from_slice(&statement_len.to_be_bytes());
    material.extend_from_slice(&statement_bytes);
    material.extend_from_slice(binding_digest.as_bytes());
    Ok(material)
}
pub(super) fn run_ivm_private_note_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let protocol_id = PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1;
    let fixture_seed =
        stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-fixture-encryption")?;
    let mut fixture_rng = EvidenceRng06::new(fixture_seed);
    let fixture = ivm_private_note_release_fixture_v1(maximum, &mut fixture_rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let statement = fixture.statement;
    let witness = fixture.witness;
    let expected_units = if maximum { 2 } else { 1 };
    if witness.inputs().len() != expected_units
        || witness.outputs().len() != expected_units
        || statement.nullifiers.len() != expected_units
        || statement.output_commitments.len() != expected_units
        || statement.encrypted_outputs.len() != expected_units
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let proof_seed = stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-proof")?;
    let mut proof_rng = EvidenceRng09::new(proof_seed);
    let consensus_limits = PrivacyConsensusLimitsV1::taira_default();
    let consensus_binding =
        native_release_fixture_consensus_binding_v1(&statement.context, &consensus_limits)?;
    let proof = prove_ivm_private_note_v1_with_rng(
        &statement,
        &consensus_binding,
        &consensus_limits,
        &witness,
        &mut proof_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    verify_ivm_private_note_v1(&statement, &consensus_binding, &consensus_limits, &proof)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let original_typed = PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement.clone());
    let original_material = native_bound_statement_material_v1(
        b"iroha.privacy.release.ivm-private-note.bound-statement.v1",
        &original_typed,
        &consensus_binding,
    )?;
    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            original_material,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut cross_context = statement.clone();
            cross_context.context.network_id = release_network_id_from_genesis_hash([0x4a; 32]);
            redigest_ivm_release_statement_v1(&mut cross_context)?;
            let mut cross_intent = statement.clone();
            let mut intent = *cross_intent.context.transaction_intent_digest.as_bytes();
            intent[0] ^= 0x80;
            cross_intent.context.transaction_intent_digest =
                PrivacyTransactionIntentDigestV1::new(intent);
            redigest_ivm_release_statement_v1(&mut cross_intent)?;
            let mut cross_root = statement.clone();
            let mut root = *cross_root.state_root.as_bytes();
            root[0] ^= 0x80;
            cross_root.state_root = PrivacyRootV1::new(root);
            redigest_ivm_release_statement_v1(&mut cross_root)?;
            let mut cross_epoch = statement.clone();
            cross_epoch.root_epoch = cross_epoch
                .root_epoch
                .checked_add(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            cross_epoch.execution_epoch = cross_epoch.root_epoch;
            redigest_ivm_release_statement_v1(&mut cross_epoch)?;
            for mutation in [&cross_context, &cross_intent, &cross_root, &cross_epoch] {
                if verify_ivm_private_note_v1(
                    mutation,
                    &consensus_binding,
                    &consensus_limits,
                    &proof,
                )
                .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
            }
            let mut changed_genesis_binding = consensus_binding.clone();
            changed_genesis_binding.genesis_hash[0] ^= 0x80;
            if verify_ivm_private_note_v1(
                &statement,
                &changed_genesis_binding,
                &consensus_limits,
                &proof,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                native_bound_statement_material_v1(
                    b"iroha.privacy.release.ivm-private-note.bound-statement.v1",
                    &original_typed,
                    &changed_genesis_binding,
                )?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let invalid_fixture_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-fixture-encryption")?;
            let mut invalid_fixture_rng = EvidenceRng06::new(invalid_fixture_seed);
            let invalid =
                ivm_private_note_release_invalid_path_fixture_v1(&mut invalid_fixture_rng)
                    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            let invalid_proof_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-proof")?;
            let mut invalid_proof_rng = EvidenceRng09::new(invalid_proof_seed);
            let invalid_consensus_binding = native_release_fixture_consensus_binding_v1(
                &invalid.statement.context,
                &consensus_limits,
            )?;
            if prove_ivm_private_note_v1_with_rng(
                &invalid.statement,
                &invalid_consensus_binding,
                &consensus_limits,
                &invalid.witness,
                &mut invalid_proof_rng,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::InvalidWitnessPathAccepted);
            }
            let mut corrupt_header = proof.clone();
            let first = corrupt_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_ivm_private_note_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_header,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let mut corrupt_interior = proof.clone();
            let interior = corrupt_interior.len() / 2;
            let byte = corrupt_interior
                .get_mut(interior)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *byte ^= 0x01;
            if verify_ivm_private_note_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_interior,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let truncated_length = proof
                .len()
                .checked_sub(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_ivm_private_note_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &proof[..truncated_length],
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                original_material,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };
    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof,
            u64::try_from(IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1)
                .expect("closed private-note proof ceiling fits u64"),
        ),
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(witness.inputs().len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: u64::try_from(PRIVATE_NOTE_MAX_INPUTS_V1)
                .expect("closed private-note input ceiling fits u64"),
            secondary_units: u64::try_from(witness.outputs().len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            secondary_ceiling: u64::try_from(PRIVATE_NOTE_MAX_OUTPUTS_V1)
                .expect("closed private-note output ceiling fits u64"),
            relation_depth: u64::try_from(PRIVATE_NOTE_TREE_DEPTH_V1)
                .expect("closed private-note tree depth fits u64"),
            relation_depth_ceiling: u64::try_from(PRIVATE_NOTE_TREE_DEPTH_V1)
                .expect("closed private-note tree depth fits u64"),
        },
        failure_class,
    })
}
pub(super) fn run_pq_masp_stage_v1(
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<StageMaterialV1, PrivacyReleaseEvidenceErrorClassV1> {
    let maximum = case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource;
    let protocol_id = PrivacyProtocolIdV1::PqMaspStarkV1;
    let keygen_seed = stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-fixture-keygen")?;
    let fixture_seed =
        stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-fixture-encryption")?;
    let mut fixture_rng = EvidenceRng09::new(fixture_seed);
    let fixture = pq_masp_release_fixture_v1(maximum, keygen_seed, &mut fixture_rng)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
    let statement = fixture.statement;
    let witness = fixture.witness;
    let authorization_secret_key = fixture.authorization_secret_key;
    let expected_units = if maximum { 2 } else { 1 };
    if witness.inputs().len() != expected_units
        || witness.outputs().len() != expected_units
        || statement.nullifiers.len() != expected_units
        || statement.output_commitments.len() != expected_units
        || statement.encrypted_outputs.len() != expected_units
    {
        return Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant);
    }
    let proof_seed = stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-proof")?;
    let mut proof_rng = EvidenceRng09::new(proof_seed);
    let consensus_limits = PrivacyConsensusLimitsV1::taira_default();
    let consensus_binding =
        native_release_fixture_consensus_binding_v1(&statement.context, &consensus_limits)?;
    let proof = prove_pq_masp_v1_with_rng(
        &statement,
        &consensus_binding,
        &consensus_limits,
        &witness,
        authorization_secret_key.as_slice(),
        &mut proof_rng,
    )
    .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeProverRejected)?;
    verify_pq_masp_v1(&statement, &consensus_binding, &consensus_limits, &proof)
        .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected)?;
    let original_typed = PrivacyStatementV1::PqMaspStarkV1(statement.clone());
    let original_material = native_bound_statement_material_v1(
        b"iroha.privacy.release.pq-masp.bound-statement.v1",
        &original_typed,
        &consensus_binding,
    )?;
    let (public_statement_material, failure_class) = match case_kind {
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        | PrivacyReleaseCaseKindV1::MaximumShapeResource => (
            original_material,
            PrivacyReleaseFailureClassV1::NotApplicable,
        ),
        PrivacyReleaseCaseKindV1::PublicStatementBindingMutation => {
            let mut cross_context = statement.clone();
            cross_context.context.network_id = release_network_id_from_genesis_hash([0x51; 32]);
            let mut cross_intent = statement.clone();
            let mut intent = *cross_intent.context.transaction_intent_digest.as_bytes();
            intent[0] ^= 0x80;
            cross_intent.context.transaction_intent_digest =
                PrivacyTransactionIntentDigestV1::new(intent);
            let mut cross_anchor = statement.clone();
            let mut anchor = *cross_anchor.anchor.as_bytes();
            anchor[0] ^= 0x80;
            cross_anchor.anchor = PrivacyRootV1::new(anchor);
            let mut cross_epoch = statement.clone();
            cross_epoch.anchor_epoch = cross_epoch
                .anchor_epoch
                .checked_add(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            cross_epoch.authorization_epoch = cross_epoch.anchor_epoch;
            let mut cross_key = statement.clone();
            let mut key_digest = *cross_key.authorization_key_digest.as_bytes();
            key_digest[0] ^= 0x80;
            cross_key.authorization_key_digest =
                iroha_data_model::privacy::PrivacyAuthorizationKeyDigestV1::new(key_digest);
            for mutation in [
                &cross_context,
                &cross_intent,
                &cross_anchor,
                &cross_epoch,
                &cross_key,
            ] {
                if verify_pq_masp_v1(mutation, &consensus_binding, &consensus_limits, &proof)
                    .is_ok()
                {
                    return Err(
                        PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted,
                    );
                }
            }
            let mut changed_genesis_binding = consensus_binding.clone();
            changed_genesis_binding.genesis_hash[0] ^= 0x80;
            if verify_pq_masp_v1(
                &statement,
                &changed_genesis_binding,
                &consensus_limits,
                &proof,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::PublicStatementMutationAccepted);
            }
            (
                native_bound_statement_material_v1(
                    b"iroha.privacy.release.pq-masp.bound-statement.v1",
                    &original_typed,
                    &changed_genesis_binding,
                )?,
                PrivacyReleaseFailureClassV1::PublicStatementBindingRejected,
            )
        }
        PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation => {
            let invalid_keygen_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-fixture-keygen")?;
            let invalid_fixture_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-fixture-encryption")?;
            let mut invalid_fixture_rng = EvidenceRng09::new(invalid_fixture_seed);
            let invalid = pq_masp_release_invalid_path_fixture_v1(
                invalid_keygen_seed,
                &mut invalid_fixture_rng,
            )
            .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)?;
            let invalid_proof_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-proof")?;
            let mut invalid_proof_rng = EvidenceRng09::new(invalid_proof_seed);
            let invalid_consensus_binding = native_release_fixture_consensus_binding_v1(
                &invalid.statement.context,
                &consensus_limits,
            )?;
            if prove_pq_masp_v1_with_rng(
                &invalid.statement,
                &invalid_consensus_binding,
                &consensus_limits,
                &invalid.witness,
                invalid.authorization_secret_key.as_slice(),
                &mut invalid_proof_rng,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::InvalidWitnessPathAccepted);
            }
            let mut corrupt_header = proof.clone();
            let first = corrupt_header
                .first_mut()
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *first ^= 0x80;
            if verify_pq_masp_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_header,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let mut corrupt_inner_header = proof.clone();
            let inner_header = corrupt_inner_header
                .get_mut(PQ_MASP_AUTHORIZATION_HEADER_BYTES_V1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *inner_header ^= 0x80;
            if verify_pq_masp_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_inner_header,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let mut corrupt_interior = proof.clone();
            let interior = corrupt_interior.len() / 2;
            let byte = corrupt_interior
                .get_mut(interior)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            *byte ^= 0x01;
            if verify_pq_masp_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &corrupt_interior,
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted);
            }
            let truncated_length = proof
                .len()
                .checked_sub(1)
                .ok_or(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?;
            if verify_pq_masp_v1(
                &statement,
                &consensus_binding,
                &consensus_limits,
                &proof[..truncated_length],
            )
            .is_ok()
            {
                return Err(PrivacyReleaseEvidenceErrorClassV1::ProofTruncationAccepted);
            }
            (
                original_material,
                PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected,
            )
        }
    };
    Ok(StageMaterialV1 {
        public_statement_material,
        proof_artifacts: single_proof_artifact_v1(
            proof,
            u64::try_from(PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1)
                .expect("closed PQ-MASP proof ceiling fits u64"),
        ),
        resources: PrivacyReleaseResourceFactsV1 {
            primary_units: u64::try_from(witness.inputs().len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            primary_ceiling: u64::try_from(PQ_MASP_INPUT_BOUND_V1)
                .expect("closed PQ-MASP input ceiling fits u64"),
            secondary_units: u64::try_from(witness.outputs().len())
                .map_err(|_| PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)?,
            secondary_ceiling: u64::try_from(PQ_MASP_OUTPUT_BOUND_V1)
                .expect("closed PQ-MASP output ceiling fits u64"),
            relation_depth: u64::try_from(PQ_MASP_TREE_DEPTH_V1)
                .expect("closed PQ-MASP tree depth fits u64"),
            relation_depth_ceiling: u64::try_from(PQ_MASP_TREE_DEPTH_V1)
                .expect("closed PQ-MASP tree depth fits u64"),
        },
        failure_class,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::{
        ivm_private_note::{
            IvmPrivateNoteRelationErrorV1, PrivateNoteRelationProfileV1,
            preflight_private_note_relation_with_profile_v1,
        },
        pq_masp::relation::{PqMaspRelationErrorV1, validate_pq_masp_relation_v1},
    };
    use iroha_data_model::privacy::PrivacyNativeConsensusBindingValidationErrorV1;

    fn assert_release_fixture_consensus_binding(context: &PrivacyStatementContextV1) {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let binding = native_release_fixture_consensus_binding_v1(context, &limits)
            .expect("the producer must bind the fixture's original network");
        assert_eq!(binding.genesis_hash, *context.network_id.as_bytes());
        assert_eq!(binding.network_id, context.network_id);
        assert_eq!(binding.validate_against_context(context, &limits), Ok(()));
        let mut changed_genesis = binding;
        changed_genesis.genesis_hash[0] ^= 0x80;
        assert_eq!(
            changed_genesis.validate_against_context(context, &limits),
            Err(PrivacyNativeConsensusBindingValidationErrorV1::NetworkGenesisMismatch)
        );
        let mut invalid_context = *context;
        invalid_context.action_index = limits.max_actions_per_transaction;
        assert_eq!(
            native_release_fixture_consensus_binding_v1(&invalid_context, &limits),
            Err(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
        );
    }

    #[test]
    fn ivm_release_fixtures_reach_consensus_and_relation_preflight_without_proving() {
        let protocol_id = PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1;
        for case_kind in [
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
            PrivacyReleaseCaseKindV1::MaximumShapeResource,
        ] {
            let fixture_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-fixture-encryption")
                    .expect("canonical fixture seed");
            let mut fixture_rng = EvidenceRng06::new(fixture_seed);
            let fixture = ivm_private_note_release_fixture_v1(
                case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource,
                &mut fixture_rng,
            )
            .expect("canonical release fixture");
            assert_release_fixture_consensus_binding(&fixture.statement.context);
            assert_eq!(
                preflight_private_note_relation_with_profile_v1(
                    &fixture.statement,
                    &fixture.witness,
                    PrivateNoteRelationProfileV1::IVM_PRIVATE_NOTE,
                ),
                Ok(())
            );
        }
        let fixture_seed = stage_purpose_seed_v1(
            protocol_id,
            PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation,
            b"invalid-path-fixture-encryption",
        )
        .expect("invalid-path fixture seed");
        let mut fixture_rng = EvidenceRng06::new(fixture_seed);
        let invalid = ivm_private_note_release_invalid_path_fixture_v1(&mut fixture_rng)
            .expect("invalid-path release fixture");
        assert_release_fixture_consensus_binding(&invalid.statement.context);
        assert_eq!(
            preflight_private_note_relation_with_profile_v1(
                &invalid.statement,
                &invalid.witness,
                PrivateNoteRelationProfileV1::IVM_PRIVATE_NOTE,
            ),
            Err(IvmPrivateNoteRelationErrorV1::Membership)
        );
    }

    #[test]
    fn pq_masp_release_fixtures_reach_consensus_and_relation_preflight_without_proving() {
        let protocol_id = PrivacyProtocolIdV1::PqMaspStarkV1;
        for case_kind in [
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
            PrivacyReleaseCaseKindV1::MaximumShapeResource,
        ] {
            let keygen_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-fixture-keygen")
                    .expect("canonical keygen seed");
            let fixture_seed =
                stage_purpose_seed_v1(protocol_id, case_kind, b"canonical-fixture-encryption")
                    .expect("canonical fixture seed");
            let mut fixture_rng = EvidenceRng09::new(fixture_seed);
            let fixture = pq_masp_release_fixture_v1(
                case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource,
                keygen_seed,
                &mut fixture_rng,
            )
            .expect("canonical release fixture");
            assert_release_fixture_consensus_binding(&fixture.statement.context);
            validate_pq_masp_relation_v1(&fixture.statement, &fixture.witness)
                .expect("canonical release relation");
        }
        let case_kind = PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation;
        let keygen_seed =
            stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-fixture-keygen")
                .expect("invalid-path keygen seed");
        let fixture_seed =
            stage_purpose_seed_v1(protocol_id, case_kind, b"invalid-path-fixture-encryption")
                .expect("invalid-path fixture seed");
        let mut fixture_rng = EvidenceRng09::new(fixture_seed);
        let invalid = pq_masp_release_invalid_path_fixture_v1(keygen_seed, &mut fixture_rng)
            .expect("invalid-path release fixture");
        assert_release_fixture_consensus_binding(&invalid.statement.context);
        assert!(matches!(
            validate_pq_masp_relation_v1(&invalid.statement, &invalid.witness),
            Err(PqMaspRelationErrorV1::Membership)
        ));
    }
}
