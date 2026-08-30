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
    nativeProjectReadinessV4 { readiness bytes } -> JniObjectArray = java_native_kagemusha_project_readiness_v4;
    nativeProjectAuthenticatedArtifactSetV4 { artifact_set bytes } -> JniObjectArray = java_native_kagemusha_project_authenticated_artifact_set_v4;
    nativeProjectActiveVerifierV2 { verifier bytes } -> JniObjectArray = java_native_kagemusha_project_active_verifier_v2;
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
    nativeProjectTopUpSubmissionRequestV4 { request bytes } -> JniObjectArray = java_native_kagemusha_project_top_up_submission_request_v4;
    nativeProjectRedeemSubmissionRequestV4 { request bytes } -> JniObjectArray = java_native_kagemusha_project_redeem_submission_request_v4;
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

fn java_offline_cash_v1_result(
    env: &mut jni::JNIEnv<'_>,
    label: &str,
    result: Result<Vec<u8>, String>,
) -> jni::sys::jbyteArray {
    match result.and_then(|bytes| {
        env.byte_array_from_slice(&bytes)
            .map(jni::objects::JByteArray::into_raw)
            .map_err(|error| format!("failed to copy native result: {error}"))
    }) {
        Ok(array) => array,
        Err(message) => {
            throw_java_illegal_argument(env, format!("Offline Cash V1 {label}: {message}"));
            std::ptr::null_mut()
        }
    }
}

fn java_offline_cash_v1_request(
    env: &mut jni::JNIEnv<'_>,
    request: &jni::objects::JByteArray<'_>,
) -> Result<OfflineCashPaymentRequestV1, String> {
    let bytes = read_java_byte_array_bounded(
        env,
        request,
        "requestNorito",
        OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
    )
    .ok_or_else(|| "requestNorito is missing or oversized".to_owned())?;
    OfflineCashPaymentRequestV1::decode_canonical_exact(&bytes)
        .map_err(|error| format!("requestNorito is invalid: {error}"))
}

fn java_offline_cash_v1_payment(
    env: &mut jni::JNIEnv<'_>,
    request: &OfflineCashPaymentRequestV1,
    payment: &jni::objects::JByteArray<'_>,
) -> Result<OfflineCashPaymentV1, String> {
    let bytes = read_java_byte_array_bounded(
        env,
        payment,
        "paymentNorito",
        OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
    )
    .ok_or_else(|| "paymentNorito is missing or oversized".to_owned())?;
    OfflineCashPaymentV1::decode_canonical_exact_against(&bytes, request)
        .map_err(|error| format!("paymentNorito is invalid: {error}"))
}

pub(super) fn java_offline_cash_v1_canonicalize_request(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = java_offline_cash_v1_request(env, &request)
        .and_then(|request| norito::encode_canonical(&request).map_err(|error| error.to_string()));
    java_offline_cash_v1_result(env, "request", result)
}

pub(super) fn java_offline_cash_v1_canonicalize_payment(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = java_offline_cash_v1_request(env, &request)
        .and_then(|request| java_offline_cash_v1_payment(env, &request, &payment))
        .and_then(|payment| norito::encode_canonical(&payment).map_err(|error| error.to_string()));
    java_offline_cash_v1_result(env, "payment", result)
}

pub(super) fn java_offline_cash_v1_canonicalize_payment_for_verification_session(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    expected_artifact_manifest_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = java_offline_cash_v1_request(env, &request).and_then(|request| {
        java_offline_cash_v1_payment(env, &request, &payment).and_then(|payment| {
            let expected = read_java_byte_array_bounded(
                env,
                &expected_artifact_manifest_sha256,
                "expectedArtifactManifestSHA256",
                32,
            )
            .ok_or_else(|| "expectedArtifactManifestSHA256 is missing or oversized".to_owned())?;
            let expected: [u8; 32] = expected.try_into().map_err(|_| {
                "expectedArtifactManifestSHA256 must be exactly 32 bytes".to_owned()
            })?;
            if expected == [0; 32] {
                return Err("verification session artifact manifest must be non-zero".to_owned());
            }
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "system time precedes the Unix epoch".to_owned())?
                .as_millis()
                .try_into()
                .map_err(|_| "system time exceeds the Offline Cash range".to_owned())?;
            offline_cash_v1_bridge::verify_payment_once(&request, &payment, expected, now_ms)
                .map_err(|_| {
                    "authenticated Offline Cash release or paired proof rejected the payment"
                        .to_owned()
                })
        })
    });
    java_offline_cash_v1_result(env, "verification session payment", result)
}

