//! Distinct descriptor-bound current FI intake, retaining the actual installed Cash owner.
//! Decoding provides data only. No current grant or cash owner can originate in a frame.
use super::*;

const SIGNED_MAX: usize = 64 * 1024;
const AUTHORITY_MAX: usize = 128 * 1024 * 1024;
const FRAME_MAX: usize = SIGNED_MAX + AUTHORITY_MAX + 1024;
const REQUEST_MAX: usize = 8192;

/// First-release current FI transport grammar, separate from bounded monetary/C19 frames.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeCurrentControlRequestV1")]
pub struct KagemushaOrdinaryNativeCurrentControlRequestV1 {
    /// Exactly one, with no legacy format fallback.
    pub version: u16,
    /// 1 reserve, 2 actual Native account signature, 3 authenticate both full reply originals.
    pub phase: u8,
    /// Exact already-opened ordinary Core descriptor.
    pub core_handle: u64,
    /// Sole complete canonical issuer-signed reply; empty for phases 1/2.
    pub signed_original: Vec<u8>,
    /// Complete independently certified World original; empty for phases 1/2.
    pub authority_original: Vec<u8>,
}
/// Data-only transport projection, never FI or monetary capability.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeCurrentControlResponseV1")]
pub struct KagemushaOrdinaryNativeCurrentControlResponseV1 {
    /// Exactly one.
    pub version: u16,
    /// Same request phase.
    pub phase: u8,
    /// Same authenticated opened Core descriptor.
    pub core_handle: u64,
    /// Phase 1: exact request and Native signing message. Phase 2: same request and retained
    /// Native account Ed64. Phase 3: empty. No caller field chooses an authority or timestamp.
    pub fields: Vec<Vec<u8>>,
}
fn require_shape(phase: u8, handle: u64, signed: usize, authority: usize) -> Result<(), Error> {
    if handle == 0
        || !matches!((phase, signed, authority), (1 | 2, 0, 0))
            && !(phase == 3
                && (1..=SIGNED_MAX).contains(&signed)
                && (1..=AUTHORITY_MAX).contains(&authority))
    {
        return Err(Error::Rejected);
    }
    Ok(())
}
fn require_response_fields(phase: u8, fields: &[Vec<u8>]) -> Result<(), Error> {
    match (phase, fields) {
        (1 | 2, [original, second]) if !original.is_empty() && original.len() <= REQUEST_MAX => {
            let request: iroha_data_model::kagemusha::KagemushaOrdinaryCurrentControlRequestV1 =
                norito::decode_canonical_with_limits(
                    original,
                    norito::canonical_decode_limits(REQUEST_MAX),
                )
                .map_err(|_| Error::Rejected)?;
            request.validate_shape().map_err(|_| Error::Rejected)?;
            if request.canonical_bytes().map_err(|_| Error::Rejected)? != *original {
                return Err(Error::Rejected);
            }
            match phase {
                1 if second
                    == &request
                        .account_signing_message()
                        .map_err(|_| Error::Rejected)? =>
                {
                    Ok(())
                }
                2 if second.len() == 64 => request
                    .verify_account_signature(&iroha_crypto::Signature::from_bytes(second))
                    .map_err(|_| Error::Rejected),
                _ => Err(Error::Rejected),
            }
        }
        (3, []) => Ok(()),
        _ => Err(Error::Rejected),
    }
}
fn invoke_originals(
    phase: u8,
    handle: u64,
    signed: &[u8],
    authority: &[u8],
) -> Result<KagemushaOrdinaryNativeCurrentControlResponseV1, Error> {
    require_shape(phase, handle, signed.len(), authority.len())?;
    let installed = INSTALL.lock().map_err(|_| Error::Rejected)?;
    if !installed.succeeded {
        return Err(Error::Unavailable);
    }
    let backend = ACTIVE.get().ok_or(Error::Unavailable)?;
    if installed.attempted_path.as_deref() != backend.path.to_str() {
        return Err(Error::Rejected);
    }
    backend.source.recheck_originals(&backend.path)?;
    drop(installed);
    let mut owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
    if owner.handle != Some(handle) {
        return Err(Error::Rejected);
    }
    let material = backend.source.cash.as_ref().ok_or(Error::Unavailable)?;
    let session = backend
        .source
        .native_account_session
        .as_ref()
        .ok_or(Error::Unavailable)?;
    session.recheck()?;
    if owner.cash.is_none() {
        if phase != 1 || owner.cash_started {
            return Err(Error::Unavailable);
        }
        // Check publication before the exclusive move. Uncertainty after the move permanently
        // closes this holder; a caller can neither request Fresh nor recreate missing journals.
        owner
            .bootstrap
            .as_ref()
            .ok_or(Error::Unavailable)?
            .publication()
            .map_err(|_| Error::Rejected)?
            .recheck()
            .map_err(|_| Error::Rejected)?;
        owner.cash_started = true;
        let initial = owner.bootstrap.take().ok_or(Error::Unavailable)?;
        owner.cash = Some(
            initial
                .into_cash_owner(
                    &material.lineage_policy_original,
                    material.disposition == KagemushaOrdinaryEnrollmentDispositionV1::Recover,
                    &material.integrity_leases,
                    &material.receivers,
                )
                .map_err(|_| Error::Rejected)?,
        );
    }
    let cash = owner.cash.as_mut().ok_or(Error::Unavailable)?;
    let fields = match phase {
        1 => cash
            .prepare_current_financial_control_read()
            .map_err(|_| Error::Rejected)?,
        2 => cash
            .sign_current_financial_control_request(|control, financial| {
                session
                    .sign_current_control(financial, control)
                    .map_err(|_| {
                        iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity
                    })
            })
            .map_err(|_| Error::Rejected)?,
        3 => {
            cash.accept_current_financial_control_read(signed, authority)
                .map_err(|_| Error::Rejected)?;
            Vec::new()
        }
        _ => return Err(Error::Rejected),
    };
    // Session revocation during authenticating/signing/fsync prevents any transport exposure.
    session.recheck()?;
    backend.source.recheck_originals(&backend.path)?;
    require_response_fields(phase, &fields)?;
    Ok(KagemushaOrdinaryNativeCurrentControlResponseV1 {
        version: 1,
        phase,
        core_handle: handle,
        fields,
    })
}
/// Authenticate the closed first-release frame and dispatch only to the actual installed Cash.
/// # Errors
/// Refuses malformed framing, missing Native material, closed/foreign descriptor, stale
/// Native session/FI/clock/PI or substituted/uncertain complete originals.
pub fn invoke_kagemusha_native_ordinary_current_control_v1(frame: &[u8]) -> Result<Vec<u8>, Error> {
    if frame.is_empty() || frame.len() > FRAME_MAX {
        return Err(Error::Rejected);
    }
    let request: KagemushaOrdinaryNativeCurrentControlRequestV1 =
        norito::decode_canonical_with_limits(frame, norito::canonical_decode_limits(FRAME_MAX))
            .map_err(|_| Error::Rejected)?;
    if request.version != 1
        || norito::encode_canonical(&request).map_err(|_| Error::Rejected)? != frame
    {
        return Err(Error::Rejected);
    }
    let response = invoke_originals(
        request.phase,
        request.core_handle,
        &request.signed_original,
        &request.authority_original,
    )?;
    norito::encode_canonical(&response).map_err(|_| Error::Rejected)
}
/// Dedicated first-release C entry, outside the small coordinator frame maximum.
/// All output is reset before bounds/pointer validation and allocated only after actual admission.
/// # Safety
/// Nonempty originals must be readable for the supplied lengths. Output pointers must be
/// nonnull, aligned, writable, and released with the standard bridge allocator after success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_ordinary_current_control_v1(
    phase: u8,
    core_handle: u64,
    signed_ptr: *const u8,
    signed_len: usize,
    authority_ptr: *const u8,
    authority_len: usize,
    output_ptr: *mut *mut u8,
    output_len: *mut usize,
) -> std::ffi::c_int {
    if output_ptr.is_null() || output_len.is_null() {
        return crate::ERR_NULL_PTR;
    }
    unsafe {
        *output_ptr = std::ptr::null_mut();
        *output_len = 0;
    }
    if require_shape(phase, core_handle, signed_len, authority_len).is_err()
        || (signed_len != 0 && signed_ptr.is_null())
        || (authority_len != 0 && authority_ptr.is_null())
    {
        return crate::ERR_KAGEMUSHA_V1;
    }
    let signed = zeroize::Zeroizing::new(if signed_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(signed_ptr, signed_len) }.to_vec()
    });
    let authority = zeroize::Zeroizing::new(if authority_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(authority_ptr, authority_len) }.to_vec()
    });
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let response = invoke_originals(phase, core_handle, &signed, &authority)?;
        norito::encode_canonical(&response).map_err(|_| Error::Rejected)
    })) {
        Ok(Ok(response)) => unsafe { crate::write_bytes_usize(output_ptr, output_len, &response) }
            .map_or_else(|error| error, |()| 0),
        Ok(Err(Error::Unavailable)) => crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1,
        Ok(Err(Error::Rejected)) | Err(_) => crate::ERR_KAGEMUSHA_V1,
    }
}
#[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeCurrentControlV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    phase: jni::sys::jint,
    handle: jni::sys::jlong,
    signed: jni::objects::JByteArray<'_>,
    authority: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<jni::sys::jobjectArray, ()> {
            let phase = u8::try_from(phase).map_err(|_| ())?;
            let handle = u64::from_ne_bytes(handle.to_ne_bytes());
            let signed_len =
                usize::try_from(env.get_array_length(&signed).map_err(|_| ())?).map_err(|_| ())?;
            let authority_len = usize::try_from(env.get_array_length(&authority).map_err(|_| ())?)
                .map_err(|_| ())?;
            require_shape(phase, handle, signed_len, authority_len).map_err(|_| ())?;
            let signed = zeroize::Zeroizing::new(env.convert_byte_array(&signed).map_err(|_| ())?);
            let authority =
                zeroize::Zeroizing::new(env.convert_byte_array(&authority).map_err(|_| ())?);
            if signed.len() != signed_len || authority.len() != authority_len {
                return Err(());
            }
            let response = invoke_originals(phase, handle, &signed, &authority).map_err(|_| ())?;
            let mut fields = vec![
                response.version.to_le_bytes().to_vec(),
                vec![response.phase],
                response.core_handle.to_le_bytes().to_vec(),
            ];
            fields.extend(response.fields);
            let output = env
                .new_object_array(
                    fields.len() as jni::sys::jsize,
                    "[B",
                    jni::objects::JObject::null(),
                )
                .map_err(|_| ())?;
            for (index, field) in fields.iter().enumerate() {
                let array = env.byte_array_from_slice(field).map_err(|_| ())?;
                env.set_object_array_element(&output, index as jni::sys::jsize, &array)
                    .map_err(|_| ())?;
            }
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
    fn separate_current_control_exact_lifecycle_and_full_original_bounds() {
        for phase in [1, 2] {
            assert!(require_shape(phase, 1, 0, 0).is_ok());
            assert!(require_shape(phase, 1, 1, 0).is_err());
            assert!(require_shape(phase, 1, 0, 1).is_err());
        }
        assert!(require_shape(3, 1, SIGNED_MAX, AUTHORITY_MAX).is_ok());
        for args in [
            (0, 1, 0, 0),
            (4, 1, 0, 0),
            (1, 0, 0, 0),
            (3, 1, 0, 1),
            (3, 1, 1, 0),
            (3, 1, SIGNED_MAX + 1, 1),
            (3, 1, 1, AUTHORITY_MAX + 1),
            (3, 1, usize::MAX, usize::MAX),
        ] {
            assert!(require_shape(args.0, args.1, args.2, args.3).is_err());
        }
    }
    #[test]
    fn separate_current_control_c_entry_resets_outputs_before_refusal_without_reading_oversized_pointer()
     {
        let mut out = std::ptr::dangling_mut::<u8>();
        let mut len = 99;
        let error = unsafe {
            connect_norito_kagemusha_ordinary_current_control_v1(
                3,
                1,
                std::ptr::dangling(),
                SIGNED_MAX + 1,
                std::ptr::dangling(),
                1,
                &mut out,
                &mut len,
            )
        };
        assert_eq!(error, crate::ERR_KAGEMUSHA_V1);
        assert!(out.is_null());
        assert_eq!(len, 0);
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_ordinary_current_control_v1(
                    3,
                    1,
                    std::ptr::null(),
                    1,
                    std::ptr::null(),
                    1,
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
    fn separate_current_control_decoder_never_creates_native_installation() {
        let request = KagemushaOrdinaryNativeCurrentControlRequestV1 {
            version: 2,
            phase: 1,
            core_handle: 1,
            signed_original: Vec::new(),
            authority_original: Vec::new(),
        };
        let frame = norito::encode_canonical(&request).unwrap();
        assert!(invoke_kagemusha_native_ordinary_current_control_v1(&frame).is_err());
        assert!(invoke_kagemusha_native_ordinary_current_control_v1(&[]).is_err());
        assert!(require_response_fields(3, &[vec![1]]).is_err());
        assert!(require_response_fields(1, &[vec![1], vec![2]]).is_err());
    }
}
