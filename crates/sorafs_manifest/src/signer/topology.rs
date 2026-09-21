//! Candidate-bound topology receipt consistency, without production authority admission.
//!
//! Uses the common four-signature, reservation, provenance and completion checks. Independent
//! reference inputs are mandatory; matching caller-supplied state is not proof that Core executed
//! a permitted operation. TODO: add native topology custody/current-Check/operation producers and
//! exact executed input/result/output proofs before exposing a production approval consumer.
//! Generic software provisioning and the aggregate promotion gate remain closed for this role.

use super::{
    custody::{
        SignerCustodyBindingV1, SignerCustodyErrorV1, SignerCustodyTrustV1,
        SignerCustodyUseContextV1, VerifiedSignerCustodyV1, verify_signer_custody_use_v1,
    },
    protocol::{
        SignerOperationActionV1, SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
        SignerOperationCustodyV1, SignerOperationIntentV1, SignerOperationReservationV1,
        SignerOperationSignatureV1, digest_canonical, digest_parts,
    },
    receipt::{
        SignerCompletedOperationV1, SignerOperationProvenanceV1, SignerReceiptErrorV1, shared,
    },
};
use norito::codec::{Decode, Encode};
use std::fmt;
use subject::{
    PreparedTopologyApprovalV1, TopologyApprovalSubjectV1, prepare_topology_approval_v1,
};

pub mod subject;
/// Maximum canonical receipt size before any candidate-controlled allocation.
pub const TOPOLOGY_RECEIPT_MAX_BYTES_V1: usize = 64 * 1024;
/// Sole first-release topology receipt marker.
pub const TOPOLOGY_RECEIPT_MAGIC_V1: [u8; 8] = *b"IRSTPR01";

/// Secret-free errors; successful consistency does not establish native execution authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerTopologyReceiptErrorV1 {
    /// Invalid canonical frame, coordinates or bounds.
    InvalidReceipt,
    /// Another signer role or deployment purpose.
    WrongPurpose,
    /// Invalid reviewed configuration subject.
    InvalidSubject,
    /// The receipt differs from independently reviewed inputs or their validity period.
    SubjectMismatch,
    /// Independent signer authorization, current state or revocation check failed.
    Custody(SignerCustodyErrorV1),
    /// Ordered signatures, provenance or supplied completed row differ.
    Operation(SignerReceiptErrorV1),
}
impl fmt::Display for SignerTopologyReceiptErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::InvalidReceipt => "invalid canonical topology receipt",
            Self::WrongPurpose => "topology receipt has the wrong purpose",
            Self::InvalidSubject => "invalid reviewed topology subject",
            Self::SubjectMismatch => "topology receipt differs from reviewed inputs",
            Self::Custody(_) => "topology receipt lacks current independent signer authorization",
            Self::Operation(_) => "topology operation consistency check failed",
        })
    }
}
impl std::error::Error for SignerTopologyReceiptErrorV1 {}
impl From<SignerReceiptErrorV1> for SignerTopologyReceiptErrorV1 {
    fn from(error: SignerReceiptErrorV1) -> Self {
        Self::Operation(error)
    }
}
type Error = SignerTopologyReceiptErrorV1;

