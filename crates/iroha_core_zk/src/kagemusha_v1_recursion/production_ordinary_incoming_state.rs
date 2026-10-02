//! Genuine ordinary Mint/Receive State production from the same captured Native W2.
//!
//! Auxiliary inputs are public proof/fold data. Their source cannot lend a secret, install a
//! verifier, authenticate a clock or admit a source. Every financial and plaintext opening is
//! replaced by a loan from the actual Main selection before the complete State relation runs.
//! The maintained State qualification refusal remains effective until real keys qualify it.

use super::super::super::{
    KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
    KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    KagemushaOrdinaryAppRecursiveSelectionWitnessV1, KagemushaOrdinaryCashOutgoingOriginalV1,
    KagemushaOrdinaryLineageStateOriginalV1, KagemushaOrdinaryRecursiveOuterParentWitnessV1,
    KagemushaOrdinaryRecursiveReceiveIncomingOpeningV1,
    ordinary_guard_verifier::OrdinaryGuardProofWireV1, verify_ordinary_incoming_candidate_v1,
};
use super::production_ordinary_guard::{decode_ordinary_originals, derive_incoming_relation};
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaReplayInsertWitnessV1, generation::KagemushaOrdinaryRecursiveMintIncomingOpeningV1,
};
use crate::kagemusha_v1_state::{
    DigestV1, KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1,
};
use iroha_data_model::kagemusha::*;
use sha2::{Digest as _, Sha256};
use zeroize::Zeroize as _;

/// Borrow public operands for one actual incoming State proof. This type carries no authority.
pub(crate) type KagemushaOrdinaryIncomingAuxiliaryConsumerV1<'a> = dyn for<'w> FnMut(
        KagemushaRecursiveStateGenerationWitnessV1<'w>,
        KagemushaOrdinaryRecursiveOuterParentWitnessV1<'w>,
    ) -> Result<(), String>
    + 'a;

/// Native-owned public proof source, independent of financial/key/time/source admission.
/// No C/JNI registration exists. A managed caller cannot offer this source or a verifier.
pub(crate) trait KagemushaOrdinaryIncomingAuxiliaryProofSourceV1: Send + Sync {
    /// Lend immutable release-pinned inner/outer predecessor, Guard, source and fold operands.
    /// All Guard financial-secret placeholders must be zero. The consumer replaces them and
    /// all credit openings with the actual selection's separate HRTB loans. Every current and
    /// history operand is recursively proved before an owned candidate can be constructed.
    fn with_borrowed_incoming_auxiliaries(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
        hash_claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        consume: &mut KagemushaOrdinaryIncomingAuxiliaryConsumerV1<'_>,
    ) -> Result<(), String>;
}

/// Real State result retaining its authenticated private checkpoint for the next recursion.
/// There is no decoder/Clone/public constructor and no implicit State or DATA effect.
pub(crate) struct GeneratedOrdinaryIncomingCandidateOriginalsV1 {
    candidate: KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
    generated: KagemushaGeneratedRecursiveStateProofV1,
    checkpoint_original: Vec<u8>,
}
impl GeneratedOrdinaryIncomingCandidateOriginalsV1 {
    pub(crate) fn candidate(&self) -> &KagemushaAuthenticatedOrdinaryIncomingCandidateV1 {
        &self.candidate
    }
    pub(crate) fn generated_state_proof(&self) -> &KagemushaGeneratedRecursiveStateProofV1 {
        &self.generated
    }
    pub(crate) fn private_checkpoint_original(&self) -> &[u8] {
        &self.checkpoint_original
    }
    pub(crate) fn into_parts(
        self,
    ) -> (
        KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
        KagemushaGeneratedRecursiveStateProofV1,
        Vec<u8>,
    ) {
        (self.candidate, self.generated, self.checkpoint_original)
    }
}

