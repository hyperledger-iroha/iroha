//! C ABI facade for SoraFS reference validators.
//!
//! The facade returns `ValidationOutcomeV1` encoded as Norito JSON so mobile and
//! scripting SDKs can call the Rust reference validators without duplicating
//! SoraFS wire-format logic.

use std::{panic, slice};

use norito::json;

use crate::{
    FixtureBundlePayloadKindV1, FixtureBundlePayloadV1, OrderbookValidationPayloadKindV1,
    ProofStreamTier, RepairValidationPayloadKindV1, ValidationContextFieldV1, ValidationInputV1,
    ValidationOutcomeV1, validate_fixture_bundle_payloads, validate_governance_log_node_bytes,
    validate_orderbook_payload_bytes, validate_pdp_challenge_bytes,
    validate_pdp_challenge_proof_bytes, validate_pdp_commitment_bytes,
    validate_pdp_commitment_challenge_bytes, validate_pdp_commitment_challenge_proof_bytes,
    validate_pdp_proof_bytes, validate_por_challenge_proof_bytes, validate_potr_receipt_bytes,
    validate_provider_admission_envelope_bytes, validate_provider_admission_renewal_bytes,
    validate_provider_admission_revocation_bytes, validate_provider_advert_bytes,
    validate_repair_payload_bytes, validate_replication_order_bytes,
    validate_signed_replication_order_bytes,
};

/// FFI repair payload kind selector for `RepairEvidenceV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_EVIDENCE: u32 = 1;
/// FFI repair payload kind selector for `RepairReportV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_REPORT: u32 = 2;
/// FFI repair payload kind selector for `RepairTaskRecordV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_TASK_RECORD: u32 = 3;
/// FFI repair payload kind selector for `RepairSlashProposalV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_SLASH_PROPOSAL: u32 = 4;
/// FFI repair payload kind selector for `RepairEscalationPolicyV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_ESCALATION_POLICY: u32 = 5;
/// FFI repair payload kind selector for `RepairEscalationApprovalV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_ESCALATION_APPROVAL: u32 = 6;
/// FFI repair payload kind selector for `SignedAuditorRequestV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_SIGNED_AUDITOR_REQUEST: u32 = 7;
/// FFI repair payload kind selector for `RepairWorkerSignaturePayloadV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_WORKER_SIGNATURE: u32 = 8;
/// FFI repair payload kind selector for `RepairTaskEventV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_TASK_EVENT: u32 = 9;
/// FFI repair payload kind selector for `RepairAuditEventV1`.
pub const SORAFS_REFERENCE_REPAIR_KIND_AUDIT_EVENT: u32 = 10;

/// FFI orderbook payload kind selector for `OrderRequestV1`.
pub const SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST: u32 = 1;
/// FFI orderbook payload kind selector for `OrderCancelV1`.
pub const SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_CANCEL: u32 = 2;
/// FFI orderbook payload kind selector for `TradeEventV1`.
pub const SORAFS_REFERENCE_ORDERBOOK_KIND_TRADE_EVENT: u32 = 3;
/// FFI orderbook payload kind selector for `SettlementChannelV1`.
pub const SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_CHANNEL: u32 = 4;
/// FFI orderbook payload kind selector for `SettlementReceiptV1`.
pub const SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_RECEIPT: u32 = 5;

/// FFI bundle payload kind selector for `ProviderAdvertV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_PROVIDER_ADVERT: u32 = 1;
/// FFI bundle payload kind selector for `ProviderAdmissionEnvelopeV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_PROVIDER_ADMISSION_ENVELOPE: u32 = 2;
/// FFI bundle payload kind selector for `ReplicationOrderV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_REPLICATION_ORDER: u32 = 3;
/// FFI bundle payload kind selector for `PorChallengeV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_POR_CHALLENGE: u32 = 4;
/// FFI bundle payload kind selector for `PorProofV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_POR_PROOF: u32 = 5;
/// FFI bundle payload kind selector for `PotrReceiptV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_POTR_RECEIPT: u32 = 6;
/// FFI bundle payload kind selector for `RepairEvidenceV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_EVIDENCE: u32 = 7;
/// FFI bundle payload kind selector for `RepairReportV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_REPORT: u32 = 8;
/// FFI bundle payload kind selector for `RepairTaskRecordV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_TASK_RECORD: u32 = 9;
/// FFI bundle payload kind selector for `RepairSlashProposalV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_SLASH_PROPOSAL: u32 = 10;
/// FFI bundle payload kind selector for `RepairTaskEventV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_TASK_EVENT: u32 = 11;
/// FFI bundle payload kind selector for `OrderRequestV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_ORDER_REQUEST: u32 = 12;
/// FFI bundle payload kind selector for `OrderCancelV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_ORDER_CANCEL: u32 = 13;
/// FFI bundle payload kind selector for `TradeEventV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_TRADE_EVENT: u32 = 14;
/// FFI bundle payload kind selector for `SettlementChannelV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_SETTLEMENT_CHANNEL: u32 = 15;
/// FFI bundle payload kind selector for `SettlementReceiptV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_SETTLEMENT_RECEIPT: u32 = 16;
/// FFI bundle payload kind selector for `PdpCommitmentV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_PDP_COMMITMENT: u32 = 17;
/// FFI bundle payload kind selector for `PdpChallengeV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_PDP_CHALLENGE: u32 = 18;
/// FFI bundle payload kind selector for `PdpProofV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_PDP_PROOF: u32 = 19;

/// FFI proof-stream profile selector for an omitted PoTR profile.
pub const SORAFS_REFERENCE_PROFILE_NONE: u32 = 0;
/// FFI proof-stream profile selector for hot retrieval.
pub const SORAFS_REFERENCE_PROFILE_HOT: u32 = 1;
/// FFI proof-stream profile selector for warm retrieval.
pub const SORAFS_REFERENCE_PROFILE_WARM: u32 = 2;
/// FFI proof-stream profile selector for archive retrieval.
pub const SORAFS_REFERENCE_PROFILE_ARCHIVE: u32 = 3;

