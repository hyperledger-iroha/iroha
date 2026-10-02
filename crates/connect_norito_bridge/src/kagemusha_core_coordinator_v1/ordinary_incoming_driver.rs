//! Distinct first-release incoming Native lifecycle on the actual installed Cash owner.
//! No frame carries a key, verifier, release, clock, financial grant or decoded capability.
use super::*;
use iroha_core_zk::kagemusha_v1_recursion::{
    KAGEMUSHA_ORDINARY_CASH_OUTGOING_ORIGINAL_MAX_BYTES_V1 as OUTGOING_MAX,
    KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1 as COMMIT_MAX,
    KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1 as RESERVE_MAX,
};
use iroha_core_zk::kagemusha_v1_state::KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1 as RECEIVED_MAX;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1 as CREDIT_MAX,
    KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1 as FINALIZED_MAX,
    KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1 as SIGNED_MAX,
    KagemushaOrdinaryLineageRequestOperationV1, KagemushaOrdinaryLineageRequestV1,
};
use sha2::{Digest as _, Sha256};
const DATA_MAX: usize = 128 * 1024;
const AUTHORITY_MAX: usize = 128 * 1024 * 1024;
const FRAME_MAX: usize =
    AUTHORITY_MAX + SIGNED_MAX + FINALIZED_MAX + CREDIT_MAX + OUTGOING_MAX + RECEIVED_MAX + 4096;
const REQUEST_MAX: usize = 192 * 1024;

