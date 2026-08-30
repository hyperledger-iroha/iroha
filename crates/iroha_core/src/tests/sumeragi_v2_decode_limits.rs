fn v2_decode_limit_relay_peer(seed: u8) -> PeerId {
    use iroha_crypto::Algorithm;

    PeerId::new(
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("derive deterministic v2 decode-limit relay key")
            .public_key()
            .clone(),
    )
}

fn v2_decode_limit_message(
    payload: iroha_data_model::block::consensus_v2::ConsensusMessageV2Payload,
) -> NetworkMessage {
    use iroha_data_model::block::consensus_v2::ConsensusMessageV2;

    NetworkMessage::SumeragiBlock(Arc::new(BlockMessageWire::new(BlockMessage::V2(
        ConsensusMessageV2::new(payload),
    ))))
}

fn v2_decode_limit_policy(
    message: &NetworkMessage,
    framed_len: usize,
) -> (Vec<u8>, norito::DecodeLimits) {
    let encoded = ncore::to_bytes(message).expect("encode v2 decode-limit fixture");
    let view = ncore::from_bytes_view(&encoded).expect("inspect v2 decode-limit fixture");
    let limits = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
        view.as_bytes(),
        framed_len,
        view.flags(),
    )
    .expect("select v2 decode-limit policy")
    .expect("every current v2 payload installs explicit decode limits");
    (encoded, limits)
}

fn v2_decode_limit_manifest(
    chunk_hash_count: usize,
    payload_size_bytes: u64,
    chunk_size_bytes: u32,
    payload_hash: Hash,
) -> iroha_data_model::block::consensus_v2::PayloadManifest {
    use iroha_crypto::MerkleTree;
    use iroha_data_model::block::consensus_v2 as wire;

    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"v2-decode-limit-context",
        ))),
        height: 7,
        view: 3,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"v2-decode-limit-parent",
        ))),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"v2-decode-limit-block")),
        payload_hash,
    };
    let chunk_hashes = vec![Hash::new(b"v2-decode-limit-chunk"); chunk_hash_count];
    let leaves = chunk_hashes
        .iter()
        .map(|hash| *hash.as_ref())
        .collect::<Vec<[u8; iroha_crypto::Hash::LENGTH]>>();
    let chunk_root =
        MerkleTree::<[u8; iroha_crypto::Hash::LENGTH]>::from_hashed_leaves_sha256(leaves)
            .root()
            .map(Hash::from)
            .expect("non-empty v2 decode-limit manifest has a root");
    wire::PayloadManifest {
        round,
        subject,
        payload_size_bytes,
        layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes,
            data_shards: wire::MAX_DA_DATA_SHARDS,
            parity_shards: wire::MAX_DA_PARITY_SHARDS,
            max_payload_size_bytes: wire::MAX_DA_PAYLOAD_SIZE_BYTES,
            max_chunk_count: wire::MAX_DA_CHUNK_COUNT,
        },
        chunk_hashes,
        chunk_root,
    }
}

fn v2_decode_limit_proposal(
    manifest: iroha_data_model::block::consensus_v2::PayloadManifest,
    signature: Vec<u8>,
) -> NetworkMessage {
    use iroha_data_model::block::consensus_v2 as wire;

    v2_decode_limit_message(wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
        round: manifest.round,
        proposer: 0,
        subject: manifest.subject,
        manifest,
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature,
    }))
}

#[test]
fn sumeragi_v2_maximum_chunk_passes_its_intrinsic_framed_decode_policy() {
    use iroha_data_model::block::consensus_v2 as wire;

    let message = v2_decode_limit_message(wire::ConsensusMessageV2Payload::PayloadChunk(
        wire::PayloadChunk {
            manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"v2-maximum-decode-limit-manifest",
            )),
            index: 0,
            bytes: vec![0xA5; wire::MAX_DA_CHUNK_SIZE_BYTES as usize],
            sender: 0,
            signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        },
    ));
    let origin = v2_decode_limit_relay_peer(0x31);
    let target = v2_decode_limit_relay_peer(0x32);
    let framed_len = iroha_p2p::network::data_frame_wire_len(&origin, Some(&target), &message);
    assert!(
        framed_len <= super::MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES,
        "maximum canonical chunk frame encoded to {framed_len} bytes, above the intrinsic {}-byte cap",
        super::MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES
    );
    let (encoded, limits) = v2_decode_limit_policy(&message, framed_len);
    assert_eq!(
        limits.max_sequence_elements(),
        wire::MAX_DA_CHUNK_SIZE_BYTES as usize
    );
    assert_eq!(
        limits.max_field_bytes(),
        super::MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES
    );
    let decoded = ncore::decode_from_bytes_with_limits::<NetworkMessage>(&encoded, limits)
        .expect("maximum canonical chunk must decode under its intrinsic resource policy");
    assert!(matches!(decoded, NetworkMessage::SumeragiBlock(_)));
}

