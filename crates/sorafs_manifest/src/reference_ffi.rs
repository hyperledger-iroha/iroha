//! C ABI facade for SoraFS reference validators.
//!
//! The facade returns `ValidationOutcomeV1` encoded as Norito JSON so mobile and
//! scripting SDKs can call the Rust reference validators without duplicating
//! SoraFS wire-format logic.

// The FFI surface returns full validation outcomes as errors so every caller
// receives the same machine-readable diagnostics as the Rust reference CLI.
#![allow(clippy::result_large_err)]

use std::{mem, panic, slice, str};

use norito::json;

use crate::{
    FixtureBundlePayloadKindV1, FixtureBundlePayloadV1, HedgingValidationPayloadKindV1,
    OrderbookValidationPayloadKindV1, PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1,
    PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1, PDP_PROOF_MAX_CANONICAL_BYTES_V1,
    PopValidationPayloadKindV1, ProofStreamTier, RepairValidationPayloadKindV1,
    ValidationContextFieldV1, ValidationInputV1, ValidationOutcomeV1,
    validate_fixture_bundle_payloads, validate_governance_dag_block_bytes,
    validate_governance_dag_head_chain_bytes, validate_governance_log_node_bytes,
    validate_hedging_payload_bytes, validate_orderbook_payload_bytes, validate_pdp_challenge_bytes,
    validate_pdp_challenge_proof_bytes, validate_pdp_commitment_bytes,
    validate_pdp_commitment_challenge_bytes, validate_pdp_commitment_challenge_proof_bytes,
    validate_pdp_proof_bytes, validate_pop_payload_bytes, validate_por_challenge_proof_bytes,
    validate_potr_receipt_bytes, validate_provider_admission_envelope_bytes,
    validate_provider_admission_renewal_bytes, validate_provider_admission_revocation_bytes,
    validate_provider_advert_bytes, validate_repair_payload_bytes,
    validate_replication_order_bytes, validate_signed_replication_order_bytes,
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
/// FFI orderbook payload kind selector for `OrderbookRuntimeSnapshotV1`.
pub const SORAFS_REFERENCE_ORDERBOOK_KIND_RUNTIME_SNAPSHOT: u32 = 6;

/// FFI PoP payload kind selector for `PopCredentialV1`.
pub const SORAFS_REFERENCE_POP_KIND_CREDENTIAL: u32 = 1;
/// FFI PoP payload kind selector for `PopCommitmentRootV1`.
pub const SORAFS_REFERENCE_POP_KIND_COMMITMENT_ROOT: u32 = 2;
/// FFI PoP payload kind selector for `PopRevocationListV1`.
pub const SORAFS_REFERENCE_POP_KIND_REVOCATION_LIST: u32 = 3;
/// FFI PoP payload kind selector for `PopEnrollmentRequestV1`.
pub const SORAFS_REFERENCE_POP_KIND_ENROLLMENT_REQUEST: u32 = 4;
/// FFI PoP payload kind selector for `PopRenewalRequestV1`.
pub const SORAFS_REFERENCE_POP_KIND_RENEWAL_REQUEST: u32 = 5;
/// FFI PoP payload kind selector for `PopMembershipProofV1`.
pub const SORAFS_REFERENCE_POP_KIND_MEMBERSHIP_PROOF: u32 = 6;
/// FFI PoP payload kind selector for `PopIssuedCredentialBundleV1`.
pub const SORAFS_REFERENCE_POP_KIND_ISSUED_CREDENTIAL_BUNDLE: u32 = 7;

/// FFI hedging payload kind selector for `HedgingPriceFeedV1`.
pub const SORAFS_REFERENCE_HEDGING_KIND_PRICE_FEED: u32 = 1;
/// FFI hedging payload kind selector for `HedgingReferencePriceDecisionV1`.
pub const SORAFS_REFERENCE_HEDGING_KIND_REFERENCE_PRICE_DECISION: u32 = 2;
/// FFI hedging payload kind selector for `BillingLineItemV1`.
pub const SORAFS_REFERENCE_HEDGING_KIND_BILLING_LINE_ITEM: u32 = 3;
/// FFI hedging payload kind selector for `BillingStatementV1`.
pub const SORAFS_REFERENCE_HEDGING_KIND_BILLING_STATEMENT: u32 = 4;

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
/// FFI bundle payload kind selector for `OrderbookRuntimeSnapshotV1`.
pub const SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_RUNTIME_SNAPSHOT: u32 = 20;

/// FFI proof-stream profile selector for an omitted PoTR profile.
pub const SORAFS_REFERENCE_PROFILE_NONE: u32 = 0;
/// FFI proof-stream profile selector for hot retrieval.
pub const SORAFS_REFERENCE_PROFILE_HOT: u32 = 1;
/// FFI proof-stream profile selector for warm retrieval.
pub const SORAFS_REFERENCE_PROFILE_WARM: u32 = 2;
/// FFI proof-stream profile selector for archive retrieval.
pub const SORAFS_REFERENCE_PROFILE_ARCHIVE: u32 = 3;

/// Maximum number of governance DAG blocks accepted by one head-chain FFI call.
pub const SORAFS_REFERENCE_GOVERNANCE_DAG_MAX_BLOCKS_V1: u32 = 64;
/// Exact byte length of every first-release Governance DAG CID.
pub const SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1: u32 = 32;
/// Maximum aggregate input bytes accepted by one reference FFI call.
pub const SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES_V1: u32 = 67108864;
/// Maximum UTF-8 label bytes accepted by one reference FFI input.
pub const SORAFS_REFERENCE_FFI_MAX_LABEL_BYTES_V1: u32 = 1024;

const CATEGORY_INTERNAL: &str = "internal";
const SFS_FFI_ARGUMENT: &str = "SFS-FFI-001";
const SFS_FFI_PANIC: &str = "SFS-FFI-002";
const SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES: usize =
    SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES_V1 as usize;
const SORAFS_REFERENCE_FFI_MAX_LABEL_BYTES: usize =
    SORAFS_REFERENCE_FFI_MAX_LABEL_BYTES_V1 as usize;
const SORAFS_REFERENCE_FFI_MAX_BUNDLE_PAYLOADS: usize = 64;
const SORAFS_REFERENCE_FFI_MAX_BUNDLE_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const _: () = assert!(
    SORAFS_REFERENCE_GOVERNANCE_DAG_MAX_BLOCKS_V1 as usize
        == crate::GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1
);
const _: () = assert!(
    SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1 as usize == crate::GOVERNANCE_DAG_CID_BYTES_V1
);

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
    fn from_bytes(bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        if len == 0 {
            return Self {
                ptr: std::ptr::null_mut(),
                len: 0,
            };
        }
        let ptr = Box::into_raw(bytes.into_boxed_slice()).cast::<u8>();
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

/// Byte payload and UTF-8 label descriptor used by multi-input validators.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SorafsReferenceFfiInput {
    /// Pointer to payload bytes.
    pub bytes_ptr: *const u8,
    /// Length of the payload in bytes.
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
    if buffer.ptr.is_null() || buffer.len == 0 || buffer.len > isize::MAX as usize {
        return;
    }
    // SAFETY: callers pass buffers returned by this crate, which converts
    // returned bytes into boxed slices with capacity equal to length.
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

/// Validate a Norito-encoded PoP payload and return outcome JSON.
///
/// `kind` must be one of the `SORAFS_REFERENCE_POP_KIND_*` constants.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_pop_json(
    kind: u32,
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let kind = pop_kind_from_ffi(kind, generated_at)?;
        let input = read_input(&scope, bytes_ptr, bytes_len, "pop_payload", generated_at)?;
        let label = read_label(&scope, label_ptr, label_len, "pop.to", generated_at)?;
        Ok(validate_pop_payload_bytes(kind, input, label, generated_at))
    })
}

