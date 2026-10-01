//! Genuine incoming State proving under the exclusive native fold and original physical source.
//! Public credit selectors, supplied proof pairs and decoded previews cannot create admission.
use super::*;
use crate::{
    kagemusha_v1_recursion::{KagemushaReceiveFoldCreditV1, KagemushaReplayInsertWitnessV1},
    kagemusha_v1_state::KagemushaAuthenticatedIncomingProvingSelectionV1,
};
use iroha_data_model::kagemusha::KagemushaPaymentProofV1;

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Admit exactly the production release retained by the original exclusive incoming owner.
    pub fn load_incoming(
        selection: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        selection.recheck().map_err(owner_error)?;
        let owner = Self::from_selected_release(
            selection.authenticated_release().map_err(owner_error)?,
            profile,
            resolver,
        )?;
        owner.recheck_incoming_selection(selection)?;
        Ok(owner)
    }

    /// Recheck the same full original native owner; this grants no hardware or publication lease.
    pub fn recheck_incoming_selection(
        &self,
        selection: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        selection.recheck().map_err(owner_error)?;
        let release = selection.authenticated_release().map_err(owner_error)?;
        self.require_release_binding(&release)?;
        selection.recheck().map_err(owner_error)
    }

    fn recheck_incoming_source(
        &self,
        selection: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
        source: &dyn KagemushaNativeOutgoingWitnessSourceV1,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        self.recheck_incoming_selection(selection)?;
        source
            .recheck_incoming_originals(selection)
            .map_err(proving_error)?;
        self.recheck_incoming_selection(selection)
    }

    /// Prove the exact incoming transition's ordered SHA claim from original unsealed material.
    pub fn prove_incoming_state_hash_claim(
        &self,
        selection: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
    ) -> Result<KagemushaGeneratedMintHashClaimV1, KagemushaArtifactGenerationErrorV1> {
        let source = native_outgoing_witness::installed_source().map_err(proving_error)?;
        self.recheck_incoming_source(selection, source.as_ref())?;
        let eq = load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let ep = load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let mut consumption = WitnessConsumption::default();
        let result =
            source.with_borrowed_incoming_witness(selection, None, &mut |witness, seed| {
                consumption.consume(|| {
                    self.recheck_incoming_source(selection, source.as_ref())?;
                    validate_incoming_witness(
                        selection,
                        &witness,
                        self.artifacts.recursion_artifacts(),
                    )?;
                    let claim =
                        prove_kagemusha_recursive_state_hash_claim_v1(&eq, &ep, witness, seed)?;
                    self.recheck_incoming_source(selection, source.as_ref())?;
                    Ok(claim)
                })
            });
        drop(eq);
        drop(ep);
        result.map_err(proving_error)?;
        self.recheck_incoming_source(selection, source.as_ref())?;
        consumption.finish()
    }

    /// Produce and independently reverify both genuine State parities for this original fold.
    /// No balance mutation, history CAS, hardware certificate or root signature is manufactured.
    pub fn prove_incoming_state(
        &self,
        selection: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
        hash_claim: &KagemushaGeneratedMintHashClaimV1,
    ) -> Result<KagemushaGeneratedRecursiveStateProofV1, KagemushaArtifactGenerationErrorV1> {
        let source = native_outgoing_witness::installed_source().map_err(proving_error)?;
        self.recheck_incoming_source(selection, source.as_ref())?;
        let (eq, ep) = self.load_state_keys()?;
        let mut consumption = WitnessConsumption::default();
        let result = source.with_borrowed_incoming_witness(
            selection,
            Some(hash_claim),
            &mut |witness, seed| {
                consumption.consume(|| {
                    self.recheck_incoming_source(selection, source.as_ref())?;
                    validate_incoming_witness(
                        selection,
                        &witness,
                        self.artifacts.recursion_artifacts(),
                    )?;
                    let generated = prove_kagemusha_recursive_state_v1(&eq, &ep, witness, seed)?;
                    selection
                        .authenticate_pair(&generated.proof)
                        .map_err(owner_error)?;
                    self.recheck_incoming_source(selection, source.as_ref())?;
                    Ok(generated)
                })
            },
        );
        drop(eq);
        drop(ep);
        result.map_err(proving_error)?;
        self.recheck_incoming_source(selection, source.as_ref())?;
        consumption.finish()
    }
}

fn require_incoming_kind(
    operation: KagemushaOperationV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if !matches!(
        operation,
        KagemushaOperationV1::MintFold | KagemushaOperationV1::ReceiveFold
    ) {
        return Err(proving_error(
            "incoming prover refuses another transition kind",
        ));
    }
    Ok(())
}

