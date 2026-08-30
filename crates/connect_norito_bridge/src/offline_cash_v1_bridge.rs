//! Authenticated Offline Cash V1 artifact and opaque verification-session registries.
//!
//! This module is deliberately separate from the older Kagemusha V4 artifact
//! registry. Offline Cash V1 has its own threshold-authenticated 34-role
//! inventory and must never infer release authority from the eight-file V4
//! online-proof package.

use std::{
    collections::HashMap,
    fs::File,
    io::{Seek as _, SeekFrom, Write as _},
    slice,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use iroha_core::zk::offline_cash_v1::{
    OfflineCashAuthenticatedArtifactFileSetV1, OfflineCashVerifierV1, VerifiedOfflineCashCreditV1,
};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    offline::{
        OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1, OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1,
        OFFLINE_CASH_PAYMENT_MAX_BYTES_V1, OFFLINE_CASH_RELEASE_ATTESTATION_MAX_BYTES_V1,
        OFFLINE_CASH_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
        OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1, OfflineCashAcknowledgementV1,
        OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1,
        OfflineCashInternalValidationReceiptV1, OfflineCashPaymentRequestV1, OfflineCashPaymentV1,
        OfflineCashReleaseAttestationV1, OfflineCashReleaseAuthorityPolicyV1,
        OfflineCashReleaseManifestV1,
    },
};
use libc::{c_int, c_uchar, c_ulong};
use sha2::{Digest as _, Sha256};

use super::{
    BridgeError, BridgeResult, bridge_result_to_code, clear_bridge_output,
    publish_offline_cash_output_v1, run_kagemusha_recursive_spend_worker_v4,
};

const ARTIFACT_COUNT: usize = OfflineCashArtifactRoleV1::ALL.len();
const ARTIFACT_HANDLE_NAMESPACE: u64 = 0x4f31_0000_0000_0000;
const VERIFICATION_SESSION_HANDLE_NAMESPACE: u64 = 0x4f32_0000_0000_0000;
const HANDLE_COUNTER_MASK: u64 = 0x0000_ffff_ffff_ffff;
const MAX_ARTIFACT_CHUNK_BYTES: usize = 1024 * 1024;
const MAX_ARTIFACT_INGESTS: usize = ARTIFACT_COUNT * 2;
const VALIDATION_RECEIPT_MAX_BYTES: usize = 1024 * 1024;
const MAX_VERIFICATION_SESSIONS: usize = 1024;
const NETWORK_ID_LITERAL_BYTES: usize = 64;
const ASSET_DEFINITION_ID_LITERAL_MAX_BYTES: usize = 64;

static ARTIFACT_HANDLES: AtomicU64 = AtomicU64::new(1);
static VERIFICATION_SESSION_HANDLES: AtomicU64 = AtomicU64::new(1);
static ARTIFACTS: OnceLock<Mutex<HashMap<u64, Arc<Mutex<ArtifactIngest>>>>> = OnceLock::new();
static INSTALLED: OnceLock<Mutex<Option<Arc<InstalledRelease>>>> = OnceLock::new();
static VERIFICATION_SESSIONS: OnceLock<Mutex<HashMap<u64, Arc<Mutex<VerificationSession>>>>> =
    OnceLock::new();

struct ArtifactIngest {
    manifest: OfflineCashReleaseManifestV1,
    binding: OfflineCashArtifactBindingV1,
    file: File,
    hasher: Sha256,
    written: u64,
    ready: bool,
    failed: bool,
}

struct InstalledRelease {
    verifier: OfflineCashVerifierV1,
}

/// Process-local verifier receipt retention only.
///
/// This value does not own secure-device state, durable wallet state, a
/// balance, or payment-publication authority.
struct VerificationSession {
    installed: Arc<InstalledRelease>,
    request: OfflineCashPaymentRequestV1,
    payment: Option<OfflineCashPaymentV1>,
    receipt: Option<VerifiedOfflineCashCreditV1>,
    acknowledgement: Option<OfflineCashAcknowledgementV1>,
}

fn artifacts() -> &'static Mutex<HashMap<u64, Arc<Mutex<ArtifactIngest>>>> {
    ARTIFACTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn installed() -> &'static Mutex<Option<Arc<InstalledRelease>>> {
    INSTALLED.get_or_init(|| Mutex::new(None))
}

fn verification_sessions() -> &'static Mutex<HashMap<u64, Arc<Mutex<VerificationSession>>>> {
    VERIFICATION_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn artifact_error<T>() -> BridgeResult<T> {
    Err(BridgeError::OfflineCashArtifact)
}

fn verification_session_error<T>() -> BridgeResult<T> {
    Err(BridgeError::OfflineCashSession)
}

fn allocate_handle(
    counter: &AtomicU64,
    namespace: u64,
    occupied: impl Fn(u64) -> bool,
    exhausted: BridgeError,
) -> BridgeResult<u64> {
    for _ in 0..64 {
        let value = counter.fetch_add(1, Ordering::Relaxed) & HANDLE_COUNTER_MASK;
        if value == 0 {
            continue;
        }
        let candidate = namespace | value;
        if !occupied(candidate) {
            return Ok(candidate);
        }
    }
    Err(exhausted)
}

const fn is_handle(handle: u64, namespace: u64) -> bool {
    handle & !HANDLE_COUNTER_MASK == namespace && handle & HANDLE_COUNTER_MASK != 0
}

unsafe fn read_bounded_with_error(
    pointer: *const c_uchar,
    length: c_ulong,
    maximum: usize,
    invalid: BridgeError,
) -> BridgeResult<Vec<u8>> {
    if pointer.is_null() || length == 0 {
        return Err(BridgeError::NullPtr);
    }
    let length = usize::try_from(length).map_err(|_| invalid)?;
    if length > maximum {
        return Err(invalid);
    }
    let bytes = unsafe { slice::from_raw_parts(pointer, length) };
    Ok(bytes.to_vec())
}