#[test]
fn sumeragi_v2_chunk_policy_rejects_oversized_frames_and_sequences_before_allocation() {
    use iroha_data_model::block::consensus_v2 as wire;

    let chunk = |bytes: Vec<u8>, signature: Vec<u8>| {
        v2_decode_limit_message(wire::ConsensusMessageV2Payload::PayloadChunk(
            wire::PayloadChunk {
                manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"v2-adversarial-decode-limit-manifest",
                )),
                index: 0,
                bytes,
                sender: 0,
                signature,
            },
        ))
    };
    let small = chunk(vec![0xA5], vec![0x5A]);
    let small_encoded = ncore::to_bytes(&small).expect("encode small v2 chunk");
    let small_view = ncore::from_bytes_view(&small_encoded).expect("inspect small v2 chunk");
    let oversized_framed_len = super::MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES + 1;
    let frame_error = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
        small_view.as_bytes(),
        oversized_framed_len,
        small_view.flags(),
    )
    .expect_err("one byte above the chunk frame cap must fail before typed decode");
    assert!(matches!(
        frame_error,
        ncore::Error::ArchiveLengthExceeded { length, limit }
            if length == oversized_framed_len as u64
                && limit == super::MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES as u64
    ));

    let origin = v2_decode_limit_relay_peer(0x33);
    let target = v2_decode_limit_relay_peer(0x34);
    for (label, message, oversized_len, expected_limit) in [
        (
            "chunk bytes",
            chunk(
                vec![0xA5; wire::MAX_DA_CHUNK_SIZE_BYTES as usize + 1],
                vec![0x5A],
            ),
            wire::MAX_DA_CHUNK_SIZE_BYTES as u64 + 1,
            u64::from(wire::MAX_DA_CHUNK_SIZE_BYTES),
        ),
        (
            "chunk signature",
            chunk(
                vec![0xA5],
                vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES + 1],
            ),
            wire::MAX_CONSENSUS_SIGNATURE_BYTES as u64 + 1,
            wire::MAX_CONSENSUS_SIGNATURE_BYTES as u64,
        ),
    ] {
        let framed_len = iroha_p2p::network::data_frame_wire_len(&origin, Some(&target), &message);
        assert!(
            framed_len <= super::MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES,
            "{label} fixture must exercise the sequence guard rather than the frame cap"
        );
        let encoded = ncore::to_bytes(&message).expect("encode oversized v2 chunk fixture");
        assert!(
            ncore::decode_from_bytes::<NetworkMessage>(&encoded).is_ok(),
            "{label} fixture must be syntactically decodable without the resource policy"
        );
        let view = ncore::from_bytes_view(&encoded).expect("inspect oversized v2 chunk fixture");
        let error = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
            view.as_bytes(),
            framed_len,
            view.flags(),
        )
        .expect_err("oversized chunk field must fail during raw ingress preflight");
        assert!(
            matches!(
                error,
                ncore::Error::SequenceLengthExceeded { length, limit }
                    if length == oversized_len
                        && limit == expected_limit
            ),
            "unexpected {label} raw-preflight rejection: {error:?}"
        );
    }
}