/// Validate a Norito-encoded hedging/billing payload and return outcome JSON.
///
/// `kind` must be one of the `SORAFS_REFERENCE_HEDGING_KIND_*` constants.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_hedging_json(
    kind: u32,
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let kind = hedging_kind_from_ffi(kind, generated_at)?;
        let input = read_input(
            &scope,
            bytes_ptr,
            bytes_len,
            "hedging_payload",
            generated_at,
        )?;
        let label = read_label(&scope, label_ptr, label_len, "hedging.to", generated_at)?;
        Ok(validate_hedging_payload_bytes(
            kind,
            input,
            label,
            generated_at,
        ))
    })
}

/// Diagnose a Norito-encoded `PdpCommitmentV1` and return outcome JSON.
///
/// Success is diagnostic-only and never authorizes production acceptance.
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
        let input = read_input_bounded(
            &scope,
            bytes_ptr,
            bytes_len,
            "pdp_commitment",
            PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1,
            generated_at,
        )?;
        let label = read_label(&scope, label_ptr, label_len, "commitment.to", generated_at)?;
        Ok(validate_pdp_commitment_bytes(input, label, generated_at))
    })
}

/// Diagnose a Norito-encoded `PdpChallengeV1` and return outcome JSON.
///
/// Success is diagnostic-only and never authorizes production acceptance.
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
        let input = read_input_bounded(
            &scope,
            bytes_ptr,
            bytes_len,
            "pdp_challenge",
            PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1,
            generated_at,
        )?;
        let label = read_label(&scope, label_ptr, label_len, "challenge.to", generated_at)?;
        Ok(validate_pdp_challenge_bytes(input, label, generated_at))
    })
}

/// Diagnose a Norito-encoded `PdpProofV1` and return outcome JSON.
///
/// Success does not evaluate signer admission or the commitment roots.
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
        let input = read_input_bounded(
            &scope,
            bytes_ptr,
            bytes_len,
            "pdp_proof",
            PDP_PROOF_MAX_CANONICAL_BYTES_V1,
            generated_at,
        )?;
        let label = read_label(&scope, label_ptr, label_len, "proof.to", generated_at)?;
        Ok(validate_pdp_proof_bytes(input, label, generated_at))
    })
}

/// Diagnose Norito-encoded `PdpCommitmentV1` and `PdpChallengeV1` bytes.
///
/// Success does not evaluate provider admission or proof witnesses.
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
        let commitment = read_input_bounded(
            &scope,
            commitment_ptr,
            commitment_len,
            "pdp_commitment",
            PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1,
            generated_at,
        )?;
        let challenge = read_input_bounded(
            &scope,
            challenge_ptr,
            challenge_len,
            "pdp_challenge",
            PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1,
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

/// Diagnose Norito-encoded `PdpChallengeV1` and `PdpProofV1` bytes.
///
/// Success does not evaluate provider admission or commitment roots.
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
        let challenge = read_input_bounded(
            &scope,
            challenge_ptr,
            challenge_len,
            "pdp_challenge",
            PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1,
            generated_at,
        )?;
        let proof = read_input_bounded(
            &scope,
            proof_ptr,
            proof_len,
            "pdp_proof",
            PDP_PROOF_MAX_CANONICAL_BYTES_V1,
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
        Ok(validate_pdp_challenge_proof_bytes(
            challenge,
            proof,
            challenge_label,
            proof_label,
            generated_at,
        ))
    })
}

/// Exhaustively diagnose PDP commitment, challenge, proof, and both roots.
///
/// This FFI does not receive governed admission state. Success therefore uses
/// `SFS-PDP-DIAG-000` with `production_acceptance=false` and must never be
/// treated as production proof acceptance.
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
        let commitment = read_input_bounded(
            &scope,
            commitment_ptr,
            commitment_len,
            "pdp_commitment",
            PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1,
            generated_at,
        )?;
        let challenge = read_input_bounded(
            &scope,
            challenge_ptr,
            challenge_len,
            "pdp_challenge",
            PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1,
            generated_at,
        )?;
        let proof = read_input_bounded(
            &scope,
            proof_ptr,
            proof_len,
            "pdp_proof",
            PDP_PROOF_MAX_CANONICAL_BYTES_V1,
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
/// `expected_cid_ptr` and `expected_cid_len` must identify the canonical node
/// CID that belongs to the governance log node.
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
        let expected_cid = read_optional_governance_cid(
            &scope,
            expected_cid_ptr,
            expected_cid_len,
            "expected_node_cid",
            generated_at,
        )?
        .ok_or_else(|| missing_expected_node_cid_error(generated_at))?;
        Ok(validate_governance_log_node_bytes(
            input,
            label,
            Some(expected_cid),
            generated_at,
        ))
    })
}

