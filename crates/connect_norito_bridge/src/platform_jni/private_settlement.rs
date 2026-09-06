// JNI projections for fail-closed private-settlement response verification.

fn read_java_private_settlement_bytes(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JByteArray<'_>,
    maximum: usize,
) -> Option<Vec<u8>> {
    let length = usize::try_from(env.get_array_length(array).ok()?).ok()?;
    if length > maximum {
        return None;
    }
    env.convert_byte_array(array).ok()
}

fn java_private_settlement_status(body: impl FnOnce() -> Option<libc::c_int>) -> jni::sys::jint {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body))
        .ok()
        .flatten()
        .unwrap_or(ERR_PRIVATE_SETTLEMENT_RESPONSE) as jni::sys::jint
}

fn java_native_private_settlement_committee_proof_response_verify_v1(
    env: &mut jni::JNIEnv<'_>,
    response_json: jni::objects::JByteArray<'_>,
    expected_network_id: jni::objects::JByteArray<'_>,
    requested_payload_digest: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    let response = read_java_private_settlement_bytes(
        env,
        &response_json,
        CONNECT_NORITO_PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1,
    );
    let network = read_java_private_settlement_bytes(env, &expected_network_id, 32);
    let payload = read_java_private_settlement_bytes(env, &requested_payload_digest, 32);
    java_private_settlement_status(|| {
        let response = response?;
        let network = network?;
        let payload = payload?;
        Some(unsafe {
            connect_norito_private_settlement_committee_proof_response_verify_v1(
                response.as_ptr(),
                response.len() as libc::c_ulong,
                network.as_ptr(),
                network.len() as libc::c_ulong,
                payload.as_ptr(),
                payload.len() as libc::c_ulong,
            )
        })
    })
}

fn java_native_private_settlement_auditor_capsule_response_verify_with_request_v1(
    env: &mut jni::JNIEnv<'_>,
    response_json: jni::objects::JByteArray<'_>,
    request_json: jni::objects::JByteArray<'_>,
    expected_network_id: jni::objects::JByteArray<'_>,
    requested_payload_digest: jni::objects::JByteArray<'_>,
    auditor_public_key_utf8: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    let response = read_java_private_settlement_bytes(
        env,
        &response_json,
        CONNECT_NORITO_PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1,
    );
    let request = read_java_private_settlement_bytes(
        env,
        &request_json,
        CONNECT_NORITO_PRIVATE_SETTLEMENT_REQUEST_MAX_BYTES_V1,
    );
    let network = read_java_private_settlement_bytes(env, &expected_network_id, 32);
    let payload = read_java_private_settlement_bytes(env, &requested_payload_digest, 32);
    let auditor_key = read_java_private_settlement_bytes(env, &auditor_public_key_utf8, 1024);
    java_private_settlement_status(|| {
        let response = response?;
        let request = request?;
        let network = network?;
        let payload = payload?;
        let auditor_key = auditor_key?;
        Some(unsafe {
            connect_norito_private_settlement_auditor_capsule_response_verify_with_request_v1(
                response.as_ptr(),
                response.len() as libc::c_ulong,
                request.as_ptr(),
                request.len() as libc::c_ulong,
                network.as_ptr(),
                network.len() as libc::c_ulong,
                payload.as_ptr(),
                payload.len() as libc::c_ulong,
                auditor_key.as_ptr().cast(),
                auditor_key.len() as libc::c_ulong,
            )
        })
    })
}

fn java_native_private_settlement_audit_approval_response_verify_v1(
    env: &mut jni::JNIEnv<'_>,
    response_json: jni::objects::JByteArray<'_>,
    request_json: jni::objects::JByteArray<'_>,
    expected_network_id: jni::objects::JByteArray<'_>,
    requested_payload_digest: jni::objects::JByteArray<'_>,
    auditor_public_key_utf8: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    let response = read_java_private_settlement_bytes(
        env,
        &response_json,
        CONNECT_NORITO_PRIVATE_SETTLEMENT_RESPONSE_MAX_BYTES_V1,
    );
    let request = read_java_private_settlement_bytes(
        env,
        &request_json,
        CONNECT_NORITO_PRIVATE_SETTLEMENT_REQUEST_MAX_BYTES_V1,
    );
    let network = read_java_private_settlement_bytes(env, &expected_network_id, 32);
    let payload = read_java_private_settlement_bytes(env, &requested_payload_digest, 32);
    let auditor_key = read_java_private_settlement_bytes(env, &auditor_public_key_utf8, 1024);
    java_private_settlement_status(|| {
        let response = response?;
        let request = request?;
        let network = network?;
        let payload = payload?;
        let auditor_key = auditor_key?;
        Some(unsafe {
            connect_norito_private_settlement_audit_approval_response_verify_v1(
                response.as_ptr(),
                response.len() as libc::c_ulong,
                request.as_ptr(),
                request.len() as libc::c_ulong,
                network.as_ptr(),
                network.len() as libc::c_ulong,
                payload.as_ptr(),
                payload.len() as libc::c_ulong,
                auditor_key.as_ptr().cast(),
                auditor_key.len() as libc::c_ulong,
            )
        })
    })
}

jni_sdk_android_pairs! {
android: fn Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeBridgeAbiVersion();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}
android: fn Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyCommitteeProofResponseV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyCommitteeProofResponseV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    response_json: jni::objects::JByteArray<'_>,
    expected_network_id: jni::objects::JByteArray<'_>,
    requested_payload_digest: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    java_native_private_settlement_committee_proof_response_verify_v1(
        &mut env,
        response_json,
        expected_network_id,
        requested_payload_digest,
    )
}
android: fn Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditorCapsuleResponseWithRequestV1();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditorCapsuleResponseWithRequestV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    response_json: jni::objects::JByteArray<'_>,
    request_json: jni::objects::JByteArray<'_>,
    expected_network_id: jni::objects::JByteArray<'_>,
    requested_payload_digest: jni::objects::JByteArray<'_>,
    auditor_public_key_utf8: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    java_native_private_settlement_auditor_capsule_response_verify_with_request_v1(
        &mut env,
        response_json,
        request_json,
        expected_network_id,
        requested_payload_digest,
        auditor_public_key_utf8,
    )
}
android: fn Java_org_hyperledger_iroha_android_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditApprovalResponseV1();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_client_AtomicPrivateSettlementNativeResponseVerifierV1_nativeVerifyAuditApprovalResponseV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    response_json: jni::objects::JByteArray<'_>,
    request_json: jni::objects::JByteArray<'_>,
    expected_network_id: jni::objects::JByteArray<'_>,
    requested_payload_digest: jni::objects::JByteArray<'_>,
    auditor_public_key_utf8: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    java_native_private_settlement_audit_approval_response_verify_v1(
        &mut env,
        response_json,
        request_json,
        expected_network_id,
        requested_payload_digest,
        auditor_public_key_utf8,
    )
}
}