unsafe fn read_bounded(
    pointer: *const c_uchar,
    length: c_ulong,
    maximum: usize,
) -> BridgeResult<Vec<u8>> {
    unsafe { read_bounded_with_error(pointer, length, maximum, BridgeError::OfflineCashArtifact) }
}

unsafe fn read_verification_session_bounded(
    pointer: *const c_uchar,
    length: c_ulong,
    maximum: usize,
) -> BridgeResult<Vec<u8>> {
    unsafe { read_bounded_with_error(pointer, length, maximum, BridgeError::OfflineCashSession) }
}

unsafe fn read_digest(pointer: *const c_uchar, length: c_ulong) -> BridgeResult<[u8; 32]> {
    let bytes = unsafe { read_bounded(pointer, length, 32) }?;
    let digest: [u8; 32] = bytes
        .try_into()
        .map_err(|_| BridgeError::OfflineCashArtifact)?;
    if digest == [0; 32] {
        return artifact_error();
    }
    Ok(digest)
}

pub(super) unsafe fn read_verification_session_digest(
    pointer: *const c_uchar,
    length: c_ulong,
) -> BridgeResult<[u8; 32]> {
    let bytes = unsafe { read_verification_session_bounded(pointer, length, 32) }?;
    let digest: [u8; 32] = bytes
        .try_into()
        .map_err(|_| BridgeError::OfflineCashSession)?;
    if digest == [0; 32] {
        return verification_session_error();
    }
    Ok(digest)
}

fn decode_manifest(bytes: &[u8]) -> BridgeResult<OfflineCashReleaseManifestV1> {
    OfflineCashReleaseManifestV1::decode_canonical_exact(bytes)
        .map_err(|_| BridgeError::OfflineCashArtifact)
}

fn decode_validation_receipt(bytes: &[u8]) -> BridgeResult<OfflineCashInternalValidationReceiptV1> {
    if bytes.is_empty() || bytes.len() > VALIDATION_RECEIPT_MAX_BYTES {
        return artifact_error();
    }
    let receipt: OfflineCashInternalValidationReceiptV1 =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
    receipt
        .validate()
        .map_err(|_| BridgeError::OfflineCashArtifact)?;
    let canonical =
        norito::encode_canonical(&receipt).map_err(|_| BridgeError::OfflineCashArtifact)?;
    if canonical.as_slice() != bytes {
        return artifact_error();
    }
    Ok(receipt)
}

fn current_installed() -> BridgeResult<Arc<InstalledRelease>> {
    installed()
        .lock()
        .map_err(|_| BridgeError::OfflineCashSession)?
        .clone()
        .ok_or(BridgeError::OfflineCashSession)
}

fn is_current(candidate: &Arc<InstalledRelease>) -> BridgeResult<bool> {
    Ok(installed()
        .lock()
        .map_err(|_| BridgeError::OfflineCashSession)?
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, candidate)))
}

pub(crate) fn release_probe() -> BridgeResult<Option<([u8; 32], [u8; 32])>> {
    let current = installed()
        .lock()
        .map_err(|_| BridgeError::OfflineCashArtifact)?
        .clone();
    Ok(current.map(|release| {
        (
            release.verifier.release_id(),
            release.verifier.manifest_digest(),
        )
    }))
}

pub(super) fn verify_payment_once(
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    expected_manifest_digest: [u8; 32],
    now_ms: u64,
) -> BridgeResult<Vec<u8>> {
    let current = current_installed()?;
    if current.verifier.manifest_digest() != expected_manifest_digest
        || current.verifier.release_id() != request.release_id
    {
        return verification_session_error();
    }
    let request = request.clone();
    let payment = payment.clone();
    run_kagemusha_recursive_spend_worker_v4(
        "offline-cash-v1-verify",
        BridgeError::OfflineCashSession,
        move || {
            let _verified_credit = current
                .verifier
                .verify_payment(&request, &payment, now_ms)
                .map_err(|_| BridgeError::OfflineCashSession)?;
            norito::encode_canonical(&payment).map_err(|_| BridgeError::OfflineCashSession)
        },
    )
}

fn verification_session_arc(handle: u64) -> BridgeResult<Arc<Mutex<VerificationSession>>> {
    if !is_handle(handle, VERIFICATION_SESSION_HANDLE_NAMESPACE) {
        return verification_session_error();
    }
    verification_sessions()
        .lock()
        .map_err(|_| BridgeError::OfflineCashSession)?
        .get(&handle)
        .cloned()
        .ok_or(BridgeError::OfflineCashSession)
}

fn open_verification_session(
    request: OfflineCashPaymentRequestV1,
    expected_release_id: [u8; 32],
    expected_manifest_digest: [u8; 32],
) -> BridgeResult<u64> {
    request
        .validate()
        .map_err(|_| BridgeError::OfflineCashSession)?;
    let current = current_installed()?;
    if current.verifier.release_id() != expected_release_id
        || current.verifier.manifest_digest() != expected_manifest_digest
        || request.release_id != expected_release_id
    {
        return verification_session_error();
    }
    let mut registry = verification_sessions()
        .lock()
        .map_err(|_| BridgeError::OfflineCashSession)?;
    if !is_current(&current)? {
        return verification_session_error();
    }
    if registry.len() >= MAX_VERIFICATION_SESSIONS {
        return verification_session_error();
    }
    let handle = allocate_handle(
        &VERIFICATION_SESSION_HANDLES,
        VERIFICATION_SESSION_HANDLE_NAMESPACE,
        |candidate| registry.contains_key(&candidate),
        BridgeError::OfflineCashSession,
    )?;
    registry.insert(
        handle,
        Arc::new(Mutex::new(VerificationSession {
            installed: current,
            request,
            payment: None,
            receipt: None,
            acknowledgement: None,
        })),
    );
    Ok(handle)
}

