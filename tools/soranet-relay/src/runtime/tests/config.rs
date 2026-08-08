//! Runtime identity, certificate, handshake, and replay-store configuration tests.

use super::*;

#[test]
fn generates_self_signed_config() {
    let config = RelayRuntime::self_signed_server_config("relay.test");
    assert!(config.is_ok());
}

#[test]
fn relay_quic_server_rejects_tls_early_data() {
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["relay.test".to_owned()])
            .expect("generate test certificate");
    let key =
        PrivateKeyDer::try_from(signing_key.serialize_der()).expect("encode test private key");
    let tls = RelayRuntime::tls_server_config(vec![cert.der().clone()], key)
        .expect("build relay TLS configuration");

    assert_eq!(tls.max_early_data_size, 0);
}

#[test]
fn runtime_uses_fallback_identity_when_missing() {
    let json = r#"
        {
            "mode": "Entry",
            "listen": "127.0.0.1:0"
        }
    "#;
    let config = load_config(json);
    let runtime = RelayRuntime::new(config).expect("runtime");
    let context = runtime.circuit_context();

    let expected_private = PrivateKey::from_bytes(Algorithm::Ed25519, &FALLBACK_IDENTITY_SEED)
        .expect("fallback key to parse");
    let expected_pair =
        KeyPair::from_private_key(expected_private).expect("fallback keypair derive");

    assert_eq!(
        context.identity_key.public_key(),
        expected_pair.public_key()
    );
}

#[test]
fn runtime_loads_descriptor_commit_from_certificate() {
    let fixture = CertificateTestFixture::new();
    let json = format!(
        r#"{{
            "mode": "Entry",
            "listen": "127.0.0.1:0",
            "handshake": {{
                "identity_private_key_hex": "{identity_hex}",
                "descriptor_manifest_path": "{manifest}",
                "certificate": {{
                    "bundle_path": "{bundle}",
                    "issuer_ed25519_hex": "{issuer_ed}",
                    "issuer_mldsa_hex": "{issuer_mldsa}"
                }}
            }}
        }}"#,
        identity_hex = fixture.identity_seed_hex,
        manifest = fixture.manifest_file.path().display(),
        bundle = fixture.bundle_file.path().display(),
        issuer_ed = fixture.issuer_ed25519_hex,
        issuer_mldsa = fixture.issuer_mldsa_hex,
    );
    let config = load_config(&json);
    let runtime = RelayRuntime::new(config).expect("runtime");
    assert_eq!(runtime.descriptor_commit(), fixture.descriptor_commit);
    let stored_bundle = runtime
        .certificate_bundle()
        .expect("certificate bundle available");
    assert_eq!(
        stored_bundle.certificate.descriptor_commit,
        fixture.descriptor_commit
    );
}

#[test]
fn runtime_rejects_expired_certificate_at_startup() {
    let fixture = CertificateTestFixture::with_valid_until(2);
    let json = format!(
        r#"{{
            "mode": "Entry",
            "listen": "127.0.0.1:0",
            "handshake": {{
                "identity_private_key_hex": "{identity_hex}",
                "descriptor_manifest_path": "{manifest}",
                "certificate": {{
                    "bundle_path": "{bundle}",
                    "issuer_ed25519_hex": "{issuer_ed}",
                    "issuer_mldsa_hex": "{issuer_mldsa}"
                }}
            }}
        }}"#,
        identity_hex = fixture.identity_seed_hex,
        manifest = fixture.manifest_file.path().display(),
        bundle = fixture.bundle_file.path().display(),
        issuer_ed = fixture.issuer_ed25519_hex,
        issuer_mldsa = fixture.issuer_mldsa_hex,
    );
    let config = load_config(&json);
    let err = match RelayRuntime::new(config) {
        Ok(_) => panic!("expired certificate must fail at startup"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("expired"),
        "unexpected startup error: {err}"
    );
}

#[test]
fn resolve_handshake_suites_defaults_without_certificate() {
    let suites = resolve_handshake_suites(None).expect("suites");
    assert_eq!(
        suites,
        vec![
            HandshakeSuite::Nk2Hybrid,
            HandshakeSuite::Nk3PqForwardSecure
        ]
    );
}

#[test]
fn resolve_handshake_suites_uses_certificate_order() {
    let fixture = CertificateTestFixture::new();
    let suites = resolve_handshake_suites(Some(&fixture.bundle)).expect("suites");
    assert_eq!(suites, fixture.bundle.certificate.handshake_suites);
}

