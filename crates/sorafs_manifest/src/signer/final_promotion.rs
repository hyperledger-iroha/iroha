//! Purpose-specific final-promotion receipts with independent custody and completion checks.
//!
//! The prepared statement binds one canonical schema to chain, network, deployment and signer
//! identity before signer I/O. Offline verification prepares untrusted bytes again. Candidate
//! receipts never supply trust, time, active custody or completed-operation authority. This pure
//! boundary proves an authenticated receipt, not a qualified deployment or underlying lane evidence.

use super::{
    custody::{
        SignerCustodyBindingV1, SignerCustodyErrorV1, SignerCustodyTrustV1,
        SignerCustodyUseContextV1, VerifiedSignerCustodyV1, verify_signer_custody_use_v1,
    },
    protocol::{
        SignerKeyAlgorithmV1, SignerOperationActionV1, SignerOperationAuditHeadV1,
        SignerOperationCommitmentV1, SignerOperationCustodyV1, SignerOperationIntentV1,
        SignerOperationReservationV1, SignerOperationSignatureV1, SignerPurposeBindingV1,
        SignerRoleV1, digest_canonical, digest_parts,
    },
    receipt::{
        SignerCompletedOperationV1, SignerOperationProvenanceV1, SignerReceiptErrorV1, shared,
    },
};
use norito::codec::{Decode, Encode};
use statement::{PreparedFinalPromotionStatementV1, prepare_final_promotion_statement_v1};
use std::fmt;

pub mod evidence;
pub mod statement;

/// Maximum complete domain-prefixed canonical final-promotion signing message.
pub const SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1: usize = 256 * 1024;
/// Exact first-release final-promotion role-payload signature prefix.
pub const SIGNER_FINAL_PROMOTION_PAYLOAD_DOMAIN_V1: &[u8] =
    b"iroha:sorafs:production-readiness:production-promotion-provenance:v1\0";

/// Secret-free final-promotion receipt failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerFinalPromotionReceiptErrorV1 {
    /// Invalid bounds, frame, marker or canonical encoding.
    InvalidReceipt,
    /// Custody belongs to another role or deployment purpose.
    WrongPurpose,
    /// The statement does not satisfy the sole canonical final-promotion schema.
    InvalidStatement,
    /// Exact independently reviewed statement, binding or operation differs.
    StatementMismatch,
    /// Independent active signer authorization verification failed.
    Custody(SignerCustodyErrorV1),
    /// Shared operation signature, provenance or completed-state verification failed.
    Operation(SignerReceiptErrorV1),
}
impl fmt::Display for SignerFinalPromotionReceiptErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::InvalidReceipt => "invalid canonical final promotion receipt",
            Self::WrongPurpose => "final promotion receipt has the wrong purpose",
            Self::InvalidStatement => "invalid canonical final promotion statement",
            Self::StatementMismatch => "final promotion receipt differs from reviewed inputs",
            Self::Custody(_) => {
                "final promotion receipt lacks independently verified active custody"
            }
            Self::Operation(_) => "final promotion operation verification failed",
        })
    }
}
impl std::error::Error for SignerFinalPromotionReceiptErrorV1 {}
impl From<SignerReceiptErrorV1> for SignerFinalPromotionReceiptErrorV1 {
    fn from(error: SignerReceiptErrorV1) -> Self {
        Self::Operation(error)
    }
}

/// Complete canonical receipt bound, checked before any allocation from a candidate.
pub const SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1: usize = 64 * 1024;
/// Sole first-release receipt marker.
pub const SIGNER_FINAL_PROMOTION_RECEIPT_MAGIC_V1: [u8; 8] = *b"IRSFPR01";

