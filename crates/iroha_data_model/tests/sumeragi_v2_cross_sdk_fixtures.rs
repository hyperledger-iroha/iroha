//! Rust-authority checks for shared Sumeragi v2 SDK wire fixtures.

use std::collections::BTreeMap;

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    ChainId,
    block::consensus_v2::{
        BlockSubject, CertifiedBodyRequest, CertifiedBodyResponse, CommitCertificateRequest,
        CommitCertificateResponse, ConsensusMessageV2, ConsensusMessageV2Payload, ConsensusMode,
        ConsensusRound, DataAvailabilityLayout, DualQuorum, GlobalPhase, HeightContext,
        PROTOCOL_VERSION, PayloadChunk, PayloadEncoding, PayloadManifest, Proposal,
        ProposalJustification, QuorumCertificate, SumeragiV2BodyState, SumeragiV2Status,
        SumeragiV2StatusPhase, TimeoutCertificate, TimeoutJustification, TimeoutVote,
        TimeoutVoteGroup, ValidatorPower, Vote,
    },
    peer::PeerId,
};
use norito::codec::{DecodeAll, Encode};

const FIXTURES: &str = include_str!("../../../fixtures/sumeragi_v2/wire_v2.tsv");

fn peer(seed: u8) -> PeerId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("derive deterministic fixture key");
    PeerId::new(key_pair.public_key().clone())
}

fn context() -> HeightContext {
    let mut peers = (1..=4).map(peer).collect::<Vec<_>>();
    peers.sort();
    let roster = peers
        .into_iter()
        .map(|validator| ValidatorPower {
            validator,
            power: 1,
        })
        .collect::<Vec<_>>();
    HeightContext {
        chain_id: ChainId::from("sumeragi-v2-test"),
        protocol_version: PROTOCOL_VERSION,
        height: 1,
        epoch: 2,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Npos,
        parent_commit_qc: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"nexus amx context"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::Plain,
            chunk_size_bytes: 4,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1024,
            max_chunk_count: 256,
        },
        leader_seed: [0xa5; 32],
    }
}

fn round(context: &HeightContext, view: u64) -> ConsensusRound {
    ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    }
}

fn subject(seed: u8) -> BlockSubject {
    BlockSubject {
        parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new([seed, 0]))),
        block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 1])),
        payload_hash: Hash::new([seed, 2]),
    }
}

fn qc(context: &HeightContext, view: u64, phase: GlobalPhase) -> QuorumCertificate {
    QuorumCertificate {
        round: round(context, view),
        phase,
        subject: subject(u8::try_from(view + 1).expect("small fixture view")),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x5a; 48],
    }
}

fn fixture_rows() -> BTreeMap<(String, String), String> {
    FIXTURES
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut columns = line.split('\t');
            let kind = columns.next().expect("fixture kind");
            let name = columns.next().expect("fixture name");
            let hex = columns.next().expect("fixture hex");
            let expectation = columns.next().expect("fixture expectation");
            assert!(columns.next().is_none(), "fixture row has extra columns");
            assert!(matches!(expectation, "accept" | "reject"));
            ((kind.to_owned(), name.to_owned()), hex.to_owned())
        })
        .collect()
}

