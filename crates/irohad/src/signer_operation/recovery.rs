//! Exact committed-response recovery without re-reserving an id or invoking the provider.

use super::*;

/// Internal canonical-response reconstruction input, whose buffers scrub on every exit path.
pub(in crate::signer_operation) struct RecoveredSignerSignatureV1 {
    /// Exact ordered purpose from the canonical persisted response and audit.
    pub purpose: SignerKeyOperationPurposeV1,
    /// Exact validated original role/audit/provenance/response signing bytes.
    pub message: Zeroizing<Vec<u8>>,
    /// Exact persisted signature bytes, never a request for a new signature.
    pub signature: Zeroizing<Vec<u8>>,
}

/// Untrusted recovered metadata; it cannot itself construct a completed/releasable result.
pub(in crate::signer_operation) struct RecoveredSignerOperationV1 {
    /// Original custody identity authenticated by the durable completed row, never synthesized
    /// from the recovery coordinator's current record.
    pub original_custody: SignerOperationCustodyV1,
    /// Independently reconstructed validated request and original journal predecessor.
    pub intent: SignerOperationIntentV1,
    /// Exact original reservation coordinates authenticated against the durable completion row.
    pub reservation: SignerOperationReservationV1,
    /// Exact immutable original audit/response commitments.
    pub commitment: SignerOperationCommitmentV1,
    /// Exact ordered canonical messages/signatures, bounded before decoding by the producer.
    pub signatures: Vec<RecoveredSignerSignatureV1>,
}

impl SignerOperationCoordinatorV1 {
    /// Recover only an exact committed response, under fresh unchanged independent custody state.
    ///
    /// The service reconstructs exact messages from its canonical persisted audit/response and
    /// validated original request. This method authenticates the existing durable completion and
    /// every signature, then rechecks current custody. It never calls reserve, commit or hardware.
    pub(in crate::signer_operation) fn recover_completed(
        &self,
        recovered: RecoveredSignerOperationV1,
    ) -> Result<CompletedSignerOperationV1, SignerOperationErrorV1> {
        let intent_digest = recovered
            .intent
            .digest()
            .map_err(|_| SignerOperationErrorV1::InvalidOperation)?;
        let custody = self.verify(&self.source.observe(&self.binding)?)?;
        if recovered.original_custody != SignerOperationCustodyV1::from_verified(&custody) {
            return Err(SignerOperationErrorV1::CustodyChanged);
        }
        if matches!(
            recovered.intent.action,
            SignerOperationActionV1::ActivateCustody | SignerOperationActionV1::RevokeCustody
        ) || recovered.reservation.reservation_id == [0; 32]
            || recovered.reservation.fence == 0
            || recovered.reservation.expires_at_unix_ms <= custody.statement().issued_at_unix_ms
            || recovered.reservation.expires_at_unix_ms > custody.statement().expires_at_unix_ms
        {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        let mut operation = SignerOperationV1 {
            coordinator: self,
            intent: recovered.intent,
            intent_digest,
            reservation: recovered.reservation,
            custody,
            signatures: Vec::new(),
            poisoned: false,
        };
        if recovered.signatures.len() != operation.required_purposes().len() {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        for (part, required) in recovered
            .signatures
            .into_iter()
            .zip(operation.required_purposes())
        {
            if part.purpose != *required
                || part.message.is_empty()
                || part.message.len() > SIGNER_MAX_REQUEST_PAYLOAD_BYTES_V1
                || (part.purpose != SignerKeyOperationPurposeV1::RolePayload
                    && part.message.len() != 32)
                || part.signature.len() > SIGNER_MAX_SIGNATURE_BYTES_V1
            {
                return Err(SignerOperationErrorV1::InvalidOperation);
            }
            let signature = Zeroizing::new(
                Signature::try_from_bytes(&part.signature)
                    .map_err(|_| SignerOperationErrorV1::InvalidSignature)?,
            );
            signature
                .verify(&self.binding.public_key, &part.message)
                .map_err(|_| SignerOperationErrorV1::InvalidSignature)?;
            operation.signatures.push(StagedSignature {
                purpose: part.purpose,
                message_digest: digest_parts(
                    b"iroha.sorafs.signer.operation.message.v1",
                    &[part.message.as_slice()],
                ),
                signature: part.signature,
            });
        }
        let commitment = recovered.commitment;
        if commitment.audit.sequence != operation.intent.previous_audit.sequence + 1
            || commitment.audit.digest == [0; 32]
            || commitment.audit.digest == operation.intent.previous_audit.digest
            || commitment.response_digest == [0; 32]
        {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        for (purpose, message) in [
            (
                SignerKeyOperationPurposeV1::AuditRecord,
                commitment.audit.signing_message(),
            ),
            (
                SignerKeyOperationPurposeV1::Response,
                commitment.response_signing_message(),
            ),
        ] {
            let digest = digest_parts(b"iroha.sorafs.signer.operation.message.v1", &[&message]);
            if !operation
                .signatures
                .iter()
                .any(|signature| signature.purpose == purpose && signature.message_digest == digest)
            {
                return Err(SignerOperationErrorV1::InvalidOperation);
            }
        }
        let encoded = Zeroizing::new(
            norito::encode_canonical(
                &operation
                    .signatures
                    .iter()
                    .map(|signature| {
                        (
                            signature.purpose,
                            signature.message_digest,
                            digest_parts(
                                b"iroha.sorafs.signer.operation.signature.v1",
                                &[signature.signature.as_slice()],
                            ),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| SignerOperationErrorV1::InvalidOperation)?,
        );
        let signatures_digest = digest_parts(SIGNATURES_DOMAIN, &[encoded.as_slice()]);
        // The first authenticated completion observation happens after potentially expensive
        // signature verification, so a revocation/rotation during that work cannot release bytes.
        let request = SignerOperationCommitRequestV1 {
            check: operation.check(),
            commitment,
            signatures_digest,
            original_custody: recovered.original_custody,
        };
        let current = self.verify(
            &self
                .source
                .observe_committed(&request, SignerCommittedObservationPhaseV1::AfterCommit)?,
        )?;
        if !current.continues_active_state(&operation.custody) {
            return Err(SignerOperationErrorV1::CustodyChanged);
        }
        operation.custody = current;
        let request = SignerOperationCommitRequestV1 {
            check: operation.check(),
            commitment,
            signatures_digest,
            original_custody: recovered.original_custody,
        };
        let current = self.verify(
            &self
                .source
                .observe_committed(&request, SignerCommittedObservationPhaseV1::BeforeRelease)?,
        )?;
        if !current.continues_active_state(&operation.custody) {
            return Err(SignerOperationErrorV1::CustodyChanged);
        }
        Ok(CompletedSignerOperationV1 {
            original_custody: recovered.original_custody,
            custody: current,
            reservation: operation.reservation,
            commitment,
            intent_digest,
            signatures_digest,
            signatures: operation.signatures,
        })
    }
}
