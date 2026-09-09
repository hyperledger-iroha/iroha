//! Private common operation checks for purpose-owned signer receipts.
//!
//! Derived provenance, completion and receipt types remain at their original owners so canonical
//! schema identities and signed preimages do not move. Purpose owners retain exact expected
//! payload/request validation and audit/response domains. These checks neither select trust from
//! a receipt nor return a public qualification result; completion/current state must already come
//! from an independently authenticated source and custody from its existing private verifier.

use super::{SignerCompletedOperationV1, SignerOperationProvenanceV1, SignerReceiptErrorV1};
use crate::signer::{
    custody::{SignerCustodyAnchorV1, SignerCustodyUseContextV1, VerifiedSignerCustodyV1},
    protocol::{
        SignerKeyOperationPurposeV1, SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
        SignerOperationCustodyV1, SignerOperationReservationV1, SignerOperationSignatureV1,
        signer_operation_message_digest_v1, signer_operation_signatures_digest_v1,
    },
};
use iroha_crypto::Signature;

/// Common operation coordinates borrowed from one purpose-owned, already admitted receipt.
///
/// The purpose owner compares its complete request with independently expected payload/custody
/// before signature validation. These fields have no wire implementation or verified status.
pub(in crate::signer) struct SignerOperationReceiptViewV1<'a> {
    pub operation_id: [u8; 32],
    pub original_custody: SignerOperationCustodyV1,
    pub reservation: SignerOperationReservationV1,
    pub provenance: &'a SignerOperationProvenanceV1,
    pub commitment: &'a SignerOperationCommitmentV1,
}

/// Commit the exact first three signatures after their existing count/order/size checks.
///
/// The purpose owner retains the response's request/provenance tuple and domain; this helper
/// invokes the existing signature commitment owner without introducing a new encoded wrapper.
pub(in crate::signer) fn first_three_signatures_digest(
    first_signatures: &[SignerOperationSignatureV1],
) -> Result<[u8; 32], SignerReceiptErrorV1> {
    if first_signatures.len() != 3
        || first_signatures
            .iter()
            .zip([
                SignerKeyOperationPurposeV1::RolePayload,
                SignerKeyOperationPurposeV1::AuditRecord,
                SignerKeyOperationPurposeV1::Provenance,
            ])
            .any(|(signature, purpose)| {
                signature.purpose != purpose || signature.signature.len() != 64
            })
    {
        return Err(SignerReceiptErrorV1::InvalidSignature);
    }
    signer_operation_signatures_digest_v1(first_signatures)
        .map_err(|_| SignerReceiptErrorV1::InvalidSignature)
}

/// Admit exactly four entries and bind the first to the independently supplied role signature.
///
/// Array borrowing proves cardinality before either the response's first-three slice or the
/// four-message verifier. Purpose/message/cryptographic checks retain their subsequent order.
pub(in crate::signer) fn exact_four_signatures<'a>(
    signatures: &'a [SignerOperationSignatureV1],
    detached_signature: &[u8],
) -> Result<&'a [SignerOperationSignatureV1; 4], SignerReceiptErrorV1> {
    let signatures: &[SignerOperationSignatureV1; 4] = signatures
        .try_into()
        .map_err(|_| SignerReceiptErrorV1::InvalidSignature)?;
    if signatures[0].signature != detached_signature {
        return Err(SignerReceiptErrorV1::InvalidSignature);
    }
    Ok(signatures)
}