/// Independently reviewed subject expected by the receipt consumer.
///
/// There is no decoder/default: never construct this expectation from the candidate receipt.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SignerFinalPromotionExpectedV1 {
    /// Exact operation id issued by the promotion coordinator before signing.
    pub operation_id: [u8; 32],
    /// Domain-separated digest of the independently pinned reviewed statement bytes.
    pub statement_digest: [u8; 32],
    /// Exact independently pinned reviewed statement byte count.
    pub statement_size: u64,
}
impl fmt::Debug for SignerFinalPromotionExpectedV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerFinalPromotionExpectedV1")
            .finish_non_exhaustive()
    }
}

/// Canonical public final-promotion request committing exactly one reviewed statement and custody.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::signer::final_promotion::SignerFinalPromotionRequestV1")]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    PartialOrd,
    Ord,
    iroha_schema::IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct SignerFinalPromotionRequestV1 {
    /// Exact independently expected id; one durable operation per id.
    pub operation_id: [u8; 32],
    /// Commitment to the whole public role/key/network/deployment/policy binding.
    pub binding_digest: [u8; 32],
    /// Immutable custody identity under which this exact request was admitted.
    pub original_custody: SignerOperationCustodyV1,
    /// Commitment to the exact raw aggregate statement, without any reserialization.
    pub statement_digest: [u8; 32],
    /// Exact raw aggregate statement byte count.
    pub statement_size: u64,
}
impl SignerFinalPromotionRequestV1 {
    /// Construct a purpose-bound request from already verified active custody and reviewed bytes.
    ///
    /// # Errors
    /// Rejects other roles/purposes, invalid expected ids or mismatched reviewed bytes.
    pub fn new(
        custody: &VerifiedSignerCustodyV1,
        expected: &SignerFinalPromotionExpectedV1,
        prepared: &PreparedFinalPromotionStatementV1<'_>,
    ) -> Result<Self, SignerFinalPromotionReceiptErrorV1> {
        let request = Self {
            operation_id: expected.operation_id,
            binding_digest: prepared.binding_digest(),
            original_custody: SignerOperationCustodyV1::from_verified(custody),
            statement_digest: expected.statement_digest,
            statement_size: expected.statement_size,
        };
        request.validate_custody(custody)?;
        if u64::try_from(prepared.len()).ok() != Some(expected.statement_size)
            || signer_final_promotion_digest_v1(prepared.message()) != expected.statement_digest
        {
            return Err(SignerFinalPromotionReceiptErrorV1::StatementMismatch);
        }
        Ok(request)
    }

    /// Validate public request coordinates against independently verified current custody.
    ///
    /// This checks the complete binding and original custody identity, not the statement bytes.
    /// Producers must separately prepare their independently pinned canonical statement.
    ///
    /// # Errors
    /// Rejects another purpose, invalid coordinates, or substituted binding or custody.
    pub fn validate_custody(
        &self,
        custody: &VerifiedSignerCustodyV1,
    ) -> Result<(), SignerFinalPromotionReceiptErrorV1> {
        self.validate_binding(&custody.statement().binding)?;
        if self.original_custody != SignerOperationCustodyV1::from_verified(custody) {
            return Err(SignerFinalPromotionReceiptErrorV1::StatementMismatch);
        }
        Ok(())
    }