pub(super) fn java_offline_cash_v1_canonicalize_acknowledgement(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    acknowledgement: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = java_offline_cash_v1_request(env, &request).and_then(|request| {
        java_offline_cash_v1_payment(env, &request, &payment).and_then(|payment| {
            let bytes = read_java_byte_array_bounded(
                env,
                &acknowledgement,
                "acknowledgementNorito",
                OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            )
            .ok_or_else(|| "acknowledgementNorito is missing or oversized".to_owned())?;
            OfflineCashAcknowledgementV1::decode_canonical_exact_against(&bytes, &request, &payment)
                .map_err(|error| format!("acknowledgementNorito is invalid: {error}"))
                .and_then(|value| {
                    norito::encode_canonical(&value).map_err(|error| error.to_string())
                })
        })
    });
    java_offline_cash_v1_result(env, "acknowledgement", result)
}

pub(super) fn java_offline_cash_v1_peer_encode_request(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = java_offline_cash_v1_request(env, &request)
        .and_then(|request| {
            OfflineCashPeerAdapterV1
                .encode_payment_request(&request)
                .map_err(|error| error.to_string())
        })
        .map(String::into_bytes);
    java_offline_cash_v1_result(env, "request peer encoding", result)
}

pub(super) fn java_offline_cash_v1_peer_decode_request(
    env: &mut jni::JNIEnv<'_>,
    text: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = read_java_byte_array_bounded(
        env,
        &text,
        "peerText",
        iroha_data_model::offline::OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1,
    )
    .ok_or_else(|| "peerText is missing or oversized".to_owned())
    .and_then(|bytes| String::from_utf8(bytes).map_err(|error| error.to_string()))
    .and_then(|text| {
        OfflineCashPeerAdapterV1
            .decode_payment_request(&text)
            .map_err(|error| error.to_string())
    })
    .and_then(|request| norito::encode_canonical(&request).map_err(|error| error.to_string()));
    java_offline_cash_v1_result(env, "request peer decoding", result)
}

pub(super) fn java_offline_cash_v1_peer_encode_payment(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = java_offline_cash_v1_request(env, &request).and_then(|request| {
        java_offline_cash_v1_payment(env, &request, &payment).and_then(|payment| {
            OfflineCashPeerAdapterV1
                .encode_payment(&request, &payment)
                .map(String::into_bytes)
                .map_err(|error| error.to_string())
        })
    });
    java_offline_cash_v1_result(env, "payment peer encoding", result)
}

pub(super) fn java_offline_cash_v1_peer_decode_payment(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    text: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = java_offline_cash_v1_request(env, &request).and_then(|request| {
        read_java_byte_array_bounded(
            env,
            &text,
            "peerText",
            iroha_data_model::offline::OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1,
        )
        .ok_or_else(|| "peerText is missing or oversized".to_owned())
        .and_then(|bytes| String::from_utf8(bytes).map_err(|error| error.to_string()))
        .and_then(|text| {
            OfflineCashPeerAdapterV1
                .decode_payment(&request, &text)
                .map_err(|error| error.to_string())
        })
        .and_then(|payment| norito::encode_canonical(&payment).map_err(|error| error.to_string()))
    });
    java_offline_cash_v1_result(env, "payment peer decoding", result)
}

pub(super) fn java_offline_cash_v1_peer_encode_acknowledgement(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    acknowledgement: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = java_offline_cash_v1_request(env, &request).and_then(|request| {
        java_offline_cash_v1_payment(env, &request, &payment).and_then(|payment| {
            let bytes = read_java_byte_array_bounded(
                env,
                &acknowledgement,
                "acknowledgementNorito",
                OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            )
            .ok_or_else(|| "acknowledgementNorito is missing or oversized".to_owned())?;
            let acknowledgement = OfflineCashAcknowledgementV1::decode_canonical_exact_against(
                &bytes, &request, &payment,
            )
            .map_err(|error| error.to_string())?;
            OfflineCashPeerAdapterV1
                .encode_acknowledgement(&request, &payment, &acknowledgement)
                .map(String::into_bytes)
                .map_err(|error| error.to_string())
        })
    });
    java_offline_cash_v1_result(env, "acknowledgement peer encoding", result)
}