#[test]
fn shared_sdk_accept_fixtures_are_exact_current_rust_encodings() {
    let context = context();
    let prepare = qc(&context, 1, GlobalPhase::Prepare);
    let timeout = TimeoutCertificate {
        round: round(&context, 2),
        groups: vec![TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare.clone()),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x33; 48],
        }],
    };
    let manifest = PayloadManifest::derive(
        &context,
        round(&context, 1),
        subject(9),
        4,
        &[b"body".to_vec()],
    )
    .expect("derive canonical fixture manifest");
    let body_request = CertifiedBodyRequest {
        round: manifest.round,
        subject: manifest.subject,
        certificate: prepare.clone(),
        requester: context.roster[3].validator.clone(),
        signature: vec![0x44; 48],
    };
    let proposal = Proposal {
        round: manifest.round,
        proposer: 2,
        subject: manifest.subject,
        manifest: manifest.clone(),
        justification: ProposalJustification::Timeout(TimeoutJustification {
            timeout_certificate: timeout.clone(),
            highest_prepare_qc: Some(prepare.clone()),
        }),
        signature: vec![0x55; 48],
    };

    let mut messages = BTreeMap::new();
    let mut insert_message = |name: &str, payload| {
        messages.insert(name.to_owned(), ConsensusMessageV2::new(payload).encode());
    };
    insert_message("proposal", ConsensusMessageV2Payload::Proposal(proposal));
    insert_message(
        "vote",
        ConsensusMessageV2Payload::Vote(Vote {
            round: manifest.round,
            phase: GlobalPhase::Prepare,
            subject: manifest.subject,
            signer: 0,
            signature: vec![1],
        }),
    );
    insert_message(
        "quorum_certificate",
        ConsensusMessageV2Payload::QuorumCertificate(prepare.clone()),
    );
    insert_message(
        "timeout_vote",
        ConsensusMessageV2Payload::TimeoutVote(TimeoutVote {
            round: timeout.round,
            highest_prepare_qc: Some(prepare.clone()),
            signer: 0,
            signature: vec![2],
        }),
    );
    insert_message(
        "timeout_certificate",
        ConsensusMessageV2Payload::TimeoutCertificate(timeout.clone()),
    );
    insert_message(
        "payload_manifest",
        ConsensusMessageV2Payload::PayloadManifest(manifest.clone()),
    );
    insert_message(
        "payload_chunk",
        ConsensusMessageV2Payload::PayloadChunk(PayloadChunk {
            manifest_hash: HashOf::new(&manifest),
            index: 0,
            bytes: b"body".to_vec(),
            sender: 0,
            signature: vec![0x66; 48],
        }),
    );
    insert_message(
        "certified_body_request",
        ConsensusMessageV2Payload::CertifiedBodyRequest(body_request.clone()),
    );
    insert_message(
        "certified_body_response",
        ConsensusMessageV2Payload::CertifiedBodyResponse(CertifiedBodyResponse {
            request_hash: HashOf::new(&body_request),
            manifest: manifest.clone(),
            body: b"body".to_vec(),
            responder: 0,
            signature: vec![3],
        }),
    );

    let commit_request = CommitCertificateRequest {
        protocol_version: PROTOCOL_VERSION,
        chain_id: context.chain_id.clone(),
        context_id: context.id(),
        height: context.height,
        requester: peer(99),
        signature: vec![0x81; 48],
    };
    assert_eq!(commit_request.validate(&context), Ok(()));
    let commit_response = CommitCertificateResponse {
        request_hash: HashOf::new(&commit_request),
        certificate: qc(&context, 9, GlobalPhase::Commit),
        responder: peer(100),
        signature: vec![0x82; 48],
    };
    assert_eq!(
        commit_response.validate_against(&context, &commit_request),
        Ok(())
    );
    insert_message(
        "commit_certificate_request",
        ConsensusMessageV2Payload::CommitCertificateRequest(commit_request.clone()),
    );
    insert_message(
        "commit_certificate_response",
        ConsensusMessageV2Payload::CommitCertificateResponse(commit_response.clone()),
    );

    let status = SumeragiV2Status {
        protocol_version: PROTOCOL_VERSION,
        node_fingerprint: Hash::new(b"node"),
        build_fingerprint: Hash::new(b"build"),
        config_fingerprint: Hash::new(b"config"),
        height_context_id: context.id(),
        height: context.height,
        view: 3,
        phase: SumeragiV2StatusPhase::Prepare,
        leader: 2,
        locked_prepare_qc: Some(prepare.as_ref()),
        highest_prepare_qc: Some(prepare.as_ref()),
        last_timeout_certificate: Some(timeout.as_ref()),
        body_state: SumeragiV2BodyState::Validated,
        pending_persistence_id: Some(17),
        last_committed_height: context.height - 1,
        last_committed_subject: Some(prepare.subject),
    };

    let rows = fixture_rows();
    let accepted_names = rows
        .keys()
        .filter_map(|(kind, name)| (kind == "message").then_some(name.clone()))
        .collect::<Vec<_>>();
    assert_eq!(accepted_names, messages.keys().cloned().collect::<Vec<_>>());
    for (name, encoded) in messages {
        assert_eq!(
            rows.get(&("message".to_owned(), name.clone())),
            Some(&hex::encode(encoded)),
            "stale Rust-authority fixture for {name}"
        );
    }
    assert_eq!(
        rows.get(&("status".to_owned(), "compact".to_owned())),
        Some(&hex::encode(status.encode()))
    );
    assert_eq!(
        rows.get(&(
            "preimage".to_owned(),
            "commit_certificate_request".to_owned()
        )),
        Some(&hex::encode(commit_request.signature_preimage()))
    );
    assert_eq!(
        rows.get(&(
            "preimage".to_owned(),
            "commit_certificate_response".to_owned()
        )),
        Some(&hex::encode(commit_response.signature_preimage()))
    );
}

