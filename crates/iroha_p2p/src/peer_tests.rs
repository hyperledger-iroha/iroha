// Peer transport regressions are included at module scope to preserve private-item access.
#[cfg(test)]
mod tests {
    use std::{
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
    };

    use iroha_crypto::{Algorithm, KeyPair, Signature, encryption::ChaCha20Poly1305};
    use iroha_primitives::addr::SocketAddr;
    use norito::codec::{DecodeAll, Encode};
    use tokio::io::AsyncWrite;

    use super::{Connection, SoranetHandshakeConfig, cryptographer::Cryptographer, state::*};
    use crate::{ConfidentialHandshakeCaps, ConsensusConfigCaps, RelayRole};

    fn sample_consensus_config_caps() -> ConsensusConfigCaps {
        ConsensusConfigCaps {
            execution_policy_hash: [0xB4; 32],
            nexus_policy_digest: [0xA5; 32],
            v2_config_fingerprint: [0xC3; 32],
            ivm_gas_schedule_hash: [0xE7; 32],
        }
    }

    #[test]
    fn consensus_config_mismatch_rejects_execution_policy_drift() {
        let expected = sample_consensus_config_caps();
        let mut got = expected;
        got.execution_policy_hash[0] ^= 1;

        let reason = consensus_config_mismatch(&expected, &got)
            .expect("one-bit execution-policy drift must fail the handshake");
        assert!(reason.starts_with("execution_policy_hash mismatch"));
    }

    #[test]
    fn consensus_config_mismatch_rejects_nexus_policy_digest_drift() {
        let expected = sample_consensus_config_caps();
        let mut got = expected;
        got.nexus_policy_digest[0] ^= 1;

        let reason = consensus_config_mismatch(&expected, &got)
            .expect("one-bit Nexus policy drift must fail the handshake");
        assert!(reason.starts_with("nexus_policy_digest mismatch"));
    }

    #[test]
    fn consensus_config_mismatch_rejects_ivm_gas_schedule_drift() {
        let expected = sample_consensus_config_caps();
        let mut got = expected;
        got.ivm_gas_schedule_hash[0] ^= 1;

        let reason = consensus_config_mismatch(&expected, &got)
            .expect("one-bit IVM gas-schedule drift must fail the handshake");
        assert!(reason.starts_with("ivm_gas_schedule_hash mismatch"));
        assert!(reason.contains(&hex_bytes(&expected.ivm_gas_schedule_hash)));
        assert!(reason.contains(&hex_bytes(&got.ivm_gas_schedule_hash)));
    }

    struct TrackingWrite {
        buffer: Vec<u8>,
        flushes: usize,
    }

    impl TrackingWrite {
        fn new() -> Self {
            Self {
                buffer: Vec::new(),
                flushes: 0,
            }
        }
    }

    impl AsyncWrite for TrackingWrite {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.buffer.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            self.flushes = self.flushes.saturating_add(1);
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    fn unsigned_handshake_hello(key_pair: &KeyPair, addr: SocketAddr) -> HandshakeHelloV1 {
        let (algorithm, public_key) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid");
        HandshakeHelloV1 {
            algorithm,
            public_key: public_key.to_vec(),
            signature: Vec::new(),
            addr,
            relay: RelayRole::Disabled,
            consensus: HandshakeConsensusMeta {
                mode: None,
                proto_version: None,
                consensus_fingerprint: None,
                config: None,
            },
            confidential: HandshakeConfidentialMeta {
                enabled: None,
                assume_valid: None,
                verifier_backend: None,
                features: None,
            },
            crypto: HandshakeCryptoMeta {
                sm_enabled: None,
                sm_openssl_preview: None,
            },
            trust: HandshakeTrustMeta {
                trust_gossip: true,
                scion_supported: false,
            },
        }
    }

    async fn read_crafted_handshake_hello(
        hello: HandshakeHelloV1,
        cryptographer: Cryptographer<ChaCha20Poly1305>,
    ) -> Result<Ready<ChaCha20Poly1305>, crate::Error> {
        use tokio::io::AsyncWriteExt;

        let encoded =
            encode_handshake_message(&cryptographer, &hello).expect("encode crafted hello");
        let hello_len = u16::try_from(encoded.len()).expect("crafted hello fits handshake frame");

        let (stream_a, stream_b) = tokio::io::duplex(encoded.len() + 2);
        let (_sender_read, mut sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);
        sender_write
            .write_u16(hello_len)
            .await
            .expect("write hello length");
        sender_write
            .write_all(&encoded)
            .await
            .expect("write hello bytes");

        let get_key = GetKey::<ChaCha20Poly1305> {
            connection: Connection::from_split(15, receiver_read, receiver_write),
            expected_peer_id: None,
            cryptographer,
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };
        GetKey::read_their_public_key(get_key).await
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_writes_flush_frames() {
        let mut writer = TrackingWrite::new();
        super::write_pre_handshake_header(&mut writer)
            .await
            .expect("preface write");
        assert_eq!(writer.flushes, 1, "preface should flush once");

        let payload = b"hello";
        super::write_handshake_frame(&mut writer, payload)
            .await
            .expect("handshake frame write");
        assert_eq!(writer.flushes, 2, "handshake frame should flush once");

        let mut expected = Vec::from(&super::PRE_MAGIC[..]);
        expected.push(super::PRE_VERSION);
        assert_eq!(
            &writer.buffer[..expected.len()],
            expected.as_slice(),
            "preface bytes should be written first"
        );

        let frame = &writer.buffer[expected.len()..];
        assert_eq!(frame.len(), 2 + payload.len());
        let len = u16::from_be_bytes([frame[0], frame[1]]);
        assert_eq!(len as usize, payload.len());
        assert_eq!(&frame[2..], payload);
    }

    #[test]
    fn handshake_signature_payload_is_consistent_between_sides() {
        let cryptographer = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[0xA5; 32])
            .expect("valid key length");

        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let hello = unsigned_handshake_hello(&KeyPair::random(), addr);
        let chain_id = iroha_data_model::ChainId::from("test-chain");
        let sender_payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &chain_id,
            None,
        );