#[test]
fn runtime_rejects_descriptor_commit_mismatch() {
    let fixture = CertificateTestFixture::new();
    let mismatch_hex = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let json = format!(
        r#"{{
            "mode": "Entry",
            "listen": "127.0.0.1:0",
            "handshake": {{
                "identity_private_key_hex": "{identity_hex}",
                "descriptor_manifest_path": "{manifest}",
                "descriptor_commit_hex": "{mismatch}",
                "certificate": {{
                    "bundle_path": "{bundle}",
                    "issuer_ed25519_hex": "{issuer_ed}",
                    "issuer_mldsa_hex": "{issuer_mldsa}"
                }}
            }}
        }}"#,
        identity_hex = fixture.identity_seed_hex,
        manifest = fixture.manifest_file.path().display(),
        bundle = fixture.bundle_file.path().display(),
        issuer_ed = fixture.issuer_ed25519_hex,
        issuer_mldsa = fixture.issuer_mldsa_hex,
        mismatch = mismatch_hex,
    );
    let config = load_config(&json);
    match RelayRuntime::new(config) {
        Err(RelayError::Config(ConfigError::Handshake(message))) => {
            assert!(
                message.contains("descriptor_commit_hex"),
                "unexpected error message: {message}"
            );
        }
        Err(other) => panic!("expected handshake config error, got {other:?}"),
        Ok(_) => panic!("expected mismatch to error"),
    }
}

#[test]
fn runtime_config_requires_mldsa65_issuer_key() {
    let fixture = CertificateTestFixture::new();
    let json = format!(
        r#"{{
            "mode": "Entry",
            "listen": "127.0.0.1:0",
            "handshake": {{
                "identity_private_key_hex": "{identity_hex}",
                "descriptor_manifest_path": "{manifest}",
                "certificate": {{
                    "bundle_path": "{bundle}",
                    "issuer_ed25519_hex": "{issuer_ed}"
                }}
            }}
        }}"#,
        identity_hex = fixture.identity_seed_hex,
        manifest = fixture.manifest_file.path().display(),
        bundle = fixture.bundle_file.path().display(),
        issuer_ed = fixture.issuer_ed25519_hex,
    );
    let file = NamedTempFile::new().expect("create temp config");
    std::fs::write(file.path(), json).expect("write temp config");
    match RelayConfig::load(file.path()) {
        Err(ConfigError::Handshake(message)) => {
            assert!(
                message.contains("issuer_mldsa_hex"),
                "unexpected error message: {message}"
            );
        }
        Err(other) => panic!("expected handshake config error, got {other:?}"),
        Ok(_) => panic!("missing ML-DSA issuer key must fail config validation"),
    }
}

fn negotiated_caps_fixture() -> NegotiatedCapabilities {
    NegotiatedCapabilities {
        kem: KemAdvertisement {
            id: KemId::MlKem768,
            required: true,
        },
        signatures: vec![SignatureAdvertisement {
            id: SignatureId::Dilithium3,
            required: true,
        }],
        padding: 1024,
        descriptor_commit: None,
        grease: Vec::new(),
        constant_rate: None,
    }
}

#[test]
fn validate_client_selection_rejects_kem_mismatch() {
    let negotiated = negotiated_caps_fixture();
    let err = validate_client_selection(
        &negotiated,
        KemId::MlKem1024.code(),
        SignatureId::Dilithium3.code(),
    )
    .expect_err("kem mismatch should fail");
    match err {
        HandshakeError::InvalidClient(field) => {
            assert_eq!(field, "client kem_id does not match negotiated capability");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn validate_client_selection_rejects_signature_mismatch() {
    let negotiated = negotiated_caps_fixture();
    let err = validate_client_selection(
        &negotiated,
        KemId::MlKem768.code(),
        SignatureId::Falcon512.code(),
    )
    .expect_err("signature mismatch should fail");
    match err {
        HandshakeError::InvalidClient(field) => {
            assert_eq!(field, "client sig_id does not match negotiated capability");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn validate_client_selection_accepts_matching_ids() {
    let negotiated = negotiated_caps_fixture();
    validate_client_selection(
        &negotiated,
        KemId::MlKem768.code(),
        SignatureId::Dilithium3.code(),
    )
    .expect("matching ids accepted");
}

#[test]
fn append_grease_tlvs_preserves_order() {
    let base = vec![0xAA, 0xBB];
    let grease = vec![
        GreaseEntry {
            ty: 0x7f10,
            value: vec![0x01],
        },
        GreaseEntry {
            ty: 0x7f11,
            value: vec![0x02, 0x03],
        },
    ];
    let appended = append_grease_tlvs(base.clone(), &grease).expect("append grease");
    let expected = [
        0xAA, 0xBB, 0x7f, 0x10, 0x00, 0x01, 0x01, 0x7f, 0x11, 0x00, 0x02, 0x02, 0x03,
    ];
    assert_eq!(appended, expected);
}

#[test]
fn append_grease_tlvs_rejects_oversized_values_without_truncation() {
    let err = append_grease_tlvs(
        Vec::new(),
        &[GreaseEntry {
            ty: 0x7F20,
            value: vec![0xAB; usize::from(u16::MAX) + 1],
        }],
    )
    .expect_err("oversized GREASE TLV must fail");

    assert!(matches!(
        err,
        CapabilityError::CapabilityValueTooLarge {
            ty: 0x7F20,
            length
        } if length == usize::from(u16::MAX) + 1
    ));
}

#[test]
fn runtime_honours_configured_identity_key() {
    let seed_hex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    let json = format!(
        r#"{{
            "mode": "Entry",
            "listen": "127.0.0.1:0",
            "handshake": {{
                "identity_private_key_hex": "{seed_hex}"
            }}
        }}"#
    );
    let config = load_config(&json);
    let runtime = RelayRuntime::new(config).expect("runtime");
    let context = runtime.circuit_context();

    let seed_bytes = hex::decode(seed_hex).expect("valid hex");
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_bytes);
    let expected_private =
        PrivateKey::from_bytes(Algorithm::Ed25519, &seed).expect("configured key parse");
    let expected_pair =
        KeyPair::from_private_key(expected_private).expect("configured keypair derive");

    assert_eq!(
        context.identity_key.public_key(),
        expected_pair.public_key()
    );
}

#[test]
fn runtime_enables_pow_when_required() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("ticket-replays.norito");
    let json = format!(
        r#"{{
            "mode": "Entry",
            "listen": "127.0.0.1:0",
            "pow": {{
                "required": true,
                "difficulty": 6,
                "max_future_skew_secs": 120,
                "min_ticket_ttl_secs": 10,
                "revocation_store_path": "{}"
            }}
        }}"#,
        replay_path.display()
    );
    let config = load_config(&json);
    let runtime = RelayRuntime::new(config).expect("runtime");
    let context = runtime.circuit_context();

    assert!(context.dos.is_pow_required());
    assert_eq!(context.dos.current_pow_parameters().difficulty(), 6);
    let replay_state = context.ticket_replays.lock().expect("ticket replay lock");
    assert_eq!(replay_state.capacity, 8_192);
}

