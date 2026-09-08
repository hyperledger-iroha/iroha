//! Purpose-specific release-manifest receipts with independent custody and completion checks.
//!
//! Candidate receipts never provide trust, the clock, current ACTIVE state, a completed-operation
//! observation, or the expected reviewed manifest. The caller authenticates those inputs through
//! its pinned policy and finalized state source. A role-key signature alone proves none of them.
//! Aggregate inventory/schema validation belongs to the release artifact owner; this boundary
//! authenticates the exact independently reviewed bytes without JSON reserialization.

use super::{
    custody::{
        SignerCustodyAnchorV1, SignerCustodyBindingV1, SignerCustodyErrorV1, SignerCustodyTrustV1,
        SignerCustodyUseContextV1, VerifiedSignerCustodyV1, verify_signer_custody_use_v1,
    },
    protocol::{
        SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1, SignerKeyAlgorithmV1, SignerKeyOperationPurposeV1,
        SignerOperationActionV1, SignerOperationAuditHeadV1, SignerOperationCommitmentV1,
        SignerOperationCustodyV1, SignerOperationIntentV1, SignerOperationReservationV1,
        SignerOperationSignatureV1, SignerPurposeBindingV1, SignerRoleV1, digest_canonical,
        digest_parts, signer_operation_message_digest_v1, signer_operation_signatures_digest_v1,
    },
};
use iroha_crypto::Signature;
use norito::codec::{Decode, Encode};
use std::fmt;

/// Complete canonical receipt bound, checked before any allocation from a candidate.
pub const SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1: usize = 64 * 1024;
/// Sole first-release receipt marker.
pub const SIGNER_RELEASE_MANIFEST_RECEIPT_MAGIC_V1: [u8; 8] = *b"IRSRMR01";

/// Independently reviewed subject expected by the receipt consumer.
///
/// There is no decoder/default: never construct this expectation from the candidate receipt.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SignerReleaseManifestExpectedV1 {
    /// Exact operation id issued by the release coordinator before signing.
    pub operation_id: [u8; 32],
    /// Domain-separated digest of the independently pinned reviewed manifest bytes.
    pub manifest_digest: [u8; 32],
    /// Exact independently pinned reviewed manifest byte count.
    pub manifest_size: u64,
}
impl fmt::Debug for SignerReleaseManifestExpectedV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerReleaseManifestExpectedV1")
            .finish_non_exhaustive()
    }
}

/// Canonical public release request committing exactly one reviewed manifest and custody.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerReleaseManifestRequestV1 {
    /// Exact independently expected id; one durable operation per id.
    pub operation_id: [u8; 32],
    /// Commitment to the whole public role/key/network/deployment/policy binding.
    pub binding_digest: [u8; 32],
    /// Immutable custody identity under which this exact request was admitted.
    pub original_custody: SignerOperationCustodyV1,
    /// Commitment to the exact raw aggregate manifest, without any reserialization.
    pub manifest_digest: [u8; 32],
    /// Exact raw aggregate manifest byte count.
    pub manifest_size: u64,
}
impl SignerReleaseManifestRequestV1 {
    /// Construct a purpose-bound request from already verified active custody and reviewed bytes.
    ///
    /// # Errors
    /// Rejects other roles/purposes, invalid expected ids or mismatched reviewed bytes.
    pub fn new(
        custody: &VerifiedSignerCustodyV1,
        expected: &SignerReleaseManifestExpectedV1,
        manifest: &[u8],
    ) -> Result<Self, SignerReceiptErrorV1> {
        let binding = &custody.statement().binding;
        if binding.role != SignerRoleV1::ReleaseManifest
            || !matches!(
                binding.purpose,
                SignerPurposeBindingV1::ReleaseManifest { .. }
            )
            || binding.algorithm != SignerKeyAlgorithmV1::Ed25519
        {
            return Err(SignerReceiptErrorV1::WrongPurpose);
        }
        if expected.operation_id == [0; 32]
            || manifest.is_empty()
            || manifest.len() > SIGNER_RELEASE_MANIFEST_MAX_BYTES_V1
            || u64::try_from(manifest.len()).ok() != Some(expected.manifest_size)
            || signer_release_manifest_digest_v1(manifest) != expected.manifest_digest
        {
            return Err(SignerReceiptErrorV1::ManifestMismatch);
        }
        Ok(Self {
            operation_id: expected.operation_id,
            binding_digest: digest_canonical(b"iroha.sorafs.signer.custody-binding.v1", binding)
                .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)?,
            original_custody: SignerOperationCustodyV1::from_verified(custody),
            manifest_digest: expected.manifest_digest,
            manifest_size: expected.manifest_size,
        })
    }

    /// Exact canonical request commitment used by the reserved operation intent.
    ///
    /// # Errors
    /// Rejects a canonical serialization failure.
    pub fn digest(&self) -> Result<[u8; 32], SignerReceiptErrorV1> {
        digest_canonical(b"iroha.sorafs.signer.release-manifest.request.v1", self)
            .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)
    }
}

