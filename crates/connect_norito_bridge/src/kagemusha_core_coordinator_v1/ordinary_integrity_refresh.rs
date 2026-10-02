//! Descriptor-bound PI recovery/refresh; historical enrollment cannot lend money or renew PI.
use super::*;
const FRAME_MAX: usize = 192 * 1024;
const CHALLENGE_MAX: usize = 8192;
/// Sole first-release Native refresh frame; no caller clock, scope, issuer, root or disposition.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeIntegrityRefreshRequestV1")]
pub struct KagemushaOrdinaryNativeIntegrityRefreshRequestV1 {
    /// Exactly one.
    pub version: u16,
    /// 1 prepare; 2 signed challenge; 3 invocation fence; 4 DER capture; 5 PI token;
    /// 6 signed lease; 7 abandon old nonce; 8 exact retained-original recovery; 9 Bootstrap activation; 10 completed key-only metadata.
    pub phase: u8,
    /// Exact currently opened actual ordinary Native descriptor.
    pub core_handle: u64,
    /// Empty for 1/3/7/8/9/10, sole complete original for 2/4/5/6.
    pub original: Vec<u8>,
}
/// Data-only original transport projection; no decoded response creates a PI or FI capability.
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::KagemushaOrdinaryNativeIntegrityRefreshResponseV1")]
pub struct KagemushaOrdinaryNativeIntegrityRefreshResponseV1 {
    /// Exactly one.
    pub version: u16,
    /// Same request phase.
    pub phase: u8,
    /// Same owned descriptor.
    pub core_handle: u64,
    /// Exact bounded originals from actual Native custody.
    pub fields: Vec<Vec<u8>>,
}
fn require_shape(phase: u8, handle: u64, raw: &[u8]) -> Result<(), Error> {
    if handle == 0 {
        return Err(Error::Rejected);
    }
    let valid = match phase {
        1 | 3 | 7 | 8 | 9 | 10 => raw.is_empty(),
        2 => !raw.is_empty() && raw.len() <= CHALLENGE_MAX,
        4 => (8..=72).contains(&raw.len()),
        5 => !raw.is_empty() && raw.len() <= 64 * 1024,
        6 => !raw.is_empty() && raw.len() <= 4096,
        _ => false,
    };
    if valid { Ok(()) } else { Err(Error::Rejected) }
}
fn protocol(
    refresh: &mut IntegrityRefresh,
    financial: &FinancialOwner,
    phase: u8,
    raw: &[u8],
) -> Result<
    (
        Vec<Vec<u8>>,
        Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    ),
    iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryIdentityErrorV1,