/// Exact topology request admitted under one immutable custody identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::signer::topology::SignerTopologyRequestV1")]
pub struct SignerTopologyRequestV1 {
    /// Independently assigned operation id, never taken from a candidate receipt.
    pub operation_id: [u8; 32],
    /// Entire role/key/policy/network/deployment binding commitment.
    pub binding_digest: [u8; 32],
    /// Exact original active custody identity.
    pub original_custody: SignerOperationCustodyV1,
    /// Exact canonical domain-prefixed subject commitment.
    pub subject_digest: [u8; 32],
}
impl SignerTopologyRequestV1 {
    /// Bind reviewed bytes to independently verified current signer authorization.
    ///
    /// # Errors
    /// Rejects wrong purpose, substituted whole bindings and zero operation identifiers.
    pub fn new(
        custody: &VerifiedSignerCustodyV1,
        operation_id: [u8; 32],
        prepared: &PreparedTopologyApprovalV1,
    ) -> Result<Self, Error> {
        let binding = &custody.statement().binding;
        subject::validate_topology_binding(binding)?;
        if operation_id == [0; 32] {
            return Err(Error::InvalidReceipt);
        }
        let binding_digest = digest_canonical(b"iroha.sorafs.signer.custody-binding.v1", binding)
            .map_err(|_| Error::InvalidReceipt)?;
        if binding_digest != prepared.binding_digest() {
            return Err(Error::SubjectMismatch);
        }
        Ok(Self {
            operation_id,
            binding_digest,
            original_custody: SignerOperationCustodyV1::from_verified(custody),
            subject_digest: digest_parts(
                b"iroha.sorafs.signer.topology.payload.v1",
                &[prepared.message()],
            ),
        })
    }
    /// Exact request commitment reserved before any provider operation.
    ///
    /// # Errors
    /// Rejects canonical encoding failure.
    pub fn digest(&self) -> Result<[u8; 32], Error> {
        digest_canonical(b"iroha.sorafs.signer.topology.request.v1", self)
            .map_err(|_| Error::InvalidReceipt)
    }
}

/// Sole canonical receipt; possession alone is not a native approval or finality proof.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::signer::topology::SignerTopologyReceiptV1")]
pub struct SignerTopologyReceiptV1 {
    /// Exact first-release receipt marker.
    pub magic: [u8; 8],
    /// Sole version, one.
    pub version: u16,
    /// Exact independently attested original public custody record.
    pub custody_record: Vec<u8>,
    /// Entire candidate-bound topology request.
    pub request: SignerTopologyRequestV1,
    /// Exact Sign intent and prior audit head.
    pub intent: SignerOperationIntentV1,
    /// Original exclusive reservation and fence.
    pub reservation: SignerOperationReservationV1,
    /// Original signing-state provenance.
    pub provenance: SignerOperationProvenanceV1,
    /// Exact audit and response commitments.
    pub commitment: SignerOperationCommitmentV1,
    /// Four signatures in role-payload, audit, provenance, response order.
    pub signatures: Vec<SignerOperationSignatureV1>,
}
impl SignerTopologyReceiptV1 {
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
impl fmt::Debug for SignerTopologyReceiptV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("SignerTopologyReceiptV1")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

/// Independent reference inputs to the pure consistency checker; this type grants no authority.
///
/// A future production consumer must obtain current/completed state through genuine native
/// topology Check execution and canonical input/result/output proofs, never a receipt, local
/// journal or unrelated block/QC. No such topology native authority producer exists yet.
pub struct TopologyReceiptReferenceV1<'a> {
    /// Separately reviewed complete subject, including the exact release candidate.
    pub subject: &'a TopologyApprovalSubjectV1,
    /// Independently assigned original operation id.
    pub operation_id: [u8; 32],
    /// Independently pinned whole signer binding.
    pub binding: &'a SignerCustodyBindingV1,
    /// Independent signer authorization authority trust.
    pub trust: &'a SignerCustodyTrustV1,
    /// Fresh current custody/revocations and trusted clock; not read from the receipt.
    pub current: &'a SignerCustodyUseContextV1,
    /// Original immutable completed row; not read from the receipt.
    pub completion: &'a SignerCompletedOperationV1,
}

/// Derive the exact next audit after the reviewed role payload is signed.
///
/// # Errors
/// Rejects wrong action, operation, request, predecessor overflow or signature size.
pub fn signer_topology_audit_v1(
    request: &SignerTopologyRequestV1,
    intent: &SignerOperationIntentV1,
    reservation: SignerOperationReservationV1,
    signature: &[u8],
) -> Result<SignerOperationAuditHeadV1, Error> {
    if intent.action != SignerOperationActionV1::Sign
        || intent.operation_id != request.operation_id
        || intent.request_digest != request.digest()?
        || signature.len() != 64
    {
        return Err(Error::InvalidReceipt);
    }
    let intent_digest = intent.digest().map_err(|_| Error::InvalidReceipt)?;
    let sequence = intent
        .previous_audit
        .sequence
        .checked_add(1)
        .ok_or(Error::InvalidReceipt)?;
    let digest = digest_canonical(
        b"iroha.sorafs.signer.topology.audit.v1",
        &(
            *request,
            intent_digest,
            reservation,
            digest_parts(b"iroha.sorafs.signer.operation.signature.v1", &[signature]),
        ),
    )
    .map_err(|_| Error::InvalidReceipt)?;
    Ok(SignerOperationAuditHeadV1 { sequence, digest })
}