/// Closed incoming phase request. This serializable type is data only.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeIncomingRequestV1")]
pub struct KagemushaOrdinaryNativeIncomingRequestV1 {
    /// Exactly one; no retired decoder fallback.
    pub version: u16,
    /// 1 prepare actual finalized Mint W2; 2 fence W2; 3 retain raw W2; 4 recover W2;
    /// 5 prove/fsync candidate; 6 Native signed Reserve transport; 7 intake global result;
    /// 8 select fresh W1 from real Reserve; 9 fence W1; 10 retain raw W1; 11 recover W1;
    /// 12 prove/fsync Commit; 13 Native signed Commit transport; 14 StateAdvance; 15 Ack;
    /// 16 refresh genuine installed four-node Native signed clock; 17 authenticate/retain actual
    /// received source by the captured request, then select fresh Receive W2; 18 read the actual
    /// separately retained platform counter for this exact operation and W purpose.
    pub phase: u8,
    /// Same already-opened ordinary descriptor; it cannot install an owner.
    pub core_handle: u64,
    /// Sole originals accepted by the selected phase, never a caller subject/authority/clock.
    pub originals: Vec<Vec<u8>>,
}
/// Data-only original projections after actual Native invocation.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeIncomingResponseV1")]
pub struct KagemushaOrdinaryNativeIncomingResponseV1 {
    /// Exactly one.
    pub version: u16,
    /// Same request phase.
    pub phase: u8,
    /// Same actual opened ordinary descriptor.
    pub core_handle: u64,
    /// Actual owner projections. Signed transport starts with 0=pending/dispatch exact originals
    /// or 2=already acknowledged/recover exact originals; the latter needs no HTTP redispatch.
    pub fields: Vec<Vec<u8>>,
}
fn require_shape(phase: u8, handle: u64, lengths: &[usize]) -> Result<(), Error> {
    if handle == 0 {
        return Err(Error::Rejected);
    }
    let valid = match (phase, lengths) {
        (1, [finalized, credit]) => {
            (1..=FINALIZED_MAX).contains(finalized) && (1..=CREDIT_MAX).contains(credit)
        }
        (17, [32, outgoing, assertion]) => {
            (1..=OUTGOING_MAX).contains(outgoing) && (1..=RECEIVED_MAX).contains(assertion)
        }
        (18, [32, 1]) => true,
        (3 | 10, [raw]) => (1..=4096).contains(raw),
        (7, [signed, data, authority]) => {
            (1..=SIGNED_MAX).contains(signed)
                && (1..=DATA_MAX).contains(data)
                && (1..=AUTHORITY_MAX).contains(authority)
        }
        (8 | 14 | 15, [32]) => true,
        (2 | 4 | 5 | 6 | 9 | 11 | 12 | 13 | 16, []) => true,
        _ => false,
    };
    if valid { Ok(()) } else { Err(Error::Rejected) }
}
fn key(raw: &[u8]) -> Result<[u8; 32], Error> {
    let key: [u8; 32] = raw.try_into().map_err(|_| Error::Rejected)?;
    if key == [0; 32] {
        return Err(Error::Rejected);
    }
    Ok(key)
}
fn require_fields(phase: u8, fields: &[Vec<u8>]) -> Result<(), Error> {
    let valid = match (phase, fields) {
        (1 | 8 | 17, [operation, w, subject, credential]) => {
            operation.len() == 32
                && w.len() == 325
                && subject.len() == 460
                && !credential.is_empty()
                && credential.len() <= SIGNED_MAX
        }
        (2 | 4 | 9 | 11, [status, raw, original]) => {
            matches!(status.as_slice(), [0] | [1] | [2])
                && raw.len() <= 4096
                && original.len() <= SIGNED_MAX
                && if status == &[0] {
                    raw.is_empty() && original.is_empty()
                } else {
                    !raw.is_empty() && !original.is_empty()
                }
        }
        (3 | 5 | 7 | 10 | 12, [digest]) => key(digest).is_ok(),
        (6 | 13, [status, request, signature, proof, digest]) => {
            if !matches!(status.as_slice(), [0] | [2])
                || request.is_empty()
                || request.len() > REQUEST_MAX
                || signature.len() != 64
                || proof.is_empty()
                || proof.len() > if phase == 6 { RESERVE_MAX } else { COMMIT_MAX }
                || digest.as_slice() != &<[u8; 32]>::from(Sha256::digest(request))
            {
                return Err(Error::Rejected);
            }
            let decoded: KagemushaOrdinaryLineageRequestV1 = norito::decode_canonical_with_limits(
                request,
                norito::canonical_decode_limits(REQUEST_MAX),
            )
            .map_err(|_| Error::Rejected)?;
            if decoded.canonical_bytes().map_err(|_| Error::Rejected)? != *request {
                return Err(Error::Rejected);
            }
            decoded
                .verify_account_signature(&iroha_crypto::Signature::from_bytes(signature))
                .map_err(|_| Error::Rejected)?;
            matches!(
                (phase, decoded.operation),
                (
                    6,
                    KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(_)
                ) | (
                    13,
                    KagemushaOrdinaryLineageRequestOperationV1::CommitIncoming(_)
                )
            )
        }
        (18, [platform, floor]) => {
            matches!((platform.as_slice(), floor.len()), ([5], 0) | ([4], 4))
        }
        (14 | 15 | 16, []) => true,
        _ => false,
    };
    if valid { Ok(()) } else { Err(Error::Rejected) }
}
fn invoke_originals(
    phase: u8,
    handle: u64,
    originals: &[Vec<u8>],
) -> Result<KagemushaOrdinaryNativeIncomingResponseV1, Error> {
    require_shape(
        phase,
        handle,
        &originals.iter().map(Vec::len).collect::<Vec<_>>(),
    )?;
    let installed = INSTALL.lock().map_err(|_| Error::Rejected)?;
    if !installed.succeeded {
        return Err(Error::Unavailable);
    }
    let backend = ACTIVE.get().ok_or(Error::Unavailable)?;
    if installed.attempted_path.as_deref() != backend.path.to_str() {
        return Err(Error::Rejected);
    }
    // Renewal alone checks installed originals and the same registry/account selection first.
    // The expired S/W cut and shared clock are replaced only by the real startup read below.
    if phase == 16 {
        backend
            .source
            .recheck_installed_originals_for_refresh(&backend.path)?;
    } else if matches!(phase, 5 | 12) {
        // Proof work consumes captured approvals and historical decisions only. The
        // exact installed account, retirement and journal remain held; current S/W
        // observations need not last through potentially long pure computation.
        backend
            .source
            .recheck_retained_owner_originals(&backend.path)?;
    } else {
        backend.source.recheck_originals(&backend.path)?;
    }
    drop(installed);
    let session = backend
        .source
        .native_account_session
        .as_ref()
        .ok_or(Error::Unavailable)?;
    // No Main borrower/effect, callback, managed clock or caller endpoint enters renewal.
    if phase == 16 {
        {
            let owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
            if owner.handle != Some(handle) {
                return Err(Error::Rejected);
            }
        }
        session.refresh_incoming_clock()?;
        session.recheck()?;
        backend.source.recheck_originals(&backend.path)?;
        {
            let owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
            if owner.handle != Some(handle) {
                return Err(Error::Rejected);
            }
        }
        return Ok(KagemushaOrdinaryNativeIncomingResponseV1 {
            version: 1,
            phase,
            core_handle: handle,
            fields: vec![],
        });
    }
    if matches!(phase, 5 | 12) {
        session.recheck_retained_account_identity()?;
    } else {
        session.recheck()?;
    }
    let mut owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
    if owner.handle != Some(handle) {
        return Err(Error::Rejected);
    }
    // Only the already initialized genuine Cash disposition is callable. A hardware/Bootstrap
    // holder, decoded frame or first-device evidence cannot create monetary custody here.
    let cash = owner.cash.as_mut().ok_or(Error::Unavailable)?;
    let fields = match phase {
        1 => cash
            .prepare_finalized_incoming_mint_platform(&originals[0], &originals[1])
            .map_err(|_| Error::Rejected)?,
        17 => cash
            .prepare_received_incoming_platform(key(&originals[0])?, &originals[1], &originals[2])
            .map_err(|_| Error::Rejected)?,
        2 | 9 => cash
            .fence_incoming_platform(phase == 9)
            .map_err(|_| Error::Rejected)?,
        3 | 10 => vec![
            cash.retain_incoming_platform_original(phase == 10, &originals[0])
                .map_err(|_| Error::Rejected)?
                .to_vec(),
        ],
        4 | 11 => cash
            .recover_incoming_platform(phase == 11)
            .map_err(|_| Error::Rejected)?,
        5 | 12 => {
            let proving = backend
                .source
                .bootstrap
                .as_ref()
                .and_then(|m| m.proving.as_ref())
                .ok_or(Error::Unavailable)?;
            let resolver = super::super::native_core_work::Resolver(proving.resolver.clone());
            let digest = if phase == 5 {
                cash.prove_retained_incoming_reservation(proving.profile.clone(), resolver)
            } else {
                cash.prove_retained_incoming_commit(proving.profile.clone(), resolver)
            }
            .map_err(|_| Error::Rejected)?;
            vec![digest.to_vec()]
        }
        6 => cash
            .sign_incoming_reservation_transport(|original| {
                session.sign_lineage(original).map_err(|_| {
                    iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity
                })
            })
            .map_err(|_| Error::Rejected)?,
        7 => vec![
            cash.accept_retained_lineage_result(&originals[0], &originals[1], &originals[2])
                .map_err(|_| Error::Rejected)?
                .to_vec(),
        ],
        8 => {
            cash.select_retained_incoming_terminal(key(&originals[0])?)
                .map_err(|_| Error::Rejected)?;
            cash.incoming_platform_fields(true)
                .map_err(|_| Error::Rejected)?
        }
        18 => {
            let terminal = match originals[1].as_slice() {
                [2] => false,
                [1] => true,
                _ => return Err(Error::Rejected),
            };
            cash.incoming_platform_counter_original(terminal, key(&originals[0])?)
                .map_err(|_| Error::Rejected)?
        }
        13 => cash
            .sign_incoming_commit_transport(|original| {
                session.sign_lineage(original).map_err(|_| {
                    iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity
                })
            })
            .map_err(|_| Error::Rejected)?,
        14 => {
            cash.advance_incoming_commit(key(&originals[0])?)
                .map_err(|_| Error::Rejected)?;
            vec![]
        }
        15 => {
            cash.acknowledge_incoming_commit_state_advance(key(&originals[0])?)
                .map_err(|_| Error::Rejected)?;
            vec![]
        }
        _ => return Err(Error::Rejected),
    };
    if matches!(phase, 5 | 12) {
        session.recheck_retained_account_identity()?;
        backend
            .source
            .recheck_retained_owner_originals(&backend.path)?;
    } else {
        // Signing, submission, selection and State effects still require their
        // genuine fresh account cut. Phase16 alone obtains another same-S/W cut.
        session.recheck()?;
        backend.source.recheck_originals(&backend.path)?;
    }
    require_fields(phase, &fields)?;
    Ok(KagemushaOrdinaryNativeIncomingResponseV1 {
        version: 1,
        phase,
        core_handle: handle,
        fields,
    })
}
/// Decode sole bounded first-release transport then invoke the actual retained Native owner.
/// # Errors
/// Rejects malformed/foreign frames, unavailable real material and every failed custody check.
pub fn invoke_kagemusha_native_ordinary_incoming_v1(frame: &[u8]) -> Result<Vec<u8>, Error> {
    if frame.is_empty() || frame.len() > FRAME_MAX {
        return Err(Error::Rejected);
    }
    let request: KagemushaOrdinaryNativeIncomingRequestV1 =
        norito::decode_canonical_with_limits(frame, norito::canonical_decode_limits(FRAME_MAX))
            .map_err(|_| Error::Rejected)?;
    if request.version != 1
        || request.originals.len() > 3
        || norito::encode_canonical(&request).map_err(|_| Error::Rejected)? != frame
    {
        return Err(Error::Rejected);
    }
    let response = invoke_originals(request.phase, request.core_handle, &request.originals)?;
    norito::encode_canonical(&response).map_err(|_| Error::Rejected)
}
/// C entry for the same full original frame; no small coordinator limit truncates an original.
/// # Safety
/// Input must be readable for its checked length; output pointers must be nonnull/writable and
/// released by the standard bridge allocator. Output is reset before any refusal or input read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_ordinary_incoming_v1(
    input: *const u8,
    input_len: usize,
    output: *mut *mut u8,
    output_len: *mut usize,
) -> std::ffi::c_int {
    if output.is_null() || output_len.is_null() {
        return crate::ERR_NULL_PTR;
    }
    unsafe {
        *output = std::ptr::null_mut();
        *output_len = 0;
    }
    if input.is_null() || input_len == 0 || input_len > FRAME_MAX {
        return crate::ERR_KAGEMUSHA_V1;
    }
    let frame =
        zeroize::Zeroizing::new(unsafe { std::slice::from_raw_parts(input, input_len) }.to_vec());
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        invoke_kagemusha_native_ordinary_incoming_v1(&frame)
    })) {
        Ok(Ok(response)) => unsafe { crate::write_bytes_usize(output, output_len, &response) }
            .map_or_else(|error| error, |()| 0),
        Ok(Err(Error::Unavailable)) => crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1,
        _ => crate::ERR_KAGEMUSHA_V1,
    }
}
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeIncomingV1<
    'local,
