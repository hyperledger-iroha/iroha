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
android: fn Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeBridgeAbiVersion();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}
android: fn Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeCompiledProfileCatalog();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeCompiledProfileCatalog(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jbyteArray {
    java_native_privacy_compiled_profile_catalog(&mut env)
}
android: fn Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeValidateCompiledProfileCatalog();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateCompiledProfileCatalog(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    java_native_privacy_validate_compiled_profile_catalog(&mut env, archive)
}
android:
/// Validate a Torii Exact12 capability manifest for the Java Android SDK.
fn Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeValidateExact12CapabilityManifest();
sdk:
/// Validate a Torii Exact12 capability manifest for the Kotlin/JVM SDK.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateExact12CapabilityManifest(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    java_native_privacy_validate_exact12_capability_manifest(&mut env, archive)
}
android:
/// Inspect one validated Torii Exact12 manifest and compare all local profile tuples.
fn Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeInspectExact12CapabilityManifest();
sdk:
/// Inspect one validated Torii Exact12 manifest and compare all local profile tuples.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeInspectExact12CapabilityManifest(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_privacy_inspect_exact12_capability_manifest(&mut env, archive)
}
android:
/// Require active committed admission and exact local tuple equality for Java/Android.
fn Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeRequireExact12CapabilityTuple();
sdk:
/// Require active committed admission and exact local tuple equality for Kotlin/JVM.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeRequireExact12CapabilityTuple(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
    protocol_index: jni::sys::jint,
) -> jni::sys::jboolean {
    java_native_privacy_require_exact12_capability_tuple(&mut env, archive, protocol_index)
}
android:
/// Validate a canonical retained submit-proof instruction against committed Java admission.
fn Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeValidateExact12SubmitProofConstruction();
sdk:
/// Validate a canonical retained submit-proof instruction against committed Kotlin/JVM admission.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateExact12SubmitProofConstruction(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    manifest_archive: jni::objects::JByteArray<'_>,
    protocol_index: jni::sys::jint,
    instruction_archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    java_native_privacy_validate_exact12_submit_proof_construction(
        &mut env,
        manifest_archive,
        protocol_index,
        instruction_archive,
    )
}
android:
/// Return the canonical exact-12 privacy fixture bundle to the Java Android SDK.
fn Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeExact12FixtureBundle();
sdk:
/// Return the canonical exact-12 privacy fixture bundle to the Kotlin/JVM SDK.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeExact12FixtureBundle(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jbyteArray {
    java_native_privacy_exact12_fixture_bundle(&mut env)
}
android:
/// Validate an exact-12 privacy fixture bundle for the Java Android SDK.
fn Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_nativeValidateExact12FixtureBundle();
sdk:
/// Validate an exact-12 privacy fixture bundle for the Kotlin/JVM SDK.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_nativeValidateExact12FixtureBundle(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    archive: jni::objects::JByteArray<'_>,
) -> jni::sys::jint {
    java_native_privacy_validate_exact12_fixture_bundle(&mut env, archive)
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
type JniByteArray = jni::sys::jbyteArray;
type JniObjectArray = jni::sys::jobjectArray;
type JniBoolean = jni::sys::jboolean;
type JniLong = jni::sys::jlong;

macro_rules! kagemusha_jni_argument_type {
    (bytes) => { jni::objects::JByteArray<'_> };
    (objects) => { jni::objects::JObjectArray<'_> };
    (longs) => { jni::objects::JLongArray<'_> };
    (int) => { jni::sys::jint };
    (long) => { jni::sys::jlong };
}

/// Generates both JNI namespace exports for each typed Kagemusha delegate.
macro_rules! kagemusha_sdk_android_forwarders {
    (
        $(
            $(#[$attribute:meta])*
            $method:ident { $($argument:ident $argument_type:ident),* $(,)? }
                -> $return_type:ty = $delegate:path $(, $extra:expr)*;
        )*
    ) => {
        $(
            #[allow(non_snake_case)]
            mod $method {
                use super::*;

                $(#[$attribute])*
                #[unsafe(export_name = concat!(
                    "Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_",
                    stringify!($method)
                ))]
                pub unsafe extern "system" fn sdk(
                    mut env: jni::JNIEnv<'_>,
                    _class: jni::objects::JClass<'_>,
                    $($argument: kagemusha_jni_argument_type!($argument_type)),*
                ) -> $return_type {
                    $delegate(&mut env $(, $argument)* $(, $extra)*)
                }

                $(#[$attribute])*
                #[unsafe(export_name = concat!(
                    "Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_",
                    stringify!($method)
                ))]
                pub unsafe extern "system" fn android(
                    mut env: jni::JNIEnv<'_>,
                    _class: jni::objects::JClass<'_>,
                    $($argument: kagemusha_jni_argument_type!($argument_type)),*
                ) -> $return_type {
                    $delegate(&mut env $(, $argument)* $(, $extra)*)
                }
            }
        )*
    };
}

type KagemushaUnaryLifecycleBoundaryV4 =
    unsafe extern "C" fn(*const c_uchar, c_ulong, *mut *mut c_uchar, *mut c_ulong) -> c_int;

pub(super) fn java_native_kagemusha_unary_lifecycle_v4(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    label: &str,
    request_max_bytes: usize,
    boundary: KagemushaUnaryLifecycleBoundaryV4,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_lifecycle_archive_v4(
        env,
        request,
        label,
        request_max_bytes,
        |request_ptr, request_len, output, output_len| unsafe {
            boundary(request_ptr, request_len, output, output_len)
        },
    )
}

jni_sdk_android_pairs! {
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBridgeAbiVersion();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBridgeAbiVersion(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativePastaCycleV4BackendAvailable();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativePastaCycleV4BackendAvailable(
    _env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jboolean {
    java_native_kagemusha_pasta_cycle_v4_backend_available()
}
}

kagemusha_sdk_android_forwarders! {
    nativeArtifactBeginV4 { manifest_norito bytes, manifest_sha256 bytes, artifact_sha256 bytes } -> JniLong = java_native_kagemusha_artifact_begin_v4;
    nativeArtifactWriteV4 { handle long, chunk bytes } -> () = java_native_kagemusha_artifact_write_v4;
    nativeArtifactFinalizeV4 { handle long } -> () = java_native_kagemusha_artifact_finish_v4,
        false;
    nativeArtifactCancelV4 { handle long } -> () = java_native_kagemusha_artifact_finish_v4,
        true;
    nativeArtifactSetInstallV4 {
        manifest_norito bytes, manifest_sha256 bytes, trusted_policy_norito bytes, release_attestation_norito bytes,
        internal_validation_receipt_norito bytes, benchmark_evidence bytes, cryptographic_review bytes,
        promotion_record_norito bytes, handles longs
    } -> () = java_native_kagemusha_artifact_set_install_v4;
    nativeArtifactSetIsInstalledV4 { manifest_norito bytes, manifest_sha256 bytes } -> JniBoolean = java_native_kagemusha_artifact_set_is_installed_v4;
    nativeInstalledManifestSha256V4 {  } -> JniByteArray = java_native_kagemusha_installed_manifest_sha256_v4;
    nativeBuildArtifactBindingV4 { manifest_norito bytes, manifest_sha256 bytes } -> JniByteArray = java_native_kagemusha_build_artifact_binding_v4;
    nativeArtifactSetUninstallV4 { manifest_sha256 bytes } -> () = java_native_kagemusha_artifact_set_uninstall_v4;
    nativeInitSpendV4 { request_norito bytes } -> JniByteArray = java_native_kagemusha_unary_lifecycle_v4,
        "V4 init spend",
        KAGEMUSHA_RECURSIVE_SPEND_INIT_LOCAL_MAX_BYTES_V4,
        connect_norito_kagemusha_recursive_spend_init_v4;
    nativeAppendSpendV4 {
        request_norito bytes, recipient_request_norito bytes, verified_at_ms long
    } -> JniByteArray = java_native_kagemusha_append_spend_v4;
    nativeVerifySpendV4 { request_norito bytes } -> JniByteArray = java_native_kagemusha_unary_lifecycle_v4,
        "V4 verify spend",
        KAGEMUSHA_RECURSIVE_SPEND_VERIFY_LOCAL_MAX_BYTES_V4,
        connect_norito_kagemusha_recursive_spend_verify_v4;
    nativeBuildRedeemV4 { request_norito bytes } -> JniByteArray = java_native_kagemusha_unary_lifecycle_v4,
        "V4 build redeem",
        KAGEMUSHA_RECURSIVE_SPEND_REDEEM_LOCAL_MAX_BYTES_V4,
        connect_norito_kagemusha_recursive_spend_redeem_v4;
    #[allow(clippy::too_many_arguments)]
    nativePrepareRecipientRequestV2 {
        network_id bytes, chain_discriminant int, asset bytes, atomic_units bytes,
        scale int, recipient bytes, receiver_device_id bytes, receiver_public_key bytes,
        request_id bytes, issued_at_ms long, expires_at_ms long, spend_key bytes,
        rho bytes, diversifier bytes
    } -> JniObjectArray = java_native_kagemusha_prepare_recipient_request_v2;
    nativeCreateRecipientRequestV2 { payload bytes, signature bytes } -> JniByteArray = java_native_kagemusha_create_recipient_request_v2;
    nativeVerifyRecipientRequestV2 { request bytes, verified_at_ms long } -> JniByteArray = java_native_kagemusha_verify_recipient_request_v2;
    nativeCreateRecipientLineageQueryV2 {
        network_id bytes, chain_discriminant int, recipient bytes, receiver_device_id bytes,
        asset bytes, trusted_checkpoint_height long
    } -> JniByteArray = java_native_kagemusha_create_recipient_lineage_query_v2;
    #[allow(clippy::too_many_arguments)]
    nativeVerifyRecipientRegistrationLineageV2 {
        request bytes, lineage bytes, verified_at_ms long, trusted_checkpoint_height long,
        trusted_checkpoint_context_id bytes
    } -> JniObjectArray = java_native_kagemusha_verify_recipient_registration_lineage_v2;
    nativeCreateRecipientReceiveOfferV2 {
        request bytes, lineage bytes, publisher_checkpoint_envelope bytes
    } -> JniByteArray = java_native_kagemusha_create_recipient_receive_offer_v2;
    nativeProjectRecipientReceiveOfferV2 { offer bytes } -> JniObjectArray = java_native_kagemusha_project_recipient_receive_offer_v2;
    nativeVerifyRecipientReceiveOfferV2 {
        offer bytes, verified_at_ms long, trusted_checkpoint_height long, trusted_checkpoint_context_id bytes
    } -> JniObjectArray = java_native_kagemusha_verify_recipient_receive_offer_v2;
    nativeBuildOutputMembershipFrontierV4 {
        leaf_index int, flattened_siblings bytes, directions bytes, root bytes
    } -> JniByteArray = java_native_kagemusha_build_output_membership_frontier_v4;
    nativeDeriveOutputMembershipPathsV4 {
        frontier bytes, recipient_commitment bytes, change_commitment bytes
    } -> JniObjectArray = java_native_kagemusha_derive_output_membership_paths_v4;
    nativeValidateSpendableBranchV4 {
        bundle bytes, provenance bytes, membership_witness bytes, opening bytes,
        block_height long
    } -> JniByteArray = java_native_kagemusha_validate_spendable_branch_v4;
    nativeBuildOutputMembershipPathsV4 {
        initial_root bytes, final_root bytes, recipient_fields objects, change_fields objects,
        dummy_fields objects
    } -> JniByteArray = java_native_kagemusha_build_output_membership_paths_v4;
    nativeBuildInitRequestV4 {
        anchor bytes, proof bytes, roster bytes, opening bytes,
        output_membership bytes
    } -> JniByteArray = java_native_kagemusha_build_init_request_v4;
    nativeBuildTopUpProvenanceV4 {
        bundle bytes, roster bytes, anchors objects, finality_proofs objects,
        block_height long
    } -> JniByteArray = java_native_kagemusha_build_topup_provenance_v4;
    nativeValidateTopUpProvenanceV4 { bundle bytes, provenance bytes, block_height long } -> JniByteArray = java_native_kagemusha_validate_topup_provenance_v4;
    #[allow(clippy::too_many_arguments)]
    nativeBuildAppendRequestV4 {
        bundles objects, topup_provenances objects, openings objects, witnesses objects,
        change_opening bytes, output_membership bytes, verifier_commitment bytes, operation_id bytes,
        block_height long
    } -> JniByteArray = java_native_kagemusha_build_append_request_v4;
    #[allow(clippy::too_many_arguments)]
    nativeBuildVerifyRequestV4 {
        bundle bytes, recipient_request bytes, topup_provenance bytes, maximum_hops int,
        block_height long, verified_at_ms long
    } -> JniByteArray = java_native_kagemusha_build_verify_request_v4,
        JavaKagemushaArtifactRegistryV4::Production;
    #[allow(clippy::too_many_arguments)]
    nativeBuildRedeemRequestV4 {
        bundle bytes, topup_provenance bytes, opening bytes, membership_witness bytes,
        recipient bytes, chain_discriminant int, atomic_units bytes, scale int,
        change_opening bytes, change_output_membership bytes, verifier_commitment bytes, operation_id bytes,
        block_height long
    } -> JniByteArray = java_native_kagemusha_build_redeem_request_v4;
    nativeProjectPeerPaymentV4 { payment bytes } -> JniObjectArray = java_native_kagemusha_project_peer_payment_v4;
    nativeProjectInitResultV4 { result bytes } -> JniObjectArray = java_native_kagemusha_project_init_result_v4;
    nativeProjectSplitResultV4 { result bytes } -> JniObjectArray = java_native_kagemusha_project_split_result_v4;
    nativeProjectVerifyResultV4 { result bytes } -> JniObjectArray = java_native_kagemusha_project_verify_result_v4;
    nativeProjectRedeemBuildResultV4 { result bytes } -> JniObjectArray = java_native_kagemusha_project_redeem_build_result_v4;
    nativePrepareAcknowledgementV2 { request bytes, payment bytes, accepted_at_ms long } -> JniObjectArray = java_native_kagemusha_prepare_acknowledgement_v2;
    nativeCreateAcknowledgementV2 { payload bytes, signature bytes, request bytes, payment bytes } -> JniByteArray = java_native_kagemusha_create_acknowledgement_v2;
    nativeVerifyAcknowledgementV2 { acknowledgement bytes, request bytes, payment bytes } -> JniObjectArray = java_native_kagemusha_verify_acknowledgement_v2;
    #[allow(clippy::too_many_arguments)]
    nativePrepareAuthorizationV2 {
        authority bytes, chain_discriminant int, device_id bytes, asset_definition_id bytes,
        operation_id bytes, issued_at_ms long, expires_at_ms long, nonce bytes,
        payload_digest bytes, registration_hash bytes, hardware_assertion_platform bytes
    } -> JniObjectArray = java_native_kagemusha_prepare_authorization_v2;
    nativeFinalizeHardwareAuthorizationV2 {
        preparation bytes, authenticator_data bytes, signature_der bytes
    } -> JniObjectArray = java_native_kagemusha_finalize_hardware_authorization_v2;
    nativeFinalizeIosAppAttestAuthorizationV2 { preparation bytes, assertion_object bytes } -> JniObjectArray = java_native_kagemusha_finalize_ios_app_attest_authorization_v2;
    nativeFinalizeTopUpV4 { unsigned bytes, authorization bytes } -> JniByteArray = java_native_kagemusha_finalize_top_up_v4;
    nativeFinalizeRedeemV4 { build_result bytes, authorization bytes } -> JniObjectArray = java_native_kagemusha_finalize_redeem_v4;
    #[allow(clippy::too_many_arguments)]
    nativePrepareTopUpV4 {
        network_id bytes, chain_discriminant int, asset_definition bytes, payer bytes,
        atomic_units bytes, scale int, operation_id bytes, spend_key bytes,
        rho bytes, diversifier bytes, leaf_index int, flattened_siblings bytes,
        directions bytes, root bytes, shield_verifier_commitment bytes, artifact_binding bytes
    } -> JniObjectArray = java_native_kagemusha_prepare_top_up_v4;
    nativeProjectTopUpRequestOperationIdV4 { request bytes } -> JniByteArray = java_native_kagemusha_project_top_up_request_operation_id_v4;
    nativeProjectRedeemRequestOperationIdV4 { request bytes } -> JniByteArray = java_native_kagemusha_project_redeem_request_operation_id_v4;
    nativeProjectOperationReferenceV4 { reference bytes } -> JniObjectArray = java_native_kagemusha_project_operation_reference_v4;
    nativeProjectOperationStatusV4 { status bytes } -> JniObjectArray = java_native_kagemusha_project_operation_status_v4;
    nativeBranchClaimsConflictV2 { left bytes, right bytes } -> JniBoolean = java_native_kagemusha_branch_claims_conflict_v2;
    #[allow(clippy::too_many_arguments)]
    nativePrepareRedemptionChangeV4 {
        bundle bytes, input_opening bytes, atomic_units bytes, scale int,
        operation_id bytes, entropy bytes
    } -> JniObjectArray = java_native_kagemusha_prepare_redemption_change_v4;
    #[allow(clippy::too_many_arguments)]
    nativePreparePeerSplitChangeV4 {
        bundles objects, input_openings objects, recipient_request bytes, atomic_units bytes,
        scale int, operation_id bytes, entropy bytes
    } -> JniObjectArray = java_native_kagemusha_prepare_peer_split_change_v4;
    nativePrepareNoteOpeningV2 { spend_key bytes, rho bytes, diversifier bytes } -> JniByteArray = java_native_kagemusha_prepare_note_opening_v2;
    nativeProjectRecipientRequestV2 { request bytes } -> JniObjectArray = java_native_kagemusha_project_recipient_request_v2;
}

pub(super) fn ensure_min_array_length(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    required: i32,
    context: &str,
) -> bool {
    match env.get_array_length(array) {
        Ok(len) if len >= required => true,
        Ok(len) => {
            throw_java_illegal_argument(
                env,
                format!("{context} expects an output array with length >= {required}, got {len}"),
            );
            false
        }
        Err(err) => {
            throw_java_illegal_argument(
                env,
                format!("{context} failed to read array length: {err}"),
            );
            false
        }
    }
}
pub(super) fn read_long_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    context: &str,
) -> Option<Vec<i64>> {
    let len = match env.get_array_length(array) {
        Ok(value) => value,
        Err(err) => {
            throw_java_illegal_argument(
                env,
                format!("{context} failed to read array length: {err}"),
            );
            return None;
        }
    } as usize;
    let mut buf = vec![0i64; len];
    if let Err(err) = env.get_long_array_region(array, 0, &mut buf) {
        throw_java_illegal_state(
            env,
            format!("{context} failed to read array contents: {err}"),
        );
        return None;
    }
    Some(buf)
}
pub(super) fn write_long_array(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    values: &[i64],
    context: &str,
) -> bool {
    if let Err(err) = env.set_long_array_region(array, 0, values) {
        throw_java_illegal_state(
            env,
            format!("{context} failed to write output array: {err}"),
        );
        return false;
    }
    true
}
pub(super) fn convert_field_elem<L: Into<String>>(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    context: L,
) -> Option<[u64; 4]> {
    let context = context.into();
    let buf = read_long_array(env, array, &context)?;
    if buf.len() != 4 {
        throw_java_illegal_argument(env, format!("{context} expects an array of length 4"));
        return None;
    }
    let mut limbs = [0u64; 4];
    for (dst, src) in limbs.iter_mut().zip(buf.iter()) {
        *dst = *src as u64;
    }
    Some(limbs)
}
pub(super) fn convert_field_elems<L: Into<String>>(
    env: &mut jni::JNIEnv<'_>,
    array: &jni::objects::JLongArray<'_>,
    context: L,
) -> Option<Vec<[u64; 4]>> {
    let context = context.into();
    let buf = read_long_array(env, array, &context)?;
    if buf.len() % 4 != 0 {
        throw_java_illegal_argument(
            env,
            format!("{context} expects a flattened array with a length multiple of 4"),
        );
        return None;
    }
    let mut elems = Vec::with_capacity(buf.len() / 4);
    for chunk in buf.chunks_exact(4) {
        let mut limbs = [0u64; 4];
        for (dst, src) in limbs.iter_mut().zip(chunk.iter()) {
            *dst = *src as u64;
        }
        elems.push(limbs);
    }
    Some(elems)
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativePoseidon2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    a: jni::sys::jlong,
    b: jni::sys::jlong,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    if !ensure_min_array_length(&mut env, &out, 1, "poseidon2") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "poseidon2_cuda", || {
        ivm::poseidon2_cuda(a as u64, b as u64)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(hash) = result {
        let value = [hash as i64];
        if write_long_array(&mut env, &out, &value, "poseidon2") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativePoseidon2Batch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    inputs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let buf = match read_long_array(&mut env, &inputs, "poseidon2Batch inputs") {
        Some(values) => values,
        None => return JNI_FALSE,
    };
    if buf.len() % 2 != 0 {
        throw_java_illegal_argument(
            &mut env,
            "poseidon2Batch inputs must contain an even number of elements".into(),
        );
        return JNI_FALSE;
    }
    let batch_size = (buf.len() / 2) as i32;
    if !ensure_min_array_length(&mut env, &out, batch_size, "poseidon2Batch") {
        return JNI_FALSE;
    }
    let mut tuples = Vec::with_capacity(batch_size as usize);
    for chunk in buf.chunks_exact(2) {
        tuples.push((chunk[0] as u64, chunk[1] as u64));
    }
    let result = match catch_unwind_to_java(&mut env, "poseidon2_cuda_many", || {
        ivm::poseidon2_cuda_many(&tuples)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(outputs) = result {
        let values: Vec<i64> = outputs.into_iter().map(|value| value as i64).collect();
        if write_long_array(&mut env, &out, &values, "poseidon2Batch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativePoseidon6(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    inputs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    if !ensure_min_array_length(&mut env, &out, 1, "poseidon6") {
        return JNI_FALSE;
    }
    let buf = match read_long_array(&mut env, &inputs, "poseidon6 inputs") {
        Some(values) => values,
        None => return JNI_FALSE,
    };
    if buf.len() != 6 {
        throw_java_illegal_argument(&mut env, "poseidon6 expects six inputs".into());
        return JNI_FALSE;
    }
    let mut state = [0u64; 6];
    for (dst, src) in state.iter_mut().zip(buf.iter()) {
        *dst = *src as u64;
    }
    let result =
        match catch_unwind_to_java(&mut env, "poseidon6_cuda", || ivm::poseidon6_cuda(state)) {
            Some(value) => value,
            None => return JNI_FALSE,
        };
    if let Some(hash) = result {
        let value = [hash as i64];
        if write_long_array(&mut env, &out, &value, "poseidon6") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativePoseidon6Batch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    inputs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let buf = match read_long_array(&mut env, &inputs, "poseidon6Batch inputs") {
        Some(values) => values,
        None => return JNI_FALSE,
    };
    if buf.len() % 6 != 0 {
        throw_java_illegal_argument(
            &mut env,
            "poseidon6Batch inputs must be multiples of six".into(),
        );
        return JNI_FALSE;
    }
    let batch_size = (buf.len() / 6) as i32;
    if !ensure_min_array_length(&mut env, &out, batch_size, "poseidon6Batch") {
        return JNI_FALSE;
    }
    let mut states = Vec::with_capacity(batch_size as usize);
    for chunk in buf.chunks_exact(6) {
        let mut state = [0u64; 6];
        for (dst, src) in state.iter_mut().zip(chunk.iter()) {
            *dst = *src as u64;
        }
        states.push(state);
    }
    let result = match catch_unwind_to_java(&mut env, "poseidon6_cuda_many", || {
        ivm::poseidon6_cuda_many(&states)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(outputs) = result {
        let values: Vec<i64> = outputs.into_iter().map(|value| value as i64).collect();
        if write_long_array(&mut env, &out, &values, "poseidon6Batch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254Add(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    a: jni::objects::JLongArray<'_>,
    b: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    if !ensure_min_array_length(&mut env, &out, 4, "bn254Add") {
        return JNI_FALSE;
    }
    let a = match convert_field_elem(&mut env, &a, "bn254Add input a") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let b = match convert_field_elem(&mut env, &b, "bn254Add input b") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let result =
        match catch_unwind_to_java(&mut env, "bn254_add_cuda", || ivm::bn254_add_cuda(a, b)) {
            Some(value) => value,
            None => return JNI_FALSE,
        };
    if let Some(field) = result {
        let values: Vec<i64> = field.into_iter().map(|limb| limb as i64).collect();
        if write_long_array(&mut env, &out, &values, "bn254Add") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254Sub(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    a: jni::objects::JLongArray<'_>,
    b: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    if !ensure_min_array_length(&mut env, &out, 4, "bn254Sub") {
        return JNI_FALSE;
    }
    let a = match convert_field_elem(&mut env, &a, "bn254Sub input a") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let b = match convert_field_elem(&mut env, &b, "bn254Sub input b") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let result =
        match catch_unwind_to_java(&mut env, "bn254_sub_cuda", || ivm::bn254_sub_cuda(a, b)) {
            Some(value) => value,
            None => return JNI_FALSE,
        };
    if let Some(field) = result {
        let values: Vec<i64> = field.into_iter().map(|limb| limb as i64).collect();
        if write_long_array(&mut env, &out, &values, "bn254Sub") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254Mul(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    a: jni::objects::JLongArray<'_>,
    b: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    if !ensure_min_array_length(&mut env, &out, 4, "bn254Mul") {
        return JNI_FALSE;
    }
    let a = match convert_field_elem(&mut env, &a, "bn254Mul input a") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let b = match convert_field_elem(&mut env, &b, "bn254Mul input b") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let result =
        match catch_unwind_to_java(&mut env, "bn254_mul_cuda", || ivm::bn254_mul_cuda(a, b)) {
            Some(value) => value,
            None => return JNI_FALSE,
        };
    if let Some(field) = result {
        let values: Vec<i64> = field.into_iter().map(|limb| limb as i64).collect();
        if write_long_array(&mut env, &out, &values, "bn254Mul") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254AddBatch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    lhs: jni::objects::JLongArray<'_>,
    rhs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let lhs = match convert_field_elems(&mut env, &lhs, "bn254AddBatch lhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let rhs = match convert_field_elems(&mut env, &rhs, "bn254AddBatch rhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if lhs.len() != rhs.len() {
        throw_java_illegal_argument(
            &mut env,
            "bn254AddBatch expects matching batch lengths".into(),
        );
        return JNI_FALSE;
    }
    let out_len = match i32::try_from(lhs.len().saturating_mul(4)) {
        Ok(value) => value,
        Err(_) => {
            throw_java_illegal_argument(
                &mut env,
                "bn254AddBatch output exceeds Java array limits".into(),
            );
            return JNI_FALSE;
        }
    };
    if !ensure_min_array_length(&mut env, &out, out_len, "bn254AddBatch") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "bn254_add_batch_cuda", || {
        ivm::bn254_add_batch_cuda(&lhs, &rhs)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(fields) = result {
        let values: Vec<i64> = fields
            .into_iter()
            .flat_map(|field| field.into_iter().map(|limb| limb as i64))
            .collect();
        if write_long_array(&mut env, &out, &values, "bn254AddBatch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254SubBatch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    lhs: jni::objects::JLongArray<'_>,
    rhs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let lhs = match convert_field_elems(&mut env, &lhs, "bn254SubBatch lhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let rhs = match convert_field_elems(&mut env, &rhs, "bn254SubBatch rhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if lhs.len() != rhs.len() {
        throw_java_illegal_argument(
            &mut env,
            "bn254SubBatch expects matching batch lengths".into(),
        );
        return JNI_FALSE;
    }
    let out_len = match i32::try_from(lhs.len().saturating_mul(4)) {
        Ok(value) => value,
        Err(_) => {
            throw_java_illegal_argument(
                &mut env,
                "bn254SubBatch output exceeds Java array limits".into(),
            );
            return JNI_FALSE;
        }
    };
    if !ensure_min_array_length(&mut env, &out, out_len, "bn254SubBatch") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "bn254_sub_batch_cuda", || {
        ivm::bn254_sub_batch_cuda(&lhs, &rhs)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(fields) = result {
        let values: Vec<i64> = fields
            .into_iter()
            .flat_map(|field| field.into_iter().map(|limb| limb as i64))
            .collect();
        if write_long_array(&mut env, &out, &values, "bn254SubBatch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_android_gpu_CudaAccelerators_nativeBn254MulBatch(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    lhs: jni::objects::JLongArray<'_>,
    rhs: jni::objects::JLongArray<'_>,
    out: jni::objects::JLongArray<'_>,
) -> jni::sys::jboolean {
    use jni::sys::{JNI_FALSE, JNI_TRUE};
    let lhs = match convert_field_elems(&mut env, &lhs, "bn254MulBatch lhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    let rhs = match convert_field_elems(&mut env, &rhs, "bn254MulBatch rhs") {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if lhs.len() != rhs.len() {
        throw_java_illegal_argument(
            &mut env,
            "bn254MulBatch expects matching batch lengths".into(),
        );
        return JNI_FALSE;
    }
    let out_len = match i32::try_from(lhs.len().saturating_mul(4)) {
        Ok(value) => value,
        Err(_) => {
            throw_java_illegal_argument(
                &mut env,
                "bn254MulBatch output exceeds Java array limits".into(),
            );
            return JNI_FALSE;
        }
    };
    if !ensure_min_array_length(&mut env, &out, out_len, "bn254MulBatch") {
        return JNI_FALSE;
    }
    let result = match catch_unwind_to_java(&mut env, "bn254_mul_batch_cuda", || {
        ivm::bn254_mul_batch_cuda(&lhs, &rhs)
    }) {
        Some(value) => value,
        None => return JNI_FALSE,
    };
    if let Some(fields) = result {
        let values: Vec<i64> = fields
            .into_iter()
            .flat_map(|field| field.into_iter().map(|limb| limb as i64))
            .collect();
        if write_long_array(&mut env, &out, &values, "bn254MulBatch") {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
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
