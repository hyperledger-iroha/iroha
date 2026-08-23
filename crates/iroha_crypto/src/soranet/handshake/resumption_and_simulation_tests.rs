#[test]
fn process_client_hello_rejects_resume_hash_mismatch() {
    let defaults = RuntimeParams::soranet_defaults();
    let resume_a =
        hex_literal::hex!("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff");
    let resume_b =
        hex_literal::hex!("ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100");
    let client_params = RuntimeParams {
        descriptor_commit: defaults.descriptor_commit,
        client_capabilities: defaults.client_capabilities,
        relay_capabilities: defaults.relay_capabilities,
        kem_id: defaults.kem_id,
        sig_id: defaults.sig_id,
        transport_alpn: defaults.transport_alpn,
        tls_server_name: defaults.tls_server_name,
        resume_hash: Some(&resume_a),
    };
    let mismatched_params = RuntimeParams {
        descriptor_commit: defaults.descriptor_commit,
        client_capabilities: defaults.client_capabilities,
        relay_capabilities: defaults.relay_capabilities,
        kem_id: defaults.kem_id,
        sig_id: defaults.sig_id,
        transport_alpn: defaults.transport_alpn,
        tls_server_name: defaults.tls_server_name,
        resume_hash: Some(&resume_b),
    };
    let absent_params = RuntimeParams {
        descriptor_commit: defaults.descriptor_commit,
        client_capabilities: defaults.client_capabilities,
        relay_capabilities: defaults.relay_capabilities,
        kem_id: defaults.kem_id,
        sig_id: defaults.sig_id,
        transport_alpn: defaults.transport_alpn,
        tls_server_name: defaults.tls_server_name,
        resume_hash: None,
    };
    let mut rng_client = StdRng::seed_from_u64(7);
    let (client_hello, _client_state) =
        build_client_hello(&client_params, &mut rng_client).expect("client hello");
    let relay_keys = checked_random_keypair();
    let mut rng_relay = StdRng::seed_from_u64(8);
    match process_client_hello(
        &client_hello,
        &mismatched_params,
        &relay_keys,
        &mut rng_relay,
    ) {
        Err(HarnessError::Validation(message)) => {
            assert!(
                message.contains("resume hash mismatch"),
                "unexpected message: {message}"
            );
        }
        Err(err) => panic!("expected resume mismatch, got {err:?}"),
        Ok(_) => panic!("expected resume mismatch, got Ok"),
    }
    let mut rng_relay = StdRng::seed_from_u64(9);
    match process_client_hello(&client_hello, &absent_params, &relay_keys, &mut rng_relay) {
        Err(HarnessError::Validation(message)) => {
            assert!(
                message.contains("unexpected resume hash"),
                "unexpected message: {message}"
            );
        }
        Err(err) => panic!("expected unexpected resume hash error, got {err:?}"),
        Ok(_) => panic!("expected unexpected resume hash error, got Ok"),
    }
}
#[test]
fn simulate_handshake_produces_transcript_hash() {
    let client_caps = DEFAULT_CLIENT_CAPABILITIES.to_vec();
    let relay_caps = DEFAULT_RELAY_CAPABILITIES.to_vec();
    let descriptor_commit =
        decode_hex("76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f")
            .expect("descriptor");
    let client_nonce =
        decode_hex("2c1f64028dbe42410d1921cd9a316bed4f8f5b52ffb62b4dcaf149048393ca8a")
            .expect("client nonce");
    let relay_nonce =
        decode_hex("d5f4f2f9c2b1a39e88bbd3c0a4f9e178d93e7bfacaf0c3e872b712f4a341c9de")
            .expect("relay nonce");
    let (client_static_sk, relay_static_sk) = sample_static_keys();
    let result = simulate_handshake(&SimulationParams {
        client_capabilities: &client_caps,
        relay_capabilities: &relay_caps,
        client_static_sk: &client_static_sk,
        relay_static_sk: &relay_static_sk,
        resume_hash: None,
        descriptor_commit: &descriptor_commit,
        client_nonce: &client_nonce,
        relay_nonce: &relay_nonce,
        kem_id: 1,
        sig_id: 1,
    })
    .expect("simulate");
    let expected = TranscriptInputs {
        descriptor_commit: &descriptor_commit,
        client_nonce: &client_nonce,
        relay_nonce: &relay_nonce,
        capability_bytes: &client_caps,
        kem_id: 1,
        sig_id: 1,
        handshake_suite: HandshakeSuite::Nk2Hybrid,
        resume_hash: None,
    }
    .compute_hash()
    .expect("transcript hash");
    assert_eq!(result.transcript_hash, expected);
    assert_eq!(result.handshake_suite, HandshakeSuite::Nk2Hybrid);
    assert!(result.warnings.is_empty());
    assert_eq!(result.telemetry_payloads.len(), 1);
    assert_eq!(result.handshake_steps.len(), 2);
    assert_eq!(result.handshake_steps[0].note, STEP_NOTE_HYBRID_INIT);
    assert_eq!(result.handshake_steps[1].note, STEP_NOTE_HYBRID_RESPONSE);
}
fn assert_telemetry_omits_secret_material(payloads: &[Vec<u8>]) {
    for payload in payloads {
        let text = std::str::from_utf8(payload).expect("telemetry JSON must be UTF-8");
        let value: Value = norito::json::from_str(text).expect("telemetry JSON must decode");
        assert!(
            value.get("shared_secret_hex").is_none(),
            "telemetry must never export KEM or session material"
        );
    }
}
#[test]
fn simulate_handshake_negotiates_nk2_hybrid_suite() {
    let client_caps = capabilities_with_suites(
        &DEFAULT_CLIENT_CAPABILITIES,
        &[HandshakeSuite::Nk2Hybrid],
        false,
    );
    let relay_caps = capabilities_with_suites(
        &DEFAULT_RELAY_CAPABILITIES,
        &[HandshakeSuite::Nk2Hybrid],
        false,
    );
    let descriptor_commit = DEFAULT_DESCRIPTOR_COMMIT.to_vec();
    let client_nonce =
        decode_hex("1f2e3d4c5b6a79888796a5b4c3d2e1f00112233445566778899aabbccddeeff0")
            .expect("client nonce");
    let relay_nonce =
        decode_hex("2b64a7e5c1d3f4b2a9c8d7e6f5a4132233445566778899aabbccddeeff001122")
            .expect("relay nonce");
    let (client_static_sk, relay_static_sk) = sample_static_keys();
    let result = simulate_handshake(&SimulationParams {
        client_capabilities: &client_caps,
        relay_capabilities: &relay_caps,
        client_static_sk: &client_static_sk,
        relay_static_sk: &relay_static_sk,
        resume_hash: None,
        descriptor_commit: &descriptor_commit,
        client_nonce: &client_nonce,
        relay_nonce: &relay_nonce,
        kem_id: 1,
        sig_id: 1,
    })
    .expect("simulate");
    assert_eq!(result.handshake_suite, HandshakeSuite::Nk2Hybrid);
    assert!(result.warnings.is_empty());
    assert_eq!(result.handshake_steps.len(), 2);
    assert_eq!(result.handshake_steps[0].note, STEP_NOTE_HYBRID_INIT);
    assert_eq!(result.handshake_steps[1].note, STEP_NOTE_HYBRID_RESPONSE);
    assert_eq!(result.telemetry_payloads.len(), 1);
    assert_telemetry_omits_secret_material(&result.telemetry_payloads);
}
#[test]
fn simulate_handshake_negotiates_nk3_forward_secure_suite() {
    let client_caps = capabilities_with_suites(
        &DEFAULT_CLIENT_CAPABILITIES,
        &[
            HandshakeSuite::Nk3PqForwardSecure,
            HandshakeSuite::Nk2Hybrid,
        ],
        false,
    );
    let relay_caps = capabilities_with_suites(
        &DEFAULT_RELAY_CAPABILITIES,
        &[
            HandshakeSuite::Nk3PqForwardSecure,
            HandshakeSuite::Nk2Hybrid,
        ],
        false,
    );
    let descriptor_commit = DEFAULT_DESCRIPTOR_COMMIT.to_vec();
    let client_nonce =
        decode_hex("3c2b1a09180706050403020100112233445566778899aabbccddeeff00112233")
            .expect("client nonce");
    let relay_nonce =
        decode_hex("445566778899aabbccddeeff00112233445566778899aabbccddeeff10213254")
            .expect("relay nonce");
    let (client_static_sk, relay_static_sk) = sample_static_keys();
    let result = simulate_handshake(&SimulationParams {
        client_capabilities: &client_caps,
        relay_capabilities: &relay_caps,
        client_static_sk: &client_static_sk,
        relay_static_sk: &relay_static_sk,
        resume_hash: None,
        descriptor_commit: &descriptor_commit,
        client_nonce: &client_nonce,
        relay_nonce: &relay_nonce,
        kem_id: 1,
        sig_id: 1,
    })
    .expect("simulate");
    assert_eq!(result.handshake_suite, HandshakeSuite::Nk3PqForwardSecure);
    assert!(result.warnings.is_empty());
    assert_eq!(result.handshake_steps.len(), 2);
    assert_eq!(result.handshake_steps[0].note, STEP_NOTE_PQFS_COMMIT);
    assert_eq!(result.handshake_steps[1].note, STEP_NOTE_PQFS_RESPONSE);
    assert_eq!(result.telemetry_payloads.len(), 1);
    assert_telemetry_omits_secret_material(&result.telemetry_payloads);
}
