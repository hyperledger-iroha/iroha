// Kotlin-owned reserve finality JNI. There is no duplicate Java implementation or alias.

fn read_java_reserve_finality_bytes(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JByteArray<'_>,
    maximum: usize,
) -> Option<Vec<u8>> {
    let length = usize::try_from(env.get_array_length(value).ok()?).ok()?;
    if length == 0 || length > maximum {
        return None;
    }
    env.convert_byte_array(value).ok()
}

fn java_reserve_finality_output(
    env: &mut jni::JNIEnv<'_>,
    maximum: usize,
    invoke: impl FnOnce(*mut *mut c_uchar, *mut c_ulong) -> c_int,
) -> jni::sys::jbyteArray {
    let mut output = ptr::null_mut();
    let mut length = 0;
    let status = invoke(&mut output, &mut length);
    let bytes = if status == 0
        && !output.is_null()
        && length > 0
        && usize::try_from(length).is_ok_and(|size| size <= maximum)
    {
        Some(unsafe { slice::from_raw_parts(output, length as usize) }.to_vec())
    } else {
        None
    };
    if !output.is_null() {
        connect_norito_free(output);
    }
    bytes
        .and_then(|value| env.byte_array_from_slice(&value).ok())
        .map_or(ptr::null_mut(), jni::objects::JByteArray::into_raw)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaReserveFinalityJniV1_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaReserveFinalityJniV1_nativeHint(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    response: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(response) = read_java_reserve_finality_bytes(
            &mut env,
            &response,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1,
        ) else {
            return ptr::null_mut();
        };
        java_reserve_finality_output(&mut env, 512, |out, len| unsafe {
            connect_norito_kagemusha_reserve_finality_hint_v1(
                response.as_ptr(),
                response.len() as c_ulong,
                out,
                len,
            )
        })
    }))
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaReserveFinalityJniV1_nativeVerify(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    response: jni::objects::JByteArray<'_>,
    kind: jni::sys::jint,
    request: jni::objects::JByteArray<'_>,
    network: jni::objects::JByteArray<'_>,
    height_bits: jni::sys::jlong,
    context: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let maximum = match kind {
            0 => iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1,
            1 => iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1,
            _ => return ptr::null_mut(),
        };
        let Some(response) = read_java_reserve_finality_bytes(
            &mut env,
            &response,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1,
        ) else {
            return ptr::null_mut();
        };
        let Some(request) = read_java_reserve_finality_bytes(&mut env, &request, maximum) else {
            return ptr::null_mut();
        };
        let Some(network) = read_java_reserve_finality_bytes(&mut env, &network, 32) else {
            return ptr::null_mut();
        };
        let Some(context) = read_java_reserve_finality_bytes(&mut env, &context, 32) else {
            return ptr::null_mut();
        };
        java_reserve_finality_output(
            &mut env,
            if kind == 0 {
                iroha_data_model::kagemusha::KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1
            } else {
                iroha_data_model::kagemusha::KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1
            },
            |out, len| unsafe {
                // Kotlin validates an unsigned 64-bit BigInteger and passes its exact low-64-bit
                // representation. Negative JNI longs represent heights above i64::MAX, not errors.
                connect_norito_kagemusha_reserve_finality_verify_v1(
                    response.as_ptr(),
                    response.len() as c_ulong,
                    kind as u8,
                    request.as_ptr(),
                    request.len() as c_ulong,
                    network.as_ptr(),
                    network.len() as c_ulong,
                    height_bits as u64,
                    context.as_ptr(),
                    context.len() as c_ulong,
                    out,
                    len,
                )
            },
        )
    }))
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaTopUpSubmissionJniV1_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_hyperledger_iroha_sdk_offline_wallet_KagemushaTopUpSubmissionJniV1_nativeValidate(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    signed: jni::objects::JByteArray<'_>,
    expected: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(signed) = read_java_reserve_finality_bytes(
            &mut env,
            &signed,
            DETACHED_TRANSACTION_SCAFFOLD_MAX_BYTES,
        ) else {
            return BridgeError::KagemushaV1.code();
        };
        let Some(expected) = read_java_reserve_finality_bytes(
            &mut env,
            &expected,
            iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1,
        ) else {
            return BridgeError::KagemushaV1.code();
        };
        unsafe {
            connect_norito_kagemusha_top_up_signed_request_validate_v1(
                signed.as_ptr(),
                signed.len() as c_ulong,
                expected.as_ptr(),
                expected.len() as c_ulong,
            )
        }
    }))
    .unwrap_or(BridgeError::KagemushaV1.code())
}
