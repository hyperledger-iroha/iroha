//! Test-only access to the existing verified sender journal and envelope owners.
//!
//! The genuine proof corridor supplies the same opaque preparation capability, original
//! request and paired proofs as the native owner. These methods introduce no verifier,
//! hardware authority, journal shortcut or accepting production entry point.

use super::*;

impl<R, G, H> KagemushaStateMachineV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    /// Persist one exact indexed Send candidate only after the existing proof verification.
    ///
    /// # Errors
    ///
    /// Retains the canonical stage, capability, proof, reservation, capacity and journal
    /// refusals. A failed verification installs neither the candidate nor any successor.
    pub(crate) fn diagnostic_persist_outgoing_send_candidate(
        &mut self,
        capability: &KagemushaOutgoingCommitCapabilityV1,
        candidate_proof: KagemushaPairedProofV1,
    ) -> Result<PersistedOutgoingCandidateV1, KagemushaStateErrorV1> {
        let prepared = match self.outgoing_candidate_journal.stage() {
            KagemushaOutgoingJournalStageV1::Prepared(prepared) => prepared.clone(),
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        capability.authorizes(&prepared)?;
        let candidate = PersistedOutgoingCandidateV1::verify_and_persist_send(
            prepared,
            candidate_proof,
            self.proof_release.artifacts,
            &self.recursive_verifier,
        )?;
        self.persist_verified_outgoing_candidate(candidate)
    }

    /// Finalize the original committed sender payment through its exact existing owner.
    ///
    /// # Errors
    ///
    /// Refuses a wrong phase, substituted request/payment/proof, or unbounded retry metadata.
    /// The existing envelope owner verifies against its retained request and proof pins;
    /// installation preserves the canonical retry journal and outbox capacity transaction.
    pub(crate) fn diagnostic_finalize_outgoing_payment(
        &mut self,
        request: &KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        retry_metadata: Vec<u8>,
    ) -> Result<DurableOutgoingEnvelopeV1, KagemushaStateErrorV1> {
        let committed = self.committed_candidate_for_finalization()?;
        let PreparedOutgoingRecoveryViewV1::Send {
            request: original_request,
            ..
        } = committed.candidate.prepared.recovery_view()
        else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        if request != original_request {
            return Err(KagemushaStateErrorV1::InvalidPeerCredit);
        }
        let finalized = DurableOutgoingEnvelopeV1::finalize_payment(
            committed,
            payment,
            retry_metadata,
            self.proof_release.artifacts,
            &self.recursive_verifier,
        )?;
        self.install_finalized_outgoing_envelope(finalized)
    }

    /// Project the exact retained response pins without creating another release authority.
    ///
    /// The policy digest is the canonical enabled-profile set identity, distinct from the
    /// provider registry root in State. The report and key come from this machine's admitted
    /// profile and original OEM credential; no caller-supplied profile can replace them.
    ///
    /// # Errors
    ///
    /// Refuses a foreign State/release context, missing enabled profile or invalid OEM binding.
    pub(crate) fn diagnostic_release_response_context(
        &self,
    ) -> Result<(DigestV1, DigestV1, KagemushaDevicePublicKeyV1), KagemushaStateErrorV1> {
        self.recovery_metadata
            .accepted_credential
            .validate_current(&self.state, &self.proof_release)?;
        let credential = self.recovery_metadata.accepted_credential.oem_original()?;
        let profile = self
            .proof_release
            .enabled_profile(credential.hardware_profile_id)
            .ok_or(KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        let policy = iroha_data_model::kagemusha::kagemusha_hardware_policy_digest_v1(
            &self.proof_release.enabled_profiles,
        )
        .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        Ok((
            policy,
            profile.hardware_profile.qualification_report_digest,
            credential.device_public_key,
        ))
    }

    /// Release one installed Send only after its original signed ACK and device op12 reply.
    ///
    /// The proof fixture's simulated provider must sign under the Core and device keys held
    /// since bootstrap. Canonical command, ACK and device-response owners verify the same
    /// transcripts as native release; this test-only bridge grants no native qualification.
    ///
    /// # Errors
    ///
    /// Refuses wrong phase, substituted original inputs/envelope/ACK/keys, invalid signatures,
    /// or a reply that does not retain every original operation anchor. Every check precedes
    /// the existing atomic journal/index/capacity release, leaving no partial effect on refusal.
    pub(crate) fn diagnostic_release_outgoing_payment(
        &mut self,
        operation_id: DigestV1,
        canonical_command: &[u8],
        original_response: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        use crate::kagemusha_sender_wire::{
            SENDER_REPLY_MAX_BYTES_V1, SenderCommandBodyV1, SenderCommandV1,
            SenderHardwareAuthorizationV1, SenderPhaseV1, SenderReplyBodyV1, SenderReplyV1,
            SenderTerminalReceiptV1, SenderWalletContextV1, acknowledgement_digest_v1,
        };

        let command = SenderCommandV1::decode_canonical_exact(12, operation_id, canonical_command)
            .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        let SenderCommandBodyV1::Release {
            inputs_digest,
            envelope_digest,
            envelope,
            terminal_receipt: SenderTerminalReceiptV1::PaymentAcknowledgement(ack),
            hardware_authorization,
            ..
        } = &command.body
        else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        let original_envelope = self.original_terminal_envelope(operation_id)?;
        let record = self
            .outgoing_operation_index()
            .lookup(operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let actual = self
            .outgoing_candidate_journal
            .finalized_envelope(record.outbox_reservation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let KagemushaOutgoingEnvelopeV1::Payment(payment) = &actual.envelope else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        let prepared = &actual.committed.candidate.prepared;
        let PreparedOutgoingRecoveryViewV1::Send { request, .. } = prepared.recovery_view() else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(ack, request, payment)
            .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
        let receipt_digest = acknowledgement_digest_v1(ack)
            .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
        let authorization =
            SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)
                .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        let terminal = actual.committed.candidate.hardware_terminal_body()?;
        if record.phase != KagemushaOutgoingOperationPhaseV1::Installed
            || command.context != record.context
            || inputs_digest != &record.inputs_digest
            || envelope_digest != &actual.envelope_digest
            || envelope != &original_envelope
            || envelope != &actual.canonical_envelope_bytes
            || record.envelope_digest != Some(actual.envelope_digest)
            || record.context.core_authorization_key_reference
                != self.enrollment_binding().core_authorization_key_reference
            || authorization.hardware_transition_statement != prepared.hardware_statement()
            || authorization.preparation_id != prepared.preparation_id
            || authorization.prepared_one_use_authorization_digest
                != prepared.prepared_one_use_authorization_digest
            || authorization.outbox_reservation_commitment != terminal.outbox_reservation_commitment
            || authorization.terminal_receipt_digest != Some(receipt_digest)
            || authorization.hardware_one_use_nonce == [0; 32]
        {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        let (policy, report, device_key) = self.diagnostic_release_response_context()?;
        let credential = self.recovery_metadata.accepted_credential.oem_original()?;
        let response = iroha_data_model::kagemusha::kagemusha_verify_device_response_v1(
            original_response,
            canonical_command,
            12,
            operation_id,
            policy,
            report,
            &device_key,
        )
        .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        let reply: SenderReplyV1 = norito::decode_canonical_with_limits(
            response.payload,
            norito::DecodeLimits::new(
                SENDER_REPLY_MAX_BYTES_V1,
                SENDER_REPLY_MAX_BYTES_V1,
                SENDER_REPLY_MAX_BYTES_V1 * 4,
                SENDER_REPLY_MAX_BYTES_V1 * 8,
                32,
            ),
        )
        .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        let current_context = SenderWalletContextV1 {
            lane: self.state.lane.clone(),
            release: self.state.context(),
            credential_id: credential.credential_id,
            hardware_epoch: self.state.hardware_epoch,
            device_policy_binding: self.state.device_policy_binding,
            core_authorization_key_reference: self
                .enrollment_binding()
                .core_authorization_key_reference,
        };
        reply
            .validate_against(&command, &current_context)
            .map_err(|_| KagemushaStateErrorV1::HardwareCertificateMismatch)?;
        let SenderReplyBodyV1::Lookup(Some(item)) = &reply.body else {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        };
        let item = &item.record;
        let released_revision = self
            .outgoing_operation_index()
            .revision()
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if reply.index_revision != released_revision
            || item.record_revision != released_revision
            || item.phase != SenderPhaseV1::Released
            || item.operation_id != record.operation_id
            || item.context != record.context
            || item.inputs_digest != record.inputs_digest
            || item.operation_kind != record.operation_kind
            || item.preparation_id != record.preparation_id
            || item.outbox_reservation_id != record.outbox_reservation_id
            || item.outcome_id != record.outcome_id
            || item.candidate_digest != record.candidate_digest
            || item.commit_certificate_digest != record.commit_certificate_digest
            || item.envelope_digest != record.envelope_digest
            || item.terminal_receipt_digest != Some(receipt_digest)
        {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        let reservation_id = record.outbox_reservation_id;
        let original_digest = actual.envelope_digest;
        self.outgoing_candidate_journal.release_verified_terminal(
            &mut self.sender_outbox_capacity,
            reservation_id,
            original_digest,
            receipt_digest,
        )
    }
}
