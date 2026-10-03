//! Native construction of the exact retained incoming State recursive operands.
//!
//! Only the actual W2 owner can construct this source. Every protocol comes from the same
//! authenticated release; the inner predecessor is restored from its private durable original.
//! Public deterministic fold seeds confer no source, financial, clock or key authority.

use super::production_ordinary_guard::public_incoming_relation;
use super::production_ordinary_incoming_state::{
    KagemushaOrdinaryIncomingAuxiliaryConsumerV1, KagemushaOrdinaryIncomingAuxiliaryProofSourceV1,
};
use super::production_ordinary_padding::inactive_mint_parser_operands;
use super::production_ordinary_state::bootstrap_inputs::{
    inactive_column, inactive_incoming_column,
};
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    KagemushaMintFinalityHelperVerificationRequestV1, KagemushaOrdinaryCashOutgoingOriginalV1,
    KagemushaReceiveFoldCreditV1, KagemushaReplayInsertWitnessV1,
    ordinary_cash_terminal_verifier::decode_stateless_original_v1,
    ordinary_guard_verifier::{OrdinaryGuardProofWireV1, public_column},
    ordinary_lineage_proof_admission::terminal_public,
    ordinary_mint_public::{ordinary_mint_public_column_v1, ordinary_mint_public_data_v1},
    ordinary_state_reserved::kagemusha_ordinary_state_outer_protocol_positions_v1,
};
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1;
use iroha_data_model::kagemusha::*;
use zeroize::Zeroize as _;

macro_rules! proof_leg {
    ($name:ident, $field:ty, $curve:ty, $acc:ty, $fold:ty, $verify:path, $fold_fn:path, $parity:expr) => {
        struct $name {
            protocol: PlonkProtocol<$curve>,
            instances: Vec<Vec<$field>>,
            proof: Vec<u8>,
            history: $acc,
            complete: $acc,
            complete_fold: $fold,
            merged: $acc,
            merge_fold: $fold,
        }
        impl Drop for $name {
            fn drop(&mut self) {
                for c in &mut self.instances {
                    c.fill(<$field>::ZERO);
                }
                self.proof.zeroize();
            }
        }
        impl $name {
            fn actual(
                parameters: &ParamsIPA<$curve>,
                protocol: &PlonkProtocol<$curve>,
                instances: Vec<Vec<$field>>,
                proof: Vec<u8>,
                history: $acc,
                predecessor: Option<&$acc>,
                seed: &KagemushaRecoverySeedV1,
            ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
                let [column] = instances.as_slice() else {
                    return Err(proving_error(
                        "incoming actual recursive column count differs",
                    ));
                };
                let current = <$acc>::from_native(
                    &$verify(parameters, protocol, &proof, column).map_err(proving_error)?,
                )
                .map_err(|e| proving_error(e.to_string()))?;
                let complete = $fold_fn(parameters, &current, &history, seed)
                    .map_err(|e| proving_error(e.to_string()))?;
                let (merged, merge_fold) = match predecessor {
                    Some(before) => {
                        let m = $fold_fn(parameters, before, complete.successor(), seed)
                            .map_err(|e| proving_error(e.to_string()))?;
                        (m.successor().clone(), m.proof().clone())
                    }
                    None => (complete.successor().clone(), complete.proof().clone()),
                };
                Ok(Self {
                    protocol: protocol.clone(),
                    instances,
                    proof,
                    history,
                    complete: complete.successor().clone(),
                    complete_fold: complete.proof().clone(),
                    merged,
                    merge_fold,
                })
            }
            fn inactive(
                protocol: &PlonkProtocol<$curve>,
                instances: Vec<Vec<$field>>,
                history: &$acc,
            ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
                use halo2_proofs::halo2curves::group::{
                    GroupEncoding as _, prime::PrimeCurveAffine as _,
                };
                let point = <$curve>::generator().to_bytes();
                let fold = <$fold>::try_from_bytes(&dummy_fold_proof_bytes(point.as_ref()))
                    .map_err(|e| proving_error(e.to_string()))?;
                Ok(Self {
                    protocol: protocol.clone(),
                    instances,
                    proof: dummy_ordinary_proof_bytes(protocol, point.as_ref(), $parity)?,
                    history: history.clone(),
                    complete: history.clone(),
                    complete_fold: fold.clone(),
                    merged: history.clone(),
                    merge_fold: fold,
                })
            }
        }
    };
}
proof_leg!(
    EqLeg,
    Fp,
    EqAffine,
    KagemushaEqAccumulatorV1,
    KagemushaEqFoldProofV1,
    verify_eq_succinct_protocol,
    fold_kagemusha_eq_accumulators_v1,
    KagemushaPastaParityV1::Eq
);
proof_leg!(
    EpLeg,
    Fq,
    EpAffine,
    KagemushaEpAccumulatorV1,
    KagemushaEpFoldProofV1,
    verify_ep_succinct_protocol,
    fold_kagemusha_ep_accumulators_v1,
    KagemushaPastaParityV1::Ep
);