pub(super) fn java_offline_cash_v1_peer_decode_acknowledgement(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    text: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = java_offline_cash_v1_request(env, &request).and_then(|request| {
        java_offline_cash_v1_payment(env, &request, &payment).and_then(|payment| {
            read_java_byte_array_bounded(
                env,
                &text,
                "peerText",
                iroha_data_model::offline::OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1,
            )
            .ok_or_else(|| "peerText is missing or oversized".to_owned())
            .and_then(|bytes| String::from_utf8(bytes).map_err(|error| error.to_string()))
            .and_then(|text| {
                OfflineCashPeerAdapterV1
                    .decode_acknowledgement(&request, &payment, &text)
                    .map_err(|error| error.to_string())
            })
            .and_then(|value| norito::encode_canonical(&value).map_err(|error| error.to_string()))
        })
    });
    java_offline_cash_v1_result(env, "acknowledgement peer decoding", result)
}

pub(super) fn java_offline_cash_v1_release_probe(
    env: &mut jni::JNIEnv<'_>,
) -> jni::sys::jobjectArray {
    let fields = match offline_cash_v1_bridge::release_probe() {
        Ok(Some((release_id, manifest_digest))) => vec![
            vec![1],
            release_id.to_vec(),
            manifest_digest.to_vec(),
            CONNECT_NORITO_BRIDGE_ABI_VERSION.to_be_bytes().to_vec(),
        ],
        Ok(None) => vec![
            vec![0],
            vec![0; 32],
            vec![0; 32],
            CONNECT_NORITO_BRIDGE_ABI_VERSION.to_be_bytes().to_vec(),
        ],
        Err(_) => {
            throw_java_illegal_state(
                env,
                "Offline Cash V1 release registry lock is unavailable".to_owned(),
            );
            return std::ptr::null_mut();
        }
    };
    java_kagemusha_byte_arrays(env, &fields).unwrap_or_else(|message| {
        throw_java_illegal_state(env, format!("Offline Cash V1 release probe: {message}"));
        std::ptr::null_mut()
    })
}

pub(super) fn java_offline_cash_v1_artifact_begin(
    env: &mut jni::JNIEnv<'_>,
    manifest: jni::objects::JByteArray<'_>,
    role: jni::sys::jint,
) -> jni::sys::jlong {
    let result = (|| -> Result<jni::sys::jlong, String> {
        let role = u8::try_from(role)
            .ok()
            .filter(|role| {
                usize::from(*role) < iroha_data_model::offline::OfflineCashArtifactRoleV1::ALL.len()
            })
            .ok_or_else(|| "artifact role is outside the canonical 34-role inventory".to_owned())?;
        let manifest = read_java_byte_array_bounded(
            env,
            &manifest,
            "manifestNorito",
            iroha_data_model::offline::OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1,
        )
        .ok_or_else(|| "manifestNorito is missing or oversized".to_owned())?;
        let mut handle = 0_u64;
        let status = unsafe {
            offline_cash_v1_bridge::connect_norito_offline_cash_artifact_begin_v1(
                manifest.as_ptr(),
                c_ulong::try_from(manifest.len())
                    .map_err(|_| "manifestNorito exceeds the native range".to_owned())?,
                role,
                &mut handle,
            )
        };
        if status != 0 || handle == 0 {
            return Err(format!(
                "authenticated artifact begin rejected with native status {status}"
            ));
        }
        i64::try_from(handle).map_err(|_| "artifact handle exceeds the JNI range".to_owned())
    })();
    match result {
        Ok(handle) => handle,
        Err(message) => {
            throw_java_illegal_argument(env, format!("Offline Cash V1 artifact begin: {message}"));
            0
        }
    }
}

