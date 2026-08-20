fn ios_assertion_extension_bytes(bundle_version: &str, validation_category: u32) -> Vec<u8> {
    let value = ciborium::value::Value::Map(vec![
        (
            ciborium::value::Value::Text("bundleVersion".to_owned()),
            ciborium::value::Value::Text(bundle_version.to_owned()),
        ),
        (
            ciborium::value::Value::Text("validationCategory".to_owned()),
            ciborium::value::Value::Integer(validation_category.into()),
        ),
    ]);
    let mut encoded = Vec::new();
    ciborium::ser::into_writer(&value, &mut encoded)
        .expect("encode App Attest assertion extensions");
    encoded
}
fn ios_assertion_auth_data(
    rp_id_hash: [u8; 32],
    flags: u8,
    sign_count: u32,
    extension_bytes: &[u8],
) -> Vec<u8> {
    let mut auth_data = Vec::with_capacity(
        KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_FIXED_HEADER_BYTES_V1 + extension_bytes.len(),
    );
    auth_data.extend_from_slice(&rp_id_hash);
    auth_data.push(flags);
    auth_data.extend_from_slice(&sign_count.to_be_bytes());
    auth_data.extend_from_slice(extension_bytes);
    auth_data
}
fn ios_assertion_policy() -> OfflineIosAppAttestationPolicy {
    OfflineIosAppAttestationPolicy {
        team_id: "TEAMID1234".to_owned(),
        bundle_id: "io.soramitsu.pk".to_owned(),
        environment: "production".to_owned(),
        allowed_validation_categories: vec![4],
        allowed_bundle_versions: vec!["42".to_owned()],
    }
}
#[test]
fn ios_assertion_auth_data_enforces_exact_extensions_and_policy() {
    let rp_id_hash = [0xA5; 32];
    let extension_bytes = ios_assertion_extension_bytes("42", 4);
    let encoded = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        1,
        &extension_bytes,
    );
    let parsed = parse_ios_app_attest_assertion_auth_data(&encoded)
        .expect("extension-bearing assertion authData");
    assert_eq!(parsed.rp_id_hash, rp_id_hash);
    assert_eq!(parsed.sign_count, 1);
    validate_ios_app_attest_extensions_against_policy(&ios_assertion_policy(), &parsed.extensions)
        .expect("the exact governed category and bundle version are accepted");
    let reverse_order = ciborium::value::Value::Map(vec![
        (
            ciborium::value::Value::Text("validationCategory".to_owned()),
            ciborium::value::Value::Integer(4_u32.into()),
        ),
        (
            ciborium::value::Value::Text("bundleVersion".to_owned()),
            ciborium::value::Value::Text("42".to_owned()),
        ),
    ]);
    let mut reverse_order_bytes = Vec::new();
    ciborium::ser::into_writer(&reverse_order, &mut reverse_order_bytes)
        .expect("encode reverse-order Apple extension map");
    let reverse_order_auth_data = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        2,
        &reverse_order_bytes,
    );
    parse_ios_app_attest_assertion_auth_data(&reverse_order_auth_data)
        .expect("Apple does not require one map-key order");
    let mut nonminimal_definite = vec![0xB8, 0x02];
    nonminimal_definite.extend_from_slice(&extension_bytes[1..]);
    let nonminimal_definite_auth_data = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        3,
        &nonminimal_definite,
    );
    parse_ios_app_attest_assertion_auth_data(&nonminimal_definite_auth_data)
        .expect("valid definite Apple CBOR is accepted without serializer byte equality");
    let wrong_category = ios_assertion_extension_bytes("42", 5);
    let wrong_category = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        2,
        &wrong_category,
    );
    let parsed = parse_ios_app_attest_assertion_auth_data(&wrong_category)
        .expect("well-formed but unlisted extension values");
    assert!(
        validate_ios_app_attest_extensions_against_policy(
            &ios_assertion_policy(),
            &parsed.extensions,
        )
        .is_err(),
        "an unlisted validation category must fail closed",
    );
    let wrong_version = ios_assertion_extension_bytes("43", 4);
    let wrong_version = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        3,
        &wrong_version,
    );
    let parsed = parse_ios_app_attest_assertion_auth_data(&wrong_version)
        .expect("well-formed but unlisted bundle version");
    assert!(
        validate_ios_app_attest_extensions_against_policy(
            &ios_assertion_policy(),
            &parsed.extensions,
        )
        .is_err(),
        "an unlisted bundle version must fail closed",
    );
}
#[test]
fn ios_assertion_auth_data_rejects_bad_flags_trailing_and_unknown_extensions() {
    let rp_id_hash = [0xB6; 32];
    let extension_bytes = ios_assertion_extension_bytes("42", 4);
    for flags in [
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_USER_PRESENT,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_ATTESTED_CREDENTIAL_DATA,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_USER_VERIFIED,
    ] {
        let auth_data = ios_assertion_auth_data(rp_id_hash, flags, 1, &[]);
        assert!(
            parse_ios_app_attest_assertion_auth_data(&auth_data).is_err(),
            "App Attest assertion flags other than ED must fail closed",
        );
    }
    let missing_extensions = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        1,
        &[],
    );
    assert!(parse_ios_app_attest_assertion_auth_data(&missing_extensions).is_err());
    let mut indefinite_extensions = vec![0xBF];
    indefinite_extensions.extend_from_slice(&extension_bytes[1..]);
    indefinite_extensions.push(0xFF);
    let indefinite = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        1,
        &indefinite_extensions,
    );
    assert!(parse_ios_app_attest_assertion_auth_data(&indefinite).is_err());
    let extensions_without_ed = ios_assertion_auth_data(rp_id_hash, 0, 1, &extension_bytes);
    assert!(parse_ios_app_attest_assertion_auth_data(&extensions_without_ed).is_err());
    let mut trailing_extensions = extension_bytes.clone();
    trailing_extensions.push(0xF6);
    let trailing = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        1,
        &trailing_extensions,
    );
    assert!(parse_ios_app_attest_assertion_auth_data(&trailing).is_err());
    let unknown = ciborium::value::Value::Map(vec![
        (
            ciborium::value::Value::Text("bundleVersion".to_owned()),
            ciborium::value::Value::Text("42".to_owned()),
        ),
        (
            ciborium::value::Value::Text("unknown".to_owned()),
            ciborium::value::Value::Integer(7_u32.into()),
        ),
    ]);
    let mut unknown_extensions = Vec::new();
    ciborium::ser::into_writer(&unknown, &mut unknown_extensions)
        .expect("encode unknown extension fixture");
    let unknown = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        1,
        &unknown_extensions,
    );
    assert!(parse_ios_app_attest_assertion_auth_data(&unknown).is_err());
    let apple_attestation_keys = ciborium::value::Value::Map(vec![
        (
            ciborium::value::Value::Text("apple_bundle_version_01".to_owned()),
            ciborium::value::Value::Text("42".to_owned()),
        ),
        (
            ciborium::value::Value::Text("apple_validation_category_01".to_owned()),
            ciborium::value::Value::Integer(4_u32.into()),
        ),
    ]);
    let mut apple_attestation_extensions = Vec::new();
    ciborium::ser::into_writer(&apple_attestation_keys, &mut apple_attestation_extensions)
        .expect("encode attestation-only extension fixture");
    let wrong_wire_keys = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        1,
        &apple_attestation_extensions,
    );
    assert!(
        parse_ios_app_attest_assertion_auth_data(&wrong_wire_keys).is_err(),
        "attestation apple_*_01 keys must not be accepted on assertion authData",
    );
    assert!(
        decode_ios_app_attest_attestation_extensions(&extension_bytes).is_err(),
        "assertion validationCategory/bundleVersion keys must not be accepted in attestation authData",
    );
}
#[test]
fn ios_assertion_extensions_and_counter_rules_are_mandatory_and_strict() {
    let rp_id_hash = [0xC7; 32];
    let without_extensions = ios_assertion_auth_data(rp_id_hash, 0, 9, &[]);
    assert!(
        parse_ios_app_attest_assertion_auth_data(&without_extensions).is_err(),
        "extension-free assertion authData must fail closed",
    );
    let extension_bytes = ios_assertion_extension_bytes("42", 4);
    let encoded = ios_assertion_auth_data(
        rp_id_hash,
        OFFLINE_ATTESTATION_APP_ATTEST_FLAG_EXTENSION_DATA,
        9,
        &extension_bytes,
    );
    let parsed = parse_ios_app_attest_assertion_auth_data(&encoded)
        .expect("extension-bearing assertion authData is structurally valid");
    validate_ios_app_attest_extensions_against_policy(&ios_assertion_policy(), &parsed.extensions)
        .expect("the required assertion extensions satisfy the pinned policy");
    validate_ios_app_attest_assertion_binding(&parsed, rp_id_hash, 8)
        .expect("a strictly increasing counter is accepted");
    for (sign_count, last_sign_count) in [(0, 0), (8, 8), (7, 8)] {
        let candidate = IosAppAttestAssertionAuthData {
            rp_id_hash,
            sign_count,
            extensions: parsed.extensions.clone(),
        };
        assert!(
            validate_ios_app_attest_assertion_binding(&candidate, rp_id_hash, last_sign_count,)
                .is_err(),
            "zero, equal, and decreasing counters must fail closed",
        );
    }
    assert!(
        validate_ios_app_attest_assertion_binding(&parsed, [0xD8; 32], 8).is_err(),
        "the RP/application hash must match exactly",
    );
}
#[test]
fn ios_policy_rejects_reserved_or_inappropriate_validation_categories() {
    let mut policy = default_offline_device_attestation_policy()
        .expect("built-in roots form a valid test policy");
    policy.require_ios_app_policy = true;
    policy.ios_apps = vec![ios_assertion_policy()];
    validate_offline_attestation_policy(&policy, 0).expect("documented category 4 is policy-valid");
    for category in [0, 7, 8, 9, 11] {
        policy.ios_apps[0].allowed_validation_categories = vec![category];
        assert!(
            validate_offline_attestation_policy(&policy, 0).is_err(),
            "validation category {category} must be rejected regardless of governance",
        );
    }
}
#[test]
fn ios_app_admission_requires_explicit_pinned_policy() {
    let mut policy = default_offline_device_attestation_policy()
        .expect("built-in roots form a valid test policy");
    let app = ios_assertion_policy();
    assert!(
        ensure_ios_app_allowed_by_policy(&policy, &app.team_id, &app.bundle_id, &app.environment,)
            .is_err(),
        "the consensus default must not admit an arbitrary iOS app",
    );
    policy.ios_apps = vec![app.clone()];
    assert!(
        ensure_ios_app_allowed_by_policy(&policy, &app.team_id, &app.bundle_id, &app.environment,)
            .is_err(),
        "a pinned iOS app must remain disabled until governance enables App Attest",
    );
    policy.require_ios_app_policy = true;
    ensure_ios_app_allowed_by_policy(&policy, &app.team_id, &app.bundle_id, &app.environment)
        .expect("the exact enabled iOS app identity is accepted");
    assert!(
        ensure_ios_app_allowed_by_policy(
            &policy,
            &app.team_id,
            "pk.retail.wallet.ios.substitute",
            &app.environment,
        )
        .is_err(),
        "a substituted iOS bundle must fail closed",
    );
}
#[test]
fn registration_lifetime_requires_one_continuously_active_platform_root() {
    let mut policy = default_offline_device_attestation_policy()
        .expect("built-in roots form a valid test policy");
    let mut android_roots: Vec<_> = policy
        .trusted_roots
        .iter_mut()
        .filter(|root| root.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT)
        .collect();
    assert!(
        android_roots.len() >= 2,
        "test policy needs two Android roots"
    );
    android_roots[0].not_after_ms = Some(POLICY_TEST_TIME_MS + 30_000);
    android_roots[1].not_before_ms = Some(POLICY_TEST_TIME_MS + 30_000);
    drop(android_roots);
    assert!(
        offline_attestation_policy_for_registration_lifetime(
            &policy,
            OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT,
            POLICY_TEST_TIME_MS,
            POLICY_TEST_TIME_MS + 60_000,
        )
        .is_err(),
        "different roots covering opposite endpoints must not be combined into a lifetime admission",
    );
    policy
        .trusted_roots
        .iter_mut()
        .find(|root| root.platform == OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT)
        .expect("Android test root")
        .not_after_ms = Some(POLICY_TEST_TIME_MS + 60_000);
    let lifetime = offline_attestation_policy_for_registration_lifetime(
        &policy,
        OFFLINE_ATTESTATION_PLATFORM_ANDROID_KEYMINT,
        POLICY_TEST_TIME_MS,
        POLICY_TEST_TIME_MS + 60_000,
    )
    .expect("one root covering both endpoints is sufficient");
    assert_eq!(lifetime.trusted_roots.len(), 1);
}
#[test]
fn android_app_admission_requires_explicit_pinned_policy() {
    let package_name = "com.pk.retailwallet";
    let signing_digest = [0xE9; 32];
    let mut policy = default_offline_device_attestation_policy()
        .expect("built-in roots form a valid test policy");
    assert!(
        ensure_android_app_allowed_by_policy(&policy, package_name, &signing_digest,).is_err(),
        "the consensus default must not admit arbitrary Android apps",
    );
    policy.android_apps = vec![OfflineAndroidAppAttestationPolicy {
        package_name: package_name.to_owned(),
        signing_certificate_sha256: vec![signing_digest.to_vec()],
    }];
    assert!(
        ensure_android_app_allowed_by_policy(&policy, package_name, &signing_digest,).is_err(),
        "a pinned app entry must remain disabled until governance enables Android",
    );
    policy.require_android_app_policy = true;
    ensure_android_app_allowed_by_policy(&policy, package_name, &signing_digest)
        .expect("the exact enabled package and signer are accepted");
    assert!(
        ensure_android_app_allowed_by_policy(
            &policy,
            "com.pk.retailwallet.substitute",
            &signing_digest,
        )
        .is_err(),
        "a substituted package must fail closed",
    );
    assert!(
        ensure_android_app_allowed_by_policy(&policy, package_name, &[0xEA; 32]).is_err(),
        "a substituted signing certificate must fail closed",
    );
}
