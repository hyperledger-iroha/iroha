//! Exact reviewed release-manifest signing through opaque operations and durable private receipts.
//!
//! The reviewed manifest and audit predecessor are pinned at construction, never accepted from
//! signing request fields. Every signature uses the shared purpose-specific canonical contract.
//! The complete receipt is durably staged before authoritative completion and stays internal until
//! both journal identity and fresh completed custody are rechecked. Recovery never invokes hardware.
//! TODO: Wire this producer into the canonical runtime/CLI contract and real hardware/finalized
//! state adapters before retiring all remaining software service paths. This is not deployment
//! qualification, and its injected test providers cannot qualify production custody.

use super::*;
use sorafs_manifest::signer::{
    protocol::{
        SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1, SignerKeyAlgorithmV1, SignerOperationSignatureV1,
        SignerPurposeBindingV1, SignerRoleV1, signer_operation_message_digest_v1,
        signer_operation_signatures_digest_v1,
    },
    receipt::{
        SIGNER_RELEASE_MANIFEST_RECEIPT_MAGIC_V1, SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1,
        SignerOperationProvenanceV1, SignerReceiptErrorV1, SignerReleaseManifestExpectedV1,
        SignerReleaseManifestReceiptV1, SignerReleaseManifestRequestV1,
        signer_release_manifest_audit_v1, signer_release_manifest_response_digest_v1,
        validate_release_manifest_signatures_v1,
    },
};
use zeroize::Zeroize as _;
mod journal;
pub use journal::SignerReleaseManifestJournalV1;

/// Secret-free failures of exact release signing and durable recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerReleaseManifestErrorV1 {
    /// Reviewed manifest, operation, purpose or public receipt bindings failed.
    Receipt(SignerReceiptErrorV1),
    /// Independent custody, opaque key operation or authoritative reservation/commit failed.
    Operation(SignerOperationErrorV1),
    /// Durable journal bounds, permissions, identity, immutability or I/O failed.
    Journal,
}
impl fmt::Display for SignerReleaseManifestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Receipt(_) => "release manifest receipt binding rejected",
            Self::Operation(_) => "release manifest hardware operation rejected",
            Self::Journal => "release manifest private journal unavailable",
        })
    }
}
impl std::error::Error for SignerReleaseManifestErrorV1 {}
impl From<SignerReceiptErrorV1> for SignerReleaseManifestErrorV1 {
    fn from(error: SignerReceiptErrorV1) -> Self {
        Self::Receipt(error)
    }
}
impl From<SignerOperationErrorV1> for SignerReleaseManifestErrorV1 {
    fn from(error: SignerOperationErrorV1) -> Self {
        Self::Operation(error)
    }
}

/// One independently reviewed aggregate-manifest signing ceremony.
///
/// This owns no key or wrapping secret. The configured state source must verify the exact staged
/// journal before acknowledging its completion CAS; it must retain immutable completed rows and
/// failed reservation tombstones. The journal never substitutes for that authoritative state.
pub struct SignerReleaseManifestServiceV1 {
    coordinator: SignerOperationCoordinatorV1,
    expected: SignerReleaseManifestExpectedV1,
    previous_audit: SignerOperationAuditHeadV1,
    journal: SignerReleaseManifestJournalV1,
}
impl fmt::Debug for SignerReleaseManifestServiceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerReleaseManifestServiceV1")
            .finish_non_exhaustive()
    }
}
impl SignerReleaseManifestServiceV1 {
    /// Bind an independently configured coordinator, reviewed exact manifest and private journal.
    ///
    /// # Errors
    /// Rejects non-ReleaseManifest custody, invalid reviewed coordinates or unavailable fresh state.
    pub fn new(
        coordinator: SignerOperationCoordinatorV1,
        expected: SignerReleaseManifestExpectedV1,
        previous_audit: SignerOperationAuditHeadV1,
        journal: SignerReleaseManifestJournalV1,
    ) -> Result<Self, SignerReleaseManifestErrorV1> {
        if coordinator.binding.role != SignerRoleV1::ReleaseManifest
            || !matches!(
                coordinator.binding.purpose,
                SignerPurposeBindingV1::ReleaseManifest { .. }
            )
            || coordinator.binding.algorithm != SignerKeyAlgorithmV1::Ed25519
        {
            return Err(SignerReceiptErrorV1::WrongPurpose.into());
        }
        if expected.operation_id == [0; 32]
            || expected.manifest_digest == [0; 32]
            || expected.manifest_size == 0
            || expected.manifest_size > SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1 as u64
            || previous_audit.sequence == u64::MAX
            || (previous_audit.sequence == 0) != (previous_audit.digest == [0; 32])
        {
            return Err(SignerReceiptErrorV1::InvalidReceipt.into());
        }
        coordinator.verify(&coordinator.source.observe(&coordinator.binding)?)?;
        Ok(Self {
            coordinator,
            expected,
            previous_audit,
            journal,
        })
    }