fn validate_incoming_witness(
    selection: &KagemushaAuthenticatedIncomingProvingSelectionV1<'_>,
    witness: &KagemushaRecursiveStateGenerationWitnessV1<'_>,
    artifacts: KagemushaRecursionArtifactsV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    selection.recheck().map_err(owner_error)?;
    let preview = selection.transition().map_err(owner_error)?;
    let (before, after) = selection.private_state_link().map_err(owner_error)?;
    let state = &witness.state;
    let guard = &witness.guard_relation;
    let statement = &preview.proof_statement;
    require_incoming_kind(state.operation)?;
    state.validate().map_err(proving_error)?;
    guard.validate().map_err(proving_error)?;
    if state.predecessor.as_ref() != Some(before)
        || &state.successor != after
        || state.operation != KagemushaOperationV1::from(statement.kind)
        || state.amount != statement.amount
        || state.journal_revision_before != statement.journal_revision_before
        || state.journal_revision_after != statement.journal_revision_after
        || state.transition_effect_digest != statement.effect_digest
        || state.mint_finality_semantic_digest != statement.mint_finality_semantic_digest
        || state.mint_finality_proof_binding_digest != statement.mint_finality_proof_binding_digest
        || state.peer_credit_id != statement.peer_credit_id
        || state.recipient_encryption_key_binding != statement.recipient_encryption_key_binding
        || state.lifecycle_binding_digest != statement.lifecycle_binding_digest
        || state.prepared_transition_binding_digest != statement.prepared_transition_binding_digest
        || state.prepared_intent.is_some()
        || state.receive_credit_binding_digest != statement.receive_credit_binding_digest
        || state.transport_semantic_digest != preview.transport_semantic_digest
        || guard.statement != preview.normalized_guard_statement
        || state.guard_statement_digest
            != preview.hardware_statement.normalized_guard_statement_digest
        || guard.statement_digest() != state.guard_statement_digest
        || guard.canonical_empty_effect_digest != artifacts.canonical_empty_effect_digest
        || state.eq_protocol_digest != artifacts.eq_protocol_digest
        || state.ep_protocol_digest != artifacts.ep_protocol_digest
        || state.guard_eq_protocol_digest
            != artifacts
                .guard_bundle_protocol_digest(KagemushaPastaParityV1::Eq)
                .map_err(|e| proving_error(e.to_string()))?
        || state.guard_ep_protocol_digest
            != artifacts
                .guard_bundle_protocol_digest(KagemushaPastaParityV1::Ep)
                .map_err(|e| proving_error(e.to_string()))?
        || state.mint_eq_protocol_digest
            != artifacts
                .mint_finality_protocol_digest(KagemushaPastaParityV1::Eq)
                .map_err(|e| proving_error(e.to_string()))?
        || state.mint_ep_protocol_digest
            != artifacts
                .mint_finality_protocol_digest(KagemushaPastaParityV1::Ep)
                .map_err(|e| proving_error(e.to_string()))?
        || state.mint_authorization_eq_protocol_digest
            != artifacts.mint_authorization_eq_protocol_digest
        || state.mint_authorization_ep_protocol_digest
            != artifacts.mint_authorization_ep_protocol_digest
        || state.commit_wrapper_eq_protocol_digest != artifacts.commit_wrapper_eq_protocol_digest
        || state.commit_wrapper_ep_protocol_digest != artifacts.commit_wrapper_ep_protocol_digest
    {
        return Err(proving_error(
            "incoming unsealed witness differs from original native fold",
        ));
    }
    let replay =
        KagemushaReplayInsertWitnessV1::from(selection.replay_insert().map_err(owner_error)?);
    match state.operation {
        KagemushaOperationV1::MintFold => {
            let selected = selection
                .mint_fold_opening()
                .map_err(owner_error)?
                .ok_or_else(|| proving_error("native mint opening is absent"))?
                .opening();
            let supplied = witness
                .mint_fold_opening
                .ok_or_else(|| proving_error("unsealed mint opening is absent"))?
                .opening();
            if state.replay_insert.as_ref() != Some(&replay)
                || state.receive_credit.is_some()
                || witness.mint_authorization != selected.authorization()
                || witness.mint_credit != selected.credit()
                || supplied.authorization() != selected.authorization()
                || supplied.credit() != selected.credit()
                || supplied.recipient_credential() != selected.recipient_credential()
                || supplied.credit_opening() != selected.credit_opening()
            {
                return Err(proving_error(
                    "mint witness differs from original authenticated staging",
                ));
            }
        }
        KagemushaOperationV1::ReceiveFold => {
            let (_, payment, original) = selection
                .peer_originals()
                .map_err(owner_error)?
                .ok_or_else(|| proving_error("original staged peer credit is absent"))?;
            let expected = KagemushaReceiveFoldCreditV1 {
                amount: original.amount,
                credit_id: original.credit_id.0,
                recipient_lane_id: original.recipient_lane_id,
                incoming_proof_binding_digest: original.incoming_proof_binding_digest,
                request_digest: original.request_digest,
                prepared_transfer_digest: original.prepared_transfer_digest,
                transition_nullifier: original.transition_nullifier,
                recipient_encryption_key: original.recipient_encryption_key,
                ciphertext_commitment: original.ciphertext_commitment,
                credit_opening: original.credit_opening,
                receiver_binding_digest: original.receiver_binding_digest,
                payment_output_digest: original.payment_output_digest,
                replay_insert: replay,
            };
            if state.receive_credit.as_ref() != Some(&expected)
                || state.replay_insert.is_some()
                || witness.mint_fold_opening.is_some()
            {
                return Err(proving_error(
                    "receive witness differs from original authenticated staging",
                ));
            }
            require_original_pair_bytes(
                &payment.proof,
                witness.eq_incoming_credits[0].proof,
                witness.ep_incoming_credits[0].proof,
                witness.eq_incoming_credits[0].history.as_bytes(),
                witness.ep_incoming_credits[0].history.as_bytes(),
            )?;
        }
        _ => return Err(proving_error("incoming kind is not admitted")),
    }
    selection.recheck().map_err(owner_error)
}