> {
    match phase {
        1 => Ok((refresh.prepare(financial)?, None)),
        2 => Ok((refresh.accept_challenge(financial, raw)?, None)),
        3 => Ok((refresh.fence_platform_invocation(financial)?, None)),
        4 => {
            refresh.capture_signature(financial, raw)?;
            Ok((vec![], None))
        }
        5 => Ok((refresh.retain_integrity_token(financial, raw)?, None)),
        6 => {
            let lease = refresh.accept_lease(financial, raw)?;
            Ok((vec![lease.digest().to_vec()], Some(lease)))
        }
        7 => {
            refresh.abandon_pending(financial)?;
            Ok((vec![], None))
        }
        8 => Ok((refresh.recovery_fields(financial)?, None)),
        _ => Err(iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryIdentityErrorV1::Rejected),
    }
}
fn with_refresh<T>(
    owner: &mut Owner,
    root: &Path,
    create_if_absent: bool,
    consume: impl FnOnce(
        &mut IntegrityRefresh,
        &FinancialOwner,
    ) -> Result<
        T,
        iroha_core_zk::kagemusha_v1_state::KagemushaOrdinaryIdentityErrorV1,
    >,
) -> Result<T, Error> {
    if let Some(recovered) = &mut owner.integrity_recovery {
        return recovered
            .with_integrity_refresh_owner(consume)
            .map_err(|_| Error::Rejected);
    }
    let Owner {
        integrity_refresh,
        integrity_refresh_started,
        cash,
        bootstrap,
        ..
    } = owner;
    let invoke = |financial: &FinancialOwner| {
        if integrity_refresh.is_none() {
            if !create_if_absent {
                return Err(
                    iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::InvalidCandidateStage,
                );
            }
            if *integrity_refresh_started {
                return Err(
                    iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity,
                );
            }
            *integrity_refresh_started = true;
            *integrity_refresh = Some(IntegrityRefresh::create(root, financial).map_err(|_| {
                iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity
            })?);
        }
        consume(
            integrity_refresh.as_mut().ok_or(
                iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity,
            )?,
            financial,
        )
        .map_err(|_| iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity)
    };
    if let Some(cash) = cash {
        cash.with_integrity_refresh_custody(invoke)
            .map_err(|_| Error::Rejected)
    } else {
        bootstrap
            .as_mut()
            .ok_or(Error::Unavailable)?
            .with_integrity_refresh_custody(invoke)
            .map_err(|_| Error::Rejected)
    }
}
/// Invoke only the independently installed actual Native PI owner. Recovery preserves old
/// C/FI/token/nonce originals. A pending or expired lease cannot become an acknowledged grant.
/// # Errors
/// Refuses foreign descriptor, changed originals/storage, unknown invocation or invalid signature.
pub fn invoke_kagemusha_native_ordinary_integrity_refresh_v1(
    frame: &[u8],
) -> Result<Vec<u8>, Error> {
    if frame.is_empty() || frame.len() > FRAME_MAX {
        return Err(Error::Rejected);
    }
    let request: KagemushaOrdinaryNativeIntegrityRefreshRequestV1 =
        norito::decode_canonical_with_limits(frame, norito::canonical_decode_limits(FRAME_MAX))
            .map_err(|_| Error::Rejected)?;
    if request.version != 1
        || norito::encode_canonical(&request).map_err(|_| Error::Rejected)? != frame
    {
        return Err(Error::Rejected);
    }
    require_shape(request.phase, request.core_handle, &request.original)?;
    let installed = INSTALL.lock().map_err(|_| Error::Rejected)?;
    if !installed.succeeded {
        return Err(Error::Unavailable);
    }
    let backend = ACTIVE.get().ok_or(Error::Unavailable)?;
    if installed.attempted_path.as_deref() != backend.path.to_str() {
        return Err(Error::Rejected);
    }
    if request.phase == 10 {
        backend
            .source
            .recheck_retained_owner_originals(&backend.path)?;
    } else {
        backend.source.recheck_originals(&backend.path)?;
    }
    drop(installed);
    let mut owner = backend.owner.lock().map_err(|_| Error::Rejected)?;
    if owner.handle != Some(request.core_handle) {
        return Err(Error::Rejected);
    }
    if request.phase == 10 {
        let mut fields = if let Some(recovered) = &owner.integrity_recovery {
            recovered
                .completed_app_key_fields()
                .map_err(|_| Error::Rejected)?
        } else {
            let read = |financial: &FinancialOwner| {
                financial.retained_completed_app_key_fields().map_err(|_| {
                    iroha_core_zk::kagemusha_v1_state::KagemushaStateErrorV1::SnapshotIntegrity
                })
            };
            if let Some(cash) = &mut owner.cash {
                cash.with_integrity_refresh_custody(read)
                    .map_err(|_| Error::Rejected)?
            } else {
                owner
                    .bootstrap
                    .as_mut()
                    .ok_or(Error::Unavailable)?
                    .with_integrity_refresh_custody(read)
                    .map_err(|_| Error::Rejected)?
            }
        };
        if fields.len() != 9 {
            return Err(Error::Rejected);
        }
        let digest = fields.pop().ok_or(Error::Rejected)?;
        let policy = backend.source.integrity_policy_original.as_deref();
        let (original, project) = match (fields[5].as_slice(), policy) {
            ([1] | [2] | [3], Some(raw)) => {
                let p = iroha_data_model::kagemusha::kagemusha_play_integrity_provider_policy_projection_v1(raw)
                    .map_err(|_| Error::Rejected)?;
                (raw.to_vec(), p.cloud_project_number)
            }
            ([0], None) => (vec![], 0),
            _ => return Err(Error::Rejected),
        };
        fields.push(original);
        fields.push(project.to_le_bytes().to_vec());
        fields.push(digest);
        return encode_response(&request, fields, backend);
    }
    if let Some(recovered) = &mut owner.integrity_recovery {
        if request.phase == 8 {
            let fields = recovered.recovery_fields().map_err(|_| Error::Rejected)?;
            return encode_response(&request, fields, backend);
        }
        if request.phase == 1 {
            let fields = recovered
                .prepare_integrity_refresh()
                .map_err(|_| Error::Rejected)?;
            return encode_response(&request, fields, backend);
        }
    }
    let fields = if request.phase == 9 {
        vec![vec![u8::from(
            backend.activate_recovered_bootstrap(&mut owner)?,
        )]]
    } else {
        let (fields, lease) = with_refresh(
            &mut owner,
            &backend.path,
            request.phase == 1,
            |refresh, financial| protocol(refresh, financial, request.phase, &request.original),
        )?;
        if let Some(lease) = lease {
            if let Some(cash) = &mut owner.cash {
                cash.accept_integrity_lease(Arc::clone(&lease))
                    .map_err(|_| Error::Rejected)?;
            } else if let Some(initial) = &mut owner.bootstrap {
                initial
                    .accept_integrity_lease(Arc::clone(&lease))
                    .map_err(|_| Error::Rejected)?;
            } else {
                backend.activate_recovered_bootstrap(&mut owner)?;
            }
            owner.integrity_catalog =
                with_refresh(&mut owner, &backend.path, false, |refresh, financial| {
                    refresh.retained_verified_leases(financial)
                })?;
        }
        fields
    };
    encode_response(&request, fields, backend)
}
fn encode_response(
    request: &KagemushaOrdinaryNativeIntegrityRefreshRequestV1,
    fields: Vec<Vec<u8>>,
    backend: &OrdinaryBackend,
) -> Result<Vec<u8>, Error> {
    if fields.len() > if request.phase == 10 { 11 } else { 8 }
        || fields.iter().any(|raw| raw.len() > 64 * 1024)
        || fields.iter().map(Vec::len).sum::<usize>() > 128 * 1024
    {
        return Err(Error::Rejected);
    }
    if request.phase == 10 {
        backend
            .source
            .recheck_retained_owner_originals(&backend.path)?;
    } else {
        backend.source.recheck_originals(&backend.path)?;
    }
    let frame = norito::encode_canonical(&KagemushaOrdinaryNativeIntegrityRefreshResponseV1 {
        version: 1,
        phase: request.phase,
        core_handle: request.core_handle,
        fields,
    })
    .map_err(|_| Error::Rejected)?;
    if frame.len() > FRAME_MAX {
        return Err(Error::Rejected);
    }
    Ok(frame)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refresh_contract_has_no_authority_or_clock_selector_and_exact_original_caps() {
        for phase in [1, 3, 7, 8, 9, 10] {
            assert!(require_shape(phase, 1, &[]).is_ok());
            assert!(require_shape(phase, 1, &[1]).is_err());
        }
        for (phase, cap) in [(2, CHALLENGE_MAX), (4, 72), (5, 64 * 1024), (6, 4096)] {
            assert!(require_shape(phase, 1, &vec![1; cap]).is_ok());
            assert!(require_shape(phase, 1, &vec![1; cap + 1]).is_err());
            assert!(require_shape(phase, 0, &vec![1; cap]).is_err());
        }
        assert!(require_shape(0, 1, &[]).is_err());
        assert!(require_shape(11, 1, &[]).is_err());
        assert!(require_shape(4, 1, &[1; 7]).is_err());
    }
}
/// Dedicated C transport boundary. Inputs carry bounded originals only; the actual Native
/// descriptor and retained protocol owner select all scope, keys, clocks and journal effects.
/// # Safety
/// The nonempty original must be readable; output pointers must be aligned and writable and
/// the successful allocation released with the maintained standard bridge allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_ordinary_integrity_refresh_v1(
    phase: u8,
    handle: u64,
    original_ptr: *const u8,
    original_len: usize,
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
    if original_len > 64 * 1024 || (original_len != 0 && original_ptr.is_null()) {
        return crate::ERR_KAGEMUSHA_V1;
    }
    let raw = zeroize::Zeroizing::new(if original_len == 0 {
        vec![]
    } else {
        unsafe { std::slice::from_raw_parts(original_ptr, original_len) }.to_vec()
    });
    if require_shape(phase, handle, &raw).is_err() {
        return crate::ERR_KAGEMUSHA_V1;
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let frame = norito::encode_canonical(&KagemushaOrdinaryNativeIntegrityRefreshRequestV1 {
            version: 1,
            phase,
            core_handle: handle,
            original: raw.to_vec(),
        })
        .map_err(|_| Error::Rejected)?;
        invoke_kagemusha_native_ordinary_integrity_refresh_v1(&frame)
    })) {
        Ok(Ok(response)) => unsafe { crate::write_bytes_usize(output_ptr, output_len, &response) }
            .map_or_else(|error| error, |()| 0),
        Ok(Err(Error::Unavailable)) => crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1,
        Ok(Err(Error::Rejected)) | Err(_) => crate::ERR_KAGEMUSHA_V1,
    }
}
#[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaOrdinaryRuntimeJniV1_nativeIntegrityRefreshV1<
    'local,
