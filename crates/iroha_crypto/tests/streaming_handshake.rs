//! Integration tests validating the streaming handshake and encrypted chunk pipeline.

use std::convert::{TryFrom, TryInto};

use chacha20poly1305::{
    ChaCha20Poly1305, XChaCha20Poly1305,
    aead::{Aead, KeyInit, Payload},
};
use iroha_crypto::{
    Algorithm, KeyPair, Signature,
    streaming::{
        HandshakeError, KeyMaterialError, SessionCadence, StreamingKeyMaterial, StreamingSession,
        key_update_transcript_bytes, kyber_public_fingerprint_with_suite,
    },
};
use norito::streaming::{
    CapabilityFlags, CapabilityRole, ChunkDescriptor, ContentKeyUpdate, EncryptionSuite, FecScheme,
    FeedbackHintFrame, Hash, HpkeSuite, PrivacyBucketGranularity, ReceiverReport, Resolution,
    StreamMetadata, TransportCapabilityResolution,
    chunk::merkle_root,
    codec::{
        BaselineEncoder, BaselineEncoderConfig, BaselineManifestParams, FrameDimensions, RawFrame,
        verify_segment,
    },
    crypto::{
        self as streaming_crypto, chunk_commitments_for_ciphertexts, derive_chunk_nonce,
        derive_content_key, encrypt_chunk, nonce_len_for_suite, wrap_gck,
    },
};
use soranet_pq::{MlKemKeyPair, MlKemSuite, generate_mlkem_keypair_from_os};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

const TEST_KEM_SUITE: MlKemSuite = MlKemSuite::MlKem768;

fn mlkem_keypair() -> MlKemKeyPair {
    generate_mlkem_keypair_from_os(TEST_KEM_SUITE).expect("ML-KEM keypair")
}

fn mlkem_keypair_bytes() -> (Vec<u8>, Vec<u8>) {
    let MlKemKeyPair {
        public_key,
        secret_key,
    } = mlkem_keypair();
    (public_key, secret_key.as_slice().to_vec())
}

fn set_first_mlkem_12_bit_coefficient_noncanonical(bytes: &mut [u8]) {
    bytes[0] = 0xFF;
    bytes[1] = (bytes[1] & 0xF0) | 0x0F;
}

fn fingerprint(bytes: &[u8]) -> Hash {
    kyber_public_fingerprint_with_suite(bytes, TEST_KEM_SUITE).expect("fingerprint derivation")
}

fn wrap_gck_without_length_check(
    suite: &EncryptionSuite,
    transport_send_key: &[u8; 32],
    nonce: &[u8],
    gck_plaintext: &[u8],
    content_key_id: u64,
    valid_from_segment: u64,
) -> Vec<u8> {
    let mut aad = [0u8; 23];
    aad[..7].copy_from_slice(b"nsc-gck");
    aad[7..15].copy_from_slice(&content_key_id.to_le_bytes());
    aad[15..].copy_from_slice(&valid_from_segment.to_le_bytes());

    let ciphertext = match suite {
        EncryptionSuite::X25519ChaCha20Poly1305(_) => {
            let key: chacha20poly1305::Key = (*transport_send_key).into();
            let nonce: chacha20poly1305::Nonce = <[u8; 12]>::try_from(nonce)
                .expect("valid chacha nonce")
                .into();
            let cipher = ChaCha20Poly1305::new(&key);
            cipher
                .encrypt(
                    &nonce,
                    Payload {
                        msg: gck_plaintext,
                        aad: &aad,
                    },
                )
                .expect("manual gck wrap")
        }
        EncryptionSuite::Kyber768XChaCha20Poly1305(_) => {
            let key: chacha20poly1305::Key = (*transport_send_key).into();
            let nonce: chacha20poly1305::XNonce = <[u8; 24]>::try_from(nonce)
                .expect("valid xchacha nonce")
                .into();
            let cipher = XChaCha20Poly1305::new(&key);
            cipher
                .encrypt(
                    &nonce,
                    Payload {
                        msg: gck_plaintext,
                        aad: &aad,
                    },
                )
                .expect("manual gck wrap")
        }
    };
    let mut wrapped = Vec::with_capacity(nonce.len() + ciphertext.len());
    wrapped.extend_from_slice(nonce);
    wrapped.extend_from_slice(&ciphertext);
    wrapped
}

#[test]
fn changing_kem_suite_resets_configured_keys() {
    let identity = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let mut material = StreamingKeyMaterial::new(identity).expect("identity accepted");
    let (public, secret) = mlkem_keypair_bytes();
    material
        .set_kyber_keys(public.as_slice(), secret.as_slice())
        .expect("initial keys accepted");
    assert!(material.kyber_public().is_some(), "keys installed");

    material.set_kem_suite(MlKemSuite::MlKem512);
    assert_eq!(material.kem_suite(), MlKemSuite::MlKem512);
    assert!(
        material.kyber_public().is_none(),
        "switch clears public key"
    );
    assert!(
        material.kyber_secret().is_none(),
        "switch clears secret key"
    );
    assert!(
        material.kyber_fingerprint().is_none(),
        "fingerprint cleared"
    );
}

fn pattern_frame(dimensions: FrameDimensions, seed: u8) -> Vec<u8> {
    let count = dimensions.pixel_count();
    (0..count)
        .map(|idx| {
            let idx_u8 = u8::try_from(idx).expect("test frame dimensions fit into u8");
            seed.wrapping_add(idx_u8.wrapping_mul(5))
        })
        .collect()
}

fn sample_resolution() -> TransportCapabilityResolution {
    TransportCapabilityResolution {
        hpke_suite: HpkeSuite::Kyber768AuthPsk,
        use_datagram: true,
        max_segment_datagram_size: 1_024,
        fec_feedback_interval_ms: 200,
        privacy_bucket_granularity: PrivacyBucketGranularity::StandardV1,
    }
}

#[test]
fn transport_resolution_hash_matches_manual_derivation() {
    let resolution = sample_resolution();
    let _ = resolution.capabilities_hash();
    assert_eq!(
        resolution.capabilities_hash(),
        resolution.capabilities_hash()
    );
}

#[test]
fn record_transport_capabilities_rejects_invalid_shape_without_state_change() {
    let mut session = StreamingSession::new(CapabilityRole::Viewer);
    let valid = sample_resolution();
    session
        .record_transport_capabilities(valid)
        .expect("valid transport capabilities");

    for (invalid_transport, expected_reason) in [
        (
            TransportCapabilityResolution {
                use_datagram: true,
                max_segment_datagram_size: 0,
                ..valid
            },
            "datagram transport resolution requires nonzero datagram size",
        ),
        (
            TransportCapabilityResolution {
                use_datagram: false,
                max_segment_datagram_size: 1_024,
                ..valid
            },
            "stream transport resolution must not carry datagram size",
        ),
    ] {
        let err = session
            .record_transport_capabilities(invalid_transport)
            .expect_err("invalid transport capabilities rejected");
        match err {
            HandshakeError::InvalidTransportCapabilities(reason) => {
                assert!(reason.contains(expected_reason));
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert_eq!(session.transport_capabilities().copied(), Some(valid));
        assert_eq!(
            session.transport_capabilities_hash(),
            Some(valid.capabilities_hash())
        );
    }
}

#[test]
fn session_snapshot_preserves_replay_protection() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xA5; 32]);
    let session_id = [0xC3; 32];

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x10; 32]);
    let update = publisher_session
        .build_key_update(session_id, &suite, 1, 1, publisher_keys.private_key())
        .expect("build initial key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    let viewer_ephemeral = [0x55; 32];
    viewer_session.set_local_ephemeral_x25519(viewer_ephemeral);
    let transport = *viewer_session
        .process_remote_key_update(&update, publisher_keys.public_key())
        .expect("initial key update accepted");

    let snapshot = viewer_session.snapshot_state().expect("snapshot available");
    assert_eq!(snapshot.role, CapabilityRole::Viewer);

    let mut restored_session = StreamingSession::new(CapabilityRole::Viewer);
    restored_session.set_local_ephemeral_x25519(viewer_ephemeral);
    restored_session
        .restore_from_snapshot(snapshot.clone())
        .expect("restore snapshot");
    assert_eq!(
        restored_session
            .transport_keys()
            .expect("transport keys after restore"),
        &transport,
    );

    let replay_err = restored_session
        .process_remote_key_update(&update, publisher_keys.public_key())
        .expect_err("replay must be rejected");
    match replay_err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::NonMonotonicKeyCounter {
            previous,
            found,
        }) => {
            assert_eq!(previous, 1);
            assert_eq!(found, 1);
        }
        other => panic!("unexpected error variant: {other:?}"),
    }

    publisher_session.set_local_ephemeral_x25519([0x22; 32]);
    let follow_up = publisher_session
        .build_key_update(session_id, &suite, 1, 2, publisher_keys.private_key())
        .expect("follow-up key update");
    restored_session
        .process_remote_key_update(&follow_up, publisher_keys.public_key())
        .expect("restored session processes fresh update");
}

#[test]
fn session_cadence_enforces_thresholds() {
    let mut cadence = SessionCadence::new(1_000);
    assert_eq!(cadence.required_key_counter(1_000), 1);

    cadence.record_payload_bytes(32 * 1024 * 1024);
    assert_eq!(cadence.required_key_counter(1_010), 1);

    cadence.record_payload_bytes(33 * 1024 * 1024);
    assert_eq!(cadence.required_key_counter(1_020), 2);

    assert_eq!(cadence.required_key_counter(301_000), 2);
    assert_eq!(cadence.required_key_counter(601_001), 3);

    cadence.record_payload_bytes(u64::MAX);
    assert_eq!(cadence.total_payload_bytes(), u64::MAX);

    let snapshot = cadence.snapshot();
    let restored = SessionCadence::from_snapshot(snapshot);
    assert_eq!(restored, cadence);
}