fn verify_verification_session_payment(
    handle: u64,
    payment: OfflineCashPaymentV1,
    now_ms: u64,
) -> BridgeResult<Vec<u8>> {
    let verification_session = verification_session_arc(handle)?;
    let (current, request) = {
        let state = verification_session
            .lock()
            .map_err(|_| BridgeError::OfflineCashSession)?;
        if !is_current(&state.installed)? || state.acknowledgement.is_some() {
            return verification_session_error();
        }
        if let Some(existing) = state.payment.as_ref() {
            if existing != &payment || state.receipt.is_none() {
                return verification_session_error();
            }
            return norito::encode_canonical(existing).map_err(|_| BridgeError::OfflineCashSession);
        }
        (Arc::clone(&state.installed), state.request.clone())
    };
    let payment_for_verification = payment.clone();
    let receipt = run_kagemusha_recursive_spend_worker_v4(
        "offline-cash-v1-session-verify",
        BridgeError::OfflineCashSession,
        move || {
            current
                .verifier
                .verify_payment(&request, &payment_for_verification, now_ms)
                .map_err(|_| BridgeError::OfflineCashSession)
        },
    )?;
    let canonical =
        norito::encode_canonical(&payment).map_err(|_| BridgeError::OfflineCashSession)?;
    let mut state = verification_session
        .lock()
        .map_err(|_| BridgeError::OfflineCashSession)?;
    if !is_current(&state.installed)? || state.acknowledgement.is_some() {
        return verification_session_error();
    }
    if let Some(existing) = state.payment.as_ref() {
        if existing == &payment && state.receipt.is_some() {
            return Ok(canonical);
        }
        return verification_session_error();
    }
    state.payment = Some(payment);
    state.receipt = Some(receipt);
    Ok(canonical)
}

fn verify_verification_session_acknowledgement(
    handle: u64,
    acknowledgement: OfflineCashAcknowledgementV1,
) -> BridgeResult<Vec<u8>> {
    let verification_session = verification_session_arc(handle)?;
    let mut state = verification_session
        .lock()
        .map_err(|_| BridgeError::OfflineCashSession)?;
    if !is_current(&state.installed)? {
        return verification_session_error();
    }
    if let Some(existing) = state.acknowledgement.as_ref() {
        if existing != &acknowledgement {
            return verification_session_error();
        }
        return norito::encode_canonical(existing).map_err(|_| BridgeError::OfflineCashSession);
    }
    let payment = state
        .payment
        .as_ref()
        .ok_or(BridgeError::OfflineCashSession)?;
    let receipt = state
        .receipt
        .as_ref()
        .ok_or(BridgeError::OfflineCashSession)?;
    state
        .installed
        .verifier
        .verify_acknowledgement(&state.request, payment, &acknowledgement, receipt)
        .map_err(|_| BridgeError::OfflineCashSession)?;
    if !is_current(&state.installed)? {
        return verification_session_error();
    }
    let canonical =
        norito::encode_canonical(&acknowledgement).map_err(|_| BridgeError::OfflineCashSession)?;
    state.acknowledgement = Some(acknowledgement);
    Ok(canonical)
}

fn decode_exact_verification_session_context(
    expected_network_id: &[u8],
    expected_asset_definition_id: &[u8],
) -> BridgeResult<(NetworkId, AssetDefinitionId)> {
    let network_literal =
        core::str::from_utf8(expected_network_id).map_err(|_| BridgeError::OfflineCashSession)?;
    let network_id = network_literal
        .parse::<NetworkId>()
        .map_err(|_| BridgeError::OfflineCashSession)?;
    if network_id.to_string() != network_literal {
        return verification_session_error();
    }
    let asset_literal = core::str::from_utf8(expected_asset_definition_id)
        .map_err(|_| BridgeError::OfflineCashSession)?;
    let asset_definition_id = asset_literal
        .parse::<AssetDefinitionId>()
        .map_err(|_| BridgeError::OfflineCashSession)?;
    if asset_definition_id.to_string() != asset_literal {
        return verification_session_error();
    }
    Ok((network_id, asset_definition_id))
}

pub(crate) fn open_verification_session_canonical_bound(
    request: &[u8],
    expected_release_id: [u8; 32],
    expected_manifest_digest: [u8; 32],
    expected_network_id: &[u8],
    expected_asset_definition_id: &[u8],
) -> BridgeResult<u64> {
    let request = OfflineCashPaymentRequestV1::decode_canonical_exact(request)
        .map_err(|_| BridgeError::OfflineCashSession)?;
    let (expected_network_id, expected_asset_definition_id) =
        decode_exact_verification_session_context(
            expected_network_id,
            expected_asset_definition_id,
        )?;
    if request.network_id != expected_network_id || request.asset != expected_asset_definition_id {
        return verification_session_error();
    }
    open_verification_session(request, expected_release_id, expected_manifest_digest)
}

pub(crate) fn verify_verification_session_payment_canonical(
    handle: u64,
    payment: &[u8],
    observed_now_ms: u64,
) -> BridgeResult<Vec<u8>> {
    if observed_now_ms == 0
        || payment.is_empty()
        || payment.len() > OFFLINE_CASH_PAYMENT_MAX_BYTES_V1
    {
        return verification_session_error();
    }
    let verification_session = verification_session_arc(handle)?;
    let request = verification_session
        .lock()
        .map_err(|_| BridgeError::OfflineCashSession)?
        .request
        .clone();
    let payment = OfflineCashPaymentV1::decode_canonical_exact_against(payment, &request)
        .map_err(|_| BridgeError::OfflineCashSession)?;
    verify_verification_session_payment(handle, payment, observed_now_ms)
}

pub(crate) fn verify_verification_session_acknowledgement_canonical(
    handle: u64,
    acknowledgement: &[u8],
) -> BridgeResult<Vec<u8>> {
    if acknowledgement.is_empty()
        || acknowledgement.len() > OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1
    {
        return verification_session_error();
    }
    let verification_session = verification_session_arc(handle)?;
    let (request, payment) = {
        let state = verification_session
            .lock()
            .map_err(|_| BridgeError::OfflineCashSession)?;
        (
            state.request.clone(),
            state
                .payment
                .clone()
                .ok_or(BridgeError::OfflineCashSession)?,
        )
    };
    let acknowledgement = OfflineCashAcknowledgementV1::decode_canonical_exact_against(
        acknowledgement,
        &request,
        &payment,
    )
    .map_err(|_| BridgeError::OfflineCashSession)?;
    verify_verification_session_acknowledgement(handle, acknowledgement)
}

