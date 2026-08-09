use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    ChainId,
    block::{
        consensus::{
            CertPhase, LaneBlockCertificateV1, LaneBlockDescriptorV1, LaneBlockProposalV1,
            LaneBlockQcV1,
        },
        consensus_v2 as wire,
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    merge::{
        LaneDrainCertificateBodyV1, LaneDrainIntentV1, MergeCommitteeSignature, MergeLedgerEntry,
    },
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
};
use iroha_p2p::network::{NetworkReplyRoute, NetworkReplyRouteError, NetworkReplyRouteTestFixture};
use norito::codec::Encode as _;
use tempfile::TempDir;

use super::{
    BlockMessage, CryptoHash, FairV2IngressClass, InboundBlockMessage, LaneRelayMessage,
    fair_v2_ingress_is_certified_body_request, fair_v2_ingress_same_control_slot,
    test_sumeragi_handle, test_sumeragi_handle_with_source_geometry,
};

fn v2_message_with_bytes(index: u32, byte_len: usize) -> BlockMessage {
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
            manifest_hash: HashOf::from_untyped_unchecked(Hash::new(b"v2-ingress-test")),
            index,
            bytes: vec![0xA5; byte_len],
            sender: 0,
            signature: vec![0x5A],
        }),
    ))
}

fn v2_message_with_index(index: u32) -> BlockMessage {
    v2_message_with_bytes(index, 1)
}

fn v2_message() -> BlockMessage {
    v2_auxiliary_prepare(0)
}

fn lane_block_certificate(seed: u8) -> BlockMessage {
    let validator = PeerId::new(
        KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal)
            .expect("derive lane-certificate validator")
            .public_key()
            .clone(),
    );
    let validator_set = vec![validator];
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: LaneId::new(7),
        dataspace_id: DataSpaceId::new(9),
        lane_incarnation: Hash::new(b"fair-ingress-lane-incarnation"),
        proposal_height: 1,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: Hash::new(b"fair-ingress-lane-subject"),
        payload_ownership_hash: Hash::new(b"fair-ingress-lane-ownership"),
        rbc_instance_hash: Hash::new(b"fair-ingress-lane-rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::new(b"fair-ingress-lane-tx")],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        validator_count: 1,
        min_quorum: 1,
        qc_mode_tag: "permissioned:fair-ingress-lane".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let qc = |phase| LaneBlockQcV1 {
        body: proposal.vote_body(phase),
        validator_set_hash_version: proposal.descriptor.validator_set_hash_version,
        validator_set_hash: proposal.descriptor.validator_set_hash,
        validator_set: proposal.descriptor.validator_set.clone(),
        signers_bitmap: vec![1],
        bls_aggregate_signature: vec![seed; 96],
        payload_availability_qc: None,
    };
    let prepare_qc = qc(CertPhase::Prepare);
    let commit_qc = qc(CertPhase::Commit);
    BlockMessage::LaneBlockCertificate(Box::new(LaneBlockCertificateV1 {
        proposal,
        prepare_qc,
        commit_qc,
    }))
}

fn v2_commit_certificate_request(index: u64, requester: &PeerId) -> BlockMessage {
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateRequest(wire::CommitCertificateRequest {
            protocol_version: wire::PROTOCOL_VERSION,
            chain_id: ChainId::from("fair-v2-ingress-test"),
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-context",
            ))),
            height: index.saturating_add(1),
            requester: requester.clone(),
            signature: vec![u8::try_from(index).unwrap_or(u8::MAX)],
        }),
    ))
}

fn v2_certified_body_request(requester: &PeerId) -> BlockMessage {
    let BlockMessage::V2(message) = v2_vote(wire::GlobalPhase::Prepare) else {
        unreachable!("v2 vote fixture always returns a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Vote(vote) = message.payload else {
        unreachable!("v2 vote fixture always carries a vote");
    };
    let certificate = wire::QuorumCertificate {
        round: vote.round,
        proposal_round: vote.proposal_round,
        phase: wire::GlobalPhase::Prepare,
        subject: vote.subject,
        execution_commitment: vote.execution_commitment,
        signers: vec![0],
        aggregate_signature: vec![0x5A],
    };
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(wire::CertifiedBodyRequest {
            round: vote.round,
            subject: vote.subject,
            certificate,
            requester: requester.clone(),
            signature: vec![0x5A],
        }),
    ))
}