>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    phase: jni::sys::jint,
    handle: jni::sys::jlong,
    original: jni::objects::JByteArray<'local>,
) -> jni::sys::jobjectArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<jni::sys::jobjectArray, ()> {
            #[cfg(any(target_os = "android", test))]
            super::super::ordinary_android_installed_context::AndroidOrdinaryInstalledContextOwnerV1::require_early_retained_jni_class(&mut env, &_class).map_err(|_| ())?;
            let phase = u8::try_from(phase).map_err(|_| ())?;
            let handle = u64::from_ne_bytes(handle.to_ne_bytes());
            let length = usize::try_from(env.get_array_length(&original).map_err(|_| ())?)
                .map_err(|_| ())?;
            if length > 64 * 1024 {
                return Err(());
            }
            let raw = zeroize::Zeroizing::new(env.convert_byte_array(&original).map_err(|_| ())?);
            if raw.len() != length {
                return Err(());
            }
            require_shape(phase, handle, &raw).map_err(|_| ())?;
            let frame =
                norito::encode_canonical(&KagemushaOrdinaryNativeIntegrityRefreshRequestV1 {
                    version: 1,
                    phase,
                    core_handle: handle,
                    original: raw.to_vec(),
                })
                .map_err(|_| ())?;
            let response =
                invoke_kagemusha_native_ordinary_integrity_refresh_v1(&frame).map_err(|_| ())?;
            let response: KagemushaOrdinaryNativeIntegrityRefreshResponseV1 =
                norito::decode_canonical_with_limits(
                    &response,
                    norito::canonical_decode_limits(FRAME_MAX),
                )
                .map_err(|_| ())?;
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
