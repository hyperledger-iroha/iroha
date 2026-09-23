//! Purpose-bound final-promotion signing with immutable staging and authoritative completion.
//!
//! Canonical statement preparation precedes provider I/O. The audit predecessor comes from a
//! fresh authoritative signing snapshot, never a caller or constructor cache. All four signatures
//! remain private until the journal and completed operation are rechecked. Recovery performs no
//! protected key operation; its native source must still sign and fund fresh observer Checks.
//! TODO: Wire this producer through a configured signer and durable finalized-operation
//! adapters and the production command. Injected test providers do not qualify deployment custody.

use super::journal::{SignerReceiptJournalErrorV1, SignerReceiptJournalV1, SignerReceiptPurposeV1};
use super::*;
use sorafs_manifest::signer::{
    final_promotion::{
        SIGNER_FINAL_PROMOTION_RECEIPT_MAGIC_V1, SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1,
        SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1, SignerFinalPromotionExpectedV1,
        SignerFinalPromotionReceiptErrorV1, SignerFinalPromotionReceiptV1,
        SignerFinalPromotionRequestV1, signer_final_promotion_audit_v1,
        signer_final_promotion_digest_v1, signer_final_promotion_response_digest_v1,
        statement::prepare_final_promotion_statement_v1, validate_final_promotion_signatures_v1,
    },
    protocol::{
        SignerKeyAlgorithmV1, SignerOperationSignatureV1, SignerPurposeBindingV1, SignerRoleV1,
        signer_operation_message_digest_v1, signer_operation_signatures_digest_v1,
    },
    receipt::{SignerOperationProvenanceV1, SignerReceiptErrorV1},
};
use zeroize::Zeroize as _;

/// Secret-free failure of reviewed final-promotion signing or completed-receipt recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerFinalPromotionErrorV1 {
    /// Another signing or recovery lifecycle owns this service; no I/O was attempted.
    Busy,
    /// A previous lifecycle panicked; this service remains unavailable without retrying I/O.
    Poisoned,
    /// Canonical statement, purpose or public receipt verification failed.
    Receipt(SignerFinalPromotionReceiptErrorV1),
    /// Configured signing, custody or authoritative operation-state verification failed.
    Operation(SignerOperationErrorV1),
    /// Private journal identity, ownership, bounds or immutable staging failed.
    Journal,
}
impl fmt::Display for SignerFinalPromotionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Busy => "final promotion operation already in progress",
            Self::Poisoned => "final promotion lifecycle unavailable",
            Self::Receipt(_) => "final promotion receipt rejected",
            Self::Operation(_) => "final promotion signer operation rejected",
            Self::Journal => "final promotion private journal unavailable",
        })
    }
}
impl std::error::Error for SignerFinalPromotionErrorV1 {}
impl From<SignerFinalPromotionReceiptErrorV1> for SignerFinalPromotionErrorV1 {
    fn from(error: SignerFinalPromotionReceiptErrorV1) -> Self {
        Self::Receipt(error)
    }
}
impl From<SignerReceiptErrorV1> for SignerFinalPromotionErrorV1 {
    fn from(error: SignerReceiptErrorV1) -> Self {
        Self::Receipt(SignerFinalPromotionReceiptErrorV1::Operation(error))
    }
}
impl From<SignerOperationErrorV1> for SignerFinalPromotionErrorV1 {
    fn from(error: SignerOperationErrorV1) -> Self {
        Self::Operation(error)
    }
}
impl From<SignerReceiptJournalErrorV1> for SignerFinalPromotionErrorV1 {
    fn from(_: SignerReceiptJournalErrorV1) -> Self {
        Self::Journal
    }
}

/// One independently reviewed final-promotion statement and its complete durable receipt.
///
/// The service owns only public configuration and opaque operations. Its state source must
/// authenticate the exact staged receipt before completion and retain failed-operation tombstones.
/// Signing and recovery have one nonblocking gate held through their final receipt recheck.
/// Future explicit submission reconciliation must enter this same gate before any I/O.
pub struct SignerFinalPromotionServiceV1 {
    coordinator: SignerOperationCoordinatorV1,
    core: Arc<receipt_core::ReceiptCore>,
}
/// Immutable native account transactions bound to executed receipt and account Checks.
pub mod account_transaction;
/// Finalized role-14 Current Check handoff to one custody and audit observation.
pub mod current_observation;
/// Exact observer Check signing with retained native challenges and independent fee approval.
pub mod observer_transaction;
mod receipt_core;

/// Receipt-only recovery capability sharing the exact journal lease and lifecycle gate.
/// It exposes no signing method or protected provider. Native standalone recovery assembly
/// supplies only receipt observation, independently configured observer signing and spending.
pub struct SignerFinalPromotionRecoveryServiceV1 {
    core: Arc<receipt_core::ReceiptCore>,
}
impl fmt::Debug for SignerFinalPromotionRecoveryServiceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerFinalPromotionRecoveryServiceV1")
            .finish_non_exhaustive()
    }
}
impl SignerFinalPromotionRecoveryServiceV1 {
    /// Authenticate and recover the original completed receipt under fresh observer Checks.
    ///
    /// # Errors
    /// Rejects concurrent ownership, invalid receipts, unavailable observations or revoked custody.
    pub fn recover(&self) -> Result<SignerFinalPromotionReceiptV1, SignerFinalPromotionErrorV1> {
        self.core.recover()
    }
}

