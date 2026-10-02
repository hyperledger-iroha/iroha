//! Genuine ordinary Native whole Terminal producer, using only captured descriptor-held loans.
//! Native funding, DATA reservation/commit and current FI effects remain separate owner gates.
#[path = "production_ordinary_cash_commit_assembler.rs"]
mod commit_assembler;
use super::super::super::{
    composite::ordinary_cash_terminal_math::{
        OrdinaryCashCandidateCompleteProofV1, OrdinaryCashTerminalHalfWitnessV1,
        OrdinaryCashTerminalOutgoingWitnessV1, OrdinaryCashTerminalSemanticWitnessV1,
    },
    ordinary_cash_commit_wrapper::{
        OrdinaryCashCommitWrapperHalfWitnessV1, OrdinaryCashCommitWrapperWitnessV1,
    },
    ordinary_cash_terminal_circuit::OrdinaryCashTerminalCircuitWitnessV1,
    ordinary_cash_terminal_verifier::{
        KagemushaAuthenticatedOrdinaryCashTerminalV1, decode_stateless_original_v1,
        selected_public_values_v1, verify_ordinary_cash_terminal_v1,
    },
    ordinary_guard_recursive_consumer::KagemushaOrdinaryGuardCompleteProofV1,
    ordinary_guard_verifier::{
        KagemushaAuthenticatedOrdinaryTerminalGuardV1, OrdinaryGuardProofWireV1, public_column,
    },
    ordinary_receiver_request_opening::OrdinaryReceiverRequestWitnessV1,
    ordinary_redeem_output_opening::OrdinaryRedeemOutputWitnessV1,
    state_relation::{KagemushaStateRelationPublicInputsV1, KagemushaStateRelationWitnessV1},
    typed_sha_consumer::KagemushaRecursiveHashClaimParityWitnessV1,
};
use super::super::ordinary_cash_terminal_generation::{
    prove_ordinary_cash_terminal_pair_v1, prove_ordinary_cash_terminal_sha_claim_v1,
    prove_ordinary_cash_wrapper_pair_v1,
};
use super::production_ordinary_guard::{
    decode_ordinary_originals, derive_cash_relation, derive_terminal_relation,
};
use super::*;
use crate::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, from_u128};
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1;
pub(crate) use commit_assembler::GeneratedOrdinaryCashCommitOriginalsV1;
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryAppCredentialV1, KagemushaPlayIntegrityRefreshLeaseV1,
};

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Complete real ordinary Send/Redeem proof from a genuine separately captured W1 selection.
    /// No raw mathematical witness, private credit opening, caller clock or proof callback enters.
    /// The resulting closed proof is still not a committed outbox/current financial grant.
    pub(crate) fn prove_ordinary_cash_terminal(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
        terminal_guard: &KagemushaAuthenticatedOrdinaryTerminalGuardV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryCashTerminalV1, KagemushaArtifactGenerationErrorV1>
    {
        selection
            .recheck_selected_originals_and_current_custody()
            .map_err(owner_error)?;
        terminal_guard
            .recheck_terminal_selection(selection)
            .map_err(owner_error)?;
        self.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        let mut entered = false;
        let mut generated = None;
        selection
            .with_borrowed_financial_secret(&mut |secret| {
                if entered {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                entered = true;
                generated =
                    Some(self.prove_selected_ordinary_terminal(selection, terminal_guard, secret));
                Ok(())
            })
            .map_err(owner_error)?;
        let (inner, wrapper, folds) = generated.ok_or_else(|| {
            proving_error("ordinary whole Terminal financial witness was not lent")
        })??;
        let result = verify_ordinary_cash_terminal_v1(
            selection,
            terminal_guard,
            &inner,
            &wrapper,
            [&folds[0], &folds[1]],
        )
        .map_err(owner_error)?;
        terminal_guard
            .recheck_terminal_selection(selection)
            .map_err(owner_error)?;
        selection
            .recheck_selected_originals_and_current_custody()
            .map_err(owner_error)?;
        Ok(result)
    }

    fn prove_selected_ordinary_terminal(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
        terminal_guard: &KagemushaAuthenticatedOrdinaryTerminalGuardV1,
        secret: &[u8; 32],
    ) -> Result<
        (
            Vec<u8>,
            Vec<u8>,
            [[u8; super::super::super::KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1]; 2],
        ),
        KagemushaArtifactGenerationErrorV1,
    > {
        let preparation = selection.preparation_selection().map_err(owner_error)?;
        selection
            .preparation_guard()
            .recheck_preparation_selection(&preparation)
            .map_err(owner_error)?;
        selection
            .candidate()
            .recheck_preparation_selection(&preparation, selection.preparation_guard())
            .map_err(owner_error)?;
        let (credential, w1, pi1) = decode_ordinary_originals(
            selection.enrollment().app_credential().original(),
            selection.original(),
            selection
                .original_approval_integrity_lease()
                .map(|p| p.original()),
        )?;
        let (_, w2, pi2) = decode_ordinary_originals(
            preparation.enrollment().app_credential().original(),
            preparation.original(),
            preparation
                .original_approval_integrity_lease()
                .map(|p| p.original()),
        )?;
        let state = outgoing_state_witness(selection.candidate().public_inputs())?;
        let preparation_relation = derive_cash_relation(&preparation, secret)?;
        let terminal_relation = derive_terminal_relation(selection, secret)?;
        let receiver_c = selection
            .send_transport_originals()
            .map(|(_, _, _, c, _)| {
                KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(c.original())
                    .map_err(proving_error)
            })
            .transpose()?;
        let receiver_pi = selection
            .send_receiver_integrity_lease_original()
            .map(|p| {
                let value: KagemushaPlayIntegrityRefreshLeaseV1 =
                    norito::decode_canonical(p.original())
                        .map_err(|e| proving_error(e.to_string()))?;
                if value.canonical_bytes().map_err(proving_error)? != p.original() {
                    return Err(proving_error("actual receiver PI original differs"));
                }
                Ok(value)
            })
            .transpose()?;
        let outgoing = match (
            selection.send_transport_originals(),
            selection.redeem_transport_originals(),
        ) {
            (Some((request, output, encrypted_credit, _, previous_app_attest_counter)), None) => {
                OrdinaryCashTerminalOutgoingWitnessV1::Send {
                    receiver: OrdinaryReceiverRequestWitnessV1 {
                        request,
                        credential: receiver_c
                            .as_ref()
                            .ok_or_else(|| proving_error("actual receiver C was not lent"))?,
                        integrity_lease: receiver_pi.as_ref(),
                        previous_app_attest_counter,
                        enabled: true,
                    },
                    output,
                    encrypted_credit,
                    credit_opening: selection.send_credit_opening().ok_or_else(|| {
                        proving_error("actual retained credit opening was not lent")
                    })?,
                }
            }
            (None, Some((output, beneficiary, manifest_original))) => {
                if selection.send_credit_opening().is_some()
                    || receiver_c.is_some()
                    || receiver_pi.is_some()
                {
                    return Err(proving_error(
                        "Redeem cannot consume Send receiver/credit sources",
                    ));
                }
                OrdinaryCashTerminalOutgoingWitnessV1::Redeem(OrdinaryRedeemOutputWitnessV1 {
                    output,
                    beneficiary,
                    manifest_original,
                })
            }
            _ => {
                return Err(proving_error(
                    "ordinary whole Terminal selected outgoing family differs",
                ));
            }
        };
        let semantic = OrdinaryCashTerminalSemanticWitnessV1 {
            state: &state,
            preparation_relation: &preparation_relation.0,
            terminal_relation: &terminal_relation.0,
            sender_credential: &credential,
            preparation_approval: &w2,
            preparation_integrity_lease: pi2.as_ref(),
            terminal_approval: &w1,
            terminal_integrity_lease: pi1.as_ref(),
            prepared: KagemushaOrdinaryRecursivePreparedOpeningV1 {
                record: selection.candidate().prepared_record(),
                sealed_transition_inputs: selection.transition_stream(),
                sealed_recovery_seeds: selection.recovery_stream(),
            },
            intent: selection.terminal_intent(),
            record: selection.terminal_record(),
            preparation_clock: selection.preparation_clock_context(),
            issuer_table: self.artifacts.ordinary_issuer_table(),
            outgoing,
        };
        let material = self
            .verifier
            .ordinary_cash_terminal_verifier_material()
            .map_err(proving_error)?;
        // Discovery audit values are actual prior State audit data only. The genuine Terminal
        // producer replaces both values with its complete own scalar audits before proof writing.
        // This constructor accepts no fake proof frame and never returns provisional acceptance.
        let public = selected_public_values_v1(
            selection,
            &material,
            material.terminal_protocol_digests,
            [
                selection.candidate().proof().eq_deferred_audit,
                selection.candidate().proof().ep_deferred_audit,
            ],
        )
        .map_err(owner_error)?;
        let mut rng_binding = Sha256::new();
        rng_binding.update(b"iroha:kagemusha:v1:ordinary-whole-terminal-native-recovery-seed\0");
        rng_binding.update(secret);
        rng_binding.update(selection.challenge().operation_id);
        rng_binding.update(selection.challenge().nonce);
        rng_binding.update(
            selection
                .authorization_binding_digest()
                .map_err(owner_error)?,
        );
        let seed = KagemushaRecoverySeedV1::from_unsealed(rng_binding.finalize().into())
            .map_err(|e| proving_error(e.to_string()))?;
        let candidate = selection.candidate();
        let eq_column =
            candidate_column::<Fp>(candidate.public_inputs(), &candidate.proof().eq_history)?;
        let ep_column =
            candidate_column::<Fq>(candidate.public_inputs(), &candidate.proof().ep_history)?;
        let eq_instances = vec![eq_column];
        let ep_instances = vec![ep_column];
        let checkpoint = self.verifier.state_checkpoint_material();
        let guard_material = self.verifier.ordinary_guard_verifier_material();
        // Both actual Guard originals have already passed the closed selection rechecks above.
        // The same sole canonical frames/protocols are re-used for all generated complete folds.
        let g2: OrdinaryGuardProofWireV1 =
            norito::decode_canonical(selection.preparation_guard().original())
                .map_err(|e| proving_error(e.to_string()))?;
        let g1: OrdinaryGuardProofWireV1 = norito::decode_canonical(terminal_guard.original())
            .map_err(|e| proving_error(e.to_string()))?;
        let g2eq = public_column::<Fp>(
            selection.preparation_guard().original_digests(),
            &g2.eq_history,
        );
        let g2ep = public_column::<Fq>(
            selection.preparation_guard().original_digests(),
            &g2.ep_history,
        );
        let g1eq = public_column::<Fp>(terminal_guard.original_digests(), &g1.eq_history);
        let g1ep = public_column::<Fq>(terminal_guard.original_digests(), &g1.ep_history);
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&candidate.proof().eq_history)
            .map_err(|e| proving_error(e.to_string()))?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&candidate.proof().ep_history)
            .map_err(|e| proving_error(e.to_string()))?;
        let g2eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&g2.eq_history)
            .map_err(|e| proving_error(e.to_string()))?;
        let g2ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&g2.ep_history)
            .map_err(|e| proving_error(e.to_string()))?;
        let g1eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&g1.eq_history)
            .map_err(|e| proving_error(e.to_string()))?;
        let g1ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&g1.ep_history)
            .map_err(|e| proving_error(e.to_string()))?;
        macro_rules! complete {
            ($ty:ty, $verify:ident, $fold:ident, $params:expr, $protocol:expr, $proof:expr, $instances:expr, $history:expr) => {{
                let current = <$ty>::from_native(
                    &$verify($params, $protocol, $proof, $instances).map_err(proving_error)?,
                )
                .map_err(|e| proving_error(e.to_string()))?;
                $fold($params, &current, $history, &seed)
                    .map_err(|e| proving_error(e.to_string()))?
            }};
        }
        let eq_candidate = complete!(
            KagemushaEqAccumulatorV1,
            verify_eq_succinct_protocol,
            fold_kagemusha_eq_accumulators_v1,
            checkpoint.eq_parameters,
            checkpoint.outer_eq_protocol,
            &candidate.proof().eq_proof,
            &eq_instances[0],
            &eq_history
        );
        let ep_candidate = complete!(
            KagemushaEpAccumulatorV1,
            verify_ep_succinct_protocol,
            fold_kagemusha_ep_accumulators_v1,
            checkpoint.ep_parameters,
            checkpoint.outer_ep_protocol,
            &candidate.proof().ep_proof,
            &ep_instances[0],
            &ep_history
        );
        let eq_preparation = complete!(
            KagemushaEqAccumulatorV1,
            verify_eq_succinct_protocol,
            fold_kagemusha_eq_accumulators_v1,
            guard_material.eq_parameters,
            guard_material.eq_protocol,
            &g2.eq_proof,
            &g2eq,
            &g2eq_history
        );
        let ep_preparation = complete!(
            KagemushaEpAccumulatorV1,
            verify_ep_succinct_protocol,
            fold_kagemusha_ep_accumulators_v1,
            guard_material.ep_parameters,
            guard_material.ep_protocol,
            &g2.ep_proof,
            &g2ep,
            &g2ep_history
        );
        let eq_terminal = complete!(
            KagemushaEqAccumulatorV1,
            verify_eq_succinct_protocol,
            fold_kagemusha_eq_accumulators_v1,
            guard_material.eq_parameters,
            guard_material.eq_protocol,
            &g1.eq_proof,
            &g1eq,
            &g1eq_history
        );
        let ep_terminal = complete!(
            KagemushaEpAccumulatorV1,
            verify_ep_succinct_protocol,
            fold_kagemusha_ep_accumulators_v1,
            guard_material.ep_parameters,
            guard_material.ep_protocol,
            &g1.ep_proof,
            &g1ep,
            &g1ep_history
        );
        let eq_prepare_merge = fold_kagemusha_eq_accumulators_v1(
            checkpoint.eq_parameters,
            eq_candidate.successor(),
            eq_preparation.successor(),
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let ep_prepare_merge = fold_kagemusha_ep_accumulators_v1(
            checkpoint.ep_parameters,
            ep_candidate.successor(),
            ep_preparation.successor(),
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let eq_terminal_merge = fold_kagemusha_eq_accumulators_v1(
            checkpoint.eq_parameters,
            eq_prepare_merge.successor(),
            eq_terminal.successor(),
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let ep_terminal_merge = fold_kagemusha_ep_accumulators_v1(
            checkpoint.ep_parameters,
            ep_prepare_merge.successor(),
            ep_terminal.successor(),
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let eq_hash = load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let ep_hash = load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let hash = prove_ordinary_cash_terminal_sha_claim_v1(
            &eq_hash,
            &ep_hash,
            &public,
            &semantic,
            &eq_instances,
            &ep_instances,
            &seed,
        )?;
        let eq_hash_merge = fold_kagemusha_eq_accumulators_v1(
            checkpoint.eq_parameters,
            eq_terminal_merge.successor(),
            &hash.eq_complete_history,
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let ep_hash_merge = fold_kagemusha_ep_accumulators_v1(
            checkpoint.ep_parameters,
            ep_terminal_merge.successor(),
            &hash.ep_complete_history,
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let protocols = [
            hash.eq_claim_protocol_digest,
            hash.ep_claim_protocol_digest,
            hash.eq_shard_protocol_digest,
            hash.ep_shard_protocol_digest,
        ];
        let eq_history_native = eq_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let ep_history_native = ep_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let g2eq_native = g2eq_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let g2ep_native = g2ep_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let g1eq_native = g1eq_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let g1ep_native = g1ep_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let eq_hash_native = hash
            .eq_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let ep_hash_native = hash
            .ep_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let witness = OrdinaryCashTerminalCircuitWitnessV1 {
            public,
            semantic: &semantic,
            eq: OrdinaryCashTerminalHalfWitnessV1 {
                candidate: OrdinaryCashCandidateCompleteProofV1 {
                    protocol: checkpoint.outer_eq_protocol,
                    instances: &eq_instances,
                    proof: &candidate.proof().eq_proof,
                    history: &eq_history_native,
                    history_fold_proof: eq_candidate.proof().as_bytes(),
                },
                preparation_guard: KagemushaOrdinaryGuardCompleteProofV1 {
                    protocol: guard_material.eq_protocol,
                    proof: &g2.eq_proof,
                    history: &g2eq_native,
                    history_bytes: &g2.eq_history,
                    history_fold_proof: eq_preparation.proof().as_bytes(),
                },
                terminal_guard: KagemushaOrdinaryGuardCompleteProofV1 {
                    protocol: guard_material.eq_protocol,
                    proof: &g1.eq_proof,
                    history: &g1eq_native,
                    history_bytes: &g1.eq_history,
                    history_fold_proof: eq_terminal.proof().as_bytes(),
                },
                preparation_merge_fold_proof: eq_prepare_merge.proof().as_bytes(),
                terminal_merge_fold_proof: eq_terminal_merge.proof().as_bytes(),
                hash_claim: KagemushaRecursiveHashClaimParityWitnessV1 {
                    protocol_digests: protocols,
                    protocol: &eq_hash.claim_protocol,
                    instances: &hash.eq_inner_instances,
                    proof: &hash.eq_proof,
                    history: &eq_hash_native,
                    history_fold_proof: hash.eq_history_fold_proof.as_bytes(),
                    merge_fold_proof: eq_hash_merge.proof().as_bytes(),
                },
                successor_history: eq_hash_merge.successor().as_bytes(),
            },
            ep: OrdinaryCashTerminalHalfWitnessV1 {
                candidate: OrdinaryCashCandidateCompleteProofV1 {
                    protocol: checkpoint.outer_ep_protocol,
                    instances: &ep_instances,
                    proof: &candidate.proof().ep_proof,
                    history: &ep_history_native,
                    history_fold_proof: ep_candidate.proof().as_bytes(),
                },
                preparation_guard: KagemushaOrdinaryGuardCompleteProofV1 {
                    protocol: guard_material.ep_protocol,
                    proof: &g2.ep_proof,
                    history: &g2ep_native,
                    history_bytes: &g2.ep_history,
                    history_fold_proof: ep_preparation.proof().as_bytes(),
                },
                terminal_guard: KagemushaOrdinaryGuardCompleteProofV1 {
                    protocol: guard_material.ep_protocol,
                    proof: &g1.ep_proof,
                    history: &g1ep_native,
                    history_bytes: &g1.ep_history,
                    history_fold_proof: ep_terminal.proof().as_bytes(),
                },
                preparation_merge_fold_proof: ep_prepare_merge.proof().as_bytes(),
                terminal_merge_fold_proof: ep_terminal_merge.proof().as_bytes(),
                hash_claim: KagemushaRecursiveHashClaimParityWitnessV1 {
                    protocol_digests: protocols,
                    protocol: &ep_hash.claim_protocol,
                    instances: &hash.ep_inner_instances,
                    proof: &hash.ep_proof,
                    history: &ep_hash_native,
                    history_fold_proof: hash.ep_history_fold_proof.as_bytes(),
                    merge_fold_proof: ep_hash_merge.proof().as_bytes(),
                },
                successor_history: ep_hash_merge.successor().as_bytes(),
            },
        };
        let (eq_keys, ep_keys) = self.load_terminal_keys()?;
        let inner = prove_ordinary_cash_terminal_pair_v1(&eq_keys, &ep_keys, witness, &seed)?;
        drop(eq_keys);
        drop(ep_keys);
        drop(eq_hash);
        drop(ep_hash);
        drop(hash);
        halo2_proofs::release_allocator_slack();
        let eq_wrapper_fold = fold_kagemusha_eq_accumulators_v1(
            material.eq_parameters,
            &inner.eq_current,
            &inner.eq_history,
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let ep_wrapper_fold = fold_kagemusha_ep_accumulators_v1(
            material.ep_parameters,
            &inner.ep_current,
            &inner.ep_history,
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let eq_inner_history = inner
            .eq_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let ep_inner_history = inner
            .ep_history
            .to_native()
            .map_err(|e| proving_error(e.to_string()))?;
        let inner_wire =
            decode_stateless_original_v1(&inner.original, 1, &material).map_err(owner_error)?;
        let wrapper_public = selected_public_values_v1(
            selection,
            &material,
            material.wrapper_protocol_digests,
            [inner_wire.eq_deferred_audit, inner_wire.ep_deferred_audit],
        )
        .map_err(owner_error)?;
        let eq_inner_instances = vec![inner.eq_instances.clone()];
        let ep_inner_instances = vec![inner.ep_instances.clone()];
        let wrapper_witness = OrdinaryCashCommitWrapperWitnessV1 {
            public: wrapper_public,
            eq: OrdinaryCashCommitWrapperHalfWitnessV1 {
                protocol: &inner.eq_protocol,
                instances: &eq_inner_instances,
                proof: &inner_wire.eq_proof,
                history: &eq_inner_history,
                history_fold_proof: eq_wrapper_fold.proof().as_bytes(),
                successor_history: eq_wrapper_fold.successor().as_bytes(),
            },
            ep: OrdinaryCashCommitWrapperHalfWitnessV1 {
                protocol: &inner.ep_protocol,
                instances: &ep_inner_instances,
                proof: &inner_wire.ep_proof,
                history: &ep_inner_history,
                history_fold_proof: ep_wrapper_fold.proof().as_bytes(),
                successor_history: ep_wrapper_fold.successor().as_bytes(),
            },
        };
        let (eq_keys, ep_keys) = self.load_wrapper_keys()?;
        let wrapper =
            prove_ordinary_cash_wrapper_pair_v1(&eq_keys, &ep_keys, wrapper_witness, &seed)?;
        let folds = wrapper.wrapper_history_fold_originals.ok_or_else(|| {
            proving_error("actual Wrapper producer did not retain original folds")
        })?;
        drop(eq_keys);
        drop(ep_keys);
        selection
            .recheck_selected_originals_and_current_custody()
            .map_err(owner_error)?;
        Ok((inner.original, wrapper.original, folds))
    }
}
fn candidate_column<F: KagemushaPoseidonFieldV1>(
    state: &KagemushaStateRelationPublicInputsV1,
    history: &[u8],
) -> Result<Vec<F>, KagemushaArtifactGenerationErrorV1> {
    if history.len() != super::super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 {
        return Err(proving_error("actual candidate history width differs"));
    }
    let mut values = state
        .recursive_semantic_public_instances::<F>()
        .map_err(proving_error)?;
    for chunk in history.chunks_exact(16) {
        values.push(from_u128::<F>(u128::from_le_bytes(
            chunk
                .try_into()
                .map_err(|e| proving_error(format!("candidate history limb: {e}")))?,
        )));
    }
    Ok(values)
}
fn outgoing_state_witness(
    p: &KagemushaStateRelationPublicInputsV1,
) -> Result<KagemushaStateRelationWitnessV1, KagemushaArtifactGenerationErrorV1> {
    if !matches!(
        p.operation,
        KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
    ) {
        return Err(proving_error(
            "ordinary whole Terminal requires actual outgoing State",
        ));
    }
    let value = KagemushaStateRelationWitnessV1 {
        operation: p.operation,
        predecessor: p.predecessor.clone(),
        successor: p.successor.clone(),
        amount: p.amount,
        journal_revision_before: p.journal_revision_before,
        journal_revision_after: p.journal_revision_after,
        transition_effect_digest: p.transition_effect_digest,
        mint_finality_semantic_digest: p.mint_finality_semantic_digest,
        mint_finality_proof_binding_digest: p.mint_finality_proof_binding_digest,
        peer_credit_id: p.peer_credit_id,
        recipient_encryption_key_binding: p.recipient_encryption_key_binding,
        receive_credit: None,
        receive_credit_binding_digest: p.receive_credit_binding_digest,
        lifecycle_binding_digest: p.lifecycle_binding_digest,
        prepared_transition_binding_digest: p.prepared_transition_binding_digest,
        prepared_intent: p.prepared_intent,
        transport_semantic_digest: p.transport_semantic_digest,
        guard_statement_digest: p.guard_statement_digest,
        eq_protocol_digest: p.eq_protocol_digest,
        ep_protocol_digest: p.ep_protocol_digest,
        guard_eq_protocol_digest: p.guard_eq_protocol_digest,
        guard_ep_protocol_digest: p.guard_ep_protocol_digest,
        mint_eq_protocol_digest: p.mint_eq_protocol_digest,
        mint_ep_protocol_digest: p.mint_ep_protocol_digest,
        mint_authorization_eq_protocol_digest: p.mint_authorization_eq_protocol_digest,
        mint_authorization_ep_protocol_digest: p.mint_authorization_ep_protocol_digest,
        commit_wrapper_eq_protocol_digest: p.commit_wrapper_eq_protocol_digest,
        commit_wrapper_ep_protocol_digest: p.commit_wrapper_ep_protocol_digest,
        guard_eq_credential_audit: p.guard_eq_credential_audit,
        guard_ep_credential_audit: p.guard_ep_credential_audit,
        eq_deferred_audit: p.eq_deferred_audit,
        ep_deferred_audit: p.ep_deferred_audit,
        replay_insert: None,
    };
    value.validate().map_err(proving_error)?;
    Ok(value)
}