impl SignerOperationReceiptViewV1<'_> {
    /// Compare original custody, exact intent/reservation/audit and current control-state ancestry.
    pub(in crate::signer) fn validate_provenance(
        &self,
        intent_digest: [u8; 32],
        audit: SignerOperationAuditHeadV1,
        custody: &VerifiedSignerCustodyV1,
    ) -> Result<(), SignerReceiptErrorV1> {
        if self.commitment.audit != audit
            || self.provenance.original_custody != self.original_custody
            || self.provenance.intent_digest != intent_digest
            || self.provenance.reservation != self.reservation
            || self.provenance.audit != audit
            || self.provenance.signing_anchor.state_digest
                != self.original_custody.control_state_digest
            || !anchor_descends(self.provenance.signing_anchor, custody.statement().anchor)
            || !anchor_descends(custody.current_anchor(), self.provenance.signing_anchor)
        {
            return Err(SignerReceiptErrorV1::InvalidReceipt);
        }
        Ok(())
    }

    /// Verify every exact ordered signature after purpose-owned response-digest validation.
    ///
    /// The role payload is the owner's full signing message, not a candidate-selected digest.
    /// Audit, provenance and response messages retain their existing canonical method owners.
    pub(in crate::signer) fn validate_signatures(
        &self,
        signatures: &[SignerOperationSignatureV1; 4],
        role_payload: &[u8],
        audit: SignerOperationAuditHeadV1,
        custody: &VerifiedSignerCustodyV1,
    ) -> Result<[u8; 32], SignerReceiptErrorV1> {
        let audit_message = audit.signing_message();
        let provenance_message = self.provenance.signing_message()?;
        let response_message = self.commitment.response_signing_message();
        for (signature, (purpose, message)) in signatures.iter().zip([
            (SignerKeyOperationPurposeV1::RolePayload, role_payload),
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
                || signature.signature.len() != 64
                || signature.message_digest != signer_operation_message_digest_v1(message)
            {
                return Err(SignerReceiptErrorV1::InvalidSignature);
            }
            let parsed = Signature::try_from_bytes(&signature.signature)
                .map_err(|_| SignerReceiptErrorV1::InvalidSignature)?;
            parsed
                .verify(&custody.statement().binding.public_key, message)
                .map_err(|_| SignerReceiptErrorV1::InvalidSignature)?;
        }
        let signatures_digest = signer_operation_signatures_digest_v1(signatures)
            .map_err(|_| SignerReceiptErrorV1::InvalidSignature)?;
        Ok(signatures_digest)
    }

    /// Match the independently authenticated immutable completed row and original timely finality.
    ///
    /// The original reservation may already have expired at recovery time; completion must have
    /// been timely under that exact reservation and custody. No fresh reservation or relabelled
    /// custody can replace the committed row. The original guarded subtraction order is retained.
    pub(in crate::signer) fn validate_completion(
        &self,
        completed: &SignerCompletedOperationV1,
        intent_digest: [u8; 32],
        signatures_digest: [u8; 32],
        custody: &VerifiedSignerCustodyV1,
        current: &SignerCustodyUseContextV1,
    ) -> Result<(), SignerReceiptErrorV1> {
        let reservation = self.reservation;
        let anchor = completed.anchor;
        let signing_anchor = self.provenance.signing_anchor;
        if completed.operation_id != self.operation_id
            || completed.intent_digest != intent_digest
            || completed.original_custody != self.original_custody
            || completed.reservation != reservation
            || completed.commitment != *self.commitment
            || completed.signatures_digest != signatures_digest
            || reservation.reservation_id == [0; 32]
            || reservation.fence == 0
            || reservation.expires_at_unix_ms > custody.statement().expires_at_unix_ms
            || completed.completed_at_unix_ms < custody.statement().issued_at_unix_ms
            || completed.completed_at_unix_ms > current.now_unix_ms
            || completed.completed_at_unix_ms >= reservation.expires_at_unix_ms
            || reservation.expires_at_unix_ms - completed.completed_at_unix_ms > 60_000
            || anchor.height < signing_anchor.height
            || anchor.block_hash == [0; 32]
            || anchor.operation_state_digest == [0; 32]
            || (anchor.height == signing_anchor.height
                && anchor.block_hash != signing_anchor.block_hash)
            || anchor.height > current.current_anchor.height
            || (anchor.height == current.current_anchor.height
                && anchor.block_hash != current.current_anchor.block_hash)
        {
            return Err(SignerReceiptErrorV1::CompletionMismatch);
        }
        Ok(())
    }
}

fn anchor_descends(current: SignerCustodyAnchorV1, previous: SignerCustodyAnchorV1) -> bool {
    current.height != 0
        && current.block_hash != [0; 32]
        && current.state_digest != [0; 32]
        && current.height >= previous.height
        && (current.height != previous.height || current == previous)
}
