//! Genuine metadata signature/genesis tests; fixture relation bytes grant no source graph.

use super::*;
use iroha_crypto::KeyPair;
use iroha_data_model::{
    nexus::AxtAssetIncarnationV1, sumeragi_finality::test_fixtures::NativeFinalityFixture,
};
use p256::ecdsa::{Signature as G1Signature, SigningKey, signature::Signer as _};

fn canonical(value: &Value) -> Vec<u8> {
    let mut bytes = norito::json::to_json_bounded(value, APP_MANIFEST_MAX - 1)
        .unwrap()
        .into_bytes();
    bytes.push(b'\n');
    bytes
}
fn mutate(value: &mut Value, key: &str, replacement: Value) {
    let Value::Object(map) = value else {
        panic!("fixture object")
    };
    map.insert(key.into(), replacement);
}
fn application() -> Value {
    let mut app: Map = APP_FIELDS
        .iter()
        .map(|name| ((*name).into(), Value::Null))
        .collect();
    app.insert(
        "schema".into(),
        Value::String("bpng.taira-app-runtime-manifest.v6".into()),
    );
    app.insert("environment".into(), Value::String("taira-testnet".into()));
    app.insert(
        "firstDeviceAuthentication".into(),
        first_device_authentication_selection(),
    );
    Value::Object(app)
}
fn first_device_authentication_selection() -> Value {
    // Public metadata fixtures confer no authenticated-original or wallet authority.
    norito::json::json!({
        "schema":"bpng.first-device-auth-runtime-selection.v1",
        "googleOAuthClientId":"fixture-client.apps.googleusercontent.com",
        "googleOAuthIssuer":"https://accounts.google.com",
        "integrityCloudProjectNumber":123456789,
        "originalAuthPolicySha256":"11".repeat(32),
        "verifierConfigurationSha256":"22".repeat(32),
        "googlePolicySha256":"33".repeat(32)
    })
}
fn sign(key: &KeyPair, app: &[u8]) -> Vec<u8> {
    let mut message = DOMAIN.to_vec();
    message.extend_from_slice(app);
    let signature = Signature::try_new(key.private_key(), &message).unwrap();
    let (_, public) = key.public_key().to_bytes();
    canonical(
        &norito::json::json!({"schema":"bpng.taira-app-runtime-manifest-signature.v6","algorithm":"ed25519",
        "domain":"bpng:taira-app-runtime-manifest:v6","keyId":format!("sha256:{}",hex::encode(BlobV1::of(public).sha256)),
        "manifestSha256":hex::encode(BlobV1::of(app).sha256),"signatureBase64Url":URL_SAFE_NO_PAD.encode(signature.payload())}),
    )
}
#[test]
fn only_the_independently_selected_trust_key_accepts_exact_signed_app_bytes() {
    let key = KeyPair::from_seed(vec![11; 32], Algorithm::Ed25519);
    let other = KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519);
    let trust = RuntimeTrust::test(key.public_key().clone());
    let app = canonical(&application());
    let envelope = sign(&key, &app);
    signed_app(&trust, &app, &envelope).unwrap();
    assert!(
        signed_app(
            &RuntimeTrust::test(other.public_key().clone()),
            &app,
            &envelope
        )
        .is_err()
    );
    assert!(signed_app(&trust, &app, &sign(&other, &app)).is_err());
    let mut changed = app.clone();
    changed[3] ^= 1;
    assert!(signed_app(&trust, &changed, &envelope).is_err());
    let mut changed = envelope.clone();
    let last = changed.len() - 3;
    changed[last] ^= 1;
    assert!(signed_app(&trust, &app, &changed).is_err());
}
#[test]
fn even_genuinely_signed_unknown_fields_and_noncanonical_originals_refuse() {
    let key = KeyPair::from_seed(vec![13; 32], Algorithm::Ed25519);
    let trust = RuntimeTrust::test(key.public_key().clone());
    let mut app = application();
    mutate(
        &mut app,
        "offeredTrustKey",
        Value::String("untrusted".into()),
    );
    let bytes = canonical(&app);
    assert!(signed_app(&trust, &bytes, &sign(&key, &bytes)).is_err());
    let bytes = canonical(&application());
    let mut spaced = bytes.clone();
    spaced.insert(1, b' ');
    assert!(signed_app(&trust, &spaced, &sign(&key, &spaced)).is_err());
    let mut envelope = sign(&key, &bytes);
    envelope.insert(1, b' ');
    assert!(signed_app(&trust, &bytes, &envelope).is_err());
}
fn accepts_signed_authentication_selection(selection: Value) -> bool {
    let key = KeyPair::from_seed(vec![15; 32], Algorithm::Ed25519);
    let trust = RuntimeTrust::test(key.public_key().clone());
    let mut app = application();
    mutate(&mut app, "firstDeviceAuthentication", selection);
    let bytes = canonical(&app);
    signed_app(&trust, &bytes, &sign(&key, &bytes)).is_ok()
}
#[test]
fn signed_authentication_selection_requires_exact_current_fields_and_types() {
    let selection = first_device_authentication_selection();
    assert!(accepts_signed_authentication_selection(selection.clone()));
    let Value::Object(map) = &selection else {
        panic!("fixture object")
    };
    for field in map.keys() {
        let mut changed = map.clone();
        changed.remove(field);
        assert!(!accepts_signed_authentication_selection(Value::Object(
            changed
        )));
    }
    let mut changed = selection.clone();
    mutate(&mut changed, "authorized", Value::Bool(true));
    assert!(!accepts_signed_authentication_selection(changed));
    for invalid in [
        Value::Null,
        Value::Bool(true),
        Value::String("selection".into()),
        Value::Array(vec![]),
    ] {
        assert!(!accepts_signed_authentication_selection(invalid));
    }
    for invalid in [
        Value::String("bpng.first-device-auth-runtime-selection.v2".into()),
        Value::from(1_u64),
        Value::Null,
    ] {
        let mut changed = selection.clone();
        mutate(&mut changed, "schema", invalid);
        assert!(!accepts_signed_authentication_selection(changed));
    }
    let key = KeyPair::from_seed(vec![15; 32], Algorithm::Ed25519);
    let Value::Object(mut missing) = application() else {
        panic!("fixture object")
    };
    missing.remove("firstDeviceAuthentication");
    let bytes = canonical(&Value::Object(missing));
    assert!(
        signed_app(
            &RuntimeTrust::test(key.public_key().clone()),
            &bytes,
            &sign(&key, &bytes)
        )
        .is_err()
    );
}
#[test]
fn signed_authentication_oauth_strings_enforce_exact_issuer_and_ascii_client_bounds() {
    for (field, valid) in [
        ("googleOAuthClientId", "!".into()),
        ("googleOAuthClientId", "~".repeat(1024)),
        ("googleOAuthIssuer", "accounts.google.com".into()),
        ("googleOAuthIssuer", "https://accounts.google.com".into()),
    ] {
        let mut selection = first_device_authentication_selection();
        mutate(&mut selection, field, Value::String(valid));
        assert!(accepts_signed_authentication_selection(selection));
    }
    for (field, invalid) in [
        ("googleOAuthClientId", "".into()),
        ("googleOAuthClientId", "x".repeat(1025)),
        ("googleOAuthClientId", "client id".into()),
        ("googleOAuthClientId", "client\n".into()),
        ("googleOAuthClientId", "\u{7f}".into()),
        ("googleOAuthClientId", "é".into()),
        ("googleOAuthIssuer", "http://accounts.google.com".into()),
        ("googleOAuthIssuer", "https://accounts.google.com/".into()),
        (
            "googleOAuthIssuer",
            "https://accounts.google.com.evil".into(),
        ),
        ("googleOAuthIssuer", " accounts.google.com".into()),
    ] {
        let mut selection = first_device_authentication_selection();
        mutate(&mut selection, field, Value::String(invalid));
        assert!(!accepts_signed_authentication_selection(selection));
    }
    for field in ["googleOAuthClientId", "googleOAuthIssuer"] {
        for invalid in [
            Value::from(123_u64),
            Value::Bool(true),
            Value::Null,
            Value::Array(vec![]),
        ] {
            let mut selection = first_device_authentication_selection();
            mutate(&mut selection, field, invalid);
            assert!(!accepts_signed_authentication_selection(selection));
        }
    }
}
#[test]
fn signed_authentication_project_requires_a_positive_safe_json_integer() {
    for valid in [1_u64, 9_007_199_254_740_991] {
        let mut selection = first_device_authentication_selection();
        mutate(
            &mut selection,
            "integrityCloudProjectNumber",
            Value::from(valid),
        );
        assert!(accepts_signed_authentication_selection(selection));
    }
    for invalid in [
        Value::from(-1_i64),
        Value::from(0_u64),
        Value::from(1.5_f64),
        Value::from(9_007_199_254_740_992_u64),
        Value::String("1".into()),
        Value::Bool(true),
        Value::Null,
    ] {
        let mut selection = first_device_authentication_selection();
        mutate(&mut selection, "integrityCloudProjectNumber", invalid);
        assert!(!accepts_signed_authentication_selection(selection));
    }
}
#[test]
fn signed_authentication_original_pins_require_typed_nonzero_lowercase_digests() {
    for field in [
        "originalAuthPolicySha256",
        "verifierConfigurationSha256",
        "googlePolicySha256",
    ] {
        for invalid in [
            Value::String("0".repeat(64)),
            Value::String("A".repeat(64)),
            Value::String("g".repeat(64)),
            Value::String("a".repeat(63)),
            Value::String("a".repeat(65)),
            Value::from(123_u64),
            Value::Bool(false),
            Value::Null,
        ] {
            let mut selection = first_device_authentication_selection();
            mutate(&mut selection, field, invalid);
            assert!(!accepts_signed_authentication_selection(selection));
        }
    }
}
#[test]
fn lexical_boundaries_reject_duplicates_fractions_unsafe_integers_and_depth_before_owned_decode() {
    for bytes in [
        b"{\"a\":1,\"a\":2}".as_slice(),
        b"{\"a\":0.5}",
        b"{\"a\":9007199254740992}",
        b"{\"\\u00e9\":1}",
    ] {
        assert!(json(bytes, 1024, false).is_err());
    }
    assert!(json(b"", 1024, false).is_err());
    assert!(json(&[b' '; 1025], 1024, false).is_err());
    let nested = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    assert!(json(nested.as_bytes(), 1024, false).is_err());
    assert!(json(b"{\"a\":9007199254740991}", 1024, false).is_ok());
}
#[test]
fn byte_originals_have_canonical_base64_and_nonzero_lowercase_digest_forms() {
    let map = exact(&norito::json::json!({"bytes":"AQI="}), &["bytes"])
        .unwrap()
        .clone();
    assert_eq!(raw(&map, "bytes", 2).unwrap(), vec![1, 2]);
    assert!(raw(&map, "bytes", 1).is_err());
    let map = exact(&norito::json::json!({"bytes":"AQI"}), &["bytes"])
        .unwrap()
        .clone();
    assert!(raw(&map, "bytes", 2).is_err());
    for hash in [
        "0".repeat(64),
        "A".repeat(64),
        "g".repeat(64),
        "1".repeat(63),
    ] {
        assert!(sha(&hash).is_err());
    }
    assert_eq!(sha(&"1".repeat(64)).unwrap(), [0x11; 32]);
    assert!(exact(&norito::json::json!({"a":1,"b":2}), &["a"]).is_err());
}
fn g1_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes((&[seed; 32]).into()).unwrap()
}
fn g1_public(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap()
}
fn g1_sign(key: &SigningKey, bytes: &[u8]) -> KagemushaWalletSignerOutputV1<'static> {
    let signature: G1Signature = key.sign(bytes);
    KagemushaWalletSignerOutputV1::Raw(signature.to_bytes().into())
}
struct BaseFixture {
    key: KeyPair,
    app: Value,
    runtime: Value,
    genesis: Vec<u8>,
    scheme: KagemushaWalletSchemeV1,
}
impl BaseFixture {
    fn new() -> Self {
        let native = NativeFinalityFixture::start("installed-runtime-metadata-fixture");
        let genesis = native.genesis().encode_wire().unwrap();
        let epoch = genesis_epoch(native.genesis()).unwrap();
        let root = g1_key(7);
        let artifact = g1_key(8);
        let enrollment = g1_key(9);
        // These metadata-only protocol bytes deliberately have no proof keys. Real
        // InstalledRuntime still requires strict qualification of the complete graph.
        let mut body = KagemushaWalletArtifactManifestBodyV1 {
            version: 1,
            network_id: *native.network_id().as_bytes(),
            relation_id: [1; 32],
            eq_protocol_digest: [2; 32],
            ep_protocol_digest: [3; 32],
            native_profile_digest: [4; 32],
            verifying_key_set_digest: [5; 32],
            artifact_inventory_digest: [6; 32],
            provider_contract: kagemusha_wallet_provider_contract_v1(),
            signer_certificate: [1; 32],
        };
        body.relation_id = body.recomputed_relation_id();
        let scheme = KagemushaWalletSchemeV1 {
            version: 1,
            network_id: body.network_id,
            scheme_root_key: g1_public(&root),
            relation_id: body.relation_id,
            provider_contract: body.provider_contract,
        };
        let certificate = |role, signer: &SigningKey| {
            let body = KagemushaWalletSignerCertificateBodyV1 {
                version: 1,
                scheme_id: scheme.scheme_id(),
                role,
                key: g1_public(signer),
                serial: 1,
            };
            KagemushaWalletSignerCertificateV1::sign(
                body,
                &scheme,
                g1_sign(&root, &body.signing_message()),
            )
            .unwrap()
        };
        let artifact_certificate = certificate(KagemushaWalletSignerRoleV1::Artifact, &artifact);
        let enrollment_certificate =
            certificate(KagemushaWalletSignerRoleV1::Enrollment, &enrollment);
        body.signer_certificate = artifact_certificate.certificate_digest();
        let manifest = KagemushaWalletArtifactManifestV1::sign(
            body,
            &artifact_certificate,
            g1_sign(&artifact, &body.signing_message()),
        )
        .unwrap();
        let asset = KagemushaWalletAssetScopeV1::new(
            AssetDefinitionId::from_uuid_bytes([
                0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84, 0xfd,
                0xcd, 0x2f,
            ])
            .unwrap(),
            &AxtAssetIncarnationV1::try_from_bytes([2; 32]).unwrap(),
            2,
        )
        .unwrap();
        let runtime = norito::json::json!({"schema":"bpng.current-wallet-core-runtime.v1","version":1,"scheme_id":hex::encode(scheme.scheme_id()),
            "scheme_original_base64":STANDARD.encode(scheme.to_canonical_bytes().unwrap()),"asset_original_base64":STANDARD.encode(norito::encode_canonical(&asset).unwrap()),
            "enrollment_certificate_original_base64":STANDARD.encode(enrollment_certificate.to_canonical_bytes().unwrap()),
            "artifact_signer_certificate_original_base64":STANDARD.encode(artifact_certificate.to_canonical_bytes().unwrap()),
            "artifact_manifest_original_base64":STANDARD.encode(manifest.to_canonical_bytes().unwrap()),
            "regulatory_policy_original_base64":STANDARD.encode(norito::encode_canonical(&KagemushaWalletRegulatoryPolicyV1::default()).unwrap()),
            "challenge_lifetime_ms":120000,"python_path":"/usr/bin/python3","python_sha256":"11".repeat(32),"openssl_path":"/usr/bin/openssl","openssl_sha256":"22".repeat(32),
            "android":{"app_policy_hex":"33".repeat(32),"enrollment_policy_hex":"44".repeat(32),"verifier_configuration_path":"srv/etc/kagemusha/wallet-e1-android.json"},
            "apple":{"app_policy_hex":"55".repeat(32),"enrollment_policy_hex":"66".repeat(32),"verifier_configuration_path":"srv/etc/kagemusha/wallet-e1-apple.json"}});
        let mut app = application();
        mutate(
            &mut app,
            "ledger",
            norito::json::json!({"toriiUrl":"https://test.invalid","networkId":native.network_id().to_string(),"networkPrefix":42,
            "chainId":native.chain_id(),"irohaSourceCommit":"77".repeat(20),"irohaBuildSha256":"88".repeat(32)}),
        );
        let validators:Vec<_>=epoch.committee.iter().map(|member|norito::json::json!({"peerId":member.validator.public_key().to_string(),
            "directToriiUrl":"https://test.invalid","nodeFingerprint":"11".repeat(32),"buildFingerprint":"22".repeat(32),"configFingerprint":"33".repeat(32)})).collect();
        let genesis_key = native
            .genesis()
            .external_transactions()
            .next()
            .unwrap()
            .authority()
            .try_signatory()
            .unwrap()
            .to_string();
        mutate(
            &mut app,
            "consensus",
            norito::json::json!({"mode":"iroha3-consensus::permissioned-sumeragi@v1","protocolVersion":1,
            "networkId":native.network_id().to_string(),"genesisBlockHash":hex::encode(native.genesis().hash().as_ref()),
            "signedGenesisSha256":hex::encode(BlobV1::of(&genesis).sha256),"genesisPublicKey":genesis_key,"finalityVerifierSha256":"99".repeat(32),
            "validators":validators,"checkpointSha256":null,"checkpointHeight":null,"checkpointContextId":null}),
        );
        mutate(
            &mut app,
            "digitalKina",
            norito::json::json!({"assetAlias":"kina","assetDefinitionId":asset.asset.to_string(),"scale":2,"owningDomain":"test",
            "physicalLaneId":1,"physicalLaneAlias":"lane","physicalDataspaceId":1,"physicalDataspaceAlias":"space","registrationTransactionHash":"aa".repeat(32)}),
        );
        Self {
            key: KeyPair::from_seed(vec![14; 32], Algorithm::Ed25519),
            app,
            runtime,
            genesis,
            scheme,
        }
    }
    fn originals(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let runtime = norito::json::to_json_bounded(&self.runtime, WALLET_RUNTIME_MAX)
            .unwrap()
            .into_bytes();
        let mut app = self.app.clone();
        mutate(
            &mut app,
            "walletRuntime",
            norito::json::json!({"schema":"bpng.current-wallet-runtime-pin.v1","currentRuntimeSha256":hex::encode(BlobV1::of(&runtime).sha256)}),
        );
        let app = canonical(&app);
        let envelope = sign(&self.key, &app);
        (app, envelope, runtime)
    }
    fn load(&self) -> Result<Selection> {
        let (app, envelope, runtime) = self.originals();
        Selection::load(
            &RuntimeTrust::test(self.key.public_key().clone()),
            &RuntimeOriginals {
                app_manifest: &app,
                envelope: &envelope,
                wallet_runtime: &runtime,
                verifier_pack: b"",
                producer_inventory: b"",
                signed_genesis: &self.genesis,
                originals_root: b"",
            },
        )
    }
}
#[test]
fn genuine_signed_genesis_and_delegated_monetary_manifest_derive_installation_internally() {
    let fixture = BaseFixture::new();
    let selection = fixture.load().unwrap();
    assert_eq!(selection.installation.scheme_id, fixture.scheme.scheme_id());
    assert_eq!(
        *selection.genesis.network_id().as_bytes(),
        fixture.scheme.network_id
    );
    assert_eq!(
        selection._originals._signed_genesis.as_ref(),
        fixture.genesis
    );
    let (app, envelope, runtime) = fixture.originals();
    assert_eq!(selection._originals._app_manifest.as_ref(), app);
    assert_eq!(selection._originals._envelope.as_ref(), envelope);
    assert_eq!(selection._originals._wallet_runtime.as_ref(), runtime);
}
#[test]
fn exact_whole_runtime_and_signed_genesis_preimages_cannot_be_retargeted() {
    let mut fixture = BaseFixture::new();
    fixture.genesis[0] ^= 1;
    assert!(fixture.load().is_err());
    let fixture = BaseFixture::new();
    let (app, envelope, mut runtime) = fixture.originals();
    runtime.push(b' ');
    assert!(
        Selection::load(
            &RuntimeTrust::test(fixture.key.public_key().clone()),
            &RuntimeOriginals {
                app_manifest: &app,
                envelope: &envelope,
                wallet_runtime: &runtime,
                verifier_pack: b"",
                producer_inventory: b"",
                signed_genesis: &fixture.genesis,
                originals_root: b""
            }
        )
        .is_err()
    );
}
#[test]
fn genuinely_resigned_application_still_cannot_change_native_genesis_policy_or_asset() {
    let mut fixture = BaseFixture::new();
    let Value::Object(app) = &mut fixture.app else {
        panic!("app")
    };
    let consensus = app.get_mut("consensus").unwrap();
    mutate(
        consensus,
        "mode",
        Value::String("iroha3-consensus::npos-sumeragi@v1".into()),
    );
    assert!(fixture.load().is_err());
    let mut fixture = BaseFixture::new();
    let Value::Object(app) = &mut fixture.app else {
        panic!("app")
    };
    mutate(
        app.get_mut("digitalKina").unwrap(),
        "scale",
        Value::from(3u64),
    );
    assert!(fixture.load().is_err());
}
#[test]
fn issuer_tool_metadata_is_bounded_data_and_never_an_executable_input() {
    let mut fixture = BaseFixture::new();
    mutate(
        &mut fixture.runtime,
        "challenge_lifetime_ms",
        Value::from(0u64),
    );
    assert!(fixture.load().is_err());
    mutate(
        &mut fixture.runtime,
        "challenge_lifetime_ms",
        Value::from(600001u64),
    );
    assert!(fixture.load().is_err());
    mutate(
        &mut fixture.runtime,
        "challenge_lifetime_ms",
        Value::from(120000u64),
    );
    mutate(
        &mut fixture.runtime,
        "python_path",
        Value::String("relative/tool".into()),
    );
    assert!(fixture.load().is_err());
}

