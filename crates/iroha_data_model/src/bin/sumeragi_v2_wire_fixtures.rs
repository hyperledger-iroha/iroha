//! Regenerate the shared Sumeragi v2 wire and Native AMX fixtures.
//!
//! Run `cargo run --locked --offline -p iroha_data_model --features dev-tools --bin sumeragi_v2_wire_fixtures`
//! to refresh `fixtures/sumeragi_v2/wire_v2.tsv` and
//! `fixtures/sumeragi_v2/native_amx_v2_grouped.json`. The default mode writes;
//! pass `--out-dir <path>` to target a cache staging directory and add `--check`
//! to verify that destination against the current canonical Rust encodings and
//! JSON models.

mod native_amx_grouped;

use std::{
    collections::BTreeSet,
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    ChainId,
    block::consensus_v2::{
        BlockSubject, CertifiedBodyRequest, CertifiedBodyResponse, CommitCertificateRequest,
        CommitCertificateResponse, ConsensusMessageV2, ConsensusMessageV2Payload, ConsensusMode,
        ConsensusRound, DataAvailabilityLayout, DualQuorum, ExecutionCommitment, GlobalPhase,
        HeightContext, HeightContextId, PROTOCOL_VERSION, PayloadChunk, PayloadEncoding,
        PayloadManifest, Proposal, ProposalJustification, QuorumCertificate, SumeragiV2BodyState,
        SumeragiV2HeightContextStatus, SumeragiV2IgnoreCount, SumeragiV2IgnoreReason,
        SumeragiV2LivenessBlocker, SumeragiV2LivenessStatus, SumeragiV2LocalWorkStage,
        SumeragiV2OutboundIntentKind, SumeragiV2OutboundIntentStage,
        SumeragiV2OutboundIntentStatus, SumeragiV2ProgressTransition,
        SumeragiV2ProgressTransitionStatus, SumeragiV2QueueKind, SumeragiV2QueueStatus,
        SumeragiV2Status, SumeragiV2StatusPhase, SumeragiV2TimeoutQuorumStatus,
        SumeragiV2VoteQuorumStatus, SumeragiV2WorkStatus, TimeoutCertificate, TimeoutJustification,
        TimeoutVote, TimeoutVoteGroup, ValidatorPower, Vote, encode_payload_chunks,
    },
    peer::PeerId,
};
use norito::codec::{DecodeAll, Encode};

const FIXTURE_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/sumeragi_v2");
const WIRE_FIXTURE_BASENAME: &str = "wire_v2.tsv";
const CANONICAL_BODY: &[u8] = b"body";
const HEADER: &str = "# Accept rows were generated from iroha_data_model::block::consensus_v2 using Encode::encode.\n\
# Reject rows are Rust-encoded invalid values or deliberate corruptions of those payloads.\n\
# Bare Norito v1 layout with COMPACT_LEN; do not regenerate from an SDK codec.\n\
# kind\tname\thex\texpectation\n";

#[derive(Clone, Copy)]
enum Expectation {
    Accept,
    Reject,
}

impl Expectation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Reject => "reject",
        }
    }
}

struct FixtureRow {
    kind: &'static str,
    name: &'static str,
    bytes: Vec<u8>,
    expectation: Expectation,
}

impl FixtureRow {
    fn accepted(kind: &'static str, name: &'static str, bytes: Vec<u8>) -> Self {
        Self {
            kind,
            name,
            bytes,
            expectation: Expectation::Accept,
        }
    }

    fn rejected(kind: &'static str, name: &'static str, bytes: Vec<u8>) -> Self {
        Self {
            kind,
            name,
            bytes,
            expectation: Expectation::Reject,
        }
    }
}

struct NamedMessage {
    name: &'static str,
    message: ConsensusMessageV2,
}

struct FixtureValues {
    context: HeightContext,
    prepare: QuorumCertificate,
    status: SumeragiV2Status,
    commit_request: CommitCertificateRequest,
    commit_response: CommitCertificateResponse,
    messages: Vec<NamedMessage>,
}

impl FixtureValues {
    fn message(&self, name: &str) -> Result<&ConsensusMessageV2, Box<dyn Error>> {
        self.messages
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| &entry.message)
            .ok_or_else(|| format!("missing canonical message `{name}`").into())
    }
}