#[test]
fn sumeragi_v2_proposal_manifest_hash_vector_is_bounded_before_allocation() {
    use iroha_data_model::block::consensus_v2 as wire;

    let origin = v2_decode_limit_relay_peer(0x35);
    let target = v2_decode_limit_relay_peer(0x36);
    let manifest = |count| {
        v2_decode_limit_proposal(
            v2_decode_limit_manifest(
                count,
                wire::MAX_DA_PAYLOAD_SIZE_BYTES,
                32 * 1024,
                Hash::new(b"v2-manifest-decode-limit-payload"),
            ),
            vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        )
    };
    let maximum = manifest(wire::MAX_DA_CHUNK_COUNT as usize);
    let maximum_framed_len =
        iroha_p2p::network::data_frame_wire_len(&origin, Some(&target), &maximum);
    assert!(
        maximum_framed_len <= super::MAX_SUMERAGI_V2_CONTROL_NETWORK_FRAME_BYTES,
        "maximum canonical manifest frame encoded to {maximum_framed_len} bytes, above the intrinsic {}-byte cap",
        super::MAX_SUMERAGI_V2_CONTROL_NETWORK_FRAME_BYTES
    );
    let (maximum_encoded, maximum_limits) = v2_decode_limit_policy(&maximum, maximum_framed_len);
    ncore::decode_from_bytes_with_limits::<NetworkMessage>(&maximum_encoded, maximum_limits)
        .expect("maximum canonical manifest must decode under its intrinsic resource policy");

    let oversized = manifest(wire::MAX_DA_CHUNK_COUNT as usize + 1);
    let oversized_framed_len =
        iroha_p2p::network::data_frame_wire_len(&origin, Some(&target), &oversized);
    assert!(
        oversized_framed_len <= super::MAX_SUMERAGI_V2_CONTROL_NETWORK_FRAME_BYTES,
        "oversized hash-count fixture must exercise the sequence guard rather than the frame cap"
    );
    let oversized_encoded =
        ncore::to_bytes(&oversized).expect("encode oversized v2 manifest fixture");
    assert!(ncore::decode_from_bytes::<NetworkMessage>(&oversized_encoded).is_ok());
    let oversized_view =
        ncore::from_bytes_view(&oversized_encoded).expect("inspect oversized v2 manifest fixture");
    let error = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
        oversized_view.as_bytes(),
        oversized_framed_len,
        oversized_view.flags(),
    )
    .expect_err("manifest hash count above the protocol maximum must fail during raw preflight");
    assert!(matches!(
        error,
        ncore::Error::SequenceLengthExceeded { length, limit }
            if length == u64::from(wire::MAX_DA_CHUNK_COUNT) + 1
                && limit == u64::from(wire::MAX_DA_CHUNK_COUNT)
    ));
}

#[test]
fn sumeragi_v2_certified_body_response_raw_fields_enforce_exact_protocol_maxima() {
    use iroha_data_model::block::consensus_v2 as wire;

    let response = |chunk_hash_count: usize, body: Vec<u8>, signature: Vec<u8>| {
        let responder = v2_decode_limit_relay_peer(0x39);
        let manifest = v2_decode_limit_manifest(
            chunk_hash_count,
            body.len() as u64,
            wire::MAX_DA_CHUNK_SIZE_BYTES,
            Hash::new(&body),
        );
        v2_decode_limit_message(wire::ConsensusMessageV2Payload::CertifiedBodyResponse(
            wire::CertifiedBodyResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"v2-adversarial-certified-body-request",
                )),
                manifest,
                body,
                responder,
                signature,
            },
        ))
    };
    let relay = v2_decode_limit_relay_peer(0x3A);
    let target = v2_decode_limit_relay_peer(0x3B);
    for (label, message, oversized_len, expected_limit) in [
        (
            "body",
            response(
                1,
                vec![0xC3; wire::MAX_DA_PAYLOAD_SIZE_BYTES as usize + 1],
                vec![0x5A],
            ),
            wire::MAX_DA_PAYLOAD_SIZE_BYTES as u64 + 1,
            wire::MAX_DA_PAYLOAD_SIZE_BYTES as u64,
        ),
        (
            "manifest chunk hashes",
            response(
                wire::MAX_DA_CHUNK_COUNT as usize + 1,
                vec![0xC3],
                vec![0x5A],
            ),
            wire::MAX_DA_CHUNK_COUNT as u64 + 1,
            wire::MAX_DA_CHUNK_COUNT as u64,
        ),
        (
            "signature",
            response(
                1,
                vec![0xC3],
                vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES + 1],
            ),
            wire::MAX_CONSENSUS_SIGNATURE_BYTES as u64 + 1,
            wire::MAX_CONSENSUS_SIGNATURE_BYTES as u64,
        ),
    ] {
        let framed_len = iroha_p2p::network::data_frame_wire_len(&relay, Some(&target), &message);
        assert!(
            framed_len <= super::MAX_SUMERAGI_V2_CERTIFIED_BODY_RESPONSE_NETWORK_FRAME_BYTES,
            "oversized response {label} fixture must exercise the raw field guard rather than the frame cap"
        );
        let encoded = ncore::to_bytes(&message).expect("encode oversized v2 response fixture");
        assert!(
            ncore::decode_from_bytes::<NetworkMessage>(&encoded).is_ok(),
            "oversized response {label} must remain syntactically decodable without ingress policy"
        );
        let view = ncore::from_bytes_view(&encoded).expect("inspect oversized v2 response fixture");
        let error = <NetworkMessage as ClassifyTopic>::inbound_decode_limits(
            view.as_bytes(),
            framed_len,
            view.flags(),
        )
        .expect_err("oversized response field must fail during raw ingress preflight");
        assert!(
            matches!(
                error,
                ncore::Error::SequenceLengthExceeded { length, limit }
                    if length == oversized_len && limit == expected_limit
            ),
            "unexpected certified response {label} raw-preflight rejection: {error:?}"
        );
    }
}

