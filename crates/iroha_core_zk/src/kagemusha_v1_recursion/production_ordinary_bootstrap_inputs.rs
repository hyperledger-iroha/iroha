//! Native preparation of immutable, public zero-State circuit inputs.
//!
//! The public inactive slots grant no predecessor, mint, incoming payment or signature.
//! Their parser shape is bound to the selected release's actual compiled protocols. The
//! current ordinary Guard is independently verified and folded into both State histories.

use super::super::production_ordinary_auxiliaries::KagemushaRetainedOrdinaryBootstrapAuxiliariesV1;
use super::super::production_ordinary_guard::public_relation;
use super::super::production_ordinary_padding::bootstrap_mint_padding;
use super::*;
use crate::kagemusha_v1_recursion::{
    ordinary_guard_verifier::public_column,
    ordinary_state_reserved::kagemusha_ordinary_state_reserved_guard_positions_v1,
    terminal_authorization::TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
};

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Retain Native-built public operands for this exact captured ordinary Bootstrap.
    ///
    /// All keys and protocols come from the independently authenticated Native release.
    /// No real mint or incoming payment is necessary for the zero balance, and these
    /// inactive parser operands cannot pass their respective monetary verifiers. Actual
    /// financial custody is borrowed only by the subsequent genuine State prover.
    /// # Errors
    /// Rejects stale current custody, another original Guard, nonzero initial State or
    /// unexpected release protocol widths. This method never publishes a wallet.
    pub fn prepare_ordinary_bootstrap_auxiliaries(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        paired_guard_original: &[u8],
    ) -> Result<KagemushaRetainedOrdinaryBootstrapAuxiliariesV1, KagemushaArtifactGenerationErrorV1>
    {
        let originals =
            self.ordinary_state_originals(selection, approval, financial, paired_guard_original)?;
        let preview = selection.preview().map_err(owner_error)?;
        let state = &preview.state;
        if state.balance != 0
            || state.logical_sequence != 0
            || state.secure_index != 0
            || state.next_one_use_key_reference != [0; 32]
        {
            return Err(proving_error(
                "ordinary Bootstrap preview is not the zero State",
            ));
        }
        let checkpoint = self.verifier.state_checkpoint_material();
        let guard_material = self.verifier.ordinary_guard_verifier_material();
        let auxiliary_material = self
            .verifier
            .ordinary_bootstrap_auxiliary_material()
            .map_err(proving_error)?;
        let artifacts = self.artifacts.ordinary_recursion_artifacts()?;
        let eq_history = initial_kagemusha_eq_accumulator_v1(checkpoint.eq_parameters)
            .map_err(|e| proving_error(e.to_string()))?;
        let ep_history = initial_kagemusha_ep_accumulator_v1(checkpoint.ep_parameters)
            .map_err(|e| proving_error(e.to_string()))?;
        let mint = bootstrap_mint_padding(
            state,
            &selection
                .enrollment()
                .certificate()
                .subject
                .owner
                .account_id,
            checkpoint.binding.artifact_manifest_digest,
            auxiliary_material.genesis_authorization_id,
            auxiliary_material.eq_mint_authorization_protocol,
            auxiliary_material.ep_mint_authorization_protocol,
            auxiliary_material.eq_mint_protocol,
            auxiliary_material.ep_mint_protocol,
            &eq_history,
            &ep_history,
        )?;
        let authorization = &mint.authorization;
        let credit = &mint.credit;
        let eq_authorization_instances = vec![
            mint_authorization_public_instances_v1::<Fp>(
                &authorization.statement,
                authorization.proof.guard_ep_credential_audit,
                authorization.proof.eq_deferred_audit,
                authorization.proof.ep_deferred_audit,
                eq_history.as_bytes(),
            )
            .map_err(proving_error)?,
        ];
        let ep_authorization_instances = vec![
            mint_authorization_public_instances_v1::<Fq>(
                &authorization.statement,
                authorization.proof.guard_ep_credential_audit,
                authorization.proof.eq_deferred_audit,
                authorization.proof.ep_deferred_audit,
                ep_history.as_bytes(),
            )
            .map_err(proving_error)?,
        ];
        let mint_request =
            crate::kagemusha_v1_recursion::KagemushaMintFinalityHelperVerificationRequestV1 {
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
        let eq_mint_instances = vec![
            crate::kagemusha_v1_recursion::native_backend::mint_public_instances::<Fp>(
                &mint_request,
                eq_history.as_bytes(),
            )
            .map_err(proving_error)?,
        ];
        let ep_mint_instances = vec![
            crate::kagemusha_v1_recursion::native_backend::mint_public_instances::<Fq>(
                &mint_request,
                ep_history.as_bytes(),
            )
            .map_err(proving_error)?,
        ];

        let guard = &originals.guard;
        let eq_guard_history = KagemushaEqAccumulatorV1::try_from_bytes(&guard.eq_history)
            .map_err(|e| proving_error(e.to_string()))?;
        let ep_guard_history = KagemushaEpAccumulatorV1::try_from_bytes(&guard.ep_history)
            .map_err(|e| proving_error(e.to_string()))?;
        let digests = [
            guard.normalized_guard_digest,
            guard.credential_digest,
            guard.authorization_transcript_digest,
            guard.subject_signing_digest,
            guard.provider_policy_root,
        ];
        let eq_current = KagemushaEqAccumulatorV1::from_native(
            &verify_eq_succinct_protocol(
                guard_material.eq_parameters,
                guard_material.eq_protocol,
                &guard.eq_proof,
                &public_column::<Fp>(digests, &guard.eq_history),
            )
            .map_err(proving_error)?,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let ep_current = KagemushaEpAccumulatorV1::from_native(
            &verify_ep_succinct_protocol(
                guard_material.ep_parameters,
                guard_material.ep_protocol,
                &guard.ep_proof,
                &public_column::<Fq>(digests, &guard.ep_history),
            )
            .map_err(proving_error)?,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        // These operands are public. This seed binds reproducible folds to the complete
        // original Guard and canonical empty histories; it carries no financial secret.
        let mut transcript = Sha256::new();
        transcript.update(b"iroha:kagemusha:v1:ordinary-bootstrap-initial-public-fold\0");
        transcript.update(Sha256::digest(paired_guard_original));
        transcript.update(eq_history.as_bytes());
        transcript.update(ep_history.as_bytes());
        let seed = KagemushaRecoverySeedV1::from_unsealed(transcript.finalize().into())
            .map_err(|e| proving_error(e.to_string()))?;
        let eq_guard_complete = fold_kagemusha_eq_accumulators_v1(
            checkpoint.eq_parameters,
            &eq_current,
            &eq_guard_history,
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let ep_guard_complete = fold_kagemusha_ep_accumulators_v1(
            checkpoint.ep_parameters,
            &ep_current,
            &ep_guard_history,
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let eq_guard_merge = fold_kagemusha_eq_accumulators_v1(
            checkpoint.eq_parameters,
            &eq_history,
            eq_guard_complete.successor(),
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let ep_guard_merge = fold_kagemusha_ep_accumulators_v1(
            checkpoint.ep_parameters,
            &ep_history,
            ep_guard_complete.successor(),
            &seed,
        )
        .map_err(|e| proving_error(e.to_string()))?;

        let eq_point = EqAffine::generator().to_bytes();
        let ep_point = EpAffine::generator().to_bytes();
        let eq_inactive_fold = KagemushaEqFoldProofV1::try_from_bytes(
            &crate::kagemusha_v1_recursion::generation::dummy_fold_proof_bytes(eq_point.as_ref()),
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let ep_inactive_fold = KagemushaEpFoldProofV1::try_from_bytes(
            &crate::kagemusha_v1_recursion::generation::dummy_fold_proof_bytes(ep_point.as_ref()),
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let eq_parent_proof =
            crate::kagemusha_v1_recursion::generation::dummy_ordinary_proof_bytes(
                checkpoint.inner_eq_protocol,
                eq_point.as_ref(),
                KagemushaPastaParityV1::Eq,
            )?;
        let ep_parent_proof =
            crate::kagemusha_v1_recursion::generation::dummy_ordinary_proof_bytes(
                checkpoint.inner_ep_protocol,
                ep_point.as_ref(),
                KagemushaPastaParityV1::Ep,
            )?;
        let eq_incoming_proof =
            crate::kagemusha_v1_recursion::generation::dummy_ordinary_proof_bytes(
                auxiliary_material.eq_incoming_protocol,
                eq_point.as_ref(),
                KagemushaPastaParityV1::Eq,
            )?;
        let ep_incoming_proof =
            crate::kagemusha_v1_recursion::generation::dummy_ordinary_proof_bytes(
                auxiliary_material.ep_incoming_protocol,
                ep_point.as_ref(),
                KagemushaPastaParityV1::Ep,
            )?;
        let parent_width =
            crate::kagemusha_v1_recursion::generation::recursive_public_instance_count();
        let eq_parent_instances = inactive_column::<Fp>(
            &checkpoint.inner_eq_protocol.num_instance,
            parent_width,
            eq_history.as_bytes(),
        )?;
        let ep_parent_instances = inactive_column::<Fq>(
            &checkpoint.inner_ep_protocol.num_instance,
            parent_width,
            ep_history.as_bytes(),
        )?;
        let eq_incoming_instances = inactive_incoming_column::<Fp>(
            &auxiliary_material.eq_incoming_protocol.num_instance,
            artifacts.commit_wrapper_eq_protocol_digest,
            artifacts.commit_wrapper_ep_protocol_digest,
            eq_history.as_bytes(),
        )?;
        let ep_incoming_instances = inactive_incoming_column::<Fq>(
            &auxiliary_material.ep_incoming_protocol.num_instance,
            artifacts.commit_wrapper_eq_protocol_digest,
            artifacts.commit_wrapper_ep_protocol_digest,
            ep_history.as_bytes(),
        )?;
        let guard_relation = public_relation(selection)?;
        let (eq_reserved, ep_reserved) = kagemusha_ordinary_state_reserved_guard_positions_v1();
        let relation = KagemushaStateRelationWitnessV1 {
            operation: KagemushaOperationV1::Bootstrap,
            predecessor: None,
            successor: state.clone(),
            amount: 0,
            journal_revision_before: 0,
            journal_revision_after: 0,
            transition_effect_digest: preview.normalized_guard_statement.transition_effect_digest,
            mint_finality_semantic_digest: [0; 32],
            mint_finality_proof_binding_digest: [0; 32],
            peer_credit_id: [0; 32],
            recipient_encryption_key_binding: [0; 32],
            receive_credit: None,
            receive_credit_binding_digest: [0; 32],
            lifecycle_binding_digest: preview.normalized_guard_statement.lifecycle_binding_digest,
            prepared_transition_binding_digest: [0; 32],
            prepared_intent: None,
            transport_semantic_digest: preview.transport_semantic_digest,
            guard_statement_digest: preview
                .normalized_guard_statement
                .canonical_digest()
                .map_err(|e| proving_error(e.to_string()))?,
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
            replay_insert: None,
        };
        relation.validate().map_err(proving_error)?;
        let witness = KagemushaRecursiveStateGenerationWitnessV1 {
            hash_claim: None,
            state: relation,
            mint_fold_opening: None,
            mint_authorization: authorization,
            mint_credit: credit,
            guard_relation,
            hardware_selection: None,
            ordinary_selection: None,
            eq_parent_protocol: checkpoint.inner_eq_protocol,
            ep_parent_protocol: checkpoint.inner_ep_protocol,
            eq_parent_instances: &eq_parent_instances,
            ep_parent_instances: &ep_parent_instances,
            eq_parent_proof: &eq_parent_proof,
            ep_parent_proof: &ep_parent_proof,
            eq_predecessor_history: &eq_history,
            ep_predecessor_history: &ep_history,
            eq_parent_fold_proof: &eq_inactive_fold,
            ep_parent_fold_proof: &ep_inactive_fold,
            eq_incoming_protocol: auxiliary_material.eq_incoming_protocol,
            ep_incoming_protocol: auxiliary_material.ep_incoming_protocol,
            eq_incoming_credits: [KagemushaRecursiveIncomingEqGenerationWitnessV1 {
                instances: &eq_incoming_instances,
                proof: &eq_incoming_proof,
                history: &eq_history,
                history_fold_proof: &eq_inactive_fold,
                merge_fold_proof: &eq_inactive_fold,
            }],
            ep_incoming_credits: [KagemushaRecursiveIncomingEpGenerationWitnessV1 {
                instances: &ep_incoming_instances,
                proof: &ep_incoming_proof,
                history: &ep_history,
                history_fold_proof: &ep_inactive_fold,
                merge_fold_proof: &ep_inactive_fold,
            }],
            eq_successor_history: eq_guard_merge.successor(),
            ep_successor_history: ep_guard_merge.successor(),
            eq_guard_protocol: guard_material.eq_protocol,
            ep_guard_protocol: guard_material.ep_protocol,
            eq_guard_proof: &guard.eq_proof,
            ep_guard_proof: &guard.ep_proof,
            eq_guard_history: &eq_guard_history,
            ep_guard_history: &ep_guard_history,
            eq_guard_history_fold_proof: eq_guard_complete.proof(),
            ep_guard_history_fold_proof: ep_guard_complete.proof(),
            eq_guard_merge_fold_proof: eq_guard_merge.proof(),
            ep_guard_merge_fold_proof: ep_guard_merge.proof(),
            eq_mint_authorization_protocol: auxiliary_material.eq_mint_authorization_protocol,
            ep_mint_authorization_protocol: auxiliary_material.ep_mint_authorization_protocol,
            eq_mint_authorization_instances: &eq_authorization_instances,
            ep_mint_authorization_instances: &ep_authorization_instances,
            eq_mint_authorization_proof: &authorization.proof.eq_proof,
            ep_mint_authorization_proof: &authorization.proof.ep_proof,
            eq_mint_authorization_history: &eq_history,
            ep_mint_authorization_history: &ep_history,
            eq_mint_authorization_history_fold_proof: &eq_inactive_fold,
            ep_mint_authorization_history_fold_proof: &ep_inactive_fold,
            eq_mint_authorization_merge_fold_proof: &eq_inactive_fold,
            ep_mint_authorization_merge_fold_proof: &ep_inactive_fold,
            eq_mint_protocol: auxiliary_material.eq_mint_protocol,
            ep_mint_protocol: auxiliary_material.ep_mint_protocol,
            eq_mint_instances: &eq_mint_instances,
            ep_mint_instances: &ep_mint_instances,
            eq_mint_proof: &credit.proof.eq_proof,
            ep_mint_proof: &credit.proof.ep_proof,
            eq_mint_history: &eq_history,
            ep_mint_history: &ep_history,
            eq_mint_history_fold_proof: &eq_inactive_fold,
            ep_mint_history_fold_proof: &ep_inactive_fold,
            eq_mint_merge_fold_proof: &eq_inactive_fold,
            ep_mint_merge_fold_proof: &ep_inactive_fold,
        };
        let retained =
            KagemushaRetainedOrdinaryBootstrapAuxiliariesV1::retain_public_bootstrap_originals(
                witness,
                paired_guard_original.to_vec(),
                load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?,
                load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?,
            )
            .map_err(proving_error)?;
        self.recheck_ordinary_state(selection, approval, financial)?;
        Ok(retained)
    }
}

fn inactive_column<F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1>(
    actual_widths: &[usize],
    expected_width: usize,
    history: &[u8; crate::kagemusha_v1_recursion::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<Vec<F>>, KagemushaArtifactGenerationErrorV1> {
    let history_width = history.len() / 16;
    if actual_widths != [expected_width] || expected_width < history_width {
        return Err(proving_error(
            "inactive Bootstrap protocol column width differs",
        ));
    }
    let prefix = expected_width - history_width;
    let mut column = vec![F::ZERO; prefix];
    column.extend(history.chunks_exact(16).map(|bytes| {
        crate::kagemusha_v1_poseidon::from_u128::<F>(u128::from_le_bytes(
            bytes.try_into().expect("exact public history limb width"),
        ))
    }));
    Ok(vec![column])
}

// The enclosing State circuit binds both commit-wrapper identities even when the
// incoming credit selector is zero. These four public limbs therefore name the actual
// release protocols in both parities; zeroing them would make Bootstrap unsatisfiable.
fn inactive_incoming_column<F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1>(
    actual_widths: &[usize],
    eq_protocol_digest: [u8; 32],
    ep_protocol_digest: [u8; 32],
    history: &[u8; crate::kagemusha_v1_recursion::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<Vec<F>>, KagemushaArtifactGenerationErrorV1> {
    use crate::kagemusha_v1_recursion::terminal_authorization::public_instance;
    if eq_protocol_digest == [0; 32]
        || ep_protocol_digest == [0; 32]
        || eq_protocol_digest == ep_protocol_digest
    {
        return Err(proving_error(
            "inactive incoming protocol roles differ from release",
        ));
    }
    let mut columns = inactive_column::<F>(
        actual_widths,
        TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
        history,
    )?;
    let column = &mut columns[0];
    for (offset, digest) in [
        (public_instance::EQ_PROTOCOL_LO, eq_protocol_digest),
        (public_instance::EP_PROTOCOL_LO, ep_protocol_digest),
    ] {
        column[offset..offset + 2]
            .copy_from_slice(&crate::kagemusha_v1_poseidon::digest_limbs::<F>(digest));
    }
    Ok(columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_inactive_incoming<F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1>() {
        use crate::kagemusha_v1_recursion::terminal_authorization::public_instance;
        const HISTORY_BYTES: usize =
            crate::kagemusha_v1_recursion::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1;
        let mut history = [0_u8; HISTORY_BYTES];
        for (index, chunk) in history.chunks_exact_mut(16).enumerate() {
            chunk.copy_from_slice(&(index as u128 + 1).to_le_bytes());
        }
        let eq = crate::kagemusha_v1_poseidon::encode(Fp::from(7));
        let ep = crate::kagemusha_v1_poseidon::encode(Fq::from(11));
        let width = TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1;
        let columns = inactive_incoming_column::<F>(&[width], eq, ep, &history)
            .expect("release-bound inactive incoming shape");
        let column = &columns[0];
        assert_eq!(column.len(), width);
        for (offset, digest) in [
            (public_instance::EQ_PROTOCOL_LO, eq),
            (public_instance::EP_PROTOCOL_LO, ep),
        ] {
            assert_eq!(
                &column[offset..offset + 2],
                &crate::kagemusha_v1_poseidon::digest_limbs::<F>(digest),
            );
        }
        assert!(
            column[..public_instance::EQ_PROTOCOL_LO]
                .iter()
                .all(|v| *v == F::ZERO)
        );
        for index in 0..HISTORY_BYTES / 16 {
            assert_eq!(
                column[public_instance::HISTORY_START + index],
                F::from(index as u64 + 1)
            );
        }
        for (a, b) in [(eq, eq), ([0; 32], ep), (eq, [0; 32])] {
            assert!(inactive_incoming_column::<F>(&[width], a, b, &history).is_err());
        }
        assert!(inactive_incoming_column::<F>(&[width - 1], eq, ep, &history).is_err());
    }

    #[test]
    fn inactive_incoming_binds_both_release_protocol_roles_in_both_parities() {
        check_inactive_incoming::<Fp>();
        check_inactive_incoming::<Fq>();
    }

    #[test]
    fn inactive_protocol_columns_preserve_every_history_limb_and_reject_foreign_widths() {
        const HISTORY_BYTES: usize =
            crate::kagemusha_v1_recursion::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1;
        let mut history = [0_u8; HISTORY_BYTES];
        for (index, chunk) in history.chunks_exact_mut(16).enumerate() {
            chunk.copy_from_slice(&(index as u128 + 1).to_le_bytes());
        }
        let width = HISTORY_BYTES / 16 + 93;
        let fp = inactive_column::<Fp>(&[width], width, &history).expect("exact Eq shape");
        let fq = inactive_column::<Fq>(&[width], width, &history).expect("exact Ep shape");
        assert_eq!(fp[0].len(), width);
        assert_eq!(fq[0].len(), width);
        assert!(fp[0][..93].iter().all(|v| *v == Fp::ZERO));
        assert!(fq[0][..93].iter().all(|v| *v == Fq::ZERO));
        for index in 0..HISTORY_BYTES / 16 {
            assert_eq!(fp[0][93 + index], Fp::from(index as u64 + 1));
            assert_eq!(fq[0][93 + index], Fq::from(index as u64 + 1));
        }
        for actual in [vec![], vec![width - 1], vec![width + 1], vec![width, width]] {
            assert!(inactive_column::<Fp>(&actual, width, &history).is_err());
            assert!(inactive_column::<Fq>(&actual, width, &history).is_err());
        }
        assert!(inactive_column::<Fp>(&[1], 1, &history).is_err());
        assert!(inactive_column::<Fq>(&[1], 1, &history).is_err());
    }
}