const CATEGORY_INTERNAL: &str = "internal";
const SFS_FFI_ARGUMENT: &str = "SFS-FFI-001";
const SFS_FFI_PANIC: &str = "SFS-FFI-002";

struct FfiInputScope;

/// Owned bytes returned from the SoraFS reference FFI.
///
/// Call [`sorafs_reference_free_buffer`] exactly once when the caller is done
/// reading the bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SorafsReferenceFfiBuffer {
    /// Pointer to `len` bytes allocated by Rust.
    pub ptr: *mut u8,
    /// Number of bytes at `ptr`.
    pub len: usize,
}

impl SorafsReferenceFfiBuffer {
    fn from_bytes(mut bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        if len == 0 {
            return Self {
                ptr: std::ptr::null_mut(),
                len: 0,
            };
        }
        let ptr = bytes.as_mut_ptr();
        std::mem::forget(bytes);
        Self { ptr, len }
    }
}

/// Payload descriptor used by [`sorafs_reference_validate_bundle_json`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SorafsReferenceFfiBundlePayload {
    /// One of the `SORAFS_REFERENCE_BUNDLE_KIND_*` constants.
    pub kind: u32,
    /// Pointer to Norito payload bytes.
    pub bytes_ptr: *const u8,
    /// Length of the Norito payload in bytes.
    pub bytes_len: usize,
    /// Pointer to an optional UTF-8 label.
    pub label_ptr: *const u8,
    /// Length of the optional UTF-8 label.
    pub label_len: usize,
}

/// Free a buffer returned by the SoraFS reference FFI.
///
/// # Safety
/// `buffer` must have been returned by this crate and must not be freed more
/// than once. Passing a null or zero-length buffer is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_free_buffer(buffer: SorafsReferenceFfiBuffer) {
    if buffer.ptr.is_null() || buffer.len == 0 {
        return;
    }
    // SAFETY: callers pass buffers returned by this crate, which were allocated
    // from a `Vec<u8>` with length equal to capacity.
    unsafe {
        drop(Vec::from_raw_parts(buffer.ptr, buffer.len, buffer.len));
    }
}

/// Validate a Norito-encoded `ProviderAdvertV1` and return outcome JSON.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_provider_advert_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    now: u64,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(
            &scope,
            bytes_ptr,
            bytes_len,
            "provider_advert",
            generated_at,
        )?;
        let label = read_label(
            &scope,
            label_ptr,
            label_len,
            "provider_advert.to",
            generated_at,
        )?;
        Ok(validate_provider_advert_bytes(
            input,
            label,
            now,
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded `ProviderAdmissionEnvelopeV1` and return outcome JSON.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_provider_admission_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(
            &scope,
            bytes_ptr,
            bytes_len,
            "provider_admission_envelope",
            generated_at,
        )?;
        let label = read_label(&scope, label_ptr, label_len, "admission.to", generated_at)?;
        Ok(validate_provider_admission_envelope_bytes(
            input,
            label,
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded `ProviderAdmissionRenewalV1` against an envelope.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_provider_admission_renewal_json(
    envelope_ptr: *const u8,
    envelope_len: usize,
    envelope_label_ptr: *const u8,
    envelope_label_len: usize,
    renewal_ptr: *const u8,
    renewal_len: usize,
    renewal_label_ptr: *const u8,
    renewal_label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let envelope = read_input(
            &scope,
            envelope_ptr,
            envelope_len,
            "provider_admission_envelope",
            generated_at,
        )?;
        let renewal = read_input(
            &scope,
            renewal_ptr,
            renewal_len,
            "provider_admission_renewal",
            generated_at,
        )?;
        let envelope_label = read_label(
            &scope,
            envelope_label_ptr,
            envelope_label_len,
            "admission-envelope.to",
            generated_at,
        )?;
        let renewal_label = read_label(
            &scope,
            renewal_label_ptr,
            renewal_label_len,
            "admission-renewal.to",
            generated_at,
        )?;
        Ok(validate_provider_admission_renewal_bytes(
            envelope,
            renewal,
            envelope_label,
            renewal_label,
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded `ProviderAdmissionRevocationV1` against an envelope.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_provider_admission_revocation_json(
    envelope_ptr: *const u8,
    envelope_len: usize,
    envelope_label_ptr: *const u8,
    envelope_label_len: usize,
    revocation_ptr: *const u8,
    revocation_len: usize,
    revocation_label_ptr: *const u8,
    revocation_label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let envelope = read_input(
            &scope,
            envelope_ptr,
            envelope_len,
            "provider_admission_envelope",
            generated_at,
        )?;
        let revocation = read_input(
            &scope,
            revocation_ptr,
            revocation_len,
            "provider_admission_revocation",
            generated_at,
        )?;
        let envelope_label = read_label(
            &scope,
            envelope_label_ptr,
            envelope_label_len,
            "admission-envelope.to",
            generated_at,
        )?;
        let revocation_label = read_label(
            &scope,
            revocation_label_ptr,
            revocation_label_len,
            "admission-revocation.to",
            generated_at,
        )?;
        Ok(validate_provider_admission_revocation_bytes(
            envelope,
            revocation,
            envelope_label,
            revocation_label,
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded `ReplicationOrderV1` and return outcome JSON.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_replication_order_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(
            &scope,
            bytes_ptr,
            bytes_len,
            "replication_order",
            generated_at,
        )?;
        let label = read_label(&scope, label_ptr, label_len, "order.to", generated_at)?;
        Ok(validate_replication_order_bytes(input, label, generated_at))
    })
}

/// Validate a Norito-encoded `SignedReplicationOrderV1` and return outcome JSON.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_signed_replication_order_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(
            &scope,
            bytes_ptr,
            bytes_len,
            "signed_replication_order",
            generated_at,
        )?;
        let label = read_label(
            &scope,
            label_ptr,
            label_len,
            "signed-order.to",
            generated_at,
        )?;
        Ok(validate_signed_replication_order_bytes(
            input,
            label,
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded orderbook payload and return outcome JSON.
///
/// `kind` must be one of the `SORAFS_REFERENCE_ORDERBOOK_KIND_*` constants.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_orderbook_json(
    kind: u32,
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let kind = orderbook_kind_from_ffi(kind, generated_at)?;
        let input = read_input(
            &scope,
            bytes_ptr,
            bytes_len,
            "orderbook_payload",
            generated_at,
        )?;
        let label = read_label(&scope, label_ptr, label_len, "orderbook.to", generated_at)?;
        Ok(validate_orderbook_payload_bytes(
            kind,
            input,
            label,
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded `PdpCommitmentV1` and return outcome JSON.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_pdp_commitment_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(&scope, bytes_ptr, bytes_len, "pdp_commitment", generated_at)?;
        let label = read_label(&scope, label_ptr, label_len, "commitment.to", generated_at)?;
        Ok(validate_pdp_commitment_bytes(input, label, generated_at))
    })
}

/// Validate a Norito-encoded `PdpChallengeV1` and return outcome JSON.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_pdp_challenge_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(&scope, bytes_ptr, bytes_len, "pdp_challenge", generated_at)?;
        let label = read_label(&scope, label_ptr, label_len, "challenge.to", generated_at)?;
        Ok(validate_pdp_challenge_bytes(input, label, generated_at))
    })
}

/// Validate a Norito-encoded `PdpProofV1` and return outcome JSON.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_pdp_proof_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(&scope, bytes_ptr, bytes_len, "pdp_proof", generated_at)?;
        let label = read_label(&scope, label_ptr, label_len, "proof.to", generated_at)?;
        Ok(validate_pdp_proof_bytes(input, label, generated_at))
    })
}

/// Validate Norito-encoded `PdpCommitmentV1` and `PdpChallengeV1` bytes.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_pdp_commitment_challenge_json(
    commitment_ptr: *const u8,
    commitment_len: usize,
    commitment_label_ptr: *const u8,
    commitment_label_len: usize,
    challenge_ptr: *const u8,
    challenge_len: usize,
    challenge_label_ptr: *const u8,
    challenge_label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let commitment = read_input(
            &scope,
            commitment_ptr,
            commitment_len,
            "pdp_commitment",
            generated_at,
        )?;
        let challenge = read_input(
            &scope,
            challenge_ptr,
            challenge_len,
            "pdp_challenge",
            generated_at,
        )?;
        let commitment_label = read_label(
            &scope,
            commitment_label_ptr,
            commitment_label_len,
            "commitment.to",
            generated_at,
        )?;
        let challenge_label = read_label(
            &scope,
            challenge_label_ptr,
            challenge_label_len,
            "challenge.to",
            generated_at,
        )?;
        Ok(validate_pdp_commitment_challenge_bytes(
            commitment,
            challenge,
            commitment_label,
            challenge_label,
            generated_at,
        ))
    })
}