fn v2_certified_body_request_inbound(requester: &PeerId) -> InboundBlockMessage {
    let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
    let route = routes.mint(requester.clone());
    InboundBlockMessage::try_from_transport_with_reply_route(
        v2_certified_body_request(requester),
        requester.clone(),
        requester.clone(),
        route,
    )
    .expect("certified body request fixture retains its live authenticated reply route")
}

fn minimal_rs16_layout() -> wire::DataAvailabilityLayout {
    wire::DataAvailabilityLayout {
        encoding: wire::PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: 2,
        data_shards: 1,
        parity_shards: 1,
        max_payload_size_bytes: 1,
        max_chunk_count: 2,
    }
}

fn single_stripe_rs16_layout(body_len: usize) -> wire::DataAvailabilityLayout {
    let max_payload_size_bytes = u64::try_from(body_len.max(1)).expect("test body bound fits u64");
    let chunk_size_bytes = u32::try_from(body_len.max(1)).expect("test body bound fits u32");
    let chunk_size_bytes = chunk_size_bytes
        .checked_add(chunk_size_bytes % 2)
        .expect("test body bound has an even u32 successor");
    wire::DataAvailabilityLayout {
        encoding: wire::PayloadEncoding::ReedSolomon16,
        chunk_size_bytes,
        data_shards: 1,
        parity_shards: 1,
        max_payload_size_bytes,
        max_chunk_count: 2,
    }
}

fn v2_certified_body_response(
    request_ordinal: u64,
    responder: wire::ValidatorIndex,
    body_len: usize,
) -> BlockMessage {
    let body = vec![u8::try_from(request_ordinal).unwrap_or(0xA5); body_len];
    let payload_hash = Hash::new(&body);
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"fair-v2-ingress-response-context",
        ))),
        height: 1,
        view: request_ordinal,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(&request_ordinal.to_le_bytes())),
        payload_hash,
    };
    let manifest = wire::PayloadManifest {
        round,
        subject,
        payload_size_bytes: u64::try_from(body_len).expect("test body length fits u64"),
        layout: single_stripe_rs16_layout(body_len),
        chunk_hashes: vec![payload_hash; 2],
        chunk_root: Hash::new(payload_hash.as_ref()),
    };
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(wire::CertifiedBodyResponse {
            request_hash: HashOf::from_untyped_unchecked(Hash::new(&request_ordinal.to_le_bytes())),
            manifest,
            body,
            responder,
            signature: vec![0x5A],
        }),
    ))
}

fn v2_maximum_certified_body_response(layout: wire::DataAvailabilityLayout) -> BlockMessage {
    let body_len =
        usize::try_from(layout.max_payload_size_bytes).expect("test payload bound fits usize");
    let chunk_count = usize::try_from(layout.max_chunk_count).expect("test chunk count fits usize");
    let body = vec![0xA5; body_len];
    let chunk_hash = Hash::new(vec![
        0x5A;
        usize::try_from(layout.chunk_size_bytes)
            .expect("test chunk bound fits usize")
    ]);
    let chunk_hashes = vec![chunk_hash; chunk_count];
    let leaves = chunk_hashes
        .iter()
        .map(|hash| *hash.as_ref())
        .collect::<Vec<[u8; Hash::LENGTH]>>();
    let chunk_root =
        iroha_crypto::MerkleTree::<[u8; Hash::LENGTH]>::from_hashed_leaves_sha256(leaves)
            .root()
            .map(Hash::from)
            .expect("non-empty maximal manifest has a Merkle root");
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"fair-v2-ingress-max-response-context",
        ))),
        height: u64::MAX,
        view: u64::MAX,
    };
    let manifest = wire::PayloadManifest {
        round,
        subject: wire::BlockSubject {
            parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-response-parent",
            ))),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-response-block",
            )),
            payload_hash: Hash::new(&body),
        },
        payload_size_bytes: layout.max_payload_size_bytes,
        layout,
        chunk_hashes,
        chunk_root,
    };
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(wire::CertifiedBodyResponse {
            request_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-response-request",
            )),
            manifest,
            body,
            responder: u32::MAX,
            signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        }),
    ))
}

fn v2_maximum_payload_chunk(layout: wire::DataAvailabilityLayout) -> BlockMessage {
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
            manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"fair-v2-ingress-max-chunk-manifest",
            )),
            index: u32::MAX,
            bytes: vec![
                0xA5;
                usize::try_from(layout.chunk_size_bytes)
                    .expect("test chunk bound fits usize")
            ],
            sender: u32::MAX,
            signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        }),
    ))
}