impl fmt::Debug for SignerFinalPromotionServiceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerFinalPromotionServiceV1")
            .finish_non_exhaustive()
    }
}
impl SignerFinalPromotionServiceV1 {
    /// Pin one immutable statement, its independently reviewed coordinates and its receipt journal.
    ///
    /// # Errors
    /// Rejects malformed or mismatched statements before source/provider I/O, then verifies custody.
    pub fn new(
        coordinator: SignerOperationCoordinatorV1,
        expected: SignerFinalPromotionExpectedV1,
        statement: Arc<[u8]>,
        journal: SignerReceiptJournalV1,
    ) -> Result<Self, SignerFinalPromotionErrorV1> {
        let core = receipt_core::ReceiptCore::new(
            coordinator.binding.clone(),
            coordinator.record.clone(),
            coordinator.trust.clone(),
            Arc::new(Arc::clone(&coordinator.source)),
            expected,
            statement,
            journal,
        )?;
        Ok(Self {
            coordinator,
            core: Arc::new(core),
        })
    }

    /// Obtain a recovery-only view of this exact receipt owner without creating another lease.
    #[must_use]
    pub fn recovery_service(&self) -> SignerFinalPromotionRecoveryServiceV1 {
        SignerFinalPromotionRecoveryServiceV1 {
            core: Arc::clone(&self.core),
        }
    }

    /// Sign the pinned statement and release one durably completed four-signature receipt.
    ///
    /// # Errors
    /// Busy or poisoned lifecycle ownership fails before I/O. Custody/operation changes fail
    /// closed; later failures retain replay tombstones.
    pub fn sign(&self) -> Result<SignerFinalPromotionReceiptV1, SignerFinalPromotionErrorV1> {
        let _lifecycle = self.core.enter_lifecycle()?;
        let message = self.core.statement.as_ref();
        let prepared = prepare_final_promotion_statement_v1(message, &self.coordinator.binding)
            .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidStatement)?;
        let snapshot = self
            .coordinator
            .source
            .observe_signing_state(&self.coordinator.binding)?;
        let custody = self.coordinator.verify(&snapshot.custody)?;
        let request = SignerFinalPromotionRequestV1::new(&custody, &self.core.expected, &prepared)?;
        let intent = SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: self.core.expected.operation_id,
            request_digest: request.digest()?,
            previous_audit: snapshot.audit_head,
        };
        let mut operation = self.coordinator.begin(intent)?;
        if SignerOperationCustodyV1::from_verified(&operation.custody) != request.original_custody {
            return Err(SignerOperationErrorV1::CustodyChanged.into());
        }
        let mut signatures = WorkingSignatures(Vec::with_capacity(4));
        signatures.push(
            &mut operation,
            SignerKeyOperationPurposeV1::RolePayload,
            message,
        )?;
        let audit = signer_final_promotion_audit_v1(
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
            response_digest: signer_final_promotion_response_digest_v1(
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
        let candidate = PendingReceipt(SignerFinalPromotionReceiptV1 {
            magic: SIGNER_FINAL_PROMOTION_RECEIPT_MAGIC_V1,
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
                .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt)?,
        );
        drop(candidate);
        let staged = self
            .core
            .journal
            .stage(self.core.expected.operation_id, &encoded)?;
        let completed = operation.finish(commitment)?;
        staged.recheck()?;
        self.core.revalidate_completed(&intent, &completed)?;
        staged.recheck()?;
        decode_receipt(staged.bytes())
    }

    /// Recover the exact completed receipt without Reserve, Complete or protected key operations.
    /// The native source still requires fresh observer-signed custody Checks and approved fees.
    ///
    /// # Errors
    /// Rejects busy or poisoned lifecycle ownership before I/O, incomplete/tampered receipts,
    /// current custody changes or changed original completion.
    pub fn recover(&self) -> Result<SignerFinalPromotionReceiptV1, SignerFinalPromotionErrorV1> {
        self.core.recover()
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
struct PendingReceipt(SignerFinalPromotionReceiptV1);
impl Drop for PendingReceipt {
    fn drop(&mut self) {
        for signature in &mut self.0.signatures {
            signature.signature.zeroize();
        }
    }
}
fn decode_receipt(
    bytes: &[u8],
) -> Result<SignerFinalPromotionReceiptV1, SignerFinalPromotionErrorV1> {
    if bytes.is_empty() || bytes.len() > SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1 {
        return Err(SignerFinalPromotionReceiptErrorV1::InvalidReceipt.into());
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            16 * 1024,
            SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1,
            8192,
            512 * 1024,
            24,
        ),
    )
    .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt.into())
}
