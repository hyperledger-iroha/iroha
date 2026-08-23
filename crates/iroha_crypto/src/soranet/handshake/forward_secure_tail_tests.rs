#[test]
fn simulate_handshake_surfaces_warnings() {
    let client_caps = decode_hex(
        "0101000201010102000201010104000284050202000200047f100004deadbeef7f110004cafebabe",
    )
    .expect("client hex");
    let relay_caps = decode_hex(
            "0102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002840502010001010202000200047f1300040badc0de",
        )
        .expect("relay hex");
    let descriptor_commit =
        decode_hex("76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f")
            .expect("descriptor");
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
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(result.warnings[0].capability_type, 0x0101);
    assert!(result.warnings[0].message.contains("snnet.pqkem"));
    assert_eq!(result.telemetry_payloads.len(), 1);
}
#[test]
fn simulation_report_json_renders_expected_fields() {
    let client_caps = decode_hex(
        "0101000201010102000201010104000284050202000200047f100004deadbeef7f110004cafebabe",
    )
    .expect("client hex");
    let relay_caps = decode_hex(
            "0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002840502010001010202000200047f12000412345678",
        )
        .expect("relay hex");
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
    let json = simulation_report_json(&result, None).expect("json");
    let value: Value = norito::json::from_str(&json).expect("json parse");
    let transcript_hash = value
        .get("transcript_hash_hex")
        .and_then(Value::as_str)
        .expect("transcript hash");
    assert_eq!(transcript_hash.len(), 64);
    assert!(transcript_hash.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert_eq!(
        value.get("handshake_suite").and_then(Value::as_str),
        Some("nk2.hybrid.v1")
    );
    assert!(value.get("client_capabilities").is_some());
    assert!(value.get("relay_capabilities").is_some());
    assert!(value.get("handshake_steps").is_some());
}
#[test]
fn simulation_report_json_filters_warnings() {
    let client_caps = decode_hex(
        "0101000201010102000201010104000284050202000200047f100004deadbeef7f110004cafebabe",
    )
    .expect("client hex");
    let relay_caps = decode_hex(
            "0102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002840502010001010202000200047f1300040badc0de",
        )
        .expect("relay hex");
    let descriptor_commit =
        decode_hex("76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f")
            .expect("descriptor");
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
    let json = simulation_report_json(&result, Some(&[0x0202])).expect("json");
    let value: Value = norito::json::from_str(&json).expect("json parse");
    assert!(value["warnings"].as_array().unwrap().is_empty());
}
#[test]
fn suite_key_schedule_uses_distinct_domains() {
    let transcript = [0xAA; 32];
    let primary = [0x11; 32];
    let forward = [0x22; 32];
    let noise_xx_dh = NoiseXxDhSecrets {
        ee: Zeroizing::new([0x31; NOISE_SECRET_LEN]),
        es: Zeroizing::new([0x32; NOISE_SECRET_LEN]),
        se: Zeroizing::new([0x33; NOISE_SECRET_LEN]),
    };
    let (nk2_session, nk2_confirm) = derive_session_key_and_confirmation(SessionKeyInputs {
        suite: HandshakeSuite::Nk2Hybrid,
        transcript_hash: &transcript,
        noise_xx_dh: &noise_xx_dh,
        primary_shared: &primary,
        forward_shared: None,
    })
    .expect("nk2 schedule");
    let (nk3_session, nk3_confirm) = derive_session_key_and_confirmation(SessionKeyInputs {
        suite: HandshakeSuite::Nk3PqForwardSecure,
        transcript_hash: &transcript,
        noise_xx_dh: &noise_xx_dh,
        primary_shared: &primary,
        forward_shared: Some(&forward),
    })
    .expect("nk3 schedule");
    assert_ne!(nk2_session.payload(), nk3_session.payload());
    assert_ne!(nk2_confirm, nk3_confirm);
}
#[test]
fn session_key_hkdf_length_prefixes_ikm_parts() {
    let transcript = [0xA5; 32];
    let expand = |parts: &[&[u8]]| {
        let hk = hkdf_sha3_256_from_ikm_parts(Some(&transcript), parts)
            .expect("length-prefixed HKDF input derives");
        let mut out = vec![0_u8; 32];
        hk.expand(b"soranet.handshake.test", &mut out)
            .expect("fixed test output length");
        out
    };
    let left_parts: [&[u8]; 2] = [b"ab".as_ref(), b"c".as_ref()];
    let right_parts: [&[u8]; 2] = [b"a".as_ref(), b"bc".as_ref()];
    let left = expand(&left_parts);
    let duplicate_left = expand(&left_parts);
    let right = expand(&right_parts);
    assert_eq!(left, duplicate_left);
    assert_ne!(left, right);
}
#[test]
fn nk3_key_schedule_requires_forward_secret() {
    let transcript = [0xAB; 32];
    let primary = [0xCD; 32];
    let noise_xx_dh = NoiseXxDhSecrets {
        ee: Zeroizing::new([0x41; NOISE_SECRET_LEN]),
        es: Zeroizing::new([0x42; NOISE_SECRET_LEN]),
        se: Zeroizing::new([0x43; NOISE_SECRET_LEN]),
    };
    let err = match derive_session_key_and_confirmation(SessionKeyInputs {
        suite: HandshakeSuite::Nk3PqForwardSecure,
        transcript_hash: &transcript,
        noise_xx_dh: &noise_xx_dh,
        primary_shared: &primary,
        forward_shared: None,
    }) {
        Ok(_) => panic!("nk3 schedule without forward secret must error"),
        Err(err) => err,
    };
    match err {
        HarnessError::Validation(message) => assert!(
            message.contains("forward-secure"),
            "unexpected error message: {message}"
        ),
        other => panic!("unexpected error {other:?}"),
    }
}