    /// Validate public request coordinates against an independently pinned custody binding.
    ///
    /// This checks canonical signer handles, identities, purpose, bounds and the full binding
    /// commitment before I/O. It does not prove current custody or validate statement bytes.
    ///
    /// # Errors
    /// Rejects another purpose, malformed binding or request, or a substituted binding commitment.
    pub fn validate_binding(
        &self,
        binding: &SignerCustodyBindingV1,
    ) -> Result<(), SignerFinalPromotionReceiptErrorV1> {
        if binding.role != SignerRoleV1::FinalPromotionProvenance
            || !matches!(
                binding.purpose,
                SignerPurposeBindingV1::FinalPromotionProvenance { .. }
            )
            || binding.algorithm != SignerKeyAlgorithmV1::Ed25519
        {
            return Err(SignerFinalPromotionReceiptErrorV1::WrongPurpose);
        }
        super::custody::validate_binding(binding)
            .map_err(SignerFinalPromotionReceiptErrorV1::Custody)?;
        if self.operation_id == [0; 32]
            || self.statement_digest == [0; 32]
            || self.statement_size == 0
            || self.statement_size > SIGNER_FINAL_PROMOTION_STATEMENT_MAX_BYTES_V1 as u64
            || self.original_custody.record_digest == [0; 32]
            || self.original_custody.control_state_digest == [0; 32]
        {
            return Err(SignerFinalPromotionReceiptErrorV1::InvalidReceipt);
        }
        let binding_digest = digest_canonical(b"iroha.sorafs.signer.custody-binding.v1", binding)
            .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt)?;
        if self.binding_digest != binding_digest {
            return Err(SignerFinalPromotionReceiptErrorV1::StatementMismatch);
        }
        Ok(())
    }

    /// Exact canonical request commitment used by the reserved operation intent.
    ///
    /// # Errors
    /// Rejects a canonical serialization failure.
    pub fn digest(&self) -> Result<[u8; 32], SignerFinalPromotionReceiptErrorV1> {
        digest_canonical(b"iroha.sorafs.signer.final-promotion.request.v1", self)
            .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt)
    }
}

/// Sole canonical first-release statement signing receipt; contains no private key or credentials.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::signer::final_promotion::SignerFinalPromotionReceiptV1")]
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerFinalPromotionReceiptV1 {
    /// Exact [`SIGNER_FINAL_PROMOTION_RECEIPT_MAGIC_V1`] marker.
    pub magic: [u8; 8],
    /// Sole schema version, one.
    pub version: u16,
    /// Exact canonical independently attested original custody record.
    pub custody_record: Vec<u8>,
    /// Exact purpose-bound final-promotion request.
    pub request: SignerFinalPromotionRequestV1,
    /// Exact ordinary signing action and audit predecessor.
    pub intent: SignerOperationIntentV1,
    /// Immutable original reservation.
    pub reservation: SignerOperationReservationV1,
    /// Original current-control-state provenance.
    pub provenance: SignerOperationProvenanceV1,
    /// Exact resulting audit/response commitments.
    pub commitment: SignerOperationCommitmentV1,
    /// Exactly four signatures: raw statement, audit, provenance, final response.
    pub signatures: Vec<SignerOperationSignatureV1>,
}
impl SignerFinalPromotionReceiptV1 {
    /// Borrow only common operation coordinates; the final-promotion owner retains its exact request.
    fn operation_view(&self) -> shared::SignerOperationReceiptViewV1<'_> {
        shared::SignerOperationReceiptViewV1 {
            operation_id: self.request.operation_id,
            original_custody: self.request.original_custody,
            reservation: self.reservation,
            provenance: &self.provenance,
            commitment: &self.commitment,
        }
    }
}
impl fmt::Debug for SignerFinalPromotionReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerFinalPromotionReceiptV1")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

/// Privately constructed successful verification result; neither decodable nor caller-mintable.
pub struct VerifiedFinalPromotionSignerReceiptV1 {
    custody: VerifiedSignerCustodyV1,
    completion: SignerCompletedOperationV1,
    statement_digest: [u8; 32],
}
impl VerifiedFinalPromotionSignerReceiptV1 {
    /// Independently verified current custody for the exact original operation.
    #[must_use]
    pub fn custody(&self) -> &VerifiedSignerCustodyV1 {
        &self.custody
    }
    /// Independently authenticated immutable completed operation.
    #[must_use]
    pub const fn completion(&self) -> &SignerCompletedOperationV1 {
        &self.completion
    }
    /// Exact authenticated raw-statement commitment.
    #[must_use]
    pub const fn statement_digest(&self) -> [u8; 32] {
        self.statement_digest
    }
}
impl fmt::Debug for VerifiedFinalPromotionSignerReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedFinalPromotionSignerReceiptV1")
            .finish_non_exhaustive()
    }
}

