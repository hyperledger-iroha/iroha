//! Governance validation, signing, and finance behavior with owned signing fixtures.

use super::*;
use ed25519_dalek::{Signer, SigningKey};
use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature};
use std::error::Error as _;
const SMALL_ORDER_R: [u8; 32] = [
    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const NONCANONICAL_R: [u8; 32] = [
    0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f,
];
#[test]
fn governance_publication_source_pair_identity_binds_every_field() {
    let baseline =
        governance_publication_source_pair_id_v1("repair_audit", 7, [0x11; 32], 9, [0x22; 32]);
    for changed in [
        governance_publication_source_pair_id_v1(
            "reputation_snapshot",
            7,
            [0x11; 32],
            9,
            [0x22; 32],
        ),
        governance_publication_source_pair_id_v1("repair_audit", 8, [0x11; 32], 9, [0x22; 32]),
        governance_publication_source_pair_id_v1("repair_audit", 7, [0x12; 32], 9, [0x22; 32]),
        governance_publication_source_pair_id_v1("repair_audit", 7, [0x11; 32], 10, [0x22; 32]),
        governance_publication_source_pair_id_v1("repair_audit", 7, [0x11; 32], 9, [0x23; 32]),
    ] {
        assert_ne!(changed, baseline);
    }
}
fn encode_bare_with_flags<T: norito::core::NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
    let mut bytes = Vec::new();
    norito::core::serialize_to_buffer(value, &mut bytes).expect("serialize explicit layout");
    bytes
}
fn encode_frame_with_flags<T: norito::core::NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
    norito::to_bytes(value).unwrap_or_else(|error| {
        panic!("serialize explicit canonical frame with flags 0x{flags:02x}: {error}")
    })
}
fn supported_layouts() -> [u8; 8] {
    use norito::core::header_flags::{COMPACT_LEN, FIELD_BITSET, PACKED_SEQ, PACKED_STRUCT};
    [
        0,
        COMPACT_LEN,
        PACKED_SEQ,
        PACKED_SEQ | COMPACT_LEN,
        PACKED_STRUCT,
        PACKED_STRUCT | COMPACT_LEN,
        PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
        PACKED_SEQ | PACKED_STRUCT | COMPACT_LEN | FIELD_BITSET,
    ]
}
fn assert_borrowed_wire_exact<Owned, Borrowed>(label: &str, owned: &Owned, borrowed: &Borrowed)
where
    Owned: norito::core::NoritoSerialize,
    Borrowed: norito::core::NoritoSerialize,
{
    assert_eq!(
        norito::schema::identity::frame_hash::<Borrowed>(),
        norito::schema::identity::frame_hash::<Owned>(),
        "{label} schema hash changed"
    );
    assert_eq!(
        norito::to_bytes(borrowed).expect("encode borrowed canonical frame"),
        norito::to_bytes(owned).expect("encode historical owned canonical frame"),
        "{label} default canonical frame changed"
    );
    for flags in supported_layouts() {
        let owned_bytes = encode_bare_with_flags(owned, flags);
        let borrowed_bytes = encode_bare_with_flags(borrowed, flags);
        assert_eq!(
            borrowed_bytes, owned_bytes,
            "{label} canonical bytes changed for flags 0x{flags:02x}"
        );
        let _guard = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            borrowed.encoded_len_exact(),
            owned.encoded_len_exact(),
            "{label} exact size changed for flags 0x{flags:02x}"
        );
        assert_eq!(
            norito::core::encoded_payload_len(borrowed)
                .expect("borrowed governance payload length must be countable"),
            borrowed_bytes.len(),
            "{label} counted size disagrees with bytes for flags 0x{flags:02x}"
        );
        assert_eq!(
            encode_frame_with_flags(borrowed, flags),
            encode_frame_with_flags(owned, flags),
            "{label} canonical frame or layout flags changed for flags 0x{flags:02x}"
        );
    }
}
#[test]
fn governance_payload_surface_excludes_bare_por_challenges() {
    let compact_source: String = include_str!("../governance.rs")
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let retired_variant = ["PorChallenge", "(PorChallengeV1)"].concat();
    assert!(
        !compact_source.contains(&retired_variant),
        "Governance DAG PoR challenges must use PorChallengePublicationV1"
    );
    assert!(
        compact_source.contains("PorChallengePublication(PorChallengePublicationV1)"),
        "canonical PoR challenge publication variant must remain in the Governance DAG surface"
    );
}
fn signed_por_proof_payload() -> GovernanceLogPayloadV1 {
    GovernanceLogPayloadV1::PorProof(crate::por::PorProofV1 {
        version: crate::POR_PROOF_VERSION_V1,
        challenge_id: [0x11; 32],
        manifest_digest: [0x22; 32],
        provider_id: [0x33; 32],
        samples: vec![crate::por::PorProofSampleV1 {
            sample_index: 7,
            chunk_offset: 4096,
            chunk_size: 1024,
            chunk_digest: [0x44; 32],
            leaf_digest: [0x55; 32],
        }],
        auth_path: vec![[0x66; 32]],
        signature: crate::provider_advert::AdvertSignature {
            algorithm: crate::provider_advert::SignatureAlgorithm::Ed25519,
            public_key: vec![0x77; 32],
            signature: vec![0x88; 64],
        },
        submitted_at: 1_700_000_200,
    })
}
fn governance_node_for_signing() -> GovernanceLogNodeV1 {
    let mut node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid: Vec::new(),
        prev_cid: Some([0x31; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
        timestamp: 1_700_000_300,
        publisher_peer_id: b"12D3KooWGovernancePeer".to_vec(),
        submission_provenance: None,
        payload: signed_por_proof_payload(),
        publisher_signature: GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Dilithium3,
            public_key: vec![0x99; GOVERNANCE_ML_DSA_65_PUBLIC_KEY_BYTES_V1],
            signature: vec![0xAA; GOVERNANCE_ML_DSA_65_SIGNATURE_BYTES_V1],
        },
    };
    node.node_cid = node
        .recompute_node_cid()
        .expect("derive canonical governance log node CID");
    node
}
#[test]
fn governance_log_node_cid_is_stable_and_input_sensitive() {
    let payload = signed_por_proof_payload();
    let prev_cid = [0x41; GOVERNANCE_DAG_CID_BYTES_V1];
    let publisher_peer_id = b"12D3KooWGovernancePeer";
    let first = governance_log_node_cid_v1(
        Some(prev_cid.as_slice()),
        1_700_000_300,
        publisher_peer_id,
        None,
        &payload,
    )
    .expect("derive governance log node CID");
    let second = governance_log_node_cid_v1(
        Some(prev_cid.as_slice()),
        1_700_000_300,
        publisher_peer_id,
        None,
        &payload,
    )
    .expect("derive governance log node CID again");
    let changed = governance_log_node_cid_v1(
        Some(prev_cid.as_slice()),
        1_700_000_301,
        publisher_peer_id,
        None,
        &payload,
    )
    .expect("derive changed governance log node CID");
    assert_eq!(first, second);
    assert_ne!(first, changed);
    assert_eq!(first.len(), blake3::OUT_LEN);
}
pub(super) fn sign_governance_node(node: &mut GovernanceLogNodeV1, seed: &[u8; 32]) {
    let signing_key = SigningKey::from_bytes(seed);
    let payload_bytes = node
        .signature_payload_bytes()
        .expect("encode governance signing payload");
    let signature = signing_key.sign(&payload_bytes);
    node.publisher_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
}
fn sign_governance_node_mldsa(node: &mut GovernanceLogNodeV1, seed: &[u8]) {
    let key_pair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::MlDsa)
        .expect("generate ML-DSA governance keypair");
    let payload_bytes = node
        .signature_payload_bytes()
        .expect("encode governance signing payload");
    let signature = IrohaSignature::try_new(key_pair.private_key(), &payload_bytes)
        .expect("sign governance payload with ML-DSA key");
    let (algorithm, public_key) = key_pair
        .public_key()
        .try_to_bytes()
        .expect("encode ML-DSA public key");
    assert_eq!(algorithm, Algorithm::MlDsa);
    node.publisher_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Dilithium3,
        public_key: public_key.to_vec(),
        signature: signature.payload().to_vec(),
    };
}
fn empty_ed25519_signature() -> GovernanceLogSignatureV1 {
    GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}