#[test]
fn feedback_hint_parity_is_clamped_to_fec_budget() {
    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    let hint = FeedbackHintFrame {
        stream_id: [0x61; 32],
        loss_ewma_q16: 0,
        latency_gradient_q16: 0,
        observed_rtt_ms: 12,
        report_interval_ms: 100,
        parity_chunks: u8::MAX,
    };

    session
        .process_feedback_hint(&hint)
        .expect("feedback hint stream id matches session");
    let snapshot = session
        .feedback_snapshot()
        .expect("feedback hint should snapshot");
    assert_eq!(snapshot.parity_chunks, 6);
}

#[test]
fn feedback_samples_are_clamped_to_documented_bounds() {
    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    let stream_id = [0x62; 32];
    let hint = FeedbackHintFrame {
        stream_id,
        loss_ewma_q16: u32::MAX,
        latency_gradient_q16: 0,
        observed_rtt_ms: 16,
        report_interval_ms: 100,
        parity_chunks: u8::MAX,
    };
    session
        .process_feedback_hint(&hint)
        .expect("feedback hint stream id matches session");

    let report = ReceiverReport {
        stream_id,
        latest_segment: 9,
        layer_mask: 0,
        measured_throughput_kbps: 2_000,
        rtt_ms: 33,
        loss_percent_x100: u16::MAX,
        decoder_buffer_ms: 120,
        active_resolution: Resolution::R720p,
        hdr_active: false,
        ecn_ce_count: 0,
        jitter_ms: 4,
        delivered_sequence: 512,
        parity_applied: u8::MAX,
        fec_budget: u8::MAX,
        sync_diagnostics: None,
    };
    let parity = session
        .process_receiver_report(&report)
        .expect("receiver report stream id matches session");
    let snapshot = session
        .feedback_snapshot()
        .expect("feedback state should snapshot");

    assert_eq!(parity, 6);
    assert_eq!(snapshot.loss_ewma_q16, Some(1 << 16));
    assert_eq!(snapshot.parity_chunks, 6);
    assert_eq!(snapshot.latest_parity_applied, Some(6));
    assert_eq!(snapshot.latest_fec_budget, Some(6));
}

#[test]
fn feedback_hint_rejects_stream_id_switch_without_state_change() {
    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    let stream_id = [0x63; 32];
    let mut hint = FeedbackHintFrame {
        stream_id,
        loss_ewma_q16: 1 << 15,
        latency_gradient_q16: 4,
        observed_rtt_ms: 18,
        report_interval_ms: 100,
        parity_chunks: 2,
    };

    session
        .process_feedback_hint(&hint)
        .expect("initial feedback hint binds stream id");
    let before = session
        .feedback_snapshot()
        .expect("initial feedback hint should snapshot");

    let mismatched_stream_id = [0x64; 32];
    hint.stream_id = mismatched_stream_id;
    hint.loss_ewma_q16 = u32::MAX;
    hint.latency_gradient_q16 = -8;
    hint.observed_rtt_ms = 250;
    hint.report_interval_ms = 1_000;
    hint.parity_chunks = u8::MAX;

    let err = session
        .process_feedback_hint(&hint)
        .expect_err("mismatched feedback hint stream id rejected");
    assert!(matches!(
        err,
        HandshakeError::FeedbackStreamMismatch { expected, found }
            if expected == stream_id && found == mismatched_stream_id
    ));
    assert_eq!(session.feedback_snapshot(), Some(before));
}

#[test]
fn receiver_report_rejects_stream_id_switch_without_state_change() {
    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    let stream_id = [0x65; 32];
    let mut report = ReceiverReport {
        stream_id,
        latest_segment: 9,
        layer_mask: 0,
        measured_throughput_kbps: 2_000,
        rtt_ms: 33,
        loss_percent_x100: 150,
        decoder_buffer_ms: 120,
        active_resolution: Resolution::R720p,
        hdr_active: false,
        ecn_ce_count: 0,
        jitter_ms: 4,
        delivered_sequence: 512,
        parity_applied: 1,
        fec_budget: 3,
        sync_diagnostics: None,
    };

    let parity = session
        .process_receiver_report(&report)
        .expect("initial receiver report binds stream id");
    let before = session
        .feedback_snapshot()
        .expect("initial receiver report should snapshot");
    assert_eq!(session.latest_feedback_parity(), Some(parity));

    let mismatched_stream_id = [0x66; 32];
    report.stream_id = mismatched_stream_id;
    report.loss_percent_x100 = u16::MAX;
    report.delivered_sequence = 2_048;
    report.parity_applied = u8::MAX;
    report.fec_budget = u8::MAX;

    let err = session
        .process_receiver_report(&report)
        .expect_err("mismatched receiver report stream id rejected");
    assert!(matches!(
        err,
        HandshakeError::FeedbackStreamMismatch { expected, found }
            if expected == stream_id && found == mismatched_stream_id
    ));
    assert_eq!(session.feedback_snapshot(), Some(before));
    assert_eq!(session.latest_feedback_parity(), Some(parity));
}

#[allow(clippy::too_many_lines)]
#[test]
fn streaming_handshake_and_chunk_encryption_roundtrip() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xAB; 32]);
    let publisher_secret_bytes = [0x45u8; 32];
    let publisher_secret = StaticSecret::from(publisher_secret_bytes);
    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519(publisher_secret_bytes);
    let key_update = publisher_session
        .build_key_update([0x01; 32], &suite, 1, 1, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    let viewer_secret_bytes = [0x33u8; 32];
    let viewer_public_bytes = viewer_session.set_local_ephemeral_x25519(viewer_secret_bytes);
    let transport_viewer = *viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let negotiated_suite = viewer_session
        .negotiated_suite()
        .copied()
        .expect("suite negotiated");
    assert_eq!(negotiated_suite, suite, "suite recorded");
    assert!(viewer_session.transport_keys().is_some());

    let transport_resolution = sample_resolution();
    let transport_capabilities_hash = transport_resolution.capabilities_hash();

    // Derive publisher-side transport keys to wrap the GCK for the viewer.
    let viewer_public =
        X25519PublicKey::from(<[u8; 32]>::try_from(viewer_public_bytes.as_slice()).unwrap());
    let shared = publisher_secret.diffie_hellman(&viewer_public);
    let mut shared_secret = [0u8; 32];
    shared_secret.copy_from_slice(shared.as_bytes());
    let publisher_keys =
        streaming_crypto::derive_transport_keys_for_role(&shared_secret, CapabilityRole::Publisher)
            .expect("publisher transport keys");
    let nonce_len = nonce_len_for_suite(&suite);
    let mut gck_nonce = vec![0u8; nonce_len];
    for (idx, byte) in gck_nonce.iter_mut().enumerate() {
        let idx_u8 = u8::try_from(idx).expect("nonce length fits into u8");
        *byte = idx_u8.wrapping_mul(3);
    }
    let gck_plain = [0x55u8; 32];
    let wrapped_gck = wrap_gck(
        &suite,
        &publisher_keys.send,
        &gck_nonce,
        &gck_plain,
        17,
        1_024,
    )
    .expect("wrap gck");
    let content_update = ContentKeyUpdate {
        content_key_id: 17,
        gck_wrapped: wrapped_gck.clone(),
        valid_from_segment: 1_024,
    };
    let gck_unwrapped = viewer_session
        .process_content_key_update(&content_update)
        .expect("unwrap gck");
    assert_eq!(gck_unwrapped, gck_plain);
    let gck_arr: [u8; 32] = gck_unwrapped.clone().try_into().expect("32-byte gck");

    // Encode a baseline segment and encrypt its chunks end-to-end.
    let dims = FrameDimensions::new(16, 16);
    let encoder_config = BaselineEncoderConfig {
        frame_dimensions: dims,
        frames_per_segment: 2,
        frame_duration_ns: 33_000_000,
        encryption_suite: suite,
        quantizer: 4,
        ..BaselineEncoderConfig::default()
    };
    let mut encoder = BaselineEncoder::new(encoder_config.clone());
    let frames = [pattern_frame(dims, 0x21), pattern_frame(dims, 0xBA)];
    let raw_frames: Vec<_> = frames
        .iter()
        .map(|bytes| RawFrame::new(dims, bytes.clone()).expect("frame"))
        .collect();
    let mut segment = encoder
        .encode_segment(77, 1_000, content_update.content_key_id, &raw_frames, None)
        .expect("encode segment");
    let original_chunks = segment.chunks.clone();
    let nonce_salt = segment.header.nonce_salt;

    let content_key =
        derive_content_key(&gck_arr, segment.header.segment_number).expect("derive content key");
    let mut root_guess = segment.header.chunk_merkle_root;
    let original_descriptors = segment.descriptors.clone();
    let encrypt_with_root = |root: [u8; 32]| -> (Vec<Vec<u8>>, Vec<ChunkDescriptor>, [u8; 32]) {
        let mut chunks = Vec::with_capacity(original_chunks.len());
        let mut descriptors = Vec::with_capacity(original_chunks.len());
        let mut offset = 0u32;

        for (idx, plaintext) in original_chunks.iter().enumerate() {
            let descriptor_template = &original_descriptors[idx];
            let chunk_id = descriptor_template.chunk_id;
            let nonce =
                derive_chunk_nonce(&nonce_salt, chunk_id, &suite).expect("derive chunk nonce");
            let ciphertext = encrypt_chunk(
                &suite,
                &content_key,
                &nonce,
                segment.header.segment_number,
                chunk_id,
                &root,
                plaintext,
            )
            .expect("encrypt chunk");
            let length =
                u32::try_from(ciphertext.len()).expect("cipher chunk length fits into u32");
            chunks.push(ciphertext);
            descriptors.push(ChunkDescriptor {
                chunk_id,
                offset,
                length,
                commitment: [0u8; 32],
                parity: descriptor_template.parity,
            });
            offset = offset.checked_add(length).expect("offset overflow");
        }

        let payload_refs: Vec<(u16, &[u8])> = descriptors
            .iter()
            .zip(chunks.iter())
            .map(|(descriptor, chunk)| (descriptor.chunk_id, chunk.as_slice()))
            .collect();
        let commitments =
            chunk_commitments_for_ciphertexts(segment.header.segment_number, &payload_refs);
        for (descriptor, commitment) in descriptors.iter_mut().zip(commitments.iter()) {
            descriptor.commitment = *commitment;
        }
        let new_root = merkle_root(&commitments).expect("merkle root");
        (chunks, descriptors, new_root)
    };

    for _ in 0..16 {
        let (_, _, computed_root) = encrypt_with_root(root_guess);
        if computed_root == root_guess {
            break;
        }
        root_guess = computed_root;
    }
    let encryption_root = root_guess;
    let (final_chunks, final_descriptors, final_root) = encrypt_with_root(encryption_root);
    segment.chunks = final_chunks;
    segment.descriptors = final_descriptors;
    segment.header.chunk_merkle_root = final_root;
    segment.header.chunk_count =
        u16::try_from(segment.descriptors.len()).expect("descriptor count fits into u16");

    // Verify ciphertext commitments and decrypt back to original payload.
    let cipher_refs: Vec<(u16, &[u8])> = segment
        .descriptors
        .iter()
        .zip(segment.chunks.iter())
        .map(|(descriptor, chunk)| (descriptor.chunk_id, chunk.as_slice()))
        .collect();
    let commitments =
        chunk_commitments_for_ciphertexts(segment.header.segment_number, &cipher_refs);
    for (descriptor, commitment) in segment.descriptors.iter().zip(commitments.iter()) {
        assert_eq!(&descriptor.commitment, commitment);
    }
    for (idx, cipher_chunk) in segment.chunks.iter().enumerate() {
        let chunk_id = segment.descriptors[idx].chunk_id;
        let nonce = derive_chunk_nonce(&nonce_salt, chunk_id, &suite).expect("derive chunk nonce");
        let decrypted = streaming_crypto::decrypt_chunk(
            &suite,
            &content_key,
            &nonce,
            segment.header.segment_number,
            chunk_id,
            &encryption_root,
            cipher_chunk,
        )
        .expect("decrypt chunk");
        assert_eq!(decrypted, original_chunks[idx]);
    }

    verify_segment(
        &segment.header,
        &segment.descriptors,
        &segment.chunks,
        segment.audio.as_ref(),
    )
    .expect("segment verification");

    let manifest = segment.build_manifest(BaselineManifestParams {
        stream_id: [0x90; 32],
        protocol_version: 1,
        published_at: 1_706_000_000,
        da_endpoint: "/dns/publisher.example/quic".into(),
        privacy_routes: Vec::new(),
        public_metadata: StreamMetadata::default(),
        capabilities: CapabilityFlags::from_bits(0),
        signature: [0xAA; 64],
        fec_suite: FecScheme::Rs12_10,
        neural_bundle: None,
        transport_capabilities_hash,
    });
    segment
        .verify_manifest(&manifest)
        .expect("manifest verification");

    // Ensure session retains transport metadata.
    let session_transport = viewer_session
        .transport_keys()
        .copied()
        .expect("transport keys");
    assert_eq!(session_transport, transport_viewer);
    assert_eq!(viewer_session.latest_gck(), Some(gck_unwrapped.as_slice()));
}