fn v2_commit_certificate_response(request_ordinal: u64, responder: &PeerId) -> BlockMessage {
    let BlockMessage::V2(message) = v2_vote(wire::GlobalPhase::Commit) else {
        unreachable!("v2 vote fixture always returns a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Vote(vote) = message.payload else {
        unreachable!("v2 vote fixture always carries a vote");
    };
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    &request_ordinal.to_le_bytes(),
                )),
                certificate: wire::QuorumCertificate {
                    round: vote.round,
                    proposal_round: vote.proposal_round,
                    phase: wire::GlobalPhase::Commit,
                    subject: vote.subject,
                    execution_commitment: vote.execution_commitment,
                    signers: vec![0],
                    aggregate_signature: vec![0x5A],
                },
                responder: responder.clone(),
                signature: vec![0x5A],
            },
        ),
    ))
}

fn v2_vote(phase: wire::GlobalPhase) -> BlockMessage {
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"fair-v2-ingress-vote-context",
        ))),
        height: 1,
        view: 0,
    };
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round,
            proposal_round: round,
            phase,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"fair-v2-ingress-vote-block",
                )),
                payload_hash: Hash::new(b"fair-v2-ingress-vote-payload"),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"fair-v2-ingress-parent-state"),
                Hash::new(b"fair-v2-ingress-post-state"),
                Hash::new(b"fair-v2-ingress-writes"),
                1,
                Hash::new(b"fair-v2-ingress-executed-wire"),
            ),
            signer: 0,
            signature: vec![0x5A],
        }),
    ))
}

fn v2_auxiliary_prepare(index: u64) -> BlockMessage {
    let BlockMessage::V2(mut message) = v2_vote(wire::GlobalPhase::Prepare) else {
        unreachable!("v2 vote fixture always returns a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Vote(vote) = &mut message.payload else {
        unreachable!("v2 vote fixture always carries a vote");
    };
    vote.round.height = index.saturating_add(1);
    vote.proposal_round.height = vote.round.height;
    vote.signature = vec![u8::try_from(index).unwrap_or(u8::MAX)];
    BlockMessage::V2(message)
}

fn v2_timeout_vote() -> BlockMessage {
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
            round: wire::ConsensusRound {
                context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"fair-v2-ingress-timeout-context",
                ))),
                height: 1,
                view: 0,
            },
            highest_prepare_qc: None,
            signer: 0,
            signature: vec![0x5A],
        }),
    ))
}

fn v2_timeout_certificate(view: u64) -> BlockMessage {
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::TimeoutCertificate(wire::TimeoutCertificate {
            round: wire::ConsensusRound {
                context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"fair-v2-ingress-timeout-context",
                ))),
                height: 1,
                view,
            },
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0],
                aggregate_signature: vec![u8::try_from(view).unwrap_or(u8::MAX)],
            }],
        }),
    ))
}

fn v2_maximum_valid_timeout_vote_wire() -> BlockMessage {
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"fair-v2-ingress-max-timeout-context",
        ))),
        height: u64::MAX,
        view: u64::MAX,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"fair-v2-ingress-max-timeout-parent",
        ))),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"fair-v2-ingress-max-timeout-block")),
        payload_hash: Hash::new(b"fair-v2-ingress-max-timeout-payload"),
    };
    let ordinary_writes_root = Hash::new(b"fair-v2-ingress-max-writes");
    let topup_anchor_root = Hash::new(b"fair-v2-ingress-max-topup-root");
    let topup_anchor_count = wire::MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK;
    let post_state_root = wire::ExecutionCommitment::topup_post_state_root(
        topup_anchor_count,
        ordinary_writes_root,
        topup_anchor_root,
    );
    let highest_prepare_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: wire::ExecutionCommitment::new_without_merge_carrier(
            Hash::new(b"fair-v2-ingress-max-parent-state"),
            post_state_root,
            ordinary_writes_root,
            Some(topup_anchor_root),
            topup_anchor_count,
            1,
            Hash::new(b"fair-v2-ingress-max-executed-wire"),
        )
        .expect("maximum top-up projection is canonical"),
        signers: (0..wire::MAX_VALIDATORS_PER_HEIGHT)
            .map(|index| u32::try_from(index).expect("validator bound fits u32"))
            .collect(),
        aggregate_signature: vec![0xA5; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
    };
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(highest_prepare_qc),
            signer: u32::try_from(wire::MAX_VALIDATORS_PER_HEIGHT - 1)
                .expect("validator bound fits u32"),
            signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        }),
    ))
}

