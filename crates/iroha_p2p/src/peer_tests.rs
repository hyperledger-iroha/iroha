// Peer transport regressions are included at module scope to preserve private-item access.
#[cfg(test)]
mod tests {
    use super::{
        Connection, SoranetHandshakeConfig, cryptographer::Cryptographer, state::*, test_network_id,
    };
    use crate::{ConfidentialHandshakeCaps, ConsensusConfigCaps, ConsensusMode, RelayRole};
    use iroha_crypto::{Algorithm, KeyPair, Signature, encryption::ChaCha20Poly1305};
    use iroha_primitives::addr::SocketAddr;
    use norito::codec::{DecodeAll, Encode};
    use std::{
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
        time::Duration,
    };
    use tokio::io::AsyncWrite;
    const TEST_SORANET_TRANSPORT_BINDING: [u8; iroha_crypto::Hash::LENGTH] =
        [0xD7; iroha_crypto::Hash::LENGTH];
    fn delegation_test_key(seed: u8, algorithm: Algorithm) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], algorithm)
            .expect("deterministic delegation test key must be valid")
    }
    fn delegation_test_challenge(byte: u8) -> super::SoranetTransportDelegationChallenge {
        [byte; super::SORANET_TRANSPORT_DELEGATION_CHALLENGE_BYTES]
    }
    fn signed_delegation(
        node: &KeyPair,
        transport: &KeyPair,
        network_id: &iroha_data_model::NetworkId,
        challenge: super::SoranetTransportDelegationChallenge,
    ) -> super::LocalSoranetTransportDelegationV3 {
        super::sign_soranet_transport_delegation_v3(node, transport, network_id, challenge)
            .expect("valid test roles must sign a canonical delegation")
    }
    fn decode_signed_delegation(frame: &[u8]) -> super::SignedSoranetTransportDelegationV3 {
        norito::decode_canonical(frame).expect("signed delegation must be canonical Norito")
    }
    fn encode_signed_delegation(signed: &super::SignedSoranetTransportDelegationV3) -> Vec<u8> {
        norito::encode_canonical(signed)
            .expect("delegation test fixture must encode as canonical Norito")
    }
    fn sign_delegation_statement(
        signer: &KeyPair,
        statement: &super::SoranetTransportDelegationStatementV3,
    ) -> Vec<u8> {
        Signature::try_new(
            signer.private_key(),
            &super::soranet_transport_delegation_signature_payload_v3(statement),
        )
        .expect("deterministic delegation statement must be signable")
        .payload()
        .to_vec()
    }
    fn unwrap_delegation_error(error: crate::Error) -> crate::SoranetTransportDelegationError {
        match error {
            crate::Error::HandshakeSoranetDelegation(error) => error,
            other => panic!("expected SoraNet transport delegation error, got {other:?}"),
        }
    }
    async fn read_delegation_wire(
        wire: &[u8],
        expected_network_id: &iroha_data_model::NetworkId,
        expected_peer_id: &iroha_data_model::peer::PeerId,
        expected_challenge: &super::SoranetTransportDelegationChallenge,
    ) -> Result<super::VerifiedSoranetTransportDelegationV3, crate::Error> {
        use tokio::io::AsyncWriteExt;
        let (mut sender, mut receiver) = tokio::io::duplex(wire.len().saturating_add(1).max(1));
        sender
            .write_all(wire)
            .await
            .expect("delegation wire fixture must fit its duplex buffer");
        sender
            .shutdown()
            .await
            .expect("delegation wire fixture shutdown must succeed");
        super::read_and_verify_soranet_transport_delegation_v3(
            &mut receiver,
            expected_network_id,
            expected_peer_id,
            expected_challenge,
        )
        .await
    }
    #[test]
    fn soranet_transport_delegation_v3_canonical_roundtrip_is_deterministic() {
        assert_eq!(super::PRE_VERSION, 3);
        let challenge = delegation_test_challenge(0xA5);
        let network_id = test_network_id("delegation-canonical-chain");
        let node = delegation_test_key(0x11, Algorithm::BlsNormal);
        let node_id = iroha_data_model::peer::PeerId::from(node.public_key().clone());
        let transport = delegation_test_key(0x22, Algorithm::Ed25519);
        let transport_public_key = transport.public_key().clone();
        let signed = signed_delegation(&node, &transport, &network_id, challenge);
        assert!(!signed.canonical_signed_frame.is_empty());
        assert!(
            signed.canonical_signed_frame.len()
                <= super::MAX_SORANET_TRANSPORT_DELEGATION_FRAME_BYTES
        );
        let decoded = decode_signed_delegation(&signed.canonical_signed_frame);
        assert_eq!(
            encode_signed_delegation(&decoded),
            signed.canonical_signed_frame
        );
        assert_eq!(decoded.statement.p2p_preface_version, super::PRE_VERSION);
        assert_eq!(decoded.statement.challenge, challenge);
        assert_eq!(decoded.statement.network_id, network_id);
        assert_eq!(decoded.statement.node_id, node_id);
        assert_eq!(decoded.statement.transport_public_key, transport_public_key);
        let verified = super::verify_soranet_transport_delegation_v3(
            &signed.canonical_signed_frame,
            &network_id,
            &node_id,
            &challenge,
        )
        .expect("canonical delegation must verify");
        assert_eq!(verified.transport_public_key, transport_public_key);
        assert_eq!(verified.binding, signed.binding);
        assert_eq!(
            signed.binding,
            super::soranet_transport_delegation_binding_v3(&signed.canonical_signed_frame)
        );
        let repeated = signed_delegation(
            &delegation_test_key(0x11, Algorithm::BlsNormal),
            &delegation_test_key(0x22, Algorithm::Ed25519),
            &network_id,
            challenge,
        );
        assert_eq!(
            signed.canonical_signed_frame,
            repeated.canonical_signed_frame
        );
        assert_eq!(signed.binding, repeated.binding);
    }
    #[test]
    fn soranet_transport_delegation_v3_rejects_cross_role_algorithms() {
        let network_id = test_network_id("delegation-role-chain");
        let challenge = delegation_test_challenge(0x31);
        let error = super::sign_soranet_transport_delegation_v3(
            &delegation_test_key(0x31, Algorithm::Ed25519),
            &delegation_test_key(0x32, Algorithm::Ed25519),
            &network_id,
            challenge,
        )
        .expect_err("an Ed25519 key must not enter the BLS node role");
        assert!(matches!(
            unwrap_delegation_error(error),
            crate::SoranetTransportDelegationError::LocalNodeAlgorithmMismatch {
                found: Algorithm::Ed25519
            }
        ));
        let error = super::sign_soranet_transport_delegation_v3(
            &delegation_test_key(0x33, Algorithm::BlsNormal),
            &delegation_test_key(0x34, Algorithm::BlsNormal),
            &network_id,
            challenge,
        )
        .expect_err("a BLS key must not enter the Ed25519 transport role");
        assert!(matches!(
            unwrap_delegation_error(error),
            crate::SoranetTransportDelegationError::LocalTransportAlgorithmMismatch {
                found: Algorithm::BlsNormal
            }
        ));
    }
    #[test]
    fn soranet_transport_delegation_v3_replay_fails_under_fresh_challenge() {
        let network_id = test_network_id("delegation-replay-chain");
        let node = delegation_test_key(0x41, Algorithm::BlsNormal);
        let node_id = iroha_data_model::peer::PeerId::from(node.public_key().clone());
        let transport = delegation_test_key(0x42, Algorithm::Ed25519);
        let old_challenge = delegation_test_challenge(0x43);
        let fresh_challenge = delegation_test_challenge(0x44);
        let captured = signed_delegation(&node, &transport, &network_id, old_challenge);
        let fresh = signed_delegation(&node, &transport, &network_id, fresh_challenge);
        assert_ne!(
            captured.canonical_signed_frame,
            fresh.canonical_signed_frame
        );
        assert_ne!(captured.binding, fresh.binding);
        let error = super::verify_soranet_transport_delegation_v3(
            &captured.canonical_signed_frame,
            &network_id,
            &node_id,
            &fresh_challenge,
        )
        .expect_err("a captured frame must not authorize a fresh connection");
        match error {
            crate::SoranetTransportDelegationError::ChallengeMismatch { expected, found } => {
                assert_eq!(expected, fresh_challenge);
                assert_eq!(found, old_challenge);
            }
            other => panic!("expected exact challenge mismatch, got {other:?}"),
        }
        super::verify_soranet_transport_delegation_v3(
            &fresh.canonical_signed_frame,
            &network_id,
            &node_id,
            &fresh_challenge,
        )
        .expect("freshly challenged delegation must verify");
    }
    #[test]
    fn soranet_transport_delegation_v3_rejects_same_name_different_genesis() {
        let display_name = iroha_data_model::ChainId::from("delegation-shared-name");
        let signed_network = test_network_id("delegation-genesis-a");
        let expected_network = test_network_id("delegation-genesis-b");
        assert_ne!(signed_network, expected_network);
        let node = delegation_test_key(0x51, Algorithm::BlsNormal);
        let node_id = iroha_data_model::peer::PeerId::from(node.public_key().clone());
        let other_node = delegation_test_key(0x52, Algorithm::BlsNormal);
        let other_node_id = iroha_data_model::peer::PeerId::from(other_node.public_key().clone());
        let challenge = delegation_test_challenge(0x53);
        let frame = signed_delegation(
            &node,
            &delegation_test_key(0x54, Algorithm::Ed25519),
            &signed_network,
            challenge,
        );
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &frame.canonical_signed_frame,
                &expected_network,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::NetworkMismatch { expected, found })
                if expected == expected_network && found == signed_network
        ));
        assert_eq!(display_name.as_str(), "delegation-shared-name");
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &frame.canonical_signed_frame,
                &signed_network,
                &other_node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::PeerMismatch { expected, found })
                if expected == other_node_id && found == node_id
        ));
        let mut wrong_version = decode_signed_delegation(&frame.canonical_signed_frame);
        wrong_version.statement.p2p_preface_version = 2;
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &encode_signed_delegation(&wrong_version),
                &signed_network,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::UnsupportedVersion {
                expected: 3,
                found: 2
            })
        ));
        let mut wrong_signer = decode_signed_delegation(&frame.canonical_signed_frame);
        wrong_signer.node_signature =
            sign_delegation_statement(&other_node, &wrong_signer.statement);
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &encode_signed_delegation(&wrong_signer),
                &signed_network,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::InvalidNodeSignature)
        ));
    }
    #[test]
    fn soranet_transport_delegation_v3_verifier_rejects_wrong_key_roles() {
        let network_id = test_network_id("delegation-algorithm-chain");
        let challenge = delegation_test_challenge(0x61);
        let ed_node = delegation_test_key(0x62, Algorithm::Ed25519);
        let ed_node_id = iroha_data_model::peer::PeerId::from(ed_node.public_key().clone());
        let non_bls_statement = super::SoranetTransportDelegationStatementV3 {
            p2p_preface_version: super::PRE_VERSION,
            challenge,
            network_id: network_id.clone(),
            node_id: ed_node_id.clone(),
            transport_public_key: delegation_test_key(0x63, Algorithm::Ed25519)
                .public_key()
                .clone(),
        };
        let non_bls = super::SignedSoranetTransportDelegationV3 {
            node_signature: sign_delegation_statement(&ed_node, &non_bls_statement),
            statement: non_bls_statement,
        };
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &encode_signed_delegation(&non_bls),
                &network_id,
                &ed_node_id,
                &challenge,
            ),
            Err(
                crate::SoranetTransportDelegationError::NodeAlgorithmMismatch {
                    found: Algorithm::Ed25519
                }
            )
        ));
        let bls_node = delegation_test_key(0x64, Algorithm::BlsNormal);
        let bls_node_id = iroha_data_model::peer::PeerId::from(bls_node.public_key().clone());
        let non_ed_statement = super::SoranetTransportDelegationStatementV3 {
            p2p_preface_version: super::PRE_VERSION,
            challenge,
            network_id: network_id.clone(),
            node_id: bls_node_id.clone(),
            transport_public_key: delegation_test_key(0x65, Algorithm::BlsNormal)
                .public_key()
                .clone(),
        };
        let non_ed = super::SignedSoranetTransportDelegationV3 {
            node_signature: sign_delegation_statement(&bls_node, &non_ed_statement),
            statement: non_ed_statement,
        };
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &encode_signed_delegation(&non_ed),
                &network_id,
                &bls_node_id,
                &challenge,
            ),
            Err(
                crate::SoranetTransportDelegationError::TransportAlgorithmMismatch {
                    found: Algorithm::BlsNormal
                }
            )
        ));
    }
    #[test]
    fn soranet_transport_delegation_v3_rejects_signature_attacks_and_bit_mutation() {
        let network_id = test_network_id("delegation-signature-chain");
        let node = delegation_test_key(0x71, Algorithm::BlsNormal);
        let node_id = iroha_data_model::peer::PeerId::from(node.public_key().clone());
        let challenge = delegation_test_challenge(0x72);
        let frame = signed_delegation(
            &node,
            &delegation_test_key(0x73, Algorithm::Ed25519),
            &network_id,
            challenge,
        );
        let signed = decode_signed_delegation(&frame.canonical_signed_frame);
        let expected_len = Algorithm::BlsNormal.signature_payload_len();
        for found in [0, 1, expected_len - 1, expected_len + 1] {
            let mut malformed = signed.clone();
            malformed.node_signature = vec![0xA5; found];
            assert!(matches!(
                super::verify_soranet_transport_delegation_v3(
                    &encode_signed_delegation(&malformed),
                    &network_id,
                    &node_id,
                    &challenge,
                ),
                Err(crate::SoranetTransportDelegationError::NodeSignatureLength {
                    expected,
                    found: actual
                }) if expected == expected_len && actual == found
            ));
        }
        let mut all_zero = signed.clone();
        all_zero.node_signature = vec![0; expected_len];
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &encode_signed_delegation(&all_zero),
                &network_id,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::MalformedNodeSignature)
        ));
        let mut bit_flipped = signed;
        bit_flipped.node_signature[0] ^= 1;
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &encode_signed_delegation(&bit_flipped),
                &network_id,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::InvalidNodeSignature)
        ));
    }
    #[test]
    fn soranet_transport_delegation_v3_rejects_empty_oversize_truncated_and_trailing() {
        let network_id = test_network_id("delegation-frame-chain");
        let node = delegation_test_key(0x81, Algorithm::BlsNormal);
        let node_id = iroha_data_model::peer::PeerId::from(node.public_key().clone());
        let challenge = delegation_test_challenge(0x82);
        let frame = signed_delegation(
            &node,
            &delegation_test_key(0x83, Algorithm::Ed25519),
            &network_id,
            challenge,
        );
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(&[], &network_id, &node_id, &challenge,),
            Err(crate::SoranetTransportDelegationError::EmptyFrame)
        ));
        let oversized = vec![0; super::MAX_SORANET_TRANSPORT_DELEGATION_FRAME_BYTES + 1];
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &oversized,
                &network_id,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::FrameTooLarge { found, max })
                if found == super::MAX_SORANET_TRANSPORT_DELEGATION_FRAME_BYTES + 1
                    && max == super::MAX_SORANET_TRANSPORT_DELEGATION_FRAME_BYTES
        ));
        let mut truncated = frame.canonical_signed_frame.clone();
        truncated.pop();
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &truncated,
                &network_id,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::NonCanonicalEncoding(_))
        ));
        let mut trailing = frame.canonical_signed_frame.clone();
        trailing.push(0);
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &trailing,
                &network_id,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::NonCanonicalEncoding(_))
        ));
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &[0xFF; 16],
                &network_id,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::NonCanonicalEncoding(_))
        ));
    }
    #[tokio::test(flavor = "current_thread")]
    async fn soranet_transport_delegation_v3_wire_reader_enforces_boundaries() {
        let network_id = test_network_id("delegation-wire-chain");
        let node = delegation_test_key(0x91, Algorithm::BlsNormal);
        let node_id = iroha_data_model::peer::PeerId::from(node.public_key().clone());
        let challenge = delegation_test_challenge(0x92);
        let frame = signed_delegation(
            &node,
            &delegation_test_key(0x93, Algorithm::Ed25519),
            &network_id,
            challenge,
        );
        let error = read_delegation_wire(&[0, 0], &network_id, &node_id, &challenge)
            .await
            .expect_err("zero-length frame must fail");
        assert!(matches!(
            unwrap_delegation_error(error),
            crate::SoranetTransportDelegationError::EmptyFrame
        ));
        let oversized_len = u16::try_from(super::MAX_SORANET_TRANSPORT_DELEGATION_FRAME_BYTES + 1)
            .expect("wire bound fits u16");
        let error = read_delegation_wire(
            &oversized_len.to_be_bytes(),
            &network_id,
            &node_id,
            &challenge,
        )
        .await
        .expect_err("oversized declaration must fail before allocation");
        assert!(matches!(
            unwrap_delegation_error(error),
            crate::SoranetTransportDelegationError::FrameTooLarge { found, max }
                if found == usize::from(oversized_len)
                    && max == super::MAX_SORANET_TRANSPORT_DELEGATION_FRAME_BYTES
        ));
        let declared_len =
            u16::try_from(frame.canonical_signed_frame.len()).expect("fixture fits u16");
        let mut truncated_wire = declared_len.to_be_bytes().to_vec();
        truncated_wire.extend_from_slice(
            &frame.canonical_signed_frame[..frame.canonical_signed_frame.len() - 1],
        );
        assert!(matches!(
            read_delegation_wire(&truncated_wire, &network_id, &node_id, &challenge)
                .await
                .expect_err("EOF inside frame must fail"),
            crate::Error::Io(error) if error.kind() == std::io::ErrorKind::UnexpectedEof
        ));
        let trailing_len =
            u16::try_from(frame.canonical_signed_frame.len() + 1).expect("fixture fits u16");
        let mut trailing_wire = trailing_len.to_be_bytes().to_vec();
        trailing_wire.extend_from_slice(&frame.canonical_signed_frame);
        trailing_wire.push(0);
        let error = read_delegation_wire(&trailing_wire, &network_id, &node_id, &challenge)
            .await
            .expect_err("trailing payload byte must fail canonical decoding");
        assert!(matches!(
            unwrap_delegation_error(error),
            crate::SoranetTransportDelegationError::NonCanonicalEncoding(_)
        ));
    }
    #[tokio::test(flavor = "current_thread")]
    async fn soranet_transport_delegation_v3_prefaces_and_full_duplex_exchange_are_exact() {
        let network_id = test_network_id("delegation-duplex-chain");
        let node = delegation_test_key(0xA1, Algorithm::BlsNormal);
        let node_id = iroha_data_model::peer::PeerId::from(node.public_key().clone());
        let transport = delegation_test_key(0xA2, Algorithm::Ed25519);
        let transport_public_key = transport.public_key().clone();
        let challenge = delegation_test_challenge(0xA3);
        let server_chain = network_id.clone();
        let server_node = node.clone();
        let server_transport = transport.clone();
        let (mut client, mut server) = tokio::io::duplex(2048);
        let server_task = tokio::spawn(async move {
            let received = super::read_and_verify_client_pre_handshake_header(&mut server)
                .await
                .expect("valid client preface");
            assert_eq!(received, challenge);
            super::write_server_pre_handshake_header(&mut server)
                .await
                .expect("valid server confirmation");
            let frame = signed_delegation(&server_node, &server_transport, &server_chain, received);
            super::write_soranet_transport_delegation_v3(
                &mut server,
                &frame.canonical_signed_frame,
            )
            .await
            .expect("canonical frame write");
            frame.binding
        });
        super::write_client_pre_handshake_header(&mut client, &challenge)
            .await
            .expect("valid client preface write");
        super::read_and_verify_server_pre_handshake_header(&mut client)
            .await
            .expect("valid server confirmation");
        let verified = super::read_and_verify_soranet_transport_delegation_v3(
            &mut client,
            &network_id,
            &node_id,
            &challenge,
        )
        .await
        .expect("fresh delegation must verify before SoraNet work");
        assert_eq!(verified.transport_public_key, transport_public_key);
        assert_eq!(
            verified.binding,
            server_task.await.expect("server exchange must complete")
        );
        let mut client_preface = TrackingWrite::new();
        super::write_client_pre_handshake_header(&mut client_preface, &challenge)
            .await
            .expect("tracking client preface write");
        let mut expected_client = super::PRE_MAGIC.to_vec();
        expected_client.push(super::PRE_VERSION);
        expected_client.extend_from_slice(&challenge);
        assert_eq!(client_preface.buffer, expected_client);
        assert_eq!(client_preface.flushes, 1);
        let mut server_preface = TrackingWrite::new();
        super::write_server_pre_handshake_header(&mut server_preface)
            .await
            .expect("tracking server preface write");
        let mut expected_server = super::PRE_MAGIC.to_vec();
        expected_server.push(super::PRE_VERSION);
        assert_eq!(server_preface.buffer, expected_server);
        assert_eq!(server_preface.flushes, 1);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn v3_delegation_and_mandatory_soranet_kem_complete_full_duplex() {
        let network_id = test_network_id("delegation-full-handshake-chain");
        let client_node = delegation_test_key(0xB5, Algorithm::BlsNormal);
        let server_node = delegation_test_key(0xB6, Algorithm::BlsNormal);
        let server_id = iroha_data_model::peer::PeerId::from(server_node.public_key().clone());
        let server_transport = delegation_test_key(0xB7, Algorithm::Ed25519);
        let soranet = Arc::new(SoranetHandshakeConfig::defaults());
        let (client_stream, server_stream) = tokio::io::duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, server_write) = tokio::io::split(server_stream);
        let connected_to = ConnectedTo::for_transport_delegation_test(
            "127.0.0.1:1337".parse().expect("client address"),
            server_id,
            client_node,
            Connection::from_split(45, client_read, client_write),
            network_id.clone(),
            Arc::clone(&soranet),
        );
        let connected_from = ConnectedFrom {
            our_public_address: "127.0.0.1:1338".parse().expect("server address"),
            key_pair: server_node,
            soranet_transport_key_pair: server_transport,
            connection: Connection::from_split(46, server_read, server_write),
            network_id,
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            soranet_handshake: soranet,
            local_scion_supported: true,
            trust_gossip: true,
            relay_role: RelayRole::Disabled,
        };
        let (outbound, inbound) = tokio::join!(
            ConnectedTo::send_client_hello::<ChaCha20Poly1305>(connected_to),
            ConnectedFrom::read_client_hello::<ChaCha20Poly1305>(connected_from),
        );
        let outbound = outbound.expect("initiator v3 + SoraNet handshake");
        let inbound = inbound.expect("responder v3 + SoraNet handshake");
        assert_eq!(
            outbound.soranet_transport_binding, inbound.soranet_transport_binding,
            "both sides must bind the same exact signed delegation frame"
        );
        assert_eq!(
            outbound.cryptographer.session_binding, inbound.cryptographer.session_binding,
            "both sides must derive the same mandatory SoraNet ML-KEM session"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn invalid_v3_delegation_stops_connected_to_before_puzzle_or_kem_bytes() {
        use tokio::io::AsyncReadExt;
        let network_id = test_network_id("delegation-order-chain");
        let remote_node = delegation_test_key(0xA4, Algorithm::BlsNormal);
        let remote_node_id = iroha_data_model::peer::PeerId::from(remote_node.public_key().clone());
        let remote_transport = delegation_test_key(0xA5, Algorithm::Ed25519);
        let local_node = delegation_test_key(0xA6, Algorithm::BlsNormal);
        let server_chain = network_id.clone();
        let (client_stream, mut server_stream) = tokio::io::duplex(2048);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let server_task = tokio::spawn(async move {
            let received = super::read_and_verify_client_pre_handshake_header(&mut server_stream)
                .await
                .expect("client must send an exact v3 preface");
            super::write_server_pre_handshake_header(&mut server_stream)
                .await
                .expect("server confirmation");
            let mut replay_challenge = received;
            replay_challenge[0] ^= 1;
            let replay = signed_delegation(
                &remote_node,
                &remote_transport,
                &server_chain,
                replay_challenge,
            );
            super::write_soranet_transport_delegation_v3(
                &mut server_stream,
                &replay.canonical_signed_frame,
            )
            .await
            .expect("captured-frame simulation write");
            let post_delegation_read =
                tokio::time::timeout(Duration::from_secs(1), server_stream.read_u8()).await;
            (received, replay_challenge, post_delegation_read)
        });
        let connected = ConnectedTo::for_transport_delegation_test(
            "127.0.0.1:1337".parse().expect("local test address"),
            remote_node_id,
            local_node,
            Connection::from_split(44, client_read, client_write),
            network_id,
            Arc::new(SoranetHandshakeConfig::defaults()),
        );
        let error = match ConnectedTo::send_client_hello::<ChaCha20Poly1305>(connected).await {
            Err(error) => error,
            Ok(_) => panic!("a replayed delegation must stop the handshake"),
        };
        let (expected, found, post_delegation_read) = server_task
            .await
            .expect("malicious server task must finish");
        assert!(matches!(
            unwrap_delegation_error(error),
            crate::SoranetTransportDelegationError::ChallengeMismatch {
                expected: actual_expected,
                found: actual_found,
            } if actual_expected == expected && actual_found == found
        ));
        assert!(
            matches!(
                post_delegation_read,
                Ok(Err(error)) if error.kind() == std::io::ErrorKind::UnexpectedEof
            ),
            "the client must close without writing a puzzle ticket, client hello, or KEM bytes"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn soranet_transport_delegation_v3_bad_header_fails_before_challenge_read() {
        use tokio::io::AsyncWriteExt;
        let (mut sender, mut receiver) = tokio::io::duplex(5);
        sender.write_all(super::PRE_MAGIC).await.expect("magic");
        sender.write_all(&[2]).await.expect("legacy version");
        sender.shutdown().await.expect("shutdown");
        let error = super::read_and_verify_client_pre_handshake_header(&mut receiver)
            .await
            .expect_err("v2 must fail without fallback");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        let truncated_challenge = delegation_test_challenge(0xC1);
        let (mut sender, mut receiver) = tokio::io::duplex(64);
        sender.write_all(super::PRE_MAGIC).await.expect("magic");
        sender
            .write_all(&[super::PRE_VERSION])
            .await
            .expect("v3 version");
        sender
            .write_all(&truncated_challenge[..truncated_challenge.len() - 1])
            .await
            .expect("truncated challenge");
        sender.shutdown().await.expect("shutdown");
        let error = super::read_and_verify_client_pre_handshake_header(&mut receiver)
            .await
            .expect_err("a truncated v3 challenge must fail closed");
        assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn soranet_transport_delegation_v3_writer_rejects_empty_and_oversize() {
        let mut rejecting_writer = TrackingWrite::new();
        let error = super::write_soranet_transport_delegation_v3(&mut rejecting_writer, &[])
            .await
            .expect_err("empty frame must fail closed");
        assert!(matches!(
            unwrap_delegation_error(error),
            crate::SoranetTransportDelegationError::EmptyFrame
        ));
        assert!(rejecting_writer.buffer.is_empty());
        let oversized_len = super::MAX_SORANET_TRANSPORT_DELEGATION_FRAME_BYTES + 1;
        let oversized = vec![0xA5; oversized_len];
        let error = super::write_soranet_transport_delegation_v3(&mut rejecting_writer, &oversized)
            .await
            .expect_err("oversized frame must fail closed");
        assert!(matches!(
            unwrap_delegation_error(error),
            crate::SoranetTransportDelegationError::FrameTooLarge { found, max }
                if found == oversized_len
                    && max == super::MAX_SORANET_TRANSPORT_DELEGATION_FRAME_BYTES
        ));
        assert!(rejecting_writer.buffer.is_empty());
        assert_eq!(rejecting_writer.flushes, 0);
    }
    #[test]
    fn final_handshake_payload_binds_every_v3_delegation_frame_bit() {
        let network_id = test_network_id("delegation-binding-chain");
        let node = delegation_test_key(0xB1, Algorithm::BlsNormal);
        let node_id = iroha_data_model::peer::PeerId::from(node.public_key().clone());
        let challenge = delegation_test_challenge(0xB2);
        let frame = signed_delegation(
            &node,
            &delegation_test_key(0xB3, Algorithm::Ed25519),
            &network_id,
            challenge,
        );
        let cryptographer = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[0xB4; 32])
            .expect("valid deterministic session key");
        let hello = unsigned_handshake_hello(
            &node,
            "127.0.0.1:1337".parse().expect("valid fixture address"),
        );
        let canonical_payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &frame.binding,
            None,
        );
        let mut mutated_signed = decode_signed_delegation(&frame.canonical_signed_frame);
        mutated_signed.node_signature[0] ^= 1;
        let mutated_frame = encode_signed_delegation(&mutated_signed);
        let mutated_binding = super::soranet_transport_delegation_binding_v3(&mutated_frame);
        assert_ne!(frame.binding, mutated_binding);
        let mutated_payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &mutated_binding,
            None,
        );
        assert_ne!(canonical_payload, mutated_payload);
        assert!(matches!(
            super::verify_soranet_transport_delegation_v3(
                &mutated_frame,
                &network_id,
                &node_id,
                &challenge,
            ),
            Err(crate::SoranetTransportDelegationError::InvalidNodeSignature)
        ));
    }
    fn sample_consensus_config_caps() -> ConsensusConfigCaps {
        ConsensusConfigCaps {
            execution_policy_hash: [0xB4; 32],
            nexus_policy_digest: [0xA5; 32],
            v2_config_fingerprint: [0xC3; 32],
            ivm_gas_schedule_hash: [0xE7; 32],
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn soranet_handshake_accepts_bls_peer_identities() {
        let soranet = Arc::new(SoranetHandshakeConfig::defaults());
        let outbound_keys = KeyPair::try_from_seed(vec![0x81; 32], Algorithm::BlsNormal)
            .expect("derive outbound BLS peer identity");
        let inbound_keys = KeyPair::try_from_seed(vec![0x82; 32], Algorithm::BlsNormal)
            .expect("derive inbound BLS peer identity");
        let inbound_transport_keys = KeyPair::try_from_seed(vec![0x83; 32], Algorithm::Ed25519)
            .expect("derive inbound Ed25519 SoraNet transport identity");
        let outbound_addr: SocketAddr = "127.0.0.1:10011".parse().unwrap();
        let inbound_addr: SocketAddr = "127.0.0.1:10012".parse().unwrap();
        let expected_inbound_id =
            iroha_data_model::prelude::PeerId::from(inbound_keys.public_key().clone());
        let (outbound_stream, inbound_stream) = tokio::io::duplex(64 * 1024);
        let (outbound_read, outbound_write) = tokio::io::split(outbound_stream);
        let (inbound_read, inbound_write) = tokio::io::split(inbound_stream);
        let outbound = ConnectedTo::for_transport_delegation_test(
            outbound_addr,
            expected_inbound_id,
            outbound_keys,
            Connection::from_split(101, outbound_read, outbound_write),
            test_network_id("bls-soranet-test-chain"),
            soranet.clone(),
        );
        let inbound = ConnectedFrom {
            our_public_address: inbound_addr,
            key_pair: inbound_keys,
            soranet_transport_key_pair: inbound_transport_keys,
            connection: Connection::from_split(102, inbound_read, inbound_write),
            network_id: test_network_id("bls-soranet-test-chain"),
            consensus_caps: None,
            confidential_caps: None,
            crypto_caps: None,
            soranet_handshake: soranet,
            local_scion_supported: true,
            trust_gossip: true,
            relay_role: RelayRole::Disabled,
        };
        let (outbound_result, inbound_result) = tokio::join!(
            ConnectedTo::send_client_hello::<ChaCha20Poly1305>(outbound),
            ConnectedFrom::read_client_hello::<ChaCha20Poly1305>(inbound),
        );
        outbound_result.expect("outbound BLS SoraNet handshake");
        inbound_result.expect("inbound BLS SoraNet handshake");
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
            network_id: test_network_id("test-chain"),
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
        let challenge = delegation_test_challenge(0xC7);
        super::write_client_pre_handshake_header(&mut writer, &challenge)
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
        expected.extend_from_slice(&challenge);
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
        let sender_payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &TEST_SORANET_TRANSPORT_BINDING,
            None,
        );
        let receiver_payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &TEST_SORANET_TRANSPORT_BINDING,
            None,
        );
        assert_eq!(sender_payload, receiver_payload);
    }
    #[test]
    fn handshake_signature_payload_rejects_same_name_different_genesis() {
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let mut hello_a = unsigned_handshake_hello(&KeyPair::random(), addr);
        let cryptographer = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[0x5A; 32])
            .expect("valid key length");
        let display_name = iroha_data_model::ChainId::from("shared-display-name");
        let network_a = test_network_id("handshake-genesis-a");
        let network_b = test_network_id("handshake-genesis-b");
        hello_a.network_id = network_a;
        let mut hello_b = hello_a.clone();
        hello_b.network_id = network_b;
        let payload_a = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello_a,
            &TEST_SORANET_TRANSPORT_BINDING,
            None,
        );
        let payload_b = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello_b,
            &TEST_SORANET_TRANSPORT_BINDING,
            None,
        );
        assert_ne!(payload_a, payload_b);
        assert_eq!(display_name.as_str(), "shared-display-name");
    }
    #[test]
    fn handshake_signature_payload_binds_the_full_session_hash() {
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let hello = unsigned_handshake_hello(&KeyPair::random(), addr);
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
            &TEST_SORANET_TRANSPORT_BINDING,
            None,
        );
        let changed = handshake_signature_payload::<ChaCha20Poly1305>(
            &same_compact_prefix,
            &hello,
            &TEST_SORANET_TRANSPORT_BINDING,
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
        let expected = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &TEST_SORANET_TRANSPORT_BINDING,
            None,
        );
        let mut changed = hello.clone();
        changed.relay = RelayRole::Hub;
        assert_ne!(
            expected,
            handshake_signature_payload::<ChaCha20Poly1305>(
                &cryptographer,
                &changed,
                &TEST_SORANET_TRANSPORT_BINDING,
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
                &TEST_SORANET_TRANSPORT_BINDING,
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
                &TEST_SORANET_TRANSPORT_BINDING,
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
                &TEST_SORANET_TRANSPORT_BINDING,
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
                &TEST_SORANET_TRANSPORT_BINDING,
                None,
            ),
            "trust and transport capabilities must be authenticated"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn handshake_rejects_capabilities_changed_after_signing() {
        let key_pair = KeyPair::random();
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let cryptographer = Cryptographer::<ChaCha20Poly1305>::new_with_raw_key_bytes(&[0x4E; 32])
            .expect("valid key length");
        let mut hello = unsigned_handshake_hello(&key_pair, addr);
        let payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &TEST_SORANET_TRANSPORT_BINDING,
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
    async fn handshake_rejects_same_name_peer_from_a_different_genesis() {
        let addr: SocketAddr = "127.0.0.1:1337".parse().unwrap();
        let display_name = iroha_data_model::ChainId::from("shared-display-name");
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
            network_id: test_network_id("genesis-a"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("genesis-b"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            Ok(_) => panic!("a handshake from a different genesis must be rejected"),
            Err(error) => error,
        };
        sender
            .await
            .expect("sender task panicked")
            .expect("sending handshake should succeed");
        assert!(matches!(
            error,
            crate::Error::HandshakeNetworkMismatch { expected, found }
                if expected == test_network_id("genesis-b")
                    && found == test_network_id("genesis-a")
        ));
        assert_eq!(display_name.as_str(), "shared-display-name");
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
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
            network_id: test_network_id("test-chain"),
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
                crate::Error::NoritoCodec(
                    norito::core::Error::SequenceLengthExceeded { .. }
                        | norito::core::Error::TotalElementsExceeded { .. }
                        | norito::core::Error::TotalAllocationExceeded { .. }
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            let mut hello = unsigned_handshake_hello(&key_pair, addr);
            let payload = handshake_signature_payload::<ChaCha20Poly1305>(
                &cryptographer,
                &hello,
                &TEST_SORANET_TRANSPORT_BINDING,
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
                network_id: test_network_id("test-chain"),
                soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
        let mut hello = unsigned_handshake_hello(&key_pair, addr);
        let payload = handshake_signature_payload::<ChaCha20Poly1305>(
            &cryptographer,
            &hello,
            &TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
            network_id: test_network_id("test-chain"),
            soranet_transport_binding: TEST_SORANET_TRANSPORT_BINDING,
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
        let key_pair = KeyPair::try_from_seed(vec![0xD4; 32], Algorithm::BlsNormal)
            .expect("test BLS-normal node key");
        let soranet_transport_key_pair = KeyPair::try_from_seed(vec![0xE5; 32], Algorithm::Ed25519)
            .expect("test Ed25519 transport key");
        let network_id = test_network_id("test-chain");
        let our_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let (r, w) = tokio::io::split(a);
        let conn = Connection::from_split(1, r, w);
        let soranet = Arc::new(SoranetHandshakeConfig::defaults());
        let cf = ConnectedFrom {
            our_public_address: our_addr,
            key_pair,
            soranet_transport_key_pair,
            connection: conn,
            network_id,
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
        assert!(matches!(err, crate::Error::HandshakeBadPreface));
    }
}
// handshake payload is encoded/decoded as a tuple to avoid extra type definitions