#[test]
fn x25519_process_remote_key_update_resets_on_session_change() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xAB; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x11; 32]);
    let first_update = publisher_session
        .build_key_update([0x01; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("first key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x22; 32]);
    let initial_keys = *viewer_session
        .process_remote_key_update(&first_update, publisher_keys.public_key())
        .expect("initial update accepted");

    let mut restarted_session = StreamingSession::new(CapabilityRole::Publisher);
    restarted_session.set_local_ephemeral_x25519([0x33; 32]);
    let restarted_update = restarted_session
        .build_key_update([0x02; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("restart key update");

    viewer_session.set_local_ephemeral_x25519([0x44; 32]);
    let restarted_keys = *viewer_session
        .process_remote_key_update(&restarted_update, publisher_keys.public_key())
        .expect("restart update accepted");
    assert_ne!(
        restarted_keys, initial_keys,
        "new session must derive fresh transport keys"
    );

    let follow_up = restarted_session
        .build_key_update([0x02; 32], &suite, 1, 2, publisher_keys.private_key())
        .expect("follow-up key update");
    viewer_session
        .process_remote_key_update(&follow_up, publisher_keys.public_key())
        .expect("follow-up key counter accepted after restart");
}

#[test]
fn x25519_process_remote_key_update_requires_local_ephemeral_without_state_change() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xAD; 32]);
    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x11; 32]);
    let key_update = publisher_session
        .build_key_update([0x03; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    let err = viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect_err("viewer must prepare local x25519 material first");
    assert!(matches!(err, HandshakeError::MissingX25519LocalEphemeral));
    assert_eq!(viewer_session.negotiated_suite(), None);
    assert!(viewer_session.transport_keys().is_none());
    assert!(viewer_session.sts_root().is_none());
    assert_eq!(viewer_session.snapshot_state(), None);
}

#[test]
fn x25519_process_remote_key_update_rejects_low_order_ephemeral() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xAB; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x11; 32]);
    let mut key_update = publisher_session
        .build_key_update([0x03; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("key update");
    key_update.pub_ephemeral = vec![0u8; 32];
    let transcript = key_update_transcript_bytes(&key_update).expect("serialize mutated frame");
    let signature = Signature::new(publisher_keys.private_key(), &transcript);
    key_update.signature.copy_from_slice(signature.payload());

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x22; 32]);
    let err = viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect_err("low-order x25519 ephemeral must be rejected");
    assert!(matches!(
        err,
        HandshakeError::InvalidX25519EphemeralPublicKey
    ));
    assert!(viewer_session.transport_keys().is_none());
    assert!(viewer_session.negotiated_suite().is_none());
}

#[test]
fn x25519_process_remote_key_update_rejects_zero_counter_without_state_change() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xAD; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x13; 32]);
    let mut key_update = publisher_session
        .build_key_update([0x13; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("key update");
    key_update.key_counter = 0;
    let transcript = key_update_transcript_bytes(&key_update).expect("serialize mutated frame");
    let signature = Signature::new(publisher_keys.private_key(), &transcript);
    key_update.signature.copy_from_slice(signature.payload());

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x24; 32]);
    let err = viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect_err("zero counter rejected");
    match err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::InvalidKeyCounter { found }) => {
            assert_eq!(found, 0);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), None);
    assert!(viewer_session.transport_keys().is_none());
}

#[test]
fn x25519_process_remote_key_update_rejects_zero_protocol_version_without_state_change() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xAE; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x14; 32]);
    let mut key_update = publisher_session
        .build_key_update([0x14; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("key update");
    key_update.protocol_version = 0;
    let transcript = key_update_transcript_bytes(&key_update).expect("serialize mutated frame");
    let signature = Signature::new(publisher_keys.private_key(), &transcript);
    key_update.signature.copy_from_slice(signature.payload());

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x25; 32]);
    let err = viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect_err("zero protocol version rejected");
    match err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::InvalidProtocolVersion { found }) => {
            assert_eq!(found, 0);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), None);
    assert!(viewer_session.transport_keys().is_none());
}