/// Validate Norito-encoded `PdpChallengeV1` and `PdpProofV1` bytes.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_pdp_challenge_proof_json(
    challenge_ptr: *const u8,
    challenge_len: usize,
    challenge_label_ptr: *const u8,
    challenge_label_len: usize,
    proof_ptr: *const u8,
    proof_len: usize,
    proof_label_ptr: *const u8,
    proof_label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let challenge = read_input(
            &scope,
            challenge_ptr,
            challenge_len,
            "pdp_challenge",
            generated_at,
        )?;
        let proof = read_input(&scope, proof_ptr, proof_len, "pdp_proof", generated_at)?;
        let challenge_label = read_label(
            &scope,
            challenge_label_ptr,
            challenge_label_len,
            "challenge.to",
            generated_at,
        )?;
        let proof_label = read_label(
            &scope,
            proof_label_ptr,
            proof_label_len,
            "proof.to",
            generated_at,
        )?;
        Ok(validate_pdp_challenge_proof_bytes(
            challenge,
            proof,
            challenge_label,
            proof_label,
            generated_at,
        ))
    })
}

/// Validate Norito-encoded PDP commitment, challenge, and proof bytes.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_pdp_json(
    commitment_ptr: *const u8,
    commitment_len: usize,
    commitment_label_ptr: *const u8,
    commitment_label_len: usize,
    challenge_ptr: *const u8,
    challenge_len: usize,
    challenge_label_ptr: *const u8,
    challenge_label_len: usize,
    proof_ptr: *const u8,
    proof_len: usize,
    proof_label_ptr: *const u8,
    proof_label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let commitment = read_input(
            &scope,
            commitment_ptr,
            commitment_len,
            "pdp_commitment",
            generated_at,
        )?;
        let challenge = read_input(
            &scope,
            challenge_ptr,
            challenge_len,
            "pdp_challenge",
            generated_at,
        )?;
        let proof = read_input(&scope, proof_ptr, proof_len, "pdp_proof", generated_at)?;
        let commitment_label = read_label(
            &scope,
            commitment_label_ptr,
            commitment_label_len,
            "commitment.to",
            generated_at,
        )?;
        let challenge_label = read_label(
            &scope,
            challenge_label_ptr,
            challenge_label_len,
            "challenge.to",
            generated_at,
        )?;
        let proof_label = read_label(
            &scope,
            proof_label_ptr,
            proof_label_len,
            "proof.to",
            generated_at,
        )?;
        Ok(validate_pdp_commitment_challenge_proof_bytes(
            commitment,
            challenge,
            proof,
            commitment_label,
            challenge_label,
            proof_label,
            generated_at,
        ))
    })
}