#[test]
fn genuine_signed_base_cannot_omit_financial_originals_or_register_an_owner() {
    let fixture = BaseFixture::new();
    let selected = Arc::new(fixture.load().unwrap());
    let (app, envelope, runtime) = fixture.originals();
    let result = PreparedInstallation::from_selected(
        RuntimeOriginals {
            app_manifest: &app,
            envelope: &envelope,
            wallet_runtime: &runtime,
            verifier_pack: b"",
            producer_inventory: b"",
            signed_genesis: &fixture.genesis,
            originals_root: b"",
        },
        selected,
    );
    assert!(matches!(
        result,
        Err(Failure {
            status: INVALID,
            ..
        })
    ));
}

// These genuinely delegated credential signatures test signed scope only. Evidence facts
// are DATA fixtures, confer no platform qualification and never create a custody provider.
fn scoped_credential(
    selected: &Selection,
    kind: KagemushaWalletEvidenceKindV1,
    policy: [u8; 32],
    issuer_seed: u8,
) -> KagemushaWalletCredentialV1 {
    let root = g1_key(7);
    let issuer = g1_key(issuer_seed);
    let payment = g1_public(&g1_key(13));
    let issuer_body = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: selected.scheme.scheme_id(),
        role: KagemushaWalletSignerRoleV1::Enrollment,
        key: g1_public(&issuer),
        serial: 2,
    };
    let certificate = KagemushaWalletSignerCertificateV1::sign(
        issuer_body,
        &selected.scheme,
        g1_sign(&root, &issuer_body.signing_message()),
    )
    .unwrap();
    let challenge = KagemushaWalletEnrollmentChallengeV1 {
        version: 1,
        scheme_id: selected.scheme.scheme_id(),
        asset_digest: selected.asset.asset_digest(),
        account_digest: [11; 32],
        app_policy: policy,
        enrollment_policy: [12; 32],
        issuer_nonce: [4; 32],
    };
    let facts = if kind.is_android() {
        KAGEMUSHA_WALLET_ANDROID_REQUIRED_FACTS_V1
    } else {
        KAGEMUSHA_WALLET_APPLE_REQUIRED_FACTS_V1
    };
    let patch = if kind.is_android() { 202_609 } else { 0 };
    let evidence = KagemushaWalletEvidenceV1 {
        digest: [10; 32],
        time_ms: 1000,
        facts,
        os_patch_level: patch,
        vendor_patch_level: patch,
        boot_patch_level: patch,
    };
    let body = KagemushaWalletCredentialBodyV1 {
        version: 1,
        scheme_id: challenge.scheme_id,
        asset_digest: challenge.asset_digest,
        wallet_id: challenge.wallet_id(&payment),
        account_digest: challenge.account_digest,
        payment_key: payment,
        provider_contract: selected.scheme.provider_contract,
        evidence_kind: kind,
        enrollment_evidence: evidence,
        fresh_evidence: evidence,
        app_policy: policy,
        regulatory_policy: KagemushaWalletRegulatoryPolicyV1::default(),
        enrollment_id: challenge.enrollment_id(&payment),
        issued_at_ms: 2000,
        renewal_sequence: 0,
        lease_expires_at_ms: 0,
        issuer_certificate: certificate.certificate_digest(),
    };
    KagemushaWalletCredentialV1::sign(
        body,
        &certificate,
        g1_sign(&issuer, &body.signing_message()),
    )
    .unwrap()
}
#[test]
fn independently_signed_runtime_scope_cannot_be_retargeted_by_an_original_credential() {
    let fixture = BaseFixture::new();
    let selected = Arc::new(fixture.load().unwrap());
    let binding = BoundOriginals {
        selected: Arc::clone(&selected),
        android: true,
    };
    let credential = scoped_credential(
        &selected,
        KagemushaWalletEvidenceKindV1::AndroidKeyMintTee,
        selected.android_app_policy,
        9,
    )
    .to_canonical_bytes()
    .unwrap();
    binding
        .require([
            &credential,
            b"certificates",
            b"account",
            &selected.asset_original,
        ])
        .unwrap();
    let wrong = scoped_credential(
        &selected,
        KagemushaWalletEvidenceKindV1::AndroidKeyMintTee,
        selected.apple_app_policy,
        9,
    )
    .to_canonical_bytes()
    .unwrap();
    assert!(
        binding
            .require([
                &wrong,
                b"certificates",
                b"account",
                &selected.asset_original
            ])
            .is_err()
    );
    let apple = scoped_credential(
        &selected,
        KagemushaWalletEvidenceKindV1::AppleAppAttest,
        selected.android_app_policy,
        9,
    )
    .to_canonical_bytes()
    .unwrap();
    assert!(
        binding
            .require([
                &apple,
                b"certificates",
                b"account",
                &selected.asset_original
            ])
            .is_err()
    );
    let mut changed = selected.asset_original.clone();
    changed.push(0);
    assert!(
        binding
            .require([&credential, b"certificates", b"account", &changed])
            .is_err()
    );
}
#[test]
fn renewed_issuer_original_is_left_to_genuine_intake_and_never_replaced_by_current_runtime_certificate()
 {
    let fixture = BaseFixture::new();
    let selected = Arc::new(fixture.load().unwrap());
    let renewed = scoped_credential(
        &selected,
        KagemushaWalletEvidenceKindV1::AndroidKeyMintTee,
        selected.android_app_policy,
        11,
    );
    assert_ne!(
        renewed.body.issuer_certificate,
        selected.enrollment_certificate.certificate_digest()
    );
    let original = renewed.to_canonical_bytes().unwrap();
    let binding = BoundOriginals {
        selected: Arc::clone(&selected),
        android: true,
    };
    binding
        .require([
            &original,
            b"certificates",
            b"account",
            &selected.asset_original,
        ])
        .unwrap();
    // This is scope admission only. Native original intake still requires its exact
    // actual issuer certificate, retained credential and positive enrolled key.
}
