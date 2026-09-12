macro_rules! jni_sdk_android_pairs {
    (
        $(
            android: $(#[$android_attribute:meta])* fn $android_name:ident();
            sdk: $(#[$sdk_attribute:meta])*
            pub unsafe extern "system" fn $sdk_name:ident(
                $($argument:tt)*
            ) $(-> $return_type:ty)? $body:block
        )*
    ) => {
        $(
            $(#[$sdk_attribute])*
            pub unsafe extern "system" fn $sdk_name(
                $($argument)*
            ) $(-> $return_type)? $body
            $(#[$android_attribute])*
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $android_name(
                $($argument)*
            ) $(-> $return_type)? $body
        )*
    };
}

jni_sdk_android_pairs! {
android: fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativePublicKeyFromPrivate();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativePublicKeyFromPrivate(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_public_key_from_private(&mut env, algorithm_code, private_key)
}
android: fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeBridgeAbiVersion();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}
android: fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeSignerContractRevision();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeSignerContractRevision(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    native_signer_jni_contract_revision() as jni::sys::jint
}
android: fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeKeypairFromSeed();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeKeypairFromSeed(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    seed: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_keypair_from_seed(&mut env, algorithm_code, seed)
}
android: fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeSignDetached();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeSignDetached(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_sign_detached(&mut env, algorithm_code, private_key, message)
}
android: fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeVerifyDetached();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeVerifyDetached(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    algorithm_code: jni::sys::jint,
    public_key: jni::objects::JByteArray<'_>,
    message: jni::objects::JByteArray<'_>,
    signature: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    java_native_verify_detached(&mut env, algorithm_code, public_key, message, signature)
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_crypto_NativeSignerBridge_nativeEncodeRegisterZkAssetSignedTransaction();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_crypto_NativeSignerBridge_nativeEncodeRegisterZkAssetSignedTransaction(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
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
    java_native_encode_register_zk_asset_signed_transaction(
        &mut env,
        algorithm_code,
        network_id,
        chain_discriminant,
        authority,
        creation_time_ms,
        ttl_ms,
        ttl_present,
        asset,
        vk_unshield,
        vk_unshield_present,
        vk_shield,
        vk_shield_present,
        private_key,
        fee_payment_json,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeBridgeAbiVersion();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeHasGovernanceDagSymbols();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeHasGovernanceDagSymbols(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    jni::sys::JNI_TRUE
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeHasGovernanceLogNodeSymbols();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeHasGovernanceLogNodeSymbols(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    jni::sys::JNI_TRUE
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeHasFixtureBundleSymbols();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeHasFixtureBundleSymbols(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    jni::sys::JNI_TRUE
}
android:
/// Reports that the Java Android ABI contains appeal-finance validator symbols.
///
/// # Safety
/// The JVM must supply valid JNI references for the duration of this call.
fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeHasAppealFinanceSymbols();
sdk:
/// Reports that the Kotlin/JVM ABI contains appeal-finance validator symbols.
///
/// # Safety
/// The JVM must supply valid JNI references for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeHasAppealFinanceSymbols(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    jni::sys::JNI_TRUE
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidateOrderbookPayloadJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidateOrderbookPayloadJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_orderbook_payload_json(
        &mut env,
        kind,
        payload,
        label,
        generated_at,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidatePopPayloadJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidatePopPayloadJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_pop_payload_json(&mut env, kind, payload, label, generated_at)
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidateHedgingPayloadJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidateHedgingPayloadJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_hedging_payload_json(
        &mut env,
        kind,
        payload,
        label,
        generated_at,
    )
}
android:
/// JNI entrypoint for Java Android appeal-finance `CancelAssetLock` validation.
///
/// # Safety
/// The JVM must supply valid JNI references for the duration of this call.
fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidateAppealFinanceCancelAssetLockJson();
sdk:
/// JNI entrypoint for Kotlin/JVM appeal-finance `CancelAssetLock` validation.
///
/// # Safety
/// The JVM must supply valid JNI references for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidateAppealFinanceCancelAssetLockJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json(
        &mut env,
        payload,
        label,
        generated_at,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidateFixtureBundleJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidateFixtureBundleJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    kinds: jni::objects::JByteArray<'_>,
    payloads: jni::objects::JObjectArray<'_>,
    labels: jni::objects::JObjectArray<'_>,
    now: jni::sys::jlong,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_fixture_bundle_json(
        &mut env,
        kinds,
        payloads,
        labels,
        now,
        generated_at,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidateGovernanceLogNodeJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidateGovernanceLogNodeJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    expected_node_cid: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_governance_log_node_json(
        &mut env,
        payload,
        label,
        expected_node_cid,
        generated_at,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidateGovernanceDagBlockJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidateGovernanceDagBlockJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    expected_block_cid: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_governance_dag_block_json(
        &mut env,
        payload,
        label,
        expected_block_cid,
        generated_at,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidateGovernanceDagHeadChainJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidateGovernanceDagHeadChainJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    head: jni::objects::JByteArray<'_>,
    head_label: jni::objects::JByteArray<'_>,
    blocks: jni::objects::JObjectArray<'_>,
    block_labels: jni::objects::JObjectArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_governance_dag_head_chain_json(
        &mut env,
        head,
        head_label,
        blocks,
        block_labels,
        generated_at,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeSignOrderbookPayload();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeSignOrderbookPayload(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_sign_orderbook_payload(&mut env, kind, payload, private_key)
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeDeriveOrderbookOrderId();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeDeriveOrderbookOrderId(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    owner_account: jni::objects::JByteArray<'_>,
    nonce: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_derive_orderbook_order_id(&mut env, owner_account, nonce)
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeBuildSignedOrderbookOrderRequest();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeBuildSignedOrderbookOrderRequest(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    order_id: jni::objects::JByteArray<'_>,
    side: jni::sys::jint,
    tier: jni::sys::jint,
    price_per_gib: jni::objects::JByteArray<'_>,
    quantity_gib: jni::sys::jlong,
    remaining_gib: jni::sys::jlong,
    owner_account: jni::objects::JByteArray<'_>,
    provider_id: jni::objects::JByteArray<'_>,
    expiry_unix: jni::sys::jlong,
    nonce: jni::sys::jlong,
    maker_fee_bps: jni::sys::jint,
    taker_fee_bps: jni::sys::jint,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_build_signed_orderbook_order_request(
        &mut env,
        JavaSorafsOrderbookOrderRequestArrays {
            order_id,
            side,
            tier,
            price_per_gib,
            quantity_gib,
            remaining_gib,
            owner_account,
            provider_id,
            expiry_unix,
            nonce,
            maker_fee_bps,
            taker_fee_bps,
            private_key,
        },
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeBuildSignedOrderbookOrderCancel();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeBuildSignedOrderbookOrderCancel(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    order_id: jni::objects::JByteArray<'_>,
    owner_account: jni::objects::JByteArray<'_>,
    reason: jni::sys::jint,
    nonce: jni::sys::jlong,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_build_signed_orderbook_order_cancel(
        &mut env,
        order_id,
        owner_account,
        reason,
        nonce,
        private_key,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeBuildSignedOrderbookSettlementReceipt();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeBuildSignedOrderbookSettlementReceipt(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    receipt_id: jni::objects::JByteArray<'_>,
    channel_id: jni::objects::JByteArray<'_>,
    trade_id: jni::objects::JByteArray<'_>,
    range_start: jni::sys::jlong,
    range_end: jni::sys::jlong,
    chunk_hash: jni::objects::JByteArray<'_>,
    bytes_delivered: jni::sys::jlong,
    xor_debited: jni::objects::JByteArray<'_>,
    provider_credit: jni::objects::JByteArray<'_>,
    fee_amount: jni::objects::JByteArray<'_>,
    issued_at_unix: jni::sys::jlong,
    private_key: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_build_signed_orderbook_settlement_receipt(
        &mut env,
        JavaSorafsOrderbookSettlementReceiptArrays {
            receipt_id,
            channel_id,
            trade_id,
            range_start,
            range_end,
            chunk_hash,
            bytes_delivered,
            xor_debited,
            provider_credit,
            fee_amount,
            issued_at_unix,
            private_key,
        },
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidatePdpPayloadJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidatePdpPayloadJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    kind: jni::sys::jint,
    payload: jni::objects::JByteArray<'_>,
    label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_pdp_payload_json(&mut env, kind, payload, label, generated_at)
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidatePdpCommitmentChallengeJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidatePdpCommitmentChallengeJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    commitment: jni::objects::JByteArray<'_>,
    commitment_label: jni::objects::JByteArray<'_>,
    challenge: jni::objects::JByteArray<'_>,
    challenge_label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_pdp_commitment_challenge_json(
        &mut env,
        commitment,
        commitment_label,
        challenge,
        challenge_label,
        generated_at,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidatePdpChallengeProofJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidatePdpChallengeProofJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    challenge: jni::objects::JByteArray<'_>,
    challenge_label: jni::objects::JByteArray<'_>,
    proof: jni::objects::JByteArray<'_>,
    proof_label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_pdp_challenge_proof_json(
        &mut env,
        challenge,
        challenge_label,
        proof,
        proof_label,
        generated_at,
    )
}
android: fn Java_org_hyperledger_iroha_android_sorafs_SorafsReferenceValidators_nativeValidatePdpBundleJson();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_sorafs_SorafsReferenceValidators_nativeValidatePdpBundleJson(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    commitment: jni::objects::JByteArray<'_>,
    commitment_label: jni::objects::JByteArray<'_>,
    challenge: jni::objects::JByteArray<'_>,
    challenge_label: jni::objects::JByteArray<'_>,
    proof: jni::objects::JByteArray<'_>,
    proof_label: jni::objects::JByteArray<'_>,
    generated_at: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_sorafs_reference_validate_pdp_bundle_json(
        &mut env,
        JavaSorafsPdpBundleArrays {
            commitment,
            commitment_label,
            challenge,
            challenge_label,
            proof,
            proof_label,
        },
        generated_at,
    )
}
}

// Canonical privacy JNI exports are owned only by the Kotlin SDK.
/// Return the native bridge ABI version for Kotlin/JVM privacy callers.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}
/// Return this binary's canonical compiled privacy profile catalog.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeCompiledProfileCatalog(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jbyteArray {
    java_native_privacy_compiled_profile_catalog(&mut env)
}
/// Validate a canonical compiled privacy profile catalog for Kotlin/JVM callers.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateCompiledProfileCatalog(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    java_native_privacy_validate_compiled_profile_catalog(&mut env, archive)
}
/// Validate a Torii Exact12 capability manifest for the Kotlin/JVM SDK.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateExact12CapabilityManifestForNetworkV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
    expected_network: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    java_native_privacy_validate_exact12_capability_manifest(&mut env, archive, expected_network)
}
/// Inspect one validated Torii Exact12 manifest and compare all local profile tuples.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeInspectExact12CapabilityManifest(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_privacy_inspect_exact12_capability_manifest(&mut env, archive)
}
/// Require active committed admission and exact local tuple equality for Kotlin/JVM.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeRequireExact12CapabilityTupleForNetworkV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
    protocol_index: jni::sys::jint,
    expected_network: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    java_native_privacy_require_exact12_capability_tuple(
        &mut env,
        archive,
        protocol_index,
        expected_network,
    )
}
/// Validate a canonical retained submit-proof instruction against committed Kotlin/JVM admission.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateExact12SubmitProofConstructionForNetworkV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    manifest_archive: jni::objects::JByteArray<'_>,
    protocol_index: jni::sys::jint,
    instruction_archive: jni::objects::JByteArray<'_>,
    expected_network: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    java_native_privacy_validate_exact12_submit_proof_construction(
        &mut env,
        manifest_archive,
        protocol_index,
        instruction_archive,
        expected_network,
    )
}
/// Return the canonical exact-12 privacy fixture bundle to the Kotlin/JVM SDK.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeExact12FixtureBundle(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jbyteArray {
    java_native_privacy_exact12_fixture_bundle(&mut env)
}
/// Validate an exact-12 privacy fixture bundle for the Kotlin/JVM SDK.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateExact12FixtureBundle(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    java_native_privacy_validate_exact12_fixture_bundle(&mut env, archive)
}

fn clear_parliament_jni_exception(env: &mut jni::JNIEnv<'_>) {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
}

fn read_parliament_jni_bytes(
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

fn read_parliament_jni_trust_anchor(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JByteArray<'_>,
) -> Option<[u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1]> {
    read_parliament_jni_bytes(
        env,
        value,
        CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1,
    )?
    .try_into()
    .ok()
}

fn parliament_jni_checkpoint_height(value: jni::sys::jlong) -> Option<u64> {
    // JNI has no unsigned 64-bit scalar. JVM callers pass the exact u64 bit
    // pattern through `long`; reinterpret it rather than rejecting the upper
    // half of the protocol's height domain.
    let height = u64::from_ne_bytes(value.to_ne_bytes());
    (height != 0).then_some(height)
}

fn read_parliament_jni_authority(
    env: &mut jni::JNIEnv<'_>,
    value: &jni::objects::JString<'_>,
) -> Option<String> {
    let java = env.get_string(value).ok()?;
    let authority = java.to_str().ok()?;
    if authority.is_empty()
        || authority.len() > super::parliament_timed_ovn_ffi::AUTHORITY_UTF8_MAX_BYTES_V1
    {
        return None;
    }
    Some(authority.to_owned())
}

fn parliament_jni_result(
    env: &mut jni::JNIEnv<'_>,
    expected_bytes: usize,
    body: impl FnOnce(&mut jni::JNIEnv<'_>) -> Option<Vec<u8>>,
) -> jni::sys::jbyteArray {
    let output = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(env)))
        .ok()
        .flatten()
        .filter(|bytes| bytes.len() == expected_bytes);
    let Some(output) = output else {
        clear_parliament_jni_exception(env);
        return std::ptr::null_mut();
    };
    match env.byte_array_from_slice(&output) {
        Ok(array) => array.into_raw(),
        Err(_) => {
            clear_parliament_jni_exception(env);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_governance_ParliamentTimedOvnNativeEndpointV1_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_governance_ParliamentTimedOvnNativeEndpointV1_nativeVerifyCastingProofV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    proof_response: jni::objects::JByteArray<'_>,
    network_id: jni::objects::JByteArray<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
    expected_ballot_attempt_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    let verified = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let proof_response = read_parliament_jni_bytes(
            &mut env,
            &proof_response,
            CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1,
        )?;
        let network_id = read_parliament_jni_trust_anchor(&mut env, &network_id)?;
        let trusted_checkpoint_height =
            parliament_jni_checkpoint_height(trusted_checkpoint_height)?;
        let trusted_checkpoint_context_id =
            read_parliament_jni_trust_anchor(&mut env, &trusted_checkpoint_context_id)?;
        let expected_ballot_attempt_id =
            read_parliament_jni_trust_anchor(&mut env, &expected_ballot_attempt_id)?;
        super::parliament_timed_ovn_ffi::verified_casting_context_from_proof_v1(
            &proof_response,
            network_id,
            trusted_checkpoint_height,
            trusted_checkpoint_context_id,
            expected_ballot_attempt_id,
        )
        .ok()
    }))
    .ok()
    .flatten()
    .is_some();
    if verified {
        jni::sys::JNI_TRUE
    } else {
        clear_parliament_jni_exception(&mut env);
        jni::sys::JNI_FALSE
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_governance_ParliamentTimedOvnNativeEndpointV1_nativeVerifyCastingProofPageV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    proof_response: jni::objects::JByteArray<'_>,
    network_id: jni::objects::JByteArray<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
    expected_ballot_attempt_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    parliament_jni_result(
        &mut env,
        CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_RESULT_BYTES_V1,
        |env| {
            let proof_response = read_parliament_jni_bytes(
                env,
                &proof_response,
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1,
            )?;
            let network_id = read_parliament_jni_trust_anchor(env, &network_id)?;
            let trusted_checkpoint_height =
                parliament_jni_checkpoint_height(trusted_checkpoint_height)?;
            let trusted_checkpoint_context_id =
                read_parliament_jni_trust_anchor(env, &trusted_checkpoint_context_id)?;
            let expected_ballot_attempt_id =
                read_parliament_jni_trust_anchor(env, &expected_ballot_attempt_id)?;
            super::parliament_timed_ovn_ffi::verified_casting_proof_page_v1(
                &proof_response,
                network_id,
                trusted_checkpoint_height,
                trusted_checkpoint_context_id,
                expected_ballot_attempt_id,
            )
            .ok()
            .map(|page| page.canonical_result_bytes_v1().to_vec())
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_governance_ParliamentTimedOvnNativeEndpointV1_nativeRegistrationFromProofV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    proof_response: jni::objects::JByteArray<'_>,
    network_id: jni::objects::JByteArray<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
    expected_ballot_attempt_id: jni::objects::JByteArray<'_>,
    authority: jni::objects::JString<'_>,
    seed: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    parliament_jni_result(
        &mut env,
        iroha_core::governance::timed_ovn::TIMED_OVN_REGISTRATION_RECORD_BYTES_V1,
        |env| {
            let proof_response = read_parliament_jni_bytes(
                env,
                &proof_response,
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1,
            )?;
            let network_id = read_parliament_jni_trust_anchor(env, &network_id)?;
            let trusted_checkpoint_height =
                parliament_jni_checkpoint_height(trusted_checkpoint_height)?;
            let trusted_checkpoint_context_id =
                read_parliament_jni_trust_anchor(env, &trusted_checkpoint_context_id)?;
            let expected_ballot_attempt_id =
                read_parliament_jni_trust_anchor(env, &expected_ballot_attempt_id)?;
            let casting_context =
                super::parliament_timed_ovn_ffi::verified_casting_context_from_proof_v1(
                    &proof_response,
                    network_id,
                    trusted_checkpoint_height,
                    trusted_checkpoint_context_id,
                    expected_ballot_attempt_id,
                )
                .ok()?;
            let authority = read_parliament_jni_authority(env, &authority)?;
            // Never copy the Java seed until every proof/archive check succeeds.
            let seed_bytes = Zeroizing::new(read_parliament_jni_bytes(
                env,
                &seed,
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1,
            )?);
            if seed_bytes.len() != CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1 {
                return None;
            }
            let mut seed =
                Zeroizing::new([0_u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1]);
            seed.copy_from_slice(&seed_bytes);
            super::parliament_timed_ovn_ffi::registration_from_verified_context_v1(
                &casting_context,
                &authority,
                &seed,
            )
            .ok()
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_governance_ParliamentTimedOvnNativeEndpointV1_nativeBallotFromProofV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    proof_response: jni::objects::JByteArray<'_>,
    network_id: jni::objects::JByteArray<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
    expected_ballot_attempt_id: jni::objects::JByteArray<'_>,
    authority: jni::objects::JString<'_>,
    seed: jni::objects::JByteArray<'_>,
    choice: jni::sys::jint,
) -> jni::sys::jbyteArray {
    parliament_jni_result(
        &mut env,
        iroha_core::governance::timed_ovn::TIMED_OVN_BALLOT_RECORD_BYTES_V1,
        |env| {
            let choice = u8::try_from(choice).ok().filter(|choice| *choice <= 2)?;
            let proof_response = read_parliament_jni_bytes(
                env,
                &proof_response,
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1,
            )?;
            let network_id = read_parliament_jni_trust_anchor(env, &network_id)?;
            let trusted_checkpoint_height =
                parliament_jni_checkpoint_height(trusted_checkpoint_height)?;
            let trusted_checkpoint_context_id =
                read_parliament_jni_trust_anchor(env, &trusted_checkpoint_context_id)?;
            let expected_ballot_attempt_id =
                read_parliament_jni_trust_anchor(env, &expected_ballot_attempt_id)?;
            let casting_context =
                super::parliament_timed_ovn_ffi::verified_casting_context_from_proof_v1(
                    &proof_response,
                    network_id,
                    trusted_checkpoint_height,
                    trusted_checkpoint_context_id,
                    expected_ballot_attempt_id,
                )
                .ok()?;
            let authority = read_parliament_jni_authority(env, &authority)?;
            // Never copy the Java seed until every proof/archive check succeeds.
            let seed_bytes = Zeroizing::new(read_parliament_jni_bytes(
                env,
                &seed,
                CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1,
            )?);
            if seed_bytes.len() != CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1 {
                return None;
            }
            let mut seed =
                Zeroizing::new([0_u8; CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1]);
            seed.copy_from_slice(&seed_bytes);
            super::parliament_timed_ovn_ffi::ballot_from_verified_context_v1(
                &casting_context,
                &authority,
                &seed,
                choice,
            )
            .ok()
        },
    )
}

#[cfg(test)]
mod parliament_timed_ovn_jni_height_tests {
    use super::parliament_jni_checkpoint_height;

    #[test]
    fn signed_jlong_is_an_exact_nonzero_u64_bit_carrier() {
        assert_eq!(parliament_jni_checkpoint_height(0), None);
        assert_eq!(parliament_jni_checkpoint_height(1), Some(1));
        assert_eq!(
            parliament_jni_checkpoint_height(i64::MAX),
            Some(i64::MAX as u64)
        );
        assert_eq!(
            parliament_jni_checkpoint_height(i64::MIN),
            Some(1_u64 << 63)
        );
        assert_eq!(parliament_jni_checkpoint_height(-1), Some(u64::MAX));
    }
}