/// Validate a Norito-encoded `GovernanceDagBlockV1` and return outcome JSON.
///
/// An empty `expected_block_cid` omits the external CID equality check. The
/// validator always recomputes and validates the block's canonical CID.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_governance_dag_block_json(
    bytes_ptr: *const u8,
    bytes_len: usize,
    label_ptr: *const u8,
    label_len: usize,
    expected_block_cid_ptr: *const u8,
    expected_block_cid_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let input = read_input(
            &scope,
            bytes_ptr,
            bytes_len,
            "governance_dag_block",
            generated_at,
        )?;
        let label = read_label(
            &scope,
            label_ptr,
            label_len,
            "governance-dag-block.to",
            generated_at,
        )?;
        let expected_block_cid = read_optional_governance_cid(
            &scope,
            expected_block_cid_ptr,
            expected_block_cid_len,
            "expected_block_cid",
            generated_at,
        )?;
        let aggregate_input_bytes = bytes_len
            .checked_add(label_len)
            .and_then(|total| total.checked_add(expected_block_cid_len))
            .ok_or_else(|| {
                invalid_argument_error(
                    "governance_dag_block",
                    "aggregate payload and label length overflowed",
                    "Pass bounded governance DAG block inputs within the exported aggregate byte limit.",
                    generated_at,
                )
            })?;
        if aggregate_input_bytes > SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES {
            return Err(input_length_error(
                "governance_dag_block",
                aggregate_input_bytes,
                SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES,
                generated_at,
            ));
        }
        Ok(validate_governance_dag_block_bytes(
            input,
            label,
            expected_block_cid,
            generated_at,
        ))
    })
}