/// Public provenance signed within the original reserved operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerOperationProvenanceV1 {
    /// Original independently qualified record and per-role control state.
    pub original_custody: SignerOperationCustodyV1,
    /// Genuinely finalized control-state anchor observed during signing.
    pub signing_anchor: SignerCustodyAnchorV1,
    /// Exact admitted request/action/predecessor commitment.
    pub intent_digest: [u8; 32],
    /// Original exclusive reservation, never a fresh recovery reservation.
    pub reservation: SignerOperationReservationV1,
    /// Exact resulting audit record.
    pub audit: SignerOperationAuditHeadV1,
}
impl SignerOperationProvenanceV1 {
    /// Exact signing message for canonical provenance.
    ///
    /// # Errors
    /// Rejects canonical serialization failure.
    pub fn signing_message(&self) -> Result<[u8; 32], SignerReceiptErrorV1> {
        digest_canonical(b"iroha.sorafs.signer.operation.provenance.v1", self)
            .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)
    }
}

/// Finalized authoritative operation-state anchor, distinct from per-role custody control state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerOperationFinalizedAnchorV1 {
    /// Nonzero genuinely finalized block height including the completed operation.
    pub height: u64,
    /// Exact finalized block hash.
    pub block_hash: [u8; 32],
    /// Authenticated commitment to the durable operation journal at this block.
    pub operation_state_digest: [u8; 32],
}

/// Exact authoritative completed row, authenticated independently of any candidate receipt.
///
/// The record is a wire claim until a caller authenticates its inclusion and finality using the
/// deployment's independently pinned state authority. Candidate self-signatures cannot do so.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub struct SignerCompletedOperationV1 {
    /// Original request's operation identity.
    pub operation_id: [u8; 32],
    /// Exact original canonical action/request/audit-predecessor commitment.
    pub intent_digest: [u8; 32],
    /// Original independent hardware-custody identity; retained through any later renewal.
    pub original_custody: SignerOperationCustodyV1,
    /// Exact exclusive reservation under which completion committed.
    pub reservation: SignerOperationReservationV1,
    /// Exact committed audit and response.
    pub commitment: SignerOperationCommitmentV1,
    /// Commitment to every exact ordered released signature.
    pub signatures_digest: [u8; 32],
    /// Authoritative completion time; must precede the original reservation's expiry.
    pub completed_at_unix_ms: u64,
    /// Genuinely finalized operation journal containing this immutable row.
    pub anchor: SignerOperationFinalizedAnchorV1,
}