fn peer(seed: u8) -> PeerId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("deterministic Ed25519 fixture seed is valid");
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
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("four-validator roster is valid"),
        roster,
        nexus_amx_context_hash: Hash::new(b"nexus amx context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 4,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1024,
            max_chunk_count: 512,
        },
        leader_seed: [0xa5; 32],
    }
}

fn canonical_body_chunks(context: &HeightContext) -> Result<Vec<Vec<u8>>, Box<dyn Error>> {
    encode_payload_chunks(context.da_layout, CANONICAL_BODY)
        .map_err(|error| format!("failed to encode canonical fixture body: {error}").into())
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

fn execution_commitment(seed: u8) -> ExecutionCommitment {
    ExecutionCommitment::without_topups(
        Hash::new([seed, 3]),
        Hash::new([seed, 4]),
        Hash::new([seed, 5]),
        Hash::new([seed, 6]),
    )
}

fn qc(context: &HeightContext, view: u64, phase: GlobalPhase) -> QuorumCertificate {
    let seed = u8::try_from(view + 1).expect("fixture views fit in u8");
    QuorumCertificate {
        round: round(context, view),
        proposal_round: round(context, view),
        phase,
        subject: subject(seed),
        execution_commitment: execution_commitment(seed),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x5a; 48],
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the canonical fixture values are easier to audit when assembled in one deterministic sequence"
)]
fn build_values() -> Result<FixtureValues, Box<dyn Error>> {
    let context = context();
    context
        .validate()
        .map_err(|error| format!("fixture context is invalid: {error}"))?;
    let prepare = qc(&context, 1, GlobalPhase::Prepare);
    let timeout = TimeoutCertificate {
        round: round(&context, 2),
        groups: vec![TimeoutVoteGroup {
            highest_prepare_qc: Some(prepare.clone()),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x33; 48],
        }],
    };
    let encoded_body_chunks = canonical_body_chunks(&context)?;
    let manifest = PayloadManifest::derive(
        &context,
        round(&context, 1),
        subject(9),
        u64::try_from(CANONICAL_BODY.len()).expect("canonical body length fits u64"),
        &encoded_body_chunks,
    )
    .map_err(|error| format!("fixture manifest is invalid: {error}"))?;
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
    let commit_request = CommitCertificateRequest {
        protocol_version: PROTOCOL_VERSION,
        chain_id: context.chain_id.clone(),
        context_id: context.id(),
        height: context.height,
        requester: peer(99),
        signature: vec![0x81; 48],
    };
    commit_request
        .validate(&context)
        .map_err(|error| format!("fixture commit request is invalid: {error}"))?;
    let mut reproposal_commit = prepare.clone();
    reproposal_commit.round = round(&context, 9);
    reproposal_commit.proposal_round = reproposal_commit.round;
    reproposal_commit.phase = GlobalPhase::Commit;
    reproposal_commit
        .validate(&context)
        .map_err(|error| format!("fixture reproposal CommitQC is invalid: {error}"))?;
    let commit_response = CommitCertificateResponse {
        request_hash: HashOf::new(&commit_request),
        certificate: reproposal_commit.clone(),
        responder: peer(100),
        signature: vec![0x82; 48],
    };
    commit_response
        .validate_against(&context, &commit_request)
        .map_err(|error| format!("fixture commit response is invalid: {error}"))?;

    let messages = vec![
        NamedMessage {
            name: "proposal",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::Proposal(proposal)),
        },
        NamedMessage {
            name: "vote",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::Vote(Vote {
                round: manifest.round,
                proposal_round: manifest.round,
                phase: GlobalPhase::Prepare,
                subject: manifest.subject,
                execution_commitment: prepare.execution_commitment,
                signer: 0,
                signature: vec![1],
            })),
        },
        NamedMessage {
            name: "quorum_certificate",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::QuorumCertificate(
                prepare.clone(),
            )),
        },
        NamedMessage {
            name: "commit_vote_reproposal",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::Vote(Vote {
                round: reproposal_commit.round,
                proposal_round: reproposal_commit.proposal_round,
                phase: GlobalPhase::Commit,
                subject: reproposal_commit.subject,
                execution_commitment: reproposal_commit.execution_commitment,
                signer: 0,
                signature: vec![0x7A],
            })),
        },
        NamedMessage {
            name: "commit_quorum_certificate_reproposal",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::QuorumCertificate(
                reproposal_commit,
            )),
        },
        NamedMessage {
            name: "timeout_vote",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::TimeoutVote(TimeoutVote {
                round: timeout.round,
                highest_prepare_qc: Some(prepare.clone()),
                signer: 0,
                signature: vec![2],
            })),
        },
        NamedMessage {
            name: "timeout_certificate",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::TimeoutCertificate(
                timeout.clone(),
            )),
        },
        NamedMessage {
            name: "payload_manifest",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::PayloadManifest(
                manifest.clone(),
            )),
        },
        NamedMessage {
            name: "payload_chunk",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::PayloadChunk(
                PayloadChunk {
                    manifest_hash: HashOf::new(&manifest),
                    index: 0,
                    bytes: encoded_body_chunks[0].clone(),
                    sender: 0,
                    signature: vec![0x66; 48],
                },
            )),
        },
        NamedMessage {
            name: "certified_body_request",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::CertifiedBodyRequest(
                body_request.clone(),
            )),
        },
        NamedMessage {
            name: "certified_body_response",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::CertifiedBodyResponse(
                CertifiedBodyResponse {
                    request_hash: HashOf::new(&body_request),
                    manifest: manifest.clone(),
                    body: CANONICAL_BODY.to_vec(),
                    responder: 0,
                    signature: vec![3],
                },
            )),
        },
        NamedMessage {
            name: "commit_certificate_request",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::CommitCertificateRequest(
                commit_request.clone(),
            )),
        },
        NamedMessage {
            name: "commit_certificate_response",
            message: ConsensusMessageV2::new(ConsensusMessageV2Payload::CommitCertificateResponse(
                commit_response.clone(),
            )),
        },
    ];
    let status = SumeragiV2Status {
        protocol_version: PROTOCOL_VERSION,
        node_fingerprint: Hash::new(b"node"),
        build_fingerprint: Hash::new(b"build"),
        config_fingerprint: Hash::new(b"config"),
        restart_required: false,
        height_context_id: context.id(),
        height: context.height,
        view: 3,
        phase: SumeragiV2StatusPhase::Commit,
        leader: 2,
        locked_prepare_qc: Some(prepare.as_ref()),
        highest_prepare_qc: Some(prepare.as_ref()),
        last_timeout_certificate: Some(timeout.as_ref()),
        body_state: SumeragiV2BodyState::Validated,
        pending_persistence_id: Some(17),
        last_committed_height: context.height - 1,
        last_committed_subject: None,
        height_context: SumeragiV2HeightContextStatus {
            epoch: context.epoch,
            epoch_end_height: context.epoch_end_height,
            mode: context.mode,
            epoch_seed: context.leader_seed,
            validator_count: u32::try_from(context.roster.len())?,
            quorum: context.quorum,
        },
        last_commit_qc: None,
        liveness: SumeragiV2LivenessStatus {
            generation: 3,
            prepare_quorums: vec![SumeragiV2VoteQuorumStatus {
                round: prepare.round,
                proposal_round: prepare.proposal_round,
                subject: prepare.subject,
                execution_commitment: prepare.execution_commitment,
                signer_count: 2,
                signed_power: 2,
                min_signers: context.quorum.min_signers,
                total_power: context.quorum.total_power,
            }],
            commit_quorums: vec![SumeragiV2VoteQuorumStatus {
                round: round(&context, 3),
                proposal_round: round(&context, 3),
                subject: prepare.subject,
                execution_commitment: prepare.execution_commitment,
                signer_count: 1,
                signed_power: 1,
                min_signers: context.quorum.min_signers,
                total_power: context.quorum.total_power,
            }],
            timeout_quorums: vec![SumeragiV2TimeoutQuorumStatus {
                round: timeout.round,
                signer_count: 3,
                signed_power: 3,
                min_signers: context.quorum.min_signers,
                total_power: context.quorum.total_power,
                certificate_formed: true,
            }],
            outbound_intents: vec![SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::CommitVote,
                round: round(&context, 3),
                proposal_round: Some(round(&context, 3)),
                subject: Some(prepare.subject),
                execution_commitment: Some(prepare.execution_commitment),
                stage: SumeragiV2OutboundIntentStage::Sent,
            }],
            work: SumeragiV2WorkStatus {
                candidate: SumeragiV2LocalWorkStage::Complete,
                body_recovery: SumeragiV2LocalWorkStage::Complete,
                body_store: SumeragiV2LocalWorkStage::Complete,
                validation: SumeragiV2LocalWorkStage::Complete,
                application: SumeragiV2LocalWorkStage::Idle,
                successor_height: SumeragiV2LocalWorkStage::Idle,
            },
            queues: vec![SumeragiV2QueueStatus {
                queue: SumeragiV2QueueKind::EffectDispatch,
                depth: 1,
                capacity: 4,
                oldest_age_ms: Some(17),
                service_debt: 2,
            }],
            last_progress: Some(SumeragiV2ProgressTransitionStatus {
                generation: 3,
                round: prepare.round,
                transition: SumeragiV2ProgressTransition::LockInstalled,
                age_ms: 19,
            }),
            no_progress_age_ms: 19,
            blocker: Some(SumeragiV2LivenessBlocker::LocalControlPending),
            ignore_counts: vec![
                SumeragiV2IgnoreCount {
                    reason: SumeragiV2IgnoreReason::Duplicate,
                    count: 2,
                },
                SumeragiV2IgnoreCount {
                    reason: SumeragiV2IgnoreReason::IrrelevantView,
                    count: 1,
                },
            ],
        },
    };
    status
        .validate()
        .map_err(|error| format!("fixture status is invalid: {error}"))?;

    Ok(FixtureValues {
        context,
        prepare,
        status,
        commit_request,
        commit_response,
        messages,
    })
}

