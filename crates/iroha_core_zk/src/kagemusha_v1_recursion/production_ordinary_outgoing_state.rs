//! Genuine ordinary Send/Redeem State production from the same captured Native W2.
//! Public recursive operands cannot lend a financial secret or replace the actual held
//! preparation/ciphertext originals. Actual release keys and full paired histories remain required.
use super::production_ordinary_guard::{decode_ordinary_originals, derive_cash_relation};
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaAuthenticatedOrdinaryCashCandidateV1,
    KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    KagemushaOrdinaryAppRecursiveSelectionWitnessV1,
    KagemushaOrdinaryRecursiveOuterParentWitnessV1, KagemushaOrdinaryRecursivePreparedOpeningV1,
    KagemushaPreparedIntentCommitmentsV1,
    ordinary_cash_candidate_verifier::{
        capture_ordinary_cash_state_checkpoint_v1, verify_ordinary_cash_candidate_v1,
    },
    ordinary_guard_verifier::OrdinaryGuardProofWireV1,
};
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1;
use iroha_data_model::kagemusha::*;
use sha2::Sha256;
use zeroize::Zeroize as _;

/// Public proof operands only, available exclusively to the Native-owned proof consumer.
pub(crate) type KagemushaOrdinaryOutgoingAuxiliaryConsumerV1<'a> = dyn for<'w> FnMut(
        KagemushaRecursiveStateGenerationWitnessV1<'w>,
        KagemushaOrdinaryRecursiveOuterParentWitnessV1<'w>,
    ) -> Result<(), String>
    + 'a;
