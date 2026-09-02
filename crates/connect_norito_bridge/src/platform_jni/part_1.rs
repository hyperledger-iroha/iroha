#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeCudaAvailable(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    use std::panic::catch_unwind;
    let available = catch_unwind(ivm::cuda_available).unwrap_or(false);
    if available { JNI_TRUE } else { JNI_FALSE }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeCudaDisabled(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    use std::panic::catch_unwind;
    let disabled = catch_unwind(ivm::cuda_disabled).unwrap_or(false);
    if disabled { JNI_TRUE } else { JNI_FALSE }
}
pub(super) fn throw_java_illegal_argument(env: &mut jni::JNIEnv<'_>, message: String) {
    let _ = env.throw_new("java/lang/IllegalArgumentException", message);
}
pub(super) fn throw_java_illegal_state(env: &mut jni::JNIEnv<'_>, message: String) {
    let _ = env.throw_new("java/lang/IllegalStateException", message);
}
pub(super) fn catch_unwind_to_java<T, F>(env: &mut jni::JNIEnv<'_>, label: &str, f: F) -> Option<T>
where
    F: FnOnce() -> T,
{
    use std::panic::{AssertUnwindSafe, catch_unwind};
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => Some(value),
        Err(_) => {
            throw_java_illegal_state(env, format!("{label} panicked"));
            None
        }
    }
}
pub(super) fn read_java_byte_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JByteArray<'_>,
    context: &str,
) -> Option<Vec<u8>> {
    let _len = match env.get_array_length(array) {
        Ok(value) => value,
        Err(err) => {
            throw_java_illegal_argument(
                env,
                format!("{context} failed to read array length: {err}"),
            );
            return None;
        }
    } as usize;
    env.convert_byte_array(array).map_or_else(
        |err| {
            throw_java_illegal_state(
                env,
                format!("{context} failed to read array contents: {err}"),
            );
            None
        },
        Some,
    )
}
pub(super) fn read_java_byte_array_bounded(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JByteArray<'_>,
    context: &str,
    maximum: usize,
) -> Option<Vec<u8>> {
    let len = match env.get_array_length(array) {
        Ok(value) => usize::try_from(value).ok(),
        Err(err) => {
            throw_java_illegal_argument(
                env,
                format!("{context} failed to read array length: {err}"),
            );
            return None;
        }
    }?;
    if kagemusha_archive_out_of_bounds_for(len, maximum) {
        throw_java_illegal_argument(env, format!("{context} must contain 1..{maximum} bytes"));
        return None;
    }
    env.convert_byte_array(array).map_or_else(
        |err| {
            throw_java_illegal_state(
                env,
                format!("{context} failed to read array contents: {err}"),
            );
            None
        },
        Some,
    )
}
pub(super) fn java_validation_fee_policy_proof_result(
    env: &mut jni::JNIEnv<'_>,
    body: impl FnOnce(&mut jni::JNIEnv<'_>) -> Result<Vec<u8>, String>,
) -> jni::sys::jbyteArray {
    match body(env).and_then(|bytes| {
        env.byte_array_from_slice(&bytes)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    }) {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, format!("validation-fee consensus proof: {message}"));
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_validation_fee_current_policy_proof_request_v1(
    env: &mut jni::JNIEnv<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_validation_fee_policy_proof_result(env, |env| {
        let trusted_checkpoint_height = u64::try_from(trusted_checkpoint_height)
            .ok()
            .filter(|height| *height != 0)
            .ok_or_else(|| "trustedCheckpointHeight must be positive".to_owned())?;
        let trusted_checkpoint_context_id: [u8; 32] = read_java_byte_array_bounded(
            env,
            &trusted_checkpoint_context_id,
            "trustedCheckpointContextId",
            32,
        )
        .ok_or_else(|| "trustedCheckpointContextId must contain exactly 32 bytes".to_owned())?
        .try_into()
        .map_err(|_| "trustedCheckpointContextId must contain exactly 32 bytes".to_owned())?;
        validation_fee_current_policy_proof_request_v1(
            trusted_checkpoint_height,
            trusted_checkpoint_context_id,
        )
        .map_err(|_| "trusted checkpoint was rejected".to_owned())
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_validation_fee_current_policy_proof_verify_v1(
    env: &mut jni::JNIEnv<'_>,
    proof_norito: jni::objects::JByteArray<'_>,
    network_id: jni::objects::JByteArray<'_>,
    policy_chain_genesis_hash: jni::objects::JByteArray<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_validation_fee_policy_proof_result(env, |env| {
        let proof = read_java_byte_array_bounded(
            env,
            &proof_norito,
            "proofNorito",
            VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES,
        )
        .ok_or_else(|| "proofNorito must be a bounded nonempty archive".to_owned())?;
        let network_id: [u8; 32] = read_java_byte_array_bounded(env, &network_id, "networkId", 32)
            .ok_or_else(|| "networkId must contain exactly 32 bytes".to_owned())?
            .try_into()
            .map_err(|_| "networkId must contain exactly 32 bytes".to_owned())?;
        let network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed(network_id)),
        );
        let policy_chain_genesis_hash: [u8; 32] = read_java_byte_array_bounded(
            env,
            &policy_chain_genesis_hash,
            "policyChainGenesisHash",
            32,
        )
        .ok_or_else(|| "policyChainGenesisHash must contain exactly 32 bytes".to_owned())?
        .try_into()
        .map_err(|_| "policyChainGenesisHash must contain exactly 32 bytes".to_owned())?;
        let trusted_checkpoint_height = u64::try_from(trusted_checkpoint_height)
            .ok()
            .filter(|height| *height != 0)
            .ok_or_else(|| "trustedCheckpointHeight must be positive".to_owned())?;
        let trusted_checkpoint_context_id: [u8; 32] = read_java_byte_array_bounded(
            env,
            &trusted_checkpoint_context_id,
            "trustedCheckpointContextId",
            32,
        )
        .ok_or_else(|| "trustedCheckpointContextId must contain exactly 32 bytes".to_owned())?
        .try_into()
        .map_err(|_| "trustedCheckpointContextId must contain exactly 32 bytes".to_owned())?;
        validation_fee_current_policy_proof_verify_v1(
            &proof,
            network_id,
            policy_chain_genesis_hash,
            trusted_checkpoint_height,
            trusted_checkpoint_context_id,
        )
        .map_err(|_| {
            "proof, finality, registry, or immutable deployment binding was rejected".to_owned()
        })
    })
}
/// Report the exact native ABI required by the validation-fee proof bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_validationfee_ValidationFeeConsensusProofBridge_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}
/// JNI projection of
/// [`connect_norito_validation_fee_current_policy_proof_request_v1`].
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_validationfee_ValidationFeeConsensusProofBridge_nativeEncodeCurrentPolicyProofRequestV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_validation_fee_current_policy_proof_request_v1(
        &mut env,
        trusted_checkpoint_height,
        trusted_checkpoint_context_id,
    )
}
/// JNI projection of
/// [`connect_norito_validation_fee_current_policy_proof_verify_v1`].
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_validationfee_ValidationFeeConsensusProofBridge_nativeVerifyCurrentPolicyProofV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    proof_norito: jni::objects::JByteArray<'_>,
    network_id: jni::objects::JByteArray<'_>,
    policy_chain_genesis_hash: jni::objects::JByteArray<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_validation_fee_current_policy_proof_verify_v1(
        &mut env,
        proof_norito,
        network_id,
        policy_chain_genesis_hash,
        trusted_checkpoint_height,
        trusted_checkpoint_context_id,
    )
}
pub(super) fn java_validation_fee_hijiri_quote_result(
    env: &mut jni::JNIEnv<'_>,
    body: impl FnOnce(&mut jni::JNIEnv<'_>) -> Result<Vec<u8>, String>,
) -> jni::sys::jbyteArray {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        body(env).and_then(|bytes| {
            env.byte_array_from_slice(&bytes)
                .map(jni::objects::JByteArray::into_raw)
                .map_err(|error| error.to_string())
        })
    }));
    match result {
        Ok(Ok(array)) => array,
        Ok(Err(message)) => {
            throw_java_illegal_argument(env, format!("Hijiri validation-fee quote: {message}"));
            std::ptr::null_mut()
        }
        Err(_) => {
            throw_java_illegal_state(env, "Hijiri validation-fee quote panicked".to_owned());
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_validation_fee_hijiri_quote_request_v1(
    env: &mut jni::JNIEnv<'_>,
    account_id_utf8: jni::objects::JByteArray<'_>,
    qualifying_transfer_count: jni::sys::jint,
) -> jni::sys::jbyteArray {
    java_validation_fee_hijiri_quote_result(env, |env| {
        let account_id_bytes = read_java_byte_array_bounded(
            env,
            &account_id_utf8,
            "accountIdUtf8",
            VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1,
        )
        .ok_or_else(|| {
            "accountIdUtf8 must contain one bounded canonical I105 literal".to_owned()
        })?;
        let account_id = std::str::from_utf8(&account_id_bytes)
            .map_err(|_| "accountIdUtf8 must be valid UTF-8".to_owned())?;
        let qualifying_transfer_count = u32::try_from(qualifying_transfer_count)
            .map_err(|_| "qualifyingTransferCount must be in 1..100000".to_owned())?;
        validation_fee_hijiri_quote_request_v1(account_id, qualifying_transfer_count)
            .map_err(|_| "request account or transfer count was rejected".to_owned())
    })
}
pub(super) fn java_native_validation_fee_hijiri_quote_response_verify_v1(
    env: &mut jni::JNIEnv<'_>,
    response_norito: jni::objects::JByteArray<'_>,
    request_norito: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_validation_fee_hijiri_quote_result(env, |env| {
        let response = read_java_byte_array_bounded(
            env,
            &response_norito,
            "responseNorito",
            VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1,
        )
        .ok_or_else(|| "responseNorito must be one bounded nonempty archive".to_owned())?;
        let request = read_java_byte_array_bounded(
            env,
            &request_norito,
            "requestNorito",
            VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1,
        )
        .ok_or_else(|| "requestNorito must be one bounded nonempty archive".to_owned())?;
        validation_fee_hijiri_quote_response_verify_v1(&response, &request).map_err(|_| {
            "response archive, request archive, or exact request binding was rejected".to_owned()
        })
    })
}
/// Report the exact native ABI required by the Kotlin Hijiri quote bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_validationfee_ValidationFeeHijiriQuoteBridge_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}
/// JNI projection of [`connect_norito_validation_fee_hijiri_quote_request_v1`].
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_validationfee_ValidationFeeHijiriQuoteBridge_nativeEncodeRequestV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    account_id_utf8: jni::objects::JByteArray<'_>,
    qualifying_transfer_count: jni::sys::jint,
) -> jni::sys::jbyteArray {
    java_native_validation_fee_hijiri_quote_request_v1(
        &mut env,
        account_id_utf8,
        qualifying_transfer_count,
    )
}
/// JNI projection of [`connect_norito_validation_fee_hijiri_quote_response_verify_v1`].
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_validationfee_ValidationFeeHijiriQuoteBridge_nativeVerifyResponseV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    response_norito: jni::objects::JByteArray<'_>,
    request_norito: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_validation_fee_hijiri_quote_response_verify_v1(
        &mut env,
        response_norito,
        request_norito,
    )
}
/// Report the exact native ABI required by the Java Hijiri quote bridge.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_validationfee_ValidationFeeHijiriQuoteBridge_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}
/// Java SDK projection of [`connect_norito_validation_fee_hijiri_quote_request_v1`].
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_validationfee_ValidationFeeHijiriQuoteBridge_nativeEncodeRequestV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    account_id_utf8: jni::objects::JByteArray<'_>,
    qualifying_transfer_count: jni::sys::jint,
) -> jni::sys::jbyteArray {
    java_native_validation_fee_hijiri_quote_request_v1(
        &mut env,
        account_id_utf8,
        qualifying_transfer_count,
    )
}
/// Java SDK projection of [`connect_norito_validation_fee_hijiri_quote_response_verify_v1`].
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_validationfee_ValidationFeeHijiriQuoteBridge_nativeVerifyResponseV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    response_norito: jni::objects::JByteArray<'_>,
    request_norito: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_validation_fee_hijiri_quote_response_verify_v1(
        &mut env,
        response_norito,
        request_norito,
    )
}
pub(super) fn java_sorafs_reference_generated_at(
    generated_at: jni::sys::jlong,
) -> Result<u64, String> {
    u64::try_from(generated_at).map_err(|_| "generatedAtUnix must be non-negative".to_owned())
}
pub(super) fn read_java_sorafs_reference_byte_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JByteArray<'_>,
    field: &str,
    maximum: usize,
) -> Result<Vec<u8>, String> {
    let length = env
        .get_array_length(array)
        .map_err(|error| format!("failed to read {field} length: {error}"))?;
    let length = usize::try_from(length).map_err(|_| format!("{field} length is invalid"))?;
    if length > maximum {
        return Err(format!("{field} must be at most {maximum} bytes"));
    }
    read_java_byte_array(env, array, field)
        .ok_or_else(|| format!("failed to read {field} contents"))
}
pub(super) fn read_java_sorafs_reference_byte_array_vector(
    env: &mut jni::JNIEnv<'_>,
    values: &jni::objects::JObjectArray<'_>,
    field: &str,
    expected_count: Option<usize>,
    maximum_count: usize,
    maximum_bytes: usize,
) -> Result<Vec<Vec<u8>>, String> {
    let count = env
        .get_array_length(values)
        .map_err(|error| format!("failed to read {field} count: {error}"))?;
    let count = usize::try_from(count).map_err(|_| format!("{field} count is invalid"))?;
    if count == 0 || count > maximum_count {
        return Err(format!("{field} must contain 1..{maximum_count} entries"));
    }
    if let Some(expected_count) = expected_count
        && count != expected_count
    {
        return Err(format!(
            "{field} must contain exactly {expected_count} entries"
        ));
    }
    let mut result = Vec::with_capacity(count);
    for index in 0..count {
        let object = env
            .get_object_array_element(
                values,
                i32::try_from(index).map_err(|_| format!("{field} count is invalid"))?,
            )
            .map_err(|error| format!("failed to read {field}[{index}]: {error}"))?;
        if object.is_null() {
            return Err(format!("{field}[{index}] must be a byte array"));
        }
        let array = jni::objects::JByteArray::from(object);
        result.push(read_java_sorafs_reference_byte_array(
            env,
            &array,
            &format!("{field}[{index}]"),
            maximum_bytes,
        )?);
    }
    Ok(result)
}
pub(super) unsafe fn java_sorafs_reference_buffer_bytes(
    buffer: sorafs_reference_ffi::SorafsReferenceFfiBuffer,
) -> Vec<u8> {
    let bytes = if buffer.ptr.is_null() || buffer.len == 0 {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(buffer.ptr.cast_const(), buffer.len) }.to_vec()
    };
    unsafe { sorafs_reference_ffi::sorafs_reference_free_buffer(buffer) };
    bytes
}
pub(super) unsafe fn java_sorafs_reference_buffer_to_array(
    env: &mut jni::JNIEnv<'_>,
    buffer: sorafs_reference_ffi::SorafsReferenceFfiBuffer,
    context: &str,
) -> Result<jni::sys::jbyteArray, String> {
    let bytes = unsafe { java_sorafs_reference_buffer_bytes(buffer) };
    if bytes.is_empty() {
        return Err(format!("{context} returned empty outcome JSON"));
    }
    env.byte_array_from_slice(&bytes)
        .map(|array| array.into_raw())
        .map_err(|err| err.to_string())
}
pub(super) fn java_sorafs_reference_validate_orderbook_payload_json(
    env: &mut jni::JNIEnv<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let payload_bytes = read_java_byte_array(env, &payload, "noritoBytes")
            .ok_or_else(|| "invalid orderbook payload bytes".to_owned())?;
        let label_bytes = read_java_byte_array(env, &label, "label")
            .ok_or_else(|| "invalid orderbook label bytes".to_owned())?;
        let kind = u32::try_from(kind).map_err(|_| "kind must be non-negative".to_owned())?;
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_orderbook_json(
                kind,
                payload_bytes.as_ptr(),
                payload_bytes.len(),
                label_bytes.as_ptr(),
                label_bytes.len(),
                generated_at,
            )
        };
        unsafe { java_sorafs_reference_buffer_to_array(env, buffer, "SoraFS orderbook validation") }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_validate_pop_payload_json(
    env: &mut jni::JNIEnv<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let payload_bytes = read_java_byte_array(env, &payload, "noritoBytes")
            .ok_or_else(|| "invalid PoP payload bytes".to_owned())?;
        let label_bytes = read_java_byte_array(env, &label, "label")
            .ok_or_else(|| "invalid PoP label bytes".to_owned())?;
        let kind = u32::try_from(kind).map_err(|_| "kind must be non-negative".to_owned())?;
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_pop_json(
                kind,
                payload_bytes.as_ptr(),
                payload_bytes.len(),
                label_bytes.as_ptr(),
                label_bytes.len(),
                generated_at,
            )
        };
        unsafe { java_sorafs_reference_buffer_to_array(env, buffer, "SoraFS PoP validation") }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_validate_hedging_payload_json(
    env: &mut jni::JNIEnv<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let payload_bytes = read_java_byte_array(env, &payload, "noritoBytes")
            .ok_or_else(|| "invalid hedging payload bytes".to_owned())?;
        let label_bytes = read_java_byte_array(env, &label, "label")
            .ok_or_else(|| "invalid hedging label bytes".to_owned())?;
        let kind = u32::try_from(kind).map_err(|_| "kind must be non-negative".to_owned())?;
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_hedging_json(
                kind,
                payload_bytes.as_ptr(),
                payload_bytes.len(),
                label_bytes.as_ptr(),
                label_bytes.len(),
                generated_at,
            )
        };
        unsafe { java_sorafs_reference_buffer_to_array(env, buffer, "SoraFS hedging validation") }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json(
    env: &mut jni::JNIEnv<'_>,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let payload_bytes = read_java_byte_array(env, &payload, "noritoBytes")
            .ok_or_else(|| "invalid appeal-finance CancelAssetLock payload bytes".to_owned())?;
        let label_bytes = read_java_byte_array(env, &label, "label")
            .ok_or_else(|| "invalid appeal-finance CancelAssetLock label bytes".to_owned())?;
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_appeal_finance_cancel_asset_lock_json(
                payload_bytes.as_ptr(),
                payload_bytes.len(),
                label_bytes.as_ptr(),
                label_bytes.len(),
                generated_at,
            )
        };
        unsafe {
            java_sorafs_reference_buffer_to_array(
                env,
                buffer,
                "SoraFS appeal-finance CancelAssetLock validation",
            )
        }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_validate_governance_log_node_json(
    env: &mut jni::JNIEnv<'_>,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    expected_node_cid: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let maximum_input = CONNECT_NORITO_SORAFS_REFERENCE_MAX_INPUT_BYTES_V1 as usize;
        let maximum_label = CONNECT_NORITO_SORAFS_REFERENCE_MAX_LABEL_BYTES_V1 as usize;
        let cid_bytes = CONNECT_NORITO_SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1 as usize;
        let payload_bytes =
            read_java_sorafs_reference_byte_array(env, &payload, "noritoBytes", maximum_input)?;
        let label_bytes =
            read_java_sorafs_reference_byte_array(env, &label, "label", maximum_label)?;
        let expected_node_cid_bytes = read_java_sorafs_reference_byte_array(
            env,
            &expected_node_cid,
            "expectedNodeCid",
            cid_bytes,
        )?;
        if expected_node_cid_bytes.len() != cid_bytes {
            return Err(format!(
                "expectedNodeCid must contain exactly {cid_bytes} bytes"
            ));
        }
        let aggregate_bytes = payload_bytes
            .len()
            .checked_add(label_bytes.len())
            .and_then(|total| total.checked_add(expected_node_cid_bytes.len()))
            .ok_or_else(|| "governance log-node aggregate input length overflowed".to_owned())?;
        if aggregate_bytes > maximum_input {
            return Err(format!(
                "governance log-node inputs must total at most {maximum_input} bytes"
            ));
        }
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_governance_json(
                payload_bytes.as_ptr(),
                payload_bytes.len(),
                label_bytes.as_ptr(),
                label_bytes.len(),
                expected_node_cid_bytes.as_ptr(),
                expected_node_cid_bytes.len(),
                generated_at,
            )
        };
        unsafe {
            java_sorafs_reference_buffer_to_array(
                env,
                buffer,
                "SoraFS governance log-node validation",
            )
        }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_validate_governance_dag_block_json(
    env: &mut jni::JNIEnv<'_>,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    expected_block_cid: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let maximum_input = CONNECT_NORITO_SORAFS_REFERENCE_MAX_INPUT_BYTES_V1 as usize;
        let maximum_label = CONNECT_NORITO_SORAFS_REFERENCE_MAX_LABEL_BYTES_V1 as usize;
        let cid_bytes = CONNECT_NORITO_SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1 as usize;
        let payload_bytes =
            read_java_sorafs_reference_byte_array(env, &payload, "noritoBytes", maximum_input)?;
        let label_bytes =
            read_java_sorafs_reference_byte_array(env, &label, "label", maximum_label)?;
        let expected_block_cid_bytes = read_java_sorafs_reference_byte_array(
            env,
            &expected_block_cid,
            "expectedBlockCid",
            cid_bytes,
        )?;
        if !expected_block_cid_bytes.is_empty() && expected_block_cid_bytes.len() != cid_bytes {
            return Err(format!(
                "expectedBlockCid must be empty or exactly {cid_bytes} bytes"
            ));
        }
        let aggregate_bytes = payload_bytes
            .len()
            .checked_add(label_bytes.len())
            .and_then(|total| total.checked_add(expected_block_cid_bytes.len()))
            .ok_or_else(|| "governance DAG block aggregate input length overflowed".to_owned())?;
        if aggregate_bytes > maximum_input {
            return Err(format!(
                "governance DAG block inputs must total at most {maximum_input} bytes"
            ));
        }
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_governance_dag_block_json(
                payload_bytes.as_ptr(),
                payload_bytes.len(),
                label_bytes.as_ptr(),
                label_bytes.len(),
                expected_block_cid_bytes.as_ptr(),
                expected_block_cid_bytes.len(),
                generated_at,
            )
        };
        unsafe {
            java_sorafs_reference_buffer_to_array(
                env,
                buffer,
                "SoraFS governance DAG block validation",
            )
        }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_validate_governance_dag_head_chain_json(
    env: &mut jni::JNIEnv<'_>,
    head: jni::objects::JByteArray<'_>,
    head_label: jni::objects::JByteArray<'_>,
    blocks: jni::objects::JObjectArray<'_>,
    block_labels: jni::objects::JObjectArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let maximum_blocks = CONNECT_NORITO_SORAFS_REFERENCE_GOVERNANCE_DAG_MAX_BLOCKS_V1 as usize;
        let maximum_input = CONNECT_NORITO_SORAFS_REFERENCE_MAX_INPUT_BYTES_V1 as usize;
        let maximum_label = CONNECT_NORITO_SORAFS_REFERENCE_MAX_LABEL_BYTES_V1 as usize;
        let head_bytes = read_java_sorafs_reference_byte_array(env, &head, "head", maximum_input)?;
        let head_label_bytes =
            read_java_sorafs_reference_byte_array(env, &head_label, "headLabel", maximum_label)?;
        let block_bytes = read_java_sorafs_reference_byte_array_vector(
            env,
            &blocks,
            "blocks",
            None,
            maximum_blocks,
            maximum_input,
        )?;
        let block_label_bytes = read_java_sorafs_reference_byte_array_vector(
            env,
            &block_labels,
            "blockLabels",
            Some(block_bytes.len()),
            maximum_blocks,
            maximum_label,
        )?;
        let mut aggregate_bytes = head_bytes
            .len()
            .checked_add(head_label_bytes.len())
            .ok_or_else(|| {
                "governance DAG head-chain aggregate input length overflowed".to_owned()
            })?;
        let mut descriptors = Vec::with_capacity(block_bytes.len());
        for (block, label) in block_bytes.iter().zip(&block_label_bytes) {
            aggregate_bytes = aggregate_bytes
                .checked_add(block.len())
                .and_then(|total| total.checked_add(label.len()))
                .ok_or_else(|| {
                    "governance DAG head-chain aggregate input length overflowed".to_owned()
                })?;
            if aggregate_bytes > maximum_input {
                return Err(format!(
                    "governance DAG head-chain inputs must total at most {maximum_input} bytes"
                ));
            }
            descriptors.push(sorafs_reference_ffi::SorafsReferenceFfiInput {
                bytes_ptr: block.as_ptr(),
                bytes_len: block.len(),
                label_ptr: label.as_ptr(),
                label_len: label.len(),
            });
        }
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_governance_dag_head_chain_json(
                head_bytes.as_ptr(),
                head_bytes.len(),
                head_label_bytes.as_ptr(),
                head_label_bytes.len(),
                descriptors.as_ptr(),
                descriptors.len(),
                generated_at,
            )
        };
        unsafe {
            java_sorafs_reference_buffer_to_array(
                env,
                buffer,
                "SoraFS governance DAG head-chain validation",
            )
        }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_validate_fixture_bundle_json(
    env: &mut jni::JNIEnv<'_>,
    kinds: jni::objects::JByteArray<'_>,
    payloads: jni::objects::JObjectArray<'_>,
    labels: jni::objects::JObjectArray<'_>,
    now: jni::sys::jlong,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let maximum_payloads = CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_PAYLOADS_V1 as usize;
        let maximum_bytes = CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_TOTAL_BYTES_V1 as usize;
        let maximum_label = CONNECT_NORITO_SORAFS_REFERENCE_MAX_LABEL_BYTES_V1 as usize;
        let kind_bytes =
            read_java_sorafs_reference_byte_array(env, &kinds, "kinds", maximum_payloads)?;
        if kind_bytes.is_empty() {
            return Err(format!("kinds must contain 1..{maximum_payloads} entries"));
        }
        let payload_bytes = read_java_sorafs_reference_byte_array_vector(
            env,
            &payloads,
            "payloads",
            Some(kind_bytes.len()),
            maximum_payloads,
            maximum_bytes,
        )?;
        let label_bytes = read_java_sorafs_reference_byte_array_vector(
            env,
            &labels,
            "labels",
            Some(kind_bytes.len()),
            maximum_payloads,
            maximum_label,
        )?;
        let mut aggregate_bytes = 0usize;
        let mut descriptors = Vec::with_capacity(kind_bytes.len());
        for ((kind, payload), label) in kind_bytes
            .iter()
            .copied()
            .zip(&payload_bytes)
            .zip(&label_bytes)
        {
            aggregate_bytes = aggregate_bytes
                .checked_add(payload.len())
                .and_then(|total| total.checked_add(label.len()))
                .ok_or_else(|| "fixture-bundle aggregate input length overflowed".to_owned())?;
            if aggregate_bytes > maximum_bytes {
                return Err(format!(
                    "fixture-bundle inputs must total at most {maximum_bytes} bytes"
                ));
            }
            descriptors.push(sorafs_reference_ffi::SorafsReferenceFfiBundlePayload {
                kind: u32::from(kind),
                bytes_ptr: payload.as_ptr(),
                bytes_len: payload.len(),
                label_ptr: label.as_ptr(),
                label_len: label.len(),
            });
        }
        let now = java_sorafs_reference_generated_at(now)?;
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_bundle_json(
                descriptors.as_ptr(),
                descriptors.len(),
                now,
                generated_at,
            )
        };
        unsafe {
            java_sorafs_reference_buffer_to_array(env, buffer, "SoraFS fixture-bundle validation")
        }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_sign_orderbook_payload(
    env: &mut jni::JNIEnv<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let payload_bytes = read_java_byte_array(env, &payload, "noritoBytes")
            .ok_or_else(|| "invalid orderbook payload bytes".to_owned())?;
        let private_key_bytes = Zeroizing::new(
            read_java_byte_array(env, &private_key, "privateKey")
                .ok_or_else(|| "invalid orderbook private key bytes".to_owned())?,
        );
        let kind = u32::try_from(kind).map_err(|_| "kind must be non-negative".to_owned())?;
        let kind = sorafs_reference_orderbook_kind_from_bridge(kind)
            .map_err(|_| "unsupported orderbook payload kind".to_owned())?;
        let signed = sign_orderbook_payload_bytes_ed25519_v1(
            kind,
            &payload_bytes,
            private_key_bytes.as_slice(),
        )
        .map_err(|err| err.to_string())?;
        env.byte_array_from_slice(&signed)
            .map(|array| array.into_raw())
            .map_err(|err| err.to_string())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_derive_orderbook_order_id(
    env: &mut jni::JNIEnv<'_>,
    owner_account: jni::objects::JByteArray<'_>,
    nonce: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let owner_account = java_sorafs_orderbook_non_empty(
            read_java_byte_array(env, &owner_account, "ownerAccount")
                .ok_or_else(|| "invalid ownerAccount bytes".to_owned())?,
            "ownerAccount",
        )?;
        let nonce = java_sorafs_orderbook_u64(nonce, "nonce")?;
        if nonce == 0 {
            return Err("nonce must be positive".to_owned());
        }
        let order_id = derive_orderbook_order_id_v1(&owner_account, nonce);
        env.byte_array_from_slice(&order_id)
            .map(|array| array.into_raw())
            .map_err(|err| err.to_string())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_orderbook_u64(
    value: jni::sys::jlong,
    field: &str,
) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{field} must be non-negative"))
}
pub(super) fn java_sorafs_orderbook_fee_bps(
    value: jni::sys::jint,
    field: &str,
) -> Result<u16, String> {
    u16::try_from(value).map_err(|_| format!("{field} must fit in u16 basis points"))
}
pub(super) fn java_sorafs_orderbook_fixed32(
    bytes: Vec<u8>,
    field: &str,
) -> Result<[u8; 32], String> {
    if bytes.len() != 32 {
        return Err(format!("{field} must be 32 bytes"));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}
pub(super) fn java_sorafs_orderbook_provider_id(
    bytes: Vec<u8>,
) -> Result<Option<[u8; 32]>, String> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let provider_id = java_sorafs_orderbook_fixed32(bytes, "providerId")?;
    if provider_id == [0; 32] {
        return Err("providerId must not be all zero".to_owned());
    }
    Ok(Some(provider_id))
}
pub(super) fn java_sorafs_orderbook_non_empty(
    bytes: Vec<u8>,
    field: &str,
) -> Result<Vec<u8>, String> {
    if bytes.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if bytes.len() > ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 {
        return Err(format!(
            "{field} must be at most {ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1} bytes"
        ));
    }
    Ok(bytes)
}
pub(super) fn java_sorafs_orderbook_xor_quantity(
    bytes: Vec<u8>,
    field: &str,
) -> Result<XorQuantity, String> {
    sorafs_xor_quantity_from_bytes(&bytes)
        .map_err(|_| format!("{field} must be a canonical non-negative XOR quantity"))
}
pub(super) fn java_sorafs_reference_build_signed_orderbook_order_request(
    env: &mut jni::JNIEnv<'_>,
    inputs: JavaSorafsOrderbookOrderRequestArrays<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let private_key_bytes = Zeroizing::new(
            read_java_byte_array(env, &inputs.private_key, "privateKey")
                .ok_or_else(|| "invalid orderbook private key bytes".to_owned())?,
        );
        let supplied_order_id = java_sorafs_orderbook_fixed32(
            read_java_byte_array(env, &inputs.order_id, "orderId")
                .ok_or_else(|| "invalid orderId bytes".to_owned())?,
            "orderId",
        )?;
        let owner_account = java_sorafs_orderbook_non_empty(
            read_java_byte_array(env, &inputs.owner_account, "ownerAccount")
                .ok_or_else(|| "invalid ownerAccount bytes".to_owned())?,
            "ownerAccount",
        )?;
        let provider_id = java_sorafs_orderbook_provider_id(
            read_java_byte_array(env, &inputs.provider_id, "providerId")
                .ok_or_else(|| "invalid providerId bytes".to_owned())?,
        )?;
        let nonce = java_sorafs_orderbook_u64(inputs.nonce, "nonce")?;
        let expected_order_id = derive_orderbook_order_id_v1(&owner_account, nonce);
        if supplied_order_id != expected_order_id {
            return Err(format!(
                "orderId must equal the canonical owner-and-nonce derivation {}",
                hex::encode(expected_order_id)
            ));
        }
        let fields = OrderbookOrderRequestFieldsV1 {
            side: sorafs_orderbook_side_from_bridge(
                u32::try_from(inputs.side).map_err(|_| "side must be non-negative".to_owned())?,
            )
            .map_err(|_| "unsupported orderbook side".to_owned())?,
            tier: sorafs_orderbook_tier_from_bridge(
                u32::try_from(inputs.tier).map_err(|_| "tier must be non-negative".to_owned())?,
            )
            .map_err(|_| "unsupported orderbook tier".to_owned())?,
            price_per_gib: java_sorafs_orderbook_xor_quantity(
                read_java_byte_array(env, &inputs.price_per_gib, "pricePerGib")
                    .ok_or_else(|| "invalid pricePerGib bytes".to_owned())?,
                "pricePerGib",
            )?,
            quantity_gib: java_sorafs_orderbook_u64(inputs.quantity_gib, "quantityGib")?,
            remaining_gib: java_sorafs_orderbook_u64(inputs.remaining_gib, "remainingGib")?,
            owner_account,
            provider_id,
            expiry_unix: java_sorafs_orderbook_u64(inputs.expiry_unix, "expiryUnix")?,
            nonce,
            maker_fee_bps: java_sorafs_orderbook_fee_bps(inputs.maker_fee_bps, "makerFeeBps")?,
            taker_fee_bps: java_sorafs_orderbook_fee_bps(inputs.taker_fee_bps, "takerFeeBps")?,
        };
        let signed = build_signed_orderbook_order_request_bytes_ed25519_v1(
            fields,
            private_key_bytes.as_slice(),
        )
        .map_err(|err| err.to_string())?;
        env.byte_array_from_slice(&signed)
            .map(|array| array.into_raw())
            .map_err(|err| err.to_string())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) struct JavaSorafsOrderbookOrderRequestArrays<'a> {
    order_id: jni::objects::JByteArray<'a>,
    side: jni::sys::jint,
    tier: jni::sys::jint,
    price_per_gib: jni::objects::JByteArray<'a>,
    quantity_gib: jni::sys::jlong,
    remaining_gib: jni::sys::jlong,
    owner_account: jni::objects::JByteArray<'a>,
    provider_id: jni::objects::JByteArray<'a>,
    expiry_unix: jni::sys::jlong,
    nonce: jni::sys::jlong,
    maker_fee_bps: jni::sys::jint,
    taker_fee_bps: jni::sys::jint,
    private_key: jni::objects::JByteArray<'a>,
}
pub(super) fn java_sorafs_reference_build_signed_orderbook_order_cancel(
    env: &mut jni::JNIEnv<'_>,
    order_id: jni::objects::JByteArray<'_>,
    owner_account: jni::objects::JByteArray<'_>,
    reason: jni::sys::jint,
    nonce: jni::sys::jlong,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let private_key_bytes = Zeroizing::new(
            read_java_byte_array(env, &private_key, "privateKey")
                .ok_or_else(|| "invalid orderbook private key bytes".to_owned())?,
        );
        let fields = OrderbookOrderCancelFieldsV1 {
            order_id: java_sorafs_orderbook_fixed32(
                read_java_byte_array(env, &order_id, "orderId")
                    .ok_or_else(|| "invalid orderId bytes".to_owned())?,
                "orderId",
            )?,
            owner_account: java_sorafs_orderbook_non_empty(
                read_java_byte_array(env, &owner_account, "ownerAccount")
                    .ok_or_else(|| "invalid ownerAccount bytes".to_owned())?,
                "ownerAccount",
            )?,
            reason: sorafs_orderbook_cancel_reason_from_bridge(
                u32::try_from(reason).map_err(|_| "reason must be non-negative".to_owned())?,
            )
            .map_err(|_| "unsupported orderbook cancel reason".to_owned())?,
            nonce: java_sorafs_orderbook_u64(nonce, "nonce")?,
        };
        let signed = build_signed_orderbook_order_cancel_bytes_ed25519_v1(
            fields,
            private_key_bytes.as_slice(),
        )
        .map_err(|err| err.to_string())?;
        env.byte_array_from_slice(&signed)
            .map(|array| array.into_raw())
            .map_err(|err| err.to_string())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_build_signed_orderbook_settlement_receipt(
    env: &mut jni::JNIEnv<'_>,
    inputs: JavaSorafsOrderbookSettlementReceiptArrays<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let private_key_bytes = Zeroizing::new(
            read_java_byte_array(env, &inputs.private_key, "privateKey")
                .ok_or_else(|| "invalid orderbook private key bytes".to_owned())?,
        );
        let fields = OrderbookSettlementReceiptFieldsV1 {
            receipt_id: java_sorafs_orderbook_fixed32(
                read_java_byte_array(env, &inputs.receipt_id, "receiptId")
                    .ok_or_else(|| "invalid receiptId bytes".to_owned())?,
                "receiptId",
            )?,
            channel_id: java_sorafs_orderbook_fixed32(
                read_java_byte_array(env, &inputs.channel_id, "channelId")
                    .ok_or_else(|| "invalid channelId bytes".to_owned())?,
                "channelId",
            )?,
            trade_id: java_sorafs_orderbook_fixed32(
                read_java_byte_array(env, &inputs.trade_id, "tradeId")
                    .ok_or_else(|| "invalid tradeId bytes".to_owned())?,
                "tradeId",
            )?,
            range_start: java_sorafs_orderbook_u64(inputs.range_start, "rangeStart")?,
            range_end: java_sorafs_orderbook_u64(inputs.range_end, "rangeEnd")?,
            chunk_hash: java_sorafs_orderbook_fixed32(
                read_java_byte_array(env, &inputs.chunk_hash, "chunkHash")
                    .ok_or_else(|| "invalid chunkHash bytes".to_owned())?,
                "chunkHash",
            )?,
            bytes_delivered: java_sorafs_orderbook_u64(inputs.bytes_delivered, "bytesDelivered")?,
            xor_debited: java_sorafs_orderbook_xor_quantity(
                read_java_byte_array(env, &inputs.xor_debited, "xorDebited")
                    .ok_or_else(|| "invalid xorDebited bytes".to_owned())?,
                "xorDebited",
            )?,
            provider_credit: java_sorafs_orderbook_xor_quantity(
                read_java_byte_array(env, &inputs.provider_credit, "providerCredit")
                    .ok_or_else(|| "invalid providerCredit bytes".to_owned())?,
                "providerCredit",
            )?,
            fee_amount: java_sorafs_orderbook_xor_quantity(
                read_java_byte_array(env, &inputs.fee_amount, "feeAmount")
                    .ok_or_else(|| "invalid feeAmount bytes".to_owned())?,
                "feeAmount",
            )?,
            issued_at_unix: java_sorafs_orderbook_u64(inputs.issued_at_unix, "issuedAtUnix")?,
        };
        let signed = build_signed_orderbook_settlement_receipt_bytes_ed25519_v1(
            fields,
            private_key_bytes.as_slice(),
        )
        .map_err(|err| err.to_string())?;
        env.byte_array_from_slice(&signed)
            .map(|array| array.into_raw())
            .map_err(|err| err.to_string())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) struct JavaSorafsOrderbookSettlementReceiptArrays<'a> {
    receipt_id: jni::objects::JByteArray<'a>,
    channel_id: jni::objects::JByteArray<'a>,
    trade_id: jni::objects::JByteArray<'a>,
    range_start: jni::sys::jlong,
    range_end: jni::sys::jlong,
    chunk_hash: jni::objects::JByteArray<'a>,
    bytes_delivered: jni::sys::jlong,
    xor_debited: jni::objects::JByteArray<'a>,
    provider_credit: jni::objects::JByteArray<'a>,
    fee_amount: jni::objects::JByteArray<'a>,
    issued_at_unix: jni::sys::jlong,
    private_key: jni::objects::JByteArray<'a>,
}
pub(super) fn java_sorafs_reference_validate_pdp_payload_json(
    env: &mut jni::JNIEnv<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let payload_bytes = read_java_byte_array(env, &payload, "noritoBytes")
            .ok_or_else(|| "invalid PDP payload bytes".to_owned())?;
        let label_bytes = read_java_byte_array(env, &label, "label")
            .ok_or_else(|| "invalid PDP label bytes".to_owned())?;
        let kind = u32::try_from(kind).map_err(|_| "kind must be non-negative".to_owned())?;
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_validate_pdp_payload_buffer(
                kind,
                payload_bytes.as_ptr(),
                payload_bytes.len() as c_ulong,
                label_bytes.as_ptr(),
                label_bytes.len() as c_ulong,
                generated_at,
            )
        };
        unsafe { java_sorafs_reference_buffer_to_array(env, buffer, "SoraFS PDP validation") }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_validate_pdp_commitment_challenge_json(
    env: &mut jni::JNIEnv<'_>,
    commitment: jni::objects::JByteArray<'_>,
    commitment_label: jni::objects::JByteArray<'_>,
    challenge: jni::objects::JByteArray<'_>,
    challenge_label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let commitment_bytes = read_java_byte_array(env, &commitment, "commitment")
            .ok_or_else(|| "invalid PDP commitment bytes".to_owned())?;
        let commitment_label_bytes =
            read_java_byte_array(env, &commitment_label, "commitmentLabel")
                .ok_or_else(|| "invalid PDP commitment label bytes".to_owned())?;
        let challenge_bytes = read_java_byte_array(env, &challenge, "challenge")
            .ok_or_else(|| "invalid PDP challenge bytes".to_owned())?;
        let challenge_label_bytes =
            read_java_byte_array(env, &challenge_label, "challengeLabel")
                .ok_or_else(|| "invalid PDP challenge label bytes".to_owned())?;
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_pdp_commitment_challenge_json(
                commitment_bytes.as_ptr(),
                commitment_bytes.len(),
                commitment_label_bytes.as_ptr(),
                commitment_label_bytes.len(),
                challenge_bytes.as_ptr(),
                challenge_bytes.len(),
                challenge_label_bytes.as_ptr(),
                challenge_label_bytes.len(),
                generated_at,
            )
        };
        unsafe {
            java_sorafs_reference_buffer_to_array(
                env,
                buffer,
                "SoraFS PDP commitment/challenge validation",
            )
        }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_sorafs_reference_validate_pdp_challenge_proof_json(
    env: &mut jni::JNIEnv<'_>,
    challenge: jni::objects::JByteArray<'_>,
    challenge_label: jni::objects::JByteArray<'_>,
    proof: jni::objects::JByteArray<'_>,
    proof_label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let challenge_bytes = read_java_byte_array(env, &challenge, "challenge")
            .ok_or_else(|| "invalid PDP challenge bytes".to_owned())?;
        let challenge_label_bytes =
            read_java_byte_array(env, &challenge_label, "challengeLabel")
                .ok_or_else(|| "invalid PDP challenge label bytes".to_owned())?;
        let proof_bytes = read_java_byte_array(env, &proof, "proof")
            .ok_or_else(|| "invalid PDP proof bytes".to_owned())?;
        let proof_label_bytes = read_java_byte_array(env, &proof_label, "proofLabel")
            .ok_or_else(|| "invalid PDP proof label bytes".to_owned())?;
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_pdp_challenge_proof_json(
                challenge_bytes.as_ptr(),
                challenge_bytes.len(),
                challenge_label_bytes.as_ptr(),
                challenge_label_bytes.len(),
                proof_bytes.as_ptr(),
                proof_bytes.len(),
                proof_label_bytes.as_ptr(),
                proof_label_bytes.len(),
                generated_at,
            )
        };
        unsafe {
            java_sorafs_reference_buffer_to_array(
                env,
                buffer,
                "SoraFS PDP challenge/proof validation",
            )
        }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) struct JavaSorafsPdpBundleArrays<'local> {
    commitment: jni::objects::JByteArray<'local>,
    commitment_label: jni::objects::JByteArray<'local>,
    challenge: jni::objects::JByteArray<'local>,
    challenge_label: jni::objects::JByteArray<'local>,
    proof: jni::objects::JByteArray<'local>,
    proof_label: jni::objects::JByteArray<'local>,
}
pub(super) fn java_sorafs_reference_validate_pdp_bundle_json(
    env: &mut jni::JNIEnv<'_>,
    arrays: JavaSorafsPdpBundleArrays<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let commitment_bytes = read_java_byte_array(env, &arrays.commitment, "commitment")
            .ok_or_else(|| "invalid PDP commitment bytes".to_owned())?;
        let commitment_label_bytes =
            read_java_byte_array(env, &arrays.commitment_label, "commitmentLabel")
                .ok_or_else(|| "invalid PDP commitment label bytes".to_owned())?;
        let challenge_bytes = read_java_byte_array(env, &arrays.challenge, "challenge")
            .ok_or_else(|| "invalid PDP challenge bytes".to_owned())?;
        let challenge_label_bytes =
            read_java_byte_array(env, &arrays.challenge_label, "challengeLabel")
                .ok_or_else(|| "invalid PDP challenge label bytes".to_owned())?;
        let proof_bytes = read_java_byte_array(env, &arrays.proof, "proof")
            .ok_or_else(|| "invalid PDP proof bytes".to_owned())?;
        let proof_label_bytes = read_java_byte_array(env, &arrays.proof_label, "proofLabel")
            .ok_or_else(|| "invalid PDP proof label bytes".to_owned())?;
        let generated_at = java_sorafs_reference_generated_at(generated_at)?;
        let buffer = unsafe {
            sorafs_reference_ffi::sorafs_reference_validate_pdp_json(
                commitment_bytes.as_ptr(),
                commitment_bytes.len(),
                commitment_label_bytes.as_ptr(),
                commitment_label_bytes.len(),
                challenge_bytes.as_ptr(),
                challenge_bytes.len(),
                challenge_label_bytes.as_ptr(),
                challenge_label_bytes.len(),
                proof_bytes.as_ptr(),
                proof_bytes.len(),
                proof_label_bytes.as_ptr(),
                proof_label_bytes.len(),
                generated_at,
            )
        };
        unsafe {
            java_sorafs_reference_buffer_to_array(env, buffer, "SoraFS PDP bundle validation")
        }
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            ptr::null_mut()
        }
    }
}
pub(super) fn java_algorithm_from_code(
    algorithm_code: jni::sys::jint,
) -> Result<Algorithm, String> {
    let checked_code = u8::try_from(algorithm_code)
        .map_err(|_| format!("unsupported signing algorithm code: {algorithm_code}"))?;
    parse_algorithm_code(checked_code)
        .map_err(|_| format!("unsupported signing algorithm code: {algorithm_code}"))
}
pub(super) fn java_public_key_from_private_bytes(
    algorithm_code: jni::sys::jint,
    private_key: &[u8],
) -> Result<Vec<u8>, String> {
    let algorithm = java_algorithm_from_code(algorithm_code)?;
    let private_key = parse_private_key_with_algorithm(private_key, algorithm)
        .map_err(|_| "invalid private key bytes".to_string())?;
    let key_pair = KeyPair::from_private_key(private_key)
        .map_err(|_| "failed to derive public key".to_string())?;
    key_pair
        .public_key()
        .try_to_bytes()
        .map(|(_algorithm, payload)| payload.to_vec())
        .map_err(|_| "failed to extract public key bytes".to_string())
}
pub(super) fn java_keypair_from_seed_bytes(
    algorithm_code: jni::sys::jint,
    seed: &[u8],
) -> Result<(Zeroizing<Vec<u8>>, Vec<u8>), String> {
    let algorithm = java_algorithm_from_code(algorithm_code)?;
    let key_pair = KeyPair::try_from_seed(seed.to_vec(), algorithm)
        .map_err(|err| format!("failed to derive key pair: {err}"))?;
    let (public_key, private_key) = key_pair.into_parts();
    let public_bytes = public_key
        .try_to_bytes()
        .map(|(_algorithm, payload)| payload.to_vec())
        .map_err(|_| "failed to extract public key bytes".to_string())?;
    Ok((Zeroizing::new(private_key.to_bytes().1), public_bytes))
}
pub(super) fn java_sign_detached_bytes(
    algorithm_code: jni::sys::jint,
    private_key: &[u8],
    message: &[u8],
) -> Result<Vec<u8>, String> {
    let algorithm = java_algorithm_from_code(algorithm_code)?;
    let private_key = parse_private_key_with_algorithm(private_key, algorithm)
        .map_err(|_| "invalid private key bytes".to_string())?;
    Signature::try_new(&private_key, message)
        .map(|signature| signature.payload().to_vec())
        .map_err(|err| format!("failed to sign message: {err}"))
}
pub(super) fn java_verify_detached_bytes(
    algorithm_code: jni::sys::jint,
    public_key: &[u8],
    message: &[u8],
    signature: &[u8],
) -> Result<bool, String> {
    let algorithm = java_algorithm_from_code(algorithm_code)?;
    let public_key = PublicKey::from_bytes(algorithm, public_key)
        .map_err(|_| "invalid public key bytes".to_string())?;
    let signature = match connect_signature_from_algorithm_bytes(algorithm, signature) {
        Some(signature) => signature,
        None => return Ok(false),
    };
    match signature.verify(&public_key, message) {
        Ok(()) => Ok(true),
        Err(CryptoError::BadSignature) => Ok(false),
        Err(_) => Err("signature verification failed".to_string()),
    }
}
pub(super) fn java_native_public_key_from_private(
    env: &mut jni::JNIEnv<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let private_bytes = read_java_byte_array(env, &private_key, "privateKey")
            .ok_or_else(|| "invalid private key bytes".to_string())?;
        let public_bytes = java_public_key_from_private_bytes(algorithm_code, &private_bytes)?;
        let array = env
            .byte_array_from_slice(&public_bytes)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_keypair_from_seed(
    env: &mut jni::JNIEnv<'_>,
    algorithm_code: jni::sys::jint,
    seed: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let result = (|| -> Result<jni::sys::jobjectArray, String> {
        let seed_bytes = read_java_byte_array(env, &seed, "seed")
            .ok_or_else(|| "invalid seed bytes".to_string())?;
        let (private_bytes, public_bytes) =
            java_keypair_from_seed_bytes(algorithm_code, &seed_bytes)?;
        let private_array = env
            .byte_array_from_slice(&private_bytes)
            .map_err(|err| err.to_string())?;
        let public_array = env
            .byte_array_from_slice(&public_bytes)
            .map_err(|err| err.to_string())?;
        let byte_array_class = env.find_class("[B").map_err(|err| err.to_string())?;
        let array = env
            .new_object_array(2, byte_array_class, jni::objects::JObject::null())
            .map_err(|err| err.to_string())?;
        env.set_object_array_element(&array, 0, &private_array)
            .map_err(|err| err.to_string())?;
        env.set_object_array_element(&array, 1, &public_array)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_sign_detached(
    env: &mut jni::JNIEnv<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<jni::sys::jbyteArray, String> {
        let private_bytes = read_java_byte_array(env, &private_key, "privateKey")
            .ok_or_else(|| "invalid private key bytes".to_string())?;
        let message_bytes = read_java_byte_array(env, &message, "message")
            .ok_or_else(|| "invalid message bytes".to_string())?;
        let signature = java_sign_detached_bytes(algorithm_code, &private_bytes, &message_bytes)?;
        let array = env
            .byte_array_from_slice(&signature)
            .map_err(|err| err.to_string())?;
        Ok(array.into_raw())
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_verify_detached(
    env: &mut jni::JNIEnv<'_>,
    algorithm_code: jni::sys::jint,
    public_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
    signature: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    let result = (|| -> Result<jni::sys::jboolean, String> {
        let public_bytes = read_java_byte_array(env, &public_key, "publicKey")
            .ok_or_else(|| "invalid public key bytes".to_string())?;
        let message_bytes = read_java_byte_array(env, &message, "message")
            .ok_or_else(|| "invalid message bytes".to_string())?;
        let signature_bytes = read_java_byte_array(env, &signature, "signature")
            .ok_or_else(|| "invalid signature bytes".to_string())?;
        let valid = java_verify_detached_bytes(
            algorithm_code,
            &public_bytes,
            &message_bytes,
            &signature_bytes,
        )?;
        Ok(if valid { 1 } else { 0 })
    })();
    match result {
        Ok(valid) => valid,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            0
        }
    }
}
pub(super) fn java_text_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JByteArray<'_>,
    context: &str,
) -> Result<String, String> {
    let mut bytes = read_java_byte_array(env, array, context)
        .ok_or_else(|| format!("invalid {context} bytes"))?;
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) if !text.trim().is_empty() && text.trim() == text => text.to_owned(),
        Ok(_) => {
            bytes.fill(0);
            return Err(format!(
                "{context} must be non-empty without surrounding whitespace"
            ));
        }
        Err(_) => {
            bytes.fill(0);
            return Err(format!("{context} must be UTF-8"));
        }
    };
    bytes.fill(0);
    Ok(text)
}
pub(super) fn java_network_id(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JByteArray<'_>,
) -> Result<NetworkId, String> {
    let bytes = read_java_byte_array(env, array, "networkId")
        .ok_or_else(|| "invalid networkId bytes".to_owned())?;
    network_id_from_raw_bytes(&bytes).map_err(str::to_owned)
}
pub(super) fn java_optional_text_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JByteArray<'_>,
    present: jni::sys::jboolean,
    context: &str,
) -> Result<Option<String>, String> {
    if present == 0 {
        return Ok(None);
    }
    java_text_array(env, array, context).map(Some)
}
pub(super) fn java_fee_payment_intent(
    env: &mut jni::JNIEnv<'_>,
    fee_payment_json: &jni::objects::JByteArray<'_>,
) -> Result<FeePaymentIntent, String> {
    let mut bytes = read_java_byte_array(env, fee_payment_json, "feePaymentJson")
        .ok_or_else(|| "invalid feePaymentJson bytes".to_owned())?;
    let result = java_fee_payment_intent_from_json(&bytes);
    bytes.fill(0);
    result
}
pub(super) fn java_fee_payment_intent_from_json(bytes: &[u8]) -> Result<FeePaymentIntent, String> {
    let intent = norito::json::from_slice::<FeePaymentIntent>(bytes)
        .map_err(|err| format!("feePaymentJson must be canonical Norito JSON: {err}"))?;
    intent
        .validate()
        .map_err(|err| format!("invalid feePayment: {err}"))?;
    Ok(intent)
}
pub(super) fn java_verifying_key_id(
    value: Option<String>,
    context: &str,
) -> Result<Option<VerifyingKeyId>, String> {
    value
        .map(|text| {
            parse_verifying_key_id_value(&text)
                .map_err(|_| format!("{context} must use backend:name syntax"))
        })
        .transpose()
}
pub(super) fn java_private_key(
    algorithm_code: jni::sys::jint,
    private_key: &jni::objects::JByteArray<'_>,
    env: &mut jni::JNIEnv<'_>,
) -> Result<PrivateKey, String> {
    let algorithm = java_algorithm_from_code(algorithm_code)?;
    let mut private_bytes = read_java_byte_array(env, private_key, "privateKey")
        .ok_or_else(|| "invalid private key bytes".to_owned())?;
    let key = parse_private_key_with_algorithm(&private_bytes, algorithm)
        .map_err(|_| "invalid private key bytes".to_owned());
    private_bytes.fill(0);
    key
}
pub(super) fn java_signed_transaction_pair(
    env: &mut jni::JNIEnv<'_>,
    signed_bytes: &[u8],
    hash_bytes: &[u8; 32],
) -> Result<jni::sys::jobjectArray, String> {
    java_byte_array_pair(env, signed_bytes, hash_bytes)
}
pub(super) fn java_byte_array_pair(
    env: &mut jni::JNIEnv<'_>,
    first: &[u8],
    second: &[u8],
) -> Result<jni::sys::jobjectArray, String> {
    let signed_array = env
        .byte_array_from_slice(first)
        .map_err(|err| err.to_string())?;
    let hash_array = env
        .byte_array_from_slice(second)
        .map_err(|err| err.to_string())?;
    let byte_array_class = env.find_class("[B").map_err(|err| err.to_string())?;
    let array = env
        .new_object_array(2, byte_array_class, jni::objects::JObject::null())
        .map_err(|err| err.to_string())?;
    env.set_object_array_element(&array, 0, &signed_array)
        .map_err(|err| err.to_string())?;
    env.set_object_array_element(&array, 1, &hash_array)
        .map_err(|err| err.to_string())?;
    Ok(array.into_raw())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_encode_register_zk_asset_signed_transaction(
    env: &mut jni::JNIEnv<'_>,
    algorithm_code: jni::sys::jint,
    network_id: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    authority: jni::objects::JByteArray<'_>,
    creation_time_ms: jni::sys::jlong,
    ttl_ms: jni::sys::jlong,
    ttl_present: jni::sys::jboolean,
    asset: jni::objects::JByteArray<'_>,
    vk_unshield: jni::objects::JByteArray<'_>,
    vk_unshield_present: jni::sys::jboolean,
    vk_shield: jni::objects::JByteArray<'_>,
    vk_shield_present: jni::sys::jboolean,
    private_key: jni::objects::JByteArray<'_>,
    fee_payment_json: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let result = (|| -> Result<jni::sys::jobjectArray, String> {
        if creation_time_ms < 0 || ttl_ms < 0 {
            return Err("creationTimeMs and ttlMs must be non-negative".to_owned());
        }
        let network_id = java_network_id(env, &network_id)?;
        let chain_discriminant = u16::try_from(chain_discriminant)
            .map_err(|_| "chainDiscriminant must fit in u16".to_owned())?;
        let authority = parse_account_id_for_chain(
            java_text_array(env, &authority, "authority")?,
            chain_discriminant,
        )
        .map_err(|_| "invalid authority".to_owned())?;
        let asset_definition = parse_asset_definition(java_text_array(env, &asset, "asset")?)
            .map_err(|_| "invalid asset".to_owned())?;
        let vk_unshield = java_verifying_key_id(
            java_optional_text_array(
                env,
                &vk_unshield,
                vk_unshield_present,
                "unshieldVerifyingKey",
            )?,
            "unshieldVerifyingKey",
        )?;
        let vk_shield = java_verifying_key_id(
            java_optional_text_array(env, &vk_shield, vk_shield_present, "shieldVerifyingKey")?,
            "shieldVerifyingKey",
        )?;
        let private_key = java_private_key(algorithm_code, &private_key, env)?;
        let ttl =
            parse_ttl(ttl_ms as u64, ttl_present != 0).map_err(|_| "invalid ttlMs".to_owned())?;
        let register = zk::RegisterZkAsset::new(asset_definition, vk_unshield, vk_shield);
        register.validate_verifier_roles().map_err(str::to_owned)?;
        let fee_payment = java_fee_payment_intent(env, &fee_payment_json)?;
        let (signed_bytes, hash_bytes) =
            encode_asset_transaction_with_nonce_fee_payment_and_metadata(
                network_id,
                authority,
                creation_time_ms as u64,
                ttl,
                None,
                fee_payment,
                Metadata::default(),
                private_key,
                move || Executable::from([InstructionBox::from(register)]),
            )
            .map_err(|err| format!("failed to encode signed transaction ({})", err.code()))?;
        java_signed_transaction_pair(env, &signed_bytes, &hash_bytes)
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_kagemusha_pasta_cycle_v4_backend_available() -> jni::sys::jboolean {
    // This is an explicit readiness answer, not a symbol-presence probe. It
    // must stay aligned with the native V4 capability archive: merely linking
    // the symbol or retaining a partially installed release is insufficient.
    let available = iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND_AVAILABLE
        && kagemusha_recursive_spend_installed_artifact_set_registry_v4()
            .lock()
            .ok()
            .and_then(|installed| installed.clone())
            .is_some_and(|installed| installed.validate_live_inventory().is_ok());
    if available {
        jni::sys::JNI_TRUE
    } else {
        jni::sys::JNI_FALSE
    }
}
pub(super) fn java_native_kagemusha_artifact_begin_v4(
    env: &mut jni::JNIEnv<'_>,
    manifest_norito: jni::objects::JByteArray<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
    artifact_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jlong {
    let result = (|| -> Result<jni::sys::jlong, (bool, String)> {
        let manifest = read_java_byte_array_bounded(
            env,
            &manifest_norito,
            "manifestNorito",
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_MANIFEST_BYTES_V4 as usize,
        )
        .ok_or_else(|| (true, "invalid Kagemusha V4 manifest bytes".to_owned()))?;
        let manifest_digest = read_java_byte_array_bounded(
            env,
            &manifest_sha256,
            "manifestSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .ok_or_else(|| (true, "invalid Kagemusha V4 manifest digest".to_owned()))?;
        let artifact_digest = read_java_byte_array_bounded(
            env,
            &artifact_sha256,
            "artifactSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .ok_or_else(|| (true, "invalid Kagemusha V4 artifact digest".to_owned()))?;
        if manifest.is_empty() || manifest_digest.len() != 32 || artifact_digest.len() != 32 {
            return Err((
                true,
                "Kagemusha V4 begin requires a manifest and exact SHA-256 digests".to_owned(),
            ));
        }
        let mut handle = 0_u64;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_artifact_begin_v4(
                manifest.as_ptr(),
                c_ulong::try_from(manifest.len()).map_err(|_| {
                    (
                        true,
                        "Kagemusha V4 manifest exceeds the native range".to_owned(),
                    )
                })?,
                manifest_digest.as_ptr(),
                32,
                artifact_digest.as_ptr(),
                32,
                &mut handle,
            )
        };
        if status == 0 && handle != 0 {
            return i64::try_from(handle).map_err(|_| {
                (
                    false,
                    "Kagemusha V4 artifact handle exceeds the JNI range".to_owned(),
                )
            });
        }
        Err((
            status == ERR_KAGEMUSHA_RECURSIVE_SPEND_V4_ARTIFACT,
            format!("Kagemusha V4 artifact begin rejected with native status {status}"),
        ))
    })();
    match result {
        Ok(handle) => handle,
        Err((true, message)) => {
            throw_java_illegal_argument(env, message);
            0
        }
        Err((false, message)) => {
            throw_java_illegal_state(env, message);
            0
        }
    }
}
pub(super) fn java_native_kagemusha_artifact_write_v4(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
    chunk: jni::objects::JByteArray<'_>,
) {
    let result = (|| -> Result<(), String> {
        let handle = u64::try_from(handle)
            .ok()
            .filter(|handle| *handle != 0)
            .ok_or_else(|| "Kagemusha V4 artifact handle must be positive".to_owned())?;
        let chunk = read_java_byte_array_bounded(
            env,
            &chunk,
            "chunk",
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_JNI_MAX_CHUNK_BYTES_V4,
        )
        .ok_or_else(|| "Kagemusha V4 artifact chunk is invalid".to_owned())?;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_artifact_write_v4(
                handle,
                chunk.as_ptr(),
                c_ulong::try_from(chunk.len())
                    .map_err(|_| "Kagemusha V4 chunk exceeds the native range".to_owned())?,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "Kagemusha V4 artifact write rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, message);
    }
}
pub(super) fn java_native_kagemusha_artifact_finish_v4(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
    cancel: bool,
) {
    let result = (|| -> Result<(), String> {
        let handle = u64::try_from(handle)
            .ok()
            .filter(|handle| *handle != 0)
            .ok_or_else(|| "Kagemusha V4 artifact handle must be positive".to_owned())?;
        let status = if cancel {
            connect_norito_kagemusha_recursive_spend_artifact_cancel_v4(handle)
        } else {
            connect_norito_kagemusha_recursive_spend_artifact_finalize_v4(handle)
        };
        if status == 0 {
            Ok(())
        } else {
            let operation = if cancel { "cancel" } else { "finalize" };
            Err(format!(
                "Kagemusha V4 artifact {operation} rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, message);
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_artifact_set_install_v4(
    env: &mut jni::JNIEnv<'_>,
    manifest_norito: jni::objects::JByteArray<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
    trusted_policy_norito: jni::objects::JByteArray<'_>,
    release_attestation_norito: jni::objects::JByteArray<'_>,
    internal_validation_receipt_norito: jni::objects::JByteArray<'_>,
    benchmark_evidence: jni::objects::JByteArray<'_>,
    cryptographic_review: jni::objects::JByteArray<'_>,
    promotion_record_norito: jni::objects::JByteArray<'_>,
    handles: jni::objects::JLongArray<'_>,
) {
    let result = (|| -> Result<(), String> {
        let _permit = try_preacquire_kagemusha_heavy_proof_permit_v4().map_err(|_| {
            "Kagemusha V4 artifact-set install is busy; retry after the active proof completes"
                .to_owned()
        })?;
        let manifest = read_java_byte_array_bounded(
            env,
            &manifest_norito,
            "manifestNorito",
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_MANIFEST_BYTES_V4 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha V4 manifest".to_owned())?;
        let manifest_digest = read_java_byte_array_bounded(
            env,
            &manifest_sha256,
            "manifestSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .ok_or_else(|| "invalid Kagemusha V4 manifest digest".to_owned())?;
        let policy = read_java_byte_array_bounded(
            env,
            &trusted_policy_norito,
            "trustedPolicyNorito",
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_POLICY_BYTES_V1 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha V4 trusted policy".to_owned())?;
        let attestation = read_java_byte_array_bounded(
            env,
            &release_attestation_norito,
            "releaseAttestationNorito",
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_ATTESTATION_BYTES_V1 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha V4 release attestation".to_owned())?;
        let internal_validation_receipt = read_java_byte_array_bounded(
            env,
            &internal_validation_receipt_norito,
            "internalValidationReceiptNorito",
            KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha V4 internal-validation receipt".to_owned())?;
        let benchmark = read_java_byte_array_bounded(
            env,
            &benchmark_evidence,
            "benchmarkEvidence",
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha V4 benchmark evidence".to_owned())?;
        let review = read_java_byte_array_bounded(
            env,
            &cryptographic_review,
            "cryptographicReview",
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha V4 cryptographic review".to_owned())?;
        let promotion = read_java_byte_array_bounded(
            env,
            &promotion_record_norito,
            "promotionRecordNorito",
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha V4 promotion record".to_owned())?;
        if manifest.is_empty()
            || manifest_digest.len() != 32
            || policy.is_empty()
            || attestation.is_empty()
            || internal_validation_receipt.is_empty()
            || benchmark.is_empty()
            || review.is_empty()
            || promotion.is_empty()
            || promotion.len()
                > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4
        {
            return Err(
                "Kagemusha V4 install requires every authenticated release input".to_owned(),
            );
        }
        if env
            .get_array_length(&handles)
            .map_err(|error| format!("failed to read Kagemusha V4 handles: {error}"))?
            != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4 as i32
        {
            return Err("Kagemusha V4 install requires exactly eight handles".to_owned());
        }
        let mut jni_handles = [0_i64; KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4];
        env.get_long_array_region(&handles, 0, &mut jni_handles)
            .map_err(|error| format!("failed to read Kagemusha V4 handles: {error}"))?;
        let mut native_handles = [0_u64; KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4];
        for (native, handle) in native_handles.iter_mut().zip(jni_handles) {
            *native = u64::try_from(handle)
                .ok()
                .filter(|handle| *handle != 0)
                .ok_or_else(|| "Kagemusha V4 artifact handles must be positive".to_owned())?;
        }
        let lengths = [
            manifest.len(),
            policy.len(),
            attestation.len(),
            internal_validation_receipt.len(),
            benchmark.len(),
            review.len(),
            promotion.len(),
        ]
        .map(c_ulong::try_from);
        let [
            manifest_len,
            policy_len,
            attestation_len,
            internal_validation_receipt_len,
            benchmark_len,
            review_len,
            promotion_len,
        ] = lengths;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_artifact_set_install_v4(
                manifest.as_ptr(),
                manifest_len.map_err(|_| "V4 manifest exceeds native range".to_owned())?,
                manifest_digest.as_ptr(),
                32,
                policy.as_ptr(),
                policy_len.map_err(|_| "V4 policy exceeds native range".to_owned())?,
                attestation.as_ptr(),
                attestation_len.map_err(|_| "V4 attestation exceeds native range".to_owned())?,
                internal_validation_receipt.as_ptr(),
                internal_validation_receipt_len.map_err(|_| {
                    "V4 internal-validation receipt exceeds native range".to_owned()
                })?,
                benchmark.as_ptr(),
                benchmark_len.map_err(|_| "V4 benchmark exceeds native range".to_owned())?,
                review.as_ptr(),
                review_len.map_err(|_| "V4 review exceeds native range".to_owned())?,
                promotion.as_ptr(),
                promotion_len.map_err(|_| "V4 promotion record exceeds native range".to_owned())?,
                native_handles.as_ptr(),
                KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4 as c_ulong,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "Kagemusha V4 artifact-set install rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, message);
    }
}
pub(super) fn java_native_kagemusha_artifact_set_is_installed_v4(
    env: &mut jni::JNIEnv<'_>,
    manifest_norito: jni::objects::JByteArray<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    let result = (|| -> Result<bool, String> {
        let manifest = read_java_byte_array_bounded(
            env,
            &manifest_norito,
            "manifestNorito",
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_MANIFEST_BYTES_V4 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha V4 manifest".to_owned())?;
        let digest = read_java_byte_array_bounded(
            env,
            &manifest_sha256,
            "manifestSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .filter(|digest| digest.len() == KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4)
        .ok_or_else(|| "invalid Kagemusha V4 manifest digest".to_owned())?;
        let mut installed = 0_u8;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4(
                manifest.as_ptr(),
                c_ulong::try_from(manifest.len())
                    .map_err(|_| "V4 manifest exceeds native range".to_owned())?,
                digest.as_ptr(),
                32,
                &mut installed,
            )
        };
        if status != 0 || installed > 1 {
            return Err(format!(
                "Kagemusha V4 installed check rejected with native status {status}"
            ));
        }
        Ok(installed == 1)
    })();
    match result {
        Ok(true) => jni::sys::JNI_TRUE,
        Ok(false) => jni::sys::JNI_FALSE,
        Err(message) => {
            throw_java_illegal_state(env, message);
            jni::sys::JNI_FALSE
        }
    }
}
pub(super) fn java_native_kagemusha_installed_manifest_sha256_v4(
    env: &mut jni::JNIEnv<'_>,
) -> jni::sys::jbyteArray {
    let mut digest = [0_u8; 32];
    let status = unsafe {
        connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4(
            digest.as_mut_ptr(),
            digest.len() as c_ulong,
        )
    };
    if status != 0 {
        throw_java_illegal_state(
            env,
            format!("Kagemusha V4 installed release is unavailable (native status {status})"),
        );
        return std::ptr::null_mut();
    }
    match env.byte_array_from_slice(&digest) {
        Ok(array) => array.into_raw(),
        Err(error) => {
            throw_java_illegal_state(
                env,
                format!("failed to copy Kagemusha V4 manifest digest: {error}"),
            );
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_kagemusha_build_artifact_binding_v4(
    env: &mut jni::JNIEnv<'_>,
    manifest_norito: jni::objects::JByteArray<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "V4 artifact binding", |env| {
        let manifest_bytes = read_java_byte_array_bounded(
            env,
            &manifest_norito,
            "manifestNorito",
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_MANIFEST_BYTES_V4 as usize,
        )
        .ok_or_else(|| "manifestNorito is invalid".to_owned())?;
        let manifest = decode_canonical_kagemusha_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV4,
        >(&manifest_bytes)
        .map_err(|_| "manifestNorito is not canonical V4".to_owned())?;
        manifest
            .validate()
            .map_err(|_| "manifestNorito is invalid".to_owned())?;
        let digest = read_java_byte_array_bounded(
            env,
            &manifest_sha256,
            "manifestSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .filter(|bytes| bytes.len() == KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4)
        .ok_or_else(|| "manifestSha256 must contain exactly 32 bytes".to_owned())?;
        let digest: [u8; 32] = digest
            .try_into()
            .map_err(|_| "manifestSha256 must contain exactly 32 bytes".to_owned())?;
        if digest == [0; 32] || digest != <[u8; 32]>::from(Sha256::digest(&manifest_bytes)) {
            return Err("manifestSha256 does not identify manifestNorito".to_owned());
        }
        let binding = iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4 {
            version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: manifest.generation,
            manifest_sha256: digest,
        };
        binding
            .validate()
            .map_err(|_| "derived V4 artifact binding is invalid".to_owned())?;
        let archive = norito::to_bytes(&binding)
            .map_err(|error| format!("failed to encode V4 artifact binding: {error}"))?;
        env.byte_array_from_slice(&archive)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_native_kagemusha_artifact_set_uninstall_v4(
    env: &mut jni::JNIEnv<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
) {
    let result = (|| -> Result<(), String> {
        let digest = read_java_byte_array_bounded(
            env,
            &manifest_sha256,
            "manifestSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .filter(|digest| digest.len() == KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4)
        .ok_or_else(|| "invalid Kagemusha V4 manifest digest".to_owned())?;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4(digest.as_ptr(), 32)
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "Kagemusha V4 uninstall rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, message);
    }
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_artifact_begin_v4(
    env: &mut jni::JNIEnv<'_>,
    candidate_norito: jni::objects::JByteArray<'_>,
    candidate_sha256: jni::objects::JByteArray<'_>,
    artifact_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jlong {
    let result = (|| -> Result<jni::sys::jlong, String> {
        let candidate = read_java_byte_array_bounded(
            env,
            &candidate_norito,
            "candidateRecordNorito",
            KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_MAX_CANDIDATE_BYTES_V4 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha candidate record bytes".to_owned())?;
        let candidate_digest = read_java_byte_array_bounded(
            env,
            &candidate_sha256,
            "candidateRecordSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .filter(|digest| digest.len() == KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4)
        .ok_or_else(|| "candidateRecordSha256 must contain 32 bytes".to_owned())?;
        let artifact_digest = read_java_byte_array_bounded(
            env,
            &artifact_sha256,
            "artifactSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .filter(|digest| digest.len() == KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4)
        .ok_or_else(|| "artifactSha256 must contain 32 bytes".to_owned())?;
        let mut handle = 0_u64;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4(
                candidate.as_ptr(),
                c_ulong::try_from(candidate.len())
                    .map_err(|_| "candidate record exceeds native range".to_owned())?,
                candidate_digest.as_ptr(),
                32,
                artifact_digest.as_ptr(),
                32,
                &mut handle,
            )
        };
        if status != 0 || handle == 0 {
            return Err(format!(
                "candidate-lab artifact begin rejected with native status {status}"
            ));
        }
        i64::try_from(handle).map_err(|_| "candidate-lab handle exceeds JNI range".to_owned())
    })();
    match result {
        Ok(handle) => handle,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            0
        }
    }
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_artifact_write_v4(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
    chunk: jni::objects::JByteArray<'_>,
) {
    let result = (|| -> Result<(), String> {
        let handle = u64::try_from(handle)
            .ok()
            .filter(|handle| *handle != 0)
            .ok_or_else(|| "candidate-lab artifact handle must be positive".to_owned())?;
        let chunk = read_java_byte_array_bounded(
            env,
            &chunk,
            "chunk",
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_JNI_MAX_CHUNK_BYTES_V4,
        )
        .ok_or_else(|| "candidate-lab artifact chunk is invalid".to_owned())?;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_write_v4(
                handle,
                chunk.as_ptr(),
                c_ulong::try_from(chunk.len())
                    .map_err(|_| "candidate-lab chunk exceeds native range".to_owned())?,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "candidate-lab artifact write rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, message);
    }
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_artifact_finish_v4(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
    cancel: bool,
) {
    let result = (|| -> Result<(), String> {
        let handle = u64::try_from(handle)
            .ok()
            .filter(|handle| *handle != 0)
            .ok_or_else(|| "candidate-lab artifact handle must be positive".to_owned())?;
        let status = if cancel {
            connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_cancel_v4(handle)
        } else {
            connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_finalize_v4(handle)
        };
        if status == 0 {
            Ok(())
        } else {
            let operation = if cancel { "cancel" } else { "finalize" };
            Err(format!(
                "candidate-lab artifact {operation} rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, message);
    }
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_artifact_set_install_v4(
    env: &mut jni::JNIEnv<'_>,
    candidate_norito: jni::objects::JByteArray<'_>,
    candidate_sha256: jni::objects::JByteArray<'_>,
    handles: jni::objects::JLongArray<'_>,
) {
    let result = (|| -> Result<(), String> {
        let _permit = try_preacquire_kagemusha_heavy_proof_permit_v4().map_err(|_| {
            "candidate-lab Kagemusha artifact-set install is busy; retry after the active proof completes"
                .to_owned()
        })?;
        let candidate = read_java_byte_array_bounded(
            env,
            &candidate_norito,
            "candidateRecordNorito",
            KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_MAX_CANDIDATE_BYTES_V4 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha candidate record".to_owned())?;
        let digest = read_java_byte_array_bounded(
            env,
            &candidate_sha256,
            "candidateRecordSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .filter(|digest| digest.len() == KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4)
        .ok_or_else(|| "candidateRecordSha256 must contain 32 bytes".to_owned())?;
        if env
            .get_array_length(&handles)
            .map_err(|error| format!("failed to read candidate-lab handles: {error}"))?
            != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4 as i32
        {
            return Err("candidate-lab install requires exactly eight ordered handles".to_owned());
        }
        let mut jni_handles = [0_i64; KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4];
        env.get_long_array_region(&handles, 0, &mut jni_handles)
            .map_err(|error| format!("failed to read candidate-lab handles: {error}"))?;
        let mut native_handles = [0_u64; KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4];
        for (native, handle) in native_handles.iter_mut().zip(jni_handles) {
            *native = u64::try_from(handle)
                .ok()
                .filter(|handle| *handle != 0)
                .ok_or_else(|| "candidate-lab handles must be positive".to_owned())?;
        }
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_install_v4(
                candidate.as_ptr(),
                c_ulong::try_from(candidate.len())
                    .map_err(|_| "candidate record exceeds native range".to_owned())?,
                digest.as_ptr(),
                32,
                native_handles.as_ptr(),
                KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4 as c_ulong,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "candidate-lab install rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, message);
    }
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_artifact_set_is_installed_v4(
    env: &mut jni::JNIEnv<'_>,
    candidate_norito: jni::objects::JByteArray<'_>,
    candidate_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    let result = (|| -> Result<bool, String> {
        let candidate = read_java_byte_array_bounded(
            env,
            &candidate_norito,
            "candidateRecordNorito",
            KAGEMUSHA_CANDIDATE_EVIDENCE_LAB_MAX_CANDIDATE_BYTES_V4 as usize,
        )
        .ok_or_else(|| "invalid Kagemusha candidate record".to_owned())?;
        let digest = read_java_byte_array_bounded(
            env,
            &candidate_sha256,
            "candidateRecordSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .filter(|digest| digest.len() == KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4)
        .ok_or_else(|| "candidateRecordSha256 must contain 32 bytes".to_owned())?;
        let mut installed = 0_u8;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_is_installed_v4(
                candidate.as_ptr(),
                c_ulong::try_from(candidate.len())
                    .map_err(|_| "candidate record exceeds native range".to_owned())?,
                digest.as_ptr(),
                32,
                &mut installed,
            )
        };
        if status != 0 || installed > 1 {
            return Err(format!(
                "candidate-lab installed check rejected with native status {status}"
            ));
        }
        Ok(installed == 1)
    })();
    match result {
        Ok(true) => jni::sys::JNI_TRUE,
        Ok(false) => jni::sys::JNI_FALSE,
        Err(message) => {
            throw_java_illegal_state(env, message);
            jni::sys::JNI_FALSE
        }
    }
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_accepted_identity_v4(
    env: &mut jni::JNIEnv<'_>,
) -> jni::sys::jobjectArray {
    let result = (|| -> Result<jni::sys::jobjectArray, String> {
        let installed = require_kagemusha_candidate_evidence_lab_installed_v4()
            .map_err(|_| "candidate-lab identity is unavailable".to_owned())?;
        let expected = build_kagemusha_candidate_evidence_lab_accepted_identity_v4(
            &installed.candidate,
            installed.candidate_sha256,
            installed.manifest_sha256,
        )
        .map_err(|_| "candidate-lab identity failed revalidation".to_owned())?;
        if expected != installed.accepted_identity
            || expected.production_capability_observed
            || expected.source_repo_dirty
            || expected.artifacts.len() != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4
        {
            return Err("candidate-lab identity changed after installation".to_owned());
        }
        let mut fields = Vec::with_capacity(49);
        fields.extend([
            expected.candidate_record_sha256.to_vec(),
            expected.candidate_manifest_sha256.to_vec(),
            vec![u8::from(expected.production_capability_observed)],
            expected.native_accepted_inventory_sha256.to_vec(),
            expected.generation.into_bytes(),
            expected.source_commit.into_bytes(),
            expected.source_tree_sha256.to_vec(),
            vec![u8::from(expected.source_repo_dirty)],
            expected.bridge_abi_version.to_string().into_bytes(),
        ]);
        for artifact in expected.artifacts {
            fields.extend([
                artifact.role.into_bytes(),
                artifact.framed_size_bytes.to_string().into_bytes(),
                artifact.framed_sha256.to_vec(),
                artifact.payload_size_bytes.to_string().into_bytes(),
                artifact.payload_sha256.to_vec(),
            ]);
        }
        if fields.len() != 49 {
            return Err("candidate-lab identity projection has the wrong arity".to_owned());
        }
        java_kagemusha_byte_arrays(env, &fields)
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_state(env, message);
            std::ptr::null_mut()
        }
    }
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_artifact_set_uninstall_v4(
    env: &mut jni::JNIEnv<'_>,
    candidate_sha256: jni::objects::JByteArray<'_>,
) {
    let result = (|| -> Result<(), String> {
        let digest = read_java_byte_array_bounded(
            env,
            &candidate_sha256,
            "candidateRecordSha256",
            KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4,
        )
        .filter(|digest| digest.len() == KAGEMUSHA_RECURSIVE_SPEND_SHA256_BYTES_V4)
        .ok_or_else(|| "candidateRecordSha256 must contain 32 bytes".to_owned())?;
        let status = unsafe {
            connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_uninstall_v4(
                digest.as_ptr(),
                32,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "candidate-lab uninstall rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, message);
    }
}
pub(super) const KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V2: usize =
    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2;
pub(super) const KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V4: usize =
    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4;
pub(super) const KAGEMUSHA_JNI_LIFECYCLE_RESULT_MAX_BYTES_V4: usize =
    KAGEMUSHA_RECURSIVE_SPEND_LIFECYCLE_RESULT_MAX_BYTES_V4;
pub(super) enum JavaKagemushaLifecycleFailure {
    Invalid(String),
    Unavailable(String),
}
pub(super) fn java_kagemusha_lifecycle_status(
    label: &str,
    status: c_int,
) -> JavaKagemushaLifecycleFailure {
    match status {
        ERR_KAGEMUSHA_PROVE => JavaKagemushaLifecycleFailure::Invalid(format!(
            "Kagemusha {label} request or proof binding was rejected"
        )),
        ERR_KAGEMUSHA_BUSY => JavaKagemushaLifecycleFailure::Unavailable(format!(
            "Kagemusha {label} is busy; retry after the active proof completes"
        )),
        ERR_KAGEMUSHA_RECURSIVE_SPEND_V4_UNAVAILABLE => JavaKagemushaLifecycleFailure::Unavailable(
            format!("Kagemusha {label} V4 proof backend is unavailable"),
        ),
        ERR_KAGEMUSHA_RECURSIVE_SPEND_V4_ARTIFACT => {
            JavaKagemushaLifecycleFailure::Unavailable(format!(
                "Kagemusha {label} V4 artifact set is unavailable or does not match the request"
            ))
        }
        _ => JavaKagemushaLifecycleFailure::Unavailable(format!(
            "Kagemusha {label} failed with native status {status}"
        )),
    }
}
pub(super) fn java_native_kagemusha_lifecycle_archive_bounded<F>(
    env: &mut jni::JNIEnv<'_>,
    request_norito: jni::objects::JByteArray<'_>,
    label: &str,
    request_max_bytes: usize,
    result_max_bytes: usize,
    invoke: F,
) -> jni::sys::jbyteArray
where
    F: FnOnce(*const c_uchar, c_ulong, *mut *mut c_uchar, *mut c_ulong) -> c_int,
{
    let result = (|| -> Result<jni::sys::jbyteArray, JavaKagemushaLifecycleFailure> {
        let _permit = try_preacquire_kagemusha_heavy_proof_permit_v4()
            .map_err(|error| java_kagemusha_lifecycle_status(label, error.code()))?;
        let request = Zeroizing::new(
            read_java_byte_array_bounded(env, &request_norito, "requestNorito", request_max_bytes)
                .ok_or_else(|| {
                    JavaKagemushaLifecycleFailure::Invalid(format!(
                        "Kagemusha {label} request must be a byte array"
                    ))
                })?,
        );
        let request_len = c_ulong::try_from(request.len()).map_err(|_| {
            JavaKagemushaLifecycleFailure::Invalid(format!(
                "Kagemusha {label} request length exceeds the native range"
            ))
        })?;
        let mut output = std::ptr::null_mut();
        let mut output_len = 0;
        let status = invoke(request.as_ptr(), request_len, &mut output, &mut output_len);
        if status != 0 {
            if !output.is_null() {
                connect_norito_free(output);
            }
            return Err(java_kagemusha_lifecycle_status(label, status));
        }
        let output_length = usize::try_from(output_len).map_err(|_| {
            if !output.is_null() {
                connect_norito_free(output);
            }
            JavaKagemushaLifecycleFailure::Unavailable(format!(
                "Kagemusha {label} result length exceeds the JVM range"
            ))
        })?;
        if output.is_null() || output_length == 0 || output_length > result_max_bytes {
            if !output.is_null() {
                connect_norito_free(output);
            }
            return Err(JavaKagemushaLifecycleFailure::Unavailable(format!(
                "Kagemusha {label} returned an invalid result archive"
            )));
        }
        let java_archive = unsafe {
            let bytes = std::slice::from_raw_parts(output, output_length);
            env.byte_array_from_slice(bytes)
        };
        connect_norito_free(output);
        java_archive
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| {
                JavaKagemushaLifecycleFailure::Unavailable(format!(
                    "Kagemusha {label} result could not be copied to the JVM: {error}"
                ))
            })
    })();
    match result {
        Ok(archive) => archive,
        Err(JavaKagemushaLifecycleFailure::Invalid(message)) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
        Err(JavaKagemushaLifecycleFailure::Unavailable(message)) => {
            throw_java_illegal_state(env, message);
            std::ptr::null_mut()
        }
    }
}
pub(super) fn java_native_kagemusha_lifecycle_archive_v4<F>(
    env: &mut jni::JNIEnv<'_>,
    request_norito: jni::objects::JByteArray<'_>,
    label: &str,
    request_max_bytes: usize,
    invoke: F,
) -> jni::sys::jbyteArray
where
    F: FnOnce(*const c_uchar, c_ulong, *mut *mut c_uchar, *mut c_ulong) -> c_int,
{
    java_native_kagemusha_lifecycle_archive_bounded(
        env,
        request_norito,
        label,
        request_max_bytes,
        KAGEMUSHA_JNI_LIFECYCLE_RESULT_MAX_BYTES_V4,
        invoke,
    )
}
pub(super) fn java_native_kagemusha_append_spend_v4(
    env: &mut jni::JNIEnv<'_>,
    request_norito: jni::objects::JByteArray<'_>,
    recipient_request_norito: jni::objects::JByteArray<'_>,
    verified_at_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let _permit = match try_preacquire_kagemusha_heavy_proof_permit_v4() {
        Ok(permit) => permit,
        Err(error) => {
            let message = match java_kagemusha_lifecycle_status("V4 append spend", error.code()) {
                JavaKagemushaLifecycleFailure::Invalid(message)
                | JavaKagemushaLifecycleFailure::Unavailable(message) => message,
            };
            throw_java_illegal_state(env, message);
            return std::ptr::null_mut();
        }
    };
    let recipient = match read_java_byte_array_bounded(
        env,
        &recipient_request_norito,
        "recipientRequestNorito",
        KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V2,
    ) {
        Some(bytes) => bytes,
        _ => {
            throw_java_illegal_argument(
                env,
                format!(
                    "recipientRequestNorito must contain 1..{KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V2} bytes"
                ),
            );
            return std::ptr::null_mut();
        }
    };
    let verified_at_ms = match u64::try_from(verified_at_ms)
        .ok()
        .filter(|value| *value != 0)
    {
        Some(value) => value,
        None => {
            throw_java_illegal_argument(env, "verifiedAtMilliseconds must be positive".to_owned());
            return std::ptr::null_mut();
        }
    };
    let recipient_len = match c_ulong::try_from(recipient.len()) {
        Ok(length) => length,
        Err(_) => {
            throw_java_illegal_argument(
                env,
                "recipientRequestNorito length exceeds the native range".to_owned(),
            );
            return std::ptr::null_mut();
        }
    };
    java_native_kagemusha_lifecycle_archive_v4(
        env,
        request_norito,
        "V4 append spend",
        KAGEMUSHA_RECURSIVE_SPEND_APPEND_LOCAL_MAX_BYTES_V4,
        |request_ptr, request_len, output, output_len| unsafe {
            connect_norito_kagemusha_recursive_spend_append_v4(
                request_ptr,
                request_len,
                recipient.as_ptr(),
                recipient_len,
                verified_at_ms,
                output,
                output_len,
            )
        },
    )
}
#[cfg(all(
    feature = "kagemusha-candidate-evidence-lab",
    any(
        target_os = "android",
        target_os = "linux",
        target_os = "macos",
        target_os = "windows"
    )
))]
pub(super) fn java_native_kagemusha_candidate_lab_append_v4(
    env: &mut jni::JNIEnv<'_>,
    request_norito: jni::objects::JByteArray<'_>,
    recipient_request_norito: jni::objects::JByteArray<'_>,
    verified_at_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let _permit = match try_preacquire_kagemusha_heavy_proof_permit_v4() {
        Ok(permit) => permit,
        Err(error) => {
            let message =
                match java_kagemusha_lifecycle_status("candidate-lab V4 append", error.code()) {
                    JavaKagemushaLifecycleFailure::Invalid(message)
                    | JavaKagemushaLifecycleFailure::Unavailable(message) => message,
                };
            throw_java_illegal_state(env, message);
            return std::ptr::null_mut();
        }
    };
    let recipient = match read_java_byte_array_bounded(
        env,
        &recipient_request_norito,
        "recipientRequestNorito",
        KAGEMUSHA_JNI_PEER_REQUEST_MAX_BYTES_V2,
    ) {
        Some(bytes) => bytes,
        None => {
            throw_java_illegal_argument(
                env,
                "candidate-lab recipient request is invalid".to_owned(),
            );
            return std::ptr::null_mut();
        }
    };
    let verified_at_ms = match u64::try_from(verified_at_ms)
        .ok()
        .filter(|value| *value != 0)
    {
        Some(value) => value,
        None => {
            throw_java_illegal_argument(env, "verifiedAtMilliseconds must be positive".to_owned());
            return std::ptr::null_mut();
        }
    };
    let recipient_len = match c_ulong::try_from(recipient.len()) {
        Ok(length) => length,
        Err(_) => {
            throw_java_illegal_argument(
                env,
                "candidate-lab recipient request exceeds native range".to_owned(),
            );
            return std::ptr::null_mut();
        }
    };
    java_native_kagemusha_lifecycle_archive_v4(
        env,
        request_norito,
        "candidate-lab V4 append",
        KAGEMUSHA_RECURSIVE_SPEND_APPEND_LOCAL_MAX_BYTES_V4,
        |request_ptr, request_len, output, output_len| unsafe {
            connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4(
                request_ptr,
                request_len,
                recipient.as_ptr(),
                recipient_len,
                verified_at_ms,
                output,
                output_len,
            )
        },
    )
}
pub(super) fn java_kagemusha_byte_arrays(
    env: &mut jni::JNIEnv<'_>,
    fields: &[Vec<u8>],
) -> Result<jni::sys::jobjectArray, String> {
    let byte_array_class = env.find_class("[B").map_err(|error| error.to_string())?;
    let length = i32::try_from(fields.len()).map_err(|_| "too many Kagemusha result fields")?;
    let result = env
        .new_object_array(length, byte_array_class, jni::objects::JObject::null())
        .map_err(|error| error.to_string())?;
    for (index, field) in fields.iter().enumerate() {
        let value = env
            .byte_array_from_slice(field)
            .map_err(|error| error.to_string())?;
        env.set_object_array_element(&result, index as i32, value)
            .map_err(|error| error.to_string())?;
    }
    Ok(result.into_raw())
}
pub(super) fn zeroize_java_kagemusha_result_fields(fields: &mut [Vec<u8>]) {
    for field in fields {
        field.zeroize();
    }
}
pub(super) fn java_kagemusha_secret_byte_arrays(
    env: &mut jni::JNIEnv<'_>,
    fields: &mut [Vec<u8>],
) -> Result<jni::sys::jobjectArray, String> {
    let result = java_kagemusha_byte_arrays(env, fields);
    zeroize_java_kagemusha_result_fields(fields);
    result
}
pub(super) fn java_kagemusha_text(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JByteArray<'_>,
    field: &str,
) -> Result<String, String> {
    let bytes = read_java_byte_array(env, value, field)
        .ok_or_else(|| format!("{field} must be UTF-8 bytes"))?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{field} must be UTF-8"))?;
    if text.is_empty()
        || text.len() > 512
        || text.trim() != text
        || text.chars().any(char::is_control)
    {
        return Err(format!("{field} must be canonical non-empty text"));
    }
    Ok(text)
}
pub(super) fn java_kagemusha_fixed32(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JByteArray<'_>,
    field: &str,
) -> Result<[u8; 32], String> {
    let bytes = read_java_byte_array(env, value, field)
        .ok_or_else(|| format!("{field} must be a byte array"))?;
    let fixed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("{field} must contain exactly 32 bytes"))?;
    if fixed == [0; 32] {
        return Err(format!("{field} must be non-zero"));
    }
    Ok(fixed)
}
pub(super) fn java_kagemusha_fixed32_sensitive(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JByteArray<'_>,
    field: &str,
) -> Result<Zeroizing<[u8; 32]>, String> {
    let bytes = Zeroizing::new(
        read_java_byte_array(env, value, field)
            .ok_or_else(|| format!("{field} must be a byte array"))?,
    );
    if bytes.len() != 32 {
        return Err(format!("{field} must contain exactly 32 bytes"));
    }
    let mut fixed = Zeroizing::new([0_u8; 32]);
    fixed.copy_from_slice(bytes.as_slice());
    if *fixed == [0; 32] {
        return Err(format!("{field} must be non-zero"));
    }
    Ok(fixed)
}
pub(super) fn java_kagemusha_note_opening_v2(
    env: &mut jni::JNIEnv<'_>,
    spend_key: &jni::objects::JByteArray<'_>,
    rho: &jni::objects::JByteArray<'_>,
    diversifier: &jni::objects::JByteArray<'_>,
) -> Result<KagemushaNoteOpeningV2, String> {
    let spend_key = java_kagemusha_fixed32_sensitive(env, spend_key, "spendKey")?;
    let rho = java_kagemusha_fixed32_sensitive(env, rho, "rho")?;
    let diversifier = java_kagemusha_fixed32_sensitive(env, diversifier, "diversifier")?;
    let opening = KagemushaNoteOpeningV2 {
        spend_key: *spend_key,
        rho: *rho,
        diversifier: *diversifier,
    };
    opening
        .validate()
        .map_err(|_| "note opening is invalid".to_owned())?;
    Ok(opening)
}
pub(super) fn java_kagemusha_amount(
    env: &mut jni::JNIEnv<'_>,
    atomic_units: &jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
) -> Result<iroha_data_model::offline::KagemushaScaledAmountV2, String> {
    let atomic_text = java_kagemusha_text(env, atomic_units, "atomicUnits")?;
    if atomic_text.len() > 39
        || !atomic_text.bytes().all(|byte| byte.is_ascii_digit())
        || atomic_text.starts_with('0')
    {
        return Err("atomicUnits must be a canonical positive u128 decimal".to_owned());
    }
    let atomic = atomic_text
        .parse::<u128>()
        .map_err(|_| "atomicUnits must fit in u128".to_owned())?;
    let scale = u32::try_from(scale).map_err(|_| "scale must be non-negative".to_owned())?;
    iroha_data_model::offline::KagemushaScaledAmountV2::new(atomic, scale)
        .map_err(|_| "amount or scale is outside the Kagemusha domain".to_owned())
}
pub(super) fn java_kagemusha_archive_array_result<T>(
    env: &mut jni::JNIEnv<'_>,
    label: &str,
    body: impl FnOnce(&mut jni::JNIEnv<'_>) -> Result<T, String>,
) -> T
where
    T: Default,
{
    match body(env) {
        Ok(value) => value,
        Err(message) => {
            throw_java_illegal_argument(env, format!("Kagemusha {label}: {message}"));
            T::default()
        }
    }
}
pub(super) fn java_native_kagemusha_branch_claims_conflict_v2(
    env: &mut jni::JNIEnv<'_>,
    left: jni::objects::JByteArray<'_>,
    right: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    java_kagemusha_archive_array_result(env, "branch claim comparison", |env| {
        let left = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendBranchClaimV2,
        >(env, &left, "leftBranchClaim")?;
        let right = java_kagemusha_decode_archive::<
            iroha_data_model::offline::KagemushaRecursiveSpendBranchClaimV2,
        >(env, &right, "rightBranchClaim")?;
        Ok(if kagemusha_branch_claims_conflict_v2(&left, &right)? {
            jni::sys::JNI_TRUE
        } else {
            jni::sys::JNI_FALSE
        })
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_prepare_recipient_request_v2(
    env: &mut jni::JNIEnv<'_>,
    network_id: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    asset: jni::objects::JByteArray<'_>,
    atomic_units: jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
    recipient: jni::objects::JByteArray<'_>,
    receiver_device_id: jni::objects::JByteArray<'_>,
    receiver_public_key: jni::objects::JByteArray<'_>,
    request_id: jni::objects::JByteArray<'_>,
    issued_at_ms: jni::sys::jlong,
    expires_at_ms: jni::sys::jlong,
    spend_key: jni::objects::JByteArray<'_>,
    rho: jni::objects::JByteArray<'_>,
    diversifier: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "recipient request preparation", |env| {
        let chain_discriminant = u16::try_from(chain_discriminant)
            .map_err(|_| "chainDiscriminant must fit in u16".to_owned())?;
        let network_id = java_network_id(env, &network_id)?;
        let asset = parse_asset_definition(java_kagemusha_text(env, &asset, "asset")?)
            .map_err(|_| "asset must be a canonical asset-definition address".to_owned())?;
        let amount = java_kagemusha_amount(env, &atomic_units, scale)?;
        let recipient = parse_account_id_for_chain(
            java_kagemusha_text(env, &recipient, "recipient")?,
            chain_discriminant,
        )
        .map_err(|_| "recipient must be a canonical account address".to_owned())?;
        let receiver_device_id = java_kagemusha_text(env, &receiver_device_id, "receiverDeviceId")?;
        let receiver_public_key_bytes =
            read_java_byte_array(env, &receiver_public_key, "receiverPublicKey")
                .ok_or_else(|| "receiverPublicKey must be bytes".to_owned())?;
        let receiver_public_key =
            iroha_data_model::offline::KagemushaDevicePublicKeyV2::from_sec1_bytes(
                &receiver_public_key_bytes,
            )
            .map_err(|_| "receiverPublicKey must be an uncompressed P-256 point".to_owned())?;
        let request_id = java_kagemusha_fixed32(env, &request_id, "requestId")?;
        let issued_at_ms = u64::try_from(issued_at_ms)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "issuedAtMilliseconds must be positive".to_owned())?;
        let expires_at_ms = u64::try_from(expires_at_ms)
            .ok()
            .filter(|value| *value > issued_at_ms)
            .ok_or_else(|| "expiresAtMilliseconds must follow issuance".to_owned())?;
        let mut opening = java_kagemusha_note_opening_v2(env, &spend_key, &rho, &diversifier)?;
        let derivation_request =
            iroha_data_model::offline::KagemushaRecipientOutputDerivationRequestV2 {
                network_id,
                asset: asset.clone(),
                amount,
                request_id,
            };
        derivation_request
            .validate()
            .map_err(|_| "recipient output derivation fields are invalid".to_owned())?;
        let derivation_archive = norito::to_bytes(&derivation_request)
            .map_err(|error| format!("failed to encode derivation request: {error}"))?;
        let opening_archive = Zeroizing::new(
            norito::to_bytes(&opening)
                .map_err(|error| format!("failed to encode note opening: {error}"))?,
        );
        let derivation =
            kagemusha_recipient_output_derive_v2(&derivation_archive, opening_archive.as_slice())
                .map_err(|_| "recipient output derivation failed".to_owned())?;
        let recipient_key_reference =
            iroha_data_model::offline::kagemusha_receiver_key_reference_v2(&receiver_public_key)
                .map_err(|_| "receiver key reference derivation failed".to_owned())?;
        let payload = iroha_data_model::offline::KagemushaRecipientPaymentRequestSigningPayloadV2 {
            network_id,
            asset,
            amount,
            recipient,
            recipient_key_reference,
            receiver_device_id,
            receiver_public_key,
            request_id,
            issued_at_ms,
            expires_at_ms,
            recipient_output: derivation.recipient_output.clone(),
            sender_output_prover_material: derivation.sender_output_prover_material,
        };
        payload
            .validate_public_binding()
            .map_err(|_| "recipient request payload is invalid".to_owned())?;
        let payload_archive = norito::to_bytes(&payload)
            .map_err(|error| format!("failed to encode recipient payload: {error}"))?;
        let signing_bytes = payload
            .signing_bytes()
            .map_err(|_| "failed to derive recipient signing bytes".to_owned())?;
        let local_opening = opening_archive.to_vec();
        opening.zeroize();
        let mut fields = [
            payload_archive,
            signing_bytes,
            local_opening,
            derivation.recipient_output.note_commitment.to_vec(),
            derivation.recipient_output.spend_nullifier.to_vec(),
        ];
        java_kagemusha_secret_byte_arrays(env, &mut fields)
    })
}
pub(super) fn java_native_kagemusha_prepare_note_opening_v2(
    env: &mut jni::JNIEnv<'_>,
    spend_key: jni::objects::JByteArray<'_>,
    rho: jni::objects::JByteArray<'_>,
    diversifier: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "note opening preparation", |env| {
        let mut opening = java_kagemusha_note_opening_v2(env, &spend_key, &rho, &diversifier)?;
        let archive = Zeroizing::new(
            norito::to_bytes(&opening)
                .map_err(|error| format!("failed to encode note opening: {error}"))?,
        );
        opening.zeroize();
        env.byte_array_from_slice(archive.as_slice())
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_prepare_redemption_change_v5(
    env: &mut jni::JNIEnv<'_>,
    bundle: jni::objects::JByteArray<'_>,
    input_opening: jni::objects::JByteArray<'_>,
    atomic_units: jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
    recipient: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    nonce: jni::objects::JByteArray<'_>,
    entropy: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "V5 redemption change preparation", |env| {
        let bundle = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
        >(
            env,
            &bundle,
            "bundle",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
        )?;
        let input_opening = JavaKagemushaSensitiveOpeningV2 {
            value: java_kagemusha_decode_sensitive_archive::<KagemushaNoteOpeningV2>(
                env,
                &input_opening,
                "inputOpening",
            )?,
        };
        let change_amount = java_kagemusha_amount(env, &atomic_units, scale)?;
        let chain_discriminant = u16::try_from(chain_discriminant)
            .map_err(|_| "chainDiscriminant must fit in u16".to_owned())?;
        let recipient = parse_account_id_for_chain(
            java_kagemusha_text(env, &recipient, "recipient")?,
            chain_discriminant,
        )
        .map_err(|_| "recipient must be a canonical account address".to_owned())?;
        let nonce = java_kagemusha_fixed32_sensitive(env, &nonce, "nonce")?;
        let entropy = java_kagemusha_fixed32_sensitive(env, &entropy, "entropy")?;
        if nonce == entropy {
            return Err("entropy must be distinct from nonce".to_owned());
        }
        let operation_id =
            iroha_data_model::offline::derive_kagemusha_operation_id_v4(&recipient, nonce)
                .map_err(|_| "failed to derive redemption operation id".to_owned())?;
        let preparation = prepare_kagemusha_redemption_change_opening_v4(
            &bundle,
            &input_opening.value,
            change_amount,
            &operation_id,
            &entropy,
        )
        .map_err(|_| {
            "redemption change must bind a valid input note, smaller positive amount, recipient, nonce, and fresh entropy"
                .to_owned()
        })?;
        let opening_archive = Zeroizing::new(
            norito::to_bytes(&preparation.opening)
                .map_err(|error| format!("failed to encode redemption change opening: {error}"))?,
        );
        let mut fields = vec![
            opening_archive.to_vec(),
            preparation.opening.rho.to_vec(),
            preparation.opening.diversifier.to_vec(),
            preparation.output.note_commitment.to_vec(),
            preparation.output.spend_nullifier.to_vec(),
            preparation
                .output
                .amount
                .atomic_units
                .to_string()
                .into_bytes(),
            preparation.output.amount.scale.to_string().into_bytes(),
        ];
        java_kagemusha_secret_byte_arrays(env, &mut fields)
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_prepare_peer_split_change_v4(
    env: &mut jni::JNIEnv<'_>,
    bundles: jni::objects::JObjectArray<'_>,
    input_openings: jni::objects::JObjectArray<'_>,
    recipient_request: jni::objects::JByteArray<'_>,
    atomic_units: jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
    operation_id: jni::objects::JByteArray<'_>,
    entropy: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_kagemusha_archive_array_result(env, "V4 peer-split change preparation", |env| {
        let maximum_inputs = iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_INPUTS_V2;
        let bundle_archives = java_kagemusha_byte_array_vector_bounded(
            env,
            &bundles,
            "bundles",
            maximum_inputs,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V4,
        )?;
        let opening_archives = java_kagemusha_byte_array_vector_bounded(
            env,
            &input_openings,
            "inputOpenings",
            maximum_inputs,
            KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_ARCHIVE_BYTES_V2,
        )?;
        if bundle_archives.is_empty() || bundle_archives.len() != opening_archives.len() {
            return Err("bundles and inputOpenings must have the same 1..2 count".to_owned());
        }
        let mut decoded_bundles = Vec::with_capacity(bundle_archives.len());
        let mut decoded_openings = Vec::with_capacity(opening_archives.len());
        for index in 0..bundle_archives.len() {
            decoded_bundles.push(
                decode_canonical_kagemusha_recursive_archive::<
                    iroha_data_model::offline::KagemushaRecursiveSpendBundleV4,
                >(&bundle_archives[index])
                .map_err(|_| format!("bundles[{index}] is not canonical V4"))?,
            );
            decoded_openings.push(
                decode_canonical_kagemusha_sensitive_archive::<KagemushaNoteOpeningV2>(
                    &opening_archives[index],
                )
                .map_err(|_| format!("inputOpenings[{index}] is not canonical"))?,
            );
        }
        let recipient_request = java_kagemusha_decode_archive_bounded::<
            iroha_data_model::offline::KagemushaRecipientPaymentRequestV2,
        >(
            env,
            &recipient_request,
            "recipientRequest",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
        )?;
        let change_amount = java_kagemusha_amount(env, &atomic_units, scale)?;
        let operation_id = java_kagemusha_fixed32_sensitive(env, &operation_id, "operationId")?;
        let entropy = java_kagemusha_fixed32_sensitive(env, &entropy, "entropy")?;
        let preparation = prepare_kagemusha_peer_split_change_opening_v4(
            &decoded_bundles,
            &decoded_openings,
            &recipient_request,
            change_amount,
            &operation_id,
            &entropy,
        )
        .map_err(|_| {
            "peer-split change must bind distinct exact inputs, recipient request, conserved amounts, operation id, and fresh entropy"
                .to_owned()
        })?;
        let opening_archive = Zeroizing::new(
            norito::to_bytes(&preparation.opening)
                .map_err(|error| format!("failed to encode peer-split change opening: {error}"))?,
        );
        let mut fields = vec![
            opening_archive.to_vec(),
            preparation.opening.rho.to_vec(),
            preparation.opening.diversifier.to_vec(),
            preparation.output.note_commitment.to_vec(),
            preparation.output.spend_nullifier.to_vec(),
            preparation
                .output
                .amount
                .atomic_units
                .to_string()
                .into_bytes(),
            preparation.output.amount.scale.to_string().into_bytes(),
        ];
        java_kagemusha_secret_byte_arrays(env, &mut fields)
    })
}
pub(super) fn java_native_kagemusha_create_recipient_request_v2(
    env: &mut jni::JNIEnv<'_>,
    payload: jni::objects::JByteArray<'_>,
    signature: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "recipient request signing", |env| {
        let payload = read_java_byte_array(env, &payload, "payload")
            .ok_or_else(|| "payload must be bytes".to_owned())?;
        let signature = read_java_byte_array(env, &signature, "signature")
            .ok_or_else(|| "signature must be bytes".to_owned())?;
        let request = kagemusha_recipient_payment_request_create_v2(&payload, &signature)
            .map_err(|_| "signature or payload was rejected".to_owned())?;
        let archive = norito::to_bytes(&request)
            .map_err(|error| format!("failed to encode signed request: {error}"))?;
        env.byte_array_from_slice(&archive)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_native_kagemusha_verify_recipient_request_v2(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    verified_at_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "recipient request verification", |env| {
        let request = read_java_byte_array(env, &request, "request")
            .ok_or_else(|| "request must be bytes".to_owned())?;
        let verified_at_ms = u64::try_from(verified_at_ms)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "verifiedAtMilliseconds must be positive".to_owned())?;
        let digest = kagemusha_recipient_payment_request_verify_v2(&request, verified_at_ms)
            .map_err(|_| "request signature, expiry, or binding was rejected".to_owned())?;
        env.byte_array_from_slice(&digest)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
pub(super) fn java_native_kagemusha_create_recipient_lineage_query_v2(
    env: &mut jni::JNIEnv<'_>,
    network_id: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    recipient: jni::objects::JByteArray<'_>,
    receiver_device_id: jni::objects::JByteArray<'_>,
    asset: jni::objects::JByteArray<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_kagemusha_archive_array_result(env, "recipient lineage query creation", |env| {
        let chain_discriminant = u16::try_from(chain_discriminant)
            .map_err(|_| "chainDiscriminant must fit in u16".to_owned())?;
        let network_id = java_network_id(env, &network_id)?;
        let recipient = parse_account_id_for_chain(
            java_kagemusha_text(env, &recipient, "recipient")?,
            chain_discriminant,
        )
        .map_err(|_| "recipient must be a canonical account address".to_owned())?;
        let receiver_device_id = java_kagemusha_text(env, &receiver_device_id, "receiverDeviceId")?;
        let asset = parse_asset_definition(java_kagemusha_text(env, &asset, "asset")?)
            .map_err(|_| "asset must be a canonical asset-definition address".to_owned())?;
        let trusted_checkpoint_height = u64::try_from(trusted_checkpoint_height)
            .ok()
            .filter(|height| *height != 0)
            .ok_or_else(|| "trustedCheckpointHeight must be positive".to_owned())?;
        let query = kagemusha_recipient_lineage_query_create_v2(
            network_id,
            recipient,
            receiver_device_id,
            asset,
            trusted_checkpoint_height,
        )
        .map_err(|_| "selector or trusted checkpoint height was rejected".to_owned())?;
        env.byte_array_from_slice(&query)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| error.to_string())
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn java_native_kagemusha_verify_recipient_registration_lineage_v2(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    lineage: jni::objects::JByteArray<'_>,
    verified_at_ms: jni::sys::jlong,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    let result = (|| -> Result<jni::sys::jobjectArray, String> {
        let request = read_java_byte_array_bounded(
            env,
            &request,
            "request",
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_ARCHIVE_BYTES_V2,
        )
        .ok_or_else(|| "request must be a bounded nonempty archive".to_owned())?;
        let lineage = read_java_byte_array_bounded(
            env,
            &lineage,
            "lineage",
            iroha_torii_shared::offline_api::OFFLINE_RECIPIENT_LINEAGE_MAX_RESPONSE_BYTES,
        )
        .ok_or_else(|| "lineage must be a bounded nonempty archive".to_owned())?;
        let verified_at_ms = u64::try_from(verified_at_ms)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "verifiedAtMilliseconds must be positive".to_owned())?;
        let trusted_checkpoint_height = u64::try_from(trusted_checkpoint_height)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "trustedCheckpointHeight must be positive".to_owned())?;
        let trusted_checkpoint_context_id: [u8; 32] = read_java_byte_array_bounded(
            env,
            &trusted_checkpoint_context_id,
            "trustedCheckpointContextId",
            32,
        )
        .ok_or_else(|| "trustedCheckpointContextId must contain exactly 32 bytes".to_owned())?
        .try_into()
        .map_err(|_| "trustedCheckpointContextId must contain exactly 32 bytes".to_owned())?;
        if trusted_checkpoint_context_id.iter().all(|byte| *byte == 0) {
            return Err("trustedCheckpointContextId must be a non-zero Iroha hash".to_owned());
        }
        let verified = kagemusha_recipient_registration_lineage_verify_v2(
            &request,
            &lineage,
            verified_at_ms,
            trusted_checkpoint_height,
            trusted_checkpoint_context_id,
        )
        .map_err(|_| {
            "receiver lineage request, active-state proofs, policy, or finality chain was rejected"
                .to_owned()
        })?;
        java_byte_array_pair(
            env,
            &verified.lineage_archive,
            &verified.promoted_checkpoint,
        )
    })();
    match result {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, message);
            std::ptr::null_mut()
        }
    }
}