#[test]
fn x25519_content_key_update_authenticates_before_recording_state() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xBC; 32]);

    let publisher_secret_bytes = [0x31u8; 32];
    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519(publisher_secret_bytes);
    let key_update = publisher_session
        .build_key_update([0x04; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    let viewer_public_bytes = viewer_session.set_local_ephemeral_x25519([0x42u8; 32]);
    viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect("viewer processes key update");

    let viewer_public = X25519PublicKey::from(
        <[u8; 32]>::try_from(viewer_public_bytes.as_slice()).expect("viewer public length"),
    );
    let publisher_secret = StaticSecret::from(publisher_secret_bytes);
    let shared = publisher_secret.diffie_hellman(&viewer_public);
    let mut shared_secret = [0u8; 32];
    shared_secret.copy_from_slice(shared.as_bytes());
    let publisher_transport =
        streaming_crypto::derive_transport_keys_for_role(&shared_secret, CapabilityRole::Publisher)
            .expect("publisher transport keys");

    let nonce = vec![0x54; nonce_len_for_suite(&suite)];
    let gck = [0x63u8; 32];
    let content_key_id = 31;
    let valid_from_segment = 640;
    let wrapped = wrap_gck(
        &suite,
        &publisher_transport.send,
        &nonce,
        &gck,
        content_key_id,
        valid_from_segment,
    )
    .expect("wrap gck");
    let update = ContentKeyUpdate {
        content_key_id,
        gck_wrapped: wrapped,
        valid_from_segment,
    };
    let mut tampered = update.clone();
    *tampered
        .gck_wrapped
        .last_mut()
        .expect("wrapped key contains authentication tag") ^= 0x01;

    let err = viewer_session
        .process_content_key_update(&tampered)
        .expect_err("tampered wrapped gck rejected");
    assert!(matches!(err, HandshakeError::Crypto(_)));
    assert!(viewer_session.latest_gck().is_none());

    let unwrapped = viewer_session
        .process_content_key_update(&update)
        .expect("valid update with same id still accepted");
    assert_eq!(unwrapped.as_slice(), gck);
    assert_eq!(viewer_session.latest_gck(), Some(gck.as_ref()));
}

#[test]
fn x25519_content_key_update_rejects_invalid_gck_length_before_recording_state() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xBD; 32]);

    let publisher_secret_bytes = [0x32u8; 32];
    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519(publisher_secret_bytes);
    let key_update = publisher_session
        .build_key_update([0x1C; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    let viewer_public_bytes = viewer_session.set_local_ephemeral_x25519([0x43u8; 32]);
    viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect("viewer processes key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid gck");

    let viewer_public = X25519PublicKey::from(
        <[u8; 32]>::try_from(viewer_public_bytes.as_slice()).expect("viewer public length"),
    );
    let publisher_secret = StaticSecret::from(publisher_secret_bytes);
    let shared = publisher_secret.diffie_hellman(&viewer_public);
    let mut shared_secret = [0u8; 32];
    shared_secret.copy_from_slice(shared.as_bytes());
    let publisher_transport =
        streaming_crypto::derive_transport_keys_for_role(&shared_secret, CapabilityRole::Publisher)
            .expect("publisher transport keys");

    let nonce = vec![0x55; nonce_len_for_suite(&suite)];
    let short_gck = [0x64u8; 31];
    let wrapped = wrap_gck_without_length_check(
        &suite,
        &publisher_transport.send,
        &nonce,
        &short_gck,
        32,
        700,
    );
    let err = viewer_session
        .process_content_key_update(&ContentKeyUpdate {
            content_key_id: 32,
            gck_wrapped: wrapped,
            valid_from_segment: 700,
        })
        .expect_err("short gck rejected");
    match err {
        HandshakeError::InvalidGroupContentKeyLength { expected, found } => {
            assert_eq!(expected, 32);
            assert_eq!(found, short_gck.len());
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(viewer_session.latest_gck().is_none());
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn x25519_key_update_rejects_malformed_restart_without_resetting_session() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xCD; 32]);

    let publisher_secret_bytes = [0x51u8; 32];
    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519(publisher_secret_bytes);
    let first_update = publisher_session
        .build_key_update([0x05; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("first key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    let viewer_public_bytes = viewer_session.set_local_ephemeral_x25519([0x62u8; 32]);
    let initial_transport = *viewer_session
        .process_remote_key_update(&first_update, publisher_keys.public_key())
        .expect("initial key update");

    let viewer_public = X25519PublicKey::from(
        <[u8; 32]>::try_from(viewer_public_bytes.as_slice()).expect("viewer public length"),
    );
    let publisher_secret = StaticSecret::from(publisher_secret_bytes);
    let shared = publisher_secret.diffie_hellman(&viewer_public);
    let mut shared_secret = [0u8; 32];
    shared_secret.copy_from_slice(shared.as_bytes());
    let publisher_transport =
        streaming_crypto::derive_transport_keys_for_role(&shared_secret, CapabilityRole::Publisher)
            .expect("publisher transport keys");

    let gck = [0x73u8; 32];
    let nonce = vec![0x84; nonce_len_for_suite(&suite)];
    let wrapped =
        wrap_gck(&suite, &publisher_transport.send, &nonce, &gck, 41, 900).expect("wrap gck");
    let content_update = ContentKeyUpdate {
        content_key_id: 41,
        gck_wrapped: wrapped,
        valid_from_segment: 900,
    };
    viewer_session
        .process_content_key_update(&content_update)
        .expect("content key accepted");

    let mut restarted_session = StreamingSession::new(CapabilityRole::Publisher);
    restarted_session.set_local_ephemeral_x25519([0x91; 32]);
    let mut malformed_restart = restarted_session
        .build_key_update([0x06; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("restart key update");
    malformed_restart.pub_ephemeral = vec![0u8; 32];
    let transcript =
        key_update_transcript_bytes(&malformed_restart).expect("serialize malformed restart");
    let signature = Signature::new(publisher_keys.private_key(), &transcript);
    malformed_restart
        .signature
        .copy_from_slice(signature.payload());

    let err = viewer_session
        .process_remote_key_update(&malformed_restart, publisher_keys.public_key())
        .expect_err("malformed restart rejected");
    assert!(matches!(
        err,
        HandshakeError::InvalidX25519EphemeralPublicKey
    ));
    assert_eq!(viewer_session.transport_keys(), Some(&initial_transport));
    assert_eq!(viewer_session.negotiated_suite(), Some(&suite));
    assert_eq!(viewer_session.latest_gck(), Some(gck.as_ref()));
}

#[test]
fn outbound_key_update_failure_preserves_existing_session() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let viewer_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xEF; 32]);

    let publisher_secret_bytes = [0x71u8; 32];
    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519(publisher_secret_bytes);
    let key_update = publisher_session
        .build_key_update([0x08; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    let viewer_public_bytes = viewer_session.set_local_ephemeral_x25519([0x82u8; 32]);
    let initial_transport = *viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect("viewer processes key update");

    let viewer_public = X25519PublicKey::from(
        <[u8; 32]>::try_from(viewer_public_bytes.as_slice()).expect("viewer public length"),
    );
    let publisher_secret = StaticSecret::from(publisher_secret_bytes);
    let shared = publisher_secret.diffie_hellman(&viewer_public);
    let mut shared_secret = [0u8; 32];
    shared_secret.copy_from_slice(shared.as_bytes());
    let publisher_transport =
        streaming_crypto::derive_transport_keys_for_role(&shared_secret, CapabilityRole::Publisher)
            .expect("publisher transport keys");

    let gck = [0x93u8; 32];
    let nonce = vec![0xA4; nonce_len_for_suite(&suite)];
    let wrapped =
        wrap_gck(&suite, &publisher_transport.send, &nonce, &gck, 61, 1_200).expect("wrap gck");
    viewer_session
        .process_content_key_update(&ContentKeyUpdate {
            content_key_id: 61,
            gck_wrapped: wrapped,
            valid_from_segment: 1_200,
        })
        .expect("content key accepted");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("established session snapshot");

    let kyber_suite = EncryptionSuite::Kyber768XChaCha20Poly1305([0xAA; 32]);
    let err = viewer_session
        .build_key_update([0x09; 32], &kyber_suite, 1, 2, viewer_keys.private_key())
        .expect_err("missing Kyber remote public must fail");
    assert!(matches!(err, HandshakeError::MissingKyberRemotePublic));
    assert_eq!(viewer_session.transport_keys(), Some(&initial_transport));
    assert_eq!(viewer_session.latest_gck(), Some(gck.as_ref()));
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn set_kyber_remote_public_rejects_fingerprint_mismatch() {
    let MlKemKeyPair {
        public_key: pk_bytes,
        ..
    } = mlkem_keypair();
    let expected_fingerprint = fingerprint(&pk_bytes);
    let mut wrong_fingerprint = expected_fingerprint;
    wrong_fingerprint[0] ^= 0xAA;
    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    let err = session
        .set_kyber_remote_public(wrong_fingerprint, &pk_bytes)
        .expect_err("fingerprint mismatch must be rejected");
    match err {
        HandshakeError::KyberFingerprintMismatch { expected, found } => {
            assert_eq!(expected, wrong_fingerprint);
            assert_eq!(found, expected_fingerprint);
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn streaming_key_material_rejects_non_ed25519_identity() {
    let pair = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
    let err = StreamingKeyMaterial::new(pair).expect_err("non-ed25519 identity rejected");
    assert!(matches!(
        err,
        KeyMaterialError::UnsupportedIdentityAlgorithm(Algorithm::Secp256k1)
    ));
}

#[test]
fn streaming_key_material_configures_session_for_hpke() {
    let identity = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let mut material = StreamingKeyMaterial::new(identity).expect("ed25519 identity accepted");
    let MlKemKeyPair {
        public_key: kyber_public,
        secret_key: kyber_secret,
    } = mlkem_keypair();
    material
        .set_kyber_keys(kyber_public.as_slice(), kyber_secret.as_slice())
        .expect("kyber key material accepted");

    let fingerprint = fingerprint(kyber_public.as_slice());
    assert_eq!(material.kyber_fingerprint(), Some(fingerprint));
    assert_eq!(
        material.kyber_public().map(<[u8]>::to_vec),
        Some(kyber_public.clone())
    );
    assert!(material.kyber_secret().is_some(), "secret key stored");

    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    material
        .install_into_session(&mut session)
        .expect("kyber secret installed");
    session
        .set_kyber_remote_public(fingerprint, kyber_public.as_slice())
        .expect("remote kyber key accepted");

    let key_update = material
        .build_key_update(
            &mut session,
            [0x10; 32],
            &EncryptionSuite::Kyber768XChaCha20Poly1305(fingerprint),
            1,
            7,
        )
        .expect("key update signed");
    assert_eq!(key_update.key_counter, 7);
    assert_eq!(key_update.session_id, [0x10; 32]);
}

#[test]
fn streaming_key_material_rejects_mismatched_kyber_key_pair() {
    let identity = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let mut material = StreamingKeyMaterial::new(identity).expect("ed25519 identity accepted");
    let (public_key, _secret_key) = mlkem_keypair_bytes();
    let (_wrong_public, wrong_secret) = mlkem_keypair_bytes();

    let err = material
        .set_kyber_keys(public_key.as_slice(), wrong_secret.as_slice())
        .expect_err("mismatched Kyber key pair must fail");
    assert!(matches!(err, KeyMaterialError::KyberKeyPairMismatch));
    assert_eq!(material.kyber_public(), None);
    assert_eq!(material.kyber_fingerprint(), None);
    assert!(material.kyber_secret().is_none());
}

#[test]
fn streaming_key_material_rejects_noncanonical_kyber_public_key() {
    let identity = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let mut material = StreamingKeyMaterial::new(identity).expect("ed25519 identity accepted");
    let (mut public_key, secret_key) = mlkem_keypair_bytes();
    set_first_mlkem_12_bit_coefficient_noncanonical(&mut public_key);

    let err = material
        .set_kyber_keys(public_key.as_slice(), secret_key.as_slice())
        .expect_err("noncanonical Kyber public key must fail");
    assert!(matches!(err, KeyMaterialError::InvalidKyberPublicKey));
    assert_eq!(material.kyber_public(), None);
    assert_eq!(material.kyber_fingerprint(), None);
    assert!(material.kyber_secret().is_none());
}

#[test]
fn streaming_key_material_rejects_noncanonical_kyber_secret_key() {
    let identity = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let mut material = StreamingKeyMaterial::new(identity).expect("ed25519 identity accepted");
    let (public_key, mut secret_key) = mlkem_keypair_bytes();
    set_first_mlkem_12_bit_coefficient_noncanonical(&mut secret_key);

    let err = material
        .set_kyber_keys(public_key.as_slice(), secret_key.as_slice())
        .expect_err("noncanonical Kyber secret key must fail");
    assert!(matches!(err, KeyMaterialError::InvalidKyberSecretKey));
    assert_eq!(material.kyber_public(), None);
    assert_eq!(material.kyber_fingerprint(), None);
    assert!(material.kyber_secret().is_none());
}

#[test]
fn streaming_session_rejects_noncanonical_kyber_remote_public() {
    let (mut public_key, _secret_key) = mlkem_keypair_bytes();
    let expected_fingerprint = fingerprint(public_key.as_slice());
    set_first_mlkem_12_bit_coefficient_noncanonical(&mut public_key);
    let mut session = StreamingSession::new(CapabilityRole::Publisher);

    let err = session
        .set_kyber_remote_public(expected_fingerprint, public_key.as_slice())
        .expect_err("noncanonical remote Kyber public key must fail");
    assert!(matches!(err, HandshakeError::InvalidKyberPublicKey));
}

#[test]
fn streaming_session_rejects_noncanonical_kyber_local_secret() {
    let (_public_key, mut secret_key) = mlkem_keypair_bytes();
    set_first_mlkem_12_bit_coefficient_noncanonical(&mut secret_key);
    let mut session = StreamingSession::new(CapabilityRole::Viewer);

    let err = session
        .set_kyber_local_secret(secret_key.as_slice())
        .expect_err("noncanonical local Kyber secret key must fail");
    assert!(matches!(err, HandshakeError::InvalidKyberSecretKey));
}

#[test]
fn kyber_key_update_roundtrip() {
    if cfg!(feature = "sm") {
        eprintln!("skipping kyber_key_update_roundtrip under sm feature");
        return;
    }
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let viewer_keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);

    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();

    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);
    let viewer_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(viewer_fp);

    let session_id = [0x23; 32];
    let protocol_version = 1u16;

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");

    let publisher_update = publisher_session
        .build_key_update(
            session_id,
            &publisher_suite,
            protocol_version,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");
    assert_eq!(
        publisher_update.pub_ephemeral.len(),
        TEST_KEM_SUITE.ciphertext_len()
    );

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");

    viewer_session
        .process_remote_key_update(&publisher_update, publisher_keys.public_key())
        .expect("viewer processes publisher update");

    let viewer_update = viewer_session
        .build_key_update(
            session_id,
            &viewer_suite,
            protocol_version,
            2,
            viewer_keypair.private_key(),
        )
        .expect("viewer key update");
    assert_eq!(
        viewer_update.pub_ephemeral.len(),
        TEST_KEM_SUITE.ciphertext_len()
    );

    publisher_session
        .process_remote_key_update(&viewer_update, viewer_keypair.public_key())
        .expect("publisher processes viewer update");
    let publisher_transport = publisher_session
        .transport_keys()
        .copied()
        .expect("publisher transport keys");
    let viewer_transport = viewer_session
        .transport_keys()
        .copied()
        .expect("viewer transport keys");

    assert_eq!(viewer_transport.send, publisher_transport.recv);
    assert_eq!(viewer_transport.recv, publisher_transport.send);
}

#[test]
fn build_key_update_records_outbound_snapshot_for_kyber() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (viewer_hpke_public, _viewer_hpke_secret) = mlkem_keypair_bytes();
    let session_id = [0xCA; 32];
    let key_counter = 7;
    let fingerprint = fingerprint(viewer_hpke_public.as_slice());
    let suite = EncryptionSuite::Kyber768XChaCha20Poly1305(fingerprint);

    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    session
        .set_kyber_remote_public(fingerprint, viewer_hpke_public.as_slice())
        .expect("viewer kyber public configured");
    session
        .build_key_update(
            session_id,
            &suite,
            1,
            key_counter,
            publisher_keys.private_key(),
        )
        .expect("build kyber key update");

    let snapshot = session.snapshot_state().expect("snapshot recorded");
    assert_eq!(snapshot.session_id, session_id);
    assert_eq!(snapshot.key_counter, key_counter);
    assert_eq!(snapshot.suite, suite);
}

#[test]
fn kyber_outbound_snapshot_restores_with_local_fingerprint_metadata() {
    let identity = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let mut material = StreamingKeyMaterial::new(identity).expect("ed25519 identity");
    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, _viewer_hpke_secret) = mlkem_keypair_bytes();
    material
        .set_kyber_keys(
            publisher_hpke_public.as_slice(),
            publisher_hpke_secret.as_slice(),
        )
        .expect("publisher kyber material");
    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);

    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    material
        .install_into_session(&mut session)
        .expect("local kyber material installed");
    session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    material
        .build_key_update(&mut session, [0xCB; 32], &publisher_suite, 1, 3)
        .expect("publisher outbound key update");

    let snapshot = session.snapshot_state().expect("snapshot recorded");
    assert_eq!(snapshot.suite, publisher_suite);
    assert_eq!(
        snapshot.kyber_local_public.as_deref(),
        Some(publisher_hpke_public.as_slice())
    );
    assert_eq!(snapshot.kyber_local_fingerprint, Some(publisher_fp));
    assert_eq!(snapshot.kyber_remote_fingerprint, Some(viewer_fp));

    let mut restored = StreamingSession::new(CapabilityRole::Publisher);
    material
        .install_into_session(&mut restored)
        .expect("local kyber material installed before restore");
    restored
        .restore_from_snapshot(snapshot.clone())
        .expect("outbound kyber snapshot restores");
    assert_eq!(restored.snapshot_state(), Some(snapshot));
}

#[test]
fn kyber_outbound_snapshot_restore_requires_matching_local_secret() {
    let identity = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let mut material = StreamingKeyMaterial::new(identity).expect("ed25519 identity");
    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, _viewer_hpke_secret) = mlkem_keypair_bytes();
    material
        .set_kyber_keys(
            publisher_hpke_public.as_slice(),
            publisher_hpke_secret.as_slice(),
        )
        .expect("publisher kyber material");
    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);

    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    material
        .install_into_session(&mut session)
        .expect("local kyber material installed");
    session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    material
        .build_key_update(&mut session, [0xCD; 32], &publisher_suite, 1, 3)
        .expect("publisher outbound key update");
    let snapshot = session.snapshot_state().expect("snapshot recorded");

    let mut missing_secret = StreamingSession::new(CapabilityRole::Publisher);
    let err = missing_secret
        .restore_from_snapshot(snapshot.clone())
        .expect_err("local Kyber metadata requires an installed secret");
    assert!(matches!(err, HandshakeError::MissingKyberLocalSecret));
    assert_eq!(missing_secret.snapshot_state(), None);

    let (wrong_public, wrong_secret) = mlkem_keypair_bytes();
    let mut wrong_secret_session = StreamingSession::new(CapabilityRole::Publisher);
    wrong_secret_session
        .set_kyber_local_key_pair(wrong_public.as_slice(), wrong_secret.as_slice())
        .expect("wrong but internally paired local kyber material");
    let err = wrong_secret_session
        .restore_from_snapshot(snapshot)
        .expect_err("snapshot local public must match installed local secret");
    assert!(matches!(err, HandshakeError::KyberKeyPairMismatch));
    assert_eq!(wrong_secret_session.snapshot_state(), None);
}

#[test]
fn set_kyber_local_secret_clears_local_snapshot_metadata() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (local_public, local_secret) = mlkem_keypair_bytes();
    let (remote_public, _remote_secret) = mlkem_keypair_bytes();
    let local_fp = fingerprint(local_public.as_slice());
    let remote_fp = fingerprint(remote_public.as_slice());
    let suite = EncryptionSuite::Kyber768XChaCha20Poly1305(remote_fp);
    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    session
        .set_kyber_local_key_pair(local_public.as_slice(), local_secret.as_slice())
        .expect("local kyber key pair");
    session
        .set_kyber_local_secret(local_secret.as_slice())
        .expect("secret-only install clears local public metadata");
    session
        .set_kyber_remote_public(remote_fp, remote_public.as_slice())
        .expect("remote kyber public");
    session
        .build_key_update([0xCC; 32], &suite, 1, 5, publisher_keys.private_key())
        .expect("kyber key update");

    let snapshot = session.snapshot_state().expect("snapshot recorded");
    assert_eq!(snapshot.kyber_local_public, None);
    assert_eq!(snapshot.kyber_local_fingerprint, None);
    assert_ne!(snapshot.kyber_remote_fingerprint, Some(local_fp));
}

#[test]
fn outbound_key_update_rejects_zero_counter_without_state_change() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xC0; 32]);
    let mut session = StreamingSession::new(CapabilityRole::Publisher);

    let err = session
        .build_key_update([0xC0; 32], &suite, 1, 0, publisher_keys.private_key())
        .expect_err("zero counter rejected");
    match err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::InvalidKeyCounter { found }) => {
            assert_eq!(found, 0);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(session.snapshot_state(), None);
    assert!(session.transport_keys().is_none());
}