/// No public constructor, C/JNI registration, accepting verifier or financial secret source.
pub(crate) trait KagemushaOrdinaryOutgoingAuxiliaryProofSourceV1: Send + Sync {
    fn with_borrowed_outgoing_auxiliaries(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
        hash_claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        consume: &mut KagemushaOrdinaryOutgoingAuxiliaryConsumerV1<'_>,
    ) -> Result<(), String>;
}
/// No decoder/Clone/constructor. Only actual generation followed by independent admission creates it.
pub(crate) struct GeneratedOrdinaryOutgoingCandidateOriginalsV1 {
    candidate: KagemushaAuthenticatedOrdinaryCashCandidateV1,
    generated: KagemushaGeneratedRecursiveStateProofV1,
}
impl GeneratedOrdinaryOutgoingCandidateOriginalsV1 {
    pub(crate) fn into_candidate(mut self) -> KagemushaAuthenticatedOrdinaryCashCandidateV1 {
        self.generated.eq_public_instances.fill(Default::default());
        self.generated.ep_public_instances.fill(Default::default());
        self.generated
            .eq_transport_public_instances
            .fill(Default::default());
        self.generated
            .ep_transport_public_instances
            .fill(Default::default());
        self.generated.eq_inner_proof.zeroize();
        self.generated.ep_inner_proof.zeroize();
        self.candidate
    }
}
struct SelectedOriginals {
    credential: KagemushaOrdinaryAppCredentialV1,
    approval: KagemushaAppOperationApprovalV1,
    lease: Option<KagemushaPlayIntegrityRefreshLeaseV1>,
    guard: OrdinaryGuardProofWireV1,
    prepared: KagemushaOrdinaryPreparedOutgoingV1,
    transition: Vec<u8>,
    recovery: Vec<u8>,
}
impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    pub(crate) fn prove_ordinary_outgoing_state_hash_claim(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
        auxiliaries: &dyn KagemushaOrdinaryOutgoingAuxiliaryProofSourceV1,
    ) -> Result<KagemushaGeneratedMintHashClaimV1, KagemushaArtifactGenerationErrorV1> {
        let eq = load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let ep = load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        self.with_selected_outgoing_witness(selection, guard, None, auxiliaries, |w, seed| {
            prove_kagemusha_recursive_state_hash_claim_v1(&eq, &ep, w, seed)
        })
    }
    pub(crate) fn prove_ordinary_outgoing_state(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
        claim: &KagemushaGeneratedMintHashClaimV1,
        auxiliaries: &dyn KagemushaOrdinaryOutgoingAuxiliaryProofSourceV1,
    ) -> Result<GeneratedOrdinaryOutgoingCandidateOriginalsV1, KagemushaArtifactGenerationErrorV1>
    {
        let (eq, ep) = self.load_state_keys()?;
        let generated = self.with_selected_outgoing_witness(
            selection,
            guard,
            Some(claim),
            auxiliaries,
            |w, seed| prove_kagemusha_recursive_state_v1(&eq, &ep, w, seed),
        )?;
        drop(eq);
        drop(ep);
        self.recheck_ordinary_outgoing(selection, guard)?;
        let (prepared, _, _) = selection
            .retained_outgoing_proof_operands(guard)
            .map_err(owner_error)?;
        let checkpoint =
            capture_ordinary_cash_state_checkpoint_v1(selection, guard, prepared, &generated)
                .map_err(owner_error)?;
        let candidate = verify_ordinary_cash_candidate_v1(
            selection,
            guard,
            *prepared,
            generated.proof.clone(),
            &checkpoint,
        )
        .map_err(owner_error)?;
        candidate
            .recheck_preparation_selection(selection, guard)
            .map_err(owner_error)?;
        self.recheck_ordinary_outgoing(selection, guard)?;
        Ok(GeneratedOrdinaryOutgoingCandidateOriginalsV1 {
            candidate,
            generated,
        })
    }
    fn recheck_ordinary_outgoing(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        selection
            .recheck_selected_originals_and_current_custody()
            .map_err(owner_error)?;
        guard
            .recheck_preparation_selection(selection)
            .map_err(owner_error)?;
        self.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        selection
            .retained_outgoing_proof_operands(guard)
            .map_err(owner_error)?;
        Ok(())
    }
    fn selected_outgoing_originals(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    ) -> Result<SelectedOriginals, KagemushaArtifactGenerationErrorV1> {
        self.recheck_ordinary_outgoing(selection, guard)?;
        let (credential, approval, lease) = decode_ordinary_originals(
            selection.enrollment().app_credential().original(),
            selection.original(),
            selection
                .original_approval_integrity_lease()
                .map(|l| l.original()),
        )?;
        let wire: OrdinaryGuardProofWireV1 =
            norito::decode_canonical(guard.original()).map_err(ordinary_error)?;
        if norito::encode_canonical(&wire).map_err(ordinary_error)? != guard.original() {
            return Err(proving_error("outgoing Guard original is not canonical"));
        }
        let (prepared, transition, recovery) = selection
            .retained_outgoing_proof_operands(guard)
            .map_err(owner_error)?;
        Ok(SelectedOriginals {
            credential,
            approval,
            lease,
            guard: wire,
            prepared: *prepared,
            transition: transition.to_vec(),
            recovery: recovery.to_vec(),
        })
    }
    fn with_selected_outgoing_witness<T>(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
        claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        auxiliaries: &dyn KagemushaOrdinaryOutgoingAuxiliaryProofSourceV1,
        mut consume: impl FnMut(
            KagemushaRecursiveStateGenerationWitnessV1<'_>,
            &KagemushaRecoverySeedV1,
        ) -> Result<T, KagemushaArtifactGenerationErrorV1>,
    ) -> Result<T, KagemushaArtifactGenerationErrorV1> {
        let originals = self.selected_outgoing_originals(selection, guard)?;
        let mut result = None;
        let mut entered_financial = false;
        selection
            .with_borrowed_financial_secret(&mut |secret| {
                if entered_financial {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                entered_financial = true;
                let mut entered_aux = false;
                auxiliaries
                    .with_borrowed_outgoing_auxiliaries(
                        selection,
                        guard,
                        claim,
                        &mut |witness, outer| {
                            if entered_aux {
                                return Err("outgoing auxiliary source lent twice".into());
                            }
                            entered_aux = true;
                            result = Some((|| {
                                self.recheck_ordinary_outgoing(selection, guard)?;
                                let witness = self.bind_outgoing_witness(
                                    selection, guard, secret, witness, outer, &originals,
                                )?;
                                let seed = selected_outgoing_seed(secret, selection)?;
                                let value = consume(witness, &seed)?;
                                self.recheck_ordinary_outgoing(selection, guard)?;
                                Ok(value)
                            })());
                            Ok(())
                        },
                    )
                    .map_err(native_reject)?;
                Ok(())
            })
            .map_err(owner_error)?;
        self.recheck_ordinary_outgoing(selection, guard)?;
        result.ok_or_else(|| proving_error("outgoing complete witness was not lent"))?
    }
    fn bind_outgoing_witness<'a>(
        &'a self,
        selection: &'a KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
        secret: &[u8; 32],
        mut witness: KagemushaRecursiveStateGenerationWitnessV1<'a>,
        outer: KagemushaOrdinaryRecursiveOuterParentWitnessV1<'a>,
        originals: &'a SelectedOriginals,
    ) -> Result<KagemushaRecursiveStateGenerationWitnessV1<'a>, KagemushaArtifactGenerationErrorV1>
    {
        let relation = derive_cash_relation(selection, secret)?;
        let placeholders = witness.guard_relation.predecessor_device_authority_secret == [0; 32]
            && witness.guard_relation.successor_device_authority_secret == [0; 32];
        witness
            .guard_relation
            .predecessor_device_authority_secret
            .zeroize();
        witness
            .guard_relation
            .successor_device_authority_secret
            .zeroize();
        validate_outgoing_public_witness(self, selection, guard, &witness, outer, placeholders)?;
        let (prepared, transition, recovery) = selection
            .retained_outgoing_proof_operands(guard)
            .map_err(owner_error)?;
        if witness.guard_relation.statement != relation.0.statement
            || witness.guard_relation.predecessor_credential != relation.0.predecessor_credential
            || witness.guard_relation.successor_credential != relation.0.successor_credential
            || witness.guard_relation.canonical_empty_effect_digest
                != relation.0.canonical_empty_effect_digest
            || witness.eq_guard_proof != originals.guard.eq_proof
            || witness.ep_guard_proof != originals.guard.ep_proof
            || witness.eq_guard_history.as_bytes() != &originals.guard.eq_history
            || witness.ep_guard_history.as_bytes() != &originals.guard.ep_history
            || &originals.prepared != prepared
            || originals.transition != transition
            || originals.recovery != recovery
        {
            return Err(proving_error(
                "outgoing auxiliary originals differ from actual W2",
            ));
        }
        witness.guard_relation = relation.0.clone();
        witness.ordinary_selection = Some(KagemushaOrdinaryAppRecursiveSelectionWitnessV1 {
            credential: &originals.credential,
            approval: &originals.approval,
            integrity_lease: originals.lease.as_ref(),
            previous_app_attest_counter: selection.previous_app_attest_counter(),
            prepared: Some(KagemushaOrdinaryRecursivePreparedOpeningV1 {
                record: &originals.prepared,
                sealed_transition_inputs: &originals.transition,
                sealed_recovery_seeds: &originals.recovery,
            }),
            outer_parent: Some(outer),
            incoming_mint: None,
            incoming_receive: None,
        });
        witness.state.validate().map_err(proving_error)?;
        Ok(witness)
    }
}
/// Native-only production; no managed callback, raw State/proof/key/verdict can construct a candidate.
pub(crate) fn generate_ordinary_outgoing_candidate_v1<R: KagemushaArtifactByteResolverV1>(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    profile: KagemushaRecursiveVerifierProfileV1,
    resolver: R,
    auxiliaries: &dyn KagemushaOrdinaryOutgoingAuxiliaryProofSourceV1,
) -> Result<GeneratedOrdinaryOutgoingCandidateOriginalsV1, KagemushaArtifactGenerationErrorV1> {
    let owner = KagemushaProductionProverV1::load_ordinary_cash(selection, profile, resolver)?;
    let claim = owner.prove_ordinary_outgoing_state_hash_claim(selection, guard, auxiliaries)?;
    owner.prove_ordinary_outgoing_state(selection, guard, &claim, auxiliaries)
}
fn validate_outgoing_public_witness<R: KagemushaArtifactByteResolverV1>(
    owner: &KagemushaProductionProverV1<R>,
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    witness: &KagemushaRecursiveStateGenerationWitnessV1<'_>,
    outer: KagemushaOrdinaryRecursiveOuterParentWitnessV1<'_>,
    placeholders: bool,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    let before = selection.selected_predecessor_state();
    let after = selection.selected_successor_state();
    let t = selection.transition_statement();
    let (prepared, _, _) = selection
        .retained_outgoing_proof_operands(guard)
        .map_err(owner_error)?;
    let parent_original = selection
        .selected_predecessor_public_state_original()
        .map_err(owner_error)?;
    let s = &witness.state;
    let a = owner.artifacts.ordinary_recursion_artifacts()?;
    let (eq_outer, ep_outer) = super::super::super::ordinary_state_reserved::kagemusha_ordinary_state_outer_protocol_positions_v1(selection.recursive_verifier());
    if !placeholders
        || witness.hardware_selection.is_some()
        || witness.ordinary_selection.is_some()
        || witness.mint_fold_opening.is_some()
        || !matches!(
            s.operation,
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
        )
        || s.operation != KagemushaOperationV1::from(t.kind)
        || s.predecessor.as_ref() != Some(before)
        || &s.successor != after
        || s.amount != t.amount
        || s.journal_revision_before != t.journal_revision_before
        || s.journal_revision_after != t.journal_revision_after
        || s.transition_effect_digest != t.effect_digest
        || s.lifecycle_binding_digest != t.lifecycle_binding_digest
        || s.mint_finality_semantic_digest != t.mint_finality_semantic_digest
        || s.mint_finality_proof_binding_digest != t.mint_finality_proof_binding_digest
        || s.peer_credit_id != t.peer_credit_id
        || s.recipient_encryption_key_binding != t.recipient_encryption_key_binding
        || s.receive_credit_binding_digest != t.receive_credit_binding_digest
        || s.receive_credit.is_some()
        || s.replay_insert.is_some()
        || s.prepared_intent
            != Some(KagemushaPreparedIntentCommitmentsV1 {
                preparation_id: prepared.binding_digest().map_err(ordinary_error)?,
                sealed_transition_inputs_digest: prepared.stream_digests[0],
                sealed_recovery_seeds_digest: prepared.stream_digests[1],
            })
        || s.prepared_transition_binding_digest != t.prepared_transition_binding_digest
        || s.transport_semantic_digest != prepared.projection_semantic_digest
        || s.guard_statement_digest != guard.original_digests()[0]
        || s.eq_protocol_digest != a.eq_protocol_digest
        || s.ep_protocol_digest != a.ep_protocol_digest
        || s.guard_eq_protocol_digest != a.guard_bundle_eq_protocol_digest
        || s.guard_ep_protocol_digest != a.guard_bundle_ep_protocol_digest
        || s.mint_eq_protocol_digest != a.mint_finality_eq_protocol_digest
        || s.mint_ep_protocol_digest != a.mint_finality_ep_protocol_digest
        || s.mint_authorization_eq_protocol_digest != a.mint_authorization_eq_protocol_digest
        || s.mint_authorization_ep_protocol_digest != a.mint_authorization_ep_protocol_digest
        || s.commit_wrapper_eq_protocol_digest != a.commit_wrapper_eq_protocol_digest
        || s.commit_wrapper_ep_protocol_digest != a.commit_wrapper_ep_protocol_digest
        || s.guard_eq_credential_audit != eq_outer
        || s.guard_ep_credential_audit != ep_outer
        || outer.public_original != Some(parent_original.as_slice())
    {
        return Err(proving_error(
            "ordinary outgoing public source or exact selected transition differs",
        ));
    }
    let m = selection.recursive_verifier().state_checkpoint_material();
    let guard_material = selection
        .recursive_verifier()
        .ordinary_guard_verifier_material();
    let auxiliary_material = selection
        .recursive_verifier()
        .ordinary_bootstrap_auxiliary_material()
        .map_err(proving_error)?;
    for (actual, expected) in [
        (witness.eq_parent_protocol, m.inner_eq_protocol),
        (witness.eq_guard_protocol, guard_material.eq_protocol),
        (
            witness.eq_mint_authorization_protocol,
            auxiliary_material.eq_mint_authorization_protocol,
        ),
        (
            witness.eq_mint_protocol,
            auxiliary_material.eq_mint_protocol,
        ),
        (
            witness.eq_incoming_protocol,
            auxiliary_material.eq_incoming_protocol,
        ),
        (outer.eq_protocol, m.outer_eq_protocol),
    ] {
        if native_parent_protocol_digest_v1(actual, KagemushaPastaParityV1::Eq)
            .map_err(proving_error)?
            != native_parent_protocol_digest_v1(expected, KagemushaPastaParityV1::Eq)
                .map_err(proving_error)?
        {
            return Err(proving_error(
                "ordinary outgoing actual Eq protocol differs",
            ));
        }
    }
    for (actual, expected) in [
        (witness.ep_parent_protocol, m.inner_ep_protocol),
        (witness.ep_guard_protocol, guard_material.ep_protocol),
        (
            witness.ep_mint_authorization_protocol,
            auxiliary_material.ep_mint_authorization_protocol,
        ),
        (
            witness.ep_mint_protocol,
            auxiliary_material.ep_mint_protocol,
        ),
        (
            witness.ep_incoming_protocol,
            auxiliary_material.ep_incoming_protocol,
        ),
        (outer.ep_protocol, m.outer_ep_protocol),
    ] {
        if native_parent_protocol_digest_v1(actual, KagemushaPastaParityV1::Ep)
            .map_err(proving_error)?
            != native_parent_protocol_digest_v1(expected, KagemushaPastaParityV1::Ep)
                .map_err(proving_error)?
        {
            return Err(proving_error(
                "ordinary outgoing actual Ep protocol differs",
            ));
        }
    }
    if native_parent_protocol_digest_v1(witness.ep_parent_protocol, KagemushaPastaParityV1::Ep)
        .map_err(proving_error)?
        != native_parent_protocol_digest_v1(m.inner_ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(proving_error)?
        || native_parent_protocol_digest_v1(outer.eq_protocol, KagemushaPastaParityV1::Eq)
            .map_err(proving_error)?
            != a.eq_protocol_digest
        || native_parent_protocol_digest_v1(outer.ep_protocol, KagemushaPastaParityV1::Ep)
            .map_err(proving_error)?
            != a.ep_protocol_digest
    {
        return Err(proving_error(
            "ordinary outgoing exact parent protocols differ",
        ));
    }
    let mut entered = false;
    selection
        .with_retained_predecessor_checkpoint(&mut |checkpoint| {
            if entered {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            entered = true;
            require_parent_streams(
                [
                    &checkpoint.eq_inner_proof,
                    &checkpoint.ep_inner_proof,
                    checkpoint.eq_history.as_bytes(),
                    checkpoint.ep_history.as_bytes(),
                ],
                [
                    witness.eq_parent_proof,
                    witness.ep_parent_proof,
                    witness.eq_predecessor_history.as_bytes(),
                    witness.ep_predecessor_history.as_bytes(),
                ],
            )
            .map_err(native_reject)?;
            if witness.eq_parent_instances != [checkpoint.eq_public_instances.clone()]
                || witness.ep_parent_instances != [checkpoint.ep_public_instances.clone()]
                || outer.eq_instances != [checkpoint.eq_transport_public_instances.clone()]
                || outer.ep_instances != [checkpoint.ep_transport_public_instances.clone()]
                || outer.eq_proof != checkpoint.proof.eq_proof
                || outer.ep_proof != checkpoint.proof.ep_proof
                || outer.eq_history.as_bytes().as_slice() != checkpoint.proof.eq_history.as_slice()
                || outer.ep_history.as_bytes().as_slice() != checkpoint.proof.ep_history.as_slice()
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            Ok(())
        })
        .map_err(owner_error)?;
    if !entered {
        return Err(proving_error("actual predecessor checkpoint was not lent"));
    }
    Ok(())
}

fn require_parent_streams(actual: [&[u8]; 4], supplied: [&[u8]; 4]) -> Result<(), String> {
    if actual.iter().any(|b| b.is_empty()) || actual != supplied {
        return Err("ordinary outgoing inner predecessor proof/history originals differ".into());
    }
    Ok(())
}

fn selected_outgoing_seed(
    secret: &[u8; 32],
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
) -> Result<KagemushaRecoverySeedV1, KagemushaArtifactGenerationErrorV1> {
    let mut h = Sha256::new();
    h.update(b"iroha:kagemusha:v1:ordinary-outgoing-recovery-seed\0");
    h.update(secret);
    h.update(selection.challenge().operation_id);
    h.update(selection.challenge().nonce);
    h.update(Sha256::digest(selection.original()));
    h.update(
        selection
            .transition_statement()
            .digest()
            .map_err(owner_error)?,
    );
    KagemushaRecoverySeedV1::from_unsealed(h.finalize().into()).map_err(ordinary_error)
}
fn ordinary_error(e: impl core::fmt::Display) -> KagemushaArtifactGenerationErrorV1 {
    proving_error(e.to_string())
}
fn native_reject(_: impl core::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outgoing_parent_requires_exact_both_inner_proofs_and_complete_histories() {
        let originals = [vec![11; 17], vec![12; 23], vec![13; 544], vec![14; 544]];
        let actual = originals.each_ref().map(Vec::as_slice);
        assert!(require_parent_streams(actual, actual).is_ok());
        for index in 0..4 {
            let mut changed = originals.clone();
            changed[index][0] ^= 1;
            assert!(require_parent_streams(actual, changed.each_ref().map(Vec::as_slice)).is_err());
            changed[index].pop();
            assert!(require_parent_streams(actual, changed.each_ref().map(Vec::as_slice)).is_err());
        }
        assert!(require_parent_streams([&[], &[], &[], &[]], [&[], &[], &[], &[]]).is_err());
    }
}

/// Re-admit a durable public carrier only under the genuine captured W2 and private checkpoint.
/// Decoded carrier fields remain data; actual paired verification constructs the closed candidate.
pub(crate) fn readmit_retained_ordinary_outgoing_candidate_v1(
    selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    prepared: &KagemushaOrdinaryPreparedOutgoingV1,
    public_original: &[u8],
    private_checkpoint_original: &[u8],
) -> Result<KagemushaAuthenticatedOrdinaryCashCandidateV1, KagemushaStateErrorV1> {
    let carrier =
        crate::kagemusha_v1_recursion::KagemushaOrdinaryLineageStateOriginalV1::decode_original(
            public_original,
        )
        .map_err(native_reject)?;
    let candidate = verify_ordinary_cash_candidate_v1(
        selection,
        guard,
        *prepared,
        carrier.proof,
        private_checkpoint_original,
    )?;
    let canonical = crate::kagemusha_v1_recursion::KagemushaOrdinaryLineageStateOriginalV1::from_admitted_candidate(&candidate)
        .map_err(native_reject)?.canonical_bytes().map_err(native_reject)?;
    if canonical != public_original {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    candidate.recheck_preparation_selection(selection, guard)?;
    Ok(candidate)
}