#[expect(
    clippy::too_many_lines,
    reason = "accepted and rejected wire fixtures intentionally share one ordered construction sequence"
)]
fn build_rows(values: &FixtureValues) -> Result<Vec<FixtureRow>, Box<dyn Error>> {
    let mut rows = values
        .messages
        .iter()
        .map(|entry| FixtureRow::accepted("message", entry.name, entry.message.encode()))
        .collect::<Vec<_>>();
    rows.extend([
        FixtureRow::accepted("status", "compact", values.status.encode()),
        FixtureRow::accepted(
            "preimage",
            "commit_certificate_request",
            values.commit_request.signature_preimage(),
        ),
        FixtureRow::accepted(
            "preimage",
            "commit_certificate_response",
            values.commit_response.signature_preimage(),
        ),
    ]);

    let canonical_manifest = values.message("payload_manifest")?;
    let canonical_vote = values.message("vote")?;
    let canonical_reproposal_vote = values.message("commit_vote_reproposal")?;
    let canonical_reproposal_qc = values.message("commit_quorum_certificate_reproposal")?;
    let canonical_request = values.message("commit_certificate_request")?;
    let canonical_response = values.message("commit_certificate_response")?;

    let mut wrong_protocol_version = canonical_manifest.clone();
    wrong_protocol_version.protocol_version = PROTOCOL_VERSION - 1;

    let mut truncated = canonical_manifest.encode();
    truncated
        .pop()
        .ok_or("canonical payload-manifest message was unexpectedly empty")?;
    let mut trailing_byte = canonical_manifest.encode();
    trailing_byte.push(0);

    let mut noncanonical_qc = values.prepare.clone();
    noncanonical_qc.signers = vec![1, 0, 2];

    let mut native_manifest_wrong_version = values.prepare.clone();
    native_manifest_wrong_version
        .execution_commitment
        .native_amx_application_manifest_version += 1;
    let mut native_manifest_zero_count_nonempty_root = values.prepare.clone();
    native_manifest_zero_count_nonempty_root
        .execution_commitment
        .native_amx_application_manifest_root =
        Hash::new(b"non-empty Native AMX application manifest");
    let mut native_manifest_nonzero_count_empty_root = values.prepare.clone();
    native_manifest_nonzero_count_empty_root
        .execution_commitment
        .native_amx_application_manifest_count = 1;
    let mut native_manifest_count_over_bound = values.prepare.clone();
    native_manifest_count_over_bound
        .execution_commitment
        .native_amx_application_manifest_root =
        Hash::new(b"oversized Native AMX application manifest");
    native_manifest_count_over_bound
        .execution_commitment
        .native_amx_application_manifest_count =
        iroha_data_model::block::consensus_v2::MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES + 1;

    let mut commit_vote = canonical_vote.clone();
    let ConsensusMessageV2Payload::Vote(vote) = &mut commit_vote.payload else {
        return Err("canonical vote fixture contains the wrong payload".into());
    };
    vote.phase = GlobalPhase::Commit;
    let retired_zero_prepare_tag = replace_single_difference(
        &canonical_vote.encode(),
        &commit_vote.encode(),
        0,
        "global phase discriminant",
    )?;

    let overlapping_timeout_groups = TimeoutCertificate {
        round: round(&values.context, 2),
        groups: vec![
            TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0],
                aggregate_signature: vec![1],
            },
            TimeoutVoteGroup {
                highest_prepare_qc: Some(values.prepare.clone()),
                signers: vec![0, 2],
                aggregate_signature: vec![2],
            },
        ],
    };

    let mut unknown_payload_tag = canonical_manifest.encode();
    replace_first_guarded(
        &mut unknown_payload_tag,
        &[5, 0, 0, 0],
        &[11, 0, 0, 0],
        "payload-manifest discriminant",
    )?;

    let mut wrong_nested_request = values.commit_request.clone();
    wrong_nested_request.protocol_version = PROTOCOL_VERSION - 1;
    let mut empty_request_signature = values.commit_request.clone();
    empty_request_signature.signature.clear();
    let mut truncated_request_signature = canonical_request.encode();
    truncated_request_signature
        .pop()
        .ok_or("canonical commit request message was unexpectedly empty")?;

    let mut empty_response_signature = values.commit_response.clone();
    empty_response_signature.signature.clear();
    let mut truncated_response_signature = canonical_response.encode();
    truncated_response_signature
        .pop()
        .ok_or("canonical commit response message was unexpectedly empty")?;
    let mut prepare_response = values.commit_response.clone();
    prepare_response.certificate.phase = GlobalPhase::Prepare;

    let mut split_round_vote = canonical_reproposal_vote.clone();
    let ConsensusMessageV2Payload::Vote(vote) = &mut split_round_vote.payload else {
        return Err("canonical reproposal vote fixture contains the wrong payload".into());
    };
    vote.proposal_round = round(&values.context, 1);
    let mut split_round_qc = canonical_reproposal_qc.clone();
    let ConsensusMessageV2Payload::QuorumCertificate(certificate) = &mut split_round_qc.payload
    else {
        return Err("canonical reproposal QC fixture contains the wrong payload".into());
    };
    certificate.proposal_round = round(&values.context, 1);

    let mut invalid_chain_utf8 = canonical_request.encode();
    replace_first_guarded(
        &mut invalid_chain_utf8,
        b"sumeragi-v2-test",
        b"\xffumeragi-v2-test",
        "commit-request chain id",
    )?;

    rows.extend([
        FixtureRow::rejected(
            "negative_message",
            "wrong_protocol_version",
            wrong_protocol_version.encode(),
        ),
        FixtureRow::rejected("negative_message", "truncated", truncated),
        FixtureRow::rejected("negative_message", "trailing_byte", trailing_byte),
        FixtureRow::rejected(
            "negative_message",
            "noncanonical_signers",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::QuorumCertificate(
                noncanonical_qc,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "execution_commitment_native_manifest_wrong_version",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::QuorumCertificate(
                native_manifest_wrong_version,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "execution_commitment_native_manifest_zero_count_nonempty_root",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::QuorumCertificate(
                native_manifest_zero_count_nonempty_root,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "execution_commitment_native_manifest_nonzero_count_empty_root",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::QuorumCertificate(
                native_manifest_nonzero_count_empty_root,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "execution_commitment_native_manifest_count_1025",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::QuorumCertificate(
                native_manifest_count_over_bound,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "retired_zero_prepare_tag",
            retired_zero_prepare_tag,
        ),
        FixtureRow::rejected(
            "negative_message",
            "overlapping_timeout_groups",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::TimeoutCertificate(
                overlapping_timeout_groups,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "unknown_payload_tag",
            unknown_payload_tag,
        ),
        FixtureRow::rejected(
            "negative_message",
            "commit_request_wrong_nested_protocol",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::CommitCertificateRequest(
                wrong_nested_request,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "commit_request_empty_signature",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::CommitCertificateRequest(
                empty_request_signature,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "commit_request_truncated_signature",
            truncated_request_signature,
        ),
        FixtureRow::rejected(
            "negative_message",
            "commit_response_empty_signature",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::CommitCertificateResponse(
                empty_response_signature,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "commit_response_truncated_signature",
            truncated_response_signature,
        ),
        FixtureRow::rejected(
            "negative_message",
            "commit_response_prepare_certificate",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::CommitCertificateResponse(
                prepare_response,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "commit_vote_split_round",
            split_round_vote.encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "commit_quorum_certificate_split_round",
            split_round_qc.encode(),
        ),
        FixtureRow::rejected(
            "negative_message",
            "commit_request_invalid_chain_utf8",
            invalid_chain_utf8,
        ),
    ]);

    let mut wrong_request_hash = values.commit_response.clone();
    wrong_request_hash.request_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"wrong commit request"));
    let mut wrong_context = values.commit_response.clone();
    let wrong_context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"wrong height context",
    )));
    wrong_context.certificate.round.context_id = wrong_context_id;
    wrong_context.certificate.proposal_round.context_id = wrong_context_id;
    let mut wrong_height = values.commit_response.clone();
    wrong_height.certificate.round.height += 1;
    wrong_height.certificate.proposal_round.height += 1;
    rows.extend([
        FixtureRow::rejected(
            "negative_binding",
            "commit_response_wrong_request_hash",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::CommitCertificateResponse(
                wrong_request_hash,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_binding",
            "commit_response_wrong_context",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::CommitCertificateResponse(
                wrong_context,
            ))
            .encode(),
        ),
        FixtureRow::rejected(
            "negative_binding",
            "commit_response_wrong_height",
            ConsensusMessageV2::new(ConsensusMessageV2Payload::CommitCertificateResponse(
                wrong_height,
            ))
            .encode(),
        ),
    ]);

    let mut wrong_status_version = values.status.clone();
    wrong_status_version.protocol_version = PROTOCOL_VERSION - 1;
    let mut truncated_status = values.status.encode();
    truncated_status
        .pop()
        .ok_or("canonical status was unexpectedly empty")?;
    rows.extend([
        FixtureRow::rejected(
            "negative_status",
            "wrong_protocol_version",
            wrong_status_version.encode(),
        ),
        FixtureRow::rejected("negative_status", "truncated", truncated_status),
    ]);

    Ok(rows)
}

fn replace_single_difference(
    canonical: &[u8],
    comparison: &[u8],
    replacement: u8,
    label: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    if canonical.len() != comparison.len() {
        return Err(format!("{label} comparison changed the encoded length").into());
    }
    let differences = canonical
        .iter()
        .zip(comparison)
        .enumerate()
        .filter_map(|(index, (left, right))| (left != right).then_some(index))
        .collect::<Vec<_>>();
    let [index] = differences.as_slice() else {
        return Err(format!(
            "expected exactly one encoded byte for {label}, found {}",
            differences.len()
        )
        .into());
    };
    let mut corrupted = canonical.to_vec();
    corrupted[*index] = replacement;
    Ok(corrupted)
}

fn replace_first_guarded(
    bytes: &mut [u8],
    needle: &[u8],
    replacement: &[u8],
    label: &str,
) -> Result<(), Box<dyn Error>> {
    if needle.len() != replacement.len() {
        return Err(format!("{label} replacement changes encoded length").into());
    }
    let matches = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect::<Vec<_>>();
    let Some(&index) = matches.first() else {
        return Err(format!("could not locate encoded {label}").into());
    };
    if matches.len() != 1 {
        return Err(format!(
            "encoded {label} was ambiguous: found {} occurrences",
            matches.len()
        )
        .into());
    }
    bytes[index..index + needle.len()].copy_from_slice(replacement);
    Ok(())
}

fn decode_message(bytes: &[u8]) -> Result<ConsensusMessageV2, String> {
    let mut cursor = bytes;
    ConsensusMessageV2::decode_all(&mut cursor)
        .map_err(|error| format!("failed to decode message: {error:?}"))
}

fn decode_status(bytes: &[u8]) -> Result<SumeragiV2Status, String> {
    let mut cursor = bytes;
    SumeragiV2Status::decode_all(&mut cursor)
        .map_err(|error| format!("failed to decode status: {error:?}"))
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture validator audits the complete canonical row set as one invariant"
)]
fn validate_rows(rows: &[FixtureRow], values: &FixtureValues) -> Result<(), Box<dyn Error>> {
    let mut keys = BTreeSet::new();
    for row in rows {
        if row.bytes.is_empty() {
            return Err(format!("{}/{} generated an empty fixture", row.kind, row.name).into());
        }
        if !keys.insert((row.kind, row.name)) {
            return Err(format!("duplicate fixture row {}/{}", row.kind, row.name).into());
        }
    }

    for row in rows.iter().filter(|row| row.kind == "message") {
        let decoded = decode_message(&row.bytes)?;
        decoded
            .validate_version()
            .map_err(|error| format!("canonical message {} is invalid: {error}", row.name))?;
        if decoded.encode() != row.bytes {
            return Err(format!("canonical message {} did not round-trip", row.name).into());
        }
    }
    let status = row(rows, "status", "compact")?;
    if decode_status(&status.bytes)?.encode() != status.bytes {
        return Err("canonical compact status did not round-trip".into());
    }

    for name in [
        "truncated",
        "trailing_byte",
        "retired_zero_prepare_tag",
        "unknown_payload_tag",
        "commit_request_truncated_signature",
        "commit_response_truncated_signature",
        "commit_request_invalid_chain_utf8",
    ] {
        if decode_message(&row(rows, "negative_message", name)?.bytes).is_ok() {
            return Err(format!("negative message {name} unexpectedly decoded").into());
        }
    }

    let wrong_version =
        decode_message(&row(rows, "negative_message", "wrong_protocol_version")?.bytes)?;
    if wrong_version.validate_version().is_ok() {
        return Err("wrong_protocol_version unexpectedly passed validation".into());
    }

    let noncanonical =
        decode_message(&row(rows, "negative_message", "noncanonical_signers")?.bytes)?;
    let ConsensusMessageV2Payload::QuorumCertificate(certificate) = noncanonical.payload else {
        return Err("noncanonical_signers generated the wrong payload".into());
    };
    if certificate.validate(&values.context).is_ok() {
        return Err("noncanonical_signers unexpectedly passed validation".into());
    }

    for name in [
        "execution_commitment_native_manifest_wrong_version",
        "execution_commitment_native_manifest_zero_count_nonempty_root",
        "execution_commitment_native_manifest_nonzero_count_empty_root",
        "execution_commitment_native_manifest_count_1025",
    ] {
        let message = decode_message(&row(rows, "negative_message", name)?.bytes)?;
        let ConsensusMessageV2Payload::QuorumCertificate(certificate) = message.payload else {
            return Err(format!("{name} generated the wrong payload").into());
        };
        if certificate.validate(&values.context).is_ok() {
            return Err(format!("{name} unexpectedly passed validation").into());
        }
    }

    let overlapping =
        decode_message(&row(rows, "negative_message", "overlapping_timeout_groups")?.bytes)?;
    let ConsensusMessageV2Payload::TimeoutCertificate(certificate) = overlapping.payload else {
        return Err("overlapping_timeout_groups generated the wrong payload".into());
    };
    if certificate.validate(&values.context).is_ok() {
        return Err("overlapping_timeout_groups unexpectedly passed validation".into());
    }

    for name in [
        "commit_request_wrong_nested_protocol",
        "commit_request_empty_signature",
    ] {
        let message = decode_message(&row(rows, "negative_message", name)?.bytes)?;
        let ConsensusMessageV2Payload::CommitCertificateRequest(request) = message.payload else {
            return Err(format!("{name} generated the wrong payload").into());
        };
        if request.validate(&values.context).is_ok() {
            return Err(format!("{name} unexpectedly passed validation").into());
        }
    }
    for name in [
        "commit_response_empty_signature",
        "commit_response_prepare_certificate",
    ] {
        let message = decode_message(&row(rows, "negative_message", name)?.bytes)?;
        let ConsensusMessageV2Payload::CommitCertificateResponse(response) = message.payload else {
            return Err(format!("{name} generated the wrong payload").into());
        };
        if response.validate(&values.context).is_ok() {
            return Err(format!("{name} unexpectedly passed validation").into());
        }
    }

    for name in [
        "commit_response_wrong_request_hash",
        "commit_response_wrong_context",
        "commit_response_wrong_height",
    ] {
        let message = decode_message(&row(rows, "negative_binding", name)?.bytes)?;
        let ConsensusMessageV2Payload::CommitCertificateResponse(response) = message.payload else {
            return Err(format!("{name} generated the wrong payload").into());
        };
        if response
            .validate_against(&values.context, &values.commit_request)
            .is_ok()
        {
            return Err(format!("{name} unexpectedly passed binding validation").into());
        }
    }

    let wrong_status =
        decode_status(&row(rows, "negative_status", "wrong_protocol_version")?.bytes)?;
    if wrong_status.protocol_version == PROTOCOL_VERSION {
        return Err("negative status retained the canonical protocol version".into());
    }
    if decode_status(&row(rows, "negative_status", "truncated")?.bytes).is_ok() {
        return Err("truncated status unexpectedly decoded".into());
    }
    Ok(())
}

fn row<'a>(
    rows: &'a [FixtureRow],
    kind: &str,
    name: &str,
) -> Result<&'a FixtureRow, Box<dyn Error>> {
    rows.iter()
        .find(|row| row.kind == kind && row.name == name)
        .ok_or_else(|| format!("missing generated fixture {kind}/{name}").into())
}

fn render(rows: &[FixtureRow]) -> String {
    let mut rendered = String::from(HEADER);
    for row in rows {
        rendered.push_str(row.kind);
        rendered.push('\t');
        rendered.push_str(row.name);
        rendered.push('\t');
        rendered.push_str(&hex::encode(&row.bytes));
        rendered.push('\t');
        rendered.push_str(row.expectation.as_str());
        rendered.push('\n');
    }
    rendered
}

fn write_fixture(path: &Path, rendered: &str, check_only: bool) -> Result<(), Box<dyn Error>> {
    if check_only {
        let existing = fs::read_to_string(path)?;
        if existing != rendered {
            return Err(format!(
                "fixture {} is stale; run cargo run --locked --offline -p iroha_data_model --features dev-tools --bin sumeragi_v2_wire_fixtures",
                path.display(),
            )
            .into());
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, rendered)?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    check_only: bool,
    output_dir: PathBuf,
}

fn parse_options_from(
    arguments: impl IntoIterator<Item = String>,
) -> Result<Options, Box<dyn Error>> {
    let mut check_only = false;
    let mut output_dir = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--check" if !check_only => check_only = true,
            "--check" => return Err("--check was supplied more than once".into()),
            "--out-dir" if output_dir.is_none() => {
                let value = arguments
                    .next()
                    .ok_or("--out-dir requires a directory path")?;
                if value.is_empty() || value.starts_with('-') {
                    return Err("--out-dir requires a non-empty directory path".into());
                }
                output_dir = Some(PathBuf::from(value));
            }
            "--out-dir" => return Err("--out-dir was supplied more than once".into()),
            _ => {
                return Err(format!(
                    "unknown argument `{argument}`; expected --check or --out-dir <path>"
                )
                .into());
            }
        }
    }
    Ok(Options {
        check_only,
        output_dir: output_dir.unwrap_or_else(|| PathBuf::from(FIXTURE_DIRECTORY)),
    })
}

