//! Original committed-native admission for genuine terminal and transported proofs.
//!
//! No decoded completion can select these keys or witnesses. This owner uses the actual
//! freshly selected Core, independently unsealed one-use/evidence originals and exact
//! retained candidate proofs. It creates no commit and cannot repair a missing irreversible
//! op7 admission in the Core mutation kernel. Both final parities are independently verified.

use super::super::super::terminal_authorization::{
    canonical_prepared_one_use_authorization_digest_v1, canonical_terminal_commit_binding_digest_v1,
};
use super::*;
use crate::kagemusha_sender_wire::{
    SenderCommandBodyV1, SenderCommandV1, SenderHardwareAuthorizationV1,
};
use crate::kagemusha_v1_state::{
    CommittedOutgoingCandidateV1, DurableOutgoingEnvelopeV1,
    KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1,
};
use iroha_data_model::kagemusha::{KagemushaPaymentV1, kagemusha_ciphertext_digest_v1};

/// Genuine constant-size final proof, after native terminal and wrapper verification.
/// A caller must still publish it through the original Core's exact durable finalization path.
pub enum KagemushaProductionTerminalProofV1 {
    /// Request-bound peer payment proof.
    Payment(KagemushaGeneratedPaymentProofV1),
    /// Chain-facing redemption proof.
    Redemption(KagemushaGeneratedRedemptionProofV1),
}

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Authenticate the sole production release from an actual committed native selection.
    /// No supplied certificate or public projection can create this selection.
    pub fn load_committed(
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        selection.recheck().map_err(owner_error)?;
        let owner = Self::from_selected_release(
            selection.authenticated_release().map_err(owner_error)?,
            profile,
            resolver,
        )?;
        owner.recheck_committed_selection(selection)?;
        Ok(owner)
    }

    /// Reauthenticate the full native committed selection and exact original release.
    pub fn recheck_committed_selection(
        &self,
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        selection.recheck().map_err(owner_error)?;
        let release = selection.authenticated_release().map_err(owner_error)?;
        self.require_release_binding(&release)?;
        selection.recheck().map_err(owner_error)
    }

    fn recheck_committed_source(
        &self,
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
        source: &dyn KagemushaNativeOutgoingWitnessSourceV1,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        self.recheck_committed_selection(selection)?;
        source
            .recheck_committed_originals(selection)
            .map_err(proving_error)?;
        self.recheck_committed_selection(selection)
    }

    /// Prove the complete ordered terminal SHA queue from the original hardware witness.
    /// No terminal Guard placeholder or fabricated candidate is accepted for planning.
    pub fn prove_outgoing_terminal_hash_claim(
        &self,
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
    ) -> Result<KagemushaGeneratedMintHashClaimV1, KagemushaArtifactGenerationErrorV1> {
        let source = native_outgoing_witness::installed_source().map_err(proving_error)?;
        self.recheck_committed_source(selection, source.as_ref())?;
        let eq = load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let ep = load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let mut consumption = WitnessConsumption::default();
        let source_result =
            source.with_borrowed_terminal_hash_witness(selection, &mut |witness, seed| {
                consumption.consume(|| {
                    self.recheck_committed_source(selection, source.as_ref())?;
                    self.validate_terminal_originals(
                        selection,
                        witness.public,
                        witness.private_transition,
                        witness.terminal_guard_relation,
                        witness.enabled_hardware_profiles,
                    )?;
                    let committed = selection.committed().map_err(owner_error)?;
                    validate_candidate_columns(
                        committed,
                        self.artifacts.recursion_artifacts(),
                        witness.eq.candidate_protocol,
                        witness.ep.candidate_protocol,
                        witness.eq.candidate_instances,
                        witness.ep.candidate_instances,
                    )?;
                    let claim = prove_kagemusha_terminal_authorization_hash_claim_v1(
                        &eq, &ep, witness, seed,
                    )?;
                    self.recheck_committed_source(selection, source.as_ref())?;
                    Ok(claim)
                })
            });
        drop(eq);
        drop(ep);
        source_result.map_err(proving_error)?;
        self.recheck_committed_source(selection, source.as_ref())?;
        consumption.finish()
    }

    /// Produce both genuine terminal parities, fold complete ancestry, then produce the
    /// sole transported CommitWrapper pair. Every output is independently reverified against
    /// the exact committed original before returning. No balance or journal mutation occurs.
    pub fn prove_outgoing_terminal(
        &self,
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
        hash_claim: &KagemushaGeneratedMintHashClaimV1,
    ) -> Result<KagemushaProductionTerminalProofV1, KagemushaArtifactGenerationErrorV1> {
        let source = native_outgoing_witness::installed_source().map_err(proving_error)?;
        self.recheck_committed_source(selection, source.as_ref())?;
        let mut consumption = WitnessConsumption::default();
        let source_result =
            source.with_borrowed_terminal_witness(selection, hash_claim, &mut |witness, seed| {
                consumption.consume(|| {
                    self.recheck_committed_source(selection, source.as_ref())?;
                    self.validate_terminal_originals(
                        selection,
                        &witness.public,
                        &witness.private_transition,
                        &witness.terminal_guard_relation,
                        &witness.enabled_hardware_profiles,
                    )?;
                    let committed = selection.committed().map_err(owner_error)?;
                    validate_candidate_columns(
                        committed,
                        self.artifacts.recursion_artifacts(),
                        witness.eq.candidate_protocol,
                        witness.ep.candidate_protocol,
                        witness.eq.candidate_instances,
                        witness.ep.candidate_instances,
                    )?;
                    let candidate = committed
                        .candidate
                        .recovery_view()
                        .map_err(owner_error)?
                        .candidate_proof;
                    if witness.eq.candidate_proof != candidate.eq_proof
                        || witness.ep.candidate_proof != candidate.ep_proof
                        || witness.eq.candidate_history.as_bytes().as_slice()
                            != candidate.eq_history
                        || witness.ep.candidate_history.as_bytes().as_slice()
                            != candidate.ep_history
                    {
                        return Err(proving_error(
                            "terminal witness substitutes original native candidate proof/history",
                        ));
                    }
                    let proof = self.prove_terminal_and_wrap(witness, seed)?;
                    let result = self.verify_final_output(committed, proof)?;
                    self.recheck_committed_source(selection, source.as_ref())?;
                    Ok(result)
                })
            });
        source_result.map_err(proving_error)?;
        self.recheck_committed_source(selection, source.as_ref())?;
        consumption.finish()
    }

    fn prove_terminal_and_wrap(
        &self,
        witness: KagemushaTerminalAuthorizationGenerationWitnessV1<'_>,
        seed: &KagemushaRecoverySeedV1,
    ) -> Result<KagemushaGeneratedCommitWrapperProofV1, KagemushaArtifactGenerationErrorV1> {
        let public = witness.public.clone();
        let (eq, ep) = self.load_terminal_keys()?;
        let eq_protocol = compile(
            &eq.parameters,
            &eq.verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_protocol = compile(
            &ep.parameters,
            &ep.verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let terminal = prove_kagemusha_terminal_authorization_v1(&eq, &ep, witness, seed)?;
        let eq_fold = fold_kagemusha_eq_accumulators_v1(
            &eq.parameters,
            &terminal.eq_current_accumulator,
            &terminal.eq_history,
            seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let ep_fold = fold_kagemusha_ep_accumulators_v1(
            &ep.parameters,
            &terminal.ep_current_accumulator,
            &terminal.ep_history,
            seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        // Terminal PKs are not retained alongside wrapper PKs. Protocols and public proof
        // material remain immutable, and are recursively checked by the wrapper relation.
        drop(eq);
        drop(ep);
        let eq_instances = vec![terminal.eq_public_instances];
        let ep_instances = vec![terminal.ep_public_instances];
        let (eq, ep) = self.load_wrapper_keys()?;
        prove_kagemusha_commit_wrapper_v1(
            &eq,
            &ep,
            KagemushaCommitWrapperGenerationWitnessV1 {
                public,
                enabled_hardware_profiles: self.enabled_hardware_profiles,
                eq: KagemushaCommitWrapperEqGenerationWitnessV1 {
                    terminal_authorization_protocol: &eq_protocol,
                    terminal_authorization_instances: &eq_instances,
                    terminal_authorization_proof: &terminal.eq_proof,
                    terminal_authorization_history: &terminal.eq_history,
                    terminal_authorization_history_fold_proof: eq_fold.proof(),
                    successor_history: eq_fold.successor(),
                },
                ep: KagemushaCommitWrapperEpGenerationWitnessV1 {
                    terminal_authorization_protocol: &ep_protocol,
                    terminal_authorization_instances: &ep_instances,
                    terminal_authorization_proof: &terminal.ep_proof,
                    terminal_authorization_history: &terminal.ep_history,
                    terminal_authorization_history_fold_proof: ep_fold.proof(),
                    successor_history: ep_fold.successor(),
                },
            },
            seed,
        )
    }

    fn verify_final_output(
        &self,
        committed: &CommittedOutgoingCandidateV1,
        proof: KagemushaGeneratedCommitWrapperProofV1,
    ) -> Result<KagemushaProductionTerminalProofV1, KagemushaArtifactGenerationErrorV1> {
        match committed.candidate.prepared.recovery_view() {
            PreparedOutgoingRecoveryViewV1::Send {
                output,
                encrypted_credit,
                ..
            } => {
                let generated = proof.into_payment()?;
                let payment = KagemushaPaymentV1 {
                    version: KAGEMUSHA_WIRE_VERSION_V1,
                    output: output.clone(),
                    encrypted_credit: encrypted_credit.to_vec(),
                    commit_certificate: committed.commit_certificate.clone(),
                    proof: generated.proof.clone(),
                };
                let checked = DurableOutgoingEnvelopeV1::finalize_payment(
                    committed.clone(),
                    payment,
                    Vec::new(),
                    self.artifacts.recursion_artifacts(),
                    &self.verifier,
                )
                .map_err(owner_error)?;
                drop(checked);
                Ok(KagemushaProductionTerminalProofV1::Payment(generated))
            }
            PreparedOutgoingRecoveryViewV1::Redemption { .. } => {
                let generated = proof.into_redemption()?;
                let checked = DurableOutgoingEnvelopeV1::finalize_redemption(
                    committed.clone(),
                    generated.proof.clone(),
                    Vec::new(),
                    self.artifacts.recursion_artifacts(),
                    &self.verifier,
                )
                .map_err(owner_error)?;
                drop(checked);
                Ok(KagemushaProductionTerminalProofV1::Redemption(generated))
            }
        }
    }

    fn validate_terminal_originals(
        &self,
        selection: &KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>,
        public: &KagemushaTerminalAuthorizationTerminalGenerationPublicV1,
        private: &KagemushaTerminalAuthorizationPrivateGenerationWitnessV1,
        guard: &KagemushaGuardBundleRelationWitnessV1,
        enabled_profiles: &[[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        self.recheck_committed_selection(selection)?;
        let committed = selection.committed().map_err(owner_error)?;
        let prepared = &committed.candidate.prepared;
        let output = committed.public_output().map_err(owner_error)?;
        let artifact_manifest = match prepared.recovery_view() {
            PreparedOutgoingRecoveryViewV1::Send { .. } => [0; 32],
            PreparedOutgoingRecoveryViewV1::Redemption {
                artifact_manifest_digest,
                ..
            } => *artifact_manifest_digest,
        };
        let expected_public = KagemushaTerminalAuthorizationTerminalGenerationPublicV1 {
            lifecycle: output.lifecycle,
            semantic_digest: output.semantic_digest,
            candidate_envelope_digest: output.candidate_envelope_digest,
            commit_certificate_digest: output.commit_certificate_digest,
            transition_nullifier: output.transition_nullifier,
            request_digest: output.request_digest,
            receiver_binding_digest: output.receiver_binding_digest,
            ciphertext_commitment: output.ciphertext_commitment,
            amount: output.amount,
            terminal_output_binding: output.terminal_output_binding,
            artifact_manifest_digest: artifact_manifest,
        };
        let (before, after) = prepared.private_state_link();
        let ordinary = selection
            .normalized_guard_statement()
            .map_err(owner_error)?;
        let enabled = self
            .release
            .enabled_profile(public.lifecycle.hardware_profile_id)
            .ok_or_else(|| proving_error("terminal profile absent from actual release"))?;
        let send_matches = match (prepared.recovery_view(), &private.send) {
            (
                PreparedOutgoingRecoveryViewV1::Send {
                    request,
                    output,
                    encrypted_credit,
                    ..
                },
                Some(send),
            ) => {
                &send.request == request
                    && &send.output == output
                    && send.encrypted_credit_digest
                        == kagemusha_ciphertext_digest_v1(encrypted_credit)
            }
            (PreparedOutgoingRecoveryViewV1::Redemption { .. }, None) => true,
            _ => false,
        };
        if public != &expected_public
            || enabled_profiles != &self.enabled_hardware_profiles
            || private.lifecycle != public.lifecycle
            || &private.predecessor != before
            || &private.successor != after
            || private.outbox_reservation != prepared.outbox_reservation
            || private.commit_certificate != committed.commit_certificate
            || private.terminal_payload_digest != prepared.semantic_digest().map_err(owner_error)?
            || private.outgoing_sealed_streams.as_ref()
                != Some(&[
                    prepared.sealed_transition_inputs.clone(),
                    prepared.sealed_recovery_seeds.clone(),
                ])
            || private.journal_revision_before != prepared.proof_statement.journal_revision_before
            || private.journal_revision_after != prepared.proof_statement.journal_revision_after
            || private.hardware_profile != enabled.hardware_profile
            || !send_matches
            || private.hardware_credential.credential_id
                != guard.predecessor_credential.credential_issuance_digest
            || private.hardware_credential.credential_id
                != guard.successor_credential.credential_issuance_digest
        {
            return Err(proving_error(
                "terminal private/public original differs from actual committed Core",
            ));
        }
        let original = selection.original_hardware_commit().map_err(owner_error)?;
        let command = SenderCommandV1::decode_canonical_exact(
            7,
            selection.operation_id().map_err(owner_error)?,
            &original.canonical_command,
        )
        .map_err(|_| proving_error("terminal original op7 command is invalid"))?;
        let SenderCommandBodyV1::Commit {
            hardware_authorization,
            ..
        } = &command.body
        else {
            return Err(proving_error("terminal original is not native Commit"));
        };
        let authorization =
            SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)
                .map_err(|_| proving_error("terminal original Core authorization is invalid"))?;
        let prepared_authorization = canonical_prepared_one_use_authorization_digest_v1(
            KagemushaOperationV1::from(public.lifecycle.operation_kind),
            private.one_use_hardware_authorization,
            before,
            private.journal_revision_before,
            private.authorization_counter_before,
        );
        require_native_one_use_binding(
            private.one_use_hardware_authorization,
            authorization.hardware_one_use_nonce,
            prepared_authorization,
            prepared.prepared_one_use_authorization_digest,
        )?;
        let sender_authorization = if private.send.is_some() {
            prepared_authorization
        } else {
            [0; 32]
        };
        // The retained State Guard reference is its frozen prepared instant. The original
        // later irreversible commit evidence has its own authenticated private time/window;
        // native Terminal validation binds that opening to the exact full certificate.
        // Terminal binding and send-only one-use binding change from the ordinary State Guard.
        // The terminal value is derived
        // from the original full certificate and native private evidence, not a projected hash.
        let candidate_proof = committed
            .candidate
            .recovery_view()
            .map_err(owner_error)?
            .candidate_proof;
        let internal_public = public
            .clone()
            .into_internal(
                candidate_proof.eq_deferred_audit,
                candidate_proof.ep_deferred_audit,
                self.artifacts
                    .recursion_artifacts()
                    .terminal_authorization_eq_protocol_digest,
                self.artifacts
                    .recursion_artifacts()
                    .terminal_authorization_ep_protocol_digest,
            )
            .map_err(proving_error)?;
        let internal_private = private.clone().into_internal();
        internal_private
            .validate_against(&internal_public)
            .map_err(proving_error)?;
        let terminal_digest = canonical_terminal_commit_binding_digest_v1(
            &internal_public,
            &internal_private,
            ordinary.prepared_transition_binding_digest,
            sender_authorization,
            ordinary.transition_intent_digest,
            ordinary.transition_effect_digest,
            ordinary.recovery_record_digest,
            ordinary.durable_inbox_effect_digest,
            ordinary.durable_outbox_effect_digest,
        )
        .map_err(proving_error)?;
        let mut expected_guard = ordinary;
        expected_guard.terminal_commit_binding_digest = terminal_digest;
        expected_guard.sender_one_time_authorization_digest = sender_authorization;
        if terminal_digest == [0; 32] || guard.statement != expected_guard {
            return Err(proving_error(
                "terminal Guard does not authenticate exact original postcommit binding",
            ));
        }
        guard.validate().map_err(proving_error)?;
        Ok(())
    }
}

fn require_native_one_use_binding(
    unsealed_nonce: [u8; 32],
    signed_original_nonce: [u8; 32],
    derived_authorization: [u8; 32],
    selected_authorization: [u8; 32],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if unsealed_nonce == [0; 32]
        || unsealed_nonce != signed_original_nonce
        || derived_authorization == [0; 32]
        || derived_authorization != selected_authorization
    {
        return Err(proving_error(
            "terminal one-use opening differs from signed original op7 and Prepared",
        ));
    }
    Ok(())
}

fn validate_candidate_columns(
    committed: &CommittedOutgoingCandidateV1,
    artifacts: KagemushaRecursionArtifactsV1,
    eq_protocol: &PlonkProtocol<EqAffine>,
    ep_protocol: &PlonkProtocol<EpAffine>,
    eq_instances: &[Vec<Fp>],
    ep_instances: &[Vec<Fq>],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    require_protocol(
        eq_protocol,
        KagemushaPastaParityV1::Eq,
        artifacts.eq_protocol_digest,
        true,
    )?;
    require_protocol(
        ep_protocol,
        KagemushaPastaParityV1::Ep,
        artifacts.ep_protocol_digest,
        true,
    )?;
    let proof = committed
        .candidate
        .recovery_view()
        .map_err(owner_error)?
        .candidate_proof;
    let public = committed
        .candidate
        .prepared
        .candidate_public_inputs(artifacts, proof)
        .map_err(proving_error)?;
    let mut eq = public
        .recursive_semantic_public_instances::<Fp>()
        .map_err(proving_error)?;
    let mut ep = public
        .recursive_semantic_public_instances::<Fq>()
        .map_err(proving_error)?;
    let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&proof.eq_history)
        .map_err(|e| proving_error(e.to_string()))?;
    let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&proof.ep_history)
        .map_err(|e| proving_error(e.to_string()))?;
    eq.extend(history_limbs::<Fp>(eq_history.as_bytes()));
    ep.extend(history_limbs::<Fq>(ep_history.as_bytes()));
    if eq_instances != [eq] || ep_instances != [ep] {
        return Err(proving_error(
            "terminal candidate public columns differ from original stored State proof",
        ));
    }
    Ok(())
}

fn history_limbs<F: KagemushaPoseidonFieldV1>(bytes: &[u8]) -> Vec<F> {
    bytes
        .chunks_exact(16)
        .map(|limb| {
            F::from_u128(u128::from_le_bytes(
                limb.try_into().expect("native history limb width"),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_nonce_requires_signed_original_and_prepared_opening_together() {
        require_native_one_use_binding([1; 32], [1; 32], [2; 32], [2; 32]).unwrap();
        // A correct prepared opening cannot repair substitution of the original signed nonce.
        assert!(require_native_one_use_binding([1; 32], [3; 32], [2; 32], [2; 32]).is_err());
        // The signed original nonce cannot repair a different selected preparation digest.
        assert!(require_native_one_use_binding([1; 32], [1; 32], [2; 32], [4; 32]).is_err());
        assert!(require_native_one_use_binding([0; 32], [0; 32], [2; 32], [2; 32]).is_err());
        assert!(require_native_one_use_binding([1; 32], [1; 32], [0; 32], [0; 32]).is_err());
    }
    #[test]
    fn candidate_history_limbs_preserve_original_byte_order_in_both_parities() {
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(&1u128.to_le_bytes());
        bytes[16..].copy_from_slice(&u128::MAX.to_le_bytes());
        assert_eq!(
            history_limbs::<Fp>(&bytes),
            vec![from_u128::<Fp>(1), from_u128::<Fp>(u128::MAX)]
        );
        assert_eq!(
            history_limbs::<Fq>(&bytes),
            vec![from_u128::<Fq>(1), from_u128::<Fq>(u128::MAX)]
        );
    }
}