/// Commit exactly the first three signatures under the topology-only response domain.
///
/// # Errors
/// Rejects wrong count/order/size or canonical encoding failure.
pub fn signer_topology_response_digest_v1(
    request: &SignerTopologyRequestV1,
    provenance: &SignerOperationProvenanceV1,
    signatures: &[SignerOperationSignatureV1],
) -> Result<[u8; 32], Error> {
    let digest = shared::first_three_signatures_digest(signatures)?;
    digest_canonical(
        b"iroha.sorafs.signer.topology.response.v1",
        &(*request, *provenance, digest),
    )
    .map_err(|_| Error::InvalidReceipt)
}

/// Check receipt cryptography and equality to independent reference inputs, without admission.
///
/// Returns no verified-authority token. This cannot establish that the supplied completed row was
/// executed, that permissions held, or that the configuration was deployed. The production gate
/// must remain closed until a purpose-owned native authority consumer supplies those guarantees.
///
/// # Errors
/// Rejects malformed/oversized frames, substituted candidate/subject/signer, inactive or revoked
/// custody, invalid ordered signatures, original reservation/completion differences and stale review.
pub fn check_topology_receipt_consistency_v1(
    bytes: &[u8],
    detached_signature: &[u8],
    reference: &TopologyReceiptReferenceV1<'_>,
) -> Result<(), Error> {
    if bytes.is_empty()
        || bytes.len() > TOPOLOGY_RECEIPT_MAX_BYTES_V1
        || detached_signature.len() != 64
    {
        return Err(Error::InvalidReceipt);
    }
    let receipt: SignerTopologyReceiptV1 = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            16 * 1024,
            TOPOLOGY_RECEIPT_MAX_BYTES_V1,
            8192,
            512 * 1024,
            24,
        ),
    )
    .map_err(|_| Error::InvalidReceipt)?;
    if receipt.magic != TOPOLOGY_RECEIPT_MAGIC_V1 || receipt.version != 1 {
        return Err(Error::InvalidReceipt);
    }
    let prepared = prepare_topology_approval_v1(reference.subject, reference.binding)?;
    if reference.current.now_unix_ms < reference.subject.reviewed_at_unix_ms
        || reference.current.now_unix_ms >= reference.subject.expires_at_unix_ms
        || reference.completion.completed_at_unix_ms < reference.subject.reviewed_at_unix_ms
        || reference.completion.completed_at_unix_ms >= reference.subject.expires_at_unix_ms
    {
        return Err(Error::SubjectMismatch);
    }
    let custody = verify_signer_custody_use_v1(
        &receipt.custody_record,
        reference.binding,
        reference.trust,
        reference.current,
    )
    .map_err(Error::Custody)?;
    let request = SignerTopologyRequestV1::new(&custody, reference.operation_id, &prepared)?;
    if receipt.request != request {
        return Err(Error::SubjectMismatch);
    }
    let audit = signer_topology_audit_v1(
        &request,
        &receipt.intent,
        receipt.reservation,
        detached_signature,
    )?;
    let intent_digest = receipt.intent.digest().map_err(|_| Error::InvalidReceipt)?;
    let operation = receipt.operation_view();
    operation.validate_provenance(intent_digest, audit, &custody)?;
    let signatures = shared::exact_four_signatures(&receipt.signatures, detached_signature)?;
    if signer_topology_response_digest_v1(&request, &receipt.provenance, &signatures[..3])?
        != receipt.commitment.response_digest
    {
        return Err(SignerReceiptErrorV1::InvalidSignature.into());
    }
    let digest = operation.validate_signatures(signatures, prepared.message(), audit, &custody)?;
    operation.validate_completion(
        reference.completion,
        intent_digest,
        digest,
        &custody,
        reference.current,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