fn v2_maximum_structural_proposal_wire(
    layout: wire::DataAvailabilityLayout,
    roster_len: usize,
) -> BlockMessage {
    assert!(roster_len <= wire::MAX_VALIDATORS_PER_HEIGHT);
    let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"fair-v2-ingress-max-proposal-context",
    )));
    let subject = wire::BlockSubject {
        parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"fair-v2-ingress-max-proposal-parent",
        ))),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"fair-v2-ingress-max-proposal-block",
        )),
        payload_hash: Hash::new(b"fair-v2-ingress-max-proposal-payload"),
    };
    let ordinary_writes_root = Hash::new(b"fair-v2-ingress-max-proposal-writes");
    let topup_anchor_root = Hash::new(b"fair-v2-ingress-max-proposal-topup-root");
    let topup_anchor_count = wire::MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK;
    let post_state_root = wire::ExecutionCommitment::topup_post_state_root(
        topup_anchor_count,
        ordinary_writes_root,
        topup_anchor_root,
    );
    let execution_commitment = wire::ExecutionCommitment::new_without_merge_carrier(
        Hash::new(b"fair-v2-ingress-max-proposal-parent-state"),
        post_state_root,
        ordinary_writes_root,
        Some(topup_anchor_root),
        topup_anchor_count,
        1,
        Hash::new(b"fair-v2-ingress-max-proposal-executed-wire"),
    )
    .expect("maximum top-up projection is canonical");
    let signers = (0..roster_len)
        .map(|index| u32::try_from(index).expect("validator bound fits u32"))
        .collect::<Vec<_>>();
    let prepare_qc = |view| {
        let round = wire::ConsensusRound {
            context_id,
            height: u64::MAX,
            view,
        };
        wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signers: signers.clone(),
            aggregate_signature: vec![0xA5; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        }
    };
    let groups = (0..roster_len)
        .map(|index| wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare_qc(
                u64::try_from(index).expect("validator bound fits view"),
            )),
            signers: vec![u32::try_from(index).expect("validator bound fits index")],
            aggregate_signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        })
        .collect::<Vec<_>>();
    let highest_prepare_qc = groups
        .last()
        .and_then(|group| group.highest_prepare_qc.clone());
    let timeout_view = u64::try_from(roster_len).expect("validator bound fits view");
    let proposal_round = wire::ConsensusRound {
        context_id,
        height: u64::MAX,
        view: timeout_view
            .checked_add(1)
            .expect("bounded view has successor"),
    };
    let chunk_count =
        usize::try_from(layout.max_chunk_count).expect("test chunk ceiling fits usize");
    let chunk_hashes = vec![Hash::new(b"fair-v2-ingress-max-proposal-chunk"); chunk_count];
    let manifest = wire::PayloadManifest {
        round: proposal_round,
        subject,
        payload_size_bytes: layout.max_payload_size_bytes,
        layout,
        chunk_hashes,
        chunk_root: Hash::new(b"fair-v2-ingress-max-proposal-root"),
    };
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
            round: proposal_round,
            proposer: u32::try_from(roster_len.saturating_sub(1))
                .expect("validator bound fits proposer index"),
            subject,
            manifest,
            justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
                timeout_certificate: wire::TimeoutCertificate {
                    round: wire::ConsensusRound {
                        context_id,
                        height: u64::MAX,
                        view: timeout_view,
                    },
                    groups,
                },
                highest_prepare_qc,
            }),
            signature: vec![0xC3; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        }),
    ))
}