pub(crate) fn verification_session_state_code(handle: u64) -> BridgeResult<u8> {
    let verification_session = verification_session_arc(handle)?;
    let state = verification_session
        .lock()
        .map_err(|_| BridgeError::OfflineCashSession)?;
    if !is_current(&state.installed)? {
        return verification_session_error();
    }
    Ok(if state.acknowledgement.is_some() {
        3
    } else if state.payment.is_some() {
        2
    } else {
        1
    })
}

pub(crate) fn close_verification_session(handle: u64) -> BridgeResult<()> {
    if !is_handle(handle, VERIFICATION_SESSION_HANDLE_NAMESPACE) {
        return verification_session_error();
    }
    verification_sessions()
        .lock()
        .map_err(|_| BridgeError::OfflineCashSession)?
        .remove(&handle)
        .ok_or(BridgeError::OfflineCashSession)?;
    Ok(())
}

/// Start one bounded artifact spool for an exact canonical manifest role.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_artifact_begin_v1(
    manifest_ptr: *const c_uchar,
    manifest_len: c_ulong,
    role: u8,
    out_handle: *mut u64,
) -> c_int {
    if !out_handle.is_null() {
        unsafe { *out_handle = 0 };
    }
    let result = (|| {
        if out_handle.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let bytes = unsafe {
            read_bounded(
                manifest_ptr,
                manifest_len,
                OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1,
            )
        }?;
        let manifest = decode_manifest(&bytes)?;
        let role = OfflineCashArtifactRoleV1::ALL
            .get(usize::from(role))
            .copied()
            .ok_or(BridgeError::OfflineCashArtifact)?;
        let binding = manifest
            .artifacts
            .iter()
            .copied()
            .find(|candidate| candidate.role == role)
            .ok_or(BridgeError::OfflineCashArtifact)?;
        let mut registry = artifacts()
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        if registry.len() >= MAX_ARTIFACT_INGESTS {
            return artifact_error();
        }
        let mut declared = binding.byte_len;
        for active in registry.values() {
            let active = active
                .lock()
                .map_err(|_| BridgeError::OfflineCashArtifact)?;
            if active.manifest.release_id == manifest.release_id && active.binding.role == role {
                return artifact_error();
            }
            declared = declared
                .checked_add(active.binding.byte_len)
                .ok_or(BridgeError::OfflineCashArtifact)?;
        }
        if declared > OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1 {
            return artifact_error();
        }
        let handle = allocate_handle(
            &ARTIFACT_HANDLES,
            ARTIFACT_HANDLE_NAMESPACE,
            |candidate| registry.contains_key(&candidate),
            BridgeError::OfflineCashArtifact,
        )?;
        let file = tempfile::tempfile().map_err(|_| BridgeError::OfflineCashArtifact)?;
        registry.insert(
            handle,
            Arc::new(Mutex::new(ArtifactIngest {
                manifest,
                binding,
                file,
                hasher: Sha256::new(),
                written: 0,
                ready: false,
                failed: false,
            })),
        );
        unsafe { *out_handle = handle };
        Ok(())
    })();
    bridge_result_to_code(result)
}

/// Append one bounded chunk to an artifact spool.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_artifact_write_v1(
    handle: u64,
    chunk_ptr: *const c_uchar,
    chunk_len: c_ulong,
) -> c_int {
    let result = (|| {
        if !is_handle(handle, ARTIFACT_HANDLE_NAMESPACE) || chunk_ptr.is_null() || chunk_len == 0 {
            return artifact_error();
        }
        let length = usize::try_from(chunk_len).map_err(|_| BridgeError::OfflineCashArtifact)?;
        if length > MAX_ARTIFACT_CHUNK_BYTES {
            return artifact_error();
        }
        let ingest = artifacts()
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?
            .get(&handle)
            .cloned()
            .ok_or(BridgeError::OfflineCashArtifact)?;
        let mut ingest = ingest
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        if ingest.ready || ingest.failed {
            return artifact_error();
        }
        let next = ingest
            .written
            .checked_add(u64::try_from(length).map_err(|_| BridgeError::OfflineCashArtifact)?)
            .ok_or(BridgeError::OfflineCashArtifact)?;
        if next > ingest.binding.byte_len {
            ingest.failed = true;
            return artifact_error();
        }
        let chunk = unsafe { slice::from_raw_parts(chunk_ptr, length) };
        if ingest.file.write_all(chunk).is_err() {
            ingest.failed = true;
            return artifact_error();
        }
        ingest.hasher.update(chunk);
        ingest.written = next;
        Ok(())
    })();
    bridge_result_to_code(result)
}

/// Finalize an artifact only after exact length, digest, and durable-file checks.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_offline_cash_artifact_finalize_v1(handle: u64) -> c_int {
    let result = (|| {
        if !is_handle(handle, ARTIFACT_HANDLE_NAMESPACE) {
            return artifact_error();
        }
        let ingest = artifacts()
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?
            .get(&handle)
            .cloned()
            .ok_or(BridgeError::OfflineCashArtifact)?;
        let mut ingest = ingest
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        if ingest.failed {
            return artifact_error();
        }
        if ingest.ready {
            return Ok(());
        }
        let digest: [u8; 32] = ingest.hasher.clone().finalize().into();
        if ingest.written != ingest.binding.byte_len || digest != ingest.binding.sha256 {
            ingest.failed = true;
            return artifact_error();
        }
        if ingest.file.flush().is_err()
            || ingest.file.sync_all().is_err()
            || ingest.file.seek(SeekFrom::Start(0)).is_err()
        {
            ingest.failed = true;
            return artifact_error();
        }
        let metadata = ingest
            .file
            .metadata()
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        if !metadata.file_type().is_file() || metadata.len() != ingest.binding.byte_len {
            ingest.failed = true;
            return artifact_error();
        }
        let mut permissions = metadata.permissions();
        permissions.set_readonly(true);
        if ingest.file.set_permissions(permissions).is_err() {
            ingest.failed = true;
            return artifact_error();
        }
        ingest.ready = true;
        Ok(())
    })();
    bridge_result_to_code(result)
}