/// Validate Norito-encoded `PorChallengeV1` and `PorProofV1` bytes.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_por_json(
    challenge_ptr: *const u8,
    challenge_len: usize,
    challenge_label_ptr: *const u8,
    challenge_label_len: usize,
    proof_ptr: *const u8,
    proof_len: usize,
    proof_label_ptr: *const u8,
    proof_label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let challenge = read_input(
            &scope,
            challenge_ptr,
            challenge_len,
            "por_challenge",
            generated_at,
        )?;
        let proof = read_input(&scope, proof_ptr, proof_len, "por_proof", generated_at)?;
        let challenge_label = read_label(
            &scope,
            challenge_label_ptr,
            challenge_label_len,
            "challenge.to",
            generated_at,
        )?;
        let proof_label = read_label(
            &scope,
            proof_label_ptr,
            proof_label_len,
            "proof.to",
            generated_at,
        )?;
        Ok(validate_por_challenge_proof_bytes(
            challenge,
            proof,
            challenge_label,
            proof_label,
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded `PotrReceiptV1` and return outcome JSON.
///
/// `profile` must be one of the `SORAFS_REFERENCE_PROFILE_*` constants.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_potr_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    profile: u32,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(&scope, bytes_ptr, bytes_len, "potr_receipt", generated_at)?;
        let label = read_label(&scope, label_ptr, label_len, "receipt.to", generated_at)?;
        let profile = profile_from_ffi(profile, generated_at)?;
        Ok(validate_potr_receipt_bytes(
            input,
            label,
            profile,
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded repair payload and return outcome JSON.
///
/// `kind` must be one of the `SORAFS_REFERENCE_REPAIR_KIND_*` constants.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_repair_json(
    kind: u32,
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let kind = repair_kind_from_ffi(kind, generated_at)?;
        let input = read_input(&scope, bytes_ptr, bytes_len, "repair_payload", generated_at)?;
        let label = read_label(&scope, label_ptr, label_len, "repair.to", generated_at)?;
        Ok(validate_repair_payload_bytes(
            kind,
            input,
            label,
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded `GovernanceLogNodeV1` and return outcome JSON.
///
/// Pass `expected_cid_len = 0` to skip exact node-CID binding.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_governance_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    expected_cid_ptr: *const u8,
    expected_cid_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(
            &scope,
            bytes_ptr,
            bytes_len,
            "governance_log_node",
            generated_at,
        )?;
        let label = read_label(&scope, label_ptr, label_len, "governance.to", generated_at)?;
        let expected_cid = read_optional_input(
            &scope,
            expected_cid_ptr,
            expected_cid_len,
            "expected_node_cid",
            generated_at,
        )?;
        Ok(validate_governance_log_node_bytes(
            input,
            label,
            expected_cid,
            generated_at,
        ))
    })
}

/// Validate a fixture bundle payload set and return outcome JSON.
///
/// `payloads_ptr` must reference `payloads_len` descriptors. Each descriptor
/// kind must be one of the `SORAFS_REFERENCE_BUNDLE_KIND_*` constants.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_bundle_json(
    payloads_ptr: *const SorafsReferenceFfiBundlePayload,
    payloads_len: usize,
    now: u64,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let payload_descriptors =
            read_payload_descriptors(&scope, payloads_ptr, payloads_len, generated_at)?;
        let mut payloads = Vec::with_capacity(payload_descriptors.len());
        for (index, payload) in payload_descriptors.iter().enumerate() {
            let kind = bundle_kind_from_ffi(payload.kind, generated_at)?;
            let bytes = read_input(
                &scope,
                payload.bytes_ptr,
                payload.bytes_len,
                format!("bundle_payload_{index}"),
                generated_at,
            )?;
            let label = read_label(
                &scope,
                payload.label_ptr,
                payload.label_len,
                format!("bundle-payload-{index}.to"),
                generated_at,
            )?;
            payloads.push(FixtureBundlePayloadV1::new(kind, label, bytes));
        }
        Ok(validate_fixture_bundle_payloads(
            &payloads,
            now,
            generated_at,
        ))
    })
}

fn run_ffi(
    generated_at: u64,
    validate: impl FnOnce() -> Result<ValidationOutcomeV1, ValidationOutcomeV1>,
) -> SorafsReferenceFfiBuffer {
    let outcome = match panic::catch_unwind(panic::AssertUnwindSafe(validate)) {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(outcome)) => outcome,
        Err(_) => ffi_error(
            SFS_FFI_PANIC,
            "SoraFS reference FFI validator panicked",
            "Report the input payload and validator version; do not treat this outcome as accepted.",
            vec![ValidationContextFieldV1::new("ffi_error", "panic")],
            generated_at,
        ),
    };
    outcome_json_buffer(&outcome)
}

fn outcome_json_buffer(outcome: &ValidationOutcomeV1) -> SorafsReferenceFfiBuffer {
    match json::to_string_pretty(outcome) {
        Ok(mut rendered) => {
            rendered.push('\n');
            SorafsReferenceFfiBuffer::from_bytes(rendered.into_bytes())
        }
        Err(_) => SorafsReferenceFfiBuffer::from_bytes(
            b"{\"status\":\"Error\",\"code\":\"SFS-FFI-002\",\"category\":\"internal\",\"message\":\"failed to render SoraFS reference FFI outcome JSON\",\"action\":\"Report the validator version and input payload.\",\"docs_url\":\"docs/portal/docs/sorafs/reference-sdk/errors.md\",\"telemetry_tags\":[\"sorafs.reference.ffi\",\"sorafs.reference.code.SFS-FFI-002\"],\"context\":[],\"inputs\":[],\"version\":1,\"generated_at\":0}\n".to_vec(),
        ),
    }
}