enum SourceOriginals {
    Mint {
        authorization: Box<KagemushaOrdinaryMintAuthorizationV1>,
        credit: Box<KagemushaMintCreditV1>,
    },
    Receive {
        outgoing: Box<KagemushaOrdinaryCashOutgoingOriginalV1>,
        receiver_credential: Box<KagemushaOrdinaryAppCredentialV1>,
        receiver_integrity: Option<Box<KagemushaPlayIntegrityRefreshLeaseV1>>,
        receiver_counter: Option<u32>,
    },
}
struct SelectedOriginals {
    credential: KagemushaOrdinaryAppCredentialV1,
    approval: KagemushaAppOperationApprovalV1,
    lease: Option<KagemushaPlayIntegrityRefreshLeaseV1>,
    guard: OrdinaryGuardProofWireV1,
    source: SourceOriginals,
}

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Prove the full ordered SHA plan for the exact incoming originals. No financial effect.
    pub(crate) fn prove_ordinary_incoming_state_hash_claim(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
        auxiliaries: &dyn KagemushaOrdinaryIncomingAuxiliaryProofSourceV1,
    ) -> Result<KagemushaGeneratedMintHashClaimV1, KagemushaArtifactGenerationErrorV1> {
        let eq = load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let ep = load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        self.with_selected_incoming_witness(selection, guard, None, auxiliaries, |w, seed| {
            prove_kagemusha_recursive_state_hash_claim_v1(&eq, &ep, w, seed)
        })
    }

    /// Prove both real State parities and re-admit the exact public checkpoint under Main.
    /// The returned private checkpoint retains the actual inner proofs and whole histories.
    pub(crate) fn prove_ordinary_incoming_state(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
        claim: &KagemushaGeneratedMintHashClaimV1,
        auxiliaries: &dyn KagemushaOrdinaryIncomingAuxiliaryProofSourceV1,
    ) -> Result<GeneratedOrdinaryIncomingCandidateOriginalsV1, KagemushaArtifactGenerationErrorV1>
    {
        let (eq, ep) = self.load_state_keys()?;
        let generated = self.with_selected_incoming_witness(
            selection,
            guard,
            Some(claim),
            auxiliaries,
            |w, seed| prove_kagemusha_recursive_state_v1(&eq, &ep, w, seed),
        )?;
        drop(eq);
        drop(ep);
        self.admit_generated_incoming(selection, guard, generated)
    }

    fn recheck_ordinary_incoming(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        selection
            .recheck_selected_originals_and_current_custody()
            .map_err(owner_error)?;
        guard
            .recheck_incoming_selection(selection)
            .map_err(owner_error)?;
        self.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        Ok(())
    }

    fn selected_incoming_originals(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    ) -> Result<SelectedOriginals, KagemushaArtifactGenerationErrorV1> {
        self.recheck_ordinary_incoming(selection, guard)?;
        let (credential, approval, lease) = decode_ordinary_originals(
            selection.credential().map_err(owner_error)?.original(),
            selection.original().map_err(owner_error)?,
            selection
                .selected_integrity_lease()
                .map_err(owner_error)?
                .map(|l| l.original()),
        )?;
        let wire = norito::decode_canonical(guard.original()).map_err(ordinary_error)?;
        let mut source = None;
        match selection.transition_statement().map_err(owner_error)?.kind {
            crate::kagemusha_v1_state::KagemushaTransitionKindV1::MintFold => {
                selection
                    .with_finalized_mint_source(&mut |actual, credit| {
                        if source.is_some() {
                            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                        }
                        actual.recheck_retained_custody().map_err(native_reject)?;
                        let reservation = selection.reservation()?;
                        let finalized = actual.finalized_original().map_err(native_reject)?;
                        let full_credit =
                            norito::encode_canonical(credit).map_err(native_reject)?;
                        if reservation.finalized_source_original_sha256
                            != <DigestV1>::from(Sha256::digest(finalized))
                            || reservation.source_proof_original_sha256
                                != <DigestV1>::from(Sha256::digest(&full_credit))
                            || reservation.source_semantic_digest
                                != actual.source_semantic_digest().map_err(native_reject)?
                            || credit.statement
                                != *actual.credit_statement().map_err(native_reject)?
                        {
                            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                        }
                        source = Some(SourceOriginals::Mint {
                            authorization: Box::new(
                                actual
                                    .authorization()
                                    .map_err(native_reject)?
                                    .authorization()
                                    .clone(),
                            ),
                            credit: Box::new(credit.clone()),
                        });
                        actual.recheck_retained_custody().map_err(native_reject)
                    })
                    .map_err(owner_error)?;
            }
            crate::kagemusha_v1_state::KagemushaTransitionKindV1::ReceiveFold => {
                selection
                    .with_received_source_and_request(&mut |actual, request| {
                        if source.is_some() {
                            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                        }
                        request.recheck_historical_custody()?;
                        let reservation = selection.reservation()?;
                        let raw = actual.outgoing_original();
                        if actual.request_original() != request.request_original()?
                            || reservation.source_proof_original_sha256
                                != <DigestV1>::from(Sha256::digest(raw))
                            || reservation.finalized_source_original_sha256
                                != actual.received_assertion_original_sha256()
                            || reservation.source_semantic_digest
                                != actual.output().binding_digest().map_err(native_reject)?
                        {
                            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                        }
                        let receiver = request.enrollment()?.app_credential();
                        let decoded =
                            norito::decode_canonical(receiver.original()).map_err(native_reject)?;
                        let integrity = request
                            .selected_integrity_lease()?
                            .map(|l| norito::decode_canonical(l.original()).map_err(native_reject))
                            .transpose()?;
                        source = Some(SourceOriginals::Receive {
                            outgoing: Box::new(
                                KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(raw)
                                    .map_err(native_reject)?,
                            ),
                            receiver_credential: Box::new(decoded),
                            receiver_integrity: integrity.map(Box::new),
                            receiver_counter: request.previous_app_attest_counter()?,
                        });
                        request.recheck_historical_custody()
                    })
                    .map_err(owner_error)?;
            }
            _ => {
                return Err(proving_error(
                    "ordinary incoming producer refuses another operation",
                ));
            }
        }
        self.recheck_ordinary_incoming(selection, guard)?;
        Ok(SelectedOriginals {
            credential,
            approval,
            lease,
            guard: wire,
            source: source.ok_or_else(|| proving_error("actual incoming source was not lent"))?,
        })
    }

    fn with_selected_incoming_witness<T>(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
        claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        auxiliaries: &dyn KagemushaOrdinaryIncomingAuxiliaryProofSourceV1,
        mut consume: impl FnMut(
            KagemushaRecursiveStateGenerationWitnessV1<'_>,
            &KagemushaRecoverySeedV1,
        ) -> Result<T, KagemushaArtifactGenerationErrorV1>,
    ) -> Result<T, KagemushaArtifactGenerationErrorV1> {
        let originals = self.selected_incoming_originals(selection, guard)?;
        let mut result = None;
        let mut entered_financial = false;
        selection
            .with_borrowed_financial_secret(&mut |secret| {
                if entered_financial {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                entered_financial = true;
                let mut entered_opening = false;
                selection.with_borrowed_credit_opening(&mut |opening| {
                    if entered_opening {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                    entered_opening = true;
                    let mut entered_aux = false;
                    auxiliaries
                        .with_borrowed_incoming_auxiliaries(
                            selection,
                            guard,
                            claim,
                            &mut |witness, outer| {
                                if entered_aux {
                                    return Err(
                                        "ordinary incoming auxiliary source lent twice".into()
                                    );
                                }
                                entered_aux = true;
                                result = Some((|| {
                                    self.recheck_ordinary_incoming(selection, guard)?;
                                    let witness = self.bind_incoming_witness(
                                        selection, guard, secret, opening, witness, outer,
                                        &originals,
                                    )?;
                                    let seed = selected_incoming_seed(secret, selection)?;
                                    let value = consume(witness, &seed)?;
                                    self.recheck_ordinary_incoming(selection, guard)?;
                                    Ok(value)
                                })());
                                Ok(())
                            },
                        )
                        .map_err(native_reject)?;
                    Ok(())
                })
            })
            .map_err(owner_error)?;
        self.recheck_ordinary_incoming(selection, guard)?;
        result.ok_or_else(|| proving_error("ordinary incoming complete witness was not lent"))?
    }

    fn bind_incoming_witness<'a, 'owner: 'a>(
        &'a self,
        selection: &'a KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'owner>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
        secret: &[u8; 32],
        opening: &'a KagemushaCreditOpeningV1,
        mut witness: KagemushaRecursiveStateGenerationWitnessV1<'a>,
        outer: KagemushaOrdinaryRecursiveOuterParentWitnessV1<'a>,
        originals: &'a SelectedOriginals,
    ) -> Result<KagemushaRecursiveStateGenerationWitnessV1<'a>, KagemushaArtifactGenerationErrorV1>
    {
        let relation = derive_incoming_relation(selection, secret)?;
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
        validate_incoming_public_witness(self, selection, guard, &witness, outer, placeholders)?;
        if witness.guard_relation.statement != relation.0.statement
            || witness.guard_relation.predecessor_credential != relation.0.predecessor_credential
            || witness.guard_relation.successor_credential != relation.0.successor_credential
            || witness.guard_relation.canonical_empty_effect_digest
                != relation.0.canonical_empty_effect_digest
            || witness.eq_guard_proof != originals.guard.eq_proof
            || witness.ep_guard_proof != originals.guard.ep_proof
            || witness.eq_guard_history.as_bytes() != &originals.guard.eq_history
            || witness.ep_guard_history.as_bytes() != &originals.guard.ep_history
        {
            return Err(proving_error(
                "ordinary incoming auxiliary Guard differs from actual selection",
            ));
        }
        let preparation = selection.preparation().map_err(owner_error)?;
        let reservation = selection.reservation().map_err(owner_error)?;
        opening
            .validate_shape_against(
                reservation.selection.credit_id,
                reservation.selection.amount,
            )
            .map_err(ordinary_error)?;
        let (mint, receive) = match &originals.source {
            SourceOriginals::Mint {
                authorization,
                credit,
            } => {
                if witness.mint_credit != credit.as_ref()
                    || witness.state.receive_credit.is_some()
                    || witness.state.replay_insert.as_ref()
                        != Some(&KagemushaReplayInsertWitnessV1::from(
                            selection.replay_witness().map_err(owner_error)?,
                        ))
                    || witness.eq_mint_authorization_proof != authorization.proof.eq_proof
                    || witness.ep_mint_authorization_proof != authorization.proof.ep_proof
                    || witness.eq_mint_authorization_history.as_bytes().as_slice()
                        != authorization.proof.eq_history.as_slice()
                    || witness.ep_mint_authorization_history.as_bytes().as_slice()
                        != authorization.proof.ep_history.as_slice()
                    || witness.eq_mint_proof != credit.proof.eq_proof
                    || witness.ep_mint_proof != credit.proof.ep_proof
                    || witness.eq_mint_history.as_bytes().as_slice()
                        != credit.proof.eq_history.as_slice()
                    || witness.ep_mint_history.as_bytes().as_slice()
                        != credit.proof.ep_history.as_slice()
                {
                    return Err(proving_error(
                        "ordinary incoming auxiliary Mint originals differ",
                    ));
                }
                (
                    Some(KagemushaOrdinaryRecursiveMintIncomingOpeningV1 {
                        authorization,
                        reservation,
                        preparation,
                        credit_opening: opening,
                    }),
                    None,
                )
            }
            SourceOriginals::Receive {
                outgoing,
                receiver_credential,
                receiver_integrity,
                receiver_counter,
            } => {
                // The supplied DTO opening never substitutes for the actual retained-key AEAD loan.
                let credit = witness
                    .state
                    .receive_credit
                    .as_mut()
                    .ok_or_else(|| proving_error("ordinary Receive auxiliary credit absent"))?;
                if credit.credit_id != opening.credit_id
                    || credit.amount != opening.amount
                    || credit.replay_insert
                        != KagemushaReplayInsertWitnessV1::from(
                            selection.replay_witness().map_err(owner_error)?,
                        )
                    || witness.state.replay_insert.is_some()
                {
                    return Err(proving_error(
                        "ordinary Receive auxiliary replay/amount differs",
                    ));
                }
                credit.credit_opening.recipient_binding_opening.zeroize();
                credit.credit_opening.credit_commitment_opening.zeroize();
                credit.credit_opening.recovery_nonce.zeroize();
                credit.credit_opening = *opening;
                (
                    None,
                    Some(KagemushaOrdinaryRecursiveReceiveIncomingOpeningV1 {
                        outgoing,
                        reservation,
                        preparation,
                        receiver_credential,
                        receiver_integrity_lease: receiver_integrity.as_deref(),
                        previous_receiver_app_attest_counter: *receiver_counter,
                        credit_opening: opening,
                    }),
                )
            }
        };
        witness.guard_relation = relation.0.clone();
        witness.ordinary_selection = Some(KagemushaOrdinaryAppRecursiveSelectionWitnessV1 {
            credential: &originals.credential,
            approval: &originals.approval,
            integrity_lease: originals.lease.as_ref(),
            previous_app_attest_counter: selection
                .previous_app_attest_counter()
                .map_err(owner_error)?,
            prepared: None,
            outer_parent: Some(outer),
            incoming_mint: mint,
            incoming_receive: receive,
        });
        witness.state.validate().map_err(proving_error)?;
        Ok(witness)
    }

    fn admit_generated_incoming(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
        generated: KagemushaGeneratedRecursiveStateProofV1,
    ) -> Result<GeneratedOrdinaryIncomingCandidateOriginalsV1, KagemushaArtifactGenerationErrorV1>
    {
        self.recheck_ordinary_incoming(selection, guard)?;
        let width = super::super::super::state_relation::RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT;
        let eq = generated
            .eq_transport_public_instances
            .get(..width)
            .ok_or_else(|| proving_error("generated incoming Eq column absent"))?
            .to_vec();
        let ep = generated
            .ep_transport_public_instances
            .get(..width)
            .ok_or_else(|| proving_error("generated incoming Ep column absent"))?
            .to_vec();
        let original = KagemushaOrdinaryLineageStateOriginalV1 {
            version: 1,
            projection:
                super::super::super::KagemushaOrdinaryLineageStateProjectionV1::from_fields(eq, ep)
                    .map_err(proving_error)?,
            proof: generated.proof.clone(),
        }
        .canonical_bytes()
        .map_err(proving_error)?;
        let checkpoint_original = super::super::super::ordinary_incoming_candidate_verifier::capture_ordinary_incoming_state_checkpoint_v1(
            selection.recursive_verifier(), selection, &original, guard, &generated,
        ).map_err(owner_error)?;
        let candidate = verify_ordinary_incoming_candidate_v1(
            selection.recursive_verifier(),
            selection,
            &original,
            guard,
            &checkpoint_original,
        )
        .map_err(owner_error)?;
        candidate
            .recheck_incoming_selection(selection, guard)
            .map_err(owner_error)?;
        self.recheck_ordinary_incoming(selection, guard)?;
        Ok(GeneratedOrdinaryIncomingCandidateOriginalsV1 {
            candidate,
            generated,
            checkpoint_original,
        })
    }
}