/// Cancel one not-yet-installed artifact spool.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_offline_cash_artifact_cancel_v1(handle: u64) -> c_int {
    let result = (|| {
        if !is_handle(handle, ARTIFACT_HANDLE_NAMESPACE) {
            return artifact_error();
        }
        artifacts()
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?
            .remove(&handle)
            .ok_or(BridgeError::OfflineCashArtifact)?;
        Ok(())
    })();
    bridge_result_to_code(result)
}

/// Threshold-authenticate and atomically install one complete 34-role release.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_artifact_set_install_v1(
    manifest_ptr: *const c_uchar,
    manifest_len: c_ulong,
    expected_manifest_digest_ptr: *const c_uchar,
    expected_manifest_digest_len: c_ulong,
    validation_receipt_ptr: *const c_uchar,
    validation_receipt_len: c_ulong,
    trusted_policy_ptr: *const c_uchar,
    trusted_policy_len: c_ulong,
    release_attestation_ptr: *const c_uchar,
    release_attestation_len: c_ulong,
    handles_ptr: *const u64,
    handles_len: c_ulong,
) -> c_int {
    let result = (|| {
        if handles_ptr.is_null() || handles_len != ARTIFACT_COUNT as c_ulong {
            return artifact_error();
        }
        let manifest_bytes = unsafe {
            read_bounded(
                manifest_ptr,
                manifest_len,
                OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1,
            )
        }?;
        let expected_manifest_digest =
            unsafe { read_digest(expected_manifest_digest_ptr, expected_manifest_digest_len) }?;
        let receipt_bytes = unsafe {
            read_bounded(
                validation_receipt_ptr,
                validation_receipt_len,
                VALIDATION_RECEIPT_MAX_BYTES,
            )
        }?;
        let policy_bytes = unsafe {
            read_bounded(
                trusted_policy_ptr,
                trusted_policy_len,
                OFFLINE_CASH_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
            )
        }?;
        let attestation_bytes = unsafe {
            read_bounded(
                release_attestation_ptr,
                release_attestation_len,
                OFFLINE_CASH_RELEASE_ATTESTATION_MAX_BYTES_V1,
            )
        }?;
        let manifest = decode_manifest(&manifest_bytes)?;
        let receipt = decode_validation_receipt(&receipt_bytes)?;
        let policy = OfflineCashReleaseAuthorityPolicyV1::decode_canonical_exact(&policy_bytes)
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        let attestation =
            OfflineCashReleaseAttestationV1::decode_canonical_exact(&attestation_bytes)
                .map_err(|_| BridgeError::OfflineCashArtifact)?;
        let authenticated = manifest
            .authenticate(&receipt, &policy, &attestation)
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        if authenticated.manifest_digest() != expected_manifest_digest {
            return artifact_error();
        }
        let handles: [u64; ARTIFACT_COUNT] = std::array::from_fn(|index| unsafe {
            std::ptr::read_unaligned(handles_ptr.add(index))
        });
        let (snapshots, files) = {
            let registry = artifacts()
                .lock()
                .map_err(|_| BridgeError::OfflineCashArtifact)?;
            let mut snapshots = Vec::with_capacity(ARTIFACT_COUNT);
            let mut files = Vec::with_capacity(ARTIFACT_COUNT);
            for ((handle, expected_role), expected_binding) in handles
                .iter()
                .copied()
                .zip(OfflineCashArtifactRoleV1::ALL)
                .zip(manifest.artifacts.iter().copied())
            {
                if !is_handle(handle, ARTIFACT_HANDLE_NAMESPACE) {
                    return artifact_error();
                }
                let snapshot = registry
                    .get(&handle)
                    .cloned()
                    .ok_or(BridgeError::OfflineCashArtifact)?;
                let ingest = snapshot
                    .lock()
                    .map_err(|_| BridgeError::OfflineCashArtifact)?;
                if !ingest.ready
                    || ingest.failed
                    || ingest.manifest != manifest
                    || ingest.binding != expected_binding
                    || ingest.binding.role != expected_role
                {
                    return artifact_error();
                }
                let file = ingest
                    .file
                    .try_clone()
                    .map_err(|_| BridgeError::OfflineCashArtifact)?;
                files.push((expected_role, file));
                drop(ingest);
                snapshots.push((handle, snapshot));
            }
            (snapshots, files)
        };
        let verifier = run_kagemusha_recursive_spend_worker_v4(
            "offline-cash-v1-install",
            BridgeError::OfflineCashArtifact,
            move || {
                let source = OfflineCashAuthenticatedArtifactFileSetV1::new(authenticated, files)
                    .map_err(|_| BridgeError::OfflineCashArtifact)?;
                OfflineCashVerifierV1::from_authenticated_artifact_file_set(source)
                    .map_err(|_| BridgeError::OfflineCashArtifact)
            },
        )?;
        let candidate = Arc::new(InstalledRelease { verifier });
        let mut registry = artifacts()
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        for (handle, snapshot) in &snapshots {
            let current = registry
                .get(handle)
                .ok_or(BridgeError::OfflineCashArtifact)?;
            if !Arc::ptr_eq(current, snapshot) {
                return artifact_error();
            }
        }
        for (handle, snapshot) in snapshots {
            let removed = registry
                .remove(&handle)
                .ok_or(BridgeError::OfflineCashArtifact)?;
            if !Arc::ptr_eq(&removed, &snapshot) {
                return artifact_error();
            }
        }
        // Publish while excluding new verification-session insertion, then
        // discard every verifier receipt pinned to the replaced release.
        // `open_verification_session` rechecks the current Arc after taking the
        // verification-session registry lock, so a caller that
        // raced this rotation cannot insert a stale handle afterward.
        let mut verification_session_registry = verification_sessions()
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        *installed()
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)? = Some(candidate);
        verification_session_registry.clear();
        Ok(())
    })();
    bridge_result_to_code(result)
}