#[test]
fn governance_signature_validate_rejects_all_zero_material() {
    let mut signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: vec![0x11; GOVERNANCE_ED25519_PUBLIC_KEY_BYTES_V1],
        signature: vec![0; GOVERNANCE_ED25519_SIGNATURE_BYTES_V1],
    };
    assert!(matches!(
        signature.validate(),
        Err(GovernanceLogValidationError::InvalidSignature)
    ));
    signature.signature = vec![0x22; GOVERNANCE_ED25519_SIGNATURE_BYTES_V1];
    signature.public_key = vec![0; GOVERNANCE_ED25519_PUBLIC_KEY_BYTES_V1];
    assert!(matches!(
        signature.validate(),
        Err(GovernanceLogValidationError::InvalidSignature)
    ));
}
#[test]
fn governance_signature_validate_requires_exact_algorithm_lengths() {
    assert_eq!(
        GOVERNANCE_ML_DSA_65_PUBLIC_KEY_BYTES_V1,
        MlDsaSuite::MlDsa65.public_key_len()
    );
    assert_eq!(
        GOVERNANCE_ML_DSA_65_SIGNATURE_BYTES_V1,
        MlDsaSuite::MlDsa65.signature_len()
    );
    let mut ed25519 = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: vec![0x11; GOVERNANCE_ED25519_PUBLIC_KEY_BYTES_V1],
        signature: vec![0x22; GOVERNANCE_ED25519_SIGNATURE_BYTES_V1],
    };
    ed25519.validate().expect("exact Ed25519 lengths validate");
    ed25519.public_key.push(0x33);
    assert!(matches!(
        ed25519.validate(),
        Err(
            GovernanceLogValidationError::InvalidSignaturePublicKeyLength {
                algorithm: GovernanceSignatureAlgorithm::Ed25519,
                found,
                expected: GOVERNANCE_ED25519_PUBLIC_KEY_BYTES_V1,
            }
        ) if found == GOVERNANCE_ED25519_PUBLIC_KEY_BYTES_V1 + 1
    ));
    let mut mldsa = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Dilithium3,
        public_key: vec![0x44; GOVERNANCE_ML_DSA_65_PUBLIC_KEY_BYTES_V1],
        signature: vec![0x55; GOVERNANCE_ML_DSA_65_SIGNATURE_BYTES_V1],
    };
    mldsa
        .validate()
        .expect("exact ML-DSA-65 lengths validate structurally");
    mldsa.signature.pop();
    assert!(matches!(
        mldsa.validate(),
        Err(GovernanceLogValidationError::InvalidSignatureLength {
            algorithm: GovernanceSignatureAlgorithm::Dilithium3,
            found,
            expected: GOVERNANCE_ML_DSA_65_SIGNATURE_BYTES_V1,
        }) if found + 1 == GOVERNANCE_ML_DSA_65_SIGNATURE_BYTES_V1
    ));
}
#[test]
fn governance_source_and_node_preflights_accept_boundary_and_reject_one_over() {
    let payload = signed_por_proof_payload();
    let payload_len = payload
        .encoded_len_exact()
        .expect("governance source payload exact length");
    assert_eq!(
        preflight_governance_source_payload_len(&payload, payload_len)
            .expect("exact source boundary"),
        payload_len
    );
    assert!(matches!(
        preflight_governance_source_payload_len(&payload, payload_len - 1),
        Err(GovernanceLogValidationError::PayloadTooLarge {
            found,
            maximum,
        }) if found == payload_len && maximum == payload_len - 1
    ));
    let node = governance_node_for_signing();
    let node_len = node
        .encoded_len_exact()
        .expect("governance node exact length");
    assert_eq!(
        preflight_governance_log_node_len(&node, node_len).expect("exact node boundary"),
        node_len
    );
    assert!(matches!(
        preflight_governance_log_node_len(&node, node_len - 1),
        Err(GovernanceLogValidationError::NodeTooLarge {
            found,
            maximum,
        }) if found == node_len && maximum == node_len - 1
    ));
}
pub(super) fn sign_governance_block(block: &mut GovernanceDagBlockV1, seed: &[u8; 32]) {
    let signing_key = SigningKey::from_bytes(seed);
    let payload_bytes = block
        .signature_payload_bytes()
        .expect("encode governance DAG block signing payload");
    let signature = signing_key.sign(&payload_bytes);
    block.block_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
}
pub(super) fn sign_governance_head(head: &mut GovernanceDagHeadV1, seed: &[u8; 32]) {
    let signing_key = SigningKey::from_bytes(seed);
    let payload_bytes = head
        .signature_payload_bytes()
        .expect("encode governance DAG head signing payload");
    let signature = signing_key.sign(&payload_bytes);
    head.head_signature = GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: signing_key.verifying_key().to_bytes().to_vec(),
        signature: signature.to_bytes().to_vec(),
    };
}
pub(super) fn signed_governance_block(
    prev_block_cid: Option<Vec<u8>>,
    prev_node_cid: Option<Vec<u8>>,
    sequence: u64,
    timestamp: u64,
) -> GovernanceDagBlockV1 {
    let mut node = governance_node_for_signing();
    node.prev_cid = prev_node_cid;
    node.timestamp = timestamp;
    let publisher_peer_id = b"12D3KooWGovernanceDagPublisher".to_vec();
    node.publisher_peer_id.clone_from(&publisher_peer_id);
    node.node_cid = node
        .recompute_node_cid()
        .expect("derive governance DAG node CID");
    sign_governance_node(&mut node, &[0xC7; 32]);
    let block_cid = governance_dag_block_cid_v1(
        prev_block_cid.as_deref(),
        sequence,
        timestamp + 10,
        &publisher_peer_id,
        &node,
    )
    .expect("derive governance DAG block CID");
    let mut block = GovernanceDagBlockV1 {
        version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
        block_cid,
        prev_block_cid,
        sequence,
        timestamp: timestamp + 10,
        publisher_peer_id,
        node,
        block_signature: empty_ed25519_signature(),
    };
    sign_governance_block(&mut block, &[0xC7; 32]);
    block
}
#[test]
fn governance_block_preflight_accepts_boundary_and_rejects_one_over() {
    let block = signed_governance_block(None, None, 0, 1_700_000_400);
    let block_len = block
        .encoded_len_exact()
        .expect("governance block exact length");
    assert_eq!(
        preflight_governance_dag_block_len(&block, block_len).expect("exact block boundary"),
        block_len
    );
    assert!(matches!(
        preflight_governance_dag_block_len(&block, block_len - 1),
        Err(GovernanceDagBlockValidationError::BlockTooLarge {
            found,
            maximum,
        }) if found == block_len && maximum == block_len - 1
    ));
}
pub(super) fn signed_governance_head(blocks: &[GovernanceDagBlockV1]) -> GovernanceDagHeadV1 {
    let head_block_cid = blocks.last().expect("at least one block").block_cid.clone();
    let checkpoint_cid = (blocks.len() > GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1).then(|| {
        let checkpoint_index = blocks
            .len()
            .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
            .expect("long fixture history contains a checkpoint window");
        blocks[checkpoint_index].block_cid.clone()
    });
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid,
        block_count: u64::try_from(blocks.len()).expect("fixture block count fits u64"),
        generated_at: 1_700_001_000,
        publisher_peer_id: b"12D3KooWGovernanceDagPublisher".to_vec(),
        checkpoint_cid,
        head_signature: empty_ed25519_signature(),
    };
    sign_governance_head(&mut head, &[0xC7; 32]);
    head
}
#[test]
fn borrowed_governance_dag_views_preserve_every_canonical_wire_and_digest() {
    let root = signed_governance_block(None, None, 0, 1_700_000_400);
    let child = signed_governance_block(
        Some(root.block_cid.clone()),
        Some(root.node.node_cid.clone()),
        1,
        1_700_000_500,
    );
    let mut attributed = signed_governance_block(
        Some(child.block_cid.clone()),
        Some(child.node.node_cid.clone()),
        2,
        1_700_000_600,
    );
    attributed.node.payload =
        GovernanceLogPayloadV1::AppealFinanceReport(sample_appeal_finance_report());
    attributed.node.submission_provenance = Some(GovernanceDagSubmissionProvenanceV1 {
        publisher_account_digest: governance_dag_submission_account_digest_v1(
            b"canonical-norito-account",
        ),
        origin: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
    });
    attributed.node.node_cid = attributed
        .node
        .recompute_node_cid()
        .expect("derive provenance-bearing node CID");
    sign_governance_node(&mut attributed.node, &[0xC7; 32]);
    attributed.block_cid = attributed
        .recompute_block_cid()
        .expect("derive provenance-bearing block CID");
    sign_governance_block(&mut attributed, &[0xC7; 32]);
    for (label, block) in [
        ("root", &root),
        ("child", &child),
        ("attributed", &attributed),
    ] {
        let node = &block.node;
        let owned_node_cid = GovernanceLogNodeCidPayloadV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            prev_cid: node.prev_cid.clone(),
            timestamp: node.timestamp,
            publisher_peer_id: node.publisher_peer_id.clone(),
            submission_provenance: node.submission_provenance.clone(),
            payload: node.payload.clone(),
        };
        let borrowed_node_cid =
            GovernanceLogNodeCidPayloadViewV1(GovernanceLogNodeCidPayloadViewWireV1 {
                version: GOVERNANCE_LOG_VERSION_V1,
                prev_cid: borrowed_norito::Option(node.prev_cid.as_deref()),
                timestamp: node.timestamp,
                publisher_peer_id: borrowed_norito::Vec(&node.publisher_peer_id),
                submission_provenance: node
                    .submission_provenance
                    .as_ref()
                    .map(borrowed_norito::Value),
                payload: borrowed_norito::Value(&node.payload),
            });
        assert_borrowed_wire_exact(
            &format!("{label} governance node CID payload"),
            &owned_node_cid,
            &borrowed_node_cid,
        );
        let owned_node_cid_frame =
            norito::to_bytes(&owned_node_cid).expect("encode historical node CID frame");
        let mut node_hasher = Hasher::new();
        node_hasher.update(GOVERNANCE_LOG_NODE_CID_DOMAIN_V1);
        node_hasher.update(&owned_node_cid_frame);
        assert_eq!(
            node.recompute_node_cid()
                .expect("recompute borrowed node CID"),
            node_hasher.finalize().as_bytes().to_vec(),
            "{label} node CID digest changed"
        );
        let owned_node_signature = GovernanceLogSignaturePayloadV1::from(node);
        let borrowed_node_signature = GovernanceLogSignaturePayloadViewV1::from(node);
        assert_borrowed_wire_exact(
            &format!("{label} governance node signature payload"),
            &owned_node_signature,
            &borrowed_node_signature,
        );
        assert_eq!(
            node.signature_payload_bytes()
                .expect("encode borrowed node signature frame"),
            norito::to_bytes(&owned_node_signature)
                .expect("encode historical node signature frame"),
            "{label} node signature frame changed"
        );
        let owned_block_cid = GovernanceDagBlockCidPayloadV1 {
            version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
            prev_block_cid: block.prev_block_cid.clone(),
            sequence: block.sequence,
            timestamp: block.timestamp,
            publisher_peer_id: block.publisher_peer_id.clone(),
            node: block.node.clone(),
        };
        let borrowed_block_cid =
            GovernanceDagBlockCidPayloadViewV1(GovernanceDagBlockCidPayloadViewWireV1 {
                version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
                prev_block_cid: borrowed_norito::Option(block.prev_block_cid.as_deref()),
                sequence: block.sequence,
                timestamp: block.timestamp,
                publisher_peer_id: borrowed_norito::Vec(&block.publisher_peer_id),
                node: borrowed_norito::Value(&block.node),
            });
        assert_borrowed_wire_exact(
            &format!("{label} governance block CID payload"),
            &owned_block_cid,
            &borrowed_block_cid,
        );
        let owned_block_cid_frame =
            norito::to_bytes(&owned_block_cid).expect("encode historical block CID frame");
        let mut block_hasher = Hasher::new();
        block_hasher.update(GOVERNANCE_DAG_BLOCK_CID_DOMAIN_V1);
        block_hasher.update(&owned_block_cid_frame);
        assert_eq!(
            block
                .recompute_block_cid()
                .expect("recompute borrowed block CID"),
            block_hasher.finalize().as_bytes().to_vec(),
            "{label} block CID digest changed"
        );
        let owned_block_signature = GovernanceDagBlockSignaturePayloadV1::from(block);
        let borrowed_block_signature = GovernanceDagBlockSignaturePayloadViewV1::from(block);
        assert_borrowed_wire_exact(
            &format!("{label} governance block signature payload"),
            &owned_block_signature,
            &borrowed_block_signature,
        );
        assert_eq!(
            block
                .signature_payload_bytes()
                .expect("encode borrowed block signature frame"),
            norito::to_bytes(&owned_block_signature)
                .expect("encode historical block signature frame"),
            "{label} block signature frame changed"
        );
        assert_eq!(
            block.canonical_bytes().expect("stream canonical block"),
            norito::to_bytes(block).expect("encode historical canonical block"),
            "{label} streamed canonical block changed"
        );
        for flags in supported_layouts() {
            let owned_node_cid_frame = norito::encode_canonical(&owned_node_cid)
                .expect("encode fixed-layout node CID frame");
            let mut node_hasher = Hasher::new();
            node_hasher.update(GOVERNANCE_LOG_NODE_CID_DOMAIN_V1);
            node_hasher.update(&owned_node_cid_frame);
            let expected_node_cid = node_hasher.finalize().as_bytes().to_vec();
            let owned_node_signature_frame = norito::encode_canonical(&owned_node_signature)
                .expect("encode fixed-layout node signature frame");
            let owned_block_cid_frame = norito::encode_canonical(&owned_block_cid)
                .expect("encode fixed-layout block CID frame");
            let mut block_hasher = Hasher::new();
            block_hasher.update(GOVERNANCE_DAG_BLOCK_CID_DOMAIN_V1);
            block_hasher.update(&owned_block_cid_frame);
            let expected_block_cid = block_hasher.finalize().as_bytes().to_vec();
            let owned_block_signature_frame = norito::encode_canonical(&owned_block_signature)
                .expect("encode fixed-layout block signature frame");
            let owned_block_frame =
                norito::encode_canonical(block).expect("encode fixed-layout block frame");
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(
                node.recompute_node_cid()
                    .expect("recompute explicit-layout node CID"),
                expected_node_cid,
                "{label} node CID digest changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                node.signature_payload_bytes()
                    .expect("encode explicit-layout node signature frame"),
                owned_node_signature_frame,
                "{label} node signature frame changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                block
                    .recompute_block_cid()
                    .expect("recompute explicit-layout block CID"),
                expected_block_cid,
                "{label} block CID digest changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                block
                    .signature_payload_bytes()
                    .expect("encode explicit-layout block signature frame"),
                owned_block_signature_frame,
                "{label} block signature frame changed for flags 0x{flags:02x}"
            );
            assert_eq!(
                block
                    .canonical_bytes()
                    .expect("stream explicit-layout canonical block"),
                owned_block_frame,
                "{label} streamed block frame changed for flags 0x{flags:02x}"
            );
            node.validate()
                .expect("node validation ignores ambient layout");
            node.verify_publisher_signature()
                .expect("node signature verifies independently of ambient layout");
            block
                .validate()
                .expect("block CID and signatures verify independently of ambient layout");
        }
    }
    let mut heads = [
        GovernanceDagHeadV1 {
            version: GOVERNANCE_DAG_HEAD_VERSION_V1,
            head_block_cid: child.block_cid.clone(),
            block_count: 2,
            generated_at: 1_700_001_000,
            publisher_peer_id: child.publisher_peer_id.clone(),
            checkpoint_cid: None,
            head_signature: empty_ed25519_signature(),
        },
        GovernanceDagHeadV1 {
            version: GOVERNANCE_DAG_HEAD_VERSION_V1,
            head_block_cid: child.block_cid.clone(),
            block_count: 65,
            generated_at: 1_700_001_001,
            publisher_peer_id: child.publisher_peer_id.clone(),
            checkpoint_cid: Some(root.block_cid.clone()),
            head_signature: empty_ed25519_signature(),
        },
    ];
    for head in &mut heads {
        sign_governance_head(head, &[0xC7; 32]);
    }
    for (index, head) in heads.iter().enumerate() {
        let owned = GovernanceDagHeadSignaturePayloadV1::from(head);
        let borrowed = GovernanceDagHeadSignaturePayloadViewV1::from(head);
        assert_borrowed_wire_exact(
            &format!("governance head signature payload {index}"),
            &owned,
            &borrowed,
        );
        assert_eq!(
            head.signature_payload_bytes()
                .expect("encode borrowed head signature frame"),
            norito::to_bytes(&owned).expect("encode historical head signature frame"),
            "head signature frame {index} changed"
        );
        for flags in supported_layouts() {
            let owned_frame =
                norito::encode_canonical(&owned).expect("encode fixed-layout head signature frame");
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(
                head.signature_payload_bytes()
                    .expect("encode explicit-layout head signature frame"),
                owned_frame,
                "head signature frame {index} changed for flags 0x{flags:02x}"
            );
            head.verify_head_signature()
                .expect("head signature verifies independently of ambient layout");
        }
    }
}
fn signed_governance_chain(start_sequence: u64, count: usize) -> Vec<GovernanceDagBlockV1> {
    assert!(count > 0);
    let mut blocks = Vec::with_capacity(count);
    let mut prev_block_cid =
        (start_sequence > 0).then(|| [0x81; GOVERNANCE_DAG_CID_BYTES_V1].to_vec());
    let mut prev_node_cid =
        (start_sequence > 0).then(|| [0x82; GOVERNANCE_DAG_CID_BYTES_V1].to_vec());
    for offset in 0..count {
        let offset = u64::try_from(offset).expect("fixture offset fits u64");
        let sequence = start_sequence
            .checked_add(offset)
            .expect("fixture sequence does not overflow");
        let timestamp = 1_700_000_400_u64
            .checked_add(offset)
            .expect("fixture timestamp does not overflow");
        let block = signed_governance_block(prev_block_cid, prev_node_cid, sequence, timestamp);
        prev_block_cid = Some(block.block_cid.clone());
        prev_node_cid = Some(block.node.node_cid.clone());
        blocks.push(block);
    }
    blocks
}
use crate::deal::{
    DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
    DealSettlementStatusV1, DealSettlementV1, XorQuantity,
};
use crate::reputation::{
    REPUTATION_PROVIDER_INPUT_VERSION_V1, REPUTATION_PROVIDER_METRICS_VERSION_V1,
    ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
    ReputationWeightsV1, build_reputation_snapshot,
    signed::{
        REPUTATION_SCORING_EVIDENCE_VERSION_V1, ReputationScoringEvidenceV1,
        ReputationSnapshotSignatureV1, SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
        SignedReputationSnapshotV1,
    },
};
#[test]
fn governance_node_validation_succeeds() {
    let mut builder = crate::provider_advert::ProviderAdvertV1::builder();
    let range_capability = crate::provider_advert::ProviderCapabilityRangeV1 {
        max_chunk_span: 1_048_576,
        min_granularity: 4_096,
        supports_sparse_offsets: true,
        requires_alignment: false,
        supports_merkle_proof: true,
    };
    let _ = builder
        .profile_id("sorafs.sf1@1.0.0")
        .profile_aliases(vec![
            "sorafs.sf1@1.0.0".to_string(),
            "sorafs-sf1".to_string(),
        ])
        .provider_id([5; 32])
        .stake_pool_id([6; 32])
        .stake_amount(
            crate::deal::XorQuantity::try_from_micro(1_000_000)
                .expect("fixture stake is representable"),
        )
        .availability(crate::provider_advert::AvailabilityTier::Hot)
        .max_retrieval_latency_ms(250)
        .max_concurrent_streams(32)
        .add_capability(crate::provider_advert::CapabilityTlv {
            cap_type: crate::provider_advert::CapabilityType::ToriiGateway,
            payload: Vec::new(),
        })
        .add_range_capability(range_capability)
        .expect("range capability")
        .add_endpoint(crate::provider_advert::AdvertEndpoint {
            kind: crate::provider_advert::EndpointKind::Torii,
            host_pattern: "gateway.sora".to_string(),
            metadata: Vec::new(),
        })
        .add_topic(crate::provider_advert::RendezvousTopic {
            topic: "sorafs.sf1.primary".to_string(),
            region: "global".to_string(),
        })
        .path_policy_min_guard_weight(5)
        .path_policy_max_same_asn_per_path(2)
        .path_policy_max_same_pool_per_path(1)
        .stream_budget(crate::provider_advert::StreamBudgetV1 {
            max_in_flight: 4,
            max_bytes_per_sec: 512_000,
            burst_bytes: Some(64_000),
        })
        .add_transport_hint(crate::provider_advert::TransportHintV1 {
            protocol: crate::provider_advert::TransportProtocol::ToriiHttpRange,
            priority: 0,
        })
        .issued_at(1_700_000_000)
        .ttl_secs(3_600);
    let _ = builder.signature(
        crate::provider_advert::SignatureAlgorithm::Ed25519,
        vec![9; 32],
        vec![10; 64],
    );
    let advert = builder.build().expect("valid advert");
    let mut node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid: Vec::new(),
        prev_cid: Some([0x32; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
        timestamp: 1_700_000_100,
        publisher_peer_id: b"12D3KooWGovernancePeer".to_vec(),
        submission_provenance: None,
        payload: GovernanceLogPayloadV1::ProviderAdvert(advert),
        publisher_signature: empty_ed25519_signature(),
    };
    node.node_cid = node
        .recompute_node_cid()
        .expect("derive governance log node CID");
    sign_governance_node_mldsa(&mut node, &[0x12; 32]);
    node.validate().expect("valid signed governance node");
    node.verify_publisher_signature()
        .expect("fixture ML-DSA governance signature verifies");
}
#[test]
fn governance_signing_payload_limit_dominates_largest_embedded_envelope() {
    assert_eq!(
        GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1,
        MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES * 2
    );
    assert_eq!(
        GOVERNANCE_DAG_SOURCE_PAYLOAD_MAX_CANONICAL_BYTES_V1,
        MAX_SIGNED_REPUTATION_SNAPSHOT_ENCODED_BYTES
    );
    validate_governance_dag_signing_payload_len(GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1)
        .expect("the exact V1 signing ceiling must be admitted");
    assert!(
        validate_governance_dag_signing_payload_len(
            GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1 + 1
        )
        .is_err(),
        "one byte above the V1 signing ceiling must fail"
    );
}
#[test]
pub(super) fn governance_signing_payload_requires_allocation_free_exact_size() {
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "sorafs_manifest::governance::tests::governance_signing_payload_requires_allocation_free_exact_size::InexactSigningPayload"
    )]
    struct InexactSigningPayload;
    crate::signing_identity_test_support::check_rejected_identity::<InexactSigningPayload>(
        "governance/inexact",
    );

    impl norito::SerializePayload for InexactSigningPayload {
        fn serialize(
            &self,
            writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            writer.write_all(&[0x01])?;
            Ok(())
        }
    }
    let error = encode_governance_dag_signing_payload(&InexactSigningPayload)
        .expect_err("an inexact payload must fail before encoding");
    assert!(
        error
            .to_string()
            .contains("has no allocation-free exact size")
    );
}
#[test]
pub(super) fn governance_signing_payload_rejects_oversize_before_serialize_or_allocate() {
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "sorafs_manifest::governance::tests::governance_signing_payload_rejects_oversize_before_serialize_or_allocate::OversizedSigningPayload"
    )]
    struct OversizedSigningPayload;
    crate::signing_identity_test_support::check_rejected_identity::<OversizedSigningPayload>(
        "governance/oversized",
    );

    impl norito::SerializePayload for OversizedSigningPayload {
        fn serialize(
            &self,
            _writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            panic!("oversized payload must be rejected before serialization")
        }
        fn encoded_len_exact(&self) -> Option<usize> {
            Some(GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1 + 1)
        }
    }
    let error = encode_governance_dag_signing_payload(&OversizedSigningPayload)
        .expect_err("one byte over the signing ceiling must fail before serialization");
    assert!(
        error.to_string().contains("exceeding V1 limit"),
        "unexpected oversize error: {error}"
    );
}
#[test]
fn governance_block_ceiling_adds_checked_fixed_envelope_allowance() {
    assert_eq!(
        GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1,
        GOVERNANCE_DAG_SIGNING_PAYLOAD_MAX_BYTES_V1
            .checked_add(GOVERNANCE_DAG_BLOCK_ENVELOPE_MAX_BYTES_V1)
            .expect("the V1 block ceiling must fit usize")
    );
    validate_governance_dag_block_canonical_len(GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1)
        .expect("the exact V1 block ceiling must be admitted");
    assert!(
        validate_governance_dag_block_canonical_len(
            GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 + 1
        )
        .is_err(),
        "one byte above the V1 block ceiling must fail"
    );
    let block = signed_governance_block(None, None, 0, 1_700_000_400);
    let signing_bytes = block
        .signature_payload_bytes()
        .expect("encode bounded block signing payload");
    let canonical = block.canonical_bytes().expect("encode bounded block");
    assert_eq!(
        canonical,
        norito::to_bytes(&block).expect("encode canonical block directly")
    );
    assert!(
        canonical.len().saturating_sub(signing_bytes.len())
            <= GOVERNANCE_DAG_BLOCK_ENVELOPE_MAX_BYTES_V1
    );
    assert!(block.recompute_block_cid().is_ok());
    assert!(block.node.recompute_node_cid().is_ok());
}
#[test]
fn governance_signature_payload_excludes_publisher_signature() {
    let node = governance_node_for_signing();
    let mut different_signature = node.clone();
    different_signature.publisher_signature.signature = vec![0xBB; 96];
    assert_eq!(
        node.signature_payload_bytes()
            .expect("encode governance signature payload"),
        different_signature
            .signature_payload_bytes()
            .expect("encode governance signature payload")
    );
    let mut different_payload = node.clone();
    different_payload.timestamp += 1;
    assert_ne!(
        node.signature_payload_bytes()
            .expect("encode governance signature payload"),
        different_payload
            .signature_payload_bytes()
            .expect("encode governance signature payload")
    );
}
#[test]
fn verify_publisher_signature_accepts_ed25519_signed_node() {
    let seed = [0xA5; 32];
    let mut node = governance_node_for_signing();
    sign_governance_node(&mut node, &seed);
    node.verify_publisher_signature()
        .expect("governance node signature verifies");
}
#[test]
fn verify_publisher_signature_rejects_tampered_payload() {
    let seed = [0xA5; 32];
    let mut node = governance_node_for_signing();
    sign_governance_node(&mut node, &seed);
    node.timestamp += 1;
    assert!(matches!(
        node.verify_publisher_signature(),
        Err(GovernanceLogSignatureVerificationError::Verification { .. })
    ));
}
#[test]
fn verify_publisher_signature_rejects_all_zero_ed25519_signature_material() {
    let seed = [0xA5; 32];
    let mut node = governance_node_for_signing();
    sign_governance_node(&mut node, &seed);
    node.publisher_signature.signature.fill(0);
    let err = node
        .verify_publisher_signature()
        .expect_err("all-zero governance node signature must be rejected");
    assert!(matches!(
        err,
        GovernanceLogSignatureVerificationError::Verification { reason }
            if reason.contains("all zero")
    ));
}
#[test]
fn verify_publisher_signature_rejects_malformed_ed25519_signature_r() {
    for (label, replacement_r, expected_reason) in [
        ("small-order", SMALL_ORDER_R, "small-order"),
        ("noncanonical", NONCANONICAL_R, "not a canonical"),
    ] {
        let seed = [0xA5; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node(&mut node, &seed);
        node.publisher_signature.signature[..PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);
        let err = node
            .verify_publisher_signature()
            .expect_err("malformed governance publisher signature R must be rejected");
        assert!(
            matches!(
                &err,
                GovernanceLogSignatureVerificationError::Verification { reason }
                    if reason.contains(expected_reason)
            ),
            "{label} signature R produced unexpected error: {err}"
        );
    }
}
#[test]
fn verify_publisher_signature_accepts_dilithium3_signed_node() {
    let seed = [0xB5; 32];
    let mut node = governance_node_for_signing();
    sign_governance_node_mldsa(&mut node, &seed);
    node.verify_publisher_signature()
        .expect("ML-DSA governance node signature verifies");
}
#[test]
fn verify_publisher_signature_rejects_tampered_dilithium3_payload() {
    let seed = [0xB5; 32];
    let mut node = governance_node_for_signing();
    sign_governance_node_mldsa(&mut node, &seed);
    node.publisher_peer_id.extend_from_slice(b"-tampered");
    assert!(matches!(
        node.verify_publisher_signature(),
        Err(GovernanceLogSignatureVerificationError::Verification { .. })
    ));
}
#[test]
fn verify_publisher_signature_rejects_all_zero_dilithium3_signature_material() {
    let seed = [0xB5; 32];
    let mut node = governance_node_for_signing();
    sign_governance_node_mldsa(&mut node, &seed);
    node.publisher_signature.signature.fill(0);
    assert!(matches!(
        node.verify_publisher_signature(),
        Err(GovernanceLogSignatureVerificationError::Verification { .. })
    ));
}
#[test]
fn verify_publisher_signature_rejects_malformed_dilithium3_signature_lengths() {
    for label in ["short", "overlong"] {
        let seed = [0xB6; 32];
        let mut node = governance_node_for_signing();
        sign_governance_node_mldsa(&mut node, &seed);
        match label {
            "short" => {
                node.publisher_signature
                    .signature
                    .pop()
                    .expect("signed ML-DSA fixture is non-empty");
            }
            "overlong" => node.publisher_signature.signature.push(0xA5),
            _ => unreachable!("covered labels"),
        }
        let err = node
            .verify_publisher_signature()
            .expect_err("malformed ML-DSA governance signature length must be rejected");
        assert!(
            matches!(
                &err,
                GovernanceLogSignatureVerificationError::Verification { reason }
                    if reason.contains("signature")
            ),
            "{label} ML-DSA governance signature produced unexpected error: {err}"
        );
    }
}
#[test]
fn governance_dag_block_derives_cid_and_verifies_signature() {
    let block = signed_governance_block(None, None, 0, 1_700_000_400);
    block.validate().expect("valid governance DAG block");
    assert_eq!(
        block
            .recompute_block_cid()
            .expect("recompute governance DAG block CID"),
        block.block_cid
    );
    block
        .verify_block_signature()
        .expect("block signature verifies");
}
#[test]
fn governance_dag_block_rejects_all_zero_signature_material() {
    let mut block = signed_governance_block(None, None, 0, 1_700_000_400);
    block.block_signature.signature.fill(0);
    let err = block
        .verify_block_signature()
        .expect_err("all-zero governance block signature must be rejected");
    assert!(matches!(
        err,
        GovernanceLogSignatureVerificationError::Verification { reason }
            if reason.contains("all zero")
    ));
}
#[test]
fn governance_dag_block_signature_payload_excludes_signature() {
    let block = signed_governance_block(None, None, 0, 1_700_000_400);
    let mut different_signature = block.clone();
    different_signature.block_signature.signature = vec![0xEE; 64];
    assert_eq!(
        block
            .signature_payload_bytes()
            .expect("encode block signature payload"),
        different_signature
            .signature_payload_bytes()
            .expect("encode block signature payload")
    );
    let mut different_payload = block.clone();
    different_payload.sequence = 1;
    assert_ne!(
        block
            .signature_payload_bytes()
            .expect("encode block signature payload"),
        different_payload
            .signature_payload_bytes()
            .expect("encode block signature payload")
    );
}
#[test]
fn governance_dag_block_rejects_tampered_cid() {
    let mut block = signed_governance_block(None, None, 0, 1_700_000_400);
    block.block_cid[0] ^= 0x01;
    assert!(matches!(
        block.validate(),
        Err(GovernanceDagBlockValidationError::InvalidBlockCid)
    ));
}
#[test]
fn governance_log_node_requires_exact_cids_and_bounded_peer_id() {
    let mut node = governance_node_for_signing();
    node.node_cid.pop();
    assert!(matches!(
        node.validate(),
        Err(GovernanceLogValidationError::InvalidNodeCidLength { length: 31 })
    ));
    let mut node = governance_node_for_signing();
    node.prev_cid = Some(vec![0x41; GOVERNANCE_DAG_CID_BYTES_V1 + 1]);
    assert!(matches!(
        node.validate(),
        Err(GovernanceLogValidationError::InvalidPrevCidLength { length: 33 })
    ));
    let mut node = governance_node_for_signing();
    node.publisher_peer_id = vec![0x42; GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 + 1];
    assert!(matches!(
        node.validate(),
        Err(GovernanceLogValidationError::PublisherPeerIdTooLong { .. })
    ));
}
#[test]
fn governance_log_node_rejects_noncanonical_cid() {
    let mut node = governance_node_for_signing();
    node.node_cid[0] ^= 0x01;
    assert!(matches!(
        node.validate(),
        Err(GovernanceLogValidationError::InvalidNodeCid)
    ));
}
#[test]
fn governance_dag_block_requires_exact_cids_and_one_ed25519_identity() {
    let block = signed_governance_block(None, None, 0, 1_700_000_400);
    let mut invalid_cid = block.clone();
    invalid_cid.block_cid.push(0);
    assert!(matches!(
        invalid_cid.validate(),
        Err(GovernanceDagBlockValidationError::InvalidBlockCidLength { length: 33 })
    ));
    let mut invalid_prev = signed_governance_block(
        Some([0x61; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
        Some([0x62; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
        1,
        1_700_000_401,
    );
    invalid_prev.prev_block_cid = Some(vec![0x61; 31]);
    assert!(matches!(
        invalid_prev.validate(),
        Err(GovernanceDagBlockValidationError::InvalidPrevBlockCidLength { length: 31 })
    ));
    let mut oversized_peer = block.clone();
    oversized_peer.publisher_peer_id =
        vec![0x42; GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 + 1];
    assert!(matches!(
        oversized_peer.validate(),
        Err(GovernanceDagBlockValidationError::PublisherPeerIdTooLong { .. })
    ));
    let mut invalid_algorithm = block.clone();
    invalid_algorithm.block_signature.algorithm = GovernanceSignatureAlgorithm::Dilithium3;
    assert!(matches!(
        invalid_algorithm.validate(),
        Err(GovernanceDagBlockValidationError::NonEd25519BlockSignature)
    ));
    let mut invalid_peer = block.clone();
    invalid_peer.node.publisher_peer_id[0] ^= 0x01;
    invalid_peer.node.node_cid = invalid_peer
        .node
        .recompute_node_cid()
        .expect("recompute node CID");
    sign_governance_node(&mut invalid_peer.node, &[0xC7; 32]);
    assert!(matches!(
        invalid_peer.validate(),
        Err(GovernanceDagBlockValidationError::NodePublisherPeerMismatch)
    ));
    let mut invalid_key = block;
    sign_governance_node(&mut invalid_key.node, &[0xD7; 32]);
    invalid_key.block_cid = invalid_key
        .recompute_block_cid()
        .expect("recompute block CID");
    sign_governance_block(&mut invalid_key, &[0xC7; 32]);
    assert!(matches!(
        invalid_key.validate(),
        Err(GovernanceDagBlockValidationError::NodePublisherKeyMismatch)
    ));
}
#[test]
fn governance_dag_head_requires_exact_cids_bounded_peer_and_ed25519() {
    let blocks = signed_governance_chain(0, 1);
    let head = signed_governance_head(&blocks);
    let mut invalid_cid = head.clone();
    invalid_cid.head_block_cid.pop();
    assert!(matches!(
        invalid_cid.validate(),
        Err(GovernanceDagHeadValidationError::InvalidHeadBlockCidLength { length: 31 })
    ));
    let mut invalid_checkpoint = head.clone();
    invalid_checkpoint.checkpoint_cid = Some(vec![0x55; 31]);
    assert!(matches!(
        invalid_checkpoint.validate(),
        Err(GovernanceDagHeadValidationError::InvalidCheckpointCidLength { length: 31 })
    ));
    let mut oversized_peer = head.clone();
    oversized_peer.publisher_peer_id =
        vec![0x44; GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 + 1];
    assert!(matches!(
        oversized_peer.validate(),
        Err(GovernanceDagHeadValidationError::PublisherPeerIdTooLong { .. })
    ));
    let mut invalid_algorithm = head;
    invalid_algorithm.head_signature.algorithm = GovernanceSignatureAlgorithm::Dilithium3;
    assert!(matches!(
        invalid_algorithm.validate(),
        Err(GovernanceDagHeadValidationError::NonEd25519HeadSignature)
    ));
}
#[test]
fn governance_dag_chain_validates_parent_linkage_and_head() {
    let root = signed_governance_block(None, None, 0, 1_700_000_400);
    let child = signed_governance_block(
        Some(root.block_cid.clone()),
        Some(root.node.node_cid.clone()),
        1,
        1_700_000_500,
    );
    let blocks = vec![root, child];
    let expected_head = blocks[1].block_cid.clone();
    validate_governance_dag_chain_v1(&blocks, Some(&expected_head))
        .expect("valid governance DAG chain");
}
#[test]
fn governance_dag_chain_preserves_invalid_block_error_source() {
    let mut block = signed_governance_block(None, None, 0, 1_700_000_400);
    block.block_cid[0] ^= 0x01;
    let error = validate_governance_dag_chain_v1(&[block], None)
        .expect_err("a block with a tampered CID must fail chain validation");
    const SOURCE_MESSAGE: &str =
        "governance DAG block CID does not match the canonical block payload";
    assert_eq!(
        error.to_string(),
        format!("block at index 0 failed validation: {SOURCE_MESSAGE}")
    );
    assert_eq!(
        error
            .source()
            .expect("invalid block error retains its source")
            .to_string(),
        SOURCE_MESSAGE
    );
    assert!(matches!(
        &error,
        GovernanceDagChainValidationError::InvalidBlock { index: 0, source }
            if matches!(
                source.as_ref(),
                GovernanceDagBlockValidationError::InvalidBlockCid
            )
    ));
}
#[test]
fn governance_dag_chain_accepts_external_tail_anchors() {
    let block =
        signed_governance_block(Some(vec![0xA5; 32]), Some(vec![0x5A; 32]), 1, 1_700_000_500);
    validate_governance_dag_chain_v1(&[block], None)
        .expect("the first checkpoint-tail block may reference external parents");
}
#[test]
fn governance_dag_chain_rejects_noncanonical_order() {
    let root = signed_governance_block(None, None, 0, 1_700_000_400);
    let child = signed_governance_block(
        Some(root.block_cid.clone()),
        Some(root.node.node_cid.clone()),
        1,
        1_700_000_500,
    );
    let blocks = vec![child, root];
    assert!(matches!(
        validate_governance_dag_chain_v1(&blocks, None),
        Err(GovernanceDagChainValidationError::NonCanonicalOrder { index: 1 })
    ));
}
#[test]
fn governance_dag_chain_rejects_duplicate_node_cid() {
    let first = signed_governance_block(None, None, 0, 1_700_000_400);
    let mut duplicate = first.clone();
    duplicate.timestamp = duplicate
        .timestamp
        .checked_add(1)
        .expect("fixture timestamp does not overflow");
    duplicate.block_cid = duplicate
        .recompute_block_cid()
        .expect("recompute duplicate-node block CID");
    sign_governance_block(&mut duplicate, &[0xC7; 32]);
    assert!(matches!(
        validate_governance_dag_chain_v1(&[first, duplicate], None),
        Err(GovernanceDagChainValidationError::DuplicateNodeCid { index: 1 })
    ));
}
#[test]
fn governance_dag_chain_rejects_node_parent_discontinuity() {
    let root = signed_governance_block(None, None, 0, 1_700_000_400);
    let child = signed_governance_block(
        Some(root.block_cid.clone()),
        Some([0x52; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
        1,
        1_700_000_500,
    );
    assert!(matches!(
        validate_governance_dag_chain_v1(&[root, child], None),
        Err(GovernanceDagChainValidationError::NodeParentMismatch { index: 1 })
    ));
}
#[test]
fn governance_dag_chain_rejects_publisher_peer_or_key_drift() {
    let blocks = signed_governance_chain(0, 2);
    let mut peer_drift = blocks.clone();
    let child = &mut peer_drift[1];
    child.publisher_peer_id = b"12D3KooWGovernanceDagPublisherOther".to_vec();
    child
        .node
        .publisher_peer_id
        .clone_from(&child.publisher_peer_id);
    child.node.node_cid = child
        .node
        .recompute_node_cid()
        .expect("recompute peer-drift node CID");
    sign_governance_node(&mut child.node, &[0xC7; 32]);
    child.block_cid = child
        .recompute_block_cid()
        .expect("recompute peer-drift block CID");
    sign_governance_block(child, &[0xC7; 32]);
    assert!(matches!(
        validate_governance_dag_chain_v1(&peer_drift, None),
        Err(GovernanceDagChainValidationError::PublisherPeerMismatch { index: 1 })
    ));
    let mut key_drift = blocks;
    let child = &mut key_drift[1];
    sign_governance_node(&mut child.node, &[0xD7; 32]);
    child.block_cid = child
        .recompute_block_cid()
        .expect("recompute key-drift block CID");
    sign_governance_block(child, &[0xD7; 32]);
    assert!(matches!(
        validate_governance_dag_chain_v1(&key_drift, None),
        Err(GovernanceDagChainValidationError::PublisherKeyMismatch { index: 1 })
    ));
}
#[test]
fn governance_dag_chain_rejects_sequence_overflow() {
    let first = signed_governance_block(
        Some([0x91; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
        Some([0x92; GOVERNANCE_DAG_CID_BYTES_V1].to_vec()),
        u64::MAX,
        1_700_000_400,
    );
    let second = signed_governance_block(
        Some(first.block_cid.clone()),
        Some(first.node.node_cid.clone()),
        u64::MAX,
        1_700_000_401,
    );
    assert!(matches!(
        validate_governance_dag_chain_v1(&[first, second], None),
        Err(GovernanceDagChainValidationError::SequenceOverflow { index: 1 })
    ));
}
#[test]
fn governance_dag_head_manifest_signs_and_binds_chain() {
    let root = signed_governance_block(None, None, 0, 1_700_000_400);
    let child = signed_governance_block(
        Some(root.block_cid.clone()),
        Some(root.node.node_cid.clone()),
        1,
        1_700_000_500,
    );
    let blocks = vec![root, child];
    let head = signed_governance_head(&blocks);
    head.validate().expect("valid governance DAG head");
    head.verify_head_signature()
        .expect("head signature verifies");
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("head binds the governance DAG chain");
}
#[test]
fn governance_dag_head_accepts_a_tip_from_a_rotated_authority() {
    let mut blocks = signed_governance_chain(0, 2);
    let rotated_peer_id = {
        let child = &mut blocks[1];
        child.publisher_peer_id = b"12D3KooWGovernanceDagRotatedPublisher".to_vec();
        child
            .node
            .publisher_peer_id
            .clone_from(&child.publisher_peer_id);
        child.node.node_cid = child
            .node
            .recompute_node_cid()
            .expect("recompute rotated node CID");
        sign_governance_node(&mut child.node, &[0xD7; 32]);
        child.block_cid = child
            .recompute_block_cid()
            .expect("recompute rotated block CID");
        sign_governance_block(child, &[0xD7; 32]);
        child.publisher_peer_id.clone()
    };
    assert!(matches!(
        validate_governance_dag_chain_v1(&blocks, None),
        Err(GovernanceDagChainValidationError::PublisherPeerMismatch { index: 1 })
    ));
    let mut head = signed_governance_head(&blocks);
    head.publisher_peer_id = rotated_peer_id;
    sign_governance_head(&mut head, &[0xD7; 32]);
    assert!(matches!(
        validate_governance_dag_head_against_chain_v1(&head, &blocks),
        Err(GovernanceDagHeadChainValidationError::Chain(
            GovernanceDagChainValidationError::PublisherPeerMismatch { index: 1 }
        ))
    ));
    validate_governance_dag_head_against_rotatable_chain_v1(&head, &blocks)
        .expect("head accepts a tip whose separate rotation policy authenticated its authority");
}
#[test]
fn governance_dag_head_binds_full_history_checkpoint_window() {
    let blocks = signed_governance_chain(
        0,
        GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1
            .checked_add(1)
            .expect("fixture count does not overflow"),
    );
    let head = signed_governance_head(&blocks);
    let checkpoint_index = blocks
        .len()
        .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
        .expect("full history contains checkpoint window");
    assert_eq!(
        head.checkpoint_cid.as_deref(),
        Some(blocks[checkpoint_index].block_cid.as_slice())
    );
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("full history binds its newest checkpoint window");
}
#[test]
fn governance_dag_head_accepts_exact_checkpoint_tail() {
    let full = signed_governance_chain(
        0,
        GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1
            .checked_add(1)
            .expect("fixture count does not overflow"),
    );
    let head = signed_governance_head(&full);
    let tail_start = full
        .len()
        .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
        .expect("full history contains checkpoint window");
    let tail = &full[tail_start..];
    validate_governance_dag_head_against_chain_v1(&head, tail)
        .expect("exact newest checkpoint tail binds the signed head");
}
#[test]
fn governance_dag_head_rejects_short_or_misanchored_checkpoint_tail() {
    let full = signed_governance_chain(
        0,
        GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1
            .checked_add(1)
            .expect("fixture count does not overflow"),
    );
    let mut head = signed_governance_head(&full);
    let tail_start = full
        .len()
        .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
        .expect("full history contains checkpoint window");
    let tail = &full[tail_start..];
    assert!(matches!(
        validate_governance_dag_head_against_chain_v1(&head, &tail[1..]),
        Err(GovernanceDagHeadChainValidationError::CheckpointWindowLength { count: 63 })
    ));
    head.checkpoint_cid = Some([0xE5; GOVERNANCE_DAG_CID_BYTES_V1].to_vec());
    sign_governance_head(&mut head, &[0xC7; 32]);
    assert!(matches!(
        validate_governance_dag_head_against_chain_v1(&head, tail),
        Err(GovernanceDagHeadChainValidationError::CheckpointMismatch)
    ));
}
#[test]
fn governance_dag_head_rejects_checkpoint_tail_sequence_mismatch() {
    let full = signed_governance_chain(
        0,
        GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1
            .checked_add(1)
            .expect("fixture count does not overflow"),
    );
    let mut head = signed_governance_head(&full);
    head.block_count = head
        .block_count
        .checked_add(1)
        .expect("fixture block count does not overflow");
    sign_governance_head(&mut head, &[0xC7; 32]);
    let tail_start = full
        .len()
        .checked_sub(GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1)
        .expect("full history contains checkpoint window");
    assert!(matches!(
        validate_governance_dag_head_against_chain_v1(&head, &full[tail_start..]),
        Err(
            GovernanceDagHeadChainValidationError::CheckpointStartSequence {
                expected: 2,
                sequence: 1
            }
        )
    ));
    let mut missing_generated_at = head.clone();
    missing_generated_at.generated_at = 0;
    assert!(matches!(
        missing_generated_at.validate(),
        Err(GovernanceDagHeadValidationError::MissingGeneratedAt)
    ));
}
#[test]
fn governance_dag_head_rejects_checkpoint_for_short_full_history() {
    let blocks = signed_governance_chain(0, 2);
    let mut head = signed_governance_head(&blocks);
    head.checkpoint_cid = Some(blocks[0].block_cid.clone());
    sign_governance_head(&mut head, &[0xC7; 32]);
    assert!(matches!(
        validate_governance_dag_head_against_chain_v1(&head, &blocks),
        Err(GovernanceDagHeadChainValidationError::UnexpectedCheckpoint)
    ));
}
#[test]
fn governance_dag_head_rejects_signer_identity_drift() {
    let blocks = signed_governance_chain(0, 2);
    let mut head = signed_governance_head(&blocks);
    sign_governance_head(&mut head, &[0xD9; 32]);
    assert!(matches!(
        validate_governance_dag_head_against_chain_v1(&head, &blocks),
        Err(GovernanceDagHeadChainValidationError::PublisherKeyMismatch)
    ));
    let mut head = signed_governance_head(&blocks);
    head.publisher_peer_id = b"12D3KooWGovernanceDagPublisherOther".to_vec();
    sign_governance_head(&mut head, &[0xC7; 32]);
    assert!(matches!(
        validate_governance_dag_head_against_chain_v1(&head, &blocks),
        Err(GovernanceDagHeadChainValidationError::PublisherPeerMismatch)
    ));
}
#[test]
fn governance_dag_head_rejects_generated_at_before_tip() {
    let blocks = signed_governance_chain(0, 2);
    let mut head = signed_governance_head(&blocks);
    let tip_timestamp = blocks.last().expect("fixture has tip").timestamp;
    head.generated_at = tip_timestamp
        .checked_sub(1)
        .expect("fixture tip timestamp is positive");
    sign_governance_head(&mut head, &[0xC7; 32]);
    assert!(matches!(
        validate_governance_dag_head_against_chain_v1(&head, &blocks),
        Err(
            GovernanceDagHeadChainValidationError::HeadTimestampBeforeTip {
                head_generated_at,
                tip_timestamp: observed_tip
            }
        ) if head_generated_at.checked_add(1) == Some(observed_tip)
    ));
}
#[test]
fn governance_dag_head_rejects_all_zero_signature_material() {
    let root = signed_governance_block(None, None, 0, 1_700_000_400);
    let blocks = vec![root];
    let mut head = signed_governance_head(&blocks);
    head.head_signature.signature.fill(0);
    let err = head
        .verify_head_signature()
        .expect_err("all-zero governance head signature must be rejected");
    assert!(matches!(
        err,
        GovernanceLogSignatureVerificationError::Verification { reason }
            if reason.contains("all zero")
    ));
}
#[test]
fn governance_dag_head_rejects_block_count_mismatch() {
    let root = signed_governance_block(None, None, 0, 1_700_000_400);
    let blocks = vec![root];
    let mut head = signed_governance_head(&blocks);
    head.block_count += 1;
    sign_governance_head(&mut head, &[0xC7; 32]);
    assert!(matches!(
        validate_governance_dag_head_against_chain_v1(&head, &blocks),
        Err(GovernanceDagHeadChainValidationError::BlockCountMismatch {
            head_count: 2,
            chain_count: 1
        })
    ));
}
#[test]
fn governance_payload_accepts_deal_settlement() {
    let xor_nanos = |value: u128| -> XorQuantity {
        let whole = value / 1_000_000_000;
        let fractional = value % 1_000_000_000;
        format!("{whole}.{fractional:09}")
            .parse()
            .expect("nano-XOR fixture is canonical")
    };
    let mut ledger = DealLedgerSnapshotV1 {
        version: DEAL_LEDGER_VERSION_V1,
        snapshot_id: [0; 32],
        sequence: 1,
        previous_snapshot_id: None,
        deal_id: [0xAA; 32],
        terms_digest: [0x44; 32],
        provider_id: [0xBB; 32],
        client_id: [0xCC; 32],
        deal_start_epoch: 1_700_199_900,
        deal_end_epoch: 1_700_199_999,
        settlement_window_epochs: 100,
        window_start_epoch: 1_700_199_900,
        window_end_epoch: 1_700_200_000,
        provider_accrual: xor_nanos(100),
        client_liability: xor_nanos(100),
        micropayment_credit_generated: XorQuantity::zero(),
        micropayment_credit_applied: XorQuantity::zero(),
        micropayment_credit_carry: XorQuantity::zero(),
        client_debit: xor_nanos(100),
        outstanding_liability: XorQuantity::zero(),
        bond_total: xor_nanos(50),
        bond_locked: XorQuantity::zero(),
        bond_slashed: XorQuantity::zero(),
        bond_released: xor_nanos(50),
        window_expected_charge: xor_nanos(100),
        window_micropayment_generated: XorQuantity::zero(),
        window_micropayment_applied: XorQuantity::zero(),
        window_client_debit: xor_nanos(100),
        window_bond_slashed: XorQuantity::zero(),
        window_bond_released: xor_nanos(50),
        captured_at: 1_700_200_000,
    };
    ledger.snapshot_id = ledger.derive_snapshot_id().expect("ledger id");
    let mut settlement = DealSettlementV1 {
        version: DEAL_SETTLEMENT_VERSION_V1,
        settlement_id: [0; 32],
        deal_id: [0xAA; 32],
        ledger,
        status: DealSettlementStatusV1::Completed,
        settled_at: 1_700_200_000,
        audit_notes: None,
    };
    settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
    let payload = GovernanceLogPayloadV1::DealSettlement(Box::new(settlement));
    payload.validate(1_700_200_200).expect("valid settlement");
}
#[test]
fn governance_payload_accepts_reputation_snapshot() {
    let input = ReputationProviderInputV1 {
        version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
        provider_id: "provider-a".to_string(),
        metrics: ReputationProviderMetricsV1 {
            version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
            por_success_bps: 9_600,
            pdp_success_bps: 9_700,
            potr_success_bps: 9_500,
            latency_health_bps: 9_100,
            dispute_rate_bps: 0,
            token_violation_rate_bps: 0,
            repair_breach_rate_bps: 0,
        },
        reserve_stage: ReputationReserveStageV1::Active,
        previous_score_bps: None,
        active_dispute: false,
        slashing_event: false,
    };
    let inputs = vec![input];
    let snapshot = build_reputation_snapshot(
        [0x42; 16],
        1_800_000_000,
        ReputationWeightsV1::default(),
        &inputs,
        None,
    )
    .expect("reputation snapshot");
    let scoring_evidence = ReputationScoringEvidenceV1 {
        version: REPUTATION_SCORING_EVIDENCE_VERSION_V1,
        provider_inputs: inputs,
        trust_edges: Vec::new(),
    };
    let mut envelope = SignedReputationSnapshotV1 {
        version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
        policy_digest: [0xA5; 32],
        snapshot,
        scoring_evidence_digest: scoring_evidence
            .canonical_digest()
            .expect("scoring evidence digest"),
        scoring_evidence,
        signatures: Vec::new(),
    };
    let signing_key = SigningKey::from_bytes(&[0x5A; 32]);
    envelope.signatures.push(ReputationSnapshotSignatureV1 {
        signer_id: "council-1".to_owned(),
        signature: signing_key
            .sign(&envelope.signing_digest().expect("signing digest"))
            .to_bytes(),
    });
    let payload = GovernanceLogPayloadV1::SignedReputationSnapshot(envelope);
    payload
        .validate(1_800_000_100)
        .expect("valid reputation snapshot");
}
#[test]
fn governance_payload_accepts_moderation_ballot_event() {
    let event = SoraFsModerationBallotGovernanceEventV1 {
        version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        sequence: 6,
        kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
        generated_at_unix_ms: 1_800_000_030_000,
        case_id: "case-42".to_string(),
        round_id: "round-1".to_string(),
        juror_id: None,
        committed_count: 2,
        revealed_count: 2,
        challenge_count: 0,
        tally: Some(SoraFsModerationBallotGovernanceTallyV1 {
            case_id: "case-42".to_string(),
            round_id: "round-1".to_string(),
            counts: SoraFsModerationVoteCountsV1 {
                uphold: 2,
                overturn: 0,
                modify: 0,
                escalate: 0,
            },
            votes_total: 2,
            quorum: 2,
            winning_choice: Some(SoraFsModerationVoteChoiceV1::Uphold),
            contested: false,
            tallied_at_unix_ms: 1_800_000_030_000,
        }),
        challenge: None,
    };
    let payload = GovernanceLogPayloadV1::ModerationBallotEvent(event);
    payload
        .validate(1_800_000_030)
        .expect("valid moderation ballot event");
}
#[test]
fn moderation_ballot_event_enforces_exact_size_and_text_boundaries() {
    let mut event = SoraFsModerationBallotGovernanceEventV1 {
        version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        sequence: 1,
        kind: SoraFsModerationBallotGovernanceEventKindV1::BallotAnnounced,
        generated_at_unix_ms: 1_800_000_030_000,
        case_id: "c".repeat(SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1),
        round_id: "r".repeat(SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1),
        juror_id: None,
        committed_count: 0,
        revealed_count: 0,
        challenge_count: 0,
        tally: None,
        challenge: None,
    };
    event.validate().expect("bounded event validates");
    let exact = event
        .encoded_len_exact()
        .expect("moderation event exact length");
    assert_eq!(
        preflight_moderation_event_len(&event, exact).expect("exact event boundary"),
        exact
    );
    assert!(matches!(
        preflight_moderation_event_len(&event, exact - 1),
        Err(
            SoraFsModerationBallotGovernanceEventValidationError::PayloadTooLarge {
                found,
                maximum,
            }
        ) if found == exact && maximum == exact - 1
    ));
    event.case_id.push('c');
    assert!(matches!(
        event.validate(),
        Err(
            SoraFsModerationBallotGovernanceEventValidationError::InvalidBoundedText {
                field: "case_id",
                found,
                maximum: SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1,
            }
        ) if found == SORAFS_MODERATION_IDENTIFIER_MAX_BYTES_V1 + 1
    ));
}
pub(super) fn sample_appeal_finance_report() -> SoraFsAppealFinanceReportV1 {
    SoraFsAppealFinanceReportV1 {
        version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        report_id: [0x42; 16],
        case_id: "case-42".to_string(),
        round_id: Some("round-1".to_string()),
        generated_at_unix_ms: 1_800_000_031_000,
        appeal_finance_config_version: "baseline-v1".to_string(),
        evidence_bundle_digest: Some([0xA7; 32]),
        outcome: SoraFsAppealFinanceOutcomeV1::Overturn,
        deposit_xor: "420".parse().expect("canonical XOR quantity"),
        refund: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "refund-account".to_string(),
            amount_xor: "420".parse().expect("canonical XOR quantity"),
        },
        treasury: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "treasury-account".to_string(),
            amount_xor: "50".parse().expect("canonical XOR quantity"),
        },
        held: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "escrow-account".to_string(),
            amount_xor: "0".parse().expect("canonical XOR quantity"),
        },
        panel_size: 3,
        panel_reward_total_xor: "85".parse().expect("canonical XOR quantity"),
        rewards_paid_total_xor: "60".parse().expect("canonical XOR quantity"),
        rewards_forfeited_treasury_xor: "25".parse().expect("canonical XOR quantity"),
        juror_payouts: vec![
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-a".to_string(),
                stipend_xor: "25".parse().expect("canonical XOR quantity"),
                bonus_xor: "5".parse().expect("canonical XOR quantity"),
                total_xor: "30".parse().expect("canonical XOR quantity"),
            },
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-b".to_string(),
                stipend_xor: "25".parse().expect("canonical XOR quantity"),
                bonus_xor: "5".parse().expect("canonical XOR quantity"),
                total_xor: "30".parse().expect("canonical XOR quantity"),
            },
        ],
        no_show_juror_ids: vec!["juror-c".to_string()],
    }
}
#[test]
fn governance_payload_accepts_appeal_finance_report() {
    let payload = GovernanceLogPayloadV1::AppealFinanceReport(sample_appeal_finance_report());
    payload
        .validate(1_800_000_031)
        .expect("valid appeal finance report");
}
#[test]
fn governance_submission_provenance_is_required_origin_checked_and_cid_bound() {
    let payload = GovernanceLogPayloadV1::AppealFinanceReport(sample_appeal_finance_report());
    assert!(matches!(
        payload.validate_submission_provenance(None),
        Err(GovernanceLogValidationError::MissingSubmissionProvenance {
            expected: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
        })
    ));
    let provenance = GovernanceDagSubmissionProvenanceV1 {
        publisher_account_digest: governance_dag_submission_account_digest_v1(
            b"canonical-norito-publisher-account",
        ),
        origin: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
    };
    payload
        .validate_submission_provenance(Some(&provenance))
        .expect("matching authenticated provenance");
    let mut wrong_origin = provenance.clone();
    wrong_origin.origin = GovernanceDagSubmissionOriginV1::AppealFinanceWeeklyRollup;
    assert!(matches!(
        payload.validate_submission_provenance(Some(&wrong_origin)),
        Err(GovernanceLogValidationError::SubmissionOriginMismatch {
            expected: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
            found: GovernanceDagSubmissionOriginV1::AppealFinanceWeeklyRollup,
        })
    ));
    let cid = governance_log_node_cid_v1(
        None,
        1_800_000_031,
        b"12D3KooWGovernancePeer",
        Some(&provenance),
        &payload,
    )
    .expect("derive provenance-bound node CID");
    let mut other_publisher = provenance.clone();
    other_publisher.publisher_account_digest =
        governance_dag_submission_account_digest_v1(b"canonical-norito-other-publisher-account");
    let other_cid = governance_log_node_cid_v1(
        None,
        1_800_000_031,
        b"12D3KooWGovernancePeer",
        Some(&other_publisher),
        &payload,
    )
    .expect("derive changed provenance-bound node CID");
    assert_ne!(cid, other_cid);
    let internal_payload = signed_por_proof_payload();
    internal_payload
        .validate_submission_provenance(None)
        .expect("internal payload needs only its node signer attestation");
    assert!(matches!(
        internal_payload.validate_submission_provenance(Some(&provenance)),
        Err(
            GovernanceLogValidationError::UnexpectedSubmissionProvenance {
                found: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
            }
        )
    ));
    let external_payload = GovernanceLogPayloadV1::ExternalPayload(sample_external_payload());
    external_payload
        .validate_submission_provenance(None)
        .expect("trusted in-process transparency producer may omit caller provenance");
    let external_provenance = GovernanceDagSubmissionProvenanceV1 {
        publisher_account_digest: provenance.publisher_account_digest,
        origin: GovernanceDagSubmissionOriginV1::PrivacyAggregatePublishDue,
    };
    external_payload
        .validate_submission_provenance(Some(&external_provenance))
        .expect("authenticated transparency ingress retains matching provenance");
    assert!(matches!(
        external_payload.validate_submission_provenance(Some(&provenance)),
        Err(GovernanceLogValidationError::SubmissionOriginMismatch {
            expected: GovernanceDagSubmissionOriginV1::PrivacyAggregatePublishDue,
            found: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
        })
    ));
}
#[test]
fn governance_submission_account_digest_is_fixed_and_identity_bound() {
    let publisher =
        governance_dag_submission_account_digest_v1(b"canonical-norito-publisher-account");
    let same_publisher =
        governance_dag_submission_account_digest_v1(b"canonical-norito-publisher-account");
    let other_publisher =
        governance_dag_submission_account_digest_v1(b"canonical-norito-other-publisher-account");
    assert_eq!(
        publisher.len(),
        GOVERNANCE_DAG_SUBMISSION_ACCOUNT_DIGEST_BYTES_V1
    );
    assert_eq!(publisher, same_publisher);
    assert_ne!(publisher, other_publisher);
}
#[test]
fn appeal_finance_report_enforces_exact_size_text_and_panel_boundaries() {
    let mut report = sample_appeal_finance_report();
    report.case_id = "c".repeat(SORAFS_APPEAL_FINANCE_IDENTIFIER_MAX_BYTES_V1);
    report.round_id = Some("r".repeat(SORAFS_APPEAL_FINANCE_IDENTIFIER_MAX_BYTES_V1));
    report.appeal_finance_config_version =
        "v".repeat(SORAFS_APPEAL_FINANCE_CONFIG_VERSION_MAX_BYTES_V1);
    report.refund.account_id = "a".repeat(SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1);
    report.treasury.account_id = "b".repeat(SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1);
    report.held.account_id = "c".repeat(SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1);
    report.juror_payouts[0].juror_id = "d".repeat(SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1);
    report.juror_payouts[1].juror_id = "e".repeat(SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1);
    report.no_show_juror_ids[0] = "f".repeat(SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1);
    report.validate().expect("bounded report validates");
    let exact = report
        .encoded_len_exact()
        .expect("appeal finance report exact length");
    assert_eq!(
        preflight_appeal_finance_report_len(&report, exact).expect("exact report boundary"),
        exact
    );
    assert!(matches!(
        preflight_appeal_finance_report_len(&report, exact - 1),
        Err(SoraFsAppealFinanceReportValidationError::PayloadTooLarge {
            found,
            maximum,
        }) if found == exact && maximum == exact - 1
    ));
    let mut long_config = report.clone();
    long_config.appeal_finance_config_version.push('v');
    assert!(matches!(
        long_config.validate(),
        Err(SoraFsAppealFinanceReportValidationError::InvalidBoundedText {
            field: "appeal_finance_config_version",
            found,
            maximum: SORAFS_APPEAL_FINANCE_CONFIG_VERSION_MAX_BYTES_V1,
        }) if found == SORAFS_APPEAL_FINANCE_CONFIG_VERSION_MAX_BYTES_V1 + 1
    ));
    let mut long_account = report;
    long_account.juror_payouts[0].juror_id.push('d');
    assert!(matches!(
        long_account.validate(),
        Err(SoraFsAppealFinanceReportValidationError::InvalidBoundedText {
            field: "juror_payouts.juror_id",
            found,
            maximum: SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1,
        }) if found == SORAFS_APPEAL_FINANCE_ACCOUNT_MAX_BYTES_V1 + 1
    ));
    assert_eq!(
        validate_appeal_finance_panel_bounds(
            SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1 as u32,
            SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1,
            0,
        )
        .expect("exact panel boundary"),
        SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1
    );
    assert!(matches!(
        validate_appeal_finance_panel_bounds(
            SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1 as u32,
            SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1,
            1,
        ),
        Err(SoraFsAppealFinanceReportValidationError::TooManyPanelRows {
            found,
            maximum: SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1,
        }) if found == SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1 + 1
    ));
    assert!(matches!(
        validate_appeal_finance_panel_bounds(
            SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1 as u32 + 1,
            SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1,
            0,
        ),
        Err(
            SoraFsAppealFinanceReportValidationError::PanelSizeExceedsMaximum {
                maximum: SORAFS_APPEAL_FINANCE_PANEL_ROWS_MAX_V1,
                ..
            }
        )
    ));
}
fn sample_external_payload() -> GovernanceExternalPayloadV1 {
    let cycle_id = *b"cycle-2026-wk-01";
    let entry = crate::transparency::ModerationLedgerEntryV1 {
        version: crate::transparency::MODERATION_LEDGER_ENTRY_VERSION_V1,
        cycle_id,
        entry_id: [0x11; 16],
        sequence: 1,
        occurred_at_unix: 1_800_000_001,
        kind: crate::transparency::ModerationLedgerEntryKindV1::GarEnforcementReceipt,
        subject: "gar-receipt-1".to_owned(),
        subject_digest: [0x21; 32],
        payload_digest: [0x22; 32],
        summary_digest: [0x23; 32],
        policy_digest: Some([0x24; 32]),
        evidence_uris: vec!["sora://transparency/gar-receipt-1".to_owned()],
        metadata: Vec::new(),
    };
    let publication = ModerationLedgerCyclePublicationV1::from_entries(
        cycle_id,
        1_800_000_000,
        1_800_000_010,
        1_800_000_011,
        None,
        &[entry],
    )
    .expect("build transparency publication");
    let encoded_payload = norito::to_bytes(&publication).expect("encode publication");
    GovernanceExternalPayloadV1::from_transparency_ledger_publication(
        &publication,
        &encoded_payload,
    )
    .expect("wrap transparency publication")
}
#[test]
fn governance_payload_accepts_external_payload() {
    let payload = GovernanceLogPayloadV1::ExternalPayload(sample_external_payload());
    payload
        .validate(1_800_000_031)
        .expect("valid external governance payload");
}
#[test]
fn external_payload_rejects_digest_mismatch() {
    let mut payload = sample_external_payload();
    payload.encoded_blake3[0] ^= 0xFF;
    let err = payload.validate().expect_err("digest mismatch rejected");
    assert_eq!(
        err,
        GovernanceExternalPayloadValidationError::EncodedDigestMismatch
    );
    let err = GovernanceLogPayloadV1::ExternalPayload(payload)
        .validate(1_800_000_031)
        .expect_err("governance payload rejects digest mismatch");
    assert!(matches!(
        err,
        GovernanceLogValidationError::ExternalPayload(
            GovernanceExternalPayloadValidationError::EncodedDigestMismatch
        )
    ));
}
#[test]
fn external_payload_rejects_unsorted_metadata() {
    let mut payload = sample_external_payload();
    payload.metadata.swap(0, 1);
    let err = payload
        .validate()
        .expect_err("metadata ordering is canonical");
    assert_eq!(
        err,
        GovernanceExternalPayloadValidationError::MetadataKeysUnsorted
    );
}
#[test]
fn external_payload_rejects_unknown_kind_and_version() {
    let mut payload = sample_external_payload();
    payload.payload_kind = "arbitrary_payload".to_owned();
    assert!(matches!(
        payload.validate(),
        Err(GovernanceExternalPayloadValidationError::UnsupportedPayloadKind {
            payload_kind
        }) if payload_kind == "arbitrary_payload"
    ));
    let mut payload = sample_external_payload();
    payload.payload_version = MODERATION_LEDGER_PUBLICATION_VERSION_V1 + 1;
    assert!(matches!(
        payload.validate(),
        Err(GovernanceExternalPayloadValidationError::UnsupportedPayloadVersion {
            payload_kind,
            expected: MODERATION_LEDGER_PUBLICATION_VERSION_V1,
            found
        }) if payload_kind == GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1
            && found == MODERATION_LEDGER_PUBLICATION_VERSION_V1 + 1
    ));
}
#[test]
fn external_payload_rejects_oversized_and_trailing_payload_bytes() {
    let mut oversized = sample_external_payload();
    oversized.encoded_payload = vec![0xA5; SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_MAX_BYTES_V1 + 1];
    oversized.encoded_len = oversized.encoded_payload.len() as u64;
    oversized.encoded_blake3 = *blake3::hash(&oversized.encoded_payload).as_bytes();
    assert!(matches!(
        oversized.validate(),
        Err(GovernanceExternalPayloadValidationError::EncodedPayloadTooLarge { .. })
    ));
    let mut trailing = sample_external_payload();
    trailing.encoded_payload.push(0);
    trailing.encoded_len = trailing.encoded_payload.len() as u64;
    trailing.encoded_blake3 = *blake3::hash(&trailing.encoded_payload).as_bytes();
    assert!(matches!(
        trailing.validate(),
        Err(GovernanceExternalPayloadValidationError::TypedPayloadDecode { .. })
    ));
}
#[test]
fn external_payload_canonical_validation_ignores_enclosing_layout() {
    let payload = sample_external_payload();
    for flags in crate::canonical_test_support::supported_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        payload.validate().expect("canonical external publication");
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
}
#[test]
fn external_payload_rejects_noncanonical_compression_tag() {
    let mut payload = sample_external_payload();
    let publication: ModerationLedgerCyclePublicationV1 =
        norito::decode_from_bytes(&payload.encoded_payload).expect("decode publication");
    let compressed = crate::canonical_test_support::with_compression_tag(&publication);
    assert_ne!(compressed, payload.encoded_payload);
    payload.encoded_len = compressed.len() as u64;
    payload.encoded_blake3 = *blake3::hash(&compressed).as_bytes();
    payload.encoded_payload = compressed;
    assert!(matches!(
        payload.validate(),
        Err(GovernanceExternalPayloadValidationError::NonCanonicalEncodedPayload { .. })
    ));
}
#[test]
fn external_payload_rejects_invalid_typed_payload() {
    let mut payload = sample_external_payload();
    let mut publication: ModerationLedgerCyclePublicationV1 =
        norito::decode_from_bytes(&payload.encoded_payload).expect("decode publication");
    publication.version = MODERATION_LEDGER_PUBLICATION_VERSION_V1 + 1;
    payload.encoded_payload = norito::to_bytes(&publication).expect("encode invalid publication");
    payload.encoded_len = payload.encoded_payload.len() as u64;
    payload.encoded_blake3 = *blake3::hash(&payload.encoded_payload).as_bytes();
    assert!(matches!(
        payload.validate(),
        Err(GovernanceExternalPayloadValidationError::InvalidTypedPayload { .. })
    ));
}
#[test]
fn external_payload_rejects_metadata_count_key_value_duplicate_and_mismatch() {
    let mut too_many = sample_external_payload();
    too_many.metadata = (0..=SORAFS_GOVERNANCE_EXTERNAL_METADATA_MAX_ENTRIES_V1)
        .map(|index| GovernanceExternalPayloadMetadataV1 {
            key: format!("k{index:02}"),
            value: "v".to_owned(),
        })
        .collect();
    assert!(matches!(
        too_many.validate(),
        Err(GovernanceExternalPayloadValidationError::MetadataCountTooLarge { .. })
    ));
    let mut long_key = sample_external_payload();
    long_key.metadata.last_mut().expect("metadata").key =
        "z".repeat(SORAFS_GOVERNANCE_EXTERNAL_METADATA_KEY_MAX_BYTES_V1 + 1);
    assert!(matches!(
        long_key.validate(),
        Err(GovernanceExternalPayloadValidationError::MetadataKeyTooLong { .. })
    ));
    let mut long_value = sample_external_payload();
    long_value.metadata[0].value =
        "v".repeat(SORAFS_GOVERNANCE_EXTERNAL_METADATA_VALUE_MAX_BYTES_V1 + 1);
    assert!(matches!(
        long_value.validate(),
        Err(GovernanceExternalPayloadValidationError::MetadataValueTooLong { .. })
    ));
    let mut total_too_large = sample_external_payload();
    total_too_large.metadata = (0..SORAFS_GOVERNANCE_EXTERNAL_METADATA_MAX_ENTRIES_V1)
        .map(|index| GovernanceExternalPayloadMetadataV1 {
            key: format!("key-{index:02}"),
            value: "v".repeat(SORAFS_GOVERNANCE_EXTERNAL_METADATA_VALUE_MAX_BYTES_V1),
        })
        .collect();
    assert!(matches!(
        total_too_large.validate(),
        Err(GovernanceExternalPayloadValidationError::MetadataBytesTooLarge { .. })
    ));
    let mut duplicate = sample_external_payload();
    duplicate.metadata.insert(1, duplicate.metadata[0].clone());
    assert!(matches!(
        duplicate.validate(),
        Err(GovernanceExternalPayloadValidationError::DuplicateMetadataKey { .. })
    ));
    let mut mismatch = sample_external_payload();
    mismatch.metadata[0].value = "00".repeat(32);
    assert!(matches!(
        mismatch.validate(),
        Err(GovernanceExternalPayloadValidationError::MetadataMismatch { .. })
    ));
}
#[test]
fn external_repair_slash_rejects_embedded_approval() {
    let proposal = RepairSlashProposalV1 {
        version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
        ticket_id: crate::repair::RepairTicketId("REP-351".to_owned()),
        provider_id: [0x31; 32],
        manifest_digest: [0x32; 32],
        auditor_account: "auditor@sorafs".to_owned(),
        proposed_penalty: XorQuantity::try_from_micro(1)
            .expect("legacy nano-XOR penalty is representable"),
        submitted_at_unix: 1_800_000_001,
        rationale: "repeated proof failures".to_owned(),
        approval: Some(crate::repair::RepairEscalationApprovalV1 {
            version: crate::repair::REPAIR_ESCALATION_APPROVAL_VERSION_V1,
            approve_votes: 2,
            reject_votes: 1,
            abstain_votes: 0,
            approved_at_unix: 1_800_000_002,
            finalized_at_unix: 1_800_000_003,
        }),
    };
    let encoded = norito::to_bytes(&proposal).expect("encode slash proposal");
    assert_eq!(
        GovernanceExternalPayloadV1::from_repair_slash(
            &proposal,
            GovernanceExternalRepairSlashStageV1::Submitted,
            &encoded,
        ),
        Err(GovernanceExternalPayloadValidationError::RepairSlashApprovalForbidden)
    );
}
#[test]
fn appeal_finance_report_rejects_panel_reconciliation_mismatch() {
    let mut report = sample_appeal_finance_report();
    report.no_show_juror_ids.clear();
    let err = report.validate().expect_err("panel mismatch rejected");
    assert_eq!(
        err,
        SoraFsAppealFinanceReportValidationError::PanelReconciliation {
            panel_size: 3,
            accounted: 2,
        }
    );
}
// Textual inclusion preserves the original governance test-module paths.
include!("tests/appeal_finance_settlement.rs");
fn second_appeal_finance_report() -> SoraFsAppealFinanceReportV1 {
    SoraFsAppealFinanceReportV1 {
        version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        report_id: [0x43; 16],
        case_id: "case-43".to_string(),
        round_id: Some("round-1".to_string()),
        generated_at_unix_ms: 1_800_000_032_000,
        appeal_finance_config_version: "baseline-v1".to_string(),
        evidence_bundle_digest: Some([0xB8; 32]),
        outcome: SoraFsAppealFinanceOutcomeV1::Uphold,
        deposit_xor: "80.25".parse().expect("canonical XOR quantity"),
        refund: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "refund-account".to_string(),
            amount_xor: "0.25".parse().expect("canonical XOR quantity"),
        },
        treasury: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "treasury-account".to_string(),
            amount_xor: "80".parse().expect("canonical XOR quantity"),
        },
        held: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "escrow-account".to_string(),
            amount_xor: "0.00".parse().expect("canonical XOR quantity"),
        },
        panel_size: 1,
        panel_reward_total_xor: "30".parse().expect("canonical XOR quantity"),
        rewards_paid_total_xor: "30".parse().expect("canonical XOR quantity"),
        rewards_forfeited_treasury_xor: "0".parse().expect("canonical XOR quantity"),
        juror_payouts: vec![SoraFsAppealFinanceJurorPayoutV1 {
            juror_id: "juror-d".to_string(),
            stipend_xor: "25".parse().expect("canonical XOR quantity"),
            bonus_xor: "5".parse().expect("canonical XOR quantity"),
            total_xor: "30".parse().expect("canonical XOR quantity"),
        }],
        no_show_juror_ids: Vec::new(),
    }
}
fn max_source_appeal_finance_weekly_rollup() -> SoraFsAppealFinanceWeeklyRollupV1 {
    let zero = XorQuantity::try_from_micro(0).expect("zero XOR quantity");
    let report_count = u64::try_from(SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1)
        .expect("source-report ceiling fits u64");
    SoraFsAppealFinanceWeeklyRollupV1 {
        version: SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_VERSION_V1,
        cycle: PorReportIsoWeek {
            year: 2026,
            week: 26,
        },
        generated_at_unix_ms: 1_800_000_100_000,
        report_count,
        case_count: 1,
        appeal_finance_config_versions: vec!["baseline-v1".to_owned()],
        total_deposit_xor: zero.clone(),
        total_refund_xor: zero.clone(),
        total_treasury_xor: zero.clone(),
        total_held_xor: zero.clone(),
        total_panel_reward_xor: zero.clone(),
        total_rewards_paid_xor: zero.clone(),
        total_rewards_forfeited_treasury_xor: zero.clone(),
        juror_payout_count: 0,
        no_show_juror_count: 0,
        outcomes: vec![SoraFsAppealFinanceOutcomeRollupV1 {
            outcome: SoraFsAppealFinanceOutcomeV1::Uphold,
            report_count,
            case_count: 1,
            total_deposit_xor: zero.clone(),
            total_refund_xor: zero.clone(),
            total_treasury_xor: zero.clone(),
            total_held_xor: zero.clone(),
            total_panel_reward_xor: zero.clone(),
            total_rewards_paid_xor: zero.clone(),
            total_rewards_forfeited_treasury_xor: zero,
            juror_payout_count: 0,
            no_show_juror_count: 0,
        }],
        source_report_ids: (1_u128
            ..=u128::try_from(SORAFS_APPEAL_FINANCE_WEEKLY_SOURCE_REPORTS_MAX_V1)
                .expect("source-report ceiling fits u128"))
            .map(u128::to_be_bytes)
            .collect(),
    }
}
#[test]
fn governance_payload_accepts_appeal_finance_weekly_rollup() {
    let first = sample_appeal_finance_report();
    let second = second_appeal_finance_report();
    let cycle = PorReportIsoWeek {
        year: 2026,
        week: 26,
    };
    let rollup = SoraFsAppealFinanceWeeklyRollupV1::from_reports(
        cycle,
        1_800_000_100_000,
        &[second.clone(), first.clone()],
    )
    .expect("weekly rollup");
    assert_eq!(rollup.report_count, 2);
    assert_eq!(rollup.case_count, 2);
    assert_eq!(
        rollup.appeal_finance_config_versions,
        vec!["baseline-v1".to_string()]
    );
    assert_eq!(rollup.total_deposit_xor.to_string(), "500.25");
    assert_eq!(rollup.total_refund_xor.to_string(), "420.25");
    assert_eq!(rollup.total_treasury_xor.to_string(), "130");
    assert_eq!(rollup.total_held_xor.to_string(), "0");
    assert_eq!(rollup.total_panel_reward_xor.to_string(), "115");
    assert_eq!(rollup.total_rewards_paid_xor.to_string(), "90");
    assert_eq!(
        rollup.total_rewards_forfeited_treasury_xor.to_string(),
        "25"
    );
    assert_eq!(rollup.juror_payout_count, 3);
    assert_eq!(rollup.no_show_juror_count, 1);
    assert_eq!(
        rollup.source_report_ids,
        vec![first.report_id, second.report_id]
    );
    assert_eq!(rollup.outcomes.len(), 2);
    assert_eq!(
        rollup.outcomes[0].outcome,
        SoraFsAppealFinanceOutcomeV1::Uphold
    );
    assert_eq!(
        rollup.outcomes[1].outcome,
        SoraFsAppealFinanceOutcomeV1::Overturn
    );
    GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(rollup)
        .validate(1_800_000_100)
        .expect("weekly rollup payload validates");
}
include!("tests/weekly_rollup_and_moderation.rs");

impl From<&GovernanceDagBlockV1> for GovernanceDagBlockSignaturePayloadV1 {
    fn from(block: &GovernanceDagBlockV1) -> Self {
        Self {
            version: block.version,
            block_cid: block.block_cid.clone(),
            prev_block_cid: block.prev_block_cid.clone(),
            sequence: block.sequence,
            timestamp: block.timestamp,
            publisher_peer_id: block.publisher_peer_id.clone(),
            node: block.node.clone(),
        }
    }
}
impl From<&GovernanceDagHeadV1> for GovernanceDagHeadSignaturePayloadV1 {
    fn from(head: &GovernanceDagHeadV1) -> Self {
        Self {
            version: head.version,
            head_block_cid: head.head_block_cid.clone(),
            block_count: head.block_count,
            generated_at: head.generated_at,
            publisher_peer_id: head.publisher_peer_id.clone(),
            checkpoint_cid: head.checkpoint_cid.clone(),
        }
    }
}
impl From<&GovernanceLogNodeV1> for GovernanceLogSignaturePayloadV1 {
    fn from(node: &GovernanceLogNodeV1) -> Self {
        Self {
            version: node.version,
            node_cid: node.node_cid.clone(),
            prev_cid: node.prev_cid.clone(),
            timestamp: node.timestamp,
            publisher_peer_id: node.publisher_peer_id.clone(),
            submission_provenance: node.submission_provenance.clone(),
            payload: node.payload.clone(),
        }
    }
}