    /// Sign only the constructor-pinned reviewed bytes and release one fully committed receipt.
    ///
    /// # Errors
    /// Rejects input drift before any provider call; every later failure retains the reservation
    /// tombstone and keeps signatures internal. No failed signing attempt is silently retried.
    pub fn sign(
        &self,
        manifest: &[u8],
    ) -> Result<SignerReleaseManifestReceiptV1, SignerReleaseManifestErrorV1> {
        let custody = self
            .coordinator
            .verify(&self.coordinator.source.observe(&self.coordinator.binding)?)?;
        let request = SignerReleaseManifestRequestV1::new(&custody, &self.expected, manifest)?;
        let intent = SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: self.expected.operation_id,
            request_digest: request.digest()?,
            previous_audit: self.previous_audit,
        };
        let mut operation = self.coordinator.begin(intent)?;
        if SignerOperationCustodyV1::from_verified(&operation.custody) != request.original_custody {
            return Err(SignerOperationErrorV1::CustodyChanged.into());
        }
        let mut signatures = WorkingSignatures(Vec::with_capacity(4));
        signatures.push(
            &mut operation,
            SignerKeyOperationPurposeV1::RolePayload,
            manifest,
        )?;
        let audit = signer_release_manifest_audit_v1(
            &request,
            &intent,
            operation.reservation,
            &signatures.0[0].signature,
        )?;
        signatures.push(
            &mut operation,
            SignerKeyOperationPurposeV1::AuditRecord,
            &audit.signing_message(),
        )?;
        let provenance = SignerOperationProvenanceV1 {
            original_custody: request.original_custody,
            signing_anchor: operation.custody.current_anchor(),
            intent_digest: operation.intent_digest,
            reservation: operation.reservation,
            audit,
        };
        signatures.push(
            &mut operation,
            SignerKeyOperationPurposeV1::Provenance,
            &provenance.signing_message()?,
        )?;
        let commitment = SignerOperationCommitmentV1 {
            audit,
            response_digest: signer_release_manifest_response_digest_v1(
                &request,
                &provenance,
                &signatures.0,
            )?,
        };
        signatures.push(
            &mut operation,
            SignerKeyOperationPurposeV1::Response,
            &commitment.response_signing_message(),
        )?;
        let candidate = PendingReceipt(SignerReleaseManifestReceiptV1 {
            magic: SIGNER_RELEASE_MANIFEST_RECEIPT_MAGIC_V1,
            version: 1,
            custody_record: self.coordinator.record.clone(),
            request,
            intent,
            reservation: operation.reservation,
            provenance,
            commitment,
            signatures: std::mem::take(&mut signatures.0),
        });
        let encoded = Zeroizing::new(
            norito::encode_canonical(&candidate.0)
                .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)?,
        );
        drop(candidate); // All additional unreleased in-memory signature copies are scrubbed.
        let staged = self.journal.stage(self.expected.operation_id, &encoded)?;
        let completed = operation.finish(commitment)?;
        staged.recheck()?;
        self.revalidate_completed(&intent, &completed)?;
        decode_receipt(staged.bytes())
    }

    /// Recover the exact staged receipt only if authoritative state proves its timely completion.
    ///
    /// # Errors
    /// Rejects incomplete reservations, altered journal bytes, payload drift, replaced/revoked
    /// custody or any changed original completion. Never reserves, commits or invokes hardware.
    pub fn recover(
        &self,
        manifest: &[u8],
    ) -> Result<SignerReleaseManifestReceiptV1, SignerReleaseManifestErrorV1> {
        let custody = self
            .coordinator
            .verify(&self.coordinator.source.observe(&self.coordinator.binding)?)?;
        let expected_request =
            SignerReleaseManifestRequestV1::new(&custody, &self.expected, manifest)?;
        let staged = self.journal.recover(self.expected.operation_id)?;
        let candidate = PendingReceipt(decode_receipt(staged.bytes())?);
        let receipt = &candidate.0;
        if receipt.magic != SIGNER_RELEASE_MANIFEST_RECEIPT_MAGIC_V1
            || receipt.version != 1
            || receipt.custody_record != self.coordinator.record
            || receipt.request != expected_request
            || receipt.intent.previous_audit != self.previous_audit
            || receipt.signatures.len() != 4
        {
            return Err(SignerReceiptErrorV1::InvalidReceipt.into());
        }
        validate_release_manifest_signatures_v1(
            receipt,
            manifest,
            &receipt.signatures[0].signature,
            &self.expected,
            &custody,
        )?;
        let audit = receipt.commitment.audit;
        let audit_message = audit.signing_message();
        let provenance_message = receipt.provenance.signing_message()?;
        let response_message = receipt.commitment.response_signing_message();
        let mut recovered = Vec::with_capacity(4);
        for (signature, (purpose, message)) in receipt.signatures.iter().zip([
            (SignerKeyOperationPurposeV1::RolePayload, manifest),
            (
                SignerKeyOperationPurposeV1::AuditRecord,
                audit_message.as_slice(),
            ),
            (
                SignerKeyOperationPurposeV1::Provenance,
                provenance_message.as_slice(),
            ),
            (
                SignerKeyOperationPurposeV1::Response,
                response_message.as_slice(),
            ),
        ]) {
            if signature.purpose != purpose
                || signature.message_digest != signer_operation_message_digest_v1(message)
            {
                return Err(SignerReceiptErrorV1::InvalidSignature.into());
            }
            recovered.push(RecoveredSignerSignatureV1 {
                purpose,
                message: Zeroizing::new(message.to_vec()),
                signature: Zeroizing::new(signature.signature.clone()),
            });
        }
        let completed = self
            .coordinator
            .recover_completed(RecoveredSignerOperationV1 {
                original_custody: receipt.request.original_custody,
                intent: receipt.intent,
                reservation: receipt.reservation,
                commitment: receipt.commitment,
                signatures: recovered,
            })?;
        if signer_operation_signatures_digest_v1(&receipt.signatures)
            .map_err(|_| SignerReceiptErrorV1::InvalidSignature)?
            != completed.signatures_digest()
        {
            return Err(SignerReceiptErrorV1::InvalidSignature.into());
        }
        staged.recheck()?;
        self.revalidate_completed(&receipt.intent, &completed)?;
        drop(candidate);
        decode_receipt(staged.bytes())
    }
    fn revalidate_completed(
        &self,
        intent: &SignerOperationIntentV1,
        completed: &CompletedSignerOperationV1,
    ) -> Result<(), SignerReleaseManifestErrorV1> {
        let commit = SignerOperationCommitRequestV1 {
            check: SignerOperationReservationCheckV1 {
                request: SignerOperationReservationRequestV1 {
                    intent,
                    intent_digest: completed.intent_digest,
                    custody: &completed.custody,
                },
                reservation: completed.reservation,
            },
            commitment: completed.commitment,
            signatures_digest: completed.signatures_digest,
            original_custody: completed.original_custody,
        };
        let current = self
            .coordinator
            .verify(&self.coordinator.source.observe_committed(&commit)?)?;
        if !current.continues_active_state(&completed.custody) {
            return Err(SignerOperationErrorV1::CustodyChanged.into());
        }
        Ok(())
    }
}