/// Sole canonical first-release manifest signing receipt; contains no private key or credentials.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub struct SignerReleaseManifestReceiptV1 {
    /// Exact [`SIGNER_RELEASE_MANIFEST_RECEIPT_MAGIC_V1`] marker.
    pub magic: [u8; 8],
    /// Sole schema version, one.
    pub version: u16,
    /// Exact canonical independently attested original custody record.
    pub custody_record: Vec<u8>,
    /// Exact purpose-bound release request.
    pub request: SignerReleaseManifestRequestV1,
    /// Exact ordinary signing action and audit predecessor.
    pub intent: SignerOperationIntentV1,
    /// Immutable original reservation.
    pub reservation: SignerOperationReservationV1,
    /// Original current-control-state provenance.
    pub provenance: SignerOperationProvenanceV1,
    /// Exact resulting audit/response commitments.
    pub commitment: SignerOperationCommitmentV1,
    /// Exactly four signatures: raw manifest, audit, provenance, final response.
    pub signatures: Vec<SignerOperationSignatureV1>,
}
impl fmt::Debug for SignerReleaseManifestReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerReleaseManifestReceiptV1")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

/// Privately constructed successful verification result; neither decodable nor caller-mintable.
pub struct VerifiedReleaseManifestSignerReceiptV1 {
    custody: VerifiedSignerCustodyV1,
    completion: SignerCompletedOperationV1,
    manifest_digest: [u8; 32],
}
impl VerifiedReleaseManifestSignerReceiptV1 {
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
    /// Exact authenticated raw-manifest commitment.
    #[must_use]
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }
}
impl fmt::Debug for VerifiedReleaseManifestSignerReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedReleaseManifestSignerReceiptV1")
            .finish_non_exhaustive()
    }
}

/// Secret-free errors from exact release-receipt verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerReceiptErrorV1 {
    /// Invalid bounds, frame, marker, fields or canonical encoding.
    InvalidReceipt,
    /// A different role or purpose attempted to stand in for release-manifest signing.
    WrongPurpose,
    /// Exact independently reviewed manifest or request differs.
    ManifestMismatch,
    /// Independent current custody verification failed.
    Custody(SignerCustodyErrorV1),
    /// Ordered signature, signed message, audit or response differs.
    InvalidSignature,
    /// Exact independently authenticated durable completion differs or is not timely/finalized.
    CompletionMismatch,
}
impl fmt::Display for SignerReceiptErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidReceipt => "invalid canonical signer receipt",
            Self::WrongPurpose => "signer receipt has the wrong purpose",
            Self::ManifestMismatch => "signer receipt differs from the reviewed release manifest",
            Self::Custody(_) => "signer receipt lacks independently verified active custody",
            Self::InvalidSignature => "signer receipt signature binding is invalid",
            Self::CompletionMismatch => {
                "signer receipt lacks the exact authenticated finalized completion"
            }
        })
    }
}
impl std::error::Error for SignerReceiptErrorV1 {}

/// Commit exact reviewed aggregate manifest bytes without reserializing their JSON.
#[must_use]
pub fn signer_release_manifest_digest_v1(manifest: &[u8]) -> [u8; 32] {
    digest_parts(
        b"iroha.sorafs.signer.release-manifest.payload.v1",
        &[manifest],
    )
}

/// Derive the canonical next audit record from the exact reserved request and raw signature.
///
/// # Errors
/// Rejects a mismatched action/request/id, invalid predecessor or non-Ed25519 signature size.
pub fn signer_release_manifest_audit_v1(
    request: &SignerReleaseManifestRequestV1,
    intent: &SignerOperationIntentV1,
    reservation: SignerOperationReservationV1,
    manifest_signature: &[u8],
) -> Result<SignerOperationAuditHeadV1, SignerReceiptErrorV1> {
    if intent.action != SignerOperationActionV1::Sign
        || intent.operation_id != request.operation_id
        || intent.request_digest != request.digest()?
        || manifest_signature.len() != 64
    {
        return Err(SignerReceiptErrorV1::InvalidReceipt);
    }
    let intent_digest = intent
        .digest()
        .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)?;
    let sequence = intent
        .previous_audit
        .sequence
        .checked_add(1)
        .ok_or(SignerReceiptErrorV1::InvalidReceipt)?;
    let digest = digest_canonical(
        b"iroha.sorafs.signer.release-manifest.audit.v1",
        &(
            *request,
            intent_digest,
            reservation,
            digest_parts(
                b"iroha.sorafs.signer.operation.signature.v1",
                &[manifest_signature],
            ),
        ),
    )
    .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)?;
    Ok(SignerOperationAuditHeadV1 { sequence, digest })
}