#[test]
fn runtime_fails_closed_on_corrupt_ticket_replay_snapshot() {
    let dir = tempdir().expect("tempdir");
    let replay_path = dir.path().join("ticket-replays.norito");
    std::fs::write(&replay_path, b"corrupt replay snapshot").expect("write corrupt snapshot");
    let json = format!(
        r#"{{
            "mode": "Entry",
            "listen": "127.0.0.1:0",
            "pow": {{
                "required": true,
                "difficulty": 6,
                "max_future_skew_secs": 120,
                "min_ticket_ttl_secs": 10,
                "revocation_store_path": "{}"
            }}
        }}"#,
        replay_path.display()
    );
    let config = load_config(&json);
    match RelayRuntime::new(config) {
        Err(RelayError::Config(ConfigError::TicketReplayStore(message))) => {
            assert!(
                message.contains("parse"),
                "unexpected replay-store error: {message}"
            );
        }
        Err(other) => panic!("expected ticket replay-store error, got {other:?}"),
        Ok(_) => panic!("corrupt ticket replay state must fail startup"),
    }
}

#[test]
fn runtime_loads_identity_from_manifest() {
    let seed_hex = "c1d1c2f493ad2db3fbc5ff0bfb8bb4e0f2c5c2d9e9caa8ffd5d38a1808fa4c55";
    let manifest = NamedTempFile::new().expect("create manifest file");
    std::fs::write(
        manifest.path(),
        format!(
            r#"{{
                "version": 1,
                "identity": {{
                    "ed25519_private_key_hex": "{seed_hex}"
                }}
            }}"#
        ),
    )
    .expect("write manifest");

    let manifest_path = manifest.path().to_str().expect("path to utf-8");
    let json = format!(
        r#"{{
            "mode": "Entry",
            "listen": "127.0.0.1:0",
            "handshake": {{
                "descriptor_manifest_path": "{manifest_path}"
            }}
        }}"#
    );
    let config = load_config(&json);
    let runtime = RelayRuntime::new(config).expect("runtime");
    let context = runtime.circuit_context();

    let seed_bytes = hex::decode(seed_hex).expect("valid hex");
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_bytes);
    let expected_private =
        PrivateKey::from_bytes(Algorithm::Ed25519, &seed).expect("manifest key parse");
    let expected_pair =
        KeyPair::from_private_key(expected_private).expect("manifest keypair derive");

    assert_eq!(
        context.identity_key.public_key(),
        expected_pair.public_key()
    );
}

#[test]
fn runtime_fails_when_manifest_missing_key() {
    let manifest = NamedTempFile::new().expect("create manifest file");
    std::fs::write(
        manifest.path(),
        r#"{ "version": 1, "identity": { "note": "no private key yet" } }"#,
    )
    .expect("write manifest");

    let manifest_path = manifest.path().to_str().expect("path to utf-8");
    let json = format!(
        r#"{{
            "mode": "Entry",
            "listen": "127.0.0.1:0",
            "handshake": {{
                "descriptor_manifest_path": "{manifest_path}"
            }}
        }}"#
    );
    let config = load_config(&json);
    match RelayRuntime::new(config) {
        Err(RelayError::Config(ConfigError::DescriptorManifest { message, .. })) => {
            assert!(
                message.contains("missing"),
                "unexpected manifest error message: {message}"
            );
        }
        Err(other) => panic!("expected manifest error, got {other:?}"),
        Ok(_) => panic!("expected manifest error, got Ok(_)"),
    }
}