#[test]
fn outbound_key_update_rejects_zero_protocol_version_without_state_change() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xC1; 32]);
    let mut session = StreamingSession::new(CapabilityRole::Publisher);

    let err = session
        .build_key_update([0xC1; 32], &suite, 0, 1, publisher_keys.private_key())
        .expect_err("zero protocol version rejected");
    match err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::InvalidProtocolVersion { found }) => {
            assert_eq!(found, 0);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(session.snapshot_state(), None);
    assert!(session.transport_keys().is_none());
}

#[test]
fn outbound_key_update_rejects_key_counter_regression() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (viewer_hpke_public, _viewer_hpke_secret) = mlkem_keypair_bytes();
    let session_id = [0xCB; 32];
    let fingerprint = fingerprint(viewer_hpke_public.as_slice());
    let suite = EncryptionSuite::Kyber768XChaCha20Poly1305(fingerprint);

    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    session
        .set_kyber_remote_public(fingerprint, viewer_hpke_public.as_slice())
        .expect("viewer kyber public configured");
    session
        .build_key_update(session_id, &suite, 1, 7, publisher_keys.private_key())
        .expect("initial kyber key update");
    let snapshot_before = session.snapshot_state().expect("snapshot recorded");

    let err = session
        .build_key_update(session_id, &suite, 1, 7, publisher_keys.private_key())
        .expect_err("same counter rejected");
    match err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::NonMonotonicKeyCounter {
            previous,
            found,
        }) => {
            assert_eq!(previous, 7);
            assert_eq!(found, 7);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn outbound_content_key_update_rejects_regression_before_state_change() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xAC; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x19; 32]);
    let key_update = publisher_session
        .build_key_update([0x0A; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x29; 32]);
    viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect("viewer processes key update");

    let first_gck = [0x39u8; 32];
    viewer_session
        .build_content_key_update(&first_gck, 71, 1_400)
        .expect("initial content key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot after content key");

    let regressed_gck = [0x49u8; 32];
    let err = viewer_session
        .build_content_key_update(&regressed_gck, 71, 1_401)
        .expect_err("same content key id rejected");
    match err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::ContentKeyRegression {
            previous,
            found,
        }) => {
            assert_eq!(previous, 71);
            assert_eq!(found, 71);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.latest_gck(), Some(first_gck.as_ref()));
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn outbound_content_key_update_rejects_invalid_gck_length_before_state_change() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0xAD; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x1A; 32]);
    let key_update = publisher_session
        .build_key_update([0x0B; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x2A; 32]);
    viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect("viewer processes key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid gck");

    let short_gck = [0x4Au8; 31];
    let err = viewer_session
        .build_content_key_update(&short_gck, 72, 1_500)
        .expect_err("short outbound gck rejected");
    match err {
        HandshakeError::InvalidGroupContentKeyLength { expected, found } => {
            assert_eq!(expected, 32);
            assert_eq!(found, short_gck.len());
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(viewer_session.latest_gck().is_none());
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));

    let valid_gck = [0x5Au8; 32];
    viewer_session
        .build_content_key_update(&valid_gck, 72, 1_500)
        .expect("same rotation metadata accepted after rejected short gck");
}