fn read_input<'scope>(
    _scope: &'scope FfiInputScope,
    ptr: *const u8,
    len: usize,
    label: impl Into<String>,
    generated_at: u64,
) -> Result<&'scope [u8], ValidationOutcomeV1> {
    if len == 0 {
        return Ok(&[]);
    }
    let label = label.into();
    if ptr.is_null() {
        return Err(null_pointer_error(label, generated_at));
    }
    // SAFETY: FFI callers must provide a pointer valid for `len` bytes.
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

fn read_optional_input<'scope>(
    scope: &'scope FfiInputScope,
    ptr: *const u8,
    len: usize,
    label: impl Into<String>,
    generated_at: u64,
) -> Result<Option<&'scope [u8]>, ValidationOutcomeV1> {
    if len == 0 {
        return Ok(None);
    }
    read_input(scope, ptr, len, label, generated_at).map(Some)
}

fn read_label(
    scope: &FfiInputScope,
    ptr: *const u8,
    len: usize,
    default: impl Into<String>,
    generated_at: u64,
) -> Result<String, ValidationOutcomeV1> {
    let default = default.into();
    let Some(bytes) = read_optional_input(scope, ptr, len, default.clone(), generated_at)? else {
        return Ok(default);
    };
    Ok(String::from_utf8_lossy(bytes).to_string())
}

fn read_payload_descriptors<'scope>(
    _scope: &'scope FfiInputScope,
    ptr: *const SorafsReferenceFfiBundlePayload,
    len: usize,
    generated_at: u64,
) -> Result<&'scope [SorafsReferenceFfiBundlePayload], ValidationOutcomeV1> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(null_pointer_error("bundle_payloads", generated_at));
    }
    // SAFETY: FFI callers must provide a pointer valid for `len` descriptors.
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

fn profile_from_ffi(
    profile: u32,
    generated_at: u64,
) -> Result<Option<ProofStreamTier>, ValidationOutcomeV1> {
    match profile {
        SORAFS_REFERENCE_PROFILE_NONE => Ok(None),
        SORAFS_REFERENCE_PROFILE_HOT => Ok(Some(ProofStreamTier::Hot)),
        SORAFS_REFERENCE_PROFILE_WARM => Ok(Some(ProofStreamTier::Warm)),
        SORAFS_REFERENCE_PROFILE_ARCHIVE => Ok(Some(ProofStreamTier::Archive)),
        other => Err(unsupported_selector_error("profile", other, generated_at)),
    }
}

fn repair_kind_from_ffi(
    kind: u32,
    generated_at: u64,
) -> Result<RepairValidationPayloadKindV1, ValidationOutcomeV1> {
    match kind {
        SORAFS_REFERENCE_REPAIR_KIND_EVIDENCE => Ok(RepairValidationPayloadKindV1::Evidence),
        SORAFS_REFERENCE_REPAIR_KIND_REPORT => Ok(RepairValidationPayloadKindV1::Report),
        SORAFS_REFERENCE_REPAIR_KIND_TASK_RECORD => Ok(RepairValidationPayloadKindV1::TaskRecord),
        SORAFS_REFERENCE_REPAIR_KIND_SLASH_PROPOSAL => {
            Ok(RepairValidationPayloadKindV1::SlashProposal)
        }
        SORAFS_REFERENCE_REPAIR_KIND_ESCALATION_POLICY => {
            Ok(RepairValidationPayloadKindV1::EscalationPolicy)
        }
        SORAFS_REFERENCE_REPAIR_KIND_ESCALATION_APPROVAL => {
            Ok(RepairValidationPayloadKindV1::EscalationApproval)
        }
        SORAFS_REFERENCE_REPAIR_KIND_SIGNED_AUDITOR_REQUEST => {
            Ok(RepairValidationPayloadKindV1::SignedAuditorRequest)
        }
        SORAFS_REFERENCE_REPAIR_KIND_WORKER_SIGNATURE => {
            Ok(RepairValidationPayloadKindV1::WorkerSignaturePayload)
        }
        SORAFS_REFERENCE_REPAIR_KIND_TASK_EVENT => Ok(RepairValidationPayloadKindV1::TaskEvent),
        SORAFS_REFERENCE_REPAIR_KIND_AUDIT_EVENT => Ok(RepairValidationPayloadKindV1::AuditEvent),
        other => Err(unsupported_selector_error(
            "repair_kind",
            other,
            generated_at,
        )),
    }
}

fn orderbook_kind_from_ffi(
    kind: u32,
    generated_at: u64,
) -> Result<OrderbookValidationPayloadKindV1, ValidationOutcomeV1> {
    match kind {
        SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST => {
            Ok(OrderbookValidationPayloadKindV1::OrderRequest)
        }
        SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_CANCEL => {
            Ok(OrderbookValidationPayloadKindV1::OrderCancel)
        }
        SORAFS_REFERENCE_ORDERBOOK_KIND_TRADE_EVENT => {
            Ok(OrderbookValidationPayloadKindV1::TradeEvent)
        }
        SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_CHANNEL => {
            Ok(OrderbookValidationPayloadKindV1::SettlementChannel)
        }
        SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_RECEIPT => {
            Ok(OrderbookValidationPayloadKindV1::SettlementReceipt)
        }
        other => Err(unsupported_selector_error(
            "orderbook_kind",
            other,
            generated_at,
        )),
    }
}

