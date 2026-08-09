#[cfg(test)]
mod tests {
    use core::ops::Range;

    use rand::{SeedableRng, rngs::StdRng};
    use rand_core::{CryptoRng, RngCore, TryCryptoRng, TryRngCore};

    use super::*;

    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked SoraNet handshake fixture keypair")
    }

    fn checked_seeded_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked SoraNet handshake fixture keypair")
    }

    fn assert_relay_authentication_roundtrip(algorithm: Algorithm, seed: u8) {
        let relay_keys = KeyPair::try_from_seed(vec![seed; 32], algorithm)
            .expect("derive checked relay identity keypair");
        let wrong_relay_keys = KeyPair::try_from_seed(vec![seed.wrapping_add(1); 32], algorithm)
            .expect("derive checked mismatched relay identity keypair");
        let client_hello = b"algorithm-agile-client-hello";
        let relay_body = b"algorithm-agile-relay-body";
        let transcript_hash = [0xA5; 32];
        let mut frame = Vec::new();

        append_relay_authentication(
            &mut frame,
            HandshakeSuite::Nk2Hybrid,
            client_hello,
            relay_body,
            &transcript_hash,
            &relay_keys,
            b"iroha-p2p/1",
            "iroha-quic",
        )
        .expect("append authenticated relay identity");

        let mut cursor = MessageCursor::new(&frame);
        let (relay_identity, signature) =
            read_relay_authentication(&mut cursor).expect("parse authenticated relay identity");
        assert!(cursor.remaining_slice().is_empty());
        assert_eq!(relay_identity, relay_identity_bytes(&relay_keys).unwrap());
        assert_eq!(signature.len(), algorithm.signature_payload_len());
        if algorithm == Algorithm::Ed25519 {
            assert_eq!(
                relay_identity.len(),
                32,
                "Ed25519 keeps its legacy wire form"
            );
        } else {
            assert_eq!(relay_identity.first().copied(), Some(algorithm as u8));
        }

        verify_relay_authentication(
            HandshakeSuite::Nk2Hybrid,
            client_hello,
            relay_body,
            &transcript_hash,
            &relay_identity,
            &signature,
            relay_keys.public_key(),
            b"iroha-p2p/1",
            "iroha-quic",
        )
        .expect("verify authenticated relay identity");

        let mismatch = verify_relay_authentication(
            HandshakeSuite::Nk2Hybrid,
            client_hello,
            relay_body,
            &transcript_hash,
            &relay_identity,
            &signature,
            wrong_relay_keys.public_key(),
            b"iroha-p2p/1",
            "iroha-quic",
        )
        .expect_err("mismatched expected relay identity must fail");
        assert!(
            mismatch
                .to_string()
                .contains("authenticated directory identity")
        );

        let mut tampered_signature = signature;
        tampered_signature[0] ^= 0x80;
        let tampered = verify_relay_authentication(
            HandshakeSuite::Nk2Hybrid,
            client_hello,
            relay_body,
            &transcript_hash,
            &relay_identity,
            &tampered_signature,
            relay_keys.public_key(),
            b"iroha-p2p/1",
            "iroha-quic",
        )
        .expect_err("tampered relay signature must fail");
        assert!(
            tampered
                .to_string()
                .contains("signature verification failed")
        );
    }

    #[test]
    fn relay_authentication_ed25519_roundtrip_rejects_mismatch_and_tamper() {
        assert_relay_authentication_roundtrip(Algorithm::Ed25519, 0x61);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn relay_authentication_bls_normal_roundtrip_rejects_mismatch_and_tamper() {
        assert_relay_authentication_roundtrip(Algorithm::BlsNormal, 0x71);
    }

    fn authenticated_exchange(
        client_rng_seed: u64,
        relay_rng_seed: u64,
        relay_identity_seed: u8,
    ) -> (RuntimeParams<'static>, ClientState, Vec<u8>, KeyPair) {
        let params = RuntimeParams::soranet_defaults();
        let mut rng_client = StdRng::seed_from_u64(client_rng_seed);
        let mut rng_relay = StdRng::seed_from_u64(relay_rng_seed);
        let relay_keys = checked_seeded_keypair(relay_identity_seed);
        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("client hello");
        let (relay_hello, _relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("authenticated relay hello");
        (params, client_state, relay_hello, relay_keys)
    }

    struct PanicRng;

    impl RngCore for PanicRng {
        fn next_u32(&mut self) -> u32 {
            panic!("relay RNG must not be consumed before KEM preflight");
        }

        fn next_u64(&mut self) -> u64 {
            panic!("relay RNG must not be consumed before KEM preflight");
        }

        fn fill_bytes(&mut self, _dest: &mut [u8]) {
            panic!("relay RNG must not be consumed before KEM preflight");
        }
    }

    impl CryptoRng for PanicRng {}

    struct FailingTryRng;

    #[derive(Debug)]
    struct FailingTryRngError;

    impl fmt::Display for FailingTryRngError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("failing SoraNet handshake RNG")
        }
    }

    impl TryRngCore for FailingTryRng {
        type Error = FailingTryRngError;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(FailingTryRngError)
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(FailingTryRngError)
        }

        fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Self::Error> {
            Err(FailingTryRngError)
        }
    }

    impl TryCryptoRng for FailingTryRng {}

    struct FixedTryRng {
        byte: u8,
    }

    impl TryRngCore for FixedTryRng {
        type Error = core::convert::Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Ok(u32::from_le_bytes([self.byte; 4]))
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok(u64::from_le_bytes([self.byte; 8]))
        }

        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
            dst.fill(self.byte);
            Ok(())
        }
    }

    impl TryCryptoRng for FixedTryRng {}

    #[test]
    fn fill_random_rejects_all_zero_material() {
        let mut rng = FixedTryRng { byte: 0 };
        let mut dest = [0xFF; 32];

        let err = fill_random(&mut rng, "building client hello nonce", &mut dest)
            .expect_err("all-zero fill must fail");

        match err {
            HarnessError::RandomBytes { operation, message } => {
                assert_eq!(operation, "building client hello nonce");
                assert_eq!(message, "rng returned all-zero material");
            }
            other => panic!("expected RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn encode_signature_returns_prefixed_base64() {
        let encoded = encode_signature("ed25519", &[0x00, 0x01]).expect("signature encoding");
        assert_eq!(encoded, "ed25519:AAE=");
    }

    #[test]
    fn message_cursor_reads_exact_len_prefixed_and_remaining_bytes() {
        let frame = [0xAA, 0, 2, 0xBB, 0xCC, 0xDD];
        let mut cursor = MessageCursor::new(&frame);

        assert_eq!(cursor.read_u8().unwrap(), 0xAA);
        assert_eq!(cursor.read_len_prefixed().unwrap(), &[0xBB, 0xCC]);
        assert_eq!(cursor.remaining_slice(), &[0xDD]);
    }

    #[test]
    fn message_cursor_rejects_truncated_or_overflowed_reads_without_advancing() {
        let mut truncated = MessageCursor::new(&[0xAA]);
        let err = truncated
            .read_exact(2)
            .expect_err("truncated read should fail closed");
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("handshake message truncated"));
                assert!(message.contains("offset=0"));
                assert!(message.contains("need=2"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
        assert_eq!(truncated.pos, 0);

        let mut overflowed = MessageCursor::new(&[]);
        overflowed.pos = usize::MAX;
        let err = overflowed
            .read_exact(1)
            .expect_err("overflowed read should fail closed");
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("handshake message truncated"));
                assert!(message.contains(&format!("offset={}", usize::MAX)));
                assert!(message.contains("need=1"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
        assert_eq!(overflowed.pos, usize::MAX);
        assert!(overflowed.remaining_slice().is_empty());
    }

    #[test]
    fn update_suite_list_sets_required_flag() {
        let suites = [HandshakeSuite::Nk3PqForwardSecure];
        let vector = super::update_suite_list(&DEFAULT_CLIENT_CAPABILITIES, &suites, true).unwrap();
        let parsed = parse_capabilities(&vector).expect("parse updated capabilities");
        let suite_cap = parsed
            .into_iter()
            .find(|cap| cap.ty == CAPABILITY_SUITE_LIST)
            .expect("suite_list capability present");
        assert_eq!(
            suite_cap.value,
            vec![u8::from(HandshakeSuite::Nk3PqForwardSecure)]
        );
        assert!(suite_cap.required, "required flag propagated");
    }

    #[test]
    fn handshake_suite_decoder_accepts_only_first_release_wire_ids() {
        assert_eq!(
            HandshakeSuite::try_from(0x04).expect("nk2 suite id"),
            HandshakeSuite::Nk2Hybrid
        );
        assert_eq!(
            HandshakeSuite::try_from(0x05).expect("nk3 suite id"),
            HandshakeSuite::Nk3PqForwardSecure
        );
        assert!(
            HandshakeSuite::try_from(0x02).is_err(),
            "old pre-release NK2 id must be rejected"
        );
        assert!(
            HandshakeSuite::try_from(0x03).is_err(),
            "old pre-release NK3 id must be rejected"
        );
        assert_eq!(HandshakeSuite::Nk2Hybrid.label(), "nk2.hybrid.v1");
        assert_eq!(
            HandshakeSuite::Nk3PqForwardSecure.label(),
            "nk3.pq_forward_secure.v1"
        );
    }

    #[test]
    fn default_capability_vectors_advertise_first_release_suites() {
        for (label, vector) in [
            ("client", DEFAULT_CLIENT_CAPABILITIES.as_slice()),
            ("relay", DEFAULT_RELAY_CAPABILITIES.as_slice()),
        ] {
            let caps = parse_capabilities(vector).expect("default capabilities parse");
            let suite_cap = caps
                .iter()
                .find(|cap| cap.ty == CAPABILITY_SUITE_LIST)
                .unwrap_or_else(|| panic!("{label} suite_list capability present"));
            assert_eq!(
                suite_cap.value,
                vec![
                    u8::from(HandshakeSuite::Nk2Hybrid),
                    u8::from(HandshakeSuite::Nk3PqForwardSecure),
                ],
                "{label} suite_list uses first-release IDs"
            );
            assert!(suite_cap.required, "{label} suite_list is required");
        }
    }

    #[test]
    fn update_suite_list_rejects_oversized_suite_list() {
        let suites = vec![HandshakeSuite::Nk2Hybrid; usize::from(u16::MAX) + 1];
        let err = super::update_suite_list(&DEFAULT_CLIENT_CAPABILITIES, &suites, false)
            .expect_err("oversized suite list must fail");
        match err {
            HarnessError::Validation(message) => {
                assert!(
                    message.contains("suite_list"),
                    "unexpected message: {message}"
                );
                assert!(
                    message.contains("exceeds u16::MAX"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn parse_suite_list_ignores_unknown_ids() {
        let cap = CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![
                u8::from(HandshakeSuite::Nk2Hybrid),
                0x7D,
                u8::from(HandshakeSuite::Nk3PqForwardSecure),
                0x7E,
            ],
            required: false,
        };
        let list = parse_suite_list(&cap).expect("parse suite list");
        assert_eq!(
            list.suites,
            vec![
                HandshakeSuite::Nk2Hybrid,
                HandshakeSuite::Nk3PqForwardSecure
            ]
        );
        assert!(!list.required);
    }

    #[test]
    fn parse_suite_list_rejects_only_pre_release_ids() {
        let cap = CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![0x02, 0x03],
            required: true,
        };
        let err =
            parse_suite_list(&cap).expect_err("pre-release-only suite list must not negotiate");
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("pre-release handshake suite identifiers"));
                assert!(message.contains("0x02"));
                assert!(message.contains("0x03"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn parse_capabilities_rejects_unknown_type() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x0301u16.to_be_bytes());
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.push(0);
        let err = parse_capabilities(&buf).expect_err("unknown capability should fail");
        assert!(matches!(err, HarnessError::CapabilityType(_)));
    }

    #[test]
    fn parse_capabilities_rejects_truncated_header_prefixes() {
        let mut header = [0u8; 4];
        header[..2].copy_from_slice(&CAPABILITY_ROLE.to_be_bytes());
        header[3] = 1;
        for len in 1..4 {
            let err = parse_capabilities(&header[..len])
                .expect_err("truncated TLV header should fail closed");
            assert!(matches!(err, HarnessError::TlvTruncated(0)));
        }
    }

    #[test]
    fn parse_capabilities_rejects_overlong_value_without_panic() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&CAPABILITY_ROLE.to_be_bytes());
        buf.extend_from_slice(&2u16.to_be_bytes());
        buf.push(0x01);

        let err = parse_capabilities(&buf).expect_err("overlong value should fail closed");
        assert!(matches!(err, HarnessError::TlvLengthExceeded(4)));
    }

    #[test]
    fn capability_tlv_readers_reject_overflowed_offsets_without_advancing() {
        let mut header_offset = usize::MAX;
        let err = read_capability_header(&[], &mut header_offset)
            .expect_err("overflowed header cursor should fail closed");
        assert!(matches!(err, HarnessError::TlvTruncated(usize::MAX)));
        assert_eq!(header_offset, usize::MAX);

        let mut value_offset = usize::MAX;
        let err = read_capability_value(&[], &mut value_offset, 1)
            .expect_err("overflowed value cursor should fail closed");
        assert!(matches!(err, HarnessError::TlvLengthExceeded(usize::MAX)));
        assert_eq!(value_offset, usize::MAX);
    }

    #[test]
    fn parse_capabilities_rejects_duplicate_singleton() {
        let entries = vec![
            CapabilityTlv {
                ty: CAPABILITY_ROLE,
                value: vec![0x01],
                required: false,
            },
            CapabilityTlv {
                ty: CAPABILITY_ROLE,
                value: vec![0x02],
                required: false,
            },
        ];
        let buf = encode_tlvs(&entries);
        let err = parse_capabilities(&buf).expect_err("duplicate role should fail");
        assert!(matches!(err, HarnessError::DuplicateCapability(_)));
    }

    #[test]
    fn parse_capabilities_allows_duplicate_pqsig() {
        let entries = vec![
            CapabilityTlv {
                ty: CAPABILITY_PQSIG,
                value: vec![0x01, 0x00],
                required: false,
            },
            CapabilityTlv {
                ty: CAPABILITY_PQSIG,
                value: vec![0x02, 0x00],
                required: false,
            },
        ];
        let buf = encode_tlvs(&entries);
        let parsed = parse_capabilities(&buf).expect("pqsig duplicates allowed");
        let pqsigs: Vec<_> = parsed
            .iter()
            .filter(|cap| cap.ty == CAPABILITY_PQSIG)
            .collect();
        assert_eq!(pqsigs.len(), 2);
        assert_eq!(pqsigs[0].value, vec![0x01, 0x00]);
        assert_eq!(pqsigs[1].value, vec![0x02, 0x00]);
    }

    #[test]
    fn build_interop_value_exposes_session_key() {
        for spec in super::INTEROP_SPECS {
            let value = super::build_interop_value(spec).expect("interop vector");
            let root = value
                .as_object()
                .expect("interop value should be a JSON object");
            assert_eq!(
                root.get("suite").and_then(Value::as_str),
                Some(spec.suite.label())
            );
            let outputs = root
                .get("outputs")
                .and_then(Value::as_object)
                .expect("outputs object present");
            let session_key = outputs
                .get("session_key_hex")
                .and_then(Value::as_str)
                .expect("session key present");
            assert_eq!(
                session_key.len(),
                64,
                "session key hex should be 32 bytes (64 hex chars)"
            );
        }
    }

    #[test]
    fn generated_interop_values_match_canonical_rust_fixtures() {
        for spec in super::INTEROP_SPECS {
            let fixture = match spec.id {
                "snnet-interop-nk2-v1" => {
                    include_str!(
                        "../../../../tests/interop/soranet/interop/rust/snnet-interop-nk2-v1.json"
                    )
                }
                "snnet-interop-nk3-v1" => {
                    include_str!(
                        "../../../../tests/interop/soranet/interop/rust/snnet-interop-nk3-v1.json"
                    )
                }
                other => panic!("unexpected SoraNet interop spec id: {other}"),
            };
            let expected: Value =
                norito::json::from_str(fixture).expect("canonical Rust interop fixture parses");

            let mut generated = super::build_interop_value(spec).expect("generated interop value");
            let Value::Object(ref mut generated_map) = generated else {
                panic!("generated interop value should be a JSON object");
            };
            generated_map.insert("language".to_string(), Value::from("rust"));

            assert_eq!(
                generated, expected,
                "generated Rust interop vector drifted from checked-in fixture {}",
                spec.id
            );
        }
    }

    #[test]
    fn expand_material_length_prefixes_label_and_parts() {
        let baseline = expand_material(b"label", &[b"ab".as_slice(), b"c".as_slice()], 32);
        let duplicate = expand_material(b"label", &[b"ab".as_slice(), b"c".as_slice()], 32);
        assert_eq!(baseline, duplicate);

        let changed_part_boundary =
            expand_material(b"label", &[b"a".as_slice(), b"bc".as_slice()], 32);
        assert_ne!(baseline, changed_part_boundary);

        let changed_label_boundary =
            expand_material(b"labe", &[b"lab".as_slice(), b"c".as_slice()], 32);
        assert_ne!(baseline, changed_label_boundary);
    }

    #[test]
    fn prepare_capability_context_matches_defaults() {
        let spec = &super::INTEROP_SPECS[0];
        let ctx = super::prepare_capability_context(spec).expect("capability context");
        assert_eq!(
            ctx.client_caps.len(),
            super::DEFAULT_CLIENT_CAPABILITIES.len()
        );
        assert_eq!(
            ctx.relay_caps.len(),
            super::DEFAULT_RELAY_CAPABILITIES.len()
        );
        assert!(ctx.warnings.is_empty(), "expected no negotiation warnings");
        assert!(
            !ctx.client_tlvs.is_empty() && !ctx.relay_tlvs.is_empty(),
            "parsed TLVs should not be empty"
        );
    }

    #[test]
    fn decode_handshake_inputs_handles_resume_hash() {
        let base = &super::INTEROP_SPECS[0];
        let resume_hex = "aabbccddeeff00112233445566778899";
        let spec = super::InteropSpec {
            id: "resume-test",
            description: "spec exercising resume hash decoding",
            suite: base.suite,
            client_suites: base.client_suites,
            client_required: base.client_required,
            relay_suites: base.relay_suites,
            relay_required: base.relay_required,
            client_static_sk_hex: base.client_static_sk_hex,
            relay_static_sk_hex: base.relay_static_sk_hex,
            client_nonce_hex: base.client_nonce_hex,
            relay_nonce_hex: base.relay_nonce_hex,
            resume_hash_hex: Some(resume_hex),
            kem_id: base.kem_id,
            sig_id: base.sig_id,
        };

        let inputs = super::decode_handshake_inputs(&spec).expect("decoded inputs");
        assert_eq!(
            inputs.client_static.to_vec(),
            super::decode_hex(base.client_static_sk_hex)
                .expect("decode baseline client static key")
        );
        let resume = inputs.resume_hash.expect("resume hash decoded");
        assert_eq!(
            resume,
            super::decode_hex(resume_hex).expect("decode resume hex")
        );
    }

    #[test]
    fn build_session_artifacts_emits_forward_shared_for_nk3() {
        let spec = &super::INTEROP_SPECS[1];
        let ctx = super::prepare_capability_context(spec).expect("capability context");
        let inputs = super::decode_handshake_inputs(spec).expect("decoded inputs");
        let params = super::build_simulation_params(spec, &ctx, &inputs);
        let transcript =
            super::compute_transcript_hash(&params, spec.suite).expect("transcript hash");
        let material =
            super::derive_handshake_material(&params, &inputs.client_static, &inputs.relay_static)
                .expect("handshake material");

        let session =
            super::build_session_artifacts(spec, &params, &transcript, &material, &ctx.warnings)
                .expect("session artifacts");

        assert!(
            session.forward_shared.is_some(),
            "NK3 should derive forward secret"
        );
        assert!(
            session.dual_mix.is_some(),
            "NK3 should derive dual mix material"
        );
        assert_eq!(session.session_key.len(), 32, "session key length");
        assert!(
            !session.handshake.steps.is_empty(),
            "handshake steps should not be empty"
        );
    }

    struct Nk3Fixture {
        client_state: ClientState,
        relay_response: PqfsRelayParsed,
        suite: MlKemSuite,
        client_caps: Vec<u8>,
        relay_caps: Vec<u8>,
        kem_id: u8,
        sig_id: u8,
    }

    fn build_nk3_fixture() -> Nk3Fixture {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let (client_state, parsed, suite) = {
            let params = RuntimeParams {
                descriptor_commit: defaults.descriptor_commit,
                client_capabilities: client_caps.as_slice(),
                relay_capabilities: relay_caps.as_slice(),
                kem_id: defaults.kem_id,
                sig_id: defaults.sig_id,
                transport_alpn: defaults.transport_alpn,
                tls_server_name: defaults.tls_server_name,
                resume_hash: defaults.resume_hash,
            };

            let mut rng_client = StdRng::seed_from_u64(2024);
            let mut rng_relay = StdRng::seed_from_u64(4048);
            let relay_keys = checked_random_keypair();

            let (client_hello, client_state) =
                build_client_hello(&params, &mut rng_client).expect("nk3 client hello");
            let (relay_hello, _relay_state) =
                process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                    .expect("nk3 relay response");

            let profile = kem_profile(params.kem_id).expect("kem profile");
            let parsed =
                parse_pqfs_relay_response(&relay_hello, params.descriptor_commit, profile.suite())
                    .expect("parse nk3 response");

            (client_state, parsed, profile.suite())
        };

        Nk3Fixture {
            client_state,
            relay_response: parsed,
            suite,
            client_caps,
            relay_caps,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
        }
    }

    fn sample_static_keys() -> (Vec<u8>, Vec<u8>) {
        let client_static =
            decode_hex("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")
                .expect("client static hex");
        let relay_static =
            decode_hex("ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100")
                .expect("relay static hex");
        (client_static, relay_static)
    }

    fn suite_list_tlv(required: bool, suites: &[HandshakeSuite]) -> Vec<u8> {
        assert!(
            !suites.is_empty(),
            "suite list helper expects at least one entry"
        );
        let mut values: Vec<u8> = suites.iter().copied().map(u8::from).collect();
        if required {
            values[0] |= 0x80;
        }
        let mut buf = Vec::new();
        buf.extend_from_slice(&CAPABILITY_SUITE_LIST.to_be_bytes());
        let len = u16::try_from(values.len()).expect("suite list length fits u16");
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&values);
        buf
    }

    fn encode_tlvs(entries: &[CapabilityTlv]) -> Vec<u8> {
        let mut buf = Vec::new();
        for cap in entries {
            let mut value = cap.value.clone();
            apply_required_flag(cap.ty, &mut value, cap.required);
            buf.extend_from_slice(&cap.ty.to_be_bytes());
            let len = u16::try_from(value.len()).expect("capability value fits u16");
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend_from_slice(&value);
        }
        buf
    }

    fn capabilities_with_suites(base: &[u8], suites: &[HandshakeSuite], required: bool) -> Vec<u8> {
        assert!(
            !suites.is_empty(),
            "suite list helper expects at least one entry"
        );
        let mut entries = parse_capabilities(base).expect("parse base capabilities");
        let value: Vec<u8> = suites.iter().copied().map(u8::from).collect();
        if let Some(existing) = entries
            .iter_mut()
            .find(|cap| cap.ty == CAPABILITY_SUITE_LIST)
        {
            existing.value = value;
            existing.required = required;
        } else {
            entries.push(CapabilityTlv {
                ty: CAPABILITY_SUITE_LIST,
                value,
                required,
            });
        }
        entries.sort_by_key(|cap| cap.ty);
        encode_tlvs(&entries)
    }

    fn capabilities_with_selected_id(base: &[u8], ty: u16, id: u8, required: bool) -> Vec<u8> {
        let mut entries = parse_capabilities(base).expect("parse base capabilities");
        entries.push(CapabilityTlv {
            ty,
            value: vec![id, 0],
            required,
        });
        entries.sort_by_key(|cap| cap.ty);
        encode_tlvs(&entries)
    }

    fn capabilities_with_required_flag(base: &[u8], ty: u16, id: u8, required: bool) -> Vec<u8> {
        let mut entries = parse_capabilities(base).expect("parse base capabilities");
        let cap = entries
            .iter_mut()
            .find(|cap| cap.ty == ty && cap.value.first().copied() == Some(id))
            .expect("selected capability present");
        cap.required = required;
        encode_tlvs(&entries)
    }

    fn capabilities_with_replaced_id(base: &[u8], ty: u16, from_id: u8, to_id: u8) -> Vec<u8> {
        let mut entries = parse_capabilities(base).expect("parse base capabilities");
        let cap = entries
            .iter_mut()
            .find(|cap| cap.ty == ty && cap.value.first().copied() == Some(from_id))
            .expect("selected capability present");
        cap.value[0] = to_id;
        encode_tlvs(&entries)
    }

    fn len_prefixed_payload_range(frame: &[u8], offset: &mut usize) -> Range<usize> {
        let len = u16::from_be_bytes([frame[*offset], frame[*offset + 1]]) as usize;
        *offset += 2;
        let start = *offset;
        *offset += len;
        start..*offset
    }

    fn len_prefixed_header_range(frame: &[u8], offset: &mut usize) -> Range<usize> {
        let start = *offset;
        let len = u16::from_be_bytes([frame[*offset], frame[*offset + 1]]) as usize;
        *offset += 2 + len;
        start..start + 2
    }

    fn skip_len_prefixed_payload(frame: &[u8], offset: &mut usize) {
        let _ = len_prefixed_payload_range(frame, offset);
    }

    fn client_hello_ephemeral_range(frame: &[u8]) -> Range<usize> {
        let mut offset = 1;
        skip_len_prefixed_payload(frame, &mut offset);
        offset += 3;
        offset..offset + NOISE_SECRET_LEN
    }

    fn client_hello_suite_byte_index(frame: &[u8]) -> usize {
        let mut offset = 1;
        skip_len_prefixed_payload(frame, &mut offset);
        offset
    }

    fn client_hello_static_range(frame: &[u8]) -> Range<usize> {
        let mut offset = client_hello_ephemeral_range(frame).end;
        len_prefixed_payload_range(frame, &mut offset)
    }

    fn client_hello_primary_kem_range(frame: &[u8]) -> Range<usize> {
        let mut offset = client_hello_static_range(frame).end;
        len_prefixed_payload_range(frame, &mut offset)
    }

    fn client_hello_forward_kem_range(frame: &[u8]) -> Range<usize> {
        let mut offset = client_hello_primary_kem_range(frame).end;
        len_prefixed_payload_range(frame, &mut offset)
    }

    fn relay_response_ephemeral_range(frame: &[u8]) -> Range<usize> {
        let mut offset = 1;
        skip_len_prefixed_payload(frame, &mut offset);
        len_prefixed_payload_range(frame, &mut offset)
    }

    fn relay_response_static_range(frame: &[u8]) -> Range<usize> {
        let mut offset = relay_response_ephemeral_range(frame).end;
        len_prefixed_payload_range(frame, &mut offset)
    }

    fn relay_response_capabilities_range(frame: &[u8]) -> Range<usize> {
        let mut offset = relay_response_static_range(frame).end;
        len_prefixed_payload_range(frame, &mut offset)
    }

    fn overwrite_noise_key(frame: &mut [u8], range: Range<usize>, replacement: [u8; 32]) {
        assert_eq!(range.len(), NOISE_SECRET_LEN);
        frame[range].copy_from_slice(&replacement);
    }

    fn shorten_len_prefixed_payload_by_one(frame: &mut Vec<u8>, payload: Range<usize>) {
        assert!(payload.len() > 1);
        let header_start = payload.start - 2;
        let new_len = u16::try_from(payload.len() - 1).expect("payload length fits u16");
        frame[header_start..payload.start].copy_from_slice(&new_len.to_be_bytes());
        frame.remove(payload.end - 1);
        frame.push(0);
    }

    fn overwrite_len_prefix(frame: &mut [u8], range: Range<usize>, replacement_len: usize) {
        assert_eq!(range.len(), 2);
        let replacement_len =
            u16::try_from(replacement_len).expect("test signature length must fit u16");
        frame[range].copy_from_slice(&replacement_len.to_be_bytes());
    }

    fn relay_authentication_len_ranges(frame: &[u8]) -> (Range<usize>, Range<usize>) {
        let mut offset = 1;
        skip_len_prefixed_payload(frame, &mut offset);
        skip_len_prefixed_payload(frame, &mut offset);
        skip_len_prefixed_payload(frame, &mut offset);
        skip_len_prefixed_payload(frame, &mut offset);
        skip_len_prefixed_payload(frame, &mut offset);
        skip_len_prefixed_payload(frame, &mut offset);
        skip_len_prefixed_payload(frame, &mut offset);
        match frame[0] {
            HYBRID_RELAY_RESPONSE_TYPE => {
                skip_len_prefixed_payload(frame, &mut offset);
                skip_len_prefixed_payload(frame, &mut offset);
            }
            PQFS_RELAY_RESPONSE_TYPE => {
                skip_len_prefixed_payload(frame, &mut offset);
                skip_len_prefixed_payload(frame, &mut offset);
                skip_len_prefixed_payload(frame, &mut offset);
                skip_len_prefixed_payload(frame, &mut offset);
                skip_len_prefixed_payload(frame, &mut offset);
                skip_len_prefixed_payload(frame, &mut offset);
                skip_len_prefixed_payload(frame, &mut offset);
            }
            other => panic!("unexpected relay response type {other:#04x}"),
        }
        let identity = len_prefixed_header_range(frame, &mut offset);
        let signature = len_prefixed_header_range(frame, &mut offset);
        (identity, signature)
    }

    #[test]
    fn ensure_nk3_negotiation_accepts_forward_secure_suite() {
        let Nk3Fixture {
            client_caps,
            relay_caps,
            kem_id,
            sig_id,
            ..
        } = build_nk3_fixture();

        ensure_nk3_negotiation(
            client_caps.as_slice(),
            relay_caps.as_slice(),
            kem_id,
            sig_id,
        )
        .expect("nk3 negotiation succeeds");
    }

    #[test]
    fn parse_client_hello_nk2_rejects_low_order_ephemeral_key() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng = StdRng::seed_from_u64(7301);
        let (mut client_hello, _state) =
            build_client_hello(&params, &mut rng).expect("build nk2 client hello");

        let range = client_hello_ephemeral_range(&client_hello);
        overwrite_noise_key(&mut client_hello, range, [0u8; NOISE_SECRET_LEN]);
        let err = parse_client_hello(&client_hello, params.resume_hash)
            .err()
            .expect("low-order client ephemeral key must be rejected");
        assert!(
            err.to_string()
                .contains("client ephemeral key must not be low-order")
        );
    }

    #[test]
    fn parse_client_hello_nk3_rejects_low_order_static_key() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng = StdRng::seed_from_u64(7302);
        let (mut client_hello, _state) =
            build_client_hello(&params, &mut rng).expect("build nk3 client hello");

        let range = client_hello_static_range(&client_hello);
        overwrite_noise_key(&mut client_hello, range, [0u8; NOISE_SECRET_LEN]);
        let err = parse_client_hello(&client_hello, params.resume_hash)
            .err()
            .expect("low-order client static key must be rejected");
        assert!(
            err.to_string()
                .contains("client static key must not be low-order")
        );
    }

    #[test]
    fn parse_hybrid_relay_response_rejects_low_order_ephemeral_key() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(7303);
        let mut rng_relay = StdRng::seed_from_u64(7304);
        let relay_keys = checked_random_keypair();
        let (client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("build nk2 client hello");
        let (mut relay_response, _relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("build nk2 relay response");

        let range = relay_response_ephemeral_range(&relay_response);
        overwrite_noise_key(&mut relay_response, range, [0u8; NOISE_SECRET_LEN]);
        let profile = kem_profile(params.kem_id).expect("kem profile");
        let err =
            parse_hybrid_relay_response(&relay_response, params.descriptor_commit, profile.suite())
                .err()
                .expect("low-order relay ephemeral key must be rejected");
        assert!(
            err.to_string()
                .contains("relay ephemeral key must not be low-order")
        );
    }

    #[test]
    fn parse_pqfs_relay_response_rejects_low_order_static_key() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(7305);
        let mut rng_relay = StdRng::seed_from_u64(7306);
        let relay_keys = checked_random_keypair();
        let (client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("build nk3 client hello");
        let (mut relay_response, _relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("build nk3 relay response");

        let range = relay_response_static_range(&relay_response);
        overwrite_noise_key(&mut relay_response, range, [0u8; NOISE_SECRET_LEN]);
        let profile = kem_profile(params.kem_id).expect("kem profile");
        let err =
            parse_pqfs_relay_response(&relay_response, params.descriptor_commit, profile.suite())
                .err()
                .expect("low-order relay static key must be rejected");
        assert!(
            err.to_string()
                .contains("relay static key must not be low-order")
        );
    }

    #[test]
    fn verify_capabilities_alignment_rejects_unadvertised_selected_ids() {
        let defaults = RuntimeParams::soranet_defaults();

        let err = verify_capabilities_alignment(
            2,
            defaults.sig_id,
            defaults.client_capabilities,
            defaults.relay_capabilities,
        )
        .expect_err("selected ML-KEM id must be advertised by both sides");
        match err {
            HarnessError::Downgrade {
                warnings,
                telemetry,
            } => {
                assert!(
                    warnings
                        .iter()
                        .any(|warning| warning.capability_type == CAPABILITY_PQKEM
                            && warning.message.contains("selected id 0x02")),
                    "missing selected KEM warning: {warnings:?}"
                );
                assert!(telemetry.is_some(), "downgrade should emit telemetry");
            }
            other => panic!("expected downgrade error, got {other:?}"),
        }

        let err = verify_capabilities_alignment(
            defaults.kem_id,
            2,
            defaults.client_capabilities,
            defaults.relay_capabilities,
        )
        .expect_err("selected signature id must be advertised by both sides");
        match err {
            HarnessError::Downgrade { warnings, .. } => assert!(
                warnings
                    .iter()
                    .any(|warning| warning.capability_type == CAPABILITY_PQSIG
                        && warning.message.contains("selected id 0x02")),
                "missing selected signature warning: {warnings:?}"
            ),
            other => panic!("expected downgrade error, got {other:?}"),
        }
    }

    #[test]
    fn parse_client_hello_rejects_malformed_padding() {
        let params = RuntimeParams::soranet_defaults();
        let mut rng = StdRng::seed_from_u64(7314);
        let (client_hello, _state) = build_client_hello(&params, &mut rng).expect("client hello");

        let mut nonzero_padding = client_hello.clone();
        let last = nonzero_padding
            .last_mut()
            .expect("client hello should contain padding");
        assert_eq!(*last, 0, "test fixture should end in zero padding");
        *last = 0xA5;
        let err = match parse_client_hello(&nonzero_padding, params.resume_hash) {
            Ok(_) => panic!("non-zero padding must be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("zero-filled"),
            "unexpected error: {err}"
        );

        let mut truncated_padding = client_hello;
        assert_eq!(
            truncated_padding.pop(),
            Some(0),
            "test fixture should remove one padding byte"
        );
        let err = match parse_client_hello(&truncated_padding, params.resume_hash) {
            Ok(_) => panic!("non-block-sized frame must be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("multiple of"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_relay_response_rejects_short_ed25519_signature() {
        let params = RuntimeParams::soranet_defaults();
        let mut rng_client = StdRng::seed_from_u64(7312);
        let mut rng_relay = StdRng::seed_from_u64(7313);
        let relay_keys = checked_random_keypair();

        let (client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("client hello");
        let (mut relay_response, _relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("relay response");

        let (_identity_range, ed25519_range) = relay_authentication_len_ranges(&relay_response);
        overwrite_len_prefix(
            &mut relay_response,
            ed25519_range,
            ED25519_SIGNATURE_LEN - 1,
        );

        let profile = kem_profile(params.kem_id).expect("kem profile");
        let err = match relay_response.first().copied() {
            Some(HYBRID_RELAY_RESPONSE_TYPE) => parse_hybrid_relay_response(
                &relay_response,
                params.descriptor_commit,
                profile.suite(),
            )
            .map(|_| ()),
            Some(PQFS_RELAY_RESPONSE_TYPE) => parse_pqfs_relay_response(
                &relay_response,
                params.descriptor_commit,
                profile.suite(),
            )
            .map(|_| ()),
            other => panic!("unexpected relay response type {other:?}"),
        }
        .expect_err("malformed Ed25519 signature length must be rejected");
        assert!(
            err.to_string().contains("Ed25519 signature"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_relay_response_rejects_all_zero_ed25519_signature() {
        let params = RuntimeParams::soranet_defaults();
        let mut rng_client = StdRng::seed_from_u64(7315);
        let mut rng_relay = StdRng::seed_from_u64(7316);
        let relay_keys = checked_random_keypair();

        let (client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("client hello");
        let (mut relay_response, _relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("relay response");

        let (_identity_header, ed25519_header) = relay_authentication_len_ranges(&relay_response);
        let mut offset = ed25519_header.start;
        let payload = len_prefixed_payload_range(&relay_response, &mut offset);
        relay_response[payload].fill(0);

        let profile = kem_profile(params.kem_id).expect("kem profile");
        let err = match relay_response.first().copied() {
            Some(HYBRID_RELAY_RESPONSE_TYPE) => parse_hybrid_relay_response(
                &relay_response,
                params.descriptor_commit,
                profile.suite(),
            )
            .map(|_| ()),
            Some(PQFS_RELAY_RESPONSE_TYPE) => parse_pqfs_relay_response(
                &relay_response,
                params.descriptor_commit,
                profile.suite(),
            )
            .map(|_| ()),
            other => panic!("unexpected relay response type {other:?}"),
        }
        .expect_err("all-zero Ed25519 signature material must be rejected");
        assert!(
            err.to_string().contains("Ed25519 signature") && err.to_string().contains("all zero"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn client_handle_relay_hello_rejects_tampered_relay_capabilities() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_required_flag(
            defaults.client_capabilities,
            CAPABILITY_PQKEM,
            defaults.kem_id,
            false,
        );
        let relay_caps = capabilities_with_required_flag(
            defaults.relay_capabilities,
            CAPABILITY_PQKEM,
            defaults.kem_id,
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(7314);
        let mut rng_relay = StdRng::seed_from_u64(7315);
        let relay_keys = checked_random_keypair();

        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("client hello");
        let (mut relay_response, _relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("relay response");

        let mutated_relay_caps =
            capabilities_with_replaced_id(&relay_caps, CAPABILITY_PQKEM, defaults.kem_id, 2);
        let range = relay_response_capabilities_range(&relay_response);
        assert_eq!(range.len(), mutated_relay_caps.len());
        relay_response[range].copy_from_slice(&mutated_relay_caps);

        let err = match client_handle_relay_hello(
            client_state,
            &relay_response,
            relay_keys.public_key(),
            &params,
            &mut rng_client,
        ) {
            Ok(_) => panic!("relay response missing selected KEM id must fail"),
            Err(err) => err,
        };
        assert!(
            matches!(err, HarnessError::Validation(ref message) if message.contains("signature verification failed")),
            "signed relay capabilities must fail authentication before negotiation: {err:?}"
        );
    }

    #[test]
    fn client_handle_relay_hello_rejects_wrong_directory_identity() {
        let (params, client_state, relay_hello, _relay_keys) =
            authenticated_exchange(8_001, 8_002, 0x41);
        let wrong_relay_keys = checked_seeded_keypair(0x42);
        let mut rng = StdRng::seed_from_u64(8_003);

        let err = client_handle_relay_hello(
            client_state,
            &relay_hello,
            wrong_relay_keys.public_key(),
            &params,
            &mut rng,
        )
        .err()
        .expect("an identity absent from the authenticated directory must fail");

        assert!(
            matches!(err, HarnessError::Validation(ref message) if message.contains("authenticated directory identity")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn client_handle_relay_hello_rejects_forged_nonzero_signature() {
        let (params, client_state, mut relay_hello, relay_keys) =
            authenticated_exchange(8_004, 8_005, 0x43);
        let (_identity_header, signature_header) = relay_authentication_len_ranges(&relay_hello);
        let mut offset = signature_header.start;
        let signature_payload = len_prefixed_payload_range(&relay_hello, &mut offset);
        relay_hello[signature_payload.start] ^= 0x80;
        assert!(
            relay_hello[signature_payload].iter().any(|byte| *byte != 0),
            "fixture must remain a nonzero forged signature"
        );
        let mut rng = StdRng::seed_from_u64(8_006);

        let err = client_handle_relay_hello(
            client_state,
            &relay_hello,
            relay_keys.public_key(),
            &params,
            &mut rng,
        )
        .err()
        .expect("a nonzero forged signature must fail");

        assert!(
            matches!(err, HarnessError::Validation(ref message) if message.contains("signature verification failed")),
            "unexpected error: {err:?}"
        );
    }

    fn assert_transport_authentication_mismatch(
        transport_alpn: &'static [u8],
        tls_server_name: &'static str,
    ) {
        let (params, client_state, relay_hello, relay_keys) =
            authenticated_exchange(8_007, 8_008, 0x44);
        let mismatched_params = RuntimeParams {
            transport_alpn,
            tls_server_name,
            ..params
        };
        let mut rng = StdRng::seed_from_u64(8_009);

        let err = client_handle_relay_hello(
            client_state,
            &relay_hello,
            relay_keys.public_key(),
            &mismatched_params,
            &mut rng,
        )
        .err()
        .expect("transport identity mismatch must fail relay authentication");

        assert!(
            matches!(err, HarnessError::Validation(ref message) if message.contains("signature verification failed")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn client_handle_relay_hello_binds_alpn_and_tls_server_name() {
        assert_transport_authentication_mismatch(b"soranet/other", DEFAULT_TLS_SERVER_NAME);
        assert_transport_authentication_mismatch(SORANET_QUIC_ALPN, "other.soranet.invalid");
    }

    #[test]
    fn client_handle_relay_hello_rejects_client_hello_substitution() {
        let (params, client_state, _relay_hello, _relay_keys) =
            authenticated_exchange(8_010, 8_011, 0x45);
        let (_other_params, _other_state, substituted_relay_hello, relay_keys) =
            authenticated_exchange(8_012, 8_013, 0x45);
        let mut rng = StdRng::seed_from_u64(8_014);

        let err = client_handle_relay_hello(
            client_state,
            &substituted_relay_hello,
            relay_keys.public_key(),
            &params,
            &mut rng,
        )
        .err()
        .expect("a response signed for a different client hello must fail");

        assert!(
            matches!(err, HarnessError::Validation(ref message) if message.contains("signature verification failed")),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn ensure_nk3_negotiation_detects_downgrade() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );

        let err = ensure_nk3_negotiation(
            client_caps.as_slice(),
            relay_caps.as_slice(),
            defaults.kem_id,
            defaults.sig_id,
        )
        .expect_err("nk3 downgrade should surface");
        assert!(matches!(err, HarnessError::Downgrade { .. }));
    }

    #[test]
    fn compute_nk3_transcript_matches_parsed_hash() {
        let Nk3Fixture {
            client_state,
            relay_response,
            client_caps,
            kem_id,
            sig_id,
            ..
        } = build_nk3_fixture();

        let transcript = compute_nk3_transcript(
            &relay_response,
            &client_state.client_nonce,
            client_caps.as_slice(),
            kem_id,
            sig_id,
            client_state.resume_hash.as_deref(),
        )
        .expect("nk3 transcript");
        assert_eq!(transcript, relay_response.transcript_hash);
    }

    #[test]
    fn decapsulate_nk3_secrets_validates_artifacts() {
        let Nk3Fixture {
            client_state,
            relay_response,
            suite,
            client_caps,
            kem_id,
            sig_id,
            ..
        } = build_nk3_fixture();

        let transcript = compute_nk3_transcript(
            &relay_response,
            &client_state.client_nonce,
            client_caps.as_slice(),
            kem_id,
            sig_id,
            client_state.resume_hash.as_deref(),
        )
        .expect("nk3 transcript");

        let forward_secret = client_state
            .forward_kem_secret
            .as_ref()
            .expect("forward secret present");
        let forward_public = client_state
            .forward_kem_public
            .as_ref()
            .expect("forward public present");

        let (primary_shared, forward_shared) = decapsulate_nk3_secrets(
            suite,
            &client_state.client_kem_secret,
            forward_secret,
            forward_public.as_slice(),
            &relay_response,
            &transcript,
        )
        .expect("nk3 decapsulation");

        let recomputed_dual_mix = compute_dual_mix(
            primary_shared.as_bytes(),
            forward_shared.as_bytes(),
            &transcript,
        );
        assert_eq!(recomputed_dual_mix, relay_response.dual_mix);
    }

    #[test]
    fn assemble_relay_state_populates_core_fields() {
        let params = RuntimeParams::soranet_defaults();
        let parsed = ClientHelloParsed {
            client_nonce: [0x01; 32],
            handshake_suite: HandshakeSuite::Nk2Hybrid,
            kem_id: 5,
            sig_id: 7,
            client_ephemeral_public: [0x02; 32],
            client_static_public: Some([0x03; 32]),
            client_kem_public: vec![0x04, 0x05],
            forward_kem_public: None,
            forward_commitment: None,
            client_capabilities: vec![0x06],
            resume_hash: Some(vec![0x07]),
        };

        let ephemeral_secret = StaticSecret::from([0x22; 32]);
        let expected_ephemeral_public = X25519PublicKey::from(&ephemeral_secret).to_bytes();
        let static_secret = StaticSecret::from([0x33; 32]);
        let expected_static_public = X25519PublicKey::from(&static_secret).to_bytes();
        let noise = RelayNoiseState {
            nonce: [0xAA; 32],
            ephemeral_secret,
            ephemeral_public: expected_ephemeral_public,
            static_secret,
            static_public: expected_static_public,
        };

        let kem = RuntimeKemArtifacts {
            relay_public: vec![0x10, 0x11],
            relay_secret: Zeroizing::new(vec![0x20]),
            shared_secret: Zeroizing::new(vec![0x30]),
            ciphertext: vec![0x40, 0x41],
        };

        let warning_message = "test warning propagated".to_owned();
        let relay_hello = vec![0xDE, 0xAD];
        let transcript = [0xBB; 32];

        let state = assemble_relay_state(RelayStateInputs {
            parsed,
            params: &params,
            noise,
            kem,
            transcript,
            handshake_suite: HandshakeSuite::Nk2Hybrid,
            warnings: vec![CapabilityWarning {
                capability_type: 0x0101,
                message: warning_message.clone(),
            }],
            relay_hello: relay_hello.clone(),
        });

        assert_eq!(state.client_nonce, [0x01; 32]);
        assert_eq!(state.client_ephemeral_public, [0x02; 32]);
        assert_eq!(state.client_capabilities, vec![0x06]);
        assert_eq!(state.client_kem_public, vec![0x04, 0x05]);
        assert_eq!(state.resume_hash.as_deref(), Some(&[0x07][..]));
        assert_eq!(state.client_static_public, Some([0x03; 32]));
        assert_eq!(state.relay_nonce, [0xAA; 32]);
        assert_eq!(state.relay_ephemeral_public, expected_ephemeral_public);
        assert_eq!(state.relay_static_public, expected_static_public);
        assert_eq!(state.relay_capabilities, params.relay_capabilities);
        assert_eq!(state.descriptor_commit, params.descriptor_commit);
        assert_eq!(state.relay_hello, relay_hello);
        assert_eq!(state.transcript_hash, transcript);
        assert_eq!(state.kem_id, 5);
        assert_eq!(state.sig_id, 7);
        assert_eq!(state.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert_eq!(state.warnings.len(), 1);
        assert_eq!(state.warnings[0].capability_type, 0x0101);
        assert_eq!(state.warnings[0].message, warning_message);
        assert!(state.pending_session.is_none());
        assert!(state.transcript_confirm_primary.is_none());
        assert!(state.transcript_confirm_forward.is_none());
        assert!(state.forward_kem_public.is_none());
        assert!(!state.requires_client_finish);
        assert_eq!(&**state.relay_kem_shared, &[0x30]);
        assert_eq!(&**state.relay_kem_secret, &[0x20]);
        assert_eq!(state.kem_ciphertext, vec![0x40, 0x41]);
    }

    #[test]
    fn pqfs_relay_response_serializes_inputs_and_signatures() {
        let params = RuntimeParams::soranet_defaults();

        let noise = fixture_noise_state();
        let primary = fixture_kem_artifacts(
            &[0x01, 0x02, 0x03],
            &[0x04, 0x05],
            &[0x06, 0x07, 0x08, 0x09],
            &[0x0A, 0x0B, 0x0C, 0x0D],
        );
        let forward = fixture_kem_artifacts(
            &[0x21, 0x22, 0x23],
            &[0x24, 0x25],
            &[0x26, 0x27, 0x28, 0x29],
            &[0x2A, 0x2B, 0x2C, 0x2D],
        );

        let client_commit: &[u8] = b"unit-client-commit";
        let (primary_confirmation, forward_confirmation) =
            (vec![0x31, 0x32, 0x33, 0x34], vec![0x35, 0x36, 0x37, 0x38]);
        let transcript = [0x40; 32];
        let (forward_commitment, dual_mix) = (
            vec![0x41, 0x42, 0x43, 0x44],
            vec![0x50, 0x51, 0x52, 0x53, 0x54],
        );

        let relay_keys = checked_random_keypair();
        let inputs = PqfsRelayResponseInputs {
            client_commit,
            params: &params,
            noise: &noise,
            primary: &primary,
            forward: &forward,
            primary_confirmation: primary_confirmation.as_slice(),
            forward_confirmation: forward_confirmation.as_slice(),
            transcript: &transcript,
            forward_commitment: forward_commitment.as_slice(),
            dual_mix: dual_mix.as_slice(),
            relay_identity_key: &relay_keys,
        };

        let response = build_pqfs_relay_response(&inputs).expect("build pqfs relay response");
        assert_eq!(response.len() % NOISE_PADDING_BLOCK, 0);

        let mut cursor = MessageCursor::new(&response);
        assert_eq!(
            cursor.read_u8().expect("response type"),
            PQFS_RELAY_RESPONSE_TYPE
        );
        let expected_segments: [(&str, &[u8]); 14] = [
            ("nonce segment", noise.nonce.as_ref()),
            ("ephemeral public", noise.ephemeral_public.as_ref()),
            ("static public", noise.static_public.as_ref()),
            ("relay capabilities", params.relay_capabilities),
            ("descriptor commit", params.descriptor_commit),
            ("primary relay public", primary.relay_public.as_slice()),
            ("primary ciphertext", primary.ciphertext.as_slice()),
            ("forward relay public", forward.relay_public.as_slice()),
            ("forward ciphertext", forward.ciphertext.as_slice()),
            ("primary confirmation", primary_confirmation.as_slice()),
            ("forward confirmation", forward_confirmation.as_slice()),
            ("transcript hash", transcript.as_ref()),
            ("forward commitment", forward_commitment.as_slice()),
            ("dual mix", dual_mix.as_slice()),
        ];

        for (label, expected) in expected_segments {
            assert_eq!(
                cursor.read_len_prefixed().expect(label),
                expected,
                "{label} mismatch"
            );
        }

        let body_len = response.len() - cursor.remaining_slice().len();
        let relay_body = &response[..body_len];

        let relay_identity = cursor.read_len_prefixed().expect("relay identity");
        assert_eq!(relay_identity, relay_identity_bytes(&relay_keys).unwrap());
        let relay_signature = cursor.read_len_prefixed().expect("relay signature");
        verify_relay_authentication(
            HandshakeSuite::Nk3PqForwardSecure,
            client_commit,
            relay_body,
            &transcript,
            relay_identity,
            relay_signature,
            relay_keys.public_key(),
            params.transport_alpn,
            params.tls_server_name,
        )
        .expect("relay response signature verifies");

        let padding = cursor.remaining_slice();
        assert!(
            !padding.is_empty(),
            "noise padding should extend the response to the block size"
        );
        assert!(
            padding.iter().all(|&byte| byte == 0),
            "noise padding must be zeroed"
        );
    }

    #[test]
    fn parse_and_diff_capabilities_handles_required_flag() {
        let client = decode_hex("010100020101") // type 0x0101, required flag set, value 0x01
            .expect("client hex");
        let relay = Vec::new(); // relay omits the required capability
        let client_caps = parse_capabilities(&client).expect("parse client");
        let relay_caps = parse_capabilities(&relay).expect("parse relay");
        assert!(client_caps[0].required);
        let warnings = diff_capabilities(&client_caps, &relay_caps);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].capability_type, 0x0101);
        assert!(warnings[0].message.contains("0x0101"));
    }

    #[test]
    fn encode_capabilities_sets_required_flag_in_flags_byte() {
        let entries = vec![CapabilityTlv {
            ty: CAPABILITY_PQKEM,
            value: vec![0x01, 0x00],
            required: true,
        }];
        let encoded = encode_capabilities(&entries).expect("encode capabilities");
        let expected = decode_hex("010100020101").expect("expected hex");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn transcript_hash_matches_reference() {
        let descriptor_commit = [0x11u8; 32];
        let client_nonce = [0x22u8; 32];
        let relay_nonce = [0x33u8; 32];
        let caps = [0u8; 8];
        let inputs = TranscriptInputs {
            descriptor_commit: &descriptor_commit,
            client_nonce: &client_nonce,
            relay_nonce: &relay_nonce,
            capability_bytes: &caps,
            kem_id: 1,
            sig_id: 1,
            handshake_suite: HandshakeSuite::Nk2Hybrid,
            resume_hash: None,
        };
        let hash = inputs.compute_hash().expect("transcript hash");
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn transcript_capability_length_rejects_u32_overflow() {
        let err = transcript_capability_len_bytes(u32::MAX as usize + 1)
            .expect_err("oversized transcript capability vector must fail");
        match err {
            HarnessError::Validation(message) => {
                assert!(
                    message.contains("capability vector length"),
                    "unexpected message: {message}"
                );
                assert!(
                    message.contains("exceeds u32::MAX"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn suite_negotiation_prefers_highest_common_suite() {
        let client = suite_list_tlv(
            false,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
        );
        let relay = suite_list_tlv(false, &[HandshakeSuite::Nk3PqForwardSecure]);
        let client_caps = parse_capabilities(&client).expect("client");
        let relay_caps = parse_capabilities(&relay).expect("relay");
        let negotiation = negotiate_handshake_suite(&client_caps, &relay_caps).expect("negotiate");
        assert_eq!(negotiation.selected, HandshakeSuite::Nk3PqForwardSecure);
        assert!(negotiation.warnings.is_empty());
    }

    #[test]
    fn suite_negotiation_warns_on_downgrade() {
        let client = suite_list_tlv(
            true,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
        );
        let relay = suite_list_tlv(false, &[HandshakeSuite::Nk2Hybrid]);
        let client_caps = parse_capabilities(&client).expect("client");
        let relay_caps = parse_capabilities(&relay).expect("relay");
        let negotiation = negotiate_handshake_suite(&client_caps, &relay_caps).expect("negotiate");
        assert_eq!(negotiation.selected, HandshakeSuite::Nk2Hybrid);
        assert!(
            negotiation
                .warnings
                .iter()
                .any(|warn| warn.message.contains("client preferred")),
            "expected downgrade warning when preference not met"
        );
    }

    #[test]
    fn suite_negotiation_errors_without_overlap() {
        let client = suite_list_tlv(false, &[HandshakeSuite::Nk3PqForwardSecure]);
        let relay = suite_list_tlv(false, &[HandshakeSuite::Nk2Hybrid]);
        let client_caps = parse_capabilities(&client).expect("client");
        let relay_caps = parse_capabilities(&relay).expect("relay");
        let err = negotiate_handshake_suite(&client_caps, &relay_caps)
            .expect_err("expected downgrade error when suites do not overlap");
        match err {
            HarnessError::Downgrade { warnings, .. } => {
                assert!(
                    warnings
                        .iter()
                        .any(|warn| warn.message.contains("no overlapping handshake suite")),
                    "expected warning describing missing overlap"
                );
            }
            other => panic!("expected downgrade error, got {other:?}"),
        }
    }

    #[test]
    fn suite_negotiation_rejects_old_ids_even_when_first_release_ids_present() {
        let client_caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![
                0x02,
                u8::from(HandshakeSuite::Nk3PqForwardSecure),
                u8::from(HandshakeSuite::Nk2Hybrid),
            ],
            required: true,
        }];
        let relay_caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![0x03, u8::from(HandshakeSuite::Nk2Hybrid)],
            required: true,
        }];

        let err = negotiate_handshake_suite(&client_caps, &relay_caps)
            .expect_err("suite lists containing old IDs must fail");
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("pre-release handshake suite identifiers"));
                assert!(message.contains("0x02"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn suite_negotiation_rejects_pre_release_only_suite_lists() {
        let client_caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![0x02, 0x03],
            required: true,
        }];
        let relay_caps = vec![CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![0x02, 0x03],
            required: true,
        }];

        let err = negotiate_handshake_suite(&client_caps, &relay_caps)
            .expect_err("old-only suite lists must fail");
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("pre-release handshake suite identifiers"));
                assert!(message.contains("0x02"));
                assert!(message.contains("0x03"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn suite_negotiation_errors_when_relay_omits_capability() {
        let client = suite_list_tlv(
            false,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
        );
        let client_caps = parse_capabilities(&client).expect("client");
        let relay_caps = parse_capabilities(&[]).expect("relay");
        let err = negotiate_handshake_suite(&client_caps, &relay_caps)
            .expect_err("expected downgrade error when relay omits suite list");
        assert!(matches!(err, HarnessError::Downgrade { .. }));
    }

    #[test]
    fn salt_announcement_roundtrip() {
        let salt = [0x55u8; 32];
        let json = salt_announcement_json(&SaltAnnouncementParams {
            epoch_id: 7,
            previous_epoch: Some(6),
            valid_after: "2026-01-01T00:00:00Z",
            valid_until: "2026-01-02T00:00:00Z",
            blinded_cid_salt: &salt,
            emergency_rotation: false,
            notes: Some("test"),
            signature: None,
        })
        .expect("json");
        let value: Value = norito::json::from_str(&json).expect("json parse");
        assert_eq!(value["epoch_id"].as_u64(), Some(7));
        assert_eq!(value["previous_epoch"].as_u64(), Some(6));
        assert_eq!(
            value["blinded_cid_salt_hex"],
            Value::from(hex::encode(salt))
        );
        assert_eq!(value["signature"], Value::Null);
    }

    #[test]
    fn alarm_report_renders_expected_fields() {
        let json = alarm_report_json(&AlarmReport {
            relay_id: "relay",
            relay_role: 1,
            capability_type: 0x0101,
            reason_code: "MissingRequiredKEM",
            transcript_hash_hex: "deadbeef",
            observed_at_unix_ms: 1_785_120_000_000,
            circuit_id: 42,
            counter: 2,
            capability_value_hex: None,
            signature: Some("dilithium3:base64signature"),
            witness_signature: Some("ed25519:base64signature"),
        })
        .expect("alarm json");
        assert!(json.contains(r#""relay_id": "relay""#));
        assert!(json.contains("MissingRequiredKEM"));
        assert!(json.contains("dilithium3"));
        assert!(json.contains("ed25519"));
    }

    #[test]
    fn runtime_handshake_roundtrip_produces_matching_session_keys() {
        let params = RuntimeParams::soranet_defaults();
        let mut rng_client = StdRng::seed_from_u64(1);
        let mut rng_relay = StdRng::seed_from_u64(2);
        let relay_keys = checked_random_keypair();

        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("client hello");
        let (relay_hello, relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("relay hello");
        let (client_finish, client_secrets) = client_handle_relay_hello(
            client_state,
            &relay_hello,
            relay_keys.public_key(),
            &params,
            &mut rng_client,
        )
        .expect("client finish");
        let relay_secrets = match client_finish {
            Some(frame) => {
                relay_finalize_handshake(relay_state, &frame, &relay_keys).expect("relay finish")
            }
            None => relay_finalize_handshake(relay_state, &[], &relay_keys).expect("relay finish"),
        };

        assert_eq!(
            client_secrets.handshake_suite,
            relay_secrets.handshake_suite
        );
        assert_eq!(
            client_secrets.session_key.payload(),
            relay_secrets.session_key.payload()
        );
        assert_eq!(
            client_secrets.transcript_hash,
            relay_secrets.transcript_hash
        );
    }

    #[test]
    fn build_client_hello_supports_nk2_preference() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng = StdRng::seed_from_u64(99);
        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng).expect("nk2 client init");
        assert_eq!(
            client_hello.first().copied(),
            Some(HYBRID_CLIENT_HELLO_TYPE)
        );
        assert_eq!(client_state.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert!(client_state.forward_kem_public.is_none());
        assert!(client_state.forward_kem_secret.is_none());
    }

    #[test]
    fn process_client_hello_rejects_pre_release_nk2_suite_byte_before_relay_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(9102);
        let relay_keys = checked_random_keypair();
        let (mut client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk2 client");
        let suite_idx = client_hello_suite_byte_index(&client_hello);
        assert_eq!(client_hello[suite_idx], u8::from(HandshakeSuite::Nk2Hybrid));
        client_hello[suite_idx] = 0x02;

        let err = match process_client_hello(&client_hello, &params, &relay_keys, &mut PanicRng) {
            Ok(_) => panic!("pre-release NK2 suite id must fail before relay RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("unsupported handshake suite identifier 0x02"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn process_client_hello_handles_nk2_suite() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };

        let relay_keys = checked_random_keypair();
        let mut rng_client = StdRng::seed_from_u64(100);
        let mut rng_relay = StdRng::seed_from_u64(200);

        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk2 client");
        let (relay_hello, relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("nk2 relay");

        assert_eq!(relay_state.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert!(!relay_state.requires_client_finish);
        assert_eq!(
            relay_hello.first().copied(),
            Some(HYBRID_RELAY_RESPONSE_TYPE)
        );

        let (client_finish, client_secrets) = client_handle_relay_hello(
            client_state,
            &relay_hello,
            relay_keys.public_key(),
            &params,
            &mut rng_client,
        )
        .expect("nk2 client handle");
        assert!(
            client_finish.is_none(),
            "nk2 handshake must not emit client finish"
        );
        let relay_secrets =
            relay_finalize_handshake(relay_state, &[], &relay_keys).expect("nk2 relay finalize");

        assert_eq!(client_secrets.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert_eq!(relay_secrets.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert_eq!(
            client_secrets.session_key.payload(),
            relay_secrets.session_key.payload()
        );
        assert_eq!(
            client_secrets.transcript_hash,
            relay_secrets.transcript_hash
        );
    }

    #[test]
    fn process_client_hello_rejects_malformed_nk2_kem_before_relay_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(6102);
        let relay_keys = checked_random_keypair();
        let (mut client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk2 client");
        let primary_range = client_hello_primary_kem_range(&client_hello);
        shorten_len_prefixed_payload_by_one(&mut client_hello, primary_range);

        let err = match process_client_hello(&client_hello, &params, &relay_keys, &mut PanicRng) {
            Ok(_) => panic!("malformed primary KEM key must fail before relay RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Kem(message) => assert!(
                message.contains("client ML-KEM public key"),
                "unexpected error: {message}"
            ),
            other => panic!("expected KEM preflight error, got {other:?}"),
        }
    }

    #[test]
    fn process_client_hello_rejects_all_zero_nk2_kem_before_relay_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(6104);
        let relay_keys = checked_random_keypair();
        let (mut client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk2 client");
        let primary_range = client_hello_primary_kem_range(&client_hello);
        client_hello[primary_range].fill(0);

        let err = match process_client_hello(&client_hello, &params, &relay_keys, &mut PanicRng) {
            Ok(_) => panic!("all-zero primary KEM key must fail before relay RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Kem(message) => {
                assert!(message.contains("client ML-KEM public key"));
                assert!(message.contains("all zero"));
            }
            other => panic!("expected KEM preflight error, got {other:?}"),
        }
    }

    #[test]
    fn build_client_hello_supports_nk3_preference() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng = StdRng::seed_from_u64(1234);
        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng).expect("nk3 client init");
        assert_eq!(client_hello.first().copied(), Some(PQFS_CLIENT_COMMIT_TYPE));
        assert_eq!(
            client_state.handshake_suite,
            HandshakeSuite::Nk3PqForwardSecure
        );
        assert!(client_state.forward_kem_public.is_some());
        assert!(client_state.forward_kem_secret.is_some());
    }

    #[test]
    fn process_client_hello_rejects_pre_release_nk3_suite_byte_before_relay_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(9103);
        let relay_keys = checked_random_keypair();
        let (mut client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk3 client");
        let suite_idx = client_hello_suite_byte_index(&client_hello);
        assert_eq!(
            client_hello[suite_idx],
            u8::from(HandshakeSuite::Nk3PqForwardSecure)
        );
        client_hello[suite_idx] = 0x03;

        let err = match process_client_hello(&client_hello, &params, &relay_keys, &mut PanicRng) {
            Ok(_) => panic!("pre-release NK3 suite id must fail before relay RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("unsupported handshake suite identifier 0x03"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn process_client_hello_handles_nk3_suite() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };

        let relay_keys = checked_random_keypair();
        let mut rng_client = StdRng::seed_from_u64(2024);
        let mut rng_relay = StdRng::seed_from_u64(4048);

        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk3 client");
        let (relay_hello, relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("nk3 relay");

        assert_eq!(
            relay_state.handshake_suite,
            HandshakeSuite::Nk3PqForwardSecure
        );
        assert!(!relay_state.requires_client_finish);
        assert_eq!(relay_hello.first().copied(), Some(PQFS_RELAY_RESPONSE_TYPE));

        let profile = kem_profile(params.kem_id).expect("kem profile");
        parse_pqfs_relay_response(&relay_hello, params.descriptor_commit, profile.suite())
            .expect("parse nk3 response");

        let (client_finish, client_secrets) = client_handle_relay_hello(
            client_state,
            &relay_hello,
            relay_keys.public_key(),
            &params,
            &mut rng_client,
        )
        .expect("nk3 client handle");
        assert!(
            client_finish.is_none(),
            "nk3 handshake must not emit client finish"
        );
        let relay_secrets =
            relay_finalize_handshake(relay_state, &[], &relay_keys).expect("nk3 relay finalize");

        assert_eq!(
            client_secrets.handshake_suite,
            HandshakeSuite::Nk3PqForwardSecure
        );
        assert_eq!(
            relay_secrets.handshake_suite,
            HandshakeSuite::Nk3PqForwardSecure
        );
        assert_eq!(
            client_secrets.session_key.payload(),
            relay_secrets.session_key.payload()
        );
        assert_eq!(
            client_secrets.transcript_hash,
            relay_secrets.transcript_hash
        );
    }

    #[test]
    fn process_client_hello_rejects_malformed_nk3_forward_kem_before_relay_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(6103);
        let relay_keys = checked_random_keypair();
        let (mut client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk3 client");
        let forward_range = client_hello_forward_kem_range(&client_hello);
        shorten_len_prefixed_payload_by_one(&mut client_hello, forward_range);

        let err = match process_client_hello(&client_hello, &params, &relay_keys, &mut PanicRng) {
            Ok(_) => panic!("malformed forward KEM key must fail before relay RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Kem(message) => assert!(
                message.contains("forward ML-KEM public key"),
                "unexpected error: {message}"
            ),
            other => panic!("expected KEM preflight error, got {other:?}"),
        }
    }

    #[test]
    fn process_client_hello_rejects_all_zero_nk3_forward_kem_before_relay_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(6105);
        let relay_keys = checked_random_keypair();
        let (mut client_hello, _client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk3 client");
        let forward_range = client_hello_forward_kem_range(&client_hello);
        client_hello[forward_range].fill(0);

        let err = match process_client_hello(&client_hello, &params, &relay_keys, &mut PanicRng) {
            Ok(_) => panic!("all-zero forward KEM key must fail before relay RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Kem(message) => {
                assert!(message.contains("forward ML-KEM public key"));
                assert!(message.contains("all zero"));
            }
            other => panic!("expected KEM preflight error, got {other:?}"),
        }
    }

    #[test]
    fn runtime_handshake_roundtrip_supports_mlkem1024() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_capabilities =
            capabilities_with_selected_id(defaults.client_capabilities, CAPABILITY_PQKEM, 2, true);
        let relay_capabilities =
            capabilities_with_selected_id(defaults.relay_capabilities, CAPABILITY_PQKEM, 2, true);
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_capabilities.as_slice(),
            relay_capabilities: relay_capabilities.as_slice(),
            kem_id: 2,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };
        let mut rng_client = StdRng::seed_from_u64(11);
        let mut rng_relay = StdRng::seed_from_u64(12);
        let relay_keys = checked_random_keypair();

        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("client hello");
        let (relay_hello, relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("relay hello");
        let (client_finish, client_secrets) = client_handle_relay_hello(
            client_state,
            &relay_hello,
            relay_keys.public_key(),
            &params,
            &mut rng_client,
        )
        .expect("client finish");
        let relay_secrets = match client_finish {
            Some(frame) => {
                relay_finalize_handshake(relay_state, &frame, &relay_keys).expect("relay")
            }
            None => relay_finalize_handshake(relay_state, &[], &relay_keys).expect("relay"),
        };

        assert_eq!(
            client_secrets.handshake_suite,
            relay_secrets.handshake_suite
        );
        assert_eq!(
            client_secrets.session_key.payload(),
            relay_secrets.session_key.payload()
        );
        assert_eq!(
            client_secrets.transcript_hash,
            relay_secrets.transcript_hash
        );
    }

    #[test]
    fn build_client_hello_rejects_unknown_kem() {
        let mut params = RuntimeParams::soranet_defaults();
        params.kem_id = 0xFF;
        let mut rng = StdRng::seed_from_u64(21);
        let err = match build_client_hello(&params, &mut rng) {
            Ok(_) => panic!("expected invalid KEM id to produce error"),
            Err(err) => err,
        };
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("ML-KEM"), "unexpected message: {message}");
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn build_client_hello_reports_rng_failure() {
        let params = RuntimeParams::soranet_defaults();
        let mut rng = FailingTryRng;

        let err = match build_client_hello(&params, &mut rng) {
            Ok(_) => panic!("expected client RNG failure"),
            Err(err) => err,
        };

        match err {
            HarnessError::RandomBytes { operation, message } => {
                assert_eq!(operation, "building client hello nonce");
                assert!(
                    message.contains("failing SoraNet handshake RNG"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn build_client_hello_rejects_all_zero_nonce_material() {
        let params = RuntimeParams::soranet_defaults();
        let mut rng = FixedTryRng { byte: 0 };

        let err = match build_client_hello(&params, &mut rng) {
            Ok(_) => panic!("expected all-zero client nonce failure"),
            Err(err) => err,
        };

        match err {
            HarnessError::RandomBytes { operation, message } => {
                assert_eq!(operation, "building client hello nonce");
                assert_eq!(message, "rng returned all-zero material");
            }
            other => panic!("expected all-zero client nonce RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn process_client_hello_reports_relay_rng_failure() {
        let params = RuntimeParams::soranet_defaults();
        let mut client_rng = StdRng::seed_from_u64(22);
        let (client_hello, _client_state) =
            build_client_hello(&params, &mut client_rng).expect("client hello");
        let relay_keys = checked_random_keypair();
        let mut relay_rng = FailingTryRng;

        let err = match process_client_hello(&client_hello, &params, &relay_keys, &mut relay_rng) {
            Ok(_) => panic!("expected relay RNG failure"),
            Err(err) => err,
        };

        match err {
            HarnessError::RandomBytes { operation, message } => {
                assert_eq!(operation, "building relay nonce");
                assert!(
                    message.contains("failing SoraNet handshake RNG"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("expected RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn process_client_hello_rejects_all_zero_relay_nonce_material() {
        let params = RuntimeParams::soranet_defaults();
        let mut client_rng = StdRng::seed_from_u64(23);
        let (client_hello, _client_state) =
            build_client_hello(&params, &mut client_rng).expect("client hello");
        let relay_keys = checked_random_keypair();
        let mut relay_rng = FixedTryRng { byte: 0 };

        let err = match process_client_hello(&client_hello, &params, &relay_keys, &mut relay_rng) {
            Ok(_) => panic!("expected all-zero relay nonce failure"),
            Err(err) => err,
        };

        match err {
            HarnessError::RandomBytes { operation, message } => {
                assert_eq!(operation, "building relay nonce");
                assert_eq!(message, "rng returned all-zero material");
            }
            other => panic!("expected all-zero relay nonce RNG failure, got {other:?}"),
        }
    }

    #[test]
    fn build_client_hello_rejects_short_descriptor_before_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let short_descriptor = [0x44; TRANSCRIPT_BINDING_LEN - 1];
        let params = RuntimeParams {
            descriptor_commit: &short_descriptor,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: defaults.relay_capabilities,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };

        let err = match build_client_hello(&params, &mut PanicRng) {
            Ok(_) => panic!("short descriptor must fail before client RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Validation(message) => assert!(
                message.contains("descriptor commitment"),
                "unexpected message: {message}"
            ),
            other => panic!("expected descriptor validation error, got {other:?}"),
        }
    }

    #[test]
    fn build_client_hello_rejects_short_resume_hash_before_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let short_resume = [0x55; TRANSCRIPT_BINDING_LEN - 1];
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: defaults.relay_capabilities,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: Some(&short_resume),
        };

        let err = match build_client_hello(&params, &mut PanicRng) {
            Ok(_) => panic!("short resume hash must fail before client RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Validation(message) => assert!(
                message.contains("resume hash"),
                "unexpected message: {message}"
            ),
            other => panic!("expected resume validation error, got {other:?}"),
        }
    }

    #[test]
    fn build_client_hello_rejects_oversized_capabilities_before_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let mut oversized_client_capabilities = defaults.client_capabilities.to_vec();
        while u16::try_from(oversized_client_capabilities.len()).is_ok() {
            oversized_client_capabilities.extend_from_slice(&CAPABILITY_PQSIG.to_be_bytes());
            oversized_client_capabilities.extend_from_slice(&2_u16.to_be_bytes());
            oversized_client_capabilities.extend_from_slice(&[0x01, 0x00]);
        }
        parse_capabilities(&oversized_client_capabilities)
            .expect("oversized capability stream remains structurally valid");
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: &oversized_client_capabilities,
            relay_capabilities: defaults.relay_capabilities,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };

        let err = match build_client_hello(&params, &mut PanicRng) {
            Ok(_) => panic!("oversized capabilities must fail before client RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Validation(message) => assert!(
                message.contains("client capability vector")
                    && message.contains("exceeds u16::MAX"),
                "unexpected message: {message}"
            ),
            other => panic!("expected capability validation error, got {other:?}"),
        }
    }

    #[test]
    fn process_client_hello_errors_on_missing_capability() {
        let defaults = RuntimeParams::soranet_defaults();
        let bad_relay_caps = hex_literal::hex!(
            "0102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f02010001010202000200047f12000412345678"
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: &bad_relay_caps,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };

        let mut rng_client = StdRng::seed_from_u64(3);
        let (client_hello, _client_state) =
            build_client_hello(&defaults, &mut rng_client).expect("client hello");
        let relay_keys = checked_random_keypair();
        let mut rng_relay = StdRng::seed_from_u64(4);

        match process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay) {
            Err(HarnessError::Downgrade {
                warnings,
                telemetry,
            }) => {
                assert!(!warnings.is_empty(), "expected downgrade warnings");
                assert!(
                    telemetry.is_some(),
                    "downgrade should emit telemetry payload"
                );
            }
            Err(err) => panic!("expected downgrade error, got {err:?}"),
            Ok(_) => panic!("expected downgrade error, got Ok"),
        }
    }

    #[test]
    fn process_client_hello_rejects_short_resume_hash_before_relay_rng() {
        let defaults = RuntimeParams::soranet_defaults();
        let mut rng_client = StdRng::seed_from_u64(7320);
        let (client_hello, _client_state) =
            build_client_hello(&defaults, &mut rng_client).expect("client hello");
        let short_resume = [0x66; TRANSCRIPT_BINDING_LEN - 1];
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: defaults.relay_capabilities,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: Some(&short_resume),
        };
        let relay_keys = checked_random_keypair();

        let err = match process_client_hello(&client_hello, &params, &relay_keys, &mut PanicRng) {
            Ok(_) => panic!("short expected resume hash must fail before relay RNG"),
            Err(err) => err,
        };
        match err {
            HarnessError::Validation(message) => assert!(
                message.contains("resume hash"),
                "unexpected message: {message}"
            ),
            other => panic!("expected resume validation error, got {other:?}"),
        }
    }

    #[test]
    fn process_client_hello_reports_constant_rate_warning() {
        let mut client_params = RuntimeParams::soranet_defaults();
        let mut client_entries =
            parse_capabilities(client_params.client_capabilities).expect("parse defaults");
        client_entries.push(CapabilityTlv {
            ty: CAPABILITY_CONSTANT_RATE,
            value: vec![0x01, 0x01],
            required: true,
        });
        client_entries.sort_by_key(|cap| cap.ty);
        let client_cap_bytes = encode_capabilities(&client_entries).expect("encode client caps");
        client_params.client_capabilities = &client_cap_bytes;
        let mut expected_relay_entries =
            parse_capabilities(client_params.relay_capabilities).expect("parse relay defaults");
        expected_relay_entries.push(CapabilityTlv {
            ty: CAPABILITY_CONSTANT_RATE,
            value: vec![0x01, 0x01],
            required: true,
        });
        expected_relay_entries.sort_by_key(|cap| cap.ty);
        let expected_relay_caps =
            encode_capabilities(&expected_relay_entries).expect("encode relay caps");
        client_params.relay_capabilities = &expected_relay_caps;

        let mut rng_client = StdRng::seed_from_u64(31);
        let (client_hello, _client_state) =
            build_client_hello(&client_params, &mut rng_client).expect("client hello");

        let relay_keys = checked_random_keypair();
        let mut rng_relay = StdRng::seed_from_u64(32);
        let relay_params = RuntimeParams::soranet_defaults();

        match process_client_hello(&client_hello, &relay_params, &relay_keys, &mut rng_relay) {
            Err(HarnessError::Downgrade { warnings, .. }) => {
                assert!(
                    warnings
                        .iter()
                        .any(|warn| warn.capability_type == CAPABILITY_CONSTANT_RATE
                            && warn.message.contains("snnet.constant_rate")),
                    "expected downgrade warning mentioning snnet.constant_rate: {warnings:?}"
                );
            }
            Err(err) => panic!("expected downgrade error, got {err:?}"),
            Ok(_) => panic!("expected downgrade when constant-rate is missing"),
        }
    }

    #[test]
    fn client_handle_relay_hello_rejects_descriptor_commit_mismatch() {
        let defaults = RuntimeParams::soranet_defaults();
        let mut rng_client = StdRng::seed_from_u64(5);
        let (client_hello, client_state) =
            build_client_hello(&defaults, &mut rng_client).expect("client hello");

        let mut altered_descriptor = *defaults
            .descriptor_commit
            .first()
            .expect("non-empty descriptor");
        altered_descriptor ^= 0xFF;
        let mut bad_descriptor = defaults.descriptor_commit.to_vec();
        bad_descriptor[0] = altered_descriptor;

        let bad_params = RuntimeParams {
            descriptor_commit: &bad_descriptor,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: defaults.relay_capabilities,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            transport_alpn: defaults.transport_alpn,
            tls_server_name: defaults.tls_server_name,
            resume_hash: defaults.resume_hash,
        };

        let relay_keys = checked_random_keypair();
        let mut rng_relay = StdRng::seed_from_u64(6);
        let (relay_hello, _relay_state) =
            process_client_hello(&client_hello, &bad_params, &relay_keys, &mut rng_relay)
                .expect("relay hello");

        match client_handle_relay_hello(
            client_state,
            &relay_hello,
            relay_keys.public_key(),
            &defaults,
            &mut rng_client,
        ) {
            Err(HarnessError::Validation(message)) => {
                assert!(
                    message.contains("descriptor"),
                    "expected descriptor mismatch message, got {message}"
                );
            }
            Err(err) => panic!("expected descriptor mismatch, got {err:?}"),
            Ok(_) => panic!("expected descriptor mismatch, got Ok"),
        }
    }

    include!("handshake/resumption_and_simulation_tests.rs");

    include!("handshake/forward_secure_tail_tests.rs");
}