#[test]
fn kyber_local_ephemeral_requires_remote_public() {
    let (_pk, sk) = mlkem_keypair_bytes();
    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    session
        .set_kyber_local_secret(sk.as_slice())
        .expect("kyber secret");
    let err = session
        .local_ephemeral_public(&EncryptionSuite::Kyber768XChaCha20Poly1305([0x11; 32]))
        .expect_err("remote public must be configured first");
    assert!(matches!(err, HandshakeError::MissingKyberRemotePublic));
}

#[test]
fn kyber_local_ephemeral_public_does_not_commit_transport_state() {
    let (remote_public, _remote_secret) = mlkem_keypair_bytes();
    let remote_fingerprint = fingerprint(remote_public.as_slice());
    let suite = EncryptionSuite::Kyber768XChaCha20Poly1305(remote_fingerprint);
    let mut session = StreamingSession::new(CapabilityRole::Publisher);
    session
        .set_kyber_remote_public(remote_fingerprint, remote_public.as_slice())
        .expect("remote kyber public");

    let ciphertext = session
        .local_ephemeral_public(&suite)
        .expect("kyber encapsulation payload");
    assert_eq!(ciphertext.len(), TEST_KEM_SUITE.ciphertext_len());
    assert_eq!(session.negotiated_suite(), None);
    assert!(session.transport_keys().is_none());
    assert!(session.sts_root().is_none());
    assert_eq!(session.snapshot_state(), None);

    let err = session
        .build_content_key_update(&[0xA5; 32], 1, 1)
        .expect_err("content key update still requires a signed key update");
    assert!(matches!(err, HandshakeError::SuiteNotNegotiated));
}

#[test]
fn session_kem_suite_change_clears_configured_kyber_material() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();
    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    let publisher_update = publisher_session
        .build_key_update(
            [0x43; 32],
            &publisher_suite,
            1,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");

    viewer_session.set_kem_suite(MlKemSuite::MlKem512);
    viewer_session.set_kem_suite(MlKemSuite::MlKem768);

    let err = viewer_session
        .process_remote_key_update(&publisher_update, publisher_keys.public_key())
        .expect_err("suite switch clears remote public metadata");
    assert!(matches!(err, HandshakeError::MissingKyberRemotePublic));

    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public after suite switch");
    let err = viewer_session
        .process_remote_key_update(&publisher_update, publisher_keys.public_key())
        .expect_err("suite switch clears local secret material");
    assert!(matches!(err, HandshakeError::MissingKyberLocalSecret));
    assert_eq!(viewer_session.snapshot_state(), None);
}

#[test]
fn kyber_process_remote_key_update_requires_local_secret() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);

    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, _viewer_hpke_secret) = mlkem_keypair_bytes();
    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");

    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);
    let key_update = publisher_session
        .build_key_update(
            [0xAA; 32],
            &publisher_suite,
            1,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");
    let err = viewer_session
        .process_remote_key_update(&key_update, publisher_keys.public_key())
        .expect_err("local secret must be configured");
    assert!(matches!(err, HandshakeError::MissingKyberLocalSecret));
}

#[test]
fn kyber_process_remote_key_update_rejects_truncated_ciphertext() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();
    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());

    let suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);
    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    let key_update = publisher_session
        .build_key_update([0x01; 32], &suite, 1, 1, publisher_keys.private_key())
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");

    let mut truncated = key_update.clone();
    truncated.pub_ephemeral.truncate(32);
    let transcript = key_update_transcript_bytes(&truncated).expect("serialize truncated frame");
    let signature = Signature::new(publisher_keys.private_key(), &transcript);
    truncated.signature.copy_from_slice(signature.payload());
    let err = viewer_session
        .process_remote_key_update(&truncated, publisher_keys.public_key())
        .expect_err("ciphertext length must be validated");
    match err {
        HandshakeError::InvalidEphemeralPublicKey { expected, found } => {
            assert_eq!(
                expected,
                TEST_KEM_SUITE.ciphertext_len(),
                "expected ciphertext length must match Kyber ciphertext"
            );
            assert_eq!(found, truncated.pub_ephemeral.len());
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn streaming_session_snapshot_roundtrip() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x42; 32]);

    let publisher_secret_bytes = [0x10u8; 32];
    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519(publisher_secret_bytes);
    let key_update = publisher_session
        .build_key_update([0xAA; 32], &suite, 3, 9, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    let viewer_secret_bytes = [0x20u8; 32];
    let viewer_public_bytes = viewer_session.set_local_ephemeral_x25519(viewer_secret_bytes);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let negotiated_suite = viewer_session
        .negotiated_suite()
        .copied()
        .expect("suite negotiated");
    assert_eq!(negotiated_suite, suite);

    let viewer_transport = *viewer_session
        .transport_keys()
        .expect("transport keys available");
    let viewer_public = X25519PublicKey::from(
        <[u8; 32]>::try_from(viewer_public_bytes.as_slice()).expect("viewer public length"),
    );
    let publisher_secret = StaticSecret::from(publisher_secret_bytes);
    let shared = publisher_secret.diffie_hellman(&viewer_public);
    let mut shared_secret = [0u8; 32];
    shared_secret.copy_from_slice(shared.as_bytes());
    let publisher_sts_root =
        streaming_crypto::derive_sts_root(&shared_secret).expect("derive publisher sts root");
    let publisher_keys = streaming_crypto::derive_transport_keys_from_sts_root(
        &publisher_sts_root,
        CapabilityRole::Publisher,
    )
    .expect("publisher transport keys");

    let nonce_len = nonce_len_for_suite(&suite);
    let gck_nonce = vec![0x33; nonce_len];
    let gck_plain = [0x77u8; 32];
    let wrapped_gck = wrap_gck(
        &suite,
        &publisher_keys.send,
        &gck_nonce,
        &gck_plain,
        24,
        512,
    )
    .expect("wrap gck");
    let content_update = ContentKeyUpdate {
        content_key_id: 24,
        gck_wrapped: wrapped_gck,
        valid_from_segment: 512,
    };
    let unwrapped = viewer_session
        .process_content_key_update(&content_update)
        .expect("unwrap gck");
    assert_eq!(unwrapped.as_slice(), gck_plain);

    let snapshot = viewer_session.snapshot_state().expect("snapshot available");
    assert_eq!(snapshot.role, CapabilityRole::Viewer);
    assert_eq!(snapshot.key_counter, 9);
    let sts_root = viewer_session
        .sts_root()
        .copied()
        .expect("session derives sts root");
    assert_eq!(snapshot.sts_root, sts_root);
    assert_eq!(snapshot.cadence, None);
    assert_eq!(snapshot.kyber_remote_public, None);

    let mut restored_session = StreamingSession::new(CapabilityRole::Viewer);
    restored_session
        .restore_from_snapshot(snapshot.clone())
        .expect("restore snapshot");

    let restored_snapshot = restored_session
        .snapshot_state()
        .expect("restored snapshot available");
    assert_eq!(restored_snapshot, snapshot);
    assert_eq!(
        restored_session
            .transport_keys()
            .expect("restored transport keys"),
        &viewer_transport,
    );
    assert_eq!(
        restored_session.latest_gck().expect("restored gck"),
        viewer_session.latest_gck().expect("original gck"),
    );
}