/// Validate a signed `GovernanceDagHeadV1` against a bounded block chain.
///
/// `blocks_ptr` references `blocks_len` payload/label descriptors ordered from
/// either the root-history start or the exact checkpoint-window start through
/// the signed head block.
///
/// # Safety
/// Non-null pointers must be valid for their corresponding lengths until the
/// function returns. Descriptor pointers and every nested pointer must remain
/// valid for the duration of the call. The returned buffer must be freed with
/// [`sorafs_reference_free_buffer`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sorafs_reference_validate_governance_dag_head_chain_json(
    head_ptr: *const u8,
    head_len: usize,
    head_label_ptr: *const u8,
    head_label_len: usize,
    blocks_ptr: *const SorafsReferenceFfiInput,
    blocks_len: usize,
    generated_at: u64,
) -> SorafsReferenceFfiBuffer {
    run_ffi(generated_at, || {
        let scope = FfiInputScope;
        let head = read_input(
            &scope,
            head_ptr,
            head_len,
            "governance_dag_head",
            generated_at,
        )?;
        let head_label = read_label(
            &scope,
            head_label_ptr,
            head_label_len,
            "governance-dag-head.to",
            generated_at,
        )?;
        let block_descriptors =
            read_governance_block_descriptors(&scope, blocks_ptr, blocks_len, generated_at)?;

        let mut aggregate_input_bytes = head_len.checked_add(head_label_len).ok_or_else(|| {
            invalid_argument_error(
                "governance_dag_head_chain",
                "aggregate payload and label length overflowed",
                "Pass a bounded governance DAG chain within the exported aggregate byte limit.",
                generated_at,
            )
        })?;
        if aggregate_input_bytes > SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES {
            return Err(input_length_error(
                "governance_dag_head_chain",
                aggregate_input_bytes,
                SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES,
                generated_at,
            ));
        }

        let mut blocks = Vec::with_capacity(block_descriptors.len());
        for (index, block) in block_descriptors.iter().enumerate() {
            aggregate_input_bytes = aggregate_input_bytes
                .checked_add(block.bytes_len)
                .and_then(|total| total.checked_add(block.label_len))
                .ok_or_else(|| {
                    invalid_argument_error(
                        "governance_dag_head_chain",
                        "aggregate payload and label length overflowed",
                        "Pass a bounded governance DAG chain within the exported aggregate byte limit.",
                        generated_at,
                    )
                })?;
            if aggregate_input_bytes > SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES {
                return Err(input_length_error(
                    "governance_dag_head_chain",
                    aggregate_input_bytes,
                    SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES,
                    generated_at,
                ));
            }
            let bytes = read_input(
                &scope,
                block.bytes_ptr,
                block.bytes_len,
                format!("governance_dag_block_{index}"),
                generated_at,
            )?;
            let label = read_label(
                &scope,
                block.label_ptr,
                block.label_len,
                format!("governance-dag-block-{index}.to"),
                generated_at,
            )?;
            blocks.push((bytes, label));
        }

        Ok(validate_governance_dag_head_chain_bytes(
            head,
            head_label,
            &blocks,
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
        let mut aggregate_input_bytes = 0usize;
        for (index, payload) in payload_descriptors.iter().enumerate() {
            let kind = bundle_kind_from_ffi(payload.kind, generated_at)?;
            aggregate_input_bytes = aggregate_input_bytes
                .checked_add(payload.bytes_len)
                .and_then(|total| total.checked_add(payload.label_len))
                .ok_or_else(|| {
                    invalid_argument_error(
                        "bundle_payloads",
                        "aggregate payload and label length overflowed",
                        "Pass a bounded bundle within the exported aggregate byte limit.",
                        generated_at,
                    )
                })?;
            if aggregate_input_bytes > SORAFS_REFERENCE_FFI_MAX_BUNDLE_TOTAL_BYTES {
                return Err(input_length_error(
                    "bundle_payloads_total_bytes",
                    aggregate_input_bytes,
                    SORAFS_REFERENCE_FFI_MAX_BUNDLE_TOTAL_BYTES,
                    generated_at,
                ));
            }
            let bytes = read_input_bounded(
                &scope,
                payload.bytes_ptr,
                payload.bytes_len,
                format!("bundle_payload_{index}"),
                bundle_payload_maximum_bytes(kind),
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

fn bundle_payload_maximum_bytes(kind: FixtureBundlePayloadKindV1) -> usize {
    match kind {
        FixtureBundlePayloadKindV1::PdpCommitment => PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1,
        FixtureBundlePayloadKindV1::PdpChallenge => PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1,
        FixtureBundlePayloadKindV1::PdpProof => PDP_PROOF_MAX_CANONICAL_BYTES_V1,
        _ => SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES,
    }
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

fn read_input(
    scope: &FfiInputScope,
    ptr: *const u8,
    len: usize,
    label: impl Into<String>,
    generated_at: u64,
) -> Result<&[u8], ValidationOutcomeV1> {
    read_input_bounded(
        scope,
        ptr,
        len,
        label,
        SORAFS_REFERENCE_FFI_MAX_INPUT_BYTES,
        generated_at,
    )
}

fn read_input_bounded(
    _scope: &FfiInputScope,
    ptr: *const u8,
    len: usize,
    label: impl Into<String>,
    maximum_bytes: usize,
    generated_at: u64,
) -> Result<&[u8], ValidationOutcomeV1> {
    if len == 0 {
        return Ok(&[]);
    }
    let label = label.into();
    if len > isize::MAX as usize {
        return Err(input_length_error(
            label,
            len,
            isize::MAX as usize,
            generated_at,
        ));
    }
    if len > maximum_bytes {
        return Err(input_length_error(label, len, maximum_bytes, generated_at));
    }
    if ptr.is_null() {
        return Err(null_pointer_error(label, generated_at));
    }
    // SAFETY: FFI callers must provide a pointer valid for `len` bytes.
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

fn read_optional_input(
    scope: &FfiInputScope,
    ptr: *const u8,
    len: usize,
    label: impl Into<String>,
    generated_at: u64,
) -> Result<Option<&[u8]>, ValidationOutcomeV1> {
    if len == 0 {
        return Ok(None);
    }
    read_input(scope, ptr, len, label, generated_at).map(Some)
}

fn read_optional_governance_cid(
    scope: &FfiInputScope,
    ptr: *const u8,
    len: usize,
    label: impl Into<String>,
    generated_at: u64,
) -> Result<Option<&[u8]>, ValidationOutcomeV1> {
    if len == 0 {
        return Ok(None);
    }
    let label = label.into();
    let exact_bytes = SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1 as usize;
    if len != exact_bytes {
        return Err(input_length_error(label, len, exact_bytes, generated_at));
    }
    read_input_bounded(scope, ptr, len, label, exact_bytes, generated_at).map(Some)
}

fn read_label(
    scope: &FfiInputScope,
    ptr: *const u8,
    len: usize,
    default: impl Into<String>,
    generated_at: u64,
) -> Result<String, ValidationOutcomeV1> {
    let default = default.into();
    if len > SORAFS_REFERENCE_FFI_MAX_LABEL_BYTES {
        return Err(input_length_error(
            default,
            len,
            SORAFS_REFERENCE_FFI_MAX_LABEL_BYTES,
            generated_at,
        ));
    }
    let Some(bytes) = read_optional_input(scope, ptr, len, default.clone(), generated_at)? else {
        return Ok(default);
    };
    let label = str::from_utf8(bytes).map_err(|_| {
        invalid_argument_error(
            default.clone(),
            "label is not valid UTF-8",
            "Pass labels as canonical UTF-8 without replacement or control characters.",
            generated_at,
        )
    })?;
    if label.chars().any(char::is_control) {
        return Err(invalid_argument_error(
            default,
            "label contains a Unicode control character",
            "Remove control characters from the label before calling the validator.",
            generated_at,
        ));
    }
    Ok(label.to_owned())
}

fn read_payload_descriptors(
    _scope: &FfiInputScope,
    ptr: *const SorafsReferenceFfiBundlePayload,
    len: usize,
    generated_at: u64,
) -> Result<&[SorafsReferenceFfiBundlePayload], ValidationOutcomeV1> {
    if len == 0 {
        return Ok(&[]);
    }
    if len > SORAFS_REFERENCE_FFI_MAX_BUNDLE_PAYLOADS {
        return Err(input_length_error(
            "bundle_payloads",
            len,
            SORAFS_REFERENCE_FFI_MAX_BUNDLE_PAYLOADS,
            generated_at,
        ));
    }
    let descriptor_bytes = len
        .checked_mul(mem::size_of::<SorafsReferenceFfiBundlePayload>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(|| {
            invalid_argument_error(
                "bundle_payloads",
                "descriptor byte length exceeds the addressable slice range",
                "Pass a bounded descriptor array using the exported bundle payload limit.",
                generated_at,
            )
        })?;
    debug_assert!(descriptor_bytes <= isize::MAX as usize);
    if ptr.is_null() {
        return Err(null_pointer_error("bundle_payloads", generated_at));
    }
    if !(ptr as usize).is_multiple_of(mem::align_of::<SorafsReferenceFfiBundlePayload>()) {
        return Err(invalid_argument_error(
            "bundle_payloads",
            "descriptor pointer is not correctly aligned",
            "Pass a naturally aligned SorafsReferenceFfiBundlePayload array.",
            generated_at,
        ));
    }
    // SAFETY: FFI callers must provide a pointer valid for `len` descriptors.
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

fn read_governance_block_descriptors(
    _scope: &FfiInputScope,
    ptr: *const SorafsReferenceFfiInput,
    len: usize,
    generated_at: u64,
) -> Result<&[SorafsReferenceFfiInput], ValidationOutcomeV1> {
    if len == 0 {
        return Ok(&[]);
    }
    let maximum_blocks = SORAFS_REFERENCE_GOVERNANCE_DAG_MAX_BLOCKS_V1 as usize;
    if len > maximum_blocks {
        return Err(input_length_error(
            "governance_dag_blocks",
            len,
            maximum_blocks,
            generated_at,
        ));
    }
    let descriptor_bytes = len
        .checked_mul(mem::size_of::<SorafsReferenceFfiInput>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(|| {
            invalid_argument_error(
                "governance_dag_blocks",
                "descriptor byte length exceeds the addressable slice range",
                "Pass a bounded descriptor array using the exported governance DAG block limit.",
                generated_at,
            )
        })?;
    debug_assert!(descriptor_bytes <= isize::MAX as usize);
    if ptr.is_null() {
        return Err(null_pointer_error("governance_dag_blocks", generated_at));
    }
    if !(ptr as usize).is_multiple_of(mem::align_of::<SorafsReferenceFfiInput>()) {
        return Err(invalid_argument_error(
            "governance_dag_blocks",
            "descriptor pointer is not correctly aligned",
            "Pass a naturally aligned SorafsReferenceFfiInput array.",
            generated_at,
        ));
    }
    // SAFETY: callers must provide a pointer valid for `len` descriptors and
    // keep all nested inputs alive for the duration of the call.
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
        SORAFS_REFERENCE_ORDERBOOK_KIND_RUNTIME_SNAPSHOT => {
            Ok(OrderbookValidationPayloadKindV1::RuntimeSnapshot)
        }
        other => Err(unsupported_selector_error(
            "orderbook_kind",
            other,
            generated_at,
        )),
    }
}

fn pop_kind_from_ffi(
    kind: u32,
    generated_at: u64,
) -> Result<PopValidationPayloadKindV1, ValidationOutcomeV1> {
    match kind {
        SORAFS_REFERENCE_POP_KIND_CREDENTIAL => Ok(PopValidationPayloadKindV1::Credential),
        SORAFS_REFERENCE_POP_KIND_COMMITMENT_ROOT => Ok(PopValidationPayloadKindV1::CommitmentRoot),
        SORAFS_REFERENCE_POP_KIND_REVOCATION_LIST => Ok(PopValidationPayloadKindV1::RevocationList),
        SORAFS_REFERENCE_POP_KIND_ISSUED_CREDENTIAL_BUNDLE => {
            Ok(PopValidationPayloadKindV1::IssuedCredentialBundle)
        }
        SORAFS_REFERENCE_POP_KIND_ENROLLMENT_REQUEST => {
            Ok(PopValidationPayloadKindV1::EnrollmentRequest)
        }
        SORAFS_REFERENCE_POP_KIND_RENEWAL_REQUEST => Ok(PopValidationPayloadKindV1::RenewalRequest),
        SORAFS_REFERENCE_POP_KIND_MEMBERSHIP_PROOF => {
            Ok(PopValidationPayloadKindV1::MembershipProof)
        }
        other => Err(unsupported_selector_error("pop_kind", other, generated_at)),
    }
}

fn hedging_kind_from_ffi(
    kind: u32,
    generated_at: u64,
) -> Result<HedgingValidationPayloadKindV1, ValidationOutcomeV1> {
    match kind {
        SORAFS_REFERENCE_HEDGING_KIND_PRICE_FEED => Ok(HedgingValidationPayloadKindV1::PriceFeed),
        SORAFS_REFERENCE_HEDGING_KIND_REFERENCE_PRICE_DECISION => {
            Ok(HedgingValidationPayloadKindV1::ReferencePriceDecision)
        }
        SORAFS_REFERENCE_HEDGING_KIND_BILLING_LINE_ITEM => {
            Ok(HedgingValidationPayloadKindV1::BillingLineItem)
        }
        SORAFS_REFERENCE_HEDGING_KIND_BILLING_STATEMENT => {
            Ok(HedgingValidationPayloadKindV1::BillingStatement)
        }
        other => Err(unsupported_selector_error(
            "hedging_kind",
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
        SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_RUNTIME_SNAPSHOT => {
            Ok(FixtureBundlePayloadKindV1::OrderbookRuntimeSnapshot)
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

fn input_length_error(
    label: impl Into<String>,
    actual: usize,
    maximum: usize,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    ffi_error(
        SFS_FFI_ARGUMENT,
        "SoraFS reference FFI input length exceeds its bounded limit",
        "Reject the input before crossing the FFI boundary or split it according to the canonical validator limits.",
        vec![
            ValidationContextFieldV1::new("argument", label.into()),
            ValidationContextFieldV1::new("actual_length", actual.to_string()),
            ValidationContextFieldV1::new("maximum_length", maximum.to_string()),
        ],
        generated_at,
    )
}

fn invalid_argument_error(
    label: impl Into<String>,
    reason: impl Into<String>,
    action: impl Into<String>,
    generated_at: u64,
) -> ValidationOutcomeV1 {
    ffi_error(
        SFS_FFI_ARGUMENT,
        "SoraFS reference FFI received an invalid argument",
        action,
        vec![
            ValidationContextFieldV1::new("argument", label.into()),
            ValidationContextFieldV1::new("reason", reason.into()),
        ],
        generated_at,
    )
}

fn missing_expected_node_cid_error(generated_at: u64) -> ValidationOutcomeV1 {
    ffi_error(
        SFS_FFI_ARGUMENT,
        "SoraFS reference FFI requires expected governance node CID bytes",
        "Pass the canonical node CID bytes for every governance log node validation.",
        vec![ValidationContextFieldV1::new(
            "argument",
            "expected_node_cid",
        )],
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
        BillingLineDirectionV1, BillingLineItemKindV1, BillingStatementV1, ByteRangeV1,
        GovernanceLogNodeV1, HEDGING_PRICE_FEED_VERSION_V1, HedgingFeedStatusV1,
        HedgingPriceFeedV1, ORDERBOOK_ORDER_VERSION_V1, ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1,
        ORDERBOOK_TRADE_EVENT_VERSION_V1, OrderBookEntryV1, OrderRequestV1, OrderSideV1,
        OrderTierV1, OrderbookRuntimeSnapshotV1, OrderbookSignatureV1, POP_CREDENTIAL_VERSION_V1,
        PopCredentialAttributeV1, PopCredentialV1, PopEligibilityClassV1, PopSignatureAlgorithmV1,
        PopSignatureV1, ReplicationOrderSignatureV1, ReplicationOrderV1,
        SETTLEMENT_RECEIPT_VERSION_V1, SIGNED_REPLICATION_ORDER_VERSION_V1, SettlementReceiptV1,
        SignatureAlgorithm, SignedReplicationOrderV1, TradeEventV1, XorQuantity,
        build_billing_line_item_v1, build_billing_statement_v1, derive_reference_price_decision_v1,
        sign_pop_credential_ed25519_v1,
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

    #[test]
    fn ffi_buffer_from_bytes_handles_spare_capacity() {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(b"spare capacity must not affect freeing");
        assert!(bytes.capacity() > bytes.len());

        let buffer = SorafsReferenceFfiBuffer::from_bytes(bytes);

        // SAFETY: the buffer was returned by the FFI buffer constructor under test.
        let returned = unsafe { read_and_free(buffer) };
        assert_eq!(returned, b"spare capacity must not affect freeing");
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
            xor_debited: XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            provider_credit: XorQuantity::try_from_micro(90)
                .expect("legacy micro-XOR value is representable"),
            fee_amount: XorQuantity::try_from_micro(10)
                .expect("legacy micro-XOR value is representable"),
            issued_at_unix: 1_800_000_010,
            settlement_signature: OrderbookSignatureV1 {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![0xD7; PUBLIC_KEY_LENGTH],
                signature: vec![0x57; SIGNATURE_LENGTH],
            },
        }
    }

    fn orderbook_runtime_snapshot() -> OrderbookRuntimeSnapshotV1 {
        let signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: vec![0xD7; PUBLIC_KEY_LENGTH],
            signature: vec![0x57; SIGNATURE_LENGTH],
        };
        let owner_account = b"provider@sora".to_vec();
        let nonce = 8;
        let order = OrderRequestV1 {
            version: ORDERBOOK_ORDER_VERSION_V1,
            order_id: crate::derive_orderbook_order_id_v1(&owner_account, nonce),
            side: OrderSideV1::Ask,
            tier: OrderTierV1::Hot,
            price_per_gib: XorQuantity::try_from_micro(1_250_000)
                .expect("legacy micro-XOR value is representable"),
            quantity_gib: 4,
            remaining_gib: 4,
            owner_account,
            expiry_unix: 1_800_000_500,
            nonce,
            maker_fee_bps: 10,
            taker_fee_bps: 15,
            signature,
        };
        let trade = TradeEventV1 {
            version: ORDERBOOK_TRADE_EVENT_VERSION_V1,
            trade_id: [0x83; 32],
            maker_order_id: [0x71; 32],
            taker_order_id: [0x72; 32],
            tier: OrderTierV1::Hot,
            price_per_gib: XorQuantity::try_from_micro(1_250_000)
                .expect("legacy micro-XOR value is representable"),
            filled_gib: 2,
            maker_fee: XorQuantity::try_from_micro(2_500)
                .expect("legacy micro-XOR value is representable"),
            taker_fee: XorQuantity::try_from_micro(3_750)
                .expect("legacy micro-XOR value is representable"),
            timestamp_unix: 1_800_000_005,
        };
        let channel = crate::open_settlement_channel_for_trade_v1(
            &trade,
            [0x82; 32],
            b"buyer@sora".to_vec(),
            [0x91; 32],
            1_800_000_005,
        )
        .expect("orderbook fixture channel should open");
        let mut receipt = orderbook_settlement_receipt();
        receipt.channel_id = channel.channel_id;
        receipt.trade_id = channel.trade_id;
        let channel = crate::apply_settlement_receipt_v1(&channel, &receipt)
            .expect("orderbook fixture receipt should apply");
        OrderbookRuntimeSnapshotV1 {
            version: ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1,
            next_sequence: 4,
            generated_at_unix: 1_800_000_020,
            owner_nonce_high_waters: vec![crate::OrderbookOwnerNonceHighWaterV1 {
                owner_account: order.owner_account.clone(),
                highest_nonce: order.nonce,
            }],
            open_orders: vec![OrderBookEntryV1 { order, sequence: 3 }],
            trades: vec![trade],
            settlement_channels: vec![channel],
            settlement_receipts: vec![receipt],
            expired_order_ids: vec![[0x74; 32]],
        }
    }

    fn hedging_digest(label: &str) -> [u8; 32] {
        let hash = blake3::hash(label.as_bytes());
        let mut out = [0_u8; 32];
        out.copy_from_slice(hash.as_bytes());
        out
    }

    fn hedging_feed(feed_id: &str, price: &str, observed_at_unix: u64) -> HedgingPriceFeedV1 {
        HedgingPriceFeedV1 {
            version: HEDGING_PRICE_FEED_VERSION_V1,
            feed_id: feed_id.to_owned(),
            source: format!("{feed_id}-source"),
            observed_at_unix,
            xor_usd_price: price.parse().expect("canonical USD/XOR price"),
            weight_bps: 5_000,
            evidence_digest: hedging_digest(feed_id),
            status: HedgingFeedStatusV1::Ok,
        }
    }

    fn billing_statement() -> BillingStatementV1 {
        let reference_price = derive_reference_price_decision_v1(
            1_800,
            vec![
                hedging_feed("primary", "2", 1_790),
                hedging_feed("secondary", "2", 1_785),
            ],
            120,
            500,
        )
        .expect("reference decision");
        let storage = build_billing_line_item_v1(
            BillingLineItemKindV1::Storage,
            BillingLineDirectionV1::Debit,
            "deal-storage",
            "10".parse::<XorQuantity>().expect("canonical XOR amount"),
            &reference_price.xor_usd_price,
            86_400,
            Some("weekly storage".to_owned()),
        )
        .expect("storage line");
        let credit = build_billing_line_item_v1(
            BillingLineItemKindV1::IncentiveCredit,
            BillingLineDirectionV1::Credit,
            "provider-credit",
            "1".parse::<XorQuantity>().expect("canonical XOR amount"),
            &reference_price.xor_usd_price,
            1,
            None,
        )
        .expect("credit line");
        build_billing_statement_v1(
            b"buyer-account".to_vec(),
            1_000,
            1_800,
            2_000,
            reference_price,
            vec![storage, credit],
            None,
        )
        .expect("billing statement")
    }

    fn pop_digest(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn pop_scalar(value: u64) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&value.to_le_bytes());
        bytes
    }

    fn pop_nonce(value: u128) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[..16].copy_from_slice(&value.to_le_bytes());
        bytes
    }

    fn unsigned_pop_credential(signing_key: &SigningKey) -> PopCredentialV1 {
        PopCredentialV1 {
            version: POP_CREDENTIAL_VERSION_V1,
            credential_id: pop_scalar(0x11),
            holder_commitment: pop_scalar(0x12),
            eligibility_class: PopEligibilityClassV1::General,
            attributes: vec![PopCredentialAttributeV1 {
                key: "residency".to_owned(),
                value_commitment: pop_digest(0x13),
            }],
            issuer_id: "issuer.sorafs".to_owned(),
            issued_at_epoch: 100,
            expires_at_epoch: 1_000,
            renewal_at_epoch: 800,
            revocation_nonce: pop_nonce(0x14),
            commitment_root: pop_scalar(0x15),
            commitment_tree_version: 7,
            revocation_list_version: 3,
            issuer_signature: PopSignatureV1 {
                algorithm: PopSignatureAlgorithmV1::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: vec![0; SIGNATURE_LENGTH],
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
    fn ffi_governance_validator_accepts_matching_expected_cid() {
        let bytes = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/governance/node_v1.to",
        ))
        .expect("read governance fixture");
        let node: GovernanceLogNodeV1 =
            norito::decode_from_bytes(&bytes).expect("decode governance fixture");
        let label = b"governance.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_governance_json(
                bytes.as_ptr(),
                bytes.len(),
                label.as_ptr(),
                label.len(),
                node.node_cid.as_ptr(),
                node.node_cid.len(),
                124,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Ok"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-OK-000")
        );
    }

    #[test]
    fn ffi_governance_validator_rejects_missing_expected_cid() {
        let bytes = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/governance/node_v1.to",
        ))
        .expect("read governance fixture");
        let label = b"governance.to";

        // SAFETY: non-null pointers reference live test vectors; the null CID pointer has
        // zero length and must be rejected before any read.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_governance_json(
                bytes.as_ptr(),
                bytes.len(),
                label.as_ptr(),
                label.len(),
                std::ptr::null(),
                0,
                125,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some(SFS_FFI_ARGUMENT)
        );
        assert!(
            outcome
                .get("message")
                .and_then(Value::as_str)
                .is_some_and(|message| message.contains("requires expected governance node CID")),
            "{outcome:?}"
        );
    }

    #[test]
    fn ffi_governance_validators_reject_noncanonical_expected_cid_lengths_before_read() {
        let bytes = [0xA5];

        // SAFETY: both calls reject the noncanonical CID length before reading
        // the deliberately null expected-CID pointer.
        let node_outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_governance_json(
                bytes.as_ptr(),
                bytes.len(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1 as usize + 1,
                125,
            )
        });
        assert_eq!(
            node_outcome.get("code").and_then(Value::as_str),
            Some(SFS_FFI_ARGUMENT)
        );

        // SAFETY: same as above for the optional block CID.
        let block_outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_governance_dag_block_json(
                bytes.as_ptr(),
                bytes.len(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1 as usize - 1,
                126,
            )
        });
        assert_eq!(
            block_outcome.get("code").and_then(Value::as_str),
            Some(SFS_FFI_ARGUMENT)
        );
    }

    #[test]
    fn ffi_governance_dag_block_validator_returns_json_outcome() {
        let bytes = [0xA5];
        let label = b"governance-block.to";

        // SAFETY: the pointers reference live test arrays for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_governance_dag_block_json(
                bytes.as_ptr(),
                bytes.len(),
                label.as_ptr(),
                label.len(),
                std::ptr::null(),
                0,
                126,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-NORITO-001")
        );
        assert_eq!(
            outcome.get("generated_at").and_then(Value::as_u64),
            Some(126)
        );
        let inputs = outcome
            .get("inputs")
            .and_then(Value::as_array)
            .expect("outcome inputs");
        assert_eq!(
            inputs[0].get("path").and_then(Value::as_str),
            Some("governance-block.to")
        );
    }

    #[test]
    fn ffi_governance_dag_head_chain_preserves_ordered_block_labels() {
        let head = [0xA5];
        let head_label = b"governance-head.to";
        let block = [0x5A];
        let block_label = b"governance-block-0.to";
        let blocks = [SorafsReferenceFfiInput {
            bytes_ptr: block.as_ptr(),
            bytes_len: block.len(),
            label_ptr: block_label.as_ptr(),
            label_len: block_label.len(),
        }];

        // SAFETY: the descriptor and nested pointers reference live test arrays for
        // the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_governance_dag_head_chain_json(
                head.as_ptr(),
                head.len(),
                head_label.as_ptr(),
                head_label.len(),
                blocks.as_ptr(),
                blocks.len(),
                127,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-NORITO-001")
        );
        let inputs = outcome
            .get("inputs")
            .and_then(Value::as_array)
            .expect("outcome inputs");
        assert_eq!(inputs.len(), 2);
        assert_eq!(
            inputs[0].get("path").and_then(Value::as_str),
            Some("governance-head.to")
        );
        assert_eq!(
            inputs[1].get("path").and_then(Value::as_str),
            Some("governance-block-0.to")
        );
    }

    #[test]
    fn ffi_governance_dag_head_chain_rejects_excess_block_descriptors() {
        let head = [0xA5];

        // SAFETY: the oversized descriptor count is rejected before the null
        // descriptor pointer is read.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_governance_dag_head_chain_json(
                head.as_ptr(),
                head.len(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                SORAFS_REFERENCE_GOVERNANCE_DAG_MAX_BLOCKS_V1 as usize + 1,
                128,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some(SFS_FFI_ARGUMENT)
        );
    }

    #[test]
    fn ffi_governance_dag_head_chain_rejects_misaligned_descriptors() {
        let head = [0xA5];
        let descriptor_storage = [std::mem::MaybeUninit::<SorafsReferenceFfiInput>::uninit(); 2];
        // SAFETY: adding one stays within the backing descriptor storage. The
        // validator must reject the deliberately misaligned pointer before read.
        let misaligned = unsafe {
            descriptor_storage
                .as_ptr()
                .cast::<u8>()
                .add(1)
                .cast::<SorafsReferenceFfiInput>()
        };

        // SAFETY: the deliberately misaligned pointer is rejected before access.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_governance_dag_head_chain_json(
                head.as_ptr(),
                head.len(),
                std::ptr::null(),
                0,
                misaligned,
                1,
                129,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some(SFS_FFI_ARGUMENT)
        );
        assert!(
            outcome
                .get("context")
                .and_then(Value::as_array)
                .is_some_and(|fields| fields.iter().any(|field| {
                    field.get("key").and_then(Value::as_str) == Some("reason")
                        && field.get("value").and_then(Value::as_str)
                            == Some("descriptor pointer is not correctly aligned")
                }))
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
    fn ffi_orderbook_validator_accepts_runtime_snapshot_selector() {
        let snapshot = orderbook_runtime_snapshot();
        let bytes = norito::to_bytes(&snapshot).expect("encode runtime snapshot");
        let label = b"orderbook-runtime-snapshot.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_orderbook_json(
                SORAFS_REFERENCE_ORDERBOOK_KIND_RUNTIME_SNAPSHOT,
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
    fn ffi_pop_validator_returns_json_outcome() {
        let signing_key = SigningKey::from_bytes(&[0x55; 32]);
        let credential =
            sign_pop_credential_ed25519_v1(unsigned_pop_credential(&signing_key), &signing_key)
                .expect("sign PoP credential");
        let bytes = norito::to_bytes(&credential).expect("encode PoP credential");
        let label = b"pop-credential.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_pop_json(
                SORAFS_REFERENCE_POP_KIND_CREDENTIAL,
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
    fn ffi_hedging_validator_returns_json_outcome() {
        let statement = billing_statement();
        let bytes = norito::to_bytes(&statement).expect("encode billing statement");
        let label = b"billing-statement.to";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_hedging_json(
                SORAFS_REFERENCE_HEDGING_KIND_BILLING_STATEMENT,
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
    fn ffi_rejects_unknown_hedging_kind() {
        let bytes = b"not norito";

        // SAFETY: the pointers reference live test vectors for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_hedging_json(
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
            Some("SFS-PDP-DIAG-000")
        );
        assert!(
            outcome
                .get("context")
                .and_then(Value::as_array)
                .is_some_and(|fields| fields.iter().any(|field| {
                    field.get("key").and_then(Value::as_str) == Some("production_acceptance")
                        && field.get("value").and_then(Value::as_str) == Some("false")
                })),
            "{outcome:?}"
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
            Some("SFS-PDP-DIAG-000")
        );
        assert!(
            outcome
                .get("context")
                .and_then(Value::as_array)
                .is_some_and(|fields| fields.iter().any(|field| {
                    field.get("key").and_then(Value::as_str) == Some("production_acceptance")
                        && field.get("value").and_then(Value::as_str) == Some("false")
                })),
            "{outcome:?}"
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
        let snapshot = fs::read(workspace_fixture(
            "fixtures/sorafs_manifest/orderbook/runtime_snapshot_v1.to",
        ))
        .expect("read orderbook runtime snapshot fixture");
        let order_label = b"order.to";
        let proof_label = b"proof.to";
        let receipt_label = b"orderbook/settlement_receipt_v1.to";
        let snapshot_label = b"orderbook/runtime_snapshot_v1.to";
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
            SorafsReferenceFfiBundlePayload {
                kind: SORAFS_REFERENCE_BUNDLE_KIND_ORDERBOOK_RUNTIME_SNAPSHOT,
                bytes_ptr: snapshot.as_ptr(),
                bytes_len: snapshot.len(),
                label_ptr: snapshot_label.as_ptr(),
                label_len: snapshot_label.len(),
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
        let inputs = outcome
            .get("inputs")
            .and_then(Value::as_array)
            .expect("bundle outcome should include inputs");
        assert!(
            inputs.iter().any(|input| {
                input.get("kind").and_then(Value::as_str) == Some("orderbook_runtime_snapshot")
                    && input
                        .get("path")
                        .and_then(Value::as_str)
                        .is_some_and(|path| path.ends_with("orderbook/runtime_snapshot_v1.to"))
            }),
            "{outcome:?}"
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

    #[test]
    fn ffi_rejects_unknown_pop_kind() {
        let bytes = b"not norito";

        // SAFETY: the pointer references live test bytes for the duration of the call.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_pop_json(
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
    fn ffi_rejects_oversized_inputs_before_pointer_access() {
        // SAFETY: the oversized length is rejected before the null pointer can be accessed.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_pdp_proof_json(
                std::ptr::null(),
                PDP_PROOF_MAX_CANONICAL_BYTES_V1 + 1,
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
        assert!(
            outcome
                .get("context")
                .and_then(Value::as_array)
                .is_some_and(|fields| fields.iter().any(|field| {
                    field.get("key").and_then(Value::as_str) == Some("maximum_length")
                        && field
                            .get("value")
                            .and_then(Value::as_str)
                            .and_then(|value| value.parse::<usize>().ok())
                            == Some(PDP_PROOF_MAX_CANONICAL_BYTES_V1)
                }))
        );
    }

    #[test]
    fn ffi_rejects_non_utf8_and_control_character_labels() {
        let bytes = b"not norito";
        let labels: [&[u8]; 2] = [&[0xFF], b"forged\nlabel.to"];

        for label in labels {
            // SAFETY: both pointers reference live test bytes for the duration of the call.
            let outcome = outcome_from_buffer(unsafe {
                sorafs_reference_validate_replication_order_json(
                    bytes.as_ptr(),
                    bytes.len(),
                    label.as_ptr(),
                    label.len(),
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

    #[test]
    fn ffi_rejects_oversized_labels_before_pointer_access() {
        let bytes = b"not norito";
        // SAFETY: the label length is rejected before the intentionally null pointer is accessed.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_replication_order_json(
                bytes.as_ptr(),
                bytes.len(),
                std::ptr::null(),
                SORAFS_REFERENCE_FFI_MAX_LABEL_BYTES + 1,
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
    fn ffi_rejects_oversized_bundle_descriptor_count_before_pointer_access() {
        // SAFETY: the descriptor count is rejected before the null pointer can be accessed.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_bundle_json(
                std::ptr::null(),
                SORAFS_REFERENCE_FFI_MAX_BUNDLE_PAYLOADS + 1,
                123,
                456,
            )
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-FFI-001")
        );
    }

    #[test]
    fn ffi_rejects_bundle_aggregate_length_before_payload_pointer_access() {
        let descriptor = SorafsReferenceFfiBundlePayload {
            kind: SORAFS_REFERENCE_BUNDLE_KIND_REPLICATION_ORDER,
            bytes_ptr: std::ptr::null(),
            bytes_len: SORAFS_REFERENCE_FFI_MAX_BUNDLE_TOTAL_BYTES + 1,
            label_ptr: std::ptr::null(),
            label_len: 0,
        };

        // SAFETY: the live descriptor is valid and its aggregate length is rejected before its
        // intentionally null payload pointer can be accessed.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_bundle_json(&descriptor, 1, 123, 456)
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-FFI-001")
        );
    }

    #[test]
    fn ffi_rejects_misaligned_bundle_descriptors_before_access() {
        let storage = vec![0u8; mem::size_of::<SorafsReferenceFfiBundlePayload>() + 1];
        // SAFETY: adding one remains within the live byte allocation. The validator must reject
        // the resulting descriptor pointer before attempting a typed read.
        let misaligned =
            unsafe { storage.as_ptr().add(1) }.cast::<SorafsReferenceFfiBundlePayload>();

        // SAFETY: this adversarial call is specifically checking the pre-access alignment guard.
        let outcome = outcome_from_buffer(unsafe {
            sorafs_reference_validate_bundle_json(misaligned, 1, 123, 456)
        });

        assert_eq!(outcome.get("status").and_then(Value::as_str), Some("Error"));
        assert_eq!(
            outcome.get("code").and_then(Value::as_str),
            Some("SFS-FFI-001")
        );
    }
}