/// Remove only the exact installed release selected by both immutable digests.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_artifact_set_uninstall_v1(
    expected_release_id_ptr: *const c_uchar,
    expected_release_id_len: c_ulong,
    expected_manifest_digest_ptr: *const c_uchar,
    expected_manifest_digest_len: c_ulong,
) -> c_int {
    let result = (|| {
        let expected_release_id =
            unsafe { read_digest(expected_release_id_ptr, expected_release_id_len) }?;
        let expected_manifest_digest =
            unsafe { read_digest(expected_manifest_digest_ptr, expected_manifest_digest_len) }?;
        let mut verification_session_registry = verification_sessions()
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        let mut active = installed()
            .lock()
            .map_err(|_| BridgeError::OfflineCashArtifact)?;
        match active.as_ref() {
            Some(current)
                if current.verifier.release_id() != expected_release_id
                    || current.verifier.manifest_digest() != expected_manifest_digest =>
            {
                return artifact_error();
            }
            Some(_) => {
                *active = None;
                verification_session_registry.clear();
            }
            None => {}
        }
        Ok(())
    })();
    bridge_result_to_code(result)
}

/// Unbound verification-session open is unavailable because exact network and
/// asset context is mandatory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_verification_session_open_v1(
    request_ptr: *const c_uchar,
    request_len: c_ulong,
    expected_release_id_ptr: *const c_uchar,
    expected_release_id_len: c_ulong,
    expected_manifest_digest_ptr: *const c_uchar,
    expected_manifest_digest_len: c_ulong,
    out_handle: *mut u64,
) -> c_int {
    if !out_handle.is_null() {
        unsafe { *out_handle = 0 };
    }
    let _ = (
        request_ptr,
        request_len,
        expected_release_id_ptr,
        expected_release_id_len,
        expected_manifest_digest_ptr,
        expected_manifest_digest_len,
    );
    bridge_result_to_code(verification_session_error::<()>())
}

/// Create one opaque verifier-only receiver session bound to exact
/// app-selected network and asset identities.
///
/// The returned handle retains proof-verification receipts in this process. It
/// is not a wallet-runtime handle and cannot authorize device or monetary state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_verification_session_open_bound_v1(
    request_ptr: *const c_uchar,
    request_len: c_ulong,
    expected_release_id_ptr: *const c_uchar,
    expected_release_id_len: c_ulong,
    expected_manifest_digest_ptr: *const c_uchar,
    expected_manifest_digest_len: c_ulong,
    expected_network_id_ptr: *const c_uchar,
    expected_network_id_len: c_ulong,
    expected_asset_definition_id_ptr: *const c_uchar,
    expected_asset_definition_id_len: c_ulong,
    out_handle: *mut u64,
) -> c_int {
    if !out_handle.is_null() {
        unsafe { *out_handle = 0 };
    }
    let result = (|| {
        if out_handle.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let request = unsafe {
            read_verification_session_bounded(
                request_ptr,
                request_len,
                iroha_data_model::offline::OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
            )
        }?;
        let expected_release_id = unsafe {
            read_verification_session_digest(expected_release_id_ptr, expected_release_id_len)
        }?;
        let expected_manifest_digest = unsafe {
            read_verification_session_digest(
                expected_manifest_digest_ptr,
                expected_manifest_digest_len,
            )
        }?;
        let expected_network_id = unsafe {
            read_verification_session_bounded(
                expected_network_id_ptr,
                expected_network_id_len,
                NETWORK_ID_LITERAL_BYTES,
            )
        }?;
        if expected_network_id.len() != NETWORK_ID_LITERAL_BYTES {
            return verification_session_error();
        }
        let expected_asset_definition_id = unsafe {
            read_verification_session_bounded(
                expected_asset_definition_id_ptr,
                expected_asset_definition_id_len,
                ASSET_DEFINITION_ID_LITERAL_MAX_BYTES,
            )
        }?;
        let handle = open_verification_session_canonical_bound(
            &request,
            expected_release_id,
            expected_manifest_digest,
            &expected_network_id,
            &expected_asset_definition_id,
        )?;
        unsafe { *out_handle = handle };
        Ok(())
    })();
    bridge_result_to_code(result)
}

/// Verify both parity proofs and retain their move-only receipt in the
/// verifier-only process session.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_verification_session_verify_payment_v1(
    handle: u64,
    payment_ptr: *const c_uchar,
    payment_len: c_ulong,
    observed_now_ms: u64,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    clear_bridge_output(out_ptr, out_len);
    let result = (|| {
        let payment = unsafe {
            read_verification_session_bounded(
                payment_ptr,
                payment_len,
                OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
            )
        }?;
        let canonical =
            verify_verification_session_payment_canonical(handle, &payment, observed_now_ms)?;
        unsafe {
            publish_offline_cash_output_v1(
                out_ptr,
                out_len,
                &canonical,
                OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
            )
        }
    })();
    bridge_result_to_code(result)
}

/// Verify an acknowledgement against the exact retained paired-proof receipt.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_verification_session_verify_acknowledgement_v1(
    handle: u64,
    acknowledgement_ptr: *const c_uchar,
    acknowledgement_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    clear_bridge_output(out_ptr, out_len);
    let result = (|| {
        let acknowledgement = unsafe {
            read_verification_session_bounded(
                acknowledgement_ptr,
                acknowledgement_len,
                OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            )
        }?;
        let canonical =
            verify_verification_session_acknowledgement_canonical(handle, &acknowledgement)?;
        unsafe {
            publish_offline_cash_output_v1(
                out_ptr,
                out_len,
                &canonical,
                OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            )
        }
    })();
    bridge_result_to_code(result)
}