>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    phase: jni::sys::jint,
    handle: jni::sys::jlong,
    originals: jni::objects::JObjectArray<'local>,
) -> jni::sys::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<jni::sys::jobjectArray, ()> {
            #[cfg(any(target_os = "android", test))]
            super::super::ordinary_android_installed_context::AndroidOrdinaryInstalledContextOwnerV1::require_early_retained_jni_class(&mut env, &_class).map_err(|_| ())?;
            let phase = u8::try_from(phase).map_err(|_| ())?;
            let handle = u64::from_ne_bytes(handle.to_ne_bytes());
            let count = usize::try_from(env.get_array_length(&originals).map_err(|_| ())?)
                .map_err(|_| ())?;
            if count > 3 {
                return Err(());
            }
            let mut arrays = Vec::with_capacity(count);
            let mut lengths = Vec::with_capacity(count);
            for n in 0..count {
                let a: jni::objects::JByteArray<'_> = env
                    .get_object_array_element(&originals, n as i32)
                    .map_err(|_| ())?
                    .into();
                lengths.push(
                    usize::try_from(env.get_array_length(&a).map_err(|_| ())?).map_err(|_| ())?,
                );
                arrays.push(a);
            }
            require_shape(phase, handle, &lengths).map_err(|_| ())?;
            let mut raw = Vec::with_capacity(count);
            for (a, n) in arrays.iter().zip(lengths) {
                let bytes = env.convert_byte_array(a).map_err(|_| ())?;
                if bytes.len() != n {
                    return Err(());
                }
                raw.push(bytes);
            }
            let raw = zeroize::Zeroizing::new(raw);
            let response = invoke_originals(phase, handle, &raw).map_err(|_| ())?;
            let mut fields = vec![
                response.version.to_le_bytes().to_vec(),
                vec![response.phase],
                response.core_handle.to_le_bytes().to_vec(),
            ];
            fields.extend(response.fields);
            let output = env
                .new_object_array(fields.len() as i32, "[B", jni::objects::JObject::null())
                .map_err(|_| ())?;
            for (n, field) in fields.iter().enumerate() {
                let a = env.byte_array_from_slice(field).map_err(|_| ())?;
                env.set_object_array_element(&output, n as i32, &a)
                    .map_err(|_| ())?;
            }
            #[cfg(any(target_os = "android", test))]
            super::super::ordinary_android_installed_context::AndroidOrdinaryInstalledContextOwnerV1::require_early_retained_jni_class(&mut env, &_class).map_err(|_| ())?;
            Ok(output.into_raw())
        },
    ));
    match result {
        Ok(Ok(output)) => output,
        _ => std::ptr::null_mut(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incoming_lifecycle_exact_roles_and_bounds() {
        for p in [2, 4, 5, 6, 9, 11, 12, 13, 16] {
            assert!(require_shape(p, 1, &[]).is_ok());
            assert!(require_shape(p, 1, &[1]).is_err());
        }
        assert!(require_shape(1, 1, &[FINALIZED_MAX, CREDIT_MAX]).is_ok());
        for lengths in [[0, 1], [1, 0], [FINALIZED_MAX + 1, 1], [1, CREDIT_MAX + 1]] {
            assert!(require_shape(1, 1, &lengths).is_err());
        }
        assert!(require_shape(7, 1, &[SIGNED_MAX, DATA_MAX, AUTHORITY_MAX]).is_ok());
        assert!(require_shape(7, 1, &[SIGNED_MAX + 1, 1, 1]).is_err());
        for p in [8, 14, 15] {
            assert!(require_shape(p, 1, &[32]).is_ok());
            assert!(require_shape(p, 1, &[31]).is_err());
        }
        assert!(require_shape(6, 0, &[]).is_err());
        assert!(require_shape(17, 1, &[]).is_err());
        assert!(key(&[0; 32]).is_err());
    }
    #[test]
    fn receive_phase_requires_complete_original_roles_without_mint_fallback() {
        assert!(require_shape(17, 1, &[32, OUTGOING_MAX, RECEIVED_MAX]).is_ok());
        for lengths in [
            [31, 1, 1],
            [32, 0, 1],
            [32, 1, 0],
            [32, OUTGOING_MAX + 1, 1],
            [32, 1, RECEIVED_MAX + 1],
        ] {
            assert!(require_shape(17, 1, &lengths).is_err());
        }
        assert!(require_shape(17, 1, &[1, 1]).is_err());
        assert!(require_shape(1, 1, &[32, 1, 1]).is_err());
        assert!(require_shape(17, 0, &[32, 1, 1]).is_err());
    }
    #[test]
    fn incoming_c_entry_resets_before_oversized_pointer_read() {
        let mut out = std::ptr::dangling_mut();
        let mut len = 1;
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_ordinary_incoming_v1(
                    std::ptr::dangling(),
                    FRAME_MAX + 1,
                    &mut out,
                    &mut len,
                )
            },
            crate::ERR_KAGEMUSHA_V1
        );
        assert!(out.is_null());
        assert_eq!(len, 0);
    }
    #[test]
    fn incoming_data_never_installs_native_owner() {
        assert!(invoke_kagemusha_native_ordinary_incoming_v1(&[]).is_err());
        let raw = norito::encode_canonical(&KagemushaOrdinaryNativeIncomingRequestV1 {
            version: 2,
            phase: 6,
            core_handle: 1,
            originals: vec![],
        })
        .unwrap();
        assert!(invoke_kagemusha_native_ordinary_incoming_v1(&raw).is_err());
        assert!(require_fields(14, &[vec![1]]).is_err());
        assert!(require_fields(6, &[]).is_err());
    }
    #[test]
    fn actual_swift_incoming_frames_match_native_canonical_codec() {
        // Actual Swift116 canonical vectors. This checks codecs only, without an owner,
        // signer, proof, financial grant or physical/release qualification.
        let corpus = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/kagemusha/ordinary_native_incoming_transport_v1.tsv"
        ));
        let mut count = 0;
        for line in corpus
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let (name, bytes) = line.split_once('\t').expect("finite named codec vector");
            let bytes = hex::decode(bytes).expect("public canonical vector hex");
            if name.ends_with("_request") {
                let value: KagemushaOrdinaryNativeIncomingRequestV1 =
                    norito::decode_canonical_with_limits(
                        &bytes,
                        norito::canonical_decode_limits(FRAME_MAX),
                    )
                    .expect("actual Swift request must decode through sole Native codec");
                assert_eq!(value.version, 1);
                assert_eq!(value.core_handle, 19);
                assert_eq!(
                    value.phase,
                    if name.starts_with("refresh") { 16 } else { 3 }
                );
                require_shape(
                    value.phase,
                    value.core_handle,
                    &value.originals.iter().map(Vec::len).collect::<Vec<_>>(),
                )
                .unwrap();
                assert_eq!(norito::encode_canonical(&value).unwrap(), bytes);
            } else {
                let value: KagemushaOrdinaryNativeIncomingResponseV1 =
                    norito::decode_canonical_with_limits(
                        &bytes,
                        norito::canonical_decode_limits(FRAME_MAX),
                    )
                    .expect("actual Swift response must decode through sole Native codec");
                assert_eq!(value.version, 1);
                assert_eq!(value.core_handle, 19);
                assert_eq!(
                    value.phase,
                    if name.starts_with("refresh") { 16 } else { 3 }
                );
                require_fields(value.phase, &value.fields).unwrap();
                assert_eq!(norito::encode_canonical(&value).unwrap(), bytes);
            }
            count += 1;
        }
        assert_eq!(count, 4);
    }
}