pub(super) fn java_offline_cash_v1_artifact_write(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
    chunk: jni::objects::JByteArray<'_>,
) {
    let result = (|| -> Result<(), String> {
        let handle = java_offline_cash_v1_positive_handle(handle)?;
        let chunk = read_java_byte_array_bounded(env, &chunk, "artifactChunk", 1024 * 1024)
            .filter(|chunk| !chunk.is_empty())
            .ok_or_else(|| "artifactChunk is missing or oversized".to_owned())?;
        let status = unsafe {
            offline_cash_v1_bridge::connect_norito_offline_cash_artifact_write_v1(
                handle,
                chunk.as_ptr(),
                c_ulong::try_from(chunk.len())
                    .map_err(|_| "artifactChunk exceeds the native range".to_owned())?,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "artifact write rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, format!("Offline Cash V1 artifact write: {message}"));
    }
}

pub(super) fn java_offline_cash_v1_artifact_finish(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
    cancel: bool,
) {
    let result = (|| -> Result<(), String> {
        let handle = java_offline_cash_v1_positive_handle(handle)?;
        let status = if cancel {
            offline_cash_v1_bridge::connect_norito_offline_cash_artifact_cancel_v1(handle)
        } else {
            offline_cash_v1_bridge::connect_norito_offline_cash_artifact_finalize_v1(handle)
        };
        if status == 0 {
            Ok(())
        } else {
            let operation = if cancel { "cancel" } else { "finalize" };
            Err(format!(
                "artifact {operation} rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(env, format!("Offline Cash V1 artifact: {message}"));
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn java_offline_cash_v1_artifact_set_install(
    env: &mut jni::JNIEnv<'_>,
    manifest: jni::objects::JByteArray<'_>,
    expected_manifest_sha256: jni::objects::JByteArray<'_>,
    validation_receipt: jni::objects::JByteArray<'_>,
    trusted_policy: jni::objects::JByteArray<'_>,
    release_attestation: jni::objects::JByteArray<'_>,
    handles: jni::objects::JLongArray<'_>,
) {
    let result = (|| -> Result<(), String> {
        let manifest = read_java_byte_array_bounded(
            env,
            &manifest,
            "manifestNorito",
            iroha_data_model::offline::OFFLINE_CASH_RELEASE_MANIFEST_MAX_BYTES_V1,
        )
        .ok_or_else(|| "manifestNorito is missing or oversized".to_owned())?;
        let expected_manifest_sha256 = read_java_byte_array_bounded(
            env,
            &expected_manifest_sha256,
            "expectedManifestSHA256",
            32,
        )
        .filter(|digest| digest.len() == 32 && digest.iter().any(|byte| *byte != 0))
        .ok_or_else(|| "expectedManifestSHA256 must be a non-zero digest".to_owned())?;
        let validation_receipt = read_java_byte_array_bounded(
            env,
            &validation_receipt,
            "validationReceiptNorito",
            1024 * 1024,
        )
        .ok_or_else(|| "validationReceiptNorito is missing or oversized".to_owned())?;
        let trusted_policy = read_java_byte_array_bounded(
            env,
            &trusted_policy,
            "trustedPolicyNorito",
            iroha_data_model::offline::OFFLINE_CASH_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
        )
        .ok_or_else(|| "trustedPolicyNorito is missing or oversized".to_owned())?;
        let release_attestation = read_java_byte_array_bounded(
            env,
            &release_attestation,
            "releaseAttestationNorito",
            iroha_data_model::offline::OFFLINE_CASH_RELEASE_ATTESTATION_MAX_BYTES_V1,
        )
        .ok_or_else(|| "releaseAttestationNorito is missing or oversized".to_owned())?;
        let artifact_count = iroha_data_model::offline::OfflineCashArtifactRoleV1::ALL.len();
        if env
            .get_array_length(&handles)
            .map_err(|error| format!("failed to read artifact handles: {error}"))?
            != i32::try_from(artifact_count).expect("artifact count fits JNI")
        {
            return Err("install requires exactly 34 ordered artifact handles".to_owned());
        }
        let mut jni_handles = vec![0_i64; artifact_count];
        env.get_long_array_region(&handles, 0, &mut jni_handles)
            .map_err(|error| format!("failed to read artifact handles: {error}"))?;
        let native_handles = jni_handles
            .into_iter()
            .map(|handle| {
                u64::try_from(handle)
                    .ok()
                    .filter(|handle| *handle != 0)
                    .ok_or_else(|| "artifact handles must be positive".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let status = unsafe {
            offline_cash_v1_bridge::connect_norito_offline_cash_artifact_set_install_v1(
                manifest.as_ptr(),
                c_ulong::try_from(manifest.len())
                    .map_err(|_| "manifestNorito exceeds the native range".to_owned())?,
                expected_manifest_sha256.as_ptr(),
                32,
                validation_receipt.as_ptr(),
                c_ulong::try_from(validation_receipt.len())
                    .map_err(|_| "validation receipt exceeds the native range".to_owned())?,
                trusted_policy.as_ptr(),
                c_ulong::try_from(trusted_policy.len())
                    .map_err(|_| "trusted policy exceeds the native range".to_owned())?,
                release_attestation.as_ptr(),
                c_ulong::try_from(release_attestation.len())
                    .map_err(|_| "release attestation exceeds the native range".to_owned())?,
                native_handles.as_ptr(),
                c_ulong::try_from(native_handles.len())
                    .map_err(|_| "artifact inventory exceeds the native range".to_owned())?,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "authenticated artifact-set install rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(
            env,
            format!("Offline Cash V1 artifact-set install: {message}"),
        );
    }
}

pub(super) fn java_offline_cash_v1_artifact_set_uninstall(
    env: &mut jni::JNIEnv<'_>,
    expected_release_id: jni::objects::JByteArray<'_>,
    expected_manifest_sha256: jni::objects::JByteArray<'_>,
) {
    let result = (|| -> Result<(), String> {
        let expected_release_id =
            read_java_byte_array_bounded(env, &expected_release_id, "expectedReleaseId", 32)
                .filter(|digest| digest.len() == 32 && digest.iter().any(|byte| *byte != 0))
                .ok_or_else(|| "expectedReleaseId must be a non-zero digest".to_owned())?;
        let expected_manifest_sha256 = read_java_byte_array_bounded(
            env,
            &expected_manifest_sha256,
            "expectedManifestSHA256",
            32,
        )
        .filter(|digest| digest.len() == 32 && digest.iter().any(|byte| *byte != 0))
        .ok_or_else(|| "expectedManifestSHA256 must be a non-zero digest".to_owned())?;
        let status = unsafe {
            offline_cash_v1_bridge::connect_norito_offline_cash_artifact_set_uninstall_v1(
                expected_release_id.as_ptr(),
                32,
                expected_manifest_sha256.as_ptr(),
                32,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(format!(
                "artifact-set uninstall rejected with native status {status}"
            ))
        }
    })();
    if let Err(message) = result {
        throw_java_illegal_state(
            env,
            format!("Offline Cash V1 artifact-set uninstall: {message}"),
        );
    }
}

fn java_offline_cash_v1_positive_handle(handle: jni::sys::jlong) -> Result<u64, String> {
    u64::try_from(handle)
        .ok()
        .filter(|handle| *handle != 0)
        .ok_or_else(|| "native handle must be positive".to_owned())
}

fn java_offline_cash_v1_verification_session_handle(
    handle: jni::sys::jlong,
) -> Result<u64, String> {
    java_offline_cash_v1_positive_handle(handle)
        .map_err(|_| "verification session handle must be positive".to_owned())
}

pub(super) fn java_offline_cash_v1_verification_session_open(
    env: &mut jni::JNIEnv<'_>,
    _request: jni::objects::JByteArray<'_>,
    _expected_release_id: jni::objects::JByteArray<'_>,
    _expected_artifact_manifest_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jlong {
    throw_java_illegal_state(
        env,
        "Offline Cash V1 verification session open requires exact network and asset context"
            .to_owned(),
    );
    0
}

pub(super) fn java_offline_cash_v1_verification_session_open_bound(
    env: &mut jni::JNIEnv<'_>,
    request: jni::objects::JByteArray<'_>,
    expected_release_id: jni::objects::JByteArray<'_>,
    expected_artifact_manifest_sha256: jni::objects::JByteArray<'_>,
    expected_network_id: jni::objects::JByteArray<'_>,
    expected_asset_definition_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jlong {
    let result = (|| -> Result<jni::sys::jlong, String> {
        let request = read_java_byte_array_bounded(
            env,
            &request,
            "requestNorito",
            OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
        )
        .ok_or_else(|| "requestNorito is missing or oversized".to_owned())?;
        let release_id =
            read_java_byte_array_bounded(env, &expected_release_id, "expectedReleaseId", 32)
                .ok_or_else(|| "expectedReleaseId is missing or oversized".to_owned())?;
        let release_id: [u8; 32] = release_id
            .try_into()
            .map_err(|_| "expectedReleaseId must be exactly 32 bytes".to_owned())?;
        let manifest_digest = read_java_byte_array_bounded(
            env,
            &expected_artifact_manifest_sha256,
            "expectedArtifactManifestSHA256",
            32,
        )
        .ok_or_else(|| "expectedArtifactManifestSHA256 is missing or oversized".to_owned())?;
        let manifest_digest: [u8; 32] = manifest_digest
            .try_into()
            .map_err(|_| "expectedArtifactManifestSHA256 must be exactly 32 bytes".to_owned())?;
        if release_id == [0; 32] || manifest_digest == [0; 32] {
            return Err("verification session release identities must be non-zero".to_owned());
        }
        let expected_network_id =
            read_java_byte_array_bounded(env, &expected_network_id, "expectedNetworkId", 64)
                .filter(|literal| literal.len() == 64)
                .ok_or_else(|| "expectedNetworkId must be an exact 64-byte literal".to_owned())?;
        let expected_asset_definition_id = read_java_byte_array_bounded(
            env,
            &expected_asset_definition_id,
            "expectedAssetDefinitionId",
            64,
        )
        .filter(|literal| !literal.is_empty())
        .ok_or_else(|| {
            "expectedAssetDefinitionId must be a bounded canonical literal".to_owned()
        })?;
        let handle = offline_cash_v1_bridge::open_verification_session_canonical_bound(
            &request,
            release_id,
            manifest_digest,
            &expected_network_id,
            &expected_asset_definition_id,
        )
        .map_err(|_| {
            "active authenticated release does not match the verification session".to_owned()
        })?;
        i64::try_from(handle)
            .map_err(|_| "verification session handle exceeds JNI range".to_owned())
    })();
    match result {
        Ok(handle) => handle,
        Err(message) => {
            throw_java_illegal_state(
                env,
                format!("Offline Cash V1 verification session open: {message}"),
            );
            0
        }
    }
}

pub(super) fn java_offline_cash_v1_verification_session_verify_payment(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
    payment: jni::objects::JByteArray<'_>,
    observed_now_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<Vec<u8>, String> {
        let handle = java_offline_cash_v1_verification_session_handle(handle)?;
        let observed_now_ms = u64::try_from(observed_now_ms)
            .ok()
            .filter(|value| *value != 0)
            .ok_or_else(|| "observedNowMs must be positive".to_owned())?;
        let payment = read_java_byte_array_bounded(
            env,
            &payment,
            "paymentNorito",
            OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
        )
        .ok_or_else(|| "paymentNorito is missing or oversized".to_owned())?;
        offline_cash_v1_bridge::verify_verification_session_payment_canonical(
            handle,
            &payment,
            observed_now_ms,
        )
        .map_err(|_| "paired proof or verification transition was rejected".to_owned())
    })();
    java_offline_cash_v1_result(env, "verification session payment", result)
}

pub(super) fn java_offline_cash_v1_verification_session_verify_acknowledgement(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
    acknowledgement: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    let result = (|| -> Result<Vec<u8>, String> {
        let handle = java_offline_cash_v1_verification_session_handle(handle)?;
        let acknowledgement = read_java_byte_array_bounded(
            env,
            &acknowledgement,
            "acknowledgementNorito",
            OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
        )
        .ok_or_else(|| "acknowledgementNorito is missing or oversized".to_owned())?;
        offline_cash_v1_bridge::verify_verification_session_acknowledgement_canonical(
            handle,
            &acknowledgement,
        )
        .map_err(|_| "acknowledgement or retained proof receipt was rejected".to_owned())
    })();
    java_offline_cash_v1_result(env, "verification session acknowledgement", result)
}

pub(super) fn java_offline_cash_v1_verification_session_state(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
) -> jni::sys::jint {
    let result = java_offline_cash_v1_verification_session_handle(handle).and_then(|handle| {
        offline_cash_v1_bridge::verification_session_state_code(handle)
            .map(i32::from)
            .map_err(|_| {
                "verification session is closed or its release is no longer active".to_owned()
            })
    });
    match result {
        Ok(state) => state,
        Err(message) => {
            throw_java_illegal_state(
                env,
                format!("Offline Cash V1 verification session state: {message}"),
            );
            0
        }
    }
}

pub(super) fn java_offline_cash_v1_verification_session_close(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
) {
    let result = java_offline_cash_v1_verification_session_handle(handle).and_then(|handle| {
        offline_cash_v1_bridge::close_verification_session(handle)
            .map_err(|_| "verification session is already closed or invalid".to_owned())
    });
    if let Err(message) = result {
        throw_java_illegal_state(
            env,
            format!("Offline Cash V1 verification session close: {message}"),
        );
    }
}

pub(super) fn java_offline_cash_v1_wallet_runtime_session_open(
    env: &mut jni::JNIEnv<'_>,
) -> jni::sys::jlong {
    let mut handle = u64::MAX;
    let status = unsafe {
        offline_cash_v1_bridge::connect_norito_offline_cash_wallet_runtime_session_open_v1(
            &mut handle,
        )
    };
    if handle != 0 {
        throw_java_illegal_state(
            env,
            "Offline Cash V1 wallet runtime fabricated a handle while unavailable".to_owned(),
        );
        return 0;
    }
    throw_java_illegal_state(
        env,
        format!("Offline Cash V1 wallet runtime is unavailable (native status {status})"),
    );
    0
}

pub(super) fn java_offline_cash_v1_wallet_runtime_session_status(
    env: &mut jni::JNIEnv<'_>,
) -> jni::sys::jbyteArray {
    let mut status_code = u8::MAX;
    let mut state_code = u8::MAX;
    let native_status = unsafe {
        offline_cash_v1_bridge::connect_norito_offline_cash_wallet_runtime_session_status_v1(
            &mut status_code,
            &mut state_code,
        )
    };
    let result = if native_status == 0 && status_code == 0 && state_code == 0 {
        Ok(vec![status_code, state_code])
    } else {
        Err("wallet runtime status must remain unavailable".to_owned())
    };
    java_offline_cash_v1_result(env, "wallet runtime status", result)
}

pub(super) fn java_offline_cash_v1_wallet_runtime_session_attempt(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
    action: jni::sys::jint,
) {
    let handle = u64::try_from(handle).unwrap_or(0);
    let action = u8::try_from(action).unwrap_or(u8::MAX);
    let status =
        offline_cash_v1_bridge::connect_norito_offline_cash_wallet_runtime_session_attempt_v1(
            handle, action,
        );
    throw_java_illegal_state(
        env,
        format!("Offline Cash V1 wallet runtime action is unavailable (native status {status})"),
    );
}

pub(super) fn java_offline_cash_v1_wallet_runtime_session_close(
    env: &mut jni::JNIEnv<'_>,
    handle: jni::sys::jlong,
) {
    let handle = u64::try_from(handle).unwrap_or(0);
    let status =
        offline_cash_v1_bridge::connect_norito_offline_cash_wallet_runtime_session_close_v1(
            handle,
        );
    throw_java_illegal_state(
        env,
        format!("Offline Cash V1 wallet runtime close is unavailable (native status {status})"),
    );
}

jni_sdk_android_pairs! {
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeCanonicalizePaymentRequestV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeCanonicalizePaymentRequestV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_canonicalize_request(&mut env, request)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeCanonicalizePaymentV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeCanonicalizePaymentV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_canonicalize_payment(&mut env, request, payment)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeCanonicalizePaymentForSessionV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeCanonicalizePaymentForSessionV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    expected_artifact_manifest_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_canonicalize_payment_for_verification_session(
        &mut env,
        request,
        payment,
        expected_artifact_manifest_sha256,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeCanonicalizeAcknowledgementV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeCanonicalizeAcknowledgementV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    acknowledgement: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_canonicalize_acknowledgement(
        &mut env,
        request,
        payment,
        acknowledgement,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativePeerEncodePaymentRequestV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerEncodePaymentRequestV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_peer_encode_request(&mut env, request)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativePeerDecodePaymentRequestV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerDecodePaymentRequestV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    text: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_peer_decode_request(&mut env, text)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativePeerEncodePaymentV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerEncodePaymentV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_peer_encode_payment(&mut env, request, payment)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativePeerDecodePaymentV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerDecodePaymentV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    text: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_peer_decode_payment(&mut env, request, text)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativePeerEncodeAcknowledgementV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerEncodeAcknowledgementV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    acknowledgement: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_peer_encode_acknowledgement(
        &mut env,
        request,
        payment,
        acknowledgement,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativePeerDecodeAcknowledgementV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativePeerDecodeAcknowledgementV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    text: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_peer_decode_acknowledgement(&mut env, request, payment, text)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeReleaseProbeV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeReleaseProbeV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jobjectArray {
    java_offline_cash_v1_release_probe(&mut env)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeVerificationSessionOpenV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionOpenV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    expected_release_id: jni::objects::JByteArray<'_>,
    expected_artifact_manifest_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jlong {
    java_offline_cash_v1_verification_session_open(
        &mut env,
        request,
        expected_release_id,
        expected_artifact_manifest_sha256,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeVerificationSessionOpenBoundV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionOpenBoundV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    expected_release_id: jni::objects::JByteArray<'_>,
    expected_artifact_manifest_sha256: jni::objects::JByteArray<'_>,
    expected_network_id: jni::objects::JByteArray<'_>,
    expected_asset_definition_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jlong {
    java_offline_cash_v1_verification_session_open_bound(
        &mut env,
        request,
        expected_release_id,
        expected_artifact_manifest_sha256,
        expected_network_id,
        expected_asset_definition_id,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletSessionOpenV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionOpenV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    expected_release_id: jni::objects::JByteArray<'_>,
    expected_artifact_manifest_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jlong {
    java_offline_cash_v1_verification_session_open(
        &mut env,
        request,
        expected_release_id,
        expected_artifact_manifest_sha256,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletSessionOpenBoundV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionOpenBoundV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    expected_release_id: jni::objects::JByteArray<'_>,
    expected_artifact_manifest_sha256: jni::objects::JByteArray<'_>,
    expected_network_id: jni::objects::JByteArray<'_>,
    expected_asset_definition_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jlong {
    java_offline_cash_v1_verification_session_open_bound(
        &mut env,
        request,
        expected_release_id,
        expected_artifact_manifest_sha256,
        expected_network_id,
        expected_asset_definition_id,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeArtifactBeginV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactBeginV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    manifest: jni::objects::JByteArray<'_>,
    role: jni::sys::jint,
) -> jni::sys::jlong {
    java_offline_cash_v1_artifact_begin(&mut env, manifest, role)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeArtifactWriteV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactWriteV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
    chunk: jni::objects::JByteArray<'_>,
) {
    java_offline_cash_v1_artifact_write(&mut env, handle, chunk)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeArtifactFinalizeV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactFinalizeV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) {
    java_offline_cash_v1_artifact_finish(&mut env, handle, false)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeArtifactCancelV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactCancelV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) {
    java_offline_cash_v1_artifact_finish(&mut env, handle, true)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeArtifactSetInstallV1();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactSetInstallV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    manifest: jni::objects::JByteArray<'_>,
    expected_manifest_sha256: jni::objects::JByteArray<'_>,
    validation_receipt: jni::objects::JByteArray<'_>,
    trusted_policy: jni::objects::JByteArray<'_>,
    release_attestation: jni::objects::JByteArray<'_>,
    handles: jni::objects::JLongArray<'_>,
) {
    java_offline_cash_v1_artifact_set_install(
        &mut env,
        manifest,
        expected_manifest_sha256,
        validation_receipt,
        trusted_policy,
        release_attestation,
        handles,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeArtifactSetUninstallV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeArtifactSetUninstallV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    expected_release_id: jni::objects::JByteArray<'_>,
    expected_manifest_sha256: jni::objects::JByteArray<'_>,
) {
    java_offline_cash_v1_artifact_set_uninstall(
        &mut env,
        expected_release_id,
        expected_manifest_sha256,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeVerificationSessionVerifyPaymentV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionVerifyPaymentV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
    payment: jni::objects::JByteArray<'_>,
    observed_now_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_verification_session_verify_payment(
        &mut env,
        handle,
        payment,
        observed_now_ms,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeVerificationSessionVerifyAcknowledgementV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionVerifyAcknowledgementV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
    acknowledgement: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_verification_session_verify_acknowledgement(
        &mut env,
        handle,
        acknowledgement,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeVerificationSessionStateV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionStateV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) -> jni::sys::jint {
    java_offline_cash_v1_verification_session_state(&mut env, handle)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeVerificationSessionCloseV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeVerificationSessionCloseV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) {
    java_offline_cash_v1_verification_session_close(&mut env, handle)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionOpenV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionOpenV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jlong {
    java_offline_cash_v1_wallet_runtime_session_open(&mut env)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionStatusV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionStatusV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_wallet_runtime_session_status(&mut env)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionAttemptV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionAttemptV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
    action: jni::sys::jint,
) {
    java_offline_cash_v1_wallet_runtime_session_attempt(&mut env, handle, action)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionCloseV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletRuntimeSessionCloseV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) {
    java_offline_cash_v1_wallet_runtime_session_close(&mut env, handle)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletSessionAcceptPaymentV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionAcceptPaymentV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
    payment: jni::objects::JByteArray<'_>,
    observed_now_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_verification_session_verify_payment(
        &mut env,
        handle,
        payment,
        observed_now_ms,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletSessionAcceptAcknowledgementV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionAcceptAcknowledgementV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
    acknowledgement: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_offline_cash_v1_verification_session_verify_acknowledgement(
        &mut env,
        handle,
        acknowledgement,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletSessionStateV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionStateV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) -> jni::sys::jint {
    java_offline_cash_v1_verification_session_state(&mut env, handle)
}
android: fn Java_org_hyperledger_iroha_android_offline_OfflineCashNativeV1_nativeWalletSessionCloseV1();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_OfflineCashNativeV1_nativeWalletSessionCloseV1(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) {
    java_offline_cash_v1_verification_session_close(&mut env, handle)
}
}