/// Derive the exact response commitment after the first three ordered signatures exist.
///
/// # Errors
/// Rejects incorrect signature order/count/size or canonical serialization failure.
pub fn signer_release_manifest_response_digest_v1(
    request: &SignerReleaseManifestRequestV1,
    provenance: &SignerOperationProvenanceV1,
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
    let signatures_digest = signer_operation_signatures_digest_v1(first_signatures)
        .map_err(|_| SignerReceiptErrorV1::InvalidSignature)?;
    digest_canonical(
        b"iroha.sorafs.signer.release-manifest.response.v1",
        &(*request, *provenance, signatures_digest),
    )
    .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)
}

/// Verify exact released bytes with independent reviewed inputs, ACTIVE custody and completion.
///
/// `completion` must already be authenticated against a pinned finalized operation-state source;
/// `current` must come from an independently authenticated fresh custody source and trusted clock.
/// Neither may be filled from candidate receipt fields or inferred from the role-key signature.
/// The verifier performs no I/O and neither acquires nor imports a hardware/private key.
///
/// # Errors
/// Rejects malformed/tampered bytes, wrong purpose/key/manifest, self-attested or inactive custody,
/// renewal relabeling, stale/revoked state, missing/changed completion and untimely/forked finality.
#[allow(clippy::too_many_arguments)]
pub fn verify_release_manifest_signer_receipt_v1(
    receipt_bytes: &[u8],
    manifest: &[u8],
    detached_signature: &[u8],
    expected: &SignerReleaseManifestExpectedV1,
    expected_binding: &SignerCustodyBindingV1,
    trust: &SignerCustodyTrustV1,
    current: &SignerCustodyUseContextV1,
    completion: &SignerCompletedOperationV1,
) -> Result<VerifiedReleaseManifestSignerReceiptV1, SignerReceiptErrorV1> {
    if receipt_bytes.is_empty()
        || receipt_bytes.len() > SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1
        || detached_signature.len() != 64
    {
        return Err(SignerReceiptErrorV1::InvalidReceipt);
    }
    let receipt: SignerReleaseManifestReceiptV1 = norito::decode_canonical_with_limits(
        receipt_bytes,
        norito::DecodeLimits::new(
            16 * 1024,
            SIGNER_RELEASE_MANIFEST_RECEIPT_MAX_BYTES_V1,
            8192,
            512 * 1024,
            24,
        ),
    )
    .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)?;
    if receipt.magic != SIGNER_RELEASE_MANIFEST_RECEIPT_MAGIC_V1 || receipt.version != 1 {
        return Err(SignerReceiptErrorV1::InvalidReceipt);
    }
    let custody =
        verify_signer_custody_use_v1(&receipt.custody_record, expected_binding, trust, current)
            .map_err(SignerReceiptErrorV1::Custody)?;
    let signatures_digest = validate_release_manifest_signatures_v1(
        &receipt,
        manifest,
        detached_signature,
        expected,
        &custody,
    )?;
    let intent_digest = receipt
        .intent
        .digest()
        .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)?;
    validate_completion(
        completion,
        &receipt,
        intent_digest,
        signatures_digest,
        &custody,
        current,
    )?;
    Ok(VerifiedReleaseManifestSignerReceiptV1 {
        custody,
        completion: *completion,
        manifest_digest: expected.manifest_digest,
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
/// Rejects substituted custody bytes, wrong purpose/manifest, invalid canonical bindings or signatures.
pub fn validate_release_manifest_signatures_v1(
    receipt: &SignerReleaseManifestReceiptV1,
    manifest: &[u8],
    detached_signature: &[u8],
    expected: &SignerReleaseManifestExpectedV1,
    custody: &VerifiedSignerCustodyV1,
) -> Result<[u8; 32], SignerReceiptErrorV1> {
    if receipt.magic != SIGNER_RELEASE_MANIFEST_RECEIPT_MAGIC_V1
        || receipt.version != 1
        || receipt.custody_record.is_empty()
        || receipt.custody_record.len() > super::custody::SIGNER_CUSTODY_MAX_BYTES_V1
        || detached_signature.len() != 64
        || digest_parts(
            super::custody::CUSTODY_RECORD_DIGEST_DOMAIN_V1,
            &[&receipt.custody_record],
        ) != custody.record_digest()
    {
        return Err(SignerReceiptErrorV1::InvalidReceipt);
    }
    let request = SignerReleaseManifestRequestV1::new(custody, expected, manifest)?;
    if receipt.request != request {
        return Err(SignerReceiptErrorV1::ManifestMismatch);
    }
    let intent_digest = receipt
        .intent
        .digest()
        .map_err(|_| SignerReceiptErrorV1::InvalidReceipt)?;
    let audit = signer_release_manifest_audit_v1(
        &request,
        &receipt.intent,
        receipt.reservation,
        detached_signature,
    )?;
    if receipt.commitment.audit != audit
        || receipt.provenance.original_custody != request.original_custody
        || receipt.provenance.intent_digest != intent_digest
        || receipt.provenance.reservation != receipt.reservation
        || receipt.provenance.audit != audit
        || receipt.provenance.signing_anchor.state_digest
            != request.original_custody.control_state_digest
        || !anchor_descends(
            receipt.provenance.signing_anchor,
            custody.statement().anchor,
        )
        || !anchor_descends(custody.current_anchor(), receipt.provenance.signing_anchor)
    {
        return Err(SignerReceiptErrorV1::InvalidReceipt);
    }
    if receipt.signatures.len() != 4 || receipt.signatures[0].signature != detached_signature {
        return Err(SignerReceiptErrorV1::InvalidSignature);
    }
    if signer_release_manifest_response_digest_v1(
        &request,
        &receipt.provenance,
        &receipt.signatures[..3],
    )? != receipt.commitment.response_digest
    {
        return Err(SignerReceiptErrorV1::InvalidSignature);
    }
    let audit_message = audit.signing_message();
    let provenance_message = receipt.provenance.signing_message()?;
    let response_message = receipt.commitment.response_signing_message();
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
    let signatures_digest = signer_operation_signatures_digest_v1(&receipt.signatures)
        .map_err(|_| SignerReceiptErrorV1::InvalidSignature)?;
    Ok(signatures_digest)
}

fn anchor_descends(current: SignerCustodyAnchorV1, previous: SignerCustodyAnchorV1) -> bool {
    current.height != 0
        && current.block_hash != [0; 32]
        && current.state_digest != [0; 32]
        && current.height >= previous.height
        && (current.height != previous.height || current == previous)
}

fn validate_completion(
    completed: &SignerCompletedOperationV1,
    receipt: &SignerReleaseManifestReceiptV1,
    intent_digest: [u8; 32],
    signatures_digest: [u8; 32],
    custody: &VerifiedSignerCustodyV1,
    current: &SignerCustodyUseContextV1,
) -> Result<(), SignerReceiptErrorV1> {
    let reservation = receipt.reservation;
    let anchor = completed.anchor;
    let signing_anchor = receipt.provenance.signing_anchor;
    if completed.operation_id != receipt.request.operation_id
        || completed.intent_digest != intent_digest
        || completed.original_custody != receipt.request.original_custody
        || completed.reservation != reservation
        || completed.commitment != receipt.commitment
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

#[cfg(test)]
pub(crate) mod tests;