/// Return the exact monotonic verification state code: request verified `1`,
/// payment verified `2`, acknowledgement verified `3`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_verification_session_state_v1(
    handle: u64,
    out_state: *mut u8,
) -> c_int {
    if !out_state.is_null() {
        unsafe { *out_state = 0 };
    }
    let result = (|| {
        if out_state.is_null() {
            return Err(BridgeError::NullPtr);
        }
        let code = verification_session_state_code(handle)?;
        unsafe { *out_state = code };
        Ok(())
    })();
    bridge_result_to_code(result)
}

/// Destroy one opaque verification session. Repeating close is deliberately rejected.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_offline_cash_verification_session_close_v1(handle: u64) -> c_int {
    let result = close_verification_session(handle);
    bridge_result_to_code(result)
}

// The wallet-runtime session ABI below is intentionally disjoint from the
// verifier-session registry. ABI22 has no reviewed production secure-device
// backend, so it can report only the inert unavailable sentinel and can never
// issue a handle or accept an action.

/// Attempt to open the production wallet runtime.
///
/// ABI22 always clears `out_handle` to zero and returns the Offline Cash
/// session error. A future backend must use a new reviewed implementation gate;
/// symbol presence alone cannot enable this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_wallet_runtime_session_open_v1(
    out_handle: *mut u64,
) -> c_int {
    if !out_handle.is_null() {
        unsafe { *out_handle = 0 };
    }
    if out_handle.is_null() {
        return BridgeError::NullPtr.code();
    }
    BridgeError::OfflineCashSession.code()
}

/// Report the inert wallet-runtime sentinel.
///
/// Success initializes both outputs to the only ABI22 values: status
/// unavailable `0` and state unavailable `0`. No handle is created.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_wallet_runtime_session_status_v1(
    out_status: *mut u8,
    out_state: *mut u8,
) -> c_int {
    if !out_status.is_null() {
        unsafe { *out_status = 0 };
    }
    if !out_state.is_null() {
        unsafe { *out_state = 0 };
    }
    if out_status.is_null() || out_state.is_null() {
        return BridgeError::NullPtr.code();
    }
    0
}

/// Reject every wallet-runtime action without consulting verifier state.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_offline_cash_wallet_runtime_session_attempt_v1(
    handle: u64,
    action: u8,
) -> c_int {
    let _ = (handle, action);
    BridgeError::OfflineCashSession.code()
}

/// Reject close because ABI22 can never create a wallet-runtime handle.
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_offline_cash_wallet_runtime_session_close_v1(
    handle: u64,
) -> c_int {
    let _ = handle;
    BridgeError::OfflineCashSession.code()
}

// The `wallet_session` exports below are ABI22 compatibility aliases only.
// They never represented a durable wallet runtime. New callers must use the
// truthful `verification_session` namespace above.

/// Deprecated verifier-session ABI alias; use
/// [`connect_norito_offline_cash_verification_session_open_v1`].
#[deprecated(
    note = "verifier-only ABI alias; use connect_norito_offline_cash_verification_session_open_v1"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_wallet_session_open_v1(
    request_ptr: *const c_uchar,
    request_len: c_ulong,
    expected_release_id_ptr: *const c_uchar,
    expected_release_id_len: c_ulong,
    expected_manifest_digest_ptr: *const c_uchar,
    expected_manifest_digest_len: c_ulong,
    out_handle: *mut u64,
) -> c_int {
    unsafe {
        connect_norito_offline_cash_verification_session_open_v1(
            request_ptr,
            request_len,
            expected_release_id_ptr,
            expected_release_id_len,
            expected_manifest_digest_ptr,
            expected_manifest_digest_len,
            out_handle,
        )
    }
}

/// Deprecated verifier-session ABI alias; use
/// [`connect_norito_offline_cash_verification_session_open_bound_v1`].
#[deprecated(
    note = "verifier-only ABI alias; use connect_norito_offline_cash_verification_session_open_bound_v1"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_wallet_session_open_bound_v1(
    request_ptr: *const c_uchar,
    request_len: c_ulong,
    expected_release_id_ptr: *const c_uchar,
    expected_release_id_len: c_ulong,
    expected_manifest_digest_ptr: *const c_uchar,
    expected_manifest_digest_len: c_ulong,
    expected_network_id_ptr: *const c_uchar,
    expected_network_id_len: c_ulong,
    expected_asset_definition_id_ptr: *const c_uchar,
    expected_asset_definition_id_len: c_ulong,
    out_handle: *mut u64,
) -> c_int {
    unsafe {
        connect_norito_offline_cash_verification_session_open_bound_v1(
            request_ptr,
            request_len,
            expected_release_id_ptr,
            expected_release_id_len,
            expected_manifest_digest_ptr,
            expected_manifest_digest_len,
            expected_network_id_ptr,
            expected_network_id_len,
            expected_asset_definition_id_ptr,
            expected_asset_definition_id_len,
            out_handle,
        )
    }
}

/// Deprecated verifier-session ABI alias; use
/// [`connect_norito_offline_cash_verification_session_verify_payment_v1`].
#[deprecated(
    note = "verifier-only ABI alias; use connect_norito_offline_cash_verification_session_verify_payment_v1"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_wallet_session_accept_payment_v1(
    handle: u64,
    payment_ptr: *const c_uchar,
    payment_len: c_ulong,
    observed_now_ms: u64,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        connect_norito_offline_cash_verification_session_verify_payment_v1(
            handle,
            payment_ptr,
            payment_len,
            observed_now_ms,
            out_ptr,
            out_len,
        )
    }
}