/// Native-only factory; profile/resolver supply signed artifact data, never a selected owner.
pub(crate) fn generate_ordinary_incoming_candidate_v1<R: KagemushaArtifactByteResolverV1>(
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    profile: KagemushaRecursiveVerifierProfileV1,
    resolver: R,
    auxiliaries: &dyn KagemushaOrdinaryIncomingAuxiliaryProofSourceV1,
) -> Result<GeneratedOrdinaryIncomingCandidateOriginalsV1, KagemushaArtifactGenerationErrorV1> {
    let owner = KagemushaProductionProverV1::load_ordinary_incoming(selection, profile, resolver)?;
    let claim = owner.prove_ordinary_incoming_state_hash_claim(selection, guard, auxiliaries)?;
    owner.prove_ordinary_incoming_state(selection, guard, &claim, auxiliaries)
}

fn validate_incoming_public_witness<R: KagemushaArtifactByteResolverV1>(
    owner: &KagemushaProductionProverV1<R>,
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    witness: &KagemushaRecursiveStateGenerationWitnessV1<'_>,
    outer: KagemushaOrdinaryRecursiveOuterParentWitnessV1<'_>,
    placeholders: bool,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    let before = selection
        .selected_predecessor_state()
        .map_err(owner_error)?;
    let after = selection.selected_successor_state().map_err(owner_error)?;
    let t = selection.transition_statement().map_err(owner_error)?;
    let s = &witness.state;
    let a = owner.artifacts.ordinary_recursion_artifacts()?;
    let (eq_outer, ep_outer) = super::super::super::ordinary_state_reserved::kagemusha_ordinary_state_outer_protocol_positions_v1(selection.recursive_verifier());
    if !placeholders
        || witness.hardware_selection.is_some()
        || witness.ordinary_selection.is_some()
        || witness.mint_fold_opening.is_some()
        || !matches!(
            s.operation,
            KagemushaOperationV1::MintFold | KagemushaOperationV1::ReceiveFold
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
        || s.prepared_intent.is_some()
        || s.prepared_transition_binding_digest != [0; 32]
        || s.transport_semantic_digest
            != selection.transport_semantic_digest().map_err(owner_error)?
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
        || outer.public_original
            != Some(
                selection
                    .predecessor_public_state_original()
                    .map_err(owner_error)?,
            )
    {
        return Err(proving_error(
            "ordinary incoming public source or exact selected transition differs",
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
                "ordinary incoming actual Eq protocol differs",
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
                "ordinary incoming actual Ep protocol differs",
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
            "ordinary incoming exact parent protocols differ",
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
        return Err("ordinary incoming inner predecessor proof/history originals differ".into());
    }
    Ok(())
}
fn selected_incoming_seed(
    secret: &[u8; 32],
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
) -> Result<KagemushaRecoverySeedV1, KagemushaArtifactGenerationErrorV1> {
    let mut h = Sha256::new();
    h.update(b"iroha:kagemusha:v1:ordinary-incoming-state-native-recovery-seed\0");
    h.update(secret);
    h.update(selection.challenge().map_err(owner_error)?.operation_id);
    h.update(selection.challenge().map_err(owner_error)?.nonce);
    h.update(
        selection
            .authorization_binding_digest()
            .map_err(owner_error)?,
    );
    h.update(
        selection
            .reservation()
            .map_err(owner_error)?
            .digest()
            .map_err(ordinary_error)?,
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
    fn incoming_parent_requires_exact_both_inner_proofs_and_whole_histories() {
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