fn v2_maximum_recovery_wires(
    chain_id: &ChainId,
    requester: &PeerId,
    roster_len: usize,
) -> (BlockMessage, BlockMessage, BlockMessage) {
    let layout = minimal_rs16_layout();
    let BlockMessage::V2(proposal_message) =
        v2_maximum_structural_proposal_wire(layout, roster_len)
    else {
        unreachable!("maximum proposal fixture is v2");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal_message.payload else {
        unreachable!("maximum proposal fixture carries a proposal");
    };
    let wire::ProposalJustification::Timeout(justification) = proposal.justification else {
        unreachable!("maximum proposal fixture carries timeout justification");
    };
    let certificate = justification
        .highest_prepare_qc
        .expect("maximum proposal fixture carries its highest PrepareQC");
    let round = certificate.round;
    let subject = certificate.subject;
    let certified_body_request = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(wire::CertifiedBodyRequest {
            round,
            subject,
            certificate: certificate.clone(),
            requester: requester.clone(),
            signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        }),
    ));
    let commit_certificate_request = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateRequest(wire::CommitCertificateRequest {
            protocol_version: wire::PROTOCOL_VERSION,
            chain_id: chain_id.clone(),
            context_id: round.context_id,
            height: u64::MAX,
            requester: requester.clone(),
            signature: vec![0x5A; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
        }),
    ));
    let mut commit_certificate = certificate;
    commit_certificate.phase = wire::GlobalPhase::Commit;
    let commit_certificate_response = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"fair-v2-ingress-max-recovery-request",
                )),
                certificate: commit_certificate,
                responder: requester.clone(),
                signature: vec![0xA5; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
            },
        ),
    ));
    (
        certified_body_request,
        commit_certificate_request,
        commit_certificate_response,
    )
}

fn vote_phase(inbound: &InboundBlockMessage) -> Option<wire::GlobalPhase> {
    let BlockMessage::V2(message) = inbound.message() else {
        return None;
    };
    let wire::ConsensusMessageV2Payload::Vote(vote) = &message.payload else {
        return None;
    };
    Some(vote.phase)
}

fn vote_height(inbound: &InboundBlockMessage) -> Option<u64> {
    let BlockMessage::V2(message) = inbound.message() else {
        return None;
    };
    let wire::ConsensusMessageV2Payload::Vote(vote) = &message.payload else {
        return None;
    };
    Some(vote.round.height)
}

fn payload_chunk_index(inbound: &InboundBlockMessage) -> Option<u32> {
    let BlockMessage::V2(message) = inbound.message() else {
        return None;
    };
    let wire::ConsensusMessageV2Payload::PayloadChunk(chunk) = &message.payload else {
        return None;
    };
    Some(chunk.index)
}

fn encoded_v2_len(message: &BlockMessage) -> usize {
    let BlockMessage::V2(message) = message else {
        panic!("test fixture must be a v2 envelope");
    };
    message.encode().len()
}

fn bind_test_leader_wire_gate(
    ingress: &Arc<super::FairV2Ingress>,
    validator: &PeerId,
    round: wire::ConsensusRound,
    max_chunk_count: u32,
) -> TempDir {
    ingress.close();
    ingress
        .configure_roster([validator.clone()])
        .expect("one-validator fair-ingress geometry");
    ingress.require_leader_wire_lifecycle_gate();
    ingress.state.lock().leader_wire_max_chunk_count = max_chunk_count;

    let directory = TempDir::new().expect("temporary leader-wire directory");
    let wal_path = directory.path().join("safety.wal");
    let owner = [0xA6; 32];
    let roster = [validator.clone()].into_iter().collect();
    let capacity = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
        1,
        max_chunk_count,
    )
    .expect("finite leader-wire geometry");
    let recovery_authority =
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            owner,
            0,
            false,
        );
    let (gate, restore) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &wal_path,
        round.context_id,
        round.height,
        owner,
        roster,
        capacity,
        max_chunk_count,
        recovery_authority,
        &[],
        &[],
    )
    .expect("open exact leader-wire gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            gate,
            restore,
            super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            round.context_id,
            round.height,
        )
        .expect("bind exact leader-wire gate");
    ingress.open().expect("open bound fair ingress");
    directory
}

#[derive(Clone, Copy, Debug)]
enum RestoredLeaderWireCut {
    Reserved,
    Ingress,
    Runtime,
    Volatile,
}

struct RestoredLeaderWireFixture {
    _directory: TempDir,
    ingress: Arc<super::FairV2Ingress>,
    gate: Arc<super::serviced_candidate_store::LeaderWireLifecycleStoreGate>,
    validator: PeerId,
    alternate_validator: PeerId,
    message: BlockMessage,
    token: super::FairV2IngressLeaderWireToken,
    runtime_owner: Option<super::serviced_candidate_store::LeaderWireRuntimeOwner>,
}