/// Deprecated verifier-session ABI alias; use
/// [`connect_norito_offline_cash_verification_session_verify_acknowledgement_v1`].
#[deprecated(
    note = "verifier-only ABI alias; use connect_norito_offline_cash_verification_session_verify_acknowledgement_v1"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_wallet_session_accept_acknowledgement_v1(
    handle: u64,
    acknowledgement_ptr: *const c_uchar,
    acknowledgement_len: c_ulong,
    out_ptr: *mut *mut c_uchar,
    out_len: *mut c_ulong,
) -> c_int {
    unsafe {
        connect_norito_offline_cash_verification_session_verify_acknowledgement_v1(
            handle,
            acknowledgement_ptr,
            acknowledgement_len,
            out_ptr,
            out_len,
        )
    }
}

/// Deprecated verifier-session ABI alias; use
/// [`connect_norito_offline_cash_verification_session_state_v1`].
#[deprecated(
    note = "verifier-only ABI alias; use connect_norito_offline_cash_verification_session_state_v1"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_offline_cash_wallet_session_state_v1(
    handle: u64,
    out_state: *mut u8,
) -> c_int {
    unsafe { connect_norito_offline_cash_verification_session_state_v1(handle, out_state) }
}

/// Deprecated verifier-session ABI alias; use
/// [`connect_norito_offline_cash_verification_session_close_v1`].
#[deprecated(
    note = "verifier-only ABI alias; use connect_norito_offline_cash_verification_session_close_v1"
)]
#[unsafe(no_mangle)]
pub extern "C" fn connect_norito_offline_cash_wallet_session_close_v1(handle: u64) -> c_int {
    connect_norito_offline_cash_verification_session_close_v1(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::offline::{
        OFFLINE_CASH_HANDOFF_P95_MAX_MS_V1, OFFLINE_CASH_MIN_FUZZ_CASES_V1,
        OFFLINE_CASH_MIN_QUALIFIED_HANDOFFS_V1, OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1,
        OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1, OFFLINE_CASH_PROVE_P95_MAX_MS_V1,
        OFFLINE_CASH_REPRODUCIBLE_BUILD_COUNT_V1, OFFLINE_CASH_SESSION_TARGET_BYTES_V1,
        OFFLINE_CASH_VALIDATOR_COUNT_V1, OFFLINE_CASH_VERIFY_P95_MAX_MS_V1,
        OFFLINE_CASH_WIRE_VERSION_V1,
    };

    #[test]
    fn validation_receipt_decoder_requires_exact_bounded_canonical_bytes() {
        let receipt = OfflineCashInternalValidationReceiptV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            source_tree_digest: [1; 32],
            cargo_lock_digest: [2; 32],
            profile_digest: [3; 32],
            eq_protocol_digest: [4; 32],
            ep_protocol_digest: [5; 32],
            artifact_set_digest: [6; 32],
            hardware_policy_digest: [7; 32],
            circuit_shape_report_digest: [8; 32],
            security_review_digest: [9; 32],
            kat_report_digest: [10; 32],
            fuzz_report_digest: [11; 32],
            resource_report_digest: [12; 32],
            ios_device_report_digest: [13; 32],
            android_device_report_digest: [14; 32],
            four_peer_report_digest: [15; 32],
            max_proof_pair_bytes: u32::try_from(OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1)
                .expect("proof target fits u32"),
            max_session_bytes: u32::try_from(OFFLINE_CASH_SESSION_TARGET_BYTES_V1)
                .expect("session target fits u32"),
            max_process_rss_bytes: OFFLINE_CASH_PROCESS_RSS_MAX_BYTES_V1,
            prove_p95_ms: OFFLINE_CASH_PROVE_P95_MAX_MS_V1,
            verify_p95_ms: OFFLINE_CASH_VERIFY_P95_MAX_MS_V1,
            handoff_p95_ms: OFFLINE_CASH_HANDOFF_P95_MAX_MS_V1,
            qualified_handoffs: OFFLINE_CASH_MIN_QUALIFIED_HANDOFFS_V1,
            fuzz_cases: OFFLINE_CASH_MIN_FUZZ_CASES_V1,
            reproducible_builds: OFFLINE_CASH_REPRODUCIBLE_BUILD_COUNT_V1,
            validator_count: OFFLINE_CASH_VALIDATOR_COUNT_V1,
        };
        let canonical = norito::encode_canonical(&receipt).expect("encode canonical receipt");
        assert_eq!(
            decode_validation_receipt(&canonical).expect("decode canonical receipt"),
            receipt
        );

        for rejected in [
            Vec::new(),
            vec![0; VALIDATION_RECEIPT_MAX_BYTES + 1],
            b"not-canonical-norito".to_vec(),
        ] {
            assert!(decode_validation_receipt(&rejected).is_err());
        }

        let noncanonical =
            norito::to_compressed_bytes(&receipt, Some(norito::CompressionConfig::default()))
                .expect("encode structurally valid compressed receipt");
        assert_eq!(
            norito::decode_from_bytes::<OfflineCashInternalValidationReceiptV1>(&noncanonical)
                .expect("compressed receipt remains structurally decodable"),
            receipt
        );
        assert!(decode_validation_receipt(&noncanonical).is_err());
    }

    #[test]
    fn verification_session_context_accepts_only_exact_canonical_network_and_asset_literals() {
        let network_literal = "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149";
        let mut asset_bytes = [0_u8; 16];
        asset_bytes[6] = 0x40;
        asset_bytes[8] = 0x80;
        let asset = AssetDefinitionId::from_uuid_bytes(asset_bytes).expect("canonical UUIDv4");
        let asset_literal = asset.to_string();

        let (network, decoded_asset) = decode_exact_verification_session_context(
            network_literal.as_bytes(),
            asset_literal.as_bytes(),
        )
        .expect("exact canonical context");
        assert_eq!(network.to_string(), network_literal);
        assert_eq!(decoded_asset, asset);

        assert!(
            decode_exact_verification_session_context(
                network_literal.to_uppercase().as_bytes(),
                asset_literal.as_bytes(),
            )
            .is_err()
        );
        assert!(
            decode_exact_verification_session_context(
                network_literal.as_bytes(),
                format!(" {asset_literal}").as_bytes(),
            )
            .is_err()
        );
    }
}
