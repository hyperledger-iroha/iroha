//! Genuine outgoing Cash Main lifecycle. Software financial WAL custody is retained, not
//! promoted to an OEM hardware-sealing guarantee. Qualified original app keys sign W1 only.
use super::*;
use iroha_core_zk::kagemusha_v1_recursion::{
    KAGEMUSHA_ORDINARY_LINEAGE_SERVICE_ORIGINAL_MAX_BYTES_V1 as PROOF_MAX,
    KagemushaOrdinaryLineageServiceOriginalV1,
};
use iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1 as SIGNED_MAX,
    KagemushaAppKeySecurityLevelV1 as Security, KagemushaHardwarePlatformClassV1 as Platform,
    KagemushaOrdinaryLineageRequestOperationV1, KagemushaOrdinaryLineageRequestV1,
    KagemushaOrdinaryRetailEnrollmentCertificateV1,
};
use sha2::{Digest as _, Sha256};
const DATA_MAX: usize = 128 * 1024;
const AUTHORITY_MAX: usize = 128 * 1024 * 1024;
const FRAME_MAX: usize = AUTHORITY_MAX + SIGNED_MAX + DATA_MAX + 4096;
const REQUEST_MAX: usize = 192 * 1024;

/// Preserve real owner errors. A capacity/storage/proof error is never a successful receipt.
#[derive(Debug)]
pub enum KagemushaOrdinaryOutgoingErrorV1 {
    /// Native descriptor/session/account selection failure, unchanged.
    Coordination(Error),
    /// The actual Cash Main error, including its original proof/capacity cause.
    Main(KagemushaStateErrorV1),
}
impl From<Error> for KagemushaOrdinaryOutgoingErrorV1 {
    fn from(value: Error) -> Self {
        Self::Coordination(value)
    }
}
impl From<KagemushaStateErrorV1> for KagemushaOrdinaryOutgoingErrorV1 {
    fn from(value: KagemushaStateErrorV1) -> Self {
        Self::Main(value)
    }
}
type Result<T> = core::result::Result<T, KagemushaOrdinaryOutgoingErrorV1>;