fn parse_options() -> Result<Options, Box<dyn Error>> {
    parse_options_from(env::args().skip(1))
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = parse_options()?;
    let values = build_values()?;
    let rows = build_rows(&values)?;
    validate_rows(&rows, &values)?;
    write_fixture(
        &options.output_dir.join(WIRE_FIXTURE_BASENAME),
        &render(&rows),
        options.check_only,
    )?;
    native_amx_grouped::write_fixture(
        &options
            .output_dir
            .join(native_amx_grouped::FIXTURE_BASENAME),
        options.check_only,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_body_chunks_cover_the_complete_rs16_stripe() {
        let context = context();
        let chunks = canonical_body_chunks(&context).expect("canonical encoded body chunks");
        assert_eq!(chunks, vec![CANONICAL_BODY.to_vec(); 2]);

        let manifest = PayloadManifest::derive(
            &context,
            round(&context, 1),
            subject(9),
            u64::try_from(CANONICAL_BODY.len()).expect("canonical body length fits u64"),
            &chunks,
        )
        .expect("complete encoded stripe has a valid manifest");
        assert_eq!(manifest.chunk_hashes.len(), 2);
        assert_eq!(manifest.validate(&context), Ok(()));
    }

    #[test]
    fn staged_output_directory_is_explicit() {
        let options = parse_options_from([
            "--out-dir".to_owned(),
            "/tmp/sumeragi-fixtures".to_owned(),
            "--check".to_owned(),
        ])
        .expect("valid staged options");
        assert!(options.check_only);
        assert_eq!(options.output_dir, Path::new("/tmp/sumeragi-fixtures"));
    }

    #[test]
    fn duplicate_and_incomplete_options_are_rejected() {
        for arguments in [
            vec!["--check".to_owned(), "--check".to_owned()],
            vec!["--out-dir".to_owned()],
            vec!["--out-dir".to_owned(), String::new()],
            vec!["--out-dir".to_owned(), "--check".to_owned()],
            vec!["--out-dir".to_owned(), "-h".to_owned()],
            vec![
                "--out-dir".to_owned(),
                "first".to_owned(),
                "--out-dir".to_owned(),
                "second".to_owned(),
            ],
        ] {
            assert!(parse_options_from(arguments).is_err());
        }
    }
}