/// Commit exact reviewed aggregate statement bytes without reserializing their JSON.
#[must_use]
pub fn signer_final_promotion_digest_v1(message: &[u8]) -> [u8; 32] {
    digest_parts(
        b"iroha.sorafs.signer.final-promotion.payload.v1",
        &[message],
    )
}

/// Derive the canonical next audit record from the exact reserved request and raw signature.
///
/// # Errors
/// Rejects a mismatched action/request/id, invalid predecessor or non-Ed25519 signature size.
pub fn signer_final_promotion_audit_v1(
    request: &SignerFinalPromotionRequestV1,
    intent: &SignerOperationIntentV1,
    reservation: SignerOperationReservationV1,
    statement_signature: &[u8],
) -> Result<SignerOperationAuditHeadV1, SignerFinalPromotionReceiptErrorV1> {
    if intent.action != SignerOperationActionV1::Sign
        || intent.operation_id != request.operation_id
        || intent.request_digest != request.digest()?
        || statement_signature.len() != 64
    {
        return Err(SignerFinalPromotionReceiptErrorV1::InvalidReceipt);
    }
    let intent_digest = intent
        .digest()
        .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt)?;
    let sequence = intent
        .previous_audit
        .sequence
        .checked_add(1)
        .ok_or(SignerFinalPromotionReceiptErrorV1::InvalidReceipt)?;
    let digest = digest_canonical(
        b"iroha.sorafs.signer.final-promotion.audit.v1",
        &(
            *request,
            intent_digest,
            reservation,
            digest_parts(
                b"iroha.sorafs.signer.operation.signature.v1",
                &[statement_signature],
            ),
        ),
    )
    .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt)?;
    Ok(SignerOperationAuditHeadV1 { sequence, digest })
}

/// Derive the exact response commitment after the first three ordered signatures exist.
///
/// # Errors
/// Rejects incorrect signature order/count/size or canonical serialization failure.
pub fn signer_final_promotion_response_digest_v1(
    request: &SignerFinalPromotionRequestV1,
    provenance: &SignerOperationProvenanceV1,
    first_signatures: &[SignerOperationSignatureV1],
) -> Result<[u8; 32], SignerFinalPromotionReceiptErrorV1> {
    let signatures_digest = shared::first_three_signatures_digest(first_signatures)?;
    digest_canonical(
        b"iroha.sorafs.signer.final-promotion.response.v1",
        &(*request, *provenance, signatures_digest),
    )
    .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt)
}

/// Verify exact released bytes with independent reviewed inputs, ACTIVE custody and completion.
///
/// `completion` must already be authenticated against a pinned finalized operation-state source;
/// `current` must come from an independently authenticated fresh custody source and trusted clock.
/// Neither may be filled from candidate receipt fields or inferred from the role-key signature.
/// The verifier performs no I/O and neither acquires nor imports a private key.
///
/// # Errors
/// Rejects malformed/tampered bytes, wrong purpose/key/statement, self-attested or inactive custody,
/// renewal relabeling, stale/revoked state, missing/changed completion and untimely/forked finality.
#[allow(clippy::too_many_arguments)]
pub fn verify_final_promotion_signer_receipt_v1(
    receipt_bytes: &[u8],
    message: &[u8],
    detached_signature: &[u8],
    expected: &SignerFinalPromotionExpectedV1,
    expected_binding: &SignerCustodyBindingV1,
    trust: &SignerCustodyTrustV1,
    current: &SignerCustodyUseContextV1,
    completion: &SignerCompletedOperationV1,
) -> Result<VerifiedFinalPromotionSignerReceiptV1, SignerFinalPromotionReceiptErrorV1> {
    if receipt_bytes.is_empty()
        || receipt_bytes.len() > SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1
        || detached_signature.len() != 64
    {
        return Err(SignerFinalPromotionReceiptErrorV1::InvalidReceipt);
    }
    let receipt: SignerFinalPromotionReceiptV1 = norito::decode_canonical_with_limits(
        receipt_bytes,
        norito::DecodeLimits::new(
            16 * 1024,
            SIGNER_FINAL_PROMOTION_RECEIPT_MAX_BYTES_V1,
            8192,
            512 * 1024,
            24,
        ),
    )
    .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt)?;
    if receipt.magic != SIGNER_FINAL_PROMOTION_RECEIPT_MAGIC_V1 || receipt.version != 1 {
        return Err(SignerFinalPromotionReceiptErrorV1::InvalidReceipt);
    }
    let custody =
        verify_signer_custody_use_v1(&receipt.custody_record, expected_binding, trust, current)
            .map_err(SignerFinalPromotionReceiptErrorV1::Custody)?;
    let signatures_digest = validate_final_promotion_signatures_v1(
        &receipt,
        message,
        detached_signature,
        expected,
        &custody,
    )?;
    let intent_digest = receipt
        .intent
        .digest()
        .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt)?;
    receipt.operation_view().validate_completion(
        completion,
        intent_digest,
        signatures_digest,
        &custody,
        current,
    )?;
    Ok(VerifiedFinalPromotionSignerReceiptV1 {
        custody,
        completion: *completion,
        statement_digest: expected.statement_digest,
    })
}

