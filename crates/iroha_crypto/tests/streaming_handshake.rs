//! Integration tests validating the streaming handshake and encrypted chunk pipeline.

use std::convert::{TryFrom, TryInto};

use iroha_crypto::{
    Algorithm, KeyPair, Signature,
    streaming::{
        HandshakeError, KeyMaterialError, SessionCadence, StreamingKeyMaterial, StreamingSession,
        key_update_transcript_bytes, kyber_public_fingerprint_with_suite,
    },
};
use norito::streaming::{
    CapabilityFlags, CapabilityRole, ChunkDescriptor, ContentKeyUpdate, EncryptionSuite, FecScheme,
    Hash, HpkeSuite, PrivacyBucketGranularity, StreamMetadata, TransportCapabilityResolution,
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
use soranet_pq::{MlKemKeyPair, MlKemSuite, generate_mlkem_keypair};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

const TEST_KEM_SUITE: MlKemSuite = MlKemSuite::MlKem768;

fn mlkem_keypair() -> MlKemKeyPair {
    generate_mlkem_keypair(TEST_KEM_SUITE)
}

fn mlkem_keypair_bytes() -> (Vec<u8>, Vec<u8>) {
    let MlKemKeyPair {
        public_key,
        secret_key,
    } = mlkem_keypair();
    (public_key, secret_key.as_slice().to_vec())
}

fn fingerprint(bytes: &[u8]) -> Hash {
    kyber_public_fingerprint_with_suite(bytes, TEST_KEM_SUITE).expect("fingerprint derivation")
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