fn restored_leader_wire_fixture(cut: RestoredLeaderWireCut) -> RestoredLeaderWireFixture {
    let (_handle, ingress, _relay_receiver) = test_sumeragi_handle(64);
    ingress.close();
    let validator = PeerId::new(KeyPair::random().public_key().clone());
    let alternate_validator = PeerId::new(KeyPair::random().public_key().clone());
    ingress
        .configure_roster([validator.clone(), alternate_validator.clone()])
        .expect("two-validator fair-ingress geometry");
    ingress.require_leader_wire_lifecycle_gate();
    ingress.state.lock().leader_wire_max_chunk_count = 2;

    let layout = minimal_rs16_layout();
    let message = v2_maximum_structural_proposal_wire(layout, 1);
    let BlockMessage::V2(envelope) = &message else {
        unreachable!("leader-wire restart fixture is a v2 envelope");
    };
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &envelope.payload else {
        unreachable!("leader-wire restart fixture carries Proposal");
    };
    let round = proposal.round;
    let wire_hash = CryptoHash::new(envelope.encode());
    let (identity, slot) = {
        let state = ingress.state.lock();
        match super::fair_v2_ingress_leader_wire_identity(&state, &message, &validator, wire_hash) {
            super::FairV2IngressLeaderWireDerivation::Exact { identity, slot } => (identity, slot),
            _ => panic!("proposal fixture must derive an exact leader-wire identity"),
        }
    };
    let token = super::FairV2IngressLeaderWireToken {
        source_class: identity.phase.source_class(),
        identity,
        slot,
        admission_ordinal: 7,
        scheduler_ordinal: 41,
    };

    let directory = TempDir::new().expect("temporary leader-wire restart directory");
    let wal_path = directory.path().join("safety.wal");
    let owner = [0xA7; 32];
    let roster = [validator.clone(), alternate_validator.clone()]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let capacity =
        super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(2, 2)
            .expect("finite leader-wire geometry");
    let recovery_authority =
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            owner,
            0,
            false,
        );
    let (gate, _) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &wal_path,
        round.context_id,
        round.height,
        owner,
        roster.clone(),
        capacity,
        2,
        recovery_authority,
        &[],
        &[],
    )
    .expect("open leader-wire restart fixture");
    gate.reserve(token.clone())
        .expect("reserve restart fixture token");
    if matches!(
        cut,
        RestoredLeaderWireCut::Ingress
            | RestoredLeaderWireCut::Runtime
            | RestoredLeaderWireCut::Volatile
    ) {
        gate.mark_ingress(&token)
            .expect("persist fixture ingress cut");
    }
    let runtime_owner = matches!(
        cut,
        RestoredLeaderWireCut::Runtime | RestoredLeaderWireCut::Volatile
    )
    .then(|| {
        super::serviced_candidate_store::LeaderWireRuntimeOwner::new(
            token.identity_hash(),
            token.scheduler_ordinal(),
        )
        .expect("construct fixture runtime owner")
    });
    if let Some(runtime_owner) = runtime_owner {
        let runtime = gate
            .mark_runtime(&token, runtime_owner)
            .expect("persist fixture runtime cut");
        if matches!(cut, RestoredLeaderWireCut::Volatile) {
            gate.mark_volatile_terminal(&runtime)
                .expect("persist fixture volatile cut");
        }
    }
    drop(gate);

    let recovery_authority =
        super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            round.context_id,
            round.height,
            owner,
            0,
            false,
        );
    let (gate, restore) = super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
        &wal_path,
        round.context_id,
        round.height,
        owner,
        roster,
        capacity,
        2,
        recovery_authority,
        &[],
        &[],
    )
    .expect("reopen leader-wire restart fixture");
    assert_eq!(restore.records().len(), 1);
    assert_eq!(restore.records()[0].token(), &token);
    assert_eq!(
        restore.records()[0].status(),
        super::serviced_candidate_store::LeaderWireLifecycleStatus::Dormant
    );
    assert_eq!(restore.records()[0].runtime_owner(), runtime_owner);
    assert_eq!(
        gate.earliest_ingress_scheduler_ordinal()
            .expect("read replay-dormant selector"),
        None
    );
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&gate),
            restore,
            super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0),
            round.context_id,
            round.height,
        )
        .expect("bind restored leader-wire gate");
    ingress.open().expect("open restored fair ingress");
    assert!(
        ingress
            .state
            .lock()
            .leader_wire_lifecycles
            .values()
            .all(|record| record.status == super::FairV2IngressLeaderWireStatus::Dormant)
    );

    RestoredLeaderWireFixture {
        _directory: directory,
        ingress,
        gate,
        validator,
        alternate_validator,
        message,
        token,
        runtime_owner,
    }
}