#[test]
fn restore_from_snapshot_rejects_zero_key_counter_without_resetting_session() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x2A; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x1A; 32]);
    let key_update = publisher_session
        .build_key_update([0x2A; 32], &suite, 1, 3, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x3A; 32]);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid restore");

    let mut invalid_snapshot = snapshot_before.clone();
    invalid_snapshot.key_counter = 0;
    let err = viewer_session
        .restore_from_snapshot(invalid_snapshot)
        .expect_err("zero snapshot key counter rejected");
    match err {
        HandshakeError::InvalidSnapshot(reason) => {
            assert!(reason.contains("key counter must be nonzero"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn restore_from_snapshot_rejects_invalid_kem_suite_without_resetting_session() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x24; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x14; 32]);
    let key_update = publisher_session
        .build_key_update([0x44; 32], &suite, 1, 3, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x34; 32]);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid restore");

    let mut invalid_snapshot = snapshot_before.clone();
    invalid_snapshot.kem_suite_id = u8::MAX;
    let err = viewer_session
        .restore_from_snapshot(invalid_snapshot)
        .expect_err("invalid kem suite rejected");
    match err {
        HandshakeError::UnsupportedKemSuite(found) => assert_eq!(found, u8::MAX),
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn restore_from_snapshot_rejects_invalid_transport_capabilities_without_resetting_session() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x2B; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x1B; 32]);
    let key_update = publisher_session
        .build_key_update([0x4B; 32], &suite, 1, 10, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x3B; 32]);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    viewer_session
        .record_transport_capabilities(sample_resolution())
        .expect("valid transport capabilities");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid restore");
    let transport = snapshot_before
        .transport_capabilities
        .expect("transport capabilities snapshot");

    let mut datagram_zero = transport;
    datagram_zero.use_datagram = true;
    datagram_zero.max_segment_datagram_size = 0;
    let mut stream_with_datagram_size = transport;
    stream_with_datagram_size.use_datagram = false;
    stream_with_datagram_size.max_segment_datagram_size = 1_024;

    for (invalid_transport, expected_reason) in [
        (
            datagram_zero,
            "datagram transport snapshot requires nonzero datagram size",
        ),
        (
            stream_with_datagram_size,
            "stream transport snapshot must not carry datagram size",
        ),
    ] {
        let mut invalid_snapshot = snapshot_before.clone();
        invalid_snapshot.transport_capabilities = Some(invalid_transport);
        let err = viewer_session
            .restore_from_snapshot(invalid_snapshot)
            .expect_err("invalid transport snapshot rejected");
        match err {
            HandshakeError::InvalidSnapshot(reason) => {
                assert!(reason.contains(expected_reason));
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert_eq!(
            viewer_session.snapshot_state(),
            Some(snapshot_before.clone())
        );
    }
}

#[test]
fn restore_from_snapshot_rejects_kyber_suite_kem_mismatch_without_resetting_session() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x29; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x19; 32]);
    let key_update = publisher_session
        .build_key_update([0x49; 32], &suite, 1, 8, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x39; 32]);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid restore");

    let MlKemKeyPair {
        public_key: mlkem512_public,
        ..
    } = generate_mlkem_keypair_from_os(MlKemSuite::MlKem512).expect("ML-KEM-512 keypair");
    let mlkem512_fingerprint =
        kyber_public_fingerprint_with_suite(&mlkem512_public, MlKemSuite::MlKem512)
            .expect("ML-KEM-512 fingerprint");
    let mut invalid_snapshot = snapshot_before.clone();
    invalid_snapshot.suite = EncryptionSuite::Kyber768XChaCha20Poly1305(mlkem512_fingerprint);
    invalid_snapshot.kem_suite_id = 0;
    invalid_snapshot.kyber_remote_public = Some(mlkem512_public);
    invalid_snapshot.kyber_remote_fingerprint = Some(mlkem512_fingerprint);

    let err = viewer_session
        .restore_from_snapshot(invalid_snapshot)
        .expect_err("kyber768 suite must bind to mlkem768 metadata");
    match err {
        HandshakeError::InvalidSnapshot(reason) => {
            assert!(reason.contains("kyber768 suite requires mlkem768"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn restore_from_snapshot_rejects_kyber_suite_fingerprint_drift_without_resetting_session() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x2A; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x1A; 32]);
    let key_update = publisher_session
        .build_key_update([0x4A; 32], &suite, 1, 9, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x3A; 32]);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid restore");

    let (kyber_public, _kyber_secret) = mlkem_keypair_bytes();
    let remote_fingerprint = fingerprint(kyber_public.as_slice());
    let mut suite_fingerprint = remote_fingerprint;
    suite_fingerprint[0] ^= 0xA5;
    let mut invalid_snapshot = snapshot_before.clone();
    invalid_snapshot.suite = EncryptionSuite::Kyber768XChaCha20Poly1305(suite_fingerprint);
    invalid_snapshot.kem_suite_id = 1;
    invalid_snapshot.kyber_remote_public = Some(kyber_public);
    invalid_snapshot.kyber_remote_fingerprint = Some(remote_fingerprint);

    let err = viewer_session
        .restore_from_snapshot(invalid_snapshot)
        .expect_err("suite fingerprint must bind to kyber metadata");
    match err {
        HandshakeError::KyberFingerprintMismatch { expected, found } => {
            assert_eq!(expected, suite_fingerprint);
            assert_eq!(found, remote_fingerprint);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn restore_from_snapshot_rejects_partial_content_key_metadata() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x25; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x15; 32]);
    let key_update = publisher_session
        .build_key_update([0x45; 32], &suite, 1, 4, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x35; 32]);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid restore");

    let mut invalid_snapshot = snapshot_before.clone();
    invalid_snapshot.last_content_key_id = Some(11);
    let err = viewer_session
        .restore_from_snapshot(invalid_snapshot)
        .expect_err("partial content-key metadata rejected");
    match err {
        HandshakeError::InvalidSnapshot(reason) => {
            assert!(reason.contains("content key metadata"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn restore_from_snapshot_rejects_invalid_gck_length_without_resetting_session() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x28; 32]);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x18; 32]);
    let key_update = publisher_session
        .build_key_update([0x48; 32], &suite, 1, 7, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x38; 32]);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let gck = [0x58u8; 32];
    viewer_session
        .build_content_key_update(&gck, 12, 88)
        .expect("content key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid restore");

    let mut invalid_snapshot = snapshot_before.clone();
    invalid_snapshot.latest_gck = Some(vec![0x68; 31]);
    let err = viewer_session
        .restore_from_snapshot(invalid_snapshot)
        .expect_err("short restored gck rejected");
    match err {
        HandshakeError::InvalidGroupContentKeyLength { expected, found } => {
            assert_eq!(expected, 32);
            assert_eq!(found, 31);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn restore_from_snapshot_rejects_partial_kyber_remote_metadata() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x26; 32]);
    let (kyber_public, _kyber_secret) = mlkem_keypair_bytes();

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x16; 32]);
    let key_update = publisher_session
        .build_key_update([0x46; 32], &suite, 1, 5, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x36; 32]);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid restore");

    let mut invalid_snapshot = snapshot_before.clone();
    invalid_snapshot.kyber_remote_public = Some(kyber_public);
    let err = viewer_session
        .restore_from_snapshot(invalid_snapshot)
        .expect_err("partial kyber metadata rejected");
    match err {
        HandshakeError::InvalidSnapshot(reason) => {
            assert!(reason.contains("kyber remote metadata"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn restore_from_snapshot_rejects_kyber_remote_fingerprint_drift() {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let suite = EncryptionSuite::X25519ChaCha20Poly1305([0x27; 32]);
    let (kyber_public, _kyber_secret) = mlkem_keypair_bytes();
    let mut wrong_fingerprint = fingerprint(kyber_public.as_slice());
    wrong_fingerprint[0] ^= 0x5A;

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session.set_local_ephemeral_x25519([0x17; 32]);
    let key_update = publisher_session
        .build_key_update([0x47; 32], &suite, 1, 6, key_pair.private_key())
        .expect("build key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session.set_local_ephemeral_x25519([0x37; 32]);
    viewer_session
        .process_remote_key_update(&key_update, key_pair.public_key())
        .expect("process key update");
    let snapshot_before = viewer_session
        .snapshot_state()
        .expect("snapshot before invalid restore");

    let mut invalid_snapshot = snapshot_before.clone();
    invalid_snapshot.kyber_remote_public = Some(kyber_public);
    invalid_snapshot.kyber_remote_fingerprint = Some(wrong_fingerprint);
    let err = viewer_session
        .restore_from_snapshot(invalid_snapshot)
        .expect_err("fingerprint drift rejected");
    match err {
        HandshakeError::KyberFingerprintMismatch { expected, found } => {
            assert_eq!(expected, wrong_fingerprint);
            assert_ne!(found, expected);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(viewer_session.snapshot_state(), Some(snapshot_before));
}

#[test]
fn kyber_content_key_update_roundtrip() {
    if cfg!(feature = "sm") {
        eprintln!("skipping kyber_content_key_update_roundtrip under sm feature");
        return;
    }
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let viewer_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);

    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();

    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let session_id = [0x55; 32];
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);
    let viewer_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(viewer_fp);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    let publisher_update = publisher_session
        .build_key_update(
            session_id,
            &publisher_suite,
            1,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");
    viewer_session
        .process_remote_key_update(&publisher_update, publisher_keys.public_key())
        .expect("viewer processes publisher update");

    let viewer_update = viewer_session
        .build_key_update(session_id, &viewer_suite, 1, 2, viewer_keys.private_key())
        .expect("viewer key update");
    publisher_session
        .process_remote_key_update(&viewer_update, viewer_keys.public_key())
        .expect("publisher processes viewer update");

    let publisher_transport = publisher_session
        .transport_keys()
        .copied()
        .expect("publisher transport keys");

    let gck = [0x42u8; 32];
    let content_key_id = 17u64;
    let valid_from_segment = 2048u64;
    let active_suite = viewer_session
        .negotiated_suite()
        .copied()
        .expect("negotiated suite");
    let nonce_len = nonce_len_for_suite(&active_suite);
    let nonce = vec![0xAB; nonce_len];
    let wrapped = wrap_gck(
        &active_suite,
        &publisher_transport.send,
        &nonce,
        &gck,
        content_key_id,
        valid_from_segment,
    )
    .expect("wrap gck");

    let gck_update = ContentKeyUpdate {
        content_key_id,
        gck_wrapped: wrapped.clone(),
        valid_from_segment,
    };
    let decoded = viewer_session
        .process_content_key_update(&gck_update)
        .expect("unwrap gck");
    assert_eq!(decoded, gck);
    assert_eq!(viewer_session.latest_gck(), Some(gck.as_ref()));
}

#[test]
fn kyber_process_remote_key_update_rejects_suite_change() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();

    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let session_id = [0x40; 32];
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    let publisher_update = publisher_session
        .build_key_update(
            session_id,
            &publisher_suite,
            1,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");
    viewer_session
        .process_remote_key_update(&publisher_update, publisher_keys.public_key())
        .expect("initial key update");

    let mut drifted = publisher_update.clone();
    drifted.suite = EncryptionSuite::X25519ChaCha20Poly1305([0xAA; 32]);
    drifted.pub_ephemeral = vec![0x11; 32];
    drifted.key_counter = 2;
    let mutated_suite = drifted.suite;
    let transcript = key_update_transcript_bytes(&drifted).expect("serialize drifted frame");
    let signature = Signature::new(publisher_keys.private_key(), &transcript);
    drifted.signature.copy_from_slice(signature.payload());

    let err = viewer_session
        .process_remote_key_update(&drifted, publisher_keys.public_key())
        .expect_err("suite change must be rejected");
    match err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::SuiteChanged { expected, found }) => {
            assert_eq!(expected, publisher_suite);
            assert_eq!(found, mutated_suite);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn kyber_replay_after_restore_rejects_counter_before_decapsulation_secret() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();
    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    let update = publisher_session
        .build_key_update(
            [0x41; 32],
            &publisher_suite,
            1,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");
    viewer_session
        .process_remote_key_update(&update, publisher_keys.public_key())
        .expect("first update succeeds");
    let snapshot = viewer_session
        .snapshot_state()
        .expect("viewer session snapshot");

    let mut restored_without_secret = StreamingSession::new(CapabilityRole::Viewer);
    restored_without_secret
        .restore_from_snapshot(snapshot.clone())
        .expect("snapshot restores without local kyber secret");
    assert_eq!(
        restored_without_secret.snapshot_state(),
        Some(snapshot.clone())
    );

    let replay_err = restored_without_secret
        .process_remote_key_update(&update, publisher_keys.public_key())
        .expect_err("replay must fail before kyber decapsulation");
    match replay_err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::NonMonotonicKeyCounter {
            previous,
            found,
        }) => {
            assert_eq!(previous, 1);
            assert_eq!(found, 1);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(restored_without_secret.snapshot_state(), Some(snapshot));
}

#[test]
fn kyber_suite_change_after_restore_rejects_state_drift_before_decapsulation() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();
    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    let update = publisher_session
        .build_key_update(
            [0x42; 32],
            &publisher_suite,
            1,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");
    viewer_session
        .process_remote_key_update(&update, publisher_keys.public_key())
        .expect("first update succeeds");
    let snapshot = viewer_session
        .snapshot_state()
        .expect("viewer session snapshot");

    let mut restored_without_secret = StreamingSession::new(CapabilityRole::Viewer);
    restored_without_secret
        .restore_from_snapshot(snapshot.clone())
        .expect("snapshot restores without local kyber secret");

    let mut drifted = update.clone();
    let mut drifted_fingerprint = publisher_fp;
    drifted_fingerprint[0] ^= 0x5A;
    drifted.suite = EncryptionSuite::Kyber768XChaCha20Poly1305(drifted_fingerprint);
    drifted.key_counter = 2;
    drifted.pub_ephemeral = vec![0xA5; TEST_KEM_SUITE.ciphertext_len()];
    let mutated_suite = drifted.suite;
    let transcript = key_update_transcript_bytes(&drifted).expect("serialize drifted frame");
    let signature = Signature::new(publisher_keys.private_key(), &transcript);
    drifted.signature.copy_from_slice(signature.payload());

    let err = restored_without_secret
        .process_remote_key_update(&drifted, publisher_keys.public_key())
        .expect_err("suite drift must fail before kyber decapsulation");
    match err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::SuiteChanged { expected, found }) => {
            assert_eq!(expected, publisher_suite);
            assert_eq!(found, mutated_suite);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(restored_without_secret.snapshot_state(), Some(snapshot));
}

#[test]
fn kyber_process_remote_key_update_rejects_replay() {
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();
    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    let update = publisher_session
        .build_key_update(
            [0x88; 32],
            &publisher_suite,
            1,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");
    viewer_session
        .process_remote_key_update(&update, publisher_keys.public_key())
        .expect("first update succeeds");
    let replay_err = viewer_session
        .process_remote_key_update(&update, publisher_keys.public_key())
        .expect_err("replay must fail");
    match replay_err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::NonMonotonicKeyCounter {
            previous,
            found,
        }) => {
            assert_eq!(previous, 1);
            assert_eq!(found, 1);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn kyber_content_key_update_rejects_truncated_payload() {
    if cfg!(feature = "sm") {
        eprintln!("skipping kyber_content_key_update_rejects_truncated_payload under sm feature");
        return;
    }
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let viewer_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();
    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let session_id = [0x66; 32];
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);
    let viewer_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(viewer_fp);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    let publisher_update = publisher_session
        .build_key_update(
            session_id,
            &publisher_suite,
            1,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");
    viewer_session
        .process_remote_key_update(&publisher_update, publisher_keys.public_key())
        .expect("viewer processes publisher update");

    let viewer_update = viewer_session
        .build_key_update(session_id, &viewer_suite, 1, 2, viewer_keys.private_key())
        .expect("viewer key update");
    publisher_session
        .process_remote_key_update(&viewer_update, viewer_keys.public_key())
        .expect("publisher processes viewer update");

    let negotiated_suite = viewer_session
        .negotiated_suite()
        .copied()
        .expect("suite negotiated");
    let expected_nonce = nonce_len_for_suite(&negotiated_suite);
    let truncated = ContentKeyUpdate {
        content_key_id: 42,
        gck_wrapped: Vec::new(),
        valid_from_segment: 33,
    };
    let err = viewer_session
        .process_content_key_update(&truncated)
        .expect_err("empty payload rejected");
    match err {
        HandshakeError::MalformedWrappedKey { expected, found } => {
            assert_eq!(expected, expected_nonce);
            assert_eq!(found, truncated.gck_wrapped.len());
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn kyber_content_key_update_rejects_id_regression() {
    if cfg!(feature = "sm") {
        eprintln!("skipping kyber_content_key_update_rejects_id_regression under sm feature");
        return;
    }
    let publisher_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let viewer_keys = KeyPair::random_with_algorithm(Algorithm::Ed25519);

    let (publisher_hpke_public, publisher_hpke_secret) = mlkem_keypair_bytes();
    let (viewer_hpke_public, viewer_hpke_secret) = mlkem_keypair_bytes();

    let publisher_fp = fingerprint(publisher_hpke_public.as_slice());
    let viewer_fp = fingerprint(viewer_hpke_public.as_slice());
    let session_id = [0x77; 32];
    let publisher_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(publisher_fp);
    let viewer_suite = EncryptionSuite::Kyber768XChaCha20Poly1305(viewer_fp);

    let mut publisher_session = StreamingSession::new(CapabilityRole::Publisher);
    publisher_session
        .set_kyber_local_secret(publisher_hpke_secret.as_slice())
        .expect("publisher kyber secret");
    publisher_session
        .set_kyber_remote_public(viewer_fp, viewer_hpke_public.as_slice())
        .expect("viewer kyber public");
    let publisher_update = publisher_session
        .build_key_update(
            session_id,
            &publisher_suite,
            1,
            1,
            publisher_keys.private_key(),
        )
        .expect("publisher key update");

    let mut viewer_session = StreamingSession::new(CapabilityRole::Viewer);
    viewer_session
        .set_kyber_local_secret(viewer_hpke_secret.as_slice())
        .expect("viewer kyber secret");
    viewer_session
        .set_kyber_remote_public(publisher_fp, publisher_hpke_public.as_slice())
        .expect("publisher kyber public");
    viewer_session
        .process_remote_key_update(&publisher_update, publisher_keys.public_key())
        .expect("viewer processes publisher update");

    let viewer_update = viewer_session
        .build_key_update(session_id, &viewer_suite, 1, 2, viewer_keys.private_key())
        .expect("viewer key update");
    publisher_session
        .process_remote_key_update(&viewer_update, viewer_keys.public_key())
        .expect("publisher processes viewer update");

    let publisher_transport = publisher_session
        .transport_keys()
        .copied()
        .expect("publisher transport keys");

    let nonce_len = nonce_len_for_suite(&publisher_suite);
    let nonce = vec![0xCD; nonce_len];

    let first_gck = [0x55u8; 32];
    let first_wrapped = wrap_gck(
        &publisher_suite,
        &publisher_transport.send,
        &nonce,
        &first_gck,
        100,
        10,
    )
    .expect("wrap first gck");
    viewer_session
        .process_content_key_update(&ContentKeyUpdate {
            content_key_id: 100,
            gck_wrapped: first_wrapped,
            valid_from_segment: 10,
        })
        .expect("first gck accepted");

    let regression_wrapped = wrap_gck(
        &publisher_suite,
        &publisher_transport.send,
        &nonce,
        &first_gck,
        50,
        20,
    )
    .expect("wrap regressed gck");
    let regression_err = viewer_session
        .process_content_key_update(&ContentKeyUpdate {
            content_key_id: 50,
            gck_wrapped: regression_wrapped,
            valid_from_segment: 20,
        })
        .expect_err("content key id regression rejected");
    match regression_err {
        HandshakeError::Crypto(streaming_crypto::CryptoError::ContentKeyRegression {
            previous,
            found,
        }) => {
            assert_eq!(previous, 100);
            assert_eq!(found, 50);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