fn require_original_pair_bytes(
    original: &KagemushaPaymentProofV1,
    eq_proof: &[u8],
    ep_proof: &[u8],
    eq_history: &[u8],
    ep_history: &[u8],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    require_original_pair_streams(
        [
            &original.eq_proof,
            &original.ep_proof,
            &original.eq_history,
            &original.ep_history,
        ],
        [eq_proof, ep_proof, eq_history, ep_history],
    )
}

// Exact byte correlation only, never proof authentication or a selected owner constructor.
fn require_original_pair_streams(
    original: [&[u8]; 4],
    supplied: [&[u8]; 4],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if original.iter().any(|bytes| bytes.is_empty()) || original != supplied {
        return Err(proving_error(
            "received proof/history differs from the exact native payment",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incoming_kind_is_exact_and_cannot_be_outgoing_or_bootstrap() {
        assert!(require_incoming_kind(KagemushaOperationV1::MintFold).is_ok());
        assert!(require_incoming_kind(KagemushaOperationV1::ReceiveFold).is_ok());
        for operation in [
            KagemushaOperationV1::Bootstrap,
            KagemushaOperationV1::SendSplit,
            KagemushaOperationV1::RedeemSplit,
            KagemushaOperationV1::Rotate,
        ] {
            assert!(require_incoming_kind(operation).is_err());
        }
    }
    #[test]
    fn exact_received_pair_preserves_all_four_original_streams() {
        let original: [&[u8]; 4] = [&[1, 2], &[3, 4], &[5, 6], &[7, 8]];
        assert!(require_original_pair_streams(original, original).is_ok());
    }
    #[test]
    fn original_payment_proof_fields_bind_all_four_borrowed_streams() {
        let original = KagemushaPaymentProofV1 {
            version: 1,
            eq_protocol_digest: [1; 32],
            ep_protocol_digest: [2; 32],
            semantic_digest: [3; 32],
            candidate_envelope_digest: [4; 32],
            commit_certificate_digest: [5; 32],
            eq_deferred_audit: [6; 32],
            ep_deferred_audit: [7; 32],
            eq_proof: vec![1, 2],
            ep_proof: vec![3, 4],
            eq_history: vec![5, 6],
            ep_history: vec![7, 8],
        };
        let streams: [&[u8]; 4] = [
            &original.eq_proof,
            &original.ep_proof,
            &original.eq_history,
            &original.ep_history,
        ];
        assert!(
            require_original_pair_bytes(&original, streams[0], streams[1], streams[2], streams[3])
                .is_ok()
        );
        for index in 0..4 {
            let mut substituted = streams;
            substituted[index] = &[9, 10];
            assert!(
                require_original_pair_bytes(
                    &original,
                    substituted[0],
                    substituted[1],
                    substituted[2],
                    substituted[3]
                )
                .is_err()
            );
            substituted[index] = &[];
            assert!(
                require_original_pair_bytes(
                    &original,
                    substituted[0],
                    substituted[1],
                    substituted[2],
                    substituted[3]
                )
                .is_err()
            );
        }
    }
    #[test]
    fn each_received_proof_and_history_substitution_is_rejected() {
        let original: [&[u8]; 4] = [&[1, 2], &[3, 4], &[5, 6], &[7, 8]];
        for index in 0..4 {
            let mut changed = original;
            changed[index] = &[9, 10];
            assert!(require_original_pair_streams(original, changed).is_err());
            changed[index] = &[];
            assert!(require_original_pair_streams(original, changed).is_err());
        }
        assert!(
            require_original_pair_streams(
                original,
                [original[1], original[0], original[3], original[2]]
            )
            .is_err()
        );
    }
    #[test]
    fn empty_original_pair_streams_cannot_be_proving_material() {
        let complete: [&[u8]; 4] = [&[1], &[2], &[3], &[4]];
        for index in 0..4 {
            let mut empty = complete;
            empty[index] = &[];
            assert!(require_original_pair_streams(empty, empty).is_err());
        }
    }
}