#[test]
fn sumeragi_v2_certified_body_response_alone_retains_the_seventeen_mib_policy() {
    use iroha_data_model::block::consensus_v2 as wire;

    const MAX_GEOMETRY_CHUNK_BYTES: u32 = 32 * 1024;
    let body = vec![0xC3; wire::MAX_DA_PAYLOAD_SIZE_BYTES as usize];
    let responder = v2_decode_limit_relay_peer(0x37);
    let target = v2_decode_limit_relay_peer(0x38);
    let manifest = v2_decode_limit_manifest(
        wire::MAX_DA_CHUNK_COUNT as usize,
        wire::MAX_DA_PAYLOAD_SIZE_BYTES,
        MAX_GEOMETRY_CHUNK_BYTES,
        Hash::new(&body),
    );
    let message = v2_decode_limit_message(wire::ConsensusMessageV2Payload::CertifiedBodyResponse(
        wire::CertifiedBodyResponse {
            request_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"v2-maximum-certified-body-request",
            )),
            manifest,
            body,
            responder: responder.clone(),
            signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        },
    ));
    let framed_len = iroha_p2p::network::data_frame_wire_len(&responder, Some(&target), &message);
    assert!(
        framed_len > super::MAX_SUMERAGI_V2_CHUNK_NETWORK_FRAME_BYTES,
        "response fixture must prove that the chunk cap is not applied"
    );
    assert!(
        framed_len <= super::MAX_SUMERAGI_V2_CERTIFIED_BODY_RESPONSE_NETWORK_FRAME_BYTES,
        "maximum certified-body response encoded to {framed_len} bytes, above the intrinsic {}-byte cap",
        super::MAX_SUMERAGI_V2_CERTIFIED_BODY_RESPONSE_NETWORK_FRAME_BYTES
    );
    let (encoded, limits) = v2_decode_limit_policy(&message, framed_len);
    assert_eq!(
        limits.max_field_bytes(),
        super::MAX_SUMERAGI_V2_CERTIFIED_BODY_RESPONSE_NETWORK_FRAME_BYTES
    );
    assert_eq!(
        limits.max_sequence_elements(),
        wire::MAX_DA_PAYLOAD_SIZE_BYTES as usize
    );
    let decoded = ncore::decode_from_bytes_with_limits::<NetworkMessage>(&encoded, limits)
        .expect("maximum certified-body response must decode under the sole 17 MiB v2 policy");
    let NetworkMessage::SumeragiBlock(decoded) = decoded else {
        panic!("decoded certified-body response changed its network variant");
    };
    let BlockMessage::V2(decoded) = decoded.as_ref() else {
        panic!("decoded certified-body response changed its block-message variant");
    };
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(decoded) = &decoded.payload else {
        panic!("decoded certified-body response changed its v2 payload variant");
    };
    assert_eq!(decoded.body.len(), wire::MAX_DA_PAYLOAD_SIZE_BYTES as usize);
    assert_eq!(
        decoded.manifest.chunk_hashes.len(),
        wire::MAX_DA_CHUNK_COUNT as usize
    );
}
