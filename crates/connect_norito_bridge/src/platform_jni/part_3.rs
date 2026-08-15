macro_rules! jni_sdk_android_pairs {
    (
        $(
            android: $(#[$android_attribute:meta])* fn $android_name:ident();
            sdk: $(#[$sdk_attribute:meta])*
            pub unsafe extern "system" fn $sdk_name:ident(
                $($argument:tt)*
            ) -> $return_type:ty $body:block
        )*
    ) => {
        $(
            $(#[$sdk_attribute])*
            pub unsafe extern "system" fn $sdk_name(
                $($argument)*
            ) -> $return_type $body
            $(#[$android_attribute])*
            #[unsafe(no_mangle)]
            pub unsafe extern "system" fn $android_name(
                $($argument)*
            ) -> $return_type $body
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
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeArtifactBeginV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactBeginV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    manifest_norito: jni::objects::JByteArray<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
    artifact_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jlong {
    java_native_kagemusha_artifact_begin_v4(
        &mut env,
        manifest_norito,
        manifest_sha256,
        artifact_sha256,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeArtifactWriteV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactWriteV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
    chunk: jni::objects::JByteArray<'_>,
) {
    java_native_kagemusha_artifact_write_v4(&mut env, handle, chunk);
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeArtifactFinalizeV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactFinalizeV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) {
    java_native_kagemusha_artifact_finish_v4(&mut env, handle, false);
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeArtifactCancelV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactCancelV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    handle: jni::sys::jlong,
) {
    java_native_kagemusha_artifact_finish_v4(&mut env, handle, true);
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeArtifactSetInstallV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactSetInstallV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    manifest_norito: jni::objects::JByteArray<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
    trusted_policy_norito: jni::objects::JByteArray<'_>,
    release_attestation_norito: jni::objects::JByteArray<'_>,
    benchmark_evidence: jni::objects::JByteArray<'_>,
    cryptographic_review: jni::objects::JByteArray<'_>,
    promotion_record_norito: jni::objects::JByteArray<'_>,
    handles: jni::objects::JLongArray<'_>,
) {
    java_native_kagemusha_artifact_set_install_v4(
        &mut env,
        manifest_norito,
        manifest_sha256,
        trusted_policy_norito,
        release_attestation_norito,
        benchmark_evidence,
        cryptographic_review,
        promotion_record_norito,
        handles,
    );
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeArtifactSetIsInstalledV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactSetIsInstalledV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    manifest_norito: jni::objects::JByteArray<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    java_native_kagemusha_artifact_set_is_installed_v4(&mut env, manifest_norito, manifest_sha256)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeInstalledManifestSha256V4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeInstalledManifestSha256V4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_installed_manifest_sha256_v4(&mut env)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBuildArtifactBindingV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBuildArtifactBindingV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    manifest_norito: jni::objects::JByteArray<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_build_artifact_binding_v4(&mut env, manifest_norito, manifest_sha256)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeArtifactSetUninstallV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeArtifactSetUninstallV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    manifest_sha256: jni::objects::JByteArray<'_>,
) {
    java_native_kagemusha_artifact_set_uninstall_v4(&mut env, manifest_sha256);
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeInitSpendV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeInitSpendV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_norito: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_lifecycle_archive_v4(
        &mut env,
        request_norito,
        "V4 init spend",
        KAGEMUSHA_RECURSIVE_SPEND_INIT_LOCAL_MAX_BYTES_V4,
        |request_ptr, request_len, output, output_len| unsafe {
            connect_norito_kagemusha_recursive_spend_init_v4(
                request_ptr,
                request_len,
                output,
                output_len,
            )
        },
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeAppendSpendV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeAppendSpendV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_norito: jni::objects::JByteArray<'_>,
    recipient_request_norito: jni::objects::JByteArray<'_>,
    verified_at_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_append_spend_v4(
        &mut env,
        request_norito,
        recipient_request_norito,
        verified_at_ms,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeVerifySpendV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeVerifySpendV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_norito: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_lifecycle_archive_v4(
        &mut env,
        request_norito,
        "V4 verify spend",
        KAGEMUSHA_RECURSIVE_SPEND_VERIFY_LOCAL_MAX_BYTES_V4,
        |request_ptr, request_len, output, output_len| unsafe {
            connect_norito_kagemusha_recursive_spend_verify_v4(
                request_ptr,
                request_len,
                output,
                output_len,
            )
        },
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBuildRedeemV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBuildRedeemV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request_norito: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_lifecycle_archive_v4(
        &mut env,
        request_norito,
        "V4 build redeem",
        KAGEMUSHA_RECURSIVE_SPEND_REDEEM_LOCAL_MAX_BYTES_V4,
        |request_ptr, request_len, output, output_len| unsafe {
            connect_norito_kagemusha_recursive_spend_redeem_v4(
                request_ptr,
                request_len,
                output,
                output_len,
            )
        },
    )
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativePrepareRecipientRequestV2();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativePrepareRecipientRequestV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
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
    java_native_kagemusha_prepare_recipient_request_v2(
        &mut env,
        network_id,
        chain_discriminant,
        asset,
        atomic_units,
        scale,
        recipient,
        receiver_device_id,
        receiver_public_key,
        request_id,
        issued_at_ms,
        expires_at_ms,
        spend_key,
        rho,
        diversifier,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientRequestV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientRequestV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    payload: jni::objects::JByteArray<'_>,
    signature: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_create_recipient_request_v2(&mut env, payload, signature)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientRequestV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientRequestV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    verified_at_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_verify_recipient_request_v2(&mut env, request, verified_at_ms)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientLineageQueryV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientLineageQueryV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    network_id: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    recipient: jni::objects::JByteArray<'_>,
    receiver_device_id: jni::objects::JByteArray<'_>,
    asset: jni::objects::JByteArray<'_>,
    trusted_checkpoint_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_create_recipient_lineage_query_v2(
        &mut env,
        network_id,
        chain_discriminant,
        recipient,
        receiver_device_id,
        asset,
        trusted_checkpoint_height,
    )
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientRegistrationLineageV2();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientRegistrationLineageV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    lineage: jni::objects::JByteArray<'_>,
    verified_at_ms: jni::sys::jlong,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_verify_recipient_registration_lineage_v2(
        &mut env,
        request,
        lineage,
        verified_at_ms,
        trusted_checkpoint_height,
        trusted_checkpoint_context_id,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientReceiveOfferV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateRecipientReceiveOfferV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    lineage: jni::objects::JByteArray<'_>,
    publisher_checkpoint_envelope: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_create_recipient_receive_offer_v2(
        &mut env,
        request,
        lineage,
        publisher_checkpoint_envelope,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectRecipientReceiveOfferV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectRecipientReceiveOfferV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    offer: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_recipient_receive_offer_v2(&mut env, offer)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientReceiveOfferV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeVerifyRecipientReceiveOfferV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    offer: jni::objects::JByteArray<'_>,
    verified_at_ms: jni::sys::jlong,
    trusted_checkpoint_height: jni::sys::jlong,
    trusted_checkpoint_context_id: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_verify_recipient_receive_offer_v2(
        &mut env,
        offer,
        verified_at_ms,
        trusted_checkpoint_height,
        trusted_checkpoint_context_id,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBuildOutputMembershipFrontierV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBuildOutputMembershipFrontierV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    leaf_index: jni::sys::jint,
    flattened_siblings: jni::objects::JByteArray<'_>,
    directions: jni::objects::JByteArray<'_>,
    root: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_build_output_membership_frontier_v4(
        &mut env,
        leaf_index,
        flattened_siblings,
        directions,
        root,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeDeriveOutputMembershipPathsV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeDeriveOutputMembershipPathsV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    frontier: jni::objects::JByteArray<'_>,
    recipient_commitment: jni::objects::JByteArray<'_>,
    change_commitment: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_derive_output_membership_paths_v4(
        &mut env,
        frontier,
        recipient_commitment,
        change_commitment,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeValidateSpendableBranchV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeValidateSpendableBranchV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    bundle: jni::objects::JByteArray<'_>,
    provenance: jni::objects::JByteArray<'_>,
    membership_witness: jni::objects::JByteArray<'_>,
    opening: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_validate_spendable_branch_v4(
        &mut env,
        bundle,
        provenance,
        membership_witness,
        opening,
        block_height,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBuildOutputMembershipPathsV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBuildOutputMembershipPathsV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    initial_root: jni::objects::JByteArray<'_>,
    final_root: jni::objects::JByteArray<'_>,
    recipient_fields: jni::objects::JObjectArray<'_>,
    change_fields: jni::objects::JObjectArray<'_>,
    dummy_fields: jni::objects::JObjectArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_build_output_membership_paths_v4(
        &mut env,
        initial_root,
        final_root,
        recipient_fields,
        change_fields,
        dummy_fields,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBuildInitRequestV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBuildInitRequestV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    anchor: jni::objects::JByteArray<'_>,
    proof: jni::objects::JByteArray<'_>,
    roster: jni::objects::JByteArray<'_>,
    opening: jni::objects::JByteArray<'_>,
    output_membership: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_build_init_request_v4(
        &mut env,
        anchor,
        proof,
        roster,
        opening,
        output_membership,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBuildTopUpProvenanceV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBuildTopUpProvenanceV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    bundle: jni::objects::JByteArray<'_>,
    roster: jni::objects::JByteArray<'_>,
    anchors: jni::objects::JObjectArray<'_>,
    finality_proofs: jni::objects::JObjectArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_build_topup_provenance_v4(
        &mut env,
        bundle,
        roster,
        anchors,
        finality_proofs,
        block_height,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeValidateTopUpProvenanceV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeValidateTopUpProvenanceV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    bundle: jni::objects::JByteArray<'_>,
    provenance: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_validate_topup_provenance_v4(&mut env, bundle, provenance, block_height)
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBuildAppendRequestV4();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBuildAppendRequestV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    bundles: jni::objects::JObjectArray<'_>,
    topup_provenances: jni::objects::JObjectArray<'_>,
    openings: jni::objects::JObjectArray<'_>,
    witnesses: jni::objects::JObjectArray<'_>,
    change_opening: jni::objects::JByteArray<'_>,
    output_membership: jni::objects::JByteArray<'_>,
    verifier_commitment: jni::objects::JByteArray<'_>,
    operation_id: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_build_append_request_v4(
        &mut env,
        bundles,
        topup_provenances,
        openings,
        witnesses,
        change_opening,
        output_membership,
        verifier_commitment,
        operation_id,
        block_height,
    )
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBuildVerifyRequestV4();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBuildVerifyRequestV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    bundle: jni::objects::JByteArray<'_>,
    recipient_request: jni::objects::JByteArray<'_>,
    topup_provenance: jni::objects::JByteArray<'_>,
    maximum_hops: jni::sys::jint,
    block_height: jni::sys::jlong,
    verified_at_ms: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_build_verify_request_v4(
        &mut env,
        bundle,
        recipient_request,
        topup_provenance,
        maximum_hops,
        block_height,
        verified_at_ms,
        JavaKagemushaArtifactRegistryV4::Production,
    )
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBuildRedeemRequestV4();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBuildRedeemRequestV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    bundle: jni::objects::JByteArray<'_>,
    topup_provenance: jni::objects::JByteArray<'_>,
    opening: jni::objects::JByteArray<'_>,
    membership_witness: jni::objects::JByteArray<'_>,
    recipient: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    atomic_units: jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
    change_opening: jni::objects::JByteArray<'_>,
    change_output_membership: jni::objects::JByteArray<'_>,
    verifier_commitment: jni::objects::JByteArray<'_>,
    operation_id: jni::objects::JByteArray<'_>,
    block_height: jni::sys::jlong,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_build_redeem_request_v4(
        &mut env,
        bundle,
        topup_provenance,
        opening,
        membership_witness,
        recipient,
        chain_discriminant,
        atomic_units,
        scale,
        change_opening,
        change_output_membership,
        verifier_commitment,
        operation_id,
        block_height,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectPeerPaymentV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectPeerPaymentV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_peer_payment_v4(&mut env, payment)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectInitResultV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectInitResultV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    result: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_init_result_v4(&mut env, result)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectSplitResultV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectSplitResultV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    result: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_split_result_v4(&mut env, result)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectVerifyResultV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectVerifyResultV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    result: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_verify_result_v4(&mut env, result)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectRedeemBuildResultV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectRedeemBuildResultV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    result: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_redeem_build_result_v4(&mut env, result)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativePrepareAcknowledgementV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativePrepareAcknowledgementV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
    accepted_at_ms: jni::sys::jlong,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_prepare_acknowledgement_v2(&mut env, request, payment, accepted_at_ms)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeCreateAcknowledgementV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeCreateAcknowledgementV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    payload: jni::objects::JByteArray<'_>,
    signature: jni::objects::JByteArray<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_create_acknowledgement_v2(&mut env, payload, signature, request, payment)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeVerifyAcknowledgementV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeVerifyAcknowledgementV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    acknowledgement: jni::objects::JByteArray<'_>,
    request: jni::objects::JByteArray<'_>,
    payment: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_verify_acknowledgement_v2(&mut env, acknowledgement, request, payment)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectReadinessV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectReadinessV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    readiness: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_readiness_v4(&mut env, readiness)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectAuthenticatedArtifactSetV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectAuthenticatedArtifactSetV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    artifact_set: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_authenticated_artifact_set_v4(&mut env, artifact_set)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectActiveVerifierV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectActiveVerifierV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    verifier: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_active_verifier_v2(&mut env, verifier)
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativePrepareAuthorizationV2();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativePrepareAuthorizationV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    authority: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    device_id: jni::objects::JByteArray<'_>,
    asset_definition_id: jni::objects::JByteArray<'_>,
    operation_id: jni::objects::JByteArray<'_>,
    issued_at_ms: jni::sys::jlong,
    expires_at_ms: jni::sys::jlong,
    nonce: jni::objects::JByteArray<'_>,
    payload_digest: jni::objects::JByteArray<'_>,
    registration_hash: jni::objects::JByteArray<'_>,
    hardware_assertion_platform: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_prepare_authorization_v2(
        &mut env,
        authority,
        chain_discriminant,
        device_id,
        asset_definition_id,
        operation_id,
        issued_at_ms,
        expires_at_ms,
        nonce,
        payload_digest,
        registration_hash,
        hardware_assertion_platform,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeFinalizeHardwareAuthorizationV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeFinalizeHardwareAuthorizationV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    preparation: jni::objects::JByteArray<'_>,
    authenticator_data: jni::objects::JByteArray<'_>,
    signature_der: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_finalize_hardware_authorization_v2(
        &mut env,
        preparation,
        authenticator_data,
        signature_der,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeFinalizeIosAppAttestAuthorizationV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeFinalizeIosAppAttestAuthorizationV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    preparation: jni::objects::JByteArray<'_>,
    assertion_object: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_finalize_ios_app_attest_authorization_v2(
        &mut env,
        preparation,
        assertion_object,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeFinalizeTopUpV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeFinalizeTopUpV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    unsigned: jni::objects::JByteArray<'_>,
    authorization: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_finalize_top_up_v4(&mut env, unsigned, authorization)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeFinalizeRedeemV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeFinalizeRedeemV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    build_result: jni::objects::JByteArray<'_>,
    authorization: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_finalize_redeem_v4(&mut env, build_result, authorization)
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativePrepareTopUpV4();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativePrepareTopUpV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    network_id: jni::objects::JByteArray<'_>,
    chain_discriminant: jni::sys::jint,
    asset_definition: jni::objects::JByteArray<'_>,
    payer: jni::objects::JByteArray<'_>,
    atomic_units: jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
    operation_id: jni::objects::JByteArray<'_>,
    spend_key: jni::objects::JByteArray<'_>,
    rho: jni::objects::JByteArray<'_>,
    diversifier: jni::objects::JByteArray<'_>,
    leaf_index: jni::sys::jint,
    flattened_siblings: jni::objects::JByteArray<'_>,
    directions: jni::objects::JByteArray<'_>,
    root: jni::objects::JByteArray<'_>,
    shield_verifier_commitment: jni::objects::JByteArray<'_>,
    artifact_binding: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_prepare_top_up_v4(
        &mut env,
        network_id,
        chain_discriminant,
        asset_definition,
        payer,
        atomic_units,
        scale,
        operation_id,
        spend_key,
        rho,
        diversifier,
        leaf_index,
        flattened_siblings,
        directions,
        root,
        shield_verifier_commitment,
        artifact_binding,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectOperationStatusV4();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectOperationStatusV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    status: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_operation_status_v4(&mut env, status)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeBranchClaimsConflictV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeBranchClaimsConflictV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    left: jni::objects::JByteArray<'_>,
    right: jni::objects::JByteArray<'_>,
) -> jni::sys::jboolean {
    java_native_kagemusha_branch_claims_conflict_v2(&mut env, left, right)
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativePrepareRedemptionChangeV4();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativePrepareRedemptionChangeV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    bundle: jni::objects::JByteArray<'_>,
    input_opening: jni::objects::JByteArray<'_>,
    atomic_units: jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
    operation_id: jni::objects::JByteArray<'_>,
    entropy: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_prepare_redemption_change_v4(
        &mut env,
        bundle,
        input_opening,
        atomic_units,
        scale,
        operation_id,
        entropy,
    )
}
android:
#[allow(clippy::too_many_arguments)]
fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativePreparePeerSplitChangeV4();
sdk:
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativePreparePeerSplitChangeV4(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    bundles: jni::objects::JObjectArray<'_>,
    input_openings: jni::objects::JObjectArray<'_>,
    recipient_request: jni::objects::JByteArray<'_>,
    atomic_units: jni::objects::JByteArray<'_>,
    scale: jni::sys::jint,
    operation_id: jni::objects::JByteArray<'_>,
    entropy: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_prepare_peer_split_change_v4(
        &mut env,
        bundles,
        input_openings,
        recipient_request,
        atomic_units,
        scale,
        operation_id,
        entropy,
    )
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativePrepareNoteOpeningV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativePrepareNoteOpeningV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    spend_key: jni::objects::JByteArray<'_>,
    rho: jni::objects::JByteArray<'_>,
    diversifier: jni::objects::JByteArray<'_>,
) -> jni::sys::jbyteArray {
    java_native_kagemusha_prepare_note_opening_v2(&mut env, spend_key, rho, diversifier)
}
android: fn Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_nativeProjectRecipientRequestV2();
sdk:
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_nativeProjectRecipientRequestV2(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    request: jni::objects::JByteArray<'_>,
) -> jni::sys::jobjectArray {
    java_native_kagemusha_project_recipient_request_v2(&mut env, request)
}
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
