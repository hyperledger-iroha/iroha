use super::super::schema::DurableContinuationEdge;
use super::*;
use crate::sumeragi::{
    v2::AdapterEquivocationEvidence,
    v2_certified_serve_payload_store::CertifiedServePayloadStoreV1,
    v2_core::Generation,
    v2_runtime::{RuntimeEffectOwnership, bind_adapter_effect_batch_ownership},
    v2_transport::authenticate_certified_body_request,
};
#[cfg(feature = "bls")]
use crate::sumeragi::{
    v2::VerifiedHeightContext, v2_body_store::V2BodyStore,
    v2_certified_serve_payload_store::CertifiedServePayloadNegativeOutcome,
};
#[cfg(feature = "bls")]
use iroha_crypto::SignatureOf;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
#[cfg(feature = "bls")]
use iroha_data_model::block::{BlockHeader, BlockSignature, SignedBlock};
use iroha_data_model::peer::PeerId;
use std::collections::BTreeSet;
#[cfg(feature = "bls")]
use std::num::NonZeroU64;
use tempfile::TempDir;
pub(in crate::sumeragi::v2_lifecycle_coordinator) struct ReplayCase {
    pub(in crate::sumeragi::v2_lifecycle_coordinator) authority: LifecycleReplayAuthorityV1,
    pub(in crate::sumeragi::v2_lifecycle_coordinator) key: LifecycleKey,
    pub(in crate::sumeragi::v2_lifecycle_coordinator) work_class: LifecycleWorkClass,
    pub(in crate::sumeragi::v2_lifecycle_coordinator) stage: LifecycleStage,
    pub(in crate::sumeragi::v2_lifecycle_coordinator) payload: DurablePayloadReference,
}
struct Fixture {
    context: LifecycleContext,
    tag: ReplayEventTagV1,
    enter_tag: ReplayEventTagV1,
    proposal: wire::Proposal,
    conflicting_proposal: wire::Proposal,
    prepare_vote: wire::Vote,
    commit_vote: wire::Vote,
    conflicting_vote: wire::Vote,
    timeout_vote: wire::TimeoutVote,
    conflicting_timeout_vote: wire::TimeoutVote,
    prepare_qc: wire::QuorumCertificate,
    commit_qc: wire::QuorumCertificate,
    timeout_certificate: wire::TimeoutCertificate,
    serve_request: wire::CertifiedBodyRequest,
    body_receipt: DurableBodyReceipt,
    body_payload: DurablePayloadReference,
    serve_payload: DurablePayloadReference,
}
impl Fixture {
    fn new() -> Self {
        let context_hash = Hash::new(b"lifecycle replay authority context");
        Self::for_record(
            LifecycleContext::new(digest_from_bytes(context_hash.as_ref()), 7),
            0,
        )
    }
    fn for_record(context: LifecycleContext, seed: u8) -> Self {
        let context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed(
            *context.id().as_bytes(),
        )));
        let context =
            LifecycleContext::new(digest_from_bytes(context_id.0.as_ref()), context.height());
        let round = wire::ConsensusRound {
            context_id,
            height: context.height(),
            view: u64::from(seed),
        };
        let subject_marker = seed.wrapping_add(0x31);
        let subject = self::subject(subject_marker);
        let conflicting_subject = self::subject(subject_marker.wrapping_add(1));
        let proposal_manifest = self::manifest(round, subject, subject_marker);
        let conflicting_manifest =
            self::manifest(round, conflicting_subject, subject_marker.wrapping_add(1));
        let proposal = wire::Proposal {
            round,
            proposer: 0,
            subject,
            manifest: proposal_manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: vec![subject_marker],
        };
        let conflicting_proposal = wire::Proposal {
            subject: conflicting_subject,
            manifest: conflicting_manifest,
            signature: vec![subject_marker.wrapping_add(1)],
            ..proposal.clone()
        };
        let commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"replay parent state"),
            Hash::new(b"replay post state"),
            Hash::new(b"replay ordinary writes"),
            1,
            Hash::new(b"replay executed block"),
        );
        let prepare_vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: commitment,
            signer: 0,
            signature: vec![0x41],
        };
        let commit_vote = wire::Vote {
            phase: wire::GlobalPhase::Commit,
            signature: vec![0x42],
            ..prepare_vote.clone()
        };
        let conflicting_vote = wire::Vote {
            subject: conflicting_subject,
            signature: vec![0x43],
            ..prepare_vote.clone()
        };
        let prepare_qc = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: commitment,
            signers: vec![0],
            aggregate_signature: vec![0x51],
        };
        let commit_qc = wire::QuorumCertificate {
            phase: wire::GlobalPhase::Commit,
            aggregate_signature: vec![0x52],
            ..prepare_qc.clone()
        };
        let timeout_vote = wire::TimeoutVote {
            round,
            highest_prepare_qc: Some(prepare_qc.clone()),
            signer: 0,
            signature: vec![0x61],
        };
        let conflicting_timeout_vote = wire::TimeoutVote {
            highest_prepare_qc: None,
            signature: vec![0x62],
            ..timeout_vote.clone()
        };
        let timeout_certificate = wire::TimeoutCertificate {
            round,
            groups: vec![wire::TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare_qc.clone()),
                signers: vec![0],
                aggregate_signature: vec![0x63],
            }],
        };
        let requester_key =
            KeyPair::try_from_seed(vec![seed.wrapping_add(0x91).max(1); 32], Algorithm::Ed25519)
                .expect("deterministic replay fixture requester key");
        let serve_request = wire::CertifiedBodyRequest {
            round,
            subject,
            certificate: prepare_qc.clone(),
            requester: PeerId::new(requester_key.public_key().clone()),
            signature: vec![0x71],
        };
        let body_receipt = DurableBodyReceipt::for_test(
            context_id,
            round,
            subject,
            HashOf::new(&proposal.manifest),
        );
        let body_payload = DurablePayloadReference::BodyFrame(
            durable_body_frame_reference(context, &body_receipt)
                .expect("canonical replay fixture body belongs to its context"),
        );
        let serve_payload = DurablePayloadReference::CertifiedServePending {
            request: digest_from_bytes(HashOf::new(&serve_request).as_ref()),
            certificate: digest_from_bytes(HashOf::new(&serve_request.certificate).as_ref()),
        };
        Self {
            context,
            tag: ReplayEventTagV1::new(round.height, round.view, 3),
            enter_tag: ReplayEventTagV1::new(round.height, round.view + 1, 0),
            proposal,
            conflicting_proposal,
            prepare_vote,
            commit_vote,
            conflicting_vote,
            timeout_vote,
            conflicting_timeout_vote,
            prepare_qc,
            commit_qc,
            timeout_certificate,
            serve_request,
            body_receipt,
            body_payload,
            serve_payload,
        }
    }
    fn cases(&self) -> Vec<ReplayCase> {
        let locator = RecoveredWalFrameIdentity::for_test(8, 9, [0x21; 32]).persisted_locator();
        let mut unsigned_proposal = self.proposal.clone();
        unsigned_proposal.signature.clear();
        let mut unsigned_prepare = self.prepare_vote.clone();
        unsigned_prepare.signature.clear();
        let mut unsigned_commit = self.commit_vote.clone();
        unsigned_commit.signature.clear();
        let mut unsigned_timeout = self.timeout_vote.clone();
        unsigned_timeout.signature.clear();
        let proposal_equivocation = crate::sumeragi::evidence::canonicalize_v2_conflict(
            &wire::SumeragiV2Equivocation::Proposal {
                first: self.proposal.clone(),
                second: self.conflicting_proposal.clone(),
            },
        );
        let vote_equivocation = crate::sumeragi::evidence::canonicalize_v2_conflict(
            &wire::SumeragiV2Equivocation::PhaseVote {
                first: self.prepare_vote.clone(),
                second: self.conflicting_vote.clone(),
            },
        );
        let timeout_equivocation = crate::sumeragi::evidence::canonicalize_v2_conflict(
            &wire::SumeragiV2Equivocation::TimeoutVote {
                first: self.timeout_vote.clone(),
                second: self.conflicting_timeout_vote.clone(),
            },
        );
        let sources = vec![
            (
                LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                    locator,
                    role: ReplayWalRoleV1::PROPOSAL_INTENT,
                    tag: self.tag,
                    action: WalReplayActionV1::SignProposal(unsigned_proposal),
                }),
                LifecycleStageKind::SignProposal,
                DurablePayloadReference::None,
            ),
            (
                LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                    locator,
                    role: ReplayWalRoleV1::PREPARE_INTENT,
                    tag: self.tag,
                    action: WalReplayActionV1::SignVote(unsigned_prepare),
                }),
                LifecycleStageKind::SignPrepareVote,
                DurablePayloadReference::None,
            ),
            (
                LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                    locator,
                    role: ReplayWalRoleV1::LOCK_AND_COMMIT,
                    tag: self.tag,
                    action: WalReplayActionV1::SignVote(unsigned_commit),
                }),
                LifecycleStageKind::SignCommitVote,
                DurablePayloadReference::None,
            ),
            (
                LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                    locator,
                    role: ReplayWalRoleV1::TIMEOUT_INTENT,
                    tag: self.tag,
                    action: WalReplayActionV1::SignTimeoutVote(unsigned_timeout),
                }),
                LifecycleStageKind::SignTimeoutVote,
                DurablePayloadReference::None,
            ),
            (
                LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
                    tag: self.tag,
                    origin: BodyPipelineOriginV1::Certified {
                        certificate: self.prepare_qc.clone(),
                        manifest: self.proposal.manifest.clone(),
                        fetch_manifest_present: true,
                        certified_sources: Vec::new(),
                    },
                }),
                LifecycleStageKind::FetchBody,
                DurablePayloadReference::None,
            ),
            (
                LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
                    tag: self.tag,
                    origin: BodyPipelineOriginV1::Certified {
                        certificate: self.prepare_qc.clone(),
                        manifest: self.proposal.manifest.clone(),
                        fetch_manifest_present: true,
                        certified_sources: Vec::new(),
                    },
                }),
                LifecycleStageKind::StoreBody,
                self.body_payload,
            ),
            (
                LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
                    tag: self.tag,
                    origin: BodyPipelineOriginV1::Certified {
                        certificate: self.prepare_qc.clone(),
                        manifest: self.proposal.manifest.clone(),
                        fetch_manifest_present: true,
                        certified_sources: Vec::new(),
                    },
                }),
                LifecycleStageKind::ValidateBody,
                self.body_payload,
            ),
            (
                LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                    locator,
                    role: ReplayWalRoleV1::DECISION,
                    tag: self.tag,
                    action: WalReplayActionV1::ApplyDecision(self.commit_qc.clone()),
                }),
                LifecycleStageKind::ApplyDecision,
                self.body_payload,
            ),
            broadcast_case(
                wire::ConsensusMessageV2Payload::Proposal(self.proposal.clone()),
                LifecycleStageKind::BroadcastProposal,
            ),
            broadcast_case(
                wire::ConsensusMessageV2Payload::Vote(self.prepare_vote.clone()),
                LifecycleStageKind::BroadcastPrepareVote,
            ),
            broadcast_case(
                wire::ConsensusMessageV2Payload::Vote(self.commit_vote.clone()),
                LifecycleStageKind::BroadcastCommitVote,
            ),
            broadcast_case(
                wire::ConsensusMessageV2Payload::QuorumCertificate(self.prepare_qc.clone()),
                LifecycleStageKind::BroadcastPrepareQc,
            ),
            broadcast_case(
                wire::ConsensusMessageV2Payload::QuorumCertificate(self.commit_qc.clone()),
                LifecycleStageKind::BroadcastCommitQc,
            ),
            broadcast_case(
                wire::ConsensusMessageV2Payload::TimeoutVote(self.timeout_vote.clone()),
                LifecycleStageKind::BroadcastTimeoutVote,
            ),
            broadcast_case(
                wire::ConsensusMessageV2Payload::TimeoutCertificate(
                    self.timeout_certificate.clone(),
                ),
                LifecycleStageKind::BroadcastTc,
            ),
            (
                LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
                    locator,
                    role: ReplayWalRoleV1::INSTALL_TIMEOUT,
                    tag: self.enter_tag,
                    action: WalReplayActionV1::EnterView {
                        certificate: self.timeout_certificate.clone(),
                        protected_lock: Some(self.prepare_qc.clone()),
                    },
                }),
                LifecycleStageKind::EnterView,
                DurablePayloadReference::None,
            ),
            (
                LifecycleReplaySourceV1::Equivocation(proposal_equivocation),
                LifecycleStageKind::ReportProposalEquivocation,
                DurablePayloadReference::None,
            ),
            (
                LifecycleReplaySourceV1::Equivocation(vote_equivocation),
                LifecycleStageKind::ReportVoteEquivocation,
                DurablePayloadReference::None,
            ),
            (
                LifecycleReplaySourceV1::Equivocation(timeout_equivocation),
                LifecycleStageKind::ReportTimeoutEquivocation,
                DurablePayloadReference::None,
            ),
            (
                LifecycleReplaySourceV1::InvalidCertifiedBody(InvalidBodyReplaySourceV1 {
                    validation_origin: BodyPipelineReplaySourceV1 {
                        tag: self.tag,
                        origin: BodyPipelineOriginV1::Proposal(self.proposal.clone()),
                    },
                    certificate: self.prepare_qc.clone(),
                    outcome: RejectedBodyOutcomeBindingV1 {
                        manifest: self.proposal.manifest.clone(),
                        body_frame_hash: [0x81; 32],
                        rejection_code: 0,
                    },
                }),
                LifecycleStageKind::ReportInvalidBody,
                DurablePayloadReference::None,
            ),
            (
                self.serve_storage_source(),
                LifecycleStageKind::CertifiedServe,
                self.serve_payload,
            ),
            (
                self.serve_storage_source(),
                LifecycleStageKind::ProducerTurn,
                DurablePayloadReference::None,
            ),
        ];
        sources
            .into_iter()
            .map(|(source, stage_kind, payload)| {
                replay_case(self.context, source, stage_kind, payload)
            })
            .collect()
    }
    fn recovered_tag(&self) -> EventTag {
        EventTag::new(
            self.tag.height,
            self.tag.view,
            Generation::new(self.tag.generation),
        )
    }
    fn serve_storage_source(&self) -> LifecycleReplaySourceV1 {
        LifecycleReplaySourceV1::CertifiedServeStorage(CertifiedServeStorageSourceV1 {
            request: self.serve_request.clone(),
            payload_hash: [0x91; 32],
            local_retainer: 0,
        })
    }
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_record_fixture(
    context: LifecycleContext,
    stage: LifecycleStageKind,
    seed: u8,
) -> ReplayCase {
    Fixture::for_record(context, seed)
        .cases()
        .into_iter()
        .find(|case| case.stage.kind() == stage)
        .expect("the canonical V1 fixture covers every lifecycle stage")
}