/// Actual-owner-only retained operands. No constructor, decoder or Debug exposes this source.
pub(crate) struct KagemushaRetainedOrdinaryIncomingAuxiliariesV1 {
    operation_id: [u8; 32],
    nonce: [u8; 32],
    reservation_digest: [u8; 32],
    guard_original: Vec<u8>,
    predecessor_public_original: Vec<u8>,
    eq_hash: KagemushaLoadedEqMintHashArtifactsV1,
    ep_hash: KagemushaLoadedEpMintHashArtifactsV1,
    state: KagemushaStateRelationWitnessV1,
    guard_relation: KagemushaGuardBundleRelationWitnessV1,
    authorization: KagemushaMintAuthorizationV1,
    credit: KagemushaMintCreditV1,
    eq_parent: EqLeg,
    ep_parent: EpLeg,
    eq_outer: EqLeg,
    ep_outer: EpLeg,
    eq_incoming: EqLeg,
    ep_incoming: EpLeg,
    eq_guard: EqLeg,
    ep_guard: EpLeg,
    eq_authorization: EqLeg,
    ep_authorization: EpLeg,
    eq_mint: EqLeg,
    ep_mint: EpLeg,
    eq_final_history: KagemushaEqAccumulatorV1,
    ep_final_history: KagemushaEpAccumulatorV1,
}
impl Drop for KagemushaRetainedOrdinaryIncomingAuxiliariesV1 {
    fn drop(&mut self) {
        if let Some(s) = self.state.predecessor.as_mut() {
            s.balance.zeroize();
            s.state_nonce_commitment.zeroize();
        }
        self.state.successor.balance.zeroize();
        self.state.successor.state_nonce_commitment.zeroize();
        if let Some(c) = self.state.receive_credit.as_mut() {
            c.credit_opening.credit_commitment_opening.zeroize();
            c.credit_opening.recipient_binding_opening.zeroize();
            c.credit_opening.recovery_nonce.zeroize();
        }
    }
}

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Build the entire incoming fold chronology from actual same-owner retained originals.
    /// Inactive slots are parser data under a zero selector, never fabricated predecessor State.
    pub(crate) fn prepare_ordinary_incoming_auxiliaries(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    ) -> Result<KagemushaRetainedOrdinaryIncomingAuxiliariesV1, KagemushaArtifactGenerationErrorV1>
    {
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
        let verifier = selection.recursive_verifier();
        let m = verifier.state_checkpoint_material();
        let g = verifier.ordinary_guard_verifier_material();
        let a = verifier
            .ordinary_bootstrap_auxiliary_material()
            .map_err(proving_error)?;
        let artifacts = self.artifacts.ordinary_recursion_artifacts()?;
        let wire: OrdinaryGuardProofWireV1 =
            norito::decode_canonical(guard.original()).map_err(ordinary_error)?;
        if norito::encode_canonical(&wire).map_err(ordinary_error)? != guard.original() {
            return Err(proving_error("incoming Guard canonical original differs"));
        }
        let challenge = selection.challenge().map_err(owner_error)?;
        let reservation_digest = selection
            .reservation()
            .map_err(owner_error)?
            .digest()
            .map_err(ordinary_error)?;
        let predecessor_public_original = selection
            .predecessor_public_state_original()
            .map_err(owner_error)?
            .to_vec();
        let seed = incoming_public_fold_seed(
            challenge.operation_id,
            challenge.nonce,
            reservation_digest,
            guard.original(),
            &predecessor_public_original,
        )?;
        let mut parent = None;
        selection
            .with_retained_predecessor_checkpoint(&mut |checkpoint| {
                if parent.is_some() {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                parent = Some((|| {
                    let eq = EqLeg::actual(
                        m.eq_parameters,
                        m.inner_eq_protocol,
                        vec![checkpoint.eq_public_instances.clone()],
                        checkpoint.eq_inner_proof.clone(),
                        checkpoint.eq_history.clone(),
                        None,
                        &seed,
                    )?;
                    let ep = EpLeg::actual(
                        m.ep_parameters,
                        m.inner_ep_protocol,
                        vec![checkpoint.ep_public_instances.clone()],
                        checkpoint.ep_inner_proof.clone(),
                        checkpoint.ep_history.clone(),
                        None,
                        &seed,
                    )?;
                    let eo = EqLeg::actual(
                        m.eq_parameters,
                        m.outer_eq_protocol,
                        vec![checkpoint.eq_transport_public_instances.clone()],
                        checkpoint.proof.eq_proof.clone(),
                        KagemushaEqAccumulatorV1::try_from_bytes(&checkpoint.proof.eq_history)
                            .map_err(ordinary_error)?,
                        Some(&eq.complete),
                        &seed,
                    )?;
                    let po = EpLeg::actual(
                        m.ep_parameters,
                        m.outer_ep_protocol,
                        vec![checkpoint.ep_transport_public_instances.clone()],
                        checkpoint.proof.ep_proof.clone(),
                        KagemushaEpAccumulatorV1::try_from_bytes(&checkpoint.proof.ep_history)
                            .map_err(ordinary_error)?,
                        Some(&ep.complete),
                        &seed,
                    )?;
                    Ok::<_, KagemushaArtifactGenerationErrorV1>((eq, ep, eo, po))
                })());
                Ok(())
            })
            .map_err(owner_error)?;
        let (eq_parent, ep_parent, eq_outer, ep_outer) =
            parent.ok_or_else(|| proving_error("actual incoming predecessor was not lent"))??;
        let eq_empty =
            initial_kagemusha_eq_accumulator_v1(m.eq_parameters).map_err(ordinary_error)?;
        let ep_empty =
            initial_kagemusha_ep_accumulator_v1(m.ep_parameters).map_err(ordinary_error)?;
        let state = selection.selected_successor_state().map_err(owner_error)?;
        let padding = inactive_mint_parser_operands(
            state,
            &selection
                .reservation()
                .map_err(owner_error)?
                .selection
                .lineage
                .owner
                .account_id,
            m.binding.artifact_manifest_digest,
            a.genesis_authorization_id,
            a.eq_mint_authorization_protocol,
            a.ep_mint_authorization_protocol,
            a.eq_mint_protocol,
            a.ep_mint_protocol,
            &eq_empty,
            &ep_empty,
        )?;
        let authorization = padding.authorization;
        let mut credit = padding.credit;
        let mut incoming = None;
        let mut mint_auth = None;
        let mut receive_credit = None;
        let operation =
            KagemushaOperationV1::from(selection.transition_statement().map_err(owner_error)?.kind);
        match operation {
            KagemushaOperationV1::MintFold => {
                selection
                    .with_finalized_mint_source(&mut |source, original_credit| {
                        if mint_auth.is_some() {
                            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                        }
                        source.recheck_retained_custody().map_err(native_reject)?;
                        let admitted = source.authorization().map_err(native_reject)?;
                        mint_auth = Some((|| {
                            let c = KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(
                                admitted.credential_original(),
                            )
                            .map_err(ordinary_error)?;
                            let lease = admitted
                                .selected_integrity_original()
                                .map(|raw| {
                                    norito::decode_canonical::<
                                            KagemushaPlayIntegrityRefreshLeaseV1,
                                        >(raw)
                                        .map_err(ordinary_error)
                                })
                                .transpose()?;
                            let x = admitted.authorization();
                            let public = ordinary_mint_public_data_v1(
                                &x.statement,
                                &x.approval,
                                &c,
                                lease.as_ref(),
                                self.release.provider_policy_root(),
                            )
                            .map_err(proving_error)?;
                            let eh = KagemushaEqAccumulatorV1::try_from_bytes(&x.proof.eq_history)
                                .map_err(ordinary_error)?;
                            let ph = KagemushaEpAccumulatorV1::try_from_bytes(&x.proof.ep_history)
                                .map_err(ordinary_error)?;
                            Ok::<_, KagemushaArtifactGenerationErrorV1>((
                                public,
                                x.proof.eq_proof.clone(),
                                x.proof.ep_proof.clone(),
                                eh,
                                ph,
                            ))
                        })());
                        credit = original_credit.clone();
                        source.recheck_retained_custody().map_err(native_reject)
                    })
                    .map_err(owner_error)?;
            }
            KagemushaOperationV1::ReceiveFold => {
                selection
                    .with_received_source_and_request(&mut |source, request| {
                        if incoming.is_some() {
                            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                        }
                        request.recheck_historical_custody()?;
                        if source.request_original() != request.request_original()? {
                            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                        }
                        incoming = Some((|| {
                            let outgoing =
                                KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(
                                    source.outgoing_original(),
                                )
                                .map_err(proving_error)?;
                            let terminal_material = verifier
                                .ordinary_cash_terminal_verifier_material()
                                .map_err(proving_error)?;
                            let pair = decode_stateless_original_v1(
                                outgoing.wrapper_original(),
                                2,
                                &terminal_material,
                            )
                            .map_err(owner_error)?;
                            let record = outgoing.terminal_record();
                            let body = &record.body;
                            let public = terminal_public(
                                &terminal_material,
                                outgoing.normalized_preparation(),
                                body.binding_digest().map_err(ordinary_error)?,
                                body.candidate_digest,
                                record.binding_digest().map_err(ordinary_error)?,
                                source.output().transition_nullifier,
                                body,
                                source.output().ciphertext_commitment,
                                &pair,
                            )
                            .map_err(proving_error)?;
                            let r = source.request();
                            let o = source.output();
                            // Zero secret placeholders are replaced only by the separate actual Native AEAD loan.
                            receive_credit = Some(KagemushaReceiveFoldCreditV1 {
                                amount: o.amount,
                                credit_id: o.credit_id,
                                recipient_lane_id: r.body.recipient_lane_id,
                                incoming_proof_binding_digest: body.candidate_digest,
                                request_digest: r
                                    .canonical_original_digest()
                                    .map_err(ordinary_error)?,
                                prepared_transfer_digest: body
                                    .binding_digest()
                                    .map_err(ordinary_error)?,
                                transition_nullifier: o.transition_nullifier,
                                recipient_encryption_key: r.body.recipient_encryption_key,
                                ciphertext_commitment: o.ciphertext_commitment,
                                credit_opening: KagemushaCreditOpeningV1 {
                                    version: 1,
                                    credit_id: o.credit_id,
                                    amount: o.amount,
                                    credit_commitment_opening: [0; 32],
                                    recipient_binding_opening: [0; 32],
                                    recovery_nonce: [0; 32],
                                },
                                receiver_binding_digest: source.receiver_credential_digest(),
                                payment_output_digest: o
                                    .binding_digest()
                                    .map_err(ordinary_error)?,
                                replay_insert: KagemushaReplayInsertWitnessV1::from(
                                    selection.replay_witness().map_err(owner_error)?,
                                ),
                            });
                            Ok::<_, KagemushaArtifactGenerationErrorV1>((public, pair))
                        })());
                        request.recheck_historical_custody()
                    })
                    .map_err(owner_error)?;
            }
            _ => {
                return Err(proving_error(
                    "incoming auxiliary source refuses another operation",
                ));
            }
        }
        let (eq_incoming, ep_incoming, mut eq_running, mut ep_running) = match incoming {
            Some(result) => {
                let (public, pair) = result?;
                let eq = EqLeg::actual(
                    m.eq_parameters,
                    a.eq_incoming_protocol,
                    vec![
                        public
                            .public_column::<Fp>(&pair.eq_history)
                            .map_err(proving_error)?,
                    ],
                    pair.eq_proof,
                    KagemushaEqAccumulatorV1::try_from_bytes(&pair.eq_history)
                        .map_err(ordinary_error)?,
                    Some(&eq_outer.merged),
                    &seed,
                )?;
                let ep = EpLeg::actual(
                    m.ep_parameters,
                    a.ep_incoming_protocol,
                    vec![
                        public
                            .public_column::<Fq>(&pair.ep_history)
                            .map_err(proving_error)?,
                    ],
                    pair.ep_proof,
                    KagemushaEpAccumulatorV1::try_from_bytes(&pair.ep_history)
                        .map_err(ordinary_error)?,
                    Some(&ep_outer.merged),
                    &seed,
                )?;
                let er = eq.merged.clone();
                let pr = ep.merged.clone();
                (eq, ep, er, pr)
            }
            None => (
                EqLeg::inactive(
                    a.eq_incoming_protocol,
                    inactive_incoming_column(
                        &a.eq_incoming_protocol.num_instance,
                        artifacts.commit_wrapper_eq_protocol_digest,
                        artifacts.commit_wrapper_ep_protocol_digest,
                        eq_empty.as_bytes(),
                    )?,
                    &eq_empty,
                )?,
                EpLeg::inactive(
                    a.ep_incoming_protocol,
                    inactive_incoming_column(
                        &a.ep_incoming_protocol.num_instance,
                        artifacts.commit_wrapper_eq_protocol_digest,
                        artifacts.commit_wrapper_ep_protocol_digest,
                        ep_empty.as_bytes(),
                    )?,
                    &ep_empty,
                )?,
                eq_outer.merged.clone(),
                ep_outer.merged.clone(),
            ),
        };
        let digests = guard.original_digests();
        let eq_guard = EqLeg::actual(
            m.eq_parameters,
            g.eq_protocol,
            vec![public_column::<Fp>(digests, &wire.eq_history)],
            wire.eq_proof,
            KagemushaEqAccumulatorV1::try_from_bytes(&wire.eq_history).map_err(ordinary_error)?,
            Some(&eq_running),
            &seed,
        )?;
        let ep_guard = EpLeg::actual(
            m.ep_parameters,
            g.ep_protocol,
            vec![public_column::<Fq>(digests, &wire.ep_history)],
            wire.ep_proof,
            KagemushaEpAccumulatorV1::try_from_bytes(&wire.ep_history).map_err(ordinary_error)?,
            Some(&ep_running),
            &seed,
        )?;
        eq_running = eq_guard.merged.clone();
        ep_running = ep_guard.merged.clone();
        let (eq_authorization, ep_authorization) = match mint_auth {
            Some(result) => {
                let (public, eq_proof, ep_proof, eh, ph) = result?;
                let eq = EqLeg::actual(m.eq_parameters, a.eq_mint_authorization_protocol,
                    vec![ordinary_mint_public_column_v1::<Fp>(&public, eh.as_bytes())], eq_proof, eh, Some(&eq_running), &seed)?;
                let ep = EpLeg::actual(m.ep_parameters, a.ep_mint_authorization_protocol,
                    vec![ordinary_mint_public_column_v1::<Fq>(&public, ph.as_bytes())], ep_proof, ph, Some(&ep_running), &seed)?;
                eq_running = eq.merged.clone(); ep_running = ep.merged.clone(); (eq, ep)
            }
            None => (
                EqLeg::inactive(a.eq_mint_authorization_protocol, inactive_column(&a.eq_mint_authorization_protocol.num_instance,
                    crate::kagemusha_v1_recursion::ordinary_mint_circuit::ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1, eq_empty.as_bytes())?, &eq_empty)?,
                EpLeg::inactive(a.ep_mint_authorization_protocol, inactive_column(&a.ep_mint_authorization_protocol.num_instance,
                    crate::kagemusha_v1_recursion::ordinary_mint_circuit::ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1, ep_empty.as_bytes())?, &ep_empty)?,
            ),
        };
        let request = KagemushaMintFinalityHelperVerificationRequestV1 {
            eq_protocol_digest: credit.proof.eq_protocol_digest,
            ep_protocol_digest: credit.proof.ep_protocol_digest,
            statement: &credit.statement,
            semantic_digest: credit.proof.semantic_digest,
            proof: &credit.proof,
            finality_certificate_binding: credit.finality_certificate_binding,
            finality_authority_head: credit.finality_authority_head,
            finality_genesis_authorization_id: credit.finality_genesis_authorization_id,
            finality_proof_binding_digest: credit.finality_proof_binding_digest,
            artifact_manifest_digest: credit.artifact_manifest_digest,
        };
        let eh = KagemushaEqAccumulatorV1::try_from_bytes(&credit.proof.eq_history)
            .map_err(ordinary_error)?;
        let ph = KagemushaEpAccumulatorV1::try_from_bytes(&credit.proof.ep_history)
            .map_err(ordinary_error)?;
        let ei = vec![
            crate::kagemusha_v1_recursion::native_backend::mint_public_instances::<Fp>(
                &request,
                eh.as_bytes(),
            )
            .map_err(proving_error)?,
        ];
        let pi = vec![
            crate::kagemusha_v1_recursion::native_backend::mint_public_instances::<Fq>(
                &request,
                ph.as_bytes(),
            )
            .map_err(proving_error)?,
        ];
        let (eq_mint, ep_mint) = if operation == KagemushaOperationV1::MintFold {
            let eq = EqLeg::actual(
                m.eq_parameters,
                a.eq_mint_protocol,
                ei,
                credit.proof.eq_proof.clone(),
                eh,
                Some(&eq_running),
                &seed,
            )?;
            let ep = EpLeg::actual(
                m.ep_parameters,
                a.ep_mint_protocol,
                pi,
                credit.proof.ep_proof.clone(),
                ph,
                Some(&ep_running),
                &seed,
            )?;
            eq_running = eq.merged.clone();
            ep_running = ep.merged.clone();
            (eq, ep)
        } else {
            (
                EqLeg::inactive(a.eq_mint_protocol, ei, &eh)?,
                EpLeg::inactive(a.ep_mint_protocol, pi, &ph)?,
            )
        };
        let t = selection.transition_statement().map_err(owner_error)?;
        let (eq_reserved, ep_reserved) =
            kagemusha_ordinary_state_outer_protocol_positions_v1(verifier);
        let relation = KagemushaStateRelationWitnessV1 {
            operation,
            predecessor: Some(
                selection
                    .selected_predecessor_state()
                    .map_err(owner_error)?
                    .clone(),
            ),
            successor: state.clone(),
            amount: t.amount,
            journal_revision_before: t.journal_revision_before,
            journal_revision_after: t.journal_revision_after,
            transition_effect_digest: t.effect_digest,
            mint_finality_semantic_digest: t.mint_finality_semantic_digest,
            mint_finality_proof_binding_digest: t.mint_finality_proof_binding_digest,
            peer_credit_id: t.peer_credit_id,
            recipient_encryption_key_binding: t.recipient_encryption_key_binding,
            receive_credit,
            receive_credit_binding_digest: t.receive_credit_binding_digest,
            lifecycle_binding_digest: t.lifecycle_binding_digest,
            prepared_transition_binding_digest: [0; 32],
            prepared_intent: None,
            transport_semantic_digest: selection
                .transport_semantic_digest()
                .map_err(owner_error)?,
            guard_statement_digest: digests[0],
            eq_protocol_digest: artifacts.eq_protocol_digest,
            ep_protocol_digest: artifacts.ep_protocol_digest,
            guard_eq_protocol_digest: artifacts.guard_bundle_eq_protocol_digest,
            guard_ep_protocol_digest: artifacts.guard_bundle_ep_protocol_digest,
            mint_eq_protocol_digest: artifacts.mint_finality_eq_protocol_digest,
            mint_ep_protocol_digest: artifacts.mint_finality_ep_protocol_digest,
            mint_authorization_eq_protocol_digest: artifacts.mint_authorization_eq_protocol_digest,
            mint_authorization_ep_protocol_digest: artifacts.mint_authorization_ep_protocol_digest,
            commit_wrapper_eq_protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
            commit_wrapper_ep_protocol_digest: artifacts.commit_wrapper_ep_protocol_digest,
            guard_eq_credential_audit: eq_reserved,
            guard_ep_credential_audit: ep_reserved,
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            replay_insert: if operation == KagemushaOperationV1::MintFold {
                Some(KagemushaReplayInsertWitnessV1::from(
                    selection.replay_witness().map_err(owner_error)?,
                ))
            } else {
                None
            },
        };
        let retained = KagemushaRetainedOrdinaryIncomingAuxiliariesV1 {
            operation_id: challenge.operation_id,
            nonce: challenge.nonce,
            reservation_digest,
            guard_original: guard.original().to_vec(),
            predecessor_public_original,
            eq_hash: load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?,
            ep_hash: load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?,
            state: relation,
            guard_relation: public_incoming_relation(selection)?,
            authorization,
            credit,
            eq_parent,
            ep_parent,
            eq_outer,
            ep_outer,
            eq_incoming,
            ep_incoming,
            eq_guard,
            ep_guard,
            eq_authorization,
            ep_authorization,
            eq_mint,
            ep_mint,
            eq_final_history: eq_running,
            ep_final_history: ep_running,
        };
        guard
            .recheck_incoming_selection(selection)
            .map_err(owner_error)?;
        selection
            .recheck_selected_originals_and_current_custody()
            .map_err(owner_error)?;
        Ok(retained)
    }
}