#[test]
fn shared_sdk_negative_fixtures_fail_rust_structure_or_protocol_validation() {
    let context = context();
    let rows = fixture_rows();
    let decode = |kind: &str, name: &str| {
        let encoded = rows
            .get(&(kind.to_owned(), name.to_owned()))
            .unwrap_or_else(|| panic!("missing {kind}/{name} fixture"));
        let bytes = hex::decode(encoded).expect("fixture is valid hex");
        let mut cursor = bytes.as_slice();
        ConsensusMessageV2::decode_all(&mut cursor)
    };

    for name in [
        "truncated",
        "trailing_byte",
        "retired_zero_prepare_tag",
        "unknown_payload_tag",
        "commit_request_truncated_signature",
        "commit_response_truncated_signature",
        "commit_request_invalid_chain_utf8",
    ] {
        assert!(
            decode("negative_message", name).is_err(),
            "{name} unexpectedly decoded as canonical Rust wire"
        );
    }

    let wrong_version = decode("negative_message", "wrong_protocol_version")
        .expect("wrong outer version is structurally decodable");
    assert!(wrong_version.validate_version().is_err());

    let noncanonical = decode("negative_message", "noncanonical_signers")
        .expect("noncanonical signer order is structurally decodable");
    let ConsensusMessageV2Payload::QuorumCertificate(certificate) = noncanonical.payload else {
        panic!("noncanonical signer fixture used the wrong payload")
    };
    assert!(certificate.validate(&context).is_err());

    let overlapping = decode("negative_message", "overlapping_timeout_groups")
        .expect("overlapping groups are structurally decodable");
    let ConsensusMessageV2Payload::TimeoutCertificate(certificate) = overlapping.payload else {
        panic!("overlapping group fixture used the wrong payload")
    };
    assert!(certificate.validate(&context).is_err());

    for name in [
        "commit_request_wrong_nested_protocol",
        "commit_request_empty_signature",
    ] {
        let message =
            decode("negative_message", name).expect("invalid request is structurally decodable");
        let ConsensusMessageV2Payload::CommitCertificateRequest(request) = message.payload else {
            panic!("{name} used the wrong payload")
        };
        assert!(
            request.validate(&context).is_err(),
            "{name} passed validation"
        );
    }

    for name in [
        "commit_response_empty_signature",
        "commit_response_prepare_certificate",
    ] {
        let message =
            decode("negative_message", name).expect("invalid response is structurally decodable");
        let ConsensusMessageV2Payload::CommitCertificateResponse(response) = message.payload else {
            panic!("{name} used the wrong payload")
        };
        assert!(
            response.validate(&context).is_err(),
            "{name} passed validation"
        );
    }

    let canonical_request =
        decode("message", "commit_certificate_request").expect("canonical request decodes");
    let ConsensusMessageV2Payload::CommitCertificateRequest(canonical_request) =
        canonical_request.payload
    else {
        panic!("canonical request fixture used the wrong payload")
    };
    for name in [
        "commit_response_wrong_request_hash",
        "commit_response_wrong_context",
        "commit_response_wrong_height",
    ] {
        let message =
            decode("negative_binding", name).expect("binding corruption is structurally decodable");
        let ConsensusMessageV2Payload::CommitCertificateResponse(response) = message.payload else {
            panic!("{name} used the wrong payload")
        };
        assert!(
            response
                .validate_against(&context, &canonical_request)
                .is_err(),
            "{name} passed exact-request validation"
        );
    }

    for name in ["wrong_protocol_version", "truncated"] {
        let encoded = rows
            .get(&("negative_status".to_owned(), name.to_owned()))
            .expect("negative status fixture");
        let bytes = hex::decode(encoded).expect("status fixture is valid hex");
        let mut cursor = bytes.as_slice();
        let decoded = SumeragiV2Status::decode_all(&mut cursor);
        if name == "truncated" {
            assert!(decoded.is_err());
        } else {
            let status = decoded.expect("wrong status version is structurally decodable");
            assert_ne!(status.protocol_version, PROTOCOL_VERSION);
        }
    }
}