/// Validate exact reviewed payload, canonical provenance and every ordered signature.
///
/// This does not establish durable completion or authorize releasing staged signatures. Runtime
/// recovery and offline receipt verification share this one message-binding implementation; each
/// must separately authenticate its immutable completed row before releasing or qualifying output.
/// The custody argument is privately constructed by independent current-state verification.
///
/// # Errors
/// Rejects substituted custody bytes, wrong purpose/statement, invalid canonical bindings or signatures.
pub fn validate_final_promotion_signatures_v1(
    receipt: &SignerFinalPromotionReceiptV1,
    message: &[u8],
    detached_signature: &[u8],
    expected: &SignerFinalPromotionExpectedV1,
    custody: &VerifiedSignerCustodyV1,
) -> Result<[u8; 32], SignerFinalPromotionReceiptErrorV1> {
    if receipt.magic != SIGNER_FINAL_PROMOTION_RECEIPT_MAGIC_V1
        || receipt.version != 1
        || receipt.custody_record.is_empty()
        || receipt.custody_record.len() > super::custody::SIGNER_CUSTODY_MAX_BYTES_V1
        || detached_signature.len() != 64
        || digest_parts(
            super::custody::CUSTODY_RECORD_DIGEST_DOMAIN_V1,
            &[&receipt.custody_record],
        ) != custody.record_digest()
    {
        return Err(SignerFinalPromotionReceiptErrorV1::InvalidReceipt);
    }
    let prepared = prepare_final_promotion_statement_v1(message, &custody.statement().binding)
        .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidStatement)?;
    let request = SignerFinalPromotionRequestV1::new(custody, expected, &prepared)?;
    if receipt.request != request {
        return Err(SignerFinalPromotionReceiptErrorV1::StatementMismatch);
    }
    let intent_digest = receipt
        .intent
        .digest()
        .map_err(|_| SignerFinalPromotionReceiptErrorV1::InvalidReceipt)?;
    let audit = signer_final_promotion_audit_v1(
        &request,
        &receipt.intent,
        receipt.reservation,
        detached_signature,
    )?;
    let operation = receipt.operation_view();
    operation.validate_provenance(intent_digest, audit, custody)?;
    let signatures = shared::exact_four_signatures(&receipt.signatures, detached_signature)?;
    if signer_final_promotion_response_digest_v1(&request, &receipt.provenance, &signatures[..3])?
        != receipt.commitment.response_digest
    {
        return Err(SignerReceiptErrorV1::InvalidSignature.into());
    }
    operation
        .validate_signatures(signatures, message, audit, custody)
        .map_err(Into::into)
}

#[cfg(test)]
mod tests;