fn incoming_public_fold_seed(
    operation: [u8; 32],
    nonce: [u8; 32],
    reservation: [u8; 32],
    guard: &[u8],
    parent: &[u8],
) -> Result<KagemushaRecoverySeedV1, KagemushaArtifactGenerationErrorV1> {
    let mut h = Sha256::new();
    h.update(b"iroha:kagemusha:v1:ordinary-incoming-public-fold\0");
    h.update(operation);
    h.update(nonce);
    h.update(reservation);
    h.update(Sha256::digest(guard));
    h.update(Sha256::digest(parent));
    KagemushaRecoverySeedV1::from_unsealed(h.finalize().into()).map_err(ordinary_error)
}

fn native_reject(_: impl core::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}

fn ordinary_error(e: impl core::fmt::Display) -> KagemushaArtifactGenerationErrorV1 {
    proving_error(e.to_string())
}

impl KagemushaRetainedOrdinaryIncomingAuxiliariesV1 {
    fn outer(&self) -> KagemushaOrdinaryRecursiveOuterParentWitnessV1<'_> {
        KagemushaOrdinaryRecursiveOuterParentWitnessV1 {
            public_original: Some(&self.predecessor_public_original),
            eq_protocol: &self.eq_outer.protocol,
            ep_protocol: &self.ep_outer.protocol,
            eq_instances: &self.eq_outer.instances,
            ep_instances: &self.ep_outer.instances,
            eq_proof: &self.eq_outer.proof,
            ep_proof: &self.ep_outer.proof,
            eq_history: &self.eq_outer.history,
            ep_history: &self.ep_outer.history,
            eq_history_fold: &self.eq_outer.complete_fold,
            ep_history_fold: &self.ep_outer.complete_fold,
            eq_merge_fold: &self.eq_outer.merge_fold,
            ep_merge_fold: &self.ep_outer.merge_fold,
        }
    }
    fn witness<'a>(
        &'a self,
        hash_claim: Option<KagemushaMintHashClaimGenerationWitnessV1<'a>>,
        eq_final: &'a KagemushaEqAccumulatorV1,
        ep_final: &'a KagemushaEpAccumulatorV1,
    ) -> KagemushaRecursiveStateGenerationWitnessV1<'a> {
        KagemushaRecursiveStateGenerationWitnessV1 {
            hash_claim,
            state: self.state.clone(),
            mint_fold_opening: None,
            mint_authorization: &self.authorization,
            mint_credit: &self.credit,
            guard_relation: self.guard_relation.clone(),
            hardware_selection: None,
            ordinary_selection: None,
            eq_parent_protocol: &self.eq_parent.protocol,
            ep_parent_protocol: &self.ep_parent.protocol,
            eq_parent_instances: &self.eq_parent.instances,
            ep_parent_instances: &self.ep_parent.instances,
            eq_parent_proof: &self.eq_parent.proof,
            ep_parent_proof: &self.ep_parent.proof,
            eq_predecessor_history: &self.eq_parent.history,
            ep_predecessor_history: &self.ep_parent.history,
            eq_parent_fold_proof: &self.eq_parent.complete_fold,
            ep_parent_fold_proof: &self.ep_parent.complete_fold,
            eq_incoming_protocol: &self.eq_incoming.protocol,
            ep_incoming_protocol: &self.ep_incoming.protocol,
            eq_incoming_credits: [KagemushaRecursiveIncomingEqGenerationWitnessV1 {
                instances: &self.eq_incoming.instances,
                proof: &self.eq_incoming.proof,
                history: &self.eq_incoming.history,
                history_fold_proof: &self.eq_incoming.complete_fold,
                merge_fold_proof: &self.eq_incoming.merge_fold,
            }],
            ep_incoming_credits: [KagemushaRecursiveIncomingEpGenerationWitnessV1 {
                instances: &self.ep_incoming.instances,
                proof: &self.ep_incoming.proof,
                history: &self.ep_incoming.history,
                history_fold_proof: &self.ep_incoming.complete_fold,
                merge_fold_proof: &self.ep_incoming.merge_fold,
            }],
            eq_successor_history: eq_final,
            ep_successor_history: ep_final,
            eq_guard_protocol: &self.eq_guard.protocol,
            ep_guard_protocol: &self.ep_guard.protocol,
            eq_guard_proof: &self.eq_guard.proof,
            ep_guard_proof: &self.ep_guard.proof,
            eq_guard_history: &self.eq_guard.history,
            ep_guard_history: &self.ep_guard.history,
            eq_guard_history_fold_proof: &self.eq_guard.complete_fold,
            ep_guard_history_fold_proof: &self.ep_guard.complete_fold,
            eq_guard_merge_fold_proof: &self.eq_guard.merge_fold,
            ep_guard_merge_fold_proof: &self.ep_guard.merge_fold,
            eq_mint_authorization_protocol: &self.eq_authorization.protocol,
            ep_mint_authorization_protocol: &self.ep_authorization.protocol,
            eq_mint_authorization_instances: &self.eq_authorization.instances,
            ep_mint_authorization_instances: &self.ep_authorization.instances,
            eq_mint_authorization_proof: &self.eq_authorization.proof,
            ep_mint_authorization_proof: &self.ep_authorization.proof,
            eq_mint_authorization_history: &self.eq_authorization.history,
            ep_mint_authorization_history: &self.ep_authorization.history,
            eq_mint_authorization_history_fold_proof: &self.eq_authorization.complete_fold,
            ep_mint_authorization_history_fold_proof: &self.ep_authorization.complete_fold,
            eq_mint_authorization_merge_fold_proof: &self.eq_authorization.merge_fold,
            ep_mint_authorization_merge_fold_proof: &self.ep_authorization.merge_fold,
            eq_mint_protocol: &self.eq_mint.protocol,
            ep_mint_protocol: &self.ep_mint.protocol,
            eq_mint_instances: &self.eq_mint.instances,
            ep_mint_instances: &self.ep_mint.instances,
            eq_mint_proof: &self.eq_mint.proof,
            ep_mint_proof: &self.ep_mint.proof,
            eq_mint_history: &self.eq_mint.history,
            ep_mint_history: &self.ep_mint.history,
            eq_mint_history_fold_proof: &self.eq_mint.complete_fold,
            ep_mint_history_fold_proof: &self.ep_mint.complete_fold,
            eq_mint_merge_fold_proof: &self.eq_mint.merge_fold,
            ep_mint_merge_fold_proof: &self.ep_mint.merge_fold,
        }
    }
    fn recheck(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    ) -> Result<(), String> {
        selection
            .recheck_selected_originals_and_current_custody()
            .map_err(|e| e.to_string())?;
        guard
            .recheck_incoming_selection(selection)
            .map_err(|e| e.to_string())?;
        let challenge = selection.challenge().map_err(|e| e.to_string())?;
        if challenge.operation_id != self.operation_id
            || challenge.nonce != self.nonce
            || selection
                .reservation()
                .map_err(|e| e.to_string())?
                .digest()?
                != self.reservation_digest
            || guard.original() != self.guard_original
            || selection
                .predecessor_public_state_original()
                .map_err(|e| e.to_string())?
                != self.predecessor_public_original
            || selection
                .selected_predecessor_state()
                .map_err(|e| e.to_string())?
                != self
                    .state
                    .predecessor
                    .as_ref()
                    .ok_or("incoming retained predecessor absent")?
            || selection
                .selected_successor_state()
                .map_err(|e| e.to_string())?
                != &self.state.successor
        {
            return Err("retained incoming auxiliary originals differ from actual W2".into());
        }
        Ok(())
    }
}
impl KagemushaOrdinaryIncomingAuxiliaryProofSourceV1
    for KagemushaRetainedOrdinaryIncomingAuxiliariesV1
{
    fn with_borrowed_incoming_auxiliaries(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
        claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        consume: &mut KagemushaOrdinaryIncomingAuxiliaryConsumerV1<'_>,
    ) -> Result<(), String> {
        self.recheck(selection, guard)?;
        match claim {
            None => consume(
                self.witness(None, &self.eq_final_history, &self.ep_final_history),
                self.outer(),
            )?,
            Some(claim) => {
                let mut h = Sha256::new();
                h.update(b"iroha:kagemusha:v1:ordinary-incoming-hash-history-fold\0");
                h.update(self.operation_id);
                h.update(self.nonce);
                h.update(self.reservation_digest);
                h.update(Sha256::digest(&self.guard_original));
                h.update(Sha256::digest(&claim.eq_proof));
                h.update(Sha256::digest(&claim.ep_proof));
                h.update(self.eq_final_history.as_bytes());
                h.update(self.ep_final_history.as_bytes());
                h.update(claim.eq_complete_history.as_bytes());
                h.update(claim.ep_complete_history.as_bytes());
                let seed = KagemushaRecoverySeedV1::from_unsealed(h.finalize().into())
                    .map_err(|e| e.to_string())?;
                let eq = fold_kagemusha_eq_accumulators_v1(
                    &self.eq_hash.carrier_parameters,
                    &self.eq_final_history,
                    &claim.eq_complete_history,
                    &seed,
                )
                .map_err(|e| e.to_string())?;
                let ep = fold_kagemusha_ep_accumulators_v1(
                    &self.ep_hash.carrier_parameters,
                    &self.ep_final_history,
                    &claim.ep_complete_history,
                    &seed,
                )
                .map_err(|e| e.to_string())?;
                let hash = claim
                    .consumer_witness(&self.eq_hash, &self.ep_hash, eq.proof(), ep.proof())
                    .map_err(|e| e.to_string())?;
                consume(
                    self.witness(Some(hash), eq.successor(), ep.successor()),
                    self.outer(),
                )?;
            }
        }
        self.recheck(selection, guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incoming_public_fold_seed_binds_each_complete_original_without_financial_authority() {
        // Public transcript data only; this does not construct Native custody or proof admission.
        let bytes = |seed: KagemushaRecoverySeedV1| {
            let mut rng = seed.rng(b"incoming-public-fold-test", &[31; 32]).unwrap();
            let mut raw = [0; 64];
            rng.fill_bytes(&mut raw);
            raw
        };
        let original = bytes(
            incoming_public_fold_seed([1; 32], [2; 32], [3; 32], b"guard", b"parent").unwrap(),
        );
        assert_eq!(
            original,
            bytes(
                incoming_public_fold_seed([1; 32], [2; 32], [3; 32], b"guard", b"parent").unwrap()
            )
        );
        for changed in [
            incoming_public_fold_seed([4; 32], [2; 32], [3; 32], b"guard", b"parent"),
            incoming_public_fold_seed([1; 32], [4; 32], [3; 32], b"guard", b"parent"),
            incoming_public_fold_seed([1; 32], [2; 32], [4; 32], b"guard", b"parent"),
            incoming_public_fold_seed([1; 32], [2; 32], [3; 32], b"guarD", b"parent"),
            incoming_public_fold_seed([1; 32], [2; 32], [3; 32], b"guard", b"parenT"),
        ] {
            assert_ne!(original, bytes(changed.unwrap()));
        }
    }
}
