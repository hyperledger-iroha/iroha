//! Complete pre-W1 ordinary reservation originals from an actual Main captured W2 loan.
//! Mathematical proof admission is distinct from actual Core DATA reservation/current FI effects.
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaAuthenticatedOrdinaryCashCandidateV1,
    KagemushaAuthenticatedOrdinaryPreparationGuardV1, KagemushaOrdinaryLineageOutgoingOriginalsV1,
    KagemushaOrdinaryLineageStateOriginalV1, KagemushaOrdinaryLineageStateProjectionV1,
    KagemushaOrdinaryLineageStateProofBundleV1, KagemushaOrdinaryLineageStatementOriginalV1,
    KagemushaVerifiedOrdinaryLineageReservationProofV1, ordinary_cash_carrier_budget_v1,
    verify_ordinary_lineage_reservation_v1,
};
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1;
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryFinancialHeadV1,
    KagemushaOrdinaryFinancialLineageV1, KagemushaOrdinaryLineageOperationSelectionV1,
    KagemushaOrdinaryLineageReservationV1, kagemusha_ordinary_financial_epoch_id_v1,
};

/// Exact owned public request material after genuine W2 State/Guard verification. No decoder or
/// public constructor exists. Only independently acknowledged Core DATA reservation may select W1.
pub(crate) struct GeneratedOrdinaryCashReservationOriginalsV1 {
    proof: KagemushaVerifiedOrdinaryLineageReservationProofV1,
    proof_bundle_original: Vec<u8>,
    predecessor_public_state_original: Vec<u8>,
    neutral_reservation_original: Vec<u8>,
    preparation_clock_context: KagemushaOrdinaryCashClockContextV1,
}
impl GeneratedOrdinaryCashReservationOriginalsV1 {
    pub(crate) fn proof(&self) -> &KagemushaVerifiedOrdinaryLineageReservationProofV1 {
        &self.proof
    }
    pub(crate) fn reservation(&self) -> &KagemushaOrdinaryLineageReservationV1 {
        self.proof.reservation()
    }
    pub(crate) fn proof_bundle_original(&self) -> &[u8] {
        &self.proof_bundle_original
    }
    pub(crate) fn predecessor_public_state_original(&self) -> &[u8] {
        &self.predecessor_public_state_original
    }
    pub(crate) fn neutral_reservation_original(&self) -> &[u8] {
        &self.neutral_reservation_original
    }
    /// Original Native-selected preparation context data. Core must separately authenticate its
    /// complete retained signed observations; this getter supplies no live clock grant.
    pub(crate) fn preparation_clock_context(&self) -> &KagemushaOrdinaryCashClockContextV1 {
        &self.preparation_clock_context
    }
}
impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Produce exact full Reserve material from genuine captured W2/Guard/State only. Native Main
    /// returns the actual retained predecessor, physical reservation and outgoing data internally.
    /// No offered predecessor, Model reservation, State, key, time or transport tuple enters.
    pub(crate) fn assemble_ordinary_cash_reservation(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
        candidate: &KagemushaAuthenticatedOrdinaryCashCandidateV1,
    ) -> Result<GeneratedOrdinaryCashReservationOriginalsV1, KagemushaArtifactGenerationErrorV1>
    {
        selection
            .recheck_selected_originals_and_current_custody()
            .map_err(owner_error)?;
        guard
            .recheck_preparation_selection(selection)
            .map_err(owner_error)?;
        candidate
            .recheck_preparation_selection(selection, guard)
            .map_err(owner_error)?;
        self.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        let before = selection.selected_predecessor_state();
        let after = selection.selected_successor_state();
        let normalized = selection.normalized_guard_statement();
        let predecessor_original = selection
            .selected_predecessor_public_state_original()
            .map_err(owner_error)?;
        let neutral = selection
            .outbox_reservation_original()
            .map_err(owner_error)?;
        let budget = ordinary_cash_carrier_budget_v1(&self.verifier).map_err(proving_error)?;
        budget
            .require_reserved_bytes(neutral.reserved_outbox_bytes)
            .map_err(proving_error)?;
        let neutral_original =
            norito::encode_canonical(neutral).map_err(|e| proving_error(e.to_string()))?;
        let outgoing = selection
            .outgoing_transport_originals()
            .map_err(owner_error)?;
        let output_sha = match &outgoing {
            KagemushaOrdinaryLineageOutgoingOriginalsV1::Send { output, .. } => {
                Sha256::digest(output.canonical_bytes().map_err(proving_error)?).into()
            }
            KagemushaOrdinaryLineageOutgoingOriginalsV1::Redeem { output, .. } => Sha256::digest(
                norito::encode_canonical(output).map_err(|e| proving_error(e.to_string()))?,
            )
            .into(),
        };
        let candidate_original =
            KagemushaOrdinaryLineageStateOriginalV1::from_admitted_candidate(candidate)
                .map_err(proving_error)?
                .canonical_bytes()
                .map_err(proving_error)?;
        let projection_original =
            KagemushaOrdinaryLineageStateProjectionV1::from_admitted_candidate(candidate)
                .map_err(proving_error)?
                .canonical_bytes()
                .map_err(proving_error)?;
        let bundle = KagemushaOrdinaryLineageStateProofBundleV1::from_public_parts(
            normalized.clone(),
            KagemushaOrdinaryLineageStatementOriginalV1::Outgoing(Box::new(
                selection.transition_statement().clone(),
            )),
            candidate_original.clone(),
            selection.enrollment().app_credential().original().to_vec(),
            selection.original().to_vec(),
            selection
                .original_approval_integrity_lease()
                .map(|l| l.original().to_vec()),
            guard.original().to_vec(),
            Some(candidate.prepared_record().clone()),
            Some(outgoing),
        )
        .map_err(proving_error)?
        .canonical_bytes()
        .map_err(proving_error)?;
        let c = selection.enrollment().app_credential();
        let reservation = KagemushaOrdinaryLineageReservationV1 {
            selection: KagemushaOrdinaryLineageOperationSelectionV1 {
                lineage: KagemushaOrdinaryFinancialLineageV1 {
                    version: 1,
                    owner: selection.enrollment().certificate().subject.owner.clone(),
                    financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(c.subject())
                        .map_err(proving_error)?,
                    financial_authority_commitment: c.subject().financial_authority_commitment,
                },
                operation_id: selection.challenge().operation_id,
                predecessor: KagemushaOrdinaryFinancialHeadV1 {
                    state_commitment: before.state_commitment,
                    logical_sequence: before.logical_sequence,
                    state_original_sha256: Sha256::digest(&predecessor_original).into(),
                },
                operation: normalized.operation.into(),
                amount: normalized.amount,
                scale: normalized.asset_scale,
                // Maintained model name; the actual value is the sole purpose-bound request original digest.
                receiver_request_original_sha256: candidate.prepared_record().request_digest,
                output_body_original_sha256: output_sha,
                neutral_reservation_original_sha256: Sha256::digest(&neutral_original).into(),
                neutral_reservation_digest: neutral
                    .canonical_commitment()
                    .map_err(|e| proving_error(e.to_string()))?,
            },
            successor: KagemushaOrdinaryFinancialHeadV1 {
                state_commitment: after.state_commitment,
                logical_sequence: after.logical_sequence,
                state_original_sha256: Sha256::digest(&candidate_original).into(),
            },
            purpose2_approval_original_sha256: Sha256::digest(selection.original()).into(),
            candidate_original_sha256: Sha256::digest(&projection_original).into(),
            proof_bundle_original_sha256: Sha256::digest(&bundle).into(),
        };
        reservation.validate_shape().map_err(proving_error)?;
        let proof = verify_ordinary_lineage_reservation_v1(
            &self.verifier,
            &reservation,
            &bundle,
            &predecessor_original,
            &neutral_original,
            c,
            selection
                .original_approval_integrity_lease()
                .map(|l| l.as_ref()),
        )
        .map_err(proving_error)?;
        candidate
            .recheck_preparation_selection(selection, guard)
            .map_err(owner_error)?;
        selection
            .recheck_selected_originals_and_current_custody()
            .map_err(owner_error)?;
        Ok(GeneratedOrdinaryCashReservationOriginalsV1 {
            proof,
            proof_bundle_original: bundle,
            predecessor_public_state_original: predecessor_original,
            neutral_reservation_original: neutral_original,
            preparation_clock_context: *selection.preparation_clock_context(),
        })
    }
}