struct WorkingSignatures(Vec<SignerOperationSignatureV1>);
impl WorkingSignatures {
    fn push(
        &mut self,
        operation: &mut SignerOperationV1<'_>,
        purpose: SignerKeyOperationPurposeV1,
        message: &[u8],
    ) -> Result<(), SignerOperationErrorV1> {
        let signature = operation.sign(purpose, message)?;
        self.0.push(SignerOperationSignatureV1 {
            purpose,
            message_digest: signer_operation_message_digest_v1(message),
            signature: signature.to_vec(),
        });
        Ok(())
    }
}
impl Drop for WorkingSignatures {
    fn drop(&mut self) {
        for signature in &mut self.0 {
            signature.signature.zeroize();
        }
    }
}
struct PendingReceipt(SignerReleaseManifestReceiptV1);
impl Drop for PendingReceipt {
    fn drop(&mut self) {
        for signature in &mut self.0.signatures {
            signature.signature.zeroize();
        }
    }
}
fn decode_receipt(
    bytes: &[u8],
) -> Result<SignerReleaseManifestReceiptV1, SignerReleaseManifestErrorV1> {
    if bytes.is_empty() || bytes.len() > SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1 {
        return Err(SignerReceiptErrorV1::InvalidReceipt.into());
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            16 * 1024,
            SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1,
            8192,
            512 * 1024,
            24,
        ),
    )
    .map_err(|_| SignerReceiptErrorV1::InvalidReceipt.into())
}