/// Closed canonical request, carrying only exact original responses or a retained request hash.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeOutgoingRequestV1")]
pub struct KagemushaOrdinaryNativeOutgoingRequestV1 {
    /// Exactly one. No compatibility decoder.
    pub version: u16,
    /// 5 prove/fsync W2 candidate, 6 account-signed Reserve, 7 intake actual global response,
    /// 8 select W1, 9 fsync OS fence, 10 retain/consume raw W1, 11 recover W1,
    /// 12 prove/fsync whole Commit, 13 account-signed Commit, 14 acknowledged StateAdvance,
    /// 15 distinct fresh FI Ack, 16 real Native clock refresh, 17 acknowledged delivery,
    /// 18 recheck the exact original W1/key metadata. Method19 alone prepares/captures W2.
    pub phase: u8,
    /// Same already opened actual descriptor; this never creates a monetary owner.
    pub core_handle: u64,
    /// Exact phase-selected originals, never a key, W, profile, clock or verdict.
    pub originals: Vec<Vec<u8>>,
}
/// Public data only; actual Main proof, lineage CAS and durable Ack remain Native-owned.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeOutgoingResponseV1")]
pub struct KagemushaOrdinaryNativeOutgoingResponseV1 {
    /// Exactly one.
    pub version: u16,
    /// Same requested phase.
    pub phase: u8,
    /// Same actual descriptor.
    pub core_handle: u64,
    /// Original transport or fixed purpose1 signing data. No managed-ready flag.
    pub fields: Vec<Vec<u8>>,
}
fn require_shape(phase: u8, handle: u64, lengths: &[usize]) -> core::result::Result<(), Error> {
    if handle == 0 {
        return Err(Error::Rejected);
    }
    let valid = match (phase, lengths) {
        (10, [raw]) => (1..=4096).contains(raw),
        (7, [signed, data, authority]) => {
            (1..=SIGNED_MAX).contains(signed)
                && (1..=DATA_MAX).contains(data)
                && (1..=AUTHORITY_MAX).contains(authority)
        }
        (8 | 14 | 15 | 17, [32]) => true,
        (5 | 6 | 9 | 11 | 12 | 13 | 16 | 18, []) => true,
        _ => false,
    };
    if valid { Ok(()) } else { Err(Error::Rejected) }
}
fn key(raw: &[u8]) -> core::result::Result<[u8; 32], Error> {
    let key: [u8; 32] = raw.try_into().map_err(|_| Error::Rejected)?;
    if key == [0; 32] {
        Err(Error::Rejected)
    } else {
        Ok(key)
    }
}
fn require_fields(phase: u8, fields: &[Vec<u8>]) -> core::result::Result<(), Error> {
    let valid = match (phase, fields) {
        (
            8 | 18,
            [
                operation,
                w,
                platform,
                alias,
                attestation,
                point,
                key_id,
                c,
                credential,
                subject,
                floor,
                mask,
                app_identity,
                fi,
            ],
        ) => {
            key(operation).is_ok()
                && w.len() == 325
                && matches!(platform.as_slice(), [4] | [5])
                && !alias.is_empty()
                && alias.len() <= 128
                && std::str::from_utf8(alias).is_ok()
                && key(attestation).is_ok()
                && point.len() == 65
                && point[0] == 4
                && key(key_id).is_ok()
                && *key_id == <[u8; 32]>::from(Sha256::digest(point))
                && !c.is_empty()
                && c.len() <= 4096
                && key(credential).is_ok()
                && subject.len() == 460
                && key(app_identity).is_ok()
                && !fi.is_empty()
                && fi.len() <= SIGNED_MAX
                && if platform == &[5] {
                    floor.is_empty() && matches!(mask.as_slice(), [1] | [2])
                } else {
                    floor.len() == 4 && mask == &[0]
                }
        }
        (9 | 11, [status, raw, original]) => {
            matches!(status.as_slice(), [0] | [1] | [2])
                && raw.len() <= 4096
                && original.len() <= SIGNED_MAX
                && if status == &[0] {
                    raw.is_empty() && original.is_empty()
                } else {
                    !raw.is_empty() && !original.is_empty()
                }
        }
        (5 | 7 | 10 | 12, [digest]) => key(digest).is_ok(),
        (6 | 13, [status, request, signature, proof, digest]) => {
            if !matches!(status.as_slice(), [0] | [2])
                || request.is_empty()
                || request.len() > REQUEST_MAX
                || signature.len() != 64
                || proof.is_empty()
                || proof.len() > PROOF_MAX
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
            let carrier = KagemushaOrdinaryLineageServiceOriginalV1::decode_original(proof)
                .map_err(|_| Error::Rejected)?;
            matches!(
                (phase, decoded.operation),
                (6, KagemushaOrdinaryLineageRequestOperationV1::Reserve(_))
                    | (13, KagemushaOrdinaryLineageRequestOperationV1::Commit(_))
            ) && if phase == 6 {
                carrier.reservation_originals().is_some()
            } else {
                carrier.commit_bundle_original().is_some()
            }
        }
        (14 | 15 | 16, []) => true,
        (17, [original]) => !original.is_empty() && original.len() <= PROOF_MAX,
        _ => false,
    };
    if valid { Ok(()) } else { Err(Error::Rejected) }
}
fn terminal_fields(
    owner: &Owner,
    source: &KagemushaNativeOrdinaryAppIdentitySourceV1,
    original: Vec<Vec<u8>>,
) -> Result<Vec<Vec<u8>>> {
    let [operation, w, subject, fi]: [Vec<u8>; 4] =
        original.try_into().map_err(|_| Error::Rejected)?;
    let now = source
        .selected
        .trusted_time_ms()
        .map_err(|_| Error::Rejected)?;
    let pending = owner
        .attempt
        .as_ref()
        .ok_or(Error::Unavailable)?
        .retained_pending_identity()
        .map_err(|_| Error::Rejected)?;
    let credential = owner
        .possession
        .as_ref()
        .ok_or(Error::Unavailable)?
        .final_identity(pending, now)
        .map_err(|_| Error::Rejected)?;
    let c = pending
        .preparation()
        .retained_preparation(now)
        .map_err(|_| Error::Rejected)?;
    let selected = credential.subject();
    // Decoding FI here compares data with the independently held genuine C. It installs no grant.
    let decoded: KagemushaOrdinaryRetailEnrollmentCertificateV1 =
        norito::decode_canonical_with_limits(&fi, norito::canonical_decode_limits(SIGNED_MAX))
            .map_err(|_| Error::Rejected)?;
    if decoded.canonical_bytes().map_err(|_| Error::Rejected)? != fi
        || decoded.subject.ordinary_app_credential_digest != credential.digest()
        || pending.raw_admission().subject().app_public_key != selected.app_public_key
        || pending.raw_admission().subject().attested_key_id != selected.attested_key_id
    {
        return Err(Error::Rejected.into());
    }
    // W1 retains its own original floor. Enrollment and mutable post-capture floors differ.
    let counter_floor = owner
        .cash
        .as_ref()
        .ok_or(Error::Unavailable)?
        .outgoing_terminal_app_attest_counter_floor()?;
    let (platform, mask, floor) = match (
        selected.platform_class,
        selected.security_level,
        counter_floor,
    ) {
        (Platform::AndroidKeyMint, Security::TrustedExecutionEnvironment, None) => (5, 1, vec![]),
        (Platform::AndroidKeyMint, Security::StrongBox, None) => (5, 2, vec![]),
        (Platform::AppleAppAttest, Security::AppleAppAttest, Some(floor)) => {
            (4, 0, floor.to_le_bytes().to_vec())
        }
        _ => return Err(Error::Rejected.into()),
    };
    Ok(vec![
        operation,
        w,
        vec![platform],
        pending.original_alias().as_bytes().to_vec(),
        c.challenge
            .attestation_challenge()
            .map_err(|_| Error::Rejected)?
            .to_vec(),
        selected.app_public_key.as_sec1_bytes().to_vec(),
        selected.attested_key_id.to_vec(),
        c.challenge
            .canonical_signing_bytes()
            .map_err(|_| Error::Rejected)?,
        credential.digest().to_vec(),
        subject,
        floor,
        vec![mask],
        selected.app_signing_identity_digest.to_vec(),
        fi,
    ])
}
fn invoke_originals(
    phase: u8,
    handle: u64,
    originals: &[Vec<u8>],
) -> Result<KagemushaOrdinaryNativeOutgoingResponseV1> {
    require_shape(
        phase,
        handle,
        &originals.iter().map(Vec::len).collect::<Vec<_>>(),
    )?;
    let installed = INSTALL.lock().map_err(|_| Error::Rejected)?;
    if !installed.succeeded {
        return Err(Error::Unavailable.into());
    }
    let backend = ACTIVE.get().ok_or(Error::Unavailable)?;
    if installed.attempted_path.as_deref() != backend.path.to_str() {
        return Err(Error::Rejected.into());
    }
    // Only genuine renewal replaces an expired current S/W cut. Captured proof work
    // retains the installed owner and immutable inputs without lending a live effect.
    if phase == 16 {
        backend
            .source
            .recheck_installed_originals_for_refresh(&backend.path)?;
    } else if matches!(phase, 5 | 12) {
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
    if phase == 16 {
        let owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
        if owner.handle != Some(handle) || owner.cash.is_none() {
            return Err(Error::Unavailable.into());
        }
        drop(owner);
        session.refresh_incoming_clock()?; // Existing genuine four-node original workflow.
        session.recheck()?;
        backend.source.recheck_originals(&backend.path)?;
        {
            let owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
            if owner.handle != Some(handle) || owner.cash.is_none() {
                return Err(Error::Unavailable.into());
            }
        }
        return Ok(KagemushaOrdinaryNativeOutgoingResponseV1 {
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
        return Err(Error::Rejected.into());
    }
    let cash = owner.cash.as_mut().ok_or(Error::Unavailable)?;
    let mut fields = match phase {
        5 | 8 | 12 | 6 => {
            let proving = backend
                .source
                .bootstrap
                .as_ref()
                .and_then(|m| m.proving.as_ref())
                .ok_or(Error::Unavailable)?;
            let resolver = super::super::native_core_work::Resolver(proving.resolver.clone());
            match phase {
                5 => vec![
                    cash.prove_retained_outgoing_reservation(proving.profile.clone(), resolver)?
                        .to_vec(),
                ],
                8 => cash.select_retained_outgoing_terminal(
                    key(&originals[0])?,
                    proving.profile.clone(),
                    resolver,
                )?,
                12 => vec![
                    cash.prove_retained_outgoing_commit(proving.profile.clone(), resolver)?
                        .to_vec(),
                ],
                6 => {
                    let mut account_error = None;
                    let result = cash.sign_outgoing_reservation_transport(
                        proving.profile.clone(),
                        resolver,
                        |loan| {
                            session.sign_lineage(loan).map_err(|error| {
                                account_error = Some(error);
                                KagemushaStateErrorV1::SnapshotIntegrity
                            })
                        },
                    );
                    if let Some(error) = account_error {
                        return Err(error.into());
                    }
                    let [
                        status,
                        request,
                        signature,
                        bundle,
                        prior,
                        neutral,
                        clock,
                        digest,
                    ]: [Vec<u8>; 8] = result?.try_into().map_err(|_| Error::Rejected)?;
                    let service = KagemushaOrdinaryLineageServiceOriginalV1::reservation(
                        bundle, prior, neutral, clock,
                    )
                    .map_err(|_| Error::Rejected)?
                    .canonical_bytes()
                    .map_err(|_| Error::Rejected)?;
                    vec![status, request, signature, service, digest]
                }
                _ => unreachable!(),
            }
        }
        7 => vec![
            cash.accept_retained_lineage_result(&originals[0], &originals[1], &originals[2])?
                .to_vec(),
        ],
        9 => cash.fence_outgoing_terminal_platform()?,
        10 => vec![
            cash.retain_outgoing_terminal_platform_original(&originals[0])?
                .to_vec(),
        ],
        11 => cash.recover_outgoing_terminal_platform()?,
        13 => {
            let mut account_error = None;
            let result = cash.sign_outgoing_commit_transport(|loan| {
                session.sign_lineage(loan).map_err(|error| {
                    account_error = Some(error);
                    KagemushaStateErrorV1::SnapshotIntegrity
                })
            });
            if let Some(error) = account_error {
                return Err(error.into());
            }
            let [status, request, signature, bundle, digest]: [Vec<u8>; 5] =
                result?.try_into().map_err(|_| Error::Rejected)?;
            let service = KagemushaOrdinaryLineageServiceOriginalV1::commit(bundle)
                .map_err(|_| Error::Rejected)?
                .canonical_bytes()
                .map_err(|_| Error::Rejected)?;
            vec![status, request, signature, service, digest]
        }
        14 => {
            cash.advance_outgoing_commit(key(&originals[0])?)?;
            vec![]
        }
        15 => {
            cash.acknowledge_outgoing_state_advance(key(&originals[0])?)?;
            vec![]
        }
        17 => vec![cash.outgoing_delivery_original(key(&originals[0])?)?],
        18 => cash.outgoing_terminal_platform_fields()?,
        _ => return Err(Error::Rejected.into()),
    };
    if phase == 8 || phase == 18 {
        fields = terminal_fields(&owner, &backend.source, fields)?;
    }
    if matches!(phase, 5 | 12) {
        session.recheck_retained_account_identity()?;
        backend
            .source
            .recheck_retained_owner_originals(&backend.path)?;
    } else {
        session.recheck()?;
        backend.source.recheck_originals(&backend.path)?;
    }
    require_fields(phase, &fields)?;
    Ok(KagemushaOrdinaryNativeOutgoingResponseV1 {
        version: 1,
        phase,
        core_handle: handle,
        fields,
    })
}
/// Invoke exact bounded canonical data against the already admitted Native Main owner.
/// # Errors
/// Returns actual Main/custody/proof errors unchanged; a decoder cannot install an owner.
pub fn invoke_kagemusha_native_ordinary_outgoing_v1(frame: &[u8]) -> Result<Vec<u8>> {
    if frame.is_empty() || frame.len() > FRAME_MAX {
        return Err(Error::Rejected.into());
    }
    let request: KagemushaOrdinaryNativeOutgoingRequestV1 =
        norito::decode_canonical_with_limits(frame, norito::canonical_decode_limits(FRAME_MAX))
            .map_err(|_| Error::Rejected)?;
    if request.version != 1
        || request.originals.len() > 3
        || norito::encode_canonical(&request).map_err(|_| Error::Rejected)? != frame
    {
        return Err(Error::Rejected.into());
    }
    let response = invoke_originals(request.phase, request.core_handle, &request.originals)?;
    norito::encode_canonical(&response).map_err(|_| Error::Rejected.into())
}
/// C entry: resets output before length/pointer admission. No policy/secret/key factory.
/// # Safety
/// Valid input/output pointers and standard bridge allocator release are required.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_ordinary_outgoing_v1(
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
        invoke_kagemusha_native_ordinary_outgoing_v1(&frame)
    })) {
        Ok(Ok(response)) => unsafe { crate::write_bytes_usize(output, output_len, &response) }
            .map_or_else(|error| error, |()| 0),
        Ok(Err(KagemushaOrdinaryOutgoingErrorV1::Coordination(Error::Unavailable))) => {
            crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1
        }
        _ => crate::ERR_KAGEMUSHA_V1,
    }
}
#[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeOutgoingV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    phase: jni::sys::jint,
    handle: jni::sys::jlong,
    originals: jni::objects::JObjectArray<'_>,
) -> jni::sys::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> core::result::Result<jni::sys::jobjectArray, String> {
            let phase = u8::try_from(phase).map_err(|_| "outgoing phase".to_owned())?;
            let handle = u64::from_ne_bytes(handle.to_ne_bytes());
            let count = env
                .get_array_length(&originals)
                .map_err(|e| e.to_string())? as usize;
            if count > 3 {
                return Err("outgoing field count".into());
            }
            let mut arrays = Vec::with_capacity(count);
            let mut lengths = Vec::with_capacity(count);
            for n in 0..count {
                let a: jni::objects::JByteArray<'_> = env
                    .get_object_array_element(&originals, n as i32)
                    .map_err(|e| e.to_string())?
                    .into();
                lengths.push(env.get_array_length(&a).map_err(|e| e.to_string())? as usize);
                arrays.push(a);
            }
            require_shape(phase, handle, &lengths).map_err(|e| format!("{e:?}"))?;
            let mut raw = Vec::with_capacity(count);
            for (a, n) in arrays.iter().zip(lengths) {
                let bytes = env.convert_byte_array(a).map_err(|e| e.to_string())?;
                if bytes.len() != n {
                    return Err("outgoing field changed".into());
                }
                raw.push(bytes);
            }
            let raw = zeroize::Zeroizing::new(raw);
            let response = invoke_originals(phase, handle, &raw).map_err(|e| format!("{e:?}"))?;
            let mut fields = vec![
                response.version.to_le_bytes().to_vec(),
                vec![response.phase],
                response.core_handle.to_le_bytes().to_vec(),
            ];
            fields.extend(response.fields);
            let output = env
                .new_object_array(fields.len() as i32, "[B", jni::objects::JObject::null())
                .map_err(|e| e.to_string())?;
            for (n, field) in fields.iter().enumerate() {
                let a = env
                    .byte_array_from_slice(field)
                    .map_err(|e| e.to_string())?;
                env.set_object_array_element(&output, n as i32, &a)
                    .map_err(|e| e.to_string())?;
            }
            Ok(output.into_raw())
        },
    ));
    match result {
        Ok(Ok(output)) => output,
        Ok(Err(cause)) => {
            let _ = env.throw_new("java/lang/IllegalStateException", cause);
            std::ptr::null_mut()
        }
        Err(_) => {
            let _ = env.throw_new(
                "java/lang/IllegalStateException",
                "Native outgoing outcome unknown",
            );
            std::ptr::null_mut()
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outgoing_closed_grammar_cannot_dispatch_prepare_or_caller_authority() {
        for phase in [1, 2, 3, 4, 19, 255] {
            assert!(require_shape(phase, 1, &[]).is_err());
        }
        for phase in [5, 6, 9, 11, 12, 13, 16, 18] {
            assert!(require_shape(phase, 1, &[]).is_ok());
            assert!(require_shape(phase, 1, &[32]).is_err());
        }
        assert!(require_shape(7, 1, &[SIGNED_MAX, DATA_MAX, AUTHORITY_MAX]).is_ok());
        assert!(require_shape(7, 1, &[1, 1, AUTHORITY_MAX + 1]).is_err());
        assert!(require_shape(10, 1, &[4097]).is_err());
        assert!(require_shape(5, 0, &[]).is_err());
    }
    #[test]
    fn outgoing_response_requires_real_complete_transport_and_distinct_ack() {
        for phase in [6, 13] {
            assert!(require_fields(phase, &[]).is_err());
        }
        assert!(require_fields(15, &[vec![1]]).is_err());
        assert!(require_fields(17, &[vec![]]).is_err());
        assert!(key(&[0; 32]).is_err());
        assert!(require_fields(9, &[vec![1], vec![], vec![]]).is_err());
    }
    #[test]
    fn outgoing_c_entry_resets_before_oversized_pointer_read() {
        let mut out = std::ptr::dangling_mut();
        let mut len = 1;
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_ordinary_outgoing_v1(
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
}