/// Build the canonical replay pair for one exact unsigned/signed timeout edge.
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_timeout_sign_broadcast_fixture(
    context: LifecycleContext,
    unsigned: wire::TimeoutVote,
    signed: wire::TimeoutVote,
) -> [ReplayCase; 2] {
    assert!(unsigned.signature.is_empty());
    assert!(!signed.signature.is_empty());
    let mut expected = unsigned.clone();
    expected.signature.clone_from(&signed.signature);
    assert_eq!(expected, signed);
    let locator = RecoveredWalFrameIdentity::for_test(88, 89, [0xD8; 32]).persisted_locator();
    let tag = ReplayEventTagV1::new(unsigned.round.height, unsigned.round.view, 0);
    let parent = replay_case(
        context,
        LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator,
            role: ReplayWalRoleV1::TIMEOUT_INTENT,
            tag,
            action: WalReplayActionV1::SignTimeoutVote(unsigned),
        }),
        LifecycleStageKind::SignTimeoutVote,
        DurablePayloadReference::None,
    );
    let child = replay_case(
        context,
        LifecycleReplaySourceV1::ConsensusBroadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(signed),
        )),
        LifecycleStageKind::BroadcastTimeoutVote,
        DurablePayloadReference::None,
    );
    [parent, child]
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_replay_authority_for_payload_fixture(
    context: LifecycleContext,
    stage: LifecycleStageKind,
    seed: u8,
    payload: DurablePayloadReference,
) -> LifecycleReplayAuthorityV1 {
    let case = exact_record_fixture(context, stage, seed);
    canonical_replay_authority(
        context,
        case.authority.source.clone(),
        stage,
        ReplayPayloadBindingV1::from_payload(payload),
    )
    .unwrap_or(case.authority)
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_body_record_fixture(
    context: LifecycleContext,
    stage: LifecycleStageKind,
    seed: u8,
) -> (ReplayCase, DurableBodyReceipt) {
    let fixture = Fixture::for_record(context, seed);
    let receipt = fixture.body_receipt.clone();
    let case = fixture
        .cases()
        .into_iter()
        .find(|case| case.stage.kind() == stage)
        .expect("the canonical V1 fixture covers every lifecycle stage");
    (case, receipt)
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_body_execution_commitment_fixture(
    context: LifecycleContext,
    seed: u8,
) -> wire::ExecutionCommitment {
    Fixture::for_record(context, seed)
        .prepare_qc
        .execution_commitment
}
/// Build one pending certified-Fetch candidate from its real verified fixture inputs.
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_pending_certified_fetch_candidate_fixture(
    verified: &crate::sumeragi::v2::VerifiedHeightContext,
    effect: &AdapterEffect,
    pending: &PendingRuntimeEffectBinding,
) -> Option<CandidateAdmission> {
    let context = super::super::projection::lifecycle_context(verified.context());
    let projected = super::super::projection::authority_free_admission_projection(
        context, verified, effect, pending,
    )
    .ok()?;
    let AdapterEffect::FetchBody {
        tag,
        manifest: Some(manifest),
        certified_sources,
        certificate: Some(certificate),
        ..
    } = effect
    else {
        return None;
    };
    if verified.verify_quorum_certificate(certificate).is_err()
        || !certified_sources.iter().eq(verified
            .context()
            .roster
            .iter()
            .map(|entry| &entry.validator))
    {
        return None;
    }
    let authority = canonical_replay_authority(
        context,
        LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
            tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
            origin: BodyPipelineOriginV1::Certified {
                certificate: certificate.clone(),
                manifest: manifest.clone(),
                fetch_manifest_present: true,
                certified_sources: certified_sources.clone(),
            },
        }),
        LifecycleStageKind::FetchBody,
        ReplayPayloadBindingV1::None,
    )?;
    candidate_from_authorized_projection(
        context,
        projected,
        DurablePayloadReference::None,
        authority,
    )
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_recovered_decision_terminal_family_fixture(
    context: LifecycleContext,
    certified_sources: Vec<PeerId>,
    seed: u8,
) -> ([ReplayCase; 4], wire::BlockSubject, wire::QuorumCertificate) {
    let fixture = Fixture::for_record(context, seed);
    let locator = RecoveredWalFrameIdentity::for_test(
        80_u64 + u64::from(seed),
        81_u64 + u64::from(seed),
        [seed.wrapping_add(0x51); 32],
    )
    .persisted_locator();
    let fetch = replay_case(
        context,
        LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator,
            role: ReplayWalRoleV1::DECISION,
            tag: fixture.tag,
            action: WalReplayActionV1::FetchDecision {
                certificate: fixture.commit_qc.clone(),
                certified_sources,
            },
        }),
        LifecycleStageKind::FetchBody,
        DurablePayloadReference::None,
    );
    let body_source = LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
        tag: fixture.tag,
        origin: BodyPipelineOriginV1::RecoveredDecision {
            locator,
            certificate: fixture.commit_qc.clone(),
            manifest: fixture.proposal.manifest.clone(),
        },
    });
    let store = replay_case(
        context,
        body_source.clone(),
        LifecycleStageKind::StoreBody,
        fixture.body_payload,
    );
    let validate = replay_case(
        context,
        body_source,
        LifecycleStageKind::ValidateBody,
        fixture.body_payload,
    );
    let certificate = fixture.commit_qc;
    let subject = certificate.subject;
    let apply = replay_case(
        context,
        LifecycleReplaySourceV1::Wal(WalReplaySourceV1 {
            locator,
            role: ReplayWalRoleV1::DECISION,
            tag: fixture.tag,
            action: WalReplayActionV1::ApplyDecision(certificate.clone()),
        }),
        LifecycleStageKind::ApplyDecision,
        fixture.body_payload,
    );
    ([fetch, store, validate, apply], subject, certificate)
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn durable_certified_fetch_projection_fixture(
    context: LifecycleContext,
    causal_root: CausalRoot,
    seed: u8,
) -> DurableCertifiedFetchReplayProjectionV1 {
    let fixture = Fixture::for_record(context, seed);
    let coordinates = CertifiedBodyPipelineCoordinatesV1 {
        tag: fixture.tag,
        certificate: fixture.prepare_qc,
        manifest: fixture.proposal.manifest,
        fetch_manifest_present: true,
        certified_sources: Vec::new(),
    };
    let family = exact_certified_body_pipeline_family(&coordinates, &fixture.body_receipt)
        .expect("canonical fixture binds one body-fsynced Certified Fetch family");
    let effect = exact_certified_fetch_effect(&family)
        .expect("canonical fixture reconstructs one exact Fetch effect");
    let payload = DurablePayloadReference::BodyFrame(
        durable_body_frame_reference(context, &fixture.body_receipt)
            .expect("canonical fixture body belongs to its lifecycle context"),
    );
    let authority = canonical_replay_authority(
        context,
        LifecycleReplaySourceV1::BodyPipeline(family.source),
        LifecycleStageKind::FetchBody,
        ReplayPayloadBindingV1::from_payload(payload),
    )
    .expect("canonical fixture projects one frame-bound Fetch authority");
    let causal_key = Hash::prehashed(*causal_root.digest().as_bytes());
    let effect_identity = crate::sumeragi::v2_runtime::adapter_effect_identity_for_test(&effect);
    let completion_digest = canonical_durable_certified_fetch_completion_digest(
        causal_key,
        effect_identity,
        &authority,
    );
    DurableCertifiedFetchReplayProjectionV1 {
        payload,
        authority,
        causal_key,
        effect_identity,
        completion_digest,
        expected_manifest_hash: fixture.body_receipt.manifest_hash(),
    }
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn durable_certified_fetch_waiting_record_fixture(
    context: LifecycleContext,
    seed: u8,
) -> ReplayCase {
    let fixture = Fixture::for_record(context, seed);
    let coordinates = CertifiedBodyPipelineCoordinatesV1 {
        tag: fixture.tag,
        certificate: fixture.prepare_qc,
        manifest: fixture.proposal.manifest,
        fetch_manifest_present: true,
        certified_sources: Vec::new(),
    };
    let family = exact_certified_body_pipeline_family(&coordinates, &fixture.body_receipt)
        .expect("canonical fixture binds one waiting Certified Fetch family");
    replay_case(
        context,
        LifecycleReplaySourceV1::BodyPipeline(family.source),
        LifecycleStageKind::FetchBody,
        DurablePayloadReference::None,
    )
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_durable_certified_fetch_record_fixture(
    context: LifecycleContext,
    tag: EventTag,
    certificate: wire::QuorumCertificate,
    manifest: wire::PayloadManifest,
    certified_sources: Vec<PeerId>,
    receipt: &DurableBodyReceipt,
) -> ReplayCase {
    let payload = DurablePayloadReference::BodyFrame(
        durable_body_frame_reference(context, receipt)
            .expect("durable Certified Fetch fixture body belongs to its context"),
    );
    replay_case(
        context,
        LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
            tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
            origin: BodyPipelineOriginV1::Certified {
                certificate,
                manifest,
                fetch_manifest_present: true,
                certified_sources,
            },
        }),
        LifecycleStageKind::FetchBody,
        payload,
    )
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn exact_local_body_record_fixture(
    context: LifecycleContext,
    tag: EventTag,
    manifest: wire::PayloadManifest,
    receipt: &DurableBodyReceipt,
    stage: LifecycleStageKind,
) -> Option<ReplayCase> {
    if receipt.context_id() != manifest.round.context_id
        || receipt.round() != manifest.round
        || receipt.subject() != manifest.subject
        || receipt.manifest_hash() != HashOf::new(&manifest)
    {
        return None;
    }
    let payload =
        DurablePayloadReference::BodyFrame(durable_body_frame_reference(context, receipt)?);
    Some(replay_case(
        context,
        LifecycleReplaySourceV1::BodyPipeline(BodyPipelineReplaySourceV1 {
            tag: ReplayEventTagV1::new(tag.height(), tag.view(), tag.generation().get()),
            origin: BodyPipelineOriginV1::LocalBody(manifest),
        }),
        stage,
        payload,
    ))
}
pub(in crate::sumeragi::v2_lifecycle_coordinator) fn foreign_certified_serve_family_authority_fixture(
    context: LifecycleContext,
    stage: LifecycleStageKind,
    seed: u8,
) -> LifecycleReplayAuthorityV1 {
    let case = exact_record_fixture(context, stage, seed);
    let mut authority = case.authority;
    let LifecycleReplaySourceV1::CertifiedServeStorage(source) = &mut authority.source else {
        panic!("Certified-Serve family fixture requires a Serve or ProducerTurn stage")
    };
    source.payload_hash[0] ^= 1;
    assert!(authority.structurally_matches_record(
        context,
        case.key,
        case.work_class,
        case.stage,
        case.payload,
    ));
    authority
}
struct CertifiedServeReplayFixture {
    context: wire::HeightContext,
    active_context: LifecycleContext,
    authenticated: AuthenticatedCertifiedBodyRequest,
}
impl CertifiedServeReplayFixture {
    fn new() -> Self {
        let mut keys = (0x81_u8..=0x84)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("deterministic Certified-Serve replay key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id(
                "certified-serve-replay-authority-test",
            ),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"Certified-Serve replay AMX context"),
            execution_policy_hash: Hash::new(b"Certified-Serve replay execution policy"),
            da_layout: wire::SumeragiV2GenesisContextParameters::recommended().da_layout,
            leader_seed: [0xA7; 32],
        };
        context
            .validate()
            .expect("valid Certified-Serve replay context");
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let request_subject = subject(0x91);
        let mut request = wire::CertifiedBodyRequest {
            round,
            subject: request_subject,
            certificate: wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: request_subject,
                execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                    Hash::new(b"Certified-Serve replay parent state"),
                    Hash::new(b"Certified-Serve replay post state"),
                    Hash::new(b"Certified-Serve replay ordinary writes"),
                    1,
                    Hash::new(b"Certified-Serve replay executed block"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA5; 48],
            },
            requester: PeerId::new(keys[3].public_key().clone()),
            signature: Vec::new(),
        };
        request.signature = Signature::new(keys[3].private_key(), &request.signature_preimage())
            .payload()
            .to_vec();
        let requester = request.requester.clone();
        let authenticated =
            authenticate_certified_body_request(&context, request, &requester, |_, _| {
                Ok::<(), &'static str>(())
            })
            .expect("authenticate Certified-Serve replay request");
        let active_context =
            LifecycleContext::new(digest_from_bytes(context.id().0.as_ref()), context.height);
        Self {
            context,
            active_context,
            authenticated,
        }
    }
    fn pending_payload(&self) -> DurablePayloadReference {
        DurablePayloadReference::CertifiedServePending {
            request: digest_from_bytes(self.authenticated.request_hash().as_ref()),
            certificate: digest_from_bytes(
                HashOf::new(&self.authenticated.request().certificate).as_ref(),
            ),
        }
    }
}
#[cfg(feature = "bls")]
#[derive(Clone, Copy)]
enum RecoveredServeState {
    Pending,
    Completed,
    Negative,
}
#[cfg(feature = "bls")]
struct CertifiedServeRecoveredReplayFixture {
    verified: VerifiedHeightContext,
    keys: Vec<KeyPair>,
    body: Vec<u8>,
    manifest: wire::PayloadManifest,
    authenticated: AuthenticatedCertifiedBodyRequest,
    response: wire::CertifiedBodyResponse,
}
#[cfg(feature = "bls")]
impl CertifiedServeRecoveredReplayFixture {
    #[allow(clippy::too_many_lines)]
    fn new() -> Self {
        let mut keys = (0x91_u8..=0x94)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic recovered Serve replay BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("recovered Serve replay proof of possession")
            })
            .collect::<Vec<_>>();
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id(
                "recovered-certified-serve-replay-test",
            ),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"recovered Serve replay AMX context"),
            execution_policy_hash: Hash::new(b"recovered Serve replay execution policy"),
            da_layout: wire::SumeragiV2GenesisContextParameters::recommended().da_layout,
            leader_seed: [0xB7; 32],
        };
        let verified = VerifiedHeightContext::genesis(context, proofs)
            .expect("verified recovered Serve replay context");
        let context = verified.context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let leader = context.leader(round.view);
        let leader_index = usize::try_from(leader).expect("fixture leader index fits usize");
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero fixture height"),
            None,
            None,
            None,
            1_000,
            round.view,
        );
        let block_signature =
            SignatureOf::try_from_hash(keys[leader_index].private_key(), header.hash())
                .expect("sign recovered Serve replay block");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(leader), block_signature),
            header,
            Vec::new(),
        );
        let body = block.encode_wire().expect("canonical recovered Serve body");
        let request_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&body),
        };
        let chunks = wire::encode_payload_chunks(context.da_layout, &body)
            .expect("encode recovered Serve replay body");
        let manifest = wire::PayloadManifest::derive(
            context,
            round,
            request_subject,
            u64::try_from(body.len()).expect("fixture body length fits u64"),
            &chunks,
        )
        .expect("derive recovered Serve replay manifest");
        let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"recovered Serve replay parent state"),
            Hash::new(b"recovered Serve replay post state"),
            Hash::new(b"recovered Serve replay ordinary writes"),
            1,
            Hash::new(b"recovered Serve replay executed block"),
        );
        let signers = vec![0, 1, 2];
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: request_subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small fixture signer")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate recovered Serve replay PrepareQC");
        let mut request = wire::CertifiedBodyRequest {
            round,
            subject: request_subject,
            certificate: wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject: request_subject,
                execution_commitment,
                signers,
                aggregate_signature,
            },
            requester: PeerId::new(keys[3].public_key().clone()),
            signature: Vec::new(),
        };
        request.signature = Signature::new(keys[3].private_key(), &request.signature_preimage())
            .payload()
            .to_vec();
        let requester = request.requester.clone();
        let authenticated = authenticate_certified_body_request(
            context,
            request,
            &requester,
            |context, certificate| {
                wire::finality::verify_quorum_certificate_with_validator_pops(
                    context,
                    certificate,
                    verified.proofs_of_possession(),
                )
                .map_err(|error| error.to_string())
            },
        )
        .expect("authenticate recovered Serve replay request");
        let mut response = wire::CertifiedBodyResponse {
            request_hash: authenticated.request_hash(),
            manifest: manifest.clone(),
            body: body.clone(),
            responder: 0,
            signature: Vec::new(),
        };
        response.signature = Signature::new(keys[0].private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
        Self {
            verified,
            keys,
            body,
            manifest,
            authenticated,
            response,
        }
    }
    fn replay_pair(&self, state: RecoveredServeState) -> CertifiedServeReplayEvidencePairV1 {
        let temporary = TempDir::new().expect("temporary recovered Serve replay directory");
        let context = self.verified.context();
        let mut body_store = V2BodyStore::open(temporary.path(), context.clone())
            .expect("open recovered Serve body store");
        if matches!(state, RecoveredServeState::Completed) {
            let _body_receipt = body_store
                .store(self.manifest.clone(), self.body.clone())
                .expect("persist recovered Serve body");
        }
        let (mut payload_store, _) = CertifiedServePayloadStoreV1::open(temporary.path(), context)
            .expect("open recovered Serve payload store");
        let pending = payload_store
            .persist_pending_with_verified_retention(
                &self.verified,
                &self.keys[0],
                &self.authenticated,
            )
            .expect("persist verified recovered Serve request");
        match state {
            RecoveredServeState::Pending => {}
            RecoveredServeState::Completed => {
                let _completion_receipt = payload_store
                    .persist_completed(&self.authenticated, &self.response)
                    .expect("persist recovered Serve completion");
            }
            RecoveredServeState::Negative => {
                let _negative_receipt = payload_store
                    .persist_negative(
                        pending.id(),
                        CertifiedServePayloadNegativeOutcome::Rejected(17),
                    )
                    .expect("persist recovered Serve negative outcome");
            }
        }
        drop(payload_store);
        let (_, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), context)
            .expect("reopen recovered Serve payload store");
        let authenticated_recovery = recovery
            .authenticate(&self.verified, &self.keys[0], &body_store)
            .expect("authenticate recovered Serve payload state");
        let recovered = authenticated_recovery
            .get(pending.id())
            .expect("recover exact Serve request");
        assert_eq!(recovered.local_retainer(), 0);
        assert!(recovered.exactly_matches_persisted_payload());
        let active_context =
            LifecycleContext::new(digest_from_bytes(context.id().0.as_ref()), context.height);
        CertifiedServeReplayEvidencePairV1::from_authenticated_recovery(active_context, recovered)
            .expect("reconstruct recovered Serve/Producer replay pair")
    }
}
fn broadcast_case(
    payload: wire::ConsensusMessageV2Payload,
    stage: LifecycleStageKind,
) -> (
    LifecycleReplaySourceV1,
    LifecycleStageKind,
    DurablePayloadReference,
) {
    (
        LifecycleReplaySourceV1::ConsensusBroadcast(wire::ConsensusMessageV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            payload,
        }),
        stage,
        DurablePayloadReference::None,
    )
}
fn replay_case(
    context: LifecycleContext,
    source: LifecycleReplaySourceV1,
    stage_kind: LifecycleStageKind,
    payload: DurablePayloadReference,
) -> ReplayCase {
    let payload_binding = ReplayPayloadBindingV1::from_payload(payload);
    let shape = source
        .project(context, stage_kind, &payload_binding)
        .expect("fixture replay source projects");
    let predecessor = match shape.work_class {
        LifecycleWorkClass::CertifiedServe => PredecessorScope::ReadyOrdinalPrefix,
        LifecycleWorkClass::ProducerTurn => PredecessorScope::ProducerHandoffBarrier,
        _ => PredecessorScope::Independent,
    };
    ReplayCase {
        authority: LifecycleReplayAuthorityV1 {
            format_version: REPLAY_AUTHORITY_FORMAT_VERSION,
            payload: payload_binding,
            source,
        },
        key: shape.key,
        work_class: shape.work_class,
        stage: LifecycleStage::new(stage_kind, predecessor),
        payload,
    }
}
fn pending_binding(
    effect: &AdapterEffect,
    tag: EventTag,
    ordinal: u128,
) -> PendingRuntimeEffectBinding {
    bind_adapter_effect_batch_ownership(
        core::slice::from_ref(effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
    )
    .expect("bind exact direct signed replay fixture")
    .pop()
    .expect("one direct signed replay fixture owner")
    .exact_pending_adapter_effect_binding(effect)
    .expect("mint exact direct signed replay pending binding")
}
fn signed_broadcast_effects(fixture: &Fixture) -> Vec<AdapterEffect> {
    [
        wire::ConsensusMessageV2Payload::Proposal(fixture.proposal.clone()),
        wire::ConsensusMessageV2Payload::Vote(fixture.prepare_vote.clone()),
        wire::ConsensusMessageV2Payload::Vote(fixture.commit_vote.clone()),
        wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.prepare_qc.clone()),
        wire::ConsensusMessageV2Payload::QuorumCertificate(fixture.commit_qc.clone()),
        wire::ConsensusMessageV2Payload::TimeoutVote(fixture.timeout_vote.clone()),
        wire::ConsensusMessageV2Payload::TimeoutCertificate(fixture.timeout_certificate.clone()),
    ]
    .into_iter()
    .map(|payload| AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(payload)))
    .collect()
}
fn subject(marker: u8) -> wire::BlockSubject {
    wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 0xA1])),
        payload_hash: Hash::new([marker, 0xA2]),
    }
}
fn manifest(
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    marker: u8,
) -> wire::PayloadManifest {
    wire::PayloadManifest {
        round,
        subject,
        payload_size_bytes: 1,
        layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1024,
            max_chunk_count: 2,
        },
        chunk_hashes: vec![Hash::new([marker, 0xA3])],
        chunk_root: Hash::new([marker, 0xA4]),
    }
}
