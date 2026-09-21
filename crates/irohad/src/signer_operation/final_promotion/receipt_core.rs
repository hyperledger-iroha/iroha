//! One provider-free receipt owner and canonical recovery path for signing and recovery views.

use super::*;
use std::sync::{Mutex, MutexGuard, TryLockError};

pub(super) struct ReceiptCore {
    pub(super) binding: SignerCustodyBindingV1,
    pub(super) record: Vec<u8>,
    pub(super) trust: SignerCustodyTrustV1,
    source: Arc<dyn SignerOperationObservationSourceV1>,
    pub(super) expected: SignerFinalPromotionExpectedV1,
    pub(super) statement: Arc<[u8]>,
    pub(super) journal: SignerReceiptJournalV1,
    lifecycle: Mutex<()>,
}
impl ReceiptCore {
    pub(super) fn new(
        binding: SignerCustodyBindingV1,
        record: Vec<u8>,
        trust: SignerCustodyTrustV1,
        source: Arc<dyn SignerOperationObservationSourceV1>,
        expected: SignerFinalPromotionExpectedV1,
        statement: Arc<[u8]>,
        journal: SignerReceiptJournalV1,
    ) -> Result<Self, SignerFinalPromotionErrorV1> {
        if journal.purpose() != SignerReceiptPurposeV1::FinalPromotionProvenance {
            return Err(SignerFinalPromotionErrorV1::Journal);
        }
        if binding.role != SignerRoleV1::FinalPromotionProvenance
            || !matches!(
                binding.purpose,
                SignerPurposeBindingV1::FinalPromotionProvenance { .. }
            )
            || binding.algorithm != SignerKeyAlgorithmV1::Ed25519
        {
            return Err(SignerFinalPromotionReceiptErrorV1::WrongPurpose.into());
        }
        if expected.operation_id == [0; 32]
            || expected.statement_digest == [0; 32]
            || expected.statement_size == 0
            || expected.statement_size > SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 as u64
        {
            return Err(SignerFinalPromotionReceiptErrorV1::InvalidReceipt.into());
        }
        let prepared = prepare_final_promotion_statement_v1(&statement, &binding)
            .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidStatement)?;
        if u64::try_from(prepared.len()).ok() != Some(expected.statement_size)
            || signer_final_promotion_digest_v1(prepared.message()) != expected.statement_digest
        {
            return Err(SignerFinalPromotionReceiptErrorV1::StatementMismatch.into());
        }
        let core = Self {
            binding,
            record,
            trust,
            source,
            expected,
            statement,
            journal,
            lifecycle: Mutex::new(()),
        };
        core.authority()
            .verify(&core.source.observe(&core.binding)?)?;
        Ok(core)
    }
    fn authority(&self) -> SignerOperationAuthorityV1<'_> {
        SignerOperationAuthorityV1 {
            binding: &self.binding,
            record: &self.record,
            trust: &self.trust,
            source: self.source.as_ref(),
        }
    }
    pub(super) fn enter_lifecycle(
        &self,
    ) -> Result<MutexGuard<'_, ()>, SignerFinalPromotionErrorV1> {
        match self.lifecycle.try_lock() {
            Ok(guard) => Ok(guard),
            Err(TryLockError::WouldBlock) => Err(SignerFinalPromotionErrorV1::Busy),
            Err(TryLockError::Poisoned(_)) => Err(SignerFinalPromotionErrorV1::Poisoned),
        }
    }

    pub(super) fn recover(
        &self,
    ) -> Result<SignerFinalPromotionReceiptV1, SignerFinalPromotionErrorV1> {
        let _lifecycle = self.enter_lifecycle()?;
        let message = self.statement.as_ref();
        let prepared = prepare_final_promotion_statement_v1(message, &self.binding)
            .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidStatement)?;
        let custody = self
            .authority()
            .verify(&self.source.observe(&self.binding)?)?;
        let expected_request =
            SignerFinalPromotionRequestV1::new(&custody, &self.expected, &prepared)?;
        let staged = self.journal.recover(self.expected.operation_id)?;
        let candidate = PendingReceipt(decode_receipt(staged.bytes())?);
        let receipt = &candidate.0;
        if receipt.magic != SIGNER_FINAL_PROMOTION_RECEIPT_MAGIC_V1
            || receipt.version != 1
            || receipt.custody_record != self.record
            || receipt.request != expected_request
            || receipt.signatures.len() != 4
        {
            return Err(SignerFinalPromotionReceiptErrorV1::InvalidReceipt.into());
        }
        validate_final_promotion_signatures_v1(
            receipt,
            message,
            &receipt.signatures[0].signature,
            &self.expected,
            &custody,
        )?;
        let audit_message = receipt.commitment.audit.signing_message();
        let provenance_message = receipt.provenance.signing_message()?;
        let response_message = receipt.commitment.response_signing_message();
        let mut recovered = Vec::with_capacity(4);
        for (signature, (purpose, signed)) in receipt.signatures.iter().zip([
            (SignerKeyOperationPurposeV1::RolePayload, message),
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
                || signature.message_digest != signer_operation_message_digest_v1(signed)
            {
                return Err(SignerReceiptErrorV1::InvalidSignature.into());
            }
            recovered.push(RecoveredSignerSignatureV1 {
                purpose,
                message: Zeroizing::new(signed.to_vec()),
                signature: Zeroizing::new(signature.signature.clone()),
            });
        }
        let completed = self
            .authority()
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
        staged.recheck()?;
        drop(candidate);
        decode_receipt(staged.bytes())
    }

    pub(super) fn revalidate_completed(
        &self,
        intent: &SignerOperationIntentV1,
        completed: &CompletedSignerOperationV1,
    ) -> Result<(), SignerFinalPromotionErrorV1> {
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
        let current = self.authority().verify(
            &self
                .source
                .observe_committed(&commit, SignerCommittedObservationPhaseV1::BeforeRelease)?,
        )?;
        if !current.continues_active_state(&completed.custody) {
            return Err(SignerOperationErrorV1::CustodyChanged.into());
        }
        Ok(())
    }
}