fn bundle_kind_from_ffi(
    kind: u32,
    generated_at: u64,
) -> Result<FixtureBundlePayloadKindV1, ValidationOutcomeV1> {
    match kind {
        SORAFS_REFERENCE_BUNDLE_KIND_PROVIDER_ADVERT => {
            Ok(FixtureBundlePayloadKindV1::ProviderAdvert)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_PROVIDER_ADMISSION_ENVELOPE => {
            Ok(FixtureBundlePayloadKindV1::ProviderAdmissionEnvelope)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_REPLICATION_ORDER => {
            Ok(FixtureBundlePayloadKindV1::ReplicationOrder)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_POR_CHALLENGE => Ok(FixtureBundlePayloadKindV1::PorChallenge),
        SORAFS_REFERENCE_BUNDLE_KIND_POR_PROOF => Ok(FixtureBundlePayloadKindV1::PorProof),
        SORAFS_REFERENCE_BUNDLE_KIND_POTR_RECEIPT => Ok(FixtureBundlePayloadKindV1::PotrReceipt),
        SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_EVIDENCE => {
            Ok(FixtureBundlePayloadKindV1::RepairEvidence)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_REPORT => Ok(FixtureBundlePayloadKindV1::RepairReport),
        SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_TASK_RECORD => {
            Ok(FixtureBundlePayloadKindV1::RepairTaskRecord)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_SLASH_PROPOSAL => {
            Ok(FixtureBundlePayloadKindV1::RepairSlashProposal)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_REPAIR_TASK_EVENT => {
            Ok(FixtureBundlePayloadKindV1::RepairTaskEvent)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_ORDER_REQUEST => {
            Ok(FixtureBundlePayloadKindV1::OrderbookOrderRequest)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_ORDER_CANCEL => {
            Ok(FixtureBundlePayloadKindV1::OrderbookOrderCancel)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_TRADE_EVENT => {
            Ok(FixtureBundlePayloadKindV1::OrderbookTradeEvent)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_SETTLEMENT_CHANNEL => {
            Ok(FixtureBundlePayloadKindV1::OrderbookSettlementChannel)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_SETTLEMENT_RECEIPT => {
            Ok(FixtureBundlePayloadKindV1::OrderbookSettlementReceipt)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_PDP_COMMITMENT => {
            Ok(FixtureBundlePayloadKindV1::PdpCommitment)
        }
        SORAFS_REFERENCE_BUNDLE_KIND_PDP_CHALLENGE => Ok(FixtureBundlePayloadKindV1::PdpChallenge),
        SORAFS_REFERENCE_BUNDLE_KIND_PDP_PROOF => Ok(FixtureBundlePayloadKindV1::PdpProof),
        other => Err(unsupported_selector_error(
            "bundle_kind",
            other,
            generated_at,
        )),
    }
}

fn null_pointer_error(label: impl Into<String>, generated_at: u64) -> ValidationOutcomeV1 {
    ffi_error(
        SFS_FFI_ARGUMENT,
        "SoraFS reference FFI received a null pointer for a non-empty input",
        "Pass a valid pointer for every non-zero length argument.",
        vec![ValidationContextFieldV1::new("argument", label.into())],
        generated_at,
    )
}

fn unsupported_selector_error(
    selector: &str,
    value: u32,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    ffi_error(
        SFS_FFI_ARGUMENT,
        format!("unsupported SoraFS reference FFI selector `{selector}`"),
        "Use the selector constants exported by the SoraFS reference FFI.",
        vec![
            ValidationContextFieldV1::new("selector", selector),
            ValidationContextFieldV1::new("value", value.to_string()),
        ],
        generated_at,
    )
}

fn ffi_error(
    code: impl Into<String>,
    message: impl Into<String>,
    action: impl Into<String>,
    context: Vec<ValidationContextFieldV1>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    let code = code.into();
    ValidationOutcomeV1::error(
        code.clone(),
        CATEGORY_INTERNAL,
        message,
        action,
        vec![
            "sorafs.reference.ffi".to_owned(),
            format!("sorafs.reference.code.{code}"),
        ],
        context,
        vec![ValidationInputV1::new("ffi", "sorafs_reference_ffi")],
        generated_at,
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, slice};

    use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signer, SigningKey};
    use norito::json::Value;

    use crate::{
        ByteRangeV1, OrderbookSignatureV1, ReplicationOrderSignatureV1, ReplicationOrderV1,
        SETTLEMENT_RECEIPT_VERSION_V1, SIGNED_REPLICATION_ORDER_VERSION_V1, SettlementReceiptV1,
        SignatureAlgorithm, SignedReplicationOrderV1, XorAmount,
    };

    use super::*;

    fn workspace_fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    }

    unsafe fn read_and_free(buffer: SorafsReferenceFfiBuffer) -> Vec<u8> {
        let bytes = if buffer.ptr.is_null() || buffer.len == 0 {
            Vec::new()
        } else {
            // SAFETY: the buffer was returned by the FFI functions under test.
            unsafe { slice::from_raw_parts(buffer.ptr, buffer.len).to_vec() }
        };
        // SAFETY: the buffer was returned by the FFI functions under test.
        unsafe {
            sorafs_reference_free_buffer(buffer);
        }
        bytes
    }

    fn outcome_from_buffer(buffer: SorafsReferenceFfiBuffer) -> Value {
        // SAFETY: test helper frees exactly the buffer returned by the FFI call.
        let bytes = unsafe { read_and_free(buffer) };
        json::from_slice(&bytes).expect("parse FFI outcome JSON")
    }

    fn orderbook_settlement_receipt() -> SettlementReceiptV1 {
        SettlementReceiptV1 {
            version: SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: [0x81; 32],
            channel_id: [0x82; 32],
            trade_id: [0x83; 32],
            range: ByteRangeV1 {
                start: 128,
                end: 384,
            },
            chunk_hash: [0x84; 32],
            bytes_delivered: 256,
            xor_debited: XorAmount::from_micro(100),
            provider_credit: XorAmount::from_micro(90),
            fee_amount: XorAmount::from_micro(10),
            issued_at_unix: 1_800_000_010,
            settlement_signature: OrderbookSignatureV1 {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0xD7; PUBLIC_KEY_LENGTH],
                signature: vec![0x57; SIGNATURE_LENGTH],
            },
        }
    }

    #[test]
    fn ffi_provider_advert_validator_returns_json_outcome() {
        let bytes = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/provider_admission/advert_v1.to",
        ))
        .expect("read advert fixture");
        let label = b"advert.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_provider_advert_json(
                bytes.as_ptr(),
                bytes.len(),
                label.as_ptr(),
                label.len(),
                120,
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
    }

    #[test]
    fn ffi_provider_admission_renewal_validator_returns_json_outcome() {
        let envelope = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/provider_admission/envelope_v1.to",
        ))
        .expect("read admission envelope fixture");
        let renewal = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/provider_admission/renewal_v1.to",
        ))
        .expect("read admission renewal fixture");
        let envelope_label = b"envelope.to";
        let renewal_label = b"renewal.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_provider_admission_renewal_json(
                envelope.as_ptr(),
                envelope.len(),
                envelope_label.as_ptr(),
                envelope_label.len(),
                renewal.as_ptr(),
                renewal.len(),
                renewal_label.as_ptr(),
                renewal_label.len(),
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
    }

    #[test]
    fn ffi_provider_admission_revocation_validator_returns_json_outcome() {
        let envelope = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/provider_admission/envelope_v1.to",
        ))
        .expect("read admission envelope fixture");
        let revocation = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/provider_admission/revocation_v1.to",
        ))
        .expect("read admission revocation fixture");
        let envelope_label = b"envelope.to";
        let revocation_label = b"revocation.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_provider_admission_revocation_json(
                envelope.as_ptr(),
                envelope.len(),
                envelope_label.as_ptr(),
                envelope_label.len(),
                revocation.as_ptr(),
                revocation.len(),
                revocation_label.as_ptr(),
                revocation_label.len(),
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
    }

    #[test]
    fn ffi_governance_validator_checks_expected_cid() {
        let bytes = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/governance/node_v1.to",
        ))
        .expect("read governance fixture");
        let label = b"governance.to";
        let cid = b"bafywronggovernancenode";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_governance_json(
                bytes.as_ptr(),
                bytes.len(),
                label.as_ptr(),
                label.len(),
                cid.as_ptr(),
                cid.len(),
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-GOV-003")
        );
    }

    #[test]
    fn ffi_signed_replication_order_validator_returns_json_outcome() {
        let order_bytes = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/replication_order/order_v1.to",
        ))
        .expect("read order fixture");
        let order: ReplicationOrderV1 =
            norito::decode_from_bytes(&order_bytes).expect("decode order fixture");
        let signing_key = SigningKey::from_bytes(&[0xA7; 32]);
        let mut signed_order = SignedReplicationOrderV1 {
            version: SIGNED_REPLICATION_ORDER_VERSION_V1,
            order,
            signature: ReplicationOrderSignatureV1 {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: vec![0; 64],
            },
        };
        let payload_bytes = signed_order
            .signature_payload_bytes()
            .expect("encode signed order payload");
        signed_order.signature.signature = signing_key.sign(&payload_bytes).to_bytes().to_vec();
        let bytes = norito::to_bytes(&signed_order).expect("encode signed order");
        let label = b"signed-order.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_signed_replication_order_json(
                bytes.as_ptr(),
                bytes.len(),
                label.as_ptr(),
                label.len(),
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
    }

    #[test]
    fn ffi_orderbook_validator_returns_json_outcome() {
        let receipt = orderbook_settlement_receipt();
        let bytes = norito::to_bytes(&receipt).expect("encode settlement receipt");
        let label = b"settlement-receipt.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_orderbook_json(
                SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_RECEIPT,
                bytes.as_ptr(),
                bytes.len(),
                label.as_ptr(),
                label.len(),
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
    }

    #[test]
    fn ffi_pdp_validator_returns_json_outcome() {
        let commitment = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/pdp/commitment_v1.to",
        ))
        .expect("read PDP commitment fixture");
        let challenge = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/pdp/challenge_v1.to",
        ))
        .expect("read PDP challenge fixture");
        let proof = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/pdp/proof_v1.to",
        ))
        .expect("read PDP proof fixture");
        let commitment_label = b"commitment.to";
        let challenge_label = b"challenge.to";
        let proof_label = b"proof.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let commitment_outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_pdp_commitment_json(
                commitment.as_ptr(),
                commitment.len(),
                commitment_label.as_ptr(),
                commitment_label.len(),
                123,
            )
        });
        assert_eq!(
            commitment_outcome.get("status").and_then(Value::as_str),
            Some("Ok")
        );

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let challenge_outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_pdp_challenge_json(
                challenge.as_ptr(),
                challenge.len(),
                challenge_label.as_ptr(),
                challenge_label.len(),
                123,
            )
        });
        assert_eq!(
            challenge_outcome.get("status").and_then(Value::as_str),
            Some("Ok")
        );

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let commitment_challenge_outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_pdp_commitment_challenge_json(
                commitment.as_ptr(),
                commitment.len(),
                commitment_label.as_ptr(),
                commitment_label.len(),
                challenge.as_ptr(),
                challenge.len(),
                challenge_label.as_ptr(),
                challenge_label.len(),
                123,
            )
        });
        assert_eq!(
            commitment_challenge_outcome
                .get("status")
                .and_then(Value::as_str),
            Some("Ok")
        );

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let challenge_proof_outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_pdp_challenge_proof_json(
                challenge.as_ptr(),
                challenge.len(),
                challenge_label.as_ptr(),
                challenge_label.len(),
                proof.as_ptr(),
                proof.len(),
                proof_label.as_ptr(),
                proof_label.len(),
                123,
            )
        });
        assert_eq!(
            challenge_proof_outcome
                .get("status")
                .and_then(Value::as_str),
            Some("Ok")
        );

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_pdp_json(
                commitment.as_ptr(),
                commitment.len(),
                commitment_label.as_ptr(),
                commitment_label.len(),
                challenge.as_ptr(),
                challenge.len(),
                challenge_label.as_ptr(),
                challenge_label.len(),
                proof.as_ptr(),
                proof.len(),
                proof_label.as_ptr(),
                proof_label.len(),
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
        let inputs = outcome
            .get("inputs")
            .and_then(Value::as_array)
            .expect("PDP FFI outcome should include inputs");
        assert!(
            inputs
                .iter()
                .any(|input| input.get("kind").and_then(Value::as_str) == Some("pdp_proof")),
            "{outcome:?}"
        );
    }

    #[test]
    fn ffi_pdp_proof_validator_rejects_negative_fixture() {
        let proof = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/pdp/negative/missing_signature_proof_v1.to",
        ))
        .expect("read negative PDP proof fixture");
        let label = b"missing-signature-proof.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_pdp_proof_json(
                proof.as_ptr(),
                proof.len(),
                label.as_ptr(),
                label.len(),
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-SIG-008")
        );
        assert_eq!(
            outcome.get("category").and_then(Value::as_str),
            Some("signature")
        );
    }

    #[test]
    fn ffi_bundle_validator_accepts_order_and_proof() {
        let order = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/replication_order/order_v1.to",
        ))
        .expect("read order fixture");
        let proof = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/por/proof_v1.to",
        ))
        .expect("read proof fixture");
        let order_label = b"order.to";
        let proof_label = b"proof.to";
        let payloads = [
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_REPLICATION_ORDER,
                bytes_ptr: order.as_ptr(),
                bytes_len: order.len(),
                label_ptr: order_label.as_ptr(),
                label_len: order_label.len(),
            },
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_POR_PROOF,
                bytes_ptr: proof.as_ptr(),
                bytes_len: proof.len(),
                label_ptr: proof_label.as_ptr(),
                label_len: proof_label.len(),
            },
        ];

        // SAFETY: payload descriptors point at live fixture bytes and labels.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_bundle_json(payloads.as_ptr(), payloads.len(), 120, 123)
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
    }

    #[test]
    fn ffi_bundle_validator_accepts_pdp_payloads() {
        let order = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/replication_order/order_v1.to",
        ))
        .expect("read order fixture");
        let commitment = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/pdp/commitment_v1.to",
        ))
        .expect("read PDP commitment fixture");
        let challenge = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/pdp/challenge_v1.to",
        ))
        .expect("read PDP challenge fixture");
        let proof = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/pdp/proof_v1.to",
        ))
        .expect("read PDP proof fixture");
        let order_label = b"order.to";
        let commitment_label = b"pdp/commitment_v1.to";
        let challenge_label = b"pdp/challenge_v1.to";
        let proof_label = b"pdp/proof_v1.to";
        let payloads = [
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_REPLICATION_ORDER,
                bytes_ptr: order.as_ptr(),
                bytes_len: order.len(),
                label_ptr: order_label.as_ptr(),
                label_len: order_label.len(),
            },
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_PDP_COMMITMENT,
                bytes_ptr: commitment.as_ptr(),
                bytes_len: commitment.len(),
                label_ptr: commitment_label.as_ptr(),
                label_len: commitment_label.len(),
            },
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_PDP_CHALLENGE,
                bytes_ptr: challenge.as_ptr(),
                bytes_len: challenge.len(),
                label_ptr: challenge_label.as_ptr(),
                label_len: challenge_label.len(),
            },
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_PDP_PROOF,
                bytes_ptr: proof.as_ptr(),
                bytes_len: proof.len(),
                label_ptr: proof_label.as_ptr(),
                label_len: proof_label.len(),
            },
        ];

        // SAFETY: payload descriptors point at live fixture bytes and labels.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_bundle_json(payloads.as_ptr(), payloads.len(), 120, 123)
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
    }

    #[test]
    fn ffi_bundle_validator_accepts_orderbook_payloads() {
        let order = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/replication_order/order_v1.to",
        ))
        .expect("read order fixture");
        let proof = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/por/proof_v1.to",
        ))
        .expect("read proof fixture");
        let receipt = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/orderbook/settlement_receipt_v1.to",
        ))
        .expect("read orderbook receipt fixture");
        let order_label = b"order.to";
        let proof_label = b"proof.to";
        let receipt_label = b"orderbook/settlement_receipt_v1.to";
        let payloads = [
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_REPLICATION_ORDER,
                bytes_ptr: order.as_ptr(),
                bytes_len: order.len(),
                label_ptr: order_label.as_ptr(),
                label_len: order_label.len(),
            },
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_POR_PROOF,
                bytes_ptr: proof.as_ptr(),
                bytes_len: proof.len(),
                label_ptr: proof_label.as_ptr(),
                label_len: proof_label.len(),
            },
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_SETTLEMENT_RECEIPT,
                bytes_ptr: receipt.as_ptr(),
                bytes_len: receipt.len(),
                label_ptr: receipt_label.as_ptr(),
                label_len: receipt_label.len(),
            },
        ];

        // SAFETY: payload descriptors point at live fixture bytes and labels.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_bundle_json(payloads.as_ptr(), payloads.len(), 120, 123)
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
    }

    #[test]
    fn ffi_rejects_null_non_empty_input() {
        // SAFETY: this intentionally passes a null pointer to validate error mapping.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_replication_order_json(
                std::ptr::null(),
                4,
                std::ptr::null(),
                0,
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-FFI-001")
        );
    }

    #[test]
    fn ffi_rejects_unknown_repair_kind() {
        let bytes = b"not norito";

        // SAFETY: the pointer references live test bytes for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_repair_json(
                999,
                bytes.as_ptr(),
                bytes.len(),
                std::ptr::null(),
                0,
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-FFI-001")
        );
    }

    #[test]
    fn ffi_rejects_unknown_orderbook_kind() {
        let bytes = b"not norito";

        // SAFETY: the pointer references live test bytes for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_orderbook_json(
                999,
                bytes.as_ptr(),
                bytes.len(),
                std::ptr::null(),
                0,
                123,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-FFI-001")
        );
    }
}