        let receiver_payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &chain_id,
            None,
        );

        assert_eq!(sender_payload, receiver_payload);
    }

    #[test]
    fn handshake_signature_payload_always_binds_chain_id() {
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let hello = unsigned_handshake_hello(&KeyPair::random(), addr);
        let cryptographer = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[0x5A; 32])
            .expect("valid key length");

        let chain_a: iroha_data_model::ChainId =
            "00000000-0000-0000-0000-000000000001".parse().unwrap();
        let chain_b: iroha_data_model::ChainId =
            "00000000-0000-0000-0000-000000000002".parse().unwrap();
        let payload_a =
            handshake_signature_payload::<ChaCha20Poly1305>(&cryptographer, &hello, &chain_a, None);
        let payload_b =
            handshake_signature_payload::<ChaCha20Poly1305>(&cryptographer, &hello, &chain_b, None);

        assert_ne!(payload_a, payload_b);
    }

    #[test]
    fn handshake_signature_payload_binds_the_full_session_hash() {
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let hello = unsigned_handshake_hello(&KeyPair::random(), addr);
        let chain_id = iroha_data_model::ChainId::from("test-chain");
        let cryptographer = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[0x3C; 32])
            .expect("valid key length");
        let mut same_compact_prefix = cryptographer.clone();
        same_compact_prefix.session_binding[iroha_crypto::Hash::LENGTH - 1] ^= 1;
        assert_eq!(
            cryptographer.disambiguator, same_compact_prefix.disambiguator,
            "fixture must preserve the 64-bit operational tie-breaker"
        );

        let expected = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &chain_id,
            None,
        );
        let changed = handshake_signature_payload::<ChaCha20Poly1305>(
            &same_compact_prefix,
            &hello,
            &chain_id,
            None,
        );

        assert_ne!(
            expected, changed,
            "identity authentication must bind all 256 session-binding bits"
        );
    }

    #[test]
    fn handshake_signature_payload_binds_all_advertised_capabilities() {
        let cryptographer = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[0x6D; 32])
            .expect("valid key length");
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let mut hello = unsigned_handshake_hello(&KeyPair::random(), addr);
        hello.consensus.mode = Some(ConsensusMode::Permissioned);
        hello.confidential.enabled = Some(true);
        hello.crypto.sm_enabled = Some(false);
        let chain_id = iroha_data_model::ChainId::from("test-chain");
        let expected = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &chain_id,
            None,
        );

        let mut changed = hello.clone();
        changed.relay = RelayRole::Hub;
        assert_ne!(
            expected,
            handshake_signature_payload::<ChaCha20Poly1305>(
                &cryptographer,
                &changed,
                &chain_id,
                None,
            ),
            "relay capability must be authenticated"
        );

        let mut changed = hello.clone();
        changed.consensus.mode = Some(ConsensusMode::Npos);
        assert_ne!(
            expected,
            handshake_signature_payload::<ChaCha20Poly1305>(
                &cryptographer,
                &changed,
                &chain_id,
                None,
            ),
            "consensus capabilities must be authenticated"
        );

        let mut changed = hello.clone();
        changed.confidential.enabled = Some(false);
        assert_ne!(
            expected,
            handshake_signature_payload::<ChaCha20Poly1305>(
                &cryptographer,
                &changed,
                &chain_id,
                None,
            ),
            "confidential capabilities must be authenticated"
        );

        let mut changed = hello.clone();
        changed.crypto.sm_enabled = Some(true);
        assert_ne!(
            expected,
            handshake_signature_payload::<ChaCha20Poly1305>(
                &cryptographer,
                &changed,
                &chain_id,
                None,
            ),
            "cryptographic capabilities must be authenticated"
        );

        let mut changed = hello;
        changed.trust.scion_supported = true;
        assert_ne!(
            expected,
            handshake_signature_payload::<ChaCha20Poly1305>(
                &cryptographer,
                &changed,
                &chain_id,
                None,
            ),
            "trust and transport capabilities must be authenticated"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_capabilities_changed_after_signing() {
        let key_pair = KeyPair::random();
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let chain_id = iroha_data_model::ChainId::from("test-chain");
        let cryptographer = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[0x4E; 32])
            .expect("valid key length");
        let mut hello = unsigned_handshake_hello(&key_pair, addr);
        let payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &chain_id,
            None,
        );
        hello.signature = Signature::try_new(key_pair.private_key(), &payload)
            .expect("sign canonical handshake claims")
            .payload()
            .to_vec();

        hello.trust.scion_supported = true;
        let error = match read_crafted_handshake_hello(hello, cryptographer).await {
            Ok(_) => panic!("capabilities changed after signing must fail authentication"),
            Err(error) => error,
        };
        assert!(
            matches!(error, crate::Error::Keys(_)),
            "expected signature verification failure, got {error:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_peer_from_a_different_chain() {
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let key_pair = KeyPair::random();
        let cryptographer = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[0x7C; 32])
            .expect("valid key length");
        let (stream_a, stream_b) = tokio::io::duplex(512);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr,
            expected_peer_id: None,
            key_pair,
            connection: Connection::from_split(1, sender_read, sender_write),
            cryptographer: cryptographer.clone(),
            chain_id: iroha_data_model::ChainId::from("chain-a"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });
        let get_key = GetKey::<ChaCha20Poly1305> {
            connection: Connection::from_split(2, receiver_read, receiver_write),
            expected_peer_id: None,
            cryptographer,
            chain_id: iroha_data_model::ChainId::from("chain-b"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });
        let error = match GetKey::read_their_public_key(get_key).await {
            Ok(_) => panic!("a handshake signature from a different chain must be rejected"),
            Err(error) => error,
        };
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        assert!(
            matches!(error, crate::Error::Keys(_)),
            "expected signature verification failure, got {error:?}"
        );
    }

    #[test]
    fn confidential_digest_roundtrip_preserves_zk_policy_hash() {
        let digest = crate::ConfidentialFeatureDigest::new(
            Some([0x11; 32]),
            Some(7),
            Some(11),
            Some(13),
            Some([0xA5; 32]),
        );
        let handshake = HandshakeConfidentialDigest::from(&digest);
        let encoded = handshake.encode();
        let mut slice = encoded.as_slice();
        let decoded = HandshakeConfidentialDigest::decode_all(&mut slice)
            .expect("decode confidential handshake digest");

        assert!(slice.is_empty(), "digest decode should consume all bytes");
        let roundtrip = crate::ConfidentialFeatureDigest::from(decoded);
        assert_eq!(roundtrip, digest);
        assert_eq!(roundtrip.zk_policy_hash, Some([0xA5; 32]));
    }

    fn confidential_feature_digest(
        policy_hash_byte: Option<u8>,
    ) -> crate::ConfidentialFeatureDigest {
        confidential_feature_digest_with_rules(
            Some(iroha_data_model::confidential::CONFIDENTIAL_RULES_VERSION),
            policy_hash_byte,
        )
    }

    fn confidential_feature_digest_with_rules(
        rules_version: Option<u32>,
        policy_hash_byte: Option<u8>,
    ) -> crate::ConfidentialFeatureDigest {
        confidential_feature_digest_full(None, None, None, rules_version, policy_hash_byte)
    }

    fn confidential_feature_digest_full(
        vk_set_hash_byte: Option<u8>,
        poseidon_params_id: Option<u32>,
        pedersen_params_id: Option<u32>,
        rules_version: Option<u32>,
        policy_hash_byte: Option<u8>,
    ) -> crate::ConfidentialFeatureDigest {
        crate::ConfidentialFeatureDigest::new(
            vk_set_hash_byte.map(|byte| [byte; 32]),
            poseidon_params_id,
            pedersen_params_id,
            rules_version,
            policy_hash_byte.map(|byte| [byte; 32]),
        )
    }

    fn confidential_zk_caps(
        features: Option<crate::ConfidentialFeatureDigest>,
    ) -> ConfidentialHandshakeCaps {
        ConfidentialHandshakeCaps {
            enabled: true,
            assume_valid: false,
            verifier_backend: "halo2-ipa-pallas".to_string(),
            features,
        }
    }

    fn confidential_zk_caps_with_flags(
        assume_valid: bool,
        verifier_backend: &str,
        features: Option<crate::ConfidentialFeatureDigest>,
    ) -> ConfidentialHandshakeCaps {
        confidential_zk_caps_full(true, assume_valid, verifier_backend, features)
    }

    fn confidential_zk_caps_full(
        enabled: bool,
        assume_valid: bool,
        verifier_backend: &str,
        features: Option<crate::ConfidentialFeatureDigest>,
    ) -> ConfidentialHandshakeCaps {
        ConfidentialHandshakeCaps {
            enabled,
            assume_valid,
            verifier_backend: verifier_backend.to_string(),
            features,
        }
    }

    async fn confidential_handshake_error(
        sender_caps: ConfidentialHandshakeCaps,
        receiver_caps: ConfidentialHandshakeCaps,
    ) -> crate::Error {
        confidential_handshake_error_with_caps(Some(sender_caps), Some(receiver_caps)).await
    }

    async fn confidential_handshake_error_with_caps(
        sender_caps: Option<ConfidentialHandshakeCaps>,
        receiver_caps: Option<ConfidentialHandshakeCaps>,
    ) -> crate::Error {
        let addr: SocketAddr = "127.0.0.1:1338".parse().unwrap();
        let key_pair = KeyPair::random();
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[12u8; 32]).unwrap();

        let (stream_a, stream_b) = tokio::io::duplex(1024);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr.clone(),
            expected_peer_id: None,
            key_pair,
            connection: Connection::from_split(21, sender_read, sender_write),
            cryptographer: cryptographer.clone(),
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: sender_caps,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });

        let get_key = GetKey::<ChaCha20Poly1305> {
            connection: Connection::from_split(22, receiver_read, receiver_write),
            expected_peer_id: None,
            cryptographer,
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: receiver_caps,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });

        let err = match GetKey::read_their_public_key(get_key).await {
            Ok(_) => panic!("confidential capability mismatch must reject handshake"),
            Err(err) => err,
        };
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        err
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_confidential_zk_policy_hash_mismatch() {
        let err = confidential_handshake_error(
            confidential_zk_caps(Some(confidential_feature_digest(Some(0xAA)))),
            confidential_zk_caps(Some(confidential_feature_digest(Some(0xBB)))),
        )
        .await;

        assert!(
            matches!(err, crate::Error::HandshakeConfidentialMismatch),
            "expected confidential mismatch, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_missing_confidential_feature_digest_when_expected() {
        let err = confidential_handshake_error(
            confidential_zk_caps(None),
            confidential_zk_caps(Some(confidential_feature_digest(Some(0xAA)))),
        )
        .await;

        assert!(
            matches!(err, crate::Error::HandshakeConfidentialMismatch),
            "expected confidential mismatch, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_missing_confidential_meta_when_expected() {
        let err = confidential_handshake_error_with_caps(
            None,
            Some(confidential_zk_caps(Some(confidential_feature_digest(
                Some(0xAA),
            )))),
        )
        .await;

        assert!(
            matches!(err, crate::Error::HandshakeConfidentialMismatch),
            "expected confidential mismatch, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_missing_zk_policy_hash_when_expected() {
        let err = confidential_handshake_error(
            confidential_zk_caps(Some(confidential_feature_digest(None))),
            confidential_zk_caps(Some(confidential_feature_digest(Some(0xAA)))),
        )
        .await;

        assert!(
            matches!(err, crate::Error::HandshakeConfidentialMismatch),
            "expected confidential mismatch, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_missing_confidential_rules_version_when_expected() {
        let err = confidential_handshake_error(
            confidential_zk_caps(Some(confidential_feature_digest_with_rules(
                None,
                Some(0xAA),
            ))),
            confidential_zk_caps(Some(confidential_feature_digest(Some(0xAA)))),
        )
        .await;

        assert!(
            matches!(err, crate::Error::HandshakeConfidentialMismatch),
            "expected confidential mismatch, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_confidential_rules_version_mismatch() {
        let err = confidential_handshake_error(
            confidential_zk_caps(Some(confidential_feature_digest_with_rules(
                Some(iroha_data_model::confidential::CONFIDENTIAL_RULES_VERSION + 1),
                Some(0xAA),
            ))),
            confidential_zk_caps(Some(confidential_feature_digest(Some(0xAA)))),
        )
        .await;

        assert!(
            matches!(err, crate::Error::HandshakeConfidentialMismatch),
            "expected confidential mismatch, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_confidential_feature_material_mismatches() {
        for (label, sender_features, receiver_features) in [
            (
                "vk_set_hash",
                confidential_feature_digest_full(Some(0x10), None, None, Some(1), Some(0xAA)),
                confidential_feature_digest_full(Some(0x20), None, None, Some(1), Some(0xAA)),
            ),
            (
                "poseidon_params_id",
                confidential_feature_digest_full(None, Some(1), None, Some(1), Some(0xAA)),
                confidential_feature_digest_full(None, Some(2), None, Some(1), Some(0xAA)),
            ),
            (
                "pedersen_params_id",
                confidential_feature_digest_full(None, None, Some(1), Some(1), Some(0xAA)),
                confidential_feature_digest_full(None, None, Some(2), Some(1), Some(0xAA)),
            ),
            (
                "missing_poseidon_params_id",
                confidential_feature_digest_full(None, None, None, Some(1), Some(0xAA)),
                confidential_feature_digest_full(None, Some(1), None, Some(1), Some(0xAA)),
            ),
        ] {
            let err = confidential_handshake_error(
                confidential_zk_caps(Some(sender_features)),
                confidential_zk_caps(Some(receiver_features)),
            )
            .await;

            assert!(
                matches!(err, crate::Error::HandshakeConfidentialMismatch),
                "{label} should produce confidential mismatch, got {err:?}"
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_confidential_assume_valid_mismatch() {
        let features = Some(confidential_feature_digest(Some(0xAA)));
        let err = confidential_handshake_error(
            confidential_zk_caps_with_flags(true, "halo2-ipa-pallas", features.clone()),
            confidential_zk_caps_with_flags(false, "halo2-ipa-pallas", features),
        )
        .await;

        assert!(
            matches!(err, crate::Error::HandshakeConfidentialMismatch),
            "expected confidential mismatch, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_confidential_enabled_mismatch() {
        let features = Some(confidential_feature_digest(Some(0xAA)));
        let err = confidential_handshake_error(
            confidential_zk_caps_full(false, false, "halo2-ipa-pallas", features.clone()),
            confidential_zk_caps_full(true, false, "halo2-ipa-pallas", features),
        )
        .await;

        assert!(
            matches!(err, crate::Error::HandshakeConfidentialMismatch),
            "expected confidential mismatch, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_confidential_verifier_backend_mismatch() {
        let features = Some(confidential_feature_digest(Some(0xAA)));
        let err = confidential_handshake_error(
            confidential_zk_caps_with_flags(false, "halo2-ipa-pallas-alt", features.clone()),
            confidential_zk_caps_with_flags(false, "halo2-ipa-pallas", features),
        )
        .await;

        assert!(
            matches!(err, crate::Error::HandshakeConfidentialMismatch),
            "expected confidential mismatch, got {err:?}"
        );
    }

    #[test]
    fn untagged_handshake_is_rejected() {
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[9u8; 32]).unwrap();
        let key_pair = KeyPair::random();
        let (alg, pk_bytes) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid");
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let hello = HandshakeHelloV1 {
            algorithm: alg,
            public_key: pk_bytes.to_vec(),
            signature: vec![0u8; 64],
            addr: addr.clone(),
            relay: RelayRole::Disabled,
            consensus: HandshakeConsensusMeta {
                mode: None,
                proto_version: None,
                consensus_fingerprint: None,
                config: None,
            },
            confidential: HandshakeConfidentialMeta {
                enabled: None,
                assume_valid: None,
                verifier_backend: None,
                features: None,
            },
            crypto: HandshakeCryptoMeta {
                sm_enabled: None,
                sm_openssl_preview: None,
            },
            trust: HandshakeTrustMeta {
                trust_gossip: true,
                scion_supported: false,
            },
        };

        let raw = hello.encode();
        let encrypted = cryptographer.encrypt(&raw).expect("encrypt raw handshake");
        let decoded = decode_handshake_message(&cryptographer, &encrypted);
        assert!(
            matches!(decoded, Err(crate::Error::Format)),
            "untagged handshake must be rejected"
        );
    }

    #[test]
    fn versioned_handshake_preserves_trust_flag() {
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[11u8; 32]).unwrap();
        let key_pair = KeyPair::random();
        let (alg, pk_bytes) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid");
        let addr: SocketAddr = "127.0.0.1:1444".parse().unwrap();
        let hello = HandshakeHelloV1 {
            algorithm: alg,
            public_key: pk_bytes.to_vec(),
            signature: vec![1u8; 64],
            addr: addr.clone(),
            relay: RelayRole::Hub,
            consensus: HandshakeConsensusMeta {
                mode: Some(ConsensusMode::Permissioned),
                proto_version: Some(1),
                consensus_fingerprint: Some([7u8; 32]),
                config: None,
            },
            confidential: HandshakeConfidentialMeta {
                enabled: Some(true),
                assume_valid: Some(false),
                verifier_backend: Some("backend".to_string()),
                features: None,
            },
            crypto: HandshakeCryptoMeta {
                sm_enabled: Some(false),
                sm_openssl_preview: Some(false),
            },
            trust: HandshakeTrustMeta {
                trust_gossip: true,
                scion_supported: true,
            },
        };

        let encrypted =
            encode_handshake_message(&cryptographer, &hello).expect("encode v1 handshake");
        let decoded =
            decode_handshake_message(&cryptographer, &encrypted).expect("decode v1 handshake");
        let HandshakeHello::V1(v1) = decoded;
        assert_eq!(v1.addr, addr);
        assert!(v1.trust.trust_gossip);
    }

    #[test]
    fn handshake_decode_honors_its_pre_auth_resource_budget() {
        let key_pair = KeyPair::random();
        let addr: SocketAddr = "127.0.0.1:1444".parse().unwrap();
        let hello = unsigned_handshake_hello(&key_pair, addr);
        let body = hello.encode();
        let no_sequence_budget = norito::DecodeLimits::new(0, body.len(), 0, body.len(), 16);

        let error = decode_handshake_body_with_limits(&body, no_sequence_budget)
            .expect_err("the handshake decoder must not widen its caller's resource budget");
        assert!(
            matches!(
                error,
                crate::Error::NoritoCodec(norito::core::Error::SequenceLengthExceeded { .. })
                    | crate::Error::NoritoCodec(norito::core::Error::TotalElementsExceeded { .. })
                    | crate::Error::NoritoCodec(
                        norito::core::Error::TotalAllocationExceeded { .. }
                    )
            ),
            "expected a decode-budget rejection, got {error:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_fails_when_metadata_exceeds_limit() {
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let key_pair = KeyPair::random();
        let connection = Connection::from_split(7, tokio::io::empty(), tokio::io::sink());
        let cryptographer =
            super::cryptographer::Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(
                &[42u8; 32],
            )
            .expect("valid key length");
        let caps = ConfidentialHandshakeCaps {
            enabled: true,
            assume_valid: false,
            verifier_backend: "halo2-ipa-".repeat(7000),
            features: None,
        };
        let send_key = SendKey::<ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr,
            expected_peer_id: None,
            key_pair,
            connection,
            cryptographer,
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: Some(caps),
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });
        let err = match SendKey::<ChaCha20Poly1305>::send_our_public_key(send_key).await {
            Ok(_) => panic!("expected HandshakeMessageTooLarge error"),
            Err(err) => err,
        };
        assert!(
            matches!(err, crate::Error::HandshakeMessageTooLarge),
            "expected HandshakeMessageTooLarge, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_v1_defaults_to_trust_gossip() {
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let key_pair = KeyPair::random();
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[7u8; 32]).unwrap();

        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr.clone(),
            expected_peer_id: None,
            key_pair,
            connection: Connection::from_split(1, sender_read, sender_write),
            cryptographer: cryptographer.clone(),
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });

        let get_key = GetKey::<ChaCha20Poly1305> {
            connection: Connection::from_split(2, receiver_read, receiver_write),
            expected_peer_id: None,
            cryptographer,
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });

        let ready = GetKey::read_their_public_key(get_key)
            .await
            .expect("handshake should succeed");
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        assert!(ready.trust_gossip, "handshake should enable trust gossip");
        assert!(
            ready.scion_supported,
            "handshake should propagate SCION support flag"
        );
    }

    async fn write_framed_handshake<W>(writer: &mut W, encoded: &[u8])
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt;

        let len = u16::try_from(encoded.len()).expect("fixture handshake message length fits u16");
        writer.write_u16(len).await.expect("write hello length");
        writer.write_all(encoded).await.expect("write hello bytes");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_all_zero_signature_material() {
        let addr: SocketAddr = "127.0.0.1:1443".parse().unwrap();
        let key_pair = KeyPair::random();
        let (algorithm, public_key) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must be valid");
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[13u8; 32]).unwrap();
        let hello = HandshakeHelloV1 {
            algorithm,
            public_key: public_key.to_vec(),
            signature: vec![0u8; 64],
            addr,
            relay: RelayRole::Disabled,
            consensus: HandshakeConsensusMeta {
                mode: None,
                proto_version: None,
                consensus_fingerprint: None,
                config: None,
            },
            confidential: HandshakeConfidentialMeta {
                enabled: None,
                assume_valid: None,
                verifier_backend: None,
                features: None,
            },
            crypto: HandshakeCryptoMeta {
                sm_enabled: None,
                sm_openssl_preview: None,
            },
            trust: HandshakeTrustMeta {
                trust_gossip: true,
                scion_supported: false,
            },
        };
        let encoded =
            encode_handshake_message(&cryptographer, &hello).expect("encode crafted hello");

        let (stream_a, stream_b) = tokio::io::duplex(4096);
        let (_sender_read, mut sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);
        write_framed_handshake(&mut sender_write, &encoded).await;

        let get_key = GetKey::<ChaCha20Poly1305> {
            connection: Connection::from_split(15, receiver_read, receiver_write),
            expected_peer_id: None,
            cryptographer,
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let err = match GetKey::read_their_public_key(get_key).await {
            Ok(_) => panic!("all-zero handshake signature material must be rejected"),
            Err(err) => err,
        };
        assert!(
            matches!(err, crate::Error::Keys(_)),
            "expected signature parse failure, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_malformed_ed25519_signature_r() {
        const SMALL_ORDER_R: [u8; 32] = [
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        const NONCANONICAL_R: [u8; 32] = [
            0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0x7f,
        ];

        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_R),
            ("noncanonical", NONCANONICAL_R),
        ] {
            let addr: SocketAddr = "127.0.0.1:1443".parse().unwrap();
            let key_pair = KeyPair::random();
            let cryptographer =
                Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[13u8; 32]).unwrap();
            let chain_id = iroha_data_model::ChainId::from("test-chain");
            let mut hello = unsigned_handshake_hello(&key_pair, addr);
            let payload = handshake_signature_payload::<ChaCha20Poly1305>(
                &cryptographer,
                &hello,
                &chain_id,
                None,
            );
            let mut signature = Signature::try_new(key_pair.private_key(), &payload)
                .expect("checked handshake fixture signature")
                .payload()
                .to_vec();
            signature[..replacement_r.len()].copy_from_slice(&replacement_r);
            hello.signature = signature;
            let encoded =
                encode_handshake_message(&cryptographer, &hello).expect("encode crafted hello");

            let (stream_a, stream_b) = tokio::io::duplex(4096);
            let (_sender_read, mut sender_write) = tokio::io::split(stream_a);
            let (receiver_read, receiver_write) = tokio::io::split(stream_b);
            write_framed_handshake(&mut sender_write, &encoded).await;

            let get_key = GetKey::<ChaCha20Poly1305> {
                connection: Connection::from_split(15, receiver_read, receiver_write),
                expected_peer_id: None,
                cryptographer,
                chain_id: iroha_data_model::ChainId::from("test-chain"),
                consensus_caps: None,
                confidential_caps: None,
                crypto_caps: None,
                relay_role: RelayRole::Disabled,
                local_scion_supported: true,
                trust_gossip: true,
            };

            let err = match GetKey::read_their_public_key(get_key).await {
                Ok(_) => panic!("{label} Ed25519 handshake signature R must be rejected"),
                Err(err) => err,
            };
            assert!(
                matches!(err, crate::Error::Keys(_)),
                "expected {label} signature parse failure, got {err:?}"
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_malformed_mldsa_signature_lengths() {
        let addr: SocketAddr = "127.0.0.1:1443".parse().unwrap();
        let key_pair = KeyPair::try_from_seed(
            b"p2p-handshake-mldsa-signature-admission".to_vec(),
            Algorithm::MlDsa,
        )
        .expect("derive checked ML-DSA handshake fixture keypair");
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[14u8; 32]).unwrap();
        let chain_id = iroha_data_model::ChainId::from("test-chain");
        let mut hello = unsigned_handshake_hello(&key_pair, addr);
        let payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &chain_id,
            None,
        );
        let valid_signature = Signature::try_new(key_pair.private_key(), &payload)
            .expect("checked ML-DSA handshake fixture signature")
            .payload()
            .to_vec();
        hello.signature = valid_signature.clone();

        read_crafted_handshake_hello(hello.clone(), cryptographer.clone())
            .await
            .expect("valid ML-DSA handshake signature must verify");

        let mut short = valid_signature.clone();
        short.pop();
        let mut overlong = valid_signature.clone();
        overlong.push(0x42);

        for (label, signature) in [
            ("short", short),
            ("overlong", overlong),
            ("all-zero", vec![0_u8; valid_signature.len()]),
        ] {
            let mut malformed = hello.clone();
            malformed.signature = signature;
            let err = match read_crafted_handshake_hello(malformed, cryptographer.clone()).await {
                Ok(_) => panic!("{label} ML-DSA handshake signature unexpectedly verified"),
                Err(err) => err,
            };
            assert!(
                matches!(err, crate::Error::Keys(_)),
                "expected {label} ML-DSA signature parse failure, got {err:?}"
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_accepts_matching_transport_binding() {
        let addr: SocketAddr = "127.0.0.1:1444".parse().unwrap();
        let key_pair = KeyPair::random();
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[9u8; 32]).unwrap();
        let transport_binding = [0x5Au8; iroha_crypto::Hash::LENGTH];

        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr,
            expected_peer_id: None,
            key_pair,
            connection: Connection::from_split_with_binding(
                11,
                sender_read,
                sender_write,
                Some(transport_binding),
            ),
            cryptographer: cryptographer.clone(),
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });

        let get_key = GetKey::<ChaCha20Poly1305> {
            connection: Connection::from_split_with_binding(
                12,
                receiver_read,
                receiver_write,
                Some(transport_binding),
            ),
            expected_peer_id: None,
            cryptographer,
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });

        let ready = GetKey::read_their_public_key(get_key)
            .await
            .expect("handshake should succeed with matching transport binding");
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        assert_eq!(ready.connection.transport_binding, Some(transport_binding));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_mismatched_transport_binding() {
        let addr: SocketAddr = "127.0.0.1:1446".parse().unwrap();
        let key_pair = KeyPair::random();
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[10u8; 32]).unwrap();

        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr,
            expected_peer_id: None,
            key_pair,
            connection: Connection::from_split_with_binding(
                13,
                sender_read,
                sender_write,
                Some([0x11u8; iroha_crypto::Hash::LENGTH]),
            ),
            cryptographer: cryptographer.clone(),
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });

        let get_key = GetKey::<ChaCha20Poly1305> {
            connection: Connection::from_split_with_binding(
                14,
                receiver_read,
                receiver_write,
                Some([0x22u8; iroha_crypto::Hash::LENGTH]),
            ),
            expected_peer_id: None,
            cryptographer,
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });

        let err = match GetKey::read_their_public_key(get_key).await {
            Ok(_) => panic!("mismatched transport binding must be rejected"),
            Err(err) => err,
        };
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        assert!(
            matches!(err, crate::Error::Keys(_)),
            "expected signature verification failure, got {err:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn outgoing_handshake_rejects_unexpected_peer_identity() {
        let addr: SocketAddr = "127.0.0.1:1445".parse().unwrap();
        let actual_key_pair = KeyPair::random();
        let expected_peer_id =
            iroha_data_model::prelude::PeerId::from(KeyPair::random().public_key().clone());
        let cryptographer =
            Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[8u8; 32]).unwrap();

        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (sender_read, sender_write) = tokio::io::split(stream_a);
        let (receiver_read, receiver_write) = tokio::io::split(stream_b);

        let send_key = SendKey::<ChaCha20Poly1305>::new(SendKeyInit {
            our_public_address: addr.clone(),
            expected_peer_id: None,
            key_pair: actual_key_pair,
            connection: Connection::from_split(3, sender_read, sender_write),
            cryptographer: cryptographer.clone(),
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        });

        let get_key = GetKey::<ChaCha20Poly1305> {
            connection: Connection::from_split(4, receiver_read, receiver_write),
            expected_peer_id: Some(expected_peer_id.clone()),
            cryptographer,
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            relay_role: RelayRole::Disabled,
            local_scion_supported: true,
            trust_gossip: true,
        };

        let sender = tokio::spawn(async move {
            let _ = SendKey::send_our_public_key(send_key).await?;
            Result::<(), crate::Error>::Ok(())
        });

        let err = match GetKey::read_their_public_key(get_key).await {
            Ok(_) => panic!("unexpected peer identity must be rejected"),
            Err(err) => err,
        };
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");

        match err {
            crate::Error::HandshakePeerMismatch { expected, found } => {
                assert_eq!(expected, expected_peer_id);
                assert_ne!(expected, found);
            }
            other => panic!("expected HandshakePeerMismatch, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pre_handshake_header_rejects_garbage() {
        // Build a duplex to simulate a remote sending garbage preface
        let (a, mut b) = tokio::io::duplex(64);
        // Writer side: send wrong 5 bytes then close
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let _ = b.write_all(b"BAD!!").await;
        });

        // ConnectedFrom will attempt to read the preface and should error out
        let key_pair = iroha_crypto::KeyPair::random();
        let our_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let (r, w) = tokio::io::split(a);
        let conn = Connection::from_split(1, r, w);
        let soranet = Arc::new(SoranetHandshakeConfig::defaults());
        let cf = ConnectedFrom {
            our_public_address: our_addr,
            key_pair,
            connection: conn,
            chain_id: iroha_data_model::ChainId::from("test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            soranet_handshake: soranet,
            local_scion_supported: true,
            trust_gossip: true,
            relay_role: RelayRole::Disabled,
        };
        let err =
            ConnectedFrom::read_client_hello::<iroha_crypto::encryption::ChaCha20Poly1305>(cf)
                .await
                .err()
                .expect("expected error on bad preface");
        let _ = err; // just ensure it errs
    }

    #[cfg(feature = "noise_handshake")]
    #[tokio::test(flavor = "current_thread")]
    async fn noise_handshake_roundtrip_keys_match() {
        let (stream_a, stream_b) = tokio::io::duplex(256);
        let (mut a_read, mut a_write) = tokio::io::split(stream_a);
        let (mut b_read, mut b_write) = tokio::io::split(stream_b);

        let (init_res, resp_res) = tokio::join!(
            super::noise_handshake_initiator(&mut a_read, &mut a_write),
            super::noise_handshake_responder(&mut b_read, &mut b_write),
        );

        let init_key = init_res.expect("initiator handshake");
        let resp_key = resp_res.expect("responder handshake");
        assert_eq!(init_key, resp_key, "handshake keys must match");
        assert_eq!(init_key.len(), 32, "handshake key must be 32 bytes");
    }
}

// handshake payload is encoded/decoded as a tuple to avoid extra type definitions

mod handshake_flow {
    //! Implementations of the handshake process.

    use async_trait::async_trait;

    use super::{state::*, *};

    #[async_trait]
    pub(super) trait Stage<E: Enc> {
        type NextStage;

        async fn advance_to_next_stage(self) -> Result<Self::NextStage, crate::Error>;
    }

    macro_rules! stage {
        ( $func:ident : $curstage:ty => $nextstage:ty ) => {
            stage!(@base self Self::$func(self).await ; $curstage => $nextstage);
        };
        ( $func:ident :: <$($generic_param:ident),+> : $curstage:ty => $nextstage:ty ) => {
            stage!(@base self Self::$func::<$($generic_param),+>(self).await ; $curstage => $nextstage);
        };
        // Internal case
        (@base $self:ident $call:expr ; $curstage:ty => $nextstage:ty ) => {
            #[async_trait]
            impl<E: Enc> Stage<E> for $curstage {
                type NextStage = $nextstage;

                async fn advance_to_next_stage(self) -> Result<Self::NextStage, crate::Error> {
                    // NOTE: Need this due to macro hygiene
                    let $self = self;
                    $call
                }
            }
        }
    }

    stage!(connect_to: Connecting => ConnectedTo);
    stage!(send_client_hello::<E>: ConnectedTo => SendKey<E>);
    stage!(read_client_hello::<E>: ConnectedFrom => SendKey<E>);
    stage!(send_our_public_key: SendKey<E> => GetKey<E>);
    stage!(read_their_public_key: GetKey<E> => Ready<E>);

    #[async_trait]
    pub(super) trait Handshake<E: Enc> {
        async fn handshake(self) -> Result<Ready<E>, crate::Error>;
    }

    macro_rules! impl_handshake {
        ( base_case $typ:ty ) => {
            // Base case, should be all states that lead to `Ready`
            #[async_trait]
            impl<E: Enc> Handshake<E> for $typ {
                #[inline]
                async fn handshake(self) -> Result<Ready<E>, crate::Error> {
                    <$typ as Stage<E>>::advance_to_next_stage(self).await
                }
            }
        };
        ( $typ:ty ) => {
            #[async_trait]
            impl<E: Enc> Handshake<E> for $typ {
                #[inline]
                async fn handshake(self) -> Result<Ready<E>, crate::Error> {
                    let next_stage = <$typ as Stage<E>>::advance_to_next_stage(self).await?;
                    <_ as Handshake<E>>::handshake(next_stage).await
                }
            }
        };
    }

    impl_handshake!(base_case GetKey<E>);
    impl_handshake!(SendKey<E>);
    impl_handshake!(ConnectedFrom);
    impl_handshake!(ConnectedTo);
    impl_handshake!(Connecting);
}

pub(crate) use run::{checked_data_message_wire_len, data_message_wire_len_from_payload_len};
