use std::collections::VecDeque;

use crate::sumeragi::v2_core::Generation;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::peer::PeerId;
use iroha_p2p::network::{NetworkReplyRoute, NetworkReplyRouteError, NetworkReplyRouteTestFixture};
use tempfile::TempDir;

use super::*;
use crate::sumeragi::{
    InboundBlockMessage,
    message::BlockMessage,
    v2::{
        AdapterFingerprints, DeferredBodyPipelineStageForTest, SignRequest, VerifiedHeightContext,
    },
    v2_chunks::encode_payload,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FakeCommand {
    record: Option<u8>,
    enter_view: Option<EventTag>,
    fail: bool,
}

impl FakeCommand {
    const fn record(value: u8) -> Self {
        Self {
            record: Some(value),
            enter_view: None,
            fail: false,
        }
    }

    const fn enter_view(tag: EventTag) -> Self {
        Self {
            record: None,
            enter_view: Some(tag),
            fail: false,
        }
    }

    const fn fail() -> Self {
        Self {
            record: None,
            enter_view: None,
            fail: true,
        }
    }
}

impl exact_runtime_command_identity_sealed::Sealed for FakeCommand {}

impl ExactRuntimeCommandIdentity for FakeCommand {
    fn exact_runtime_command_identity(&self) -> RuntimeCommandIdentity {
        let mut identity = Vec::new();
        match self.record {
            Some(value) => {
                identity.push(1);
                identity.push(value);
            }
            None => identity.push(0),
        }
        match self.enter_view {
            Some(tag) => {
                identity.push(1);
                append_runtime_identity_tag(&mut identity, tag);
            }
            None => identity.push(0),
        }
        identity.push(u8::from(self.fail));
        let canonical_hash = iroha_crypto::Hash::new(&identity);
        RuntimeCommandIdentity {
            kind: RuntimeCommandKind::Test,
            canonical_bytes: Arc::from(identity),
            canonical_hash,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FakeEffect {
    enter_view: Option<EventTag>,
    fresh: Option<RuntimeFreshRootKind>,
    semantic: u8,
}

impl FakeEffect {
    const fn other() -> Self {
        Self {
            enter_view: None,
            fresh: None,
            semantic: 0,
        }
    }

    const fn enter_view(tag: EventTag) -> Self {
        Self {
            enter_view: Some(tag),
            fresh: None,
            semantic: 0,
        }
    }

    const fn historical(semantic: u8) -> Self {
        Self {
            enter_view: None,
            fresh: Some(RuntimeFreshRootKind::HistoricalLockedRetransmit),
            semantic,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FakeError;

impl fmt::Display for FakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("fake driver failure")
    }
}

impl std::error::Error for FakeError {}

struct FakeDriver {
    current_tag: EventTag,
    delivered: Vec<(EventTag, u8)>,
    timeouts: Vec<EventTag>,
    retransmits: Vec<EventTag>,
    retry_once: BTreeSet<u8>,
    timer_effects: VecDeque<Vec<FakeEffect>>,
    deferred_effects: VecDeque<Vec<FakeEffect>>,
    deferred_dispatches: usize,
    deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
    deferred_active_ordinals: BTreeSet<u128>,
    deferred_occurrence_ownership: BTreeMap<u128, DeferredOccurrenceOwnershipEvidence>,
    deferred_service_cursor: DeferredPriority,
    deferred_identity_unavailable: bool,
    deferred_evidence_overrides: VecDeque<DeferredServiceEvidence>,
    admission_preflight_override: Option<RuntimeCommandAdmissionPreflight>,
    dormant_local_fifo_reservations: Vec<RuntimeDormantLocalFifoReservation>,
    protected_commit: Option<(
        wire::ConsensusRound,
        wire::BlockSubject,
        wire::ExecutionCommitment,
    )>,
    signature_fence_active: bool,
    signature_fence_identity: u64,
}

impl FakeDriver {
    fn new(tag: EventTag) -> Self {
        Self {
            current_tag: tag,
            delivered: Vec::new(),
            timeouts: Vec::new(),
            retransmits: Vec::new(),
            retry_once: BTreeSet::new(),
            timer_effects: VecDeque::new(),
            deferred_effects: VecDeque::new(),
            deferred_dispatches: 0,
            deferred_admission_ordinals: DeferredAdmissionOrdinalSource::new(0),
            deferred_active_ordinals: BTreeSet::new(),
            deferred_occurrence_ownership: BTreeMap::new(),
            deferred_service_cursor: DeferredPriority::Completion,
            deferred_identity_unavailable: false,
            deferred_evidence_overrides: VecDeque::new(),
            admission_preflight_override: None,
            dormant_local_fifo_reservations: Vec::new(),
            protected_commit: None,
            signature_fence_active: false,
            signature_fence_identity: 1,
        }
    }
}

impl RuntimeDriver for FakeDriver {
    type Command = FakeCommand;
    type Effect = FakeEffect;
    type Error = FakeError;
    type SignatureFenceIdentity = u64;

    fn current_tag(&self) -> EventTag {
        self.current_tag
    }

    fn preflight_command_admission(
        &self,
        _tag: EventTag,
        _command: &Self::Command,
    ) -> RuntimeCommandAdmissionPreflight {
        self.admission_preflight_override
            .unwrap_or(RuntimeCommandAdmissionPreflight::Admit)
    }

    fn dormant_local_fifo_reservations(
        &self,
    ) -> Result<Vec<RuntimeDormantLocalFifoReservation>, String> {
        Ok(self.dormant_local_fifo_reservations.clone())
    }

    fn dispatch(
        &mut self,
        tagged: TaggedCommand<Self::Command>,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
        if tagged.command.fail {
            return Err(FakeError);
        }
        if let Some(tag) = tagged.command.enter_view {
            self.current_tag = tag;
            return Ok(RuntimeDriverDispatch::completed(vec![
                FakeEffect::enter_view(tag),
            ]));
        }
        let value = tagged.command.record.expect("well-formed fake command");
        if self.retry_once.remove(&value) {
            return Ok(RuntimeDriverDispatch {
                effects: Vec::new(),
                deferred_ingress: None,
                deferred_ordinal: None,
                retry_unadmitted: true,
                producer_handoff: None,
                remote_proposal_replay: None,
            });
        }
        self.delivered.push((tagged.tag, value));
        Ok(RuntimeDriverDispatch::completed(vec![FakeEffect::other()]))
    }

    fn timeout_elapsed(
        &mut self,
        tag: EventTag,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
        self.timeouts.push(tag);
        Ok(RuntimeDriverDispatch::completed(
            self.timer_effects.pop_front().unwrap_or_default(),
        ))
    }

    fn retransmit_elapsed(
        &mut self,
        tag: EventTag,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error> {
        self.retransmits.push(tag);
        Ok(RuntimeDriverDispatch::completed(
            self.timer_effects.pop_front().unwrap_or_default(),
        ))
    }

    fn deferred_work_is_serviceable(&self) -> bool {
        !self.deferred_effects.is_empty()
    }

    fn signature_fence_is_active(&self) -> bool {
        self.signature_fence_active
    }

    fn signature_fence_identity(
        &self,
    ) -> Result<Option<Self::SignatureFenceIdentity>, Self::Error> {
        Ok(self
            .signature_fence_active
            .then_some(self.signature_fence_identity))
    }

    fn deferred_admission_ordinal_source(&self) -> &DeferredAdmissionOrdinalSource {
        &self.deferred_admission_ordinals
    }

    fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
        BTreeSet::new()
    }

    fn all_deferred_admission_ordinals(&self) -> BTreeSet<u128> {
        self.deferred_active_ordinals.clone()
    }

    fn deferred_occurrence_ownership(
        &self,
        admission_ordinal: u128,
    ) -> Option<DeferredOccurrenceOwnershipEvidence> {
        self.deferred_occurrence_ownership
            .get(&admission_ordinal)
            .cloned()
    }

    fn synthetic_deferred_lifecycle_owner(
        &self,
        evidence: &DeferredServiceEvidence,
    ) -> Option<RuntimeLifecycleOwner> {
        let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            evidence.original_tag,
            CommandClass::Completion,
            RuntimeFreshRootKind::StartupRecovery,
            b"fake-deferred-owner",
        );
        let lifecycle_ordinal = evidence.admission_ordinal.checked_add(1)?;
        RuntimeLifecycleOwner::new(origin, lifecycle_ordinal).ok()
    }

    fn dispatch_deferred(
        &mut self,
        _eligible: &BTreeSet<u128>,
    ) -> Result<
        Option<(
            Vec<Self::Effect>,
            DeferredServiceEvidence,
            Option<ProducerContinuationHandoffToken>,
        )>,
        Self::Error,
    > {
        self.deferred_dispatches = self.deferred_dispatches.saturating_add(1);
        let before = u64::try_from(self.deferred_effects.len())
            .expect("bounded fake deferred queue length fits u64");
        let effects = self.deferred_effects.pop_front().unwrap_or_default();
        if self.deferred_identity_unavailable {
            return Ok(None);
        }
        let evidence = match self.deferred_evidence_overrides.pop_front() {
            Some(evidence) => evidence,
            None => {
                let evidence = DeferredServiceEvidence::completion_for_test(
                    &self.deferred_admission_ordinals,
                    self.current_tag,
                    before,
                    self.deferred_service_cursor,
                );
                assert!(evidence.claim_adapter_service_for_test());
                evidence
            }
        };
        self.deferred_service_cursor = evidence.service_cursor_after;
        Ok(Some((effects, evidence, None)))
    }

    fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag> {
        effect.enter_view
    }

    fn effect_causality(
        effect: &Self::Effect,
        _source: RuntimeEffectSource,
    ) -> RuntimeEffectCausality {
        effect.fresh.map_or(
            RuntimeEffectCausality::Inherit,
            RuntimeEffectCausality::Fresh,
        )
    }

    fn fresh_effect_semantic_identity(
        effect: &Self::Effect,
        kind: RuntimeFreshRootKind,
    ) -> Vec<u8> {
        vec![kind.code(), effect.semantic]
    }

    fn effect_root_tag(_effect: &Self::Effect) -> Option<EventTag> {
        None
    }

    fn wire_ingress_may_use_progress(&self, payload: &wire::ConsensusMessageV2Payload) -> bool {
        matches!(
            (payload, self.protected_commit),
            (
                wire::ConsensusMessageV2Payload::Vote(vote),
                Some((round, subject, execution_commitment))
            ) if vote.phase == wire::GlobalPhase::Commit
                && vote.round == round
                && vote.subject == subject
                && vote.execution_commitment == execution_commitment
        )
    }
}

fn tag(view: u64) -> EventTag {
    EventTag::new(7, view, Generation::new(view + 11))
}

fn authenticated_proposal_for_test(
    manifest: wire::PayloadManifest,
) -> AuthenticatedConsensusMessage {
    AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
            round: manifest.round,
            proposer: 0,
            subject: manifest.subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: vec![1],
        }),
    ))
}

fn authenticated_runtime_context() -> (wire::HeightContext, Vec<KeyPair>) {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic runtime ingress key")
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
        network_id: crate::sumeragi::synthetic_network_id("sumeragi-v2-runtime-ingress-test"),
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: 1,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: wire::DualQuorum::from_roster(&roster).expect("runtime fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"runtime ingress nexus context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 512 * 1024,
            max_chunk_count: 1024,
        },
        leader_seed: [0x5A; 32],
    };
    (context, keys)
}

fn signed_runtime_proposal(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    marker: u8,
) -> wire::ConsensusMessageV2 {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let body = vec![marker; 4];
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
        payload_hash: Hash::new(&body),
    };
    let manifest = encode_payload(context, round, subject, &body)
        .expect("encode valid runtime fixture payload")
        .manifest()
        .clone();
    let proposer = context.leader(round.view);
    let mut proposal = wire::Proposal {
        round,
        proposer,
        subject,
        manifest,
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    proposal.signature = Signature::new(
        keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
        &proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal))
}

fn signed_runtime_vote(
    keys: &[KeyPair],
    round: wire::ConsensusRound,
    phase: wire::GlobalPhase,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
) -> wire::ConsensusMessageV2 {
    let mut vote = wire::Vote {
        round,
        proposal_round: round,
        phase,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(
        keys[usize::try_from(vote.signer).expect("small signer index")].private_key(),
        &vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote))
}

fn signed_runtime_timeout_vote(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    view: u64,
    signer: u32,
) -> wire::ConsensusMessageV2 {
    signed_runtime_timeout_vote_with_highest_prepare_qc(context, keys, view, signer, None)
}

fn signed_runtime_timeout_vote_with_highest_prepare_qc(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    view: u64,
    signer: u32,
    highest_prepare_qc: Option<wire::QuorumCertificate>,
) -> wire::ConsensusMessageV2 {
    let mut vote = wire::TimeoutVote {
        round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        },
        highest_prepare_qc,
        signer,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(
        keys[usize::try_from(signer).expect("small signer index")].private_key(),
        &vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(vote))
}

fn fair_runtime_ownership(
    message: &wire::ConsensusMessageV2,
    semantic_origin: PeerId,
    authenticated_via: PeerId,
) -> FairV2IngressOwnershipEvidence {
    let mut inbound =
        super::super::fair_v2_ingress_admit_for_test(InboundBlockMessage::from_transport(
            BlockMessage::V2(message.clone()),
            semantic_origin,
            authenticated_via,
        ));
    inbound
        .take_ingress_ownership()
        .expect("real fair ingress attaches exact ownership")
}

fn fair_runtime_ownership_at_lifecycle(
    mut ownership: FairV2IngressOwnershipEvidence,
    lifecycle_ordinal: u128,
) -> FairV2IngressOwnershipEvidence {
    ownership.first.lifecycle_ordinal = Some(lifecycle_ordinal);
    ownership.latest.lifecycle_ordinal = Some(lifecycle_ordinal);
    assert!(
        ownership.validate_exact(),
        "test lifecycle projection must preserve exact fair ownership"
    );
    ownership
}

fn fair_runtime_ownership_with_reply_route(
    message: &wire::ConsensusMessageV2,
    semantic_origin: PeerId,
    authenticated_via: PeerId,
    reply_route: NetworkReplyRoute,
) -> FairV2IngressOwnershipEvidence {
    let mut inbound = super::super::fair_v2_ingress_admit_for_test(
        InboundBlockMessage::try_from_transport_with_reply_route(
            BlockMessage::V2(message.clone()),
            semantic_origin,
            authenticated_via,
            reply_route,
        )
        .expect("test transport identities bind the reply capability"),
    );
    inbound
        .take_ingress_ownership()
        .expect("real fair ingress attaches route ownership")
}

fn signed_runtime_quorum_certificate(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    marker: u8,
) -> wire::QuorumCertificate {
    signed_runtime_quorum_certificate_for_phase(context, keys, marker, wire::GlobalPhase::Commit)
}

fn signed_runtime_quorum_certificate_for_phase(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    marker: u8,
    phase: wire::GlobalPhase,
) -> wire::QuorumCertificate {
    signed_runtime_quorum_certificate_for_phase_at_view(context, keys, marker, phase, 0)
}

fn signed_runtime_quorum_certificate_for_phase_at_view(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    marker: u8,
    phase: wire::GlobalPhase,
    view: u64,
) -> wire::QuorumCertificate {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 5])),
        payload_hash: Hash::new([marker, 6]),
    };
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new([marker, 7]),
        Hash::new([marker, 8]),
        Hash::new([marker, 9]),
        1,
        Hash::new([marker, 10]),
    );
    let signers = vec![0, 1, 2];
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase,
        subject,
        execution_commitment,
        signer: signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase,
        subject,
        execution_commitment,
        signers,
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
            .expect("aggregate runtime fixture certificate"),
    }
}

fn signed_runtime_timeout_certificate(
    context: &wire::HeightContext,
    keys: &[KeyPair],
) -> wire::TimeoutCertificate {
    signed_runtime_timeout_certificate_for_view(context, keys, 0)
}

fn signed_runtime_timeout_certificate_for_view(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    view: u64,
) -> wire::TimeoutCertificate {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    };
    let signers = vec![0, 1, 2];
    let preimage = wire::TimeoutVote {
        round,
        highest_prepare_qc: None,
        signer: signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate runtime fixture timeout certificate"),
        }],
    }
}

fn runtime_manifest(context: &wire::HeightContext, marker: u8) -> wire::PayloadManifest {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let body = vec![marker; 4];
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 3])),
        payload_hash: Hash::new(&body),
    };
    encode_payload(context, round, subject, &body)
        .expect("encode valid runtime manifest payload")
        .manifest()
        .clone()
}

#[test]
fn pending_certified_fetch_derives_exact_ordinal_free_body_successors() {
    let (context, keys) = authenticated_runtime_context();
    let manifest = runtime_manifest(&context, 0x68);
    let tag = EventTag::new(context.height, 0, Generation::new(1));
    let mut certificate = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x69,
        wire::GlobalPhase::Prepare,
    );
    certificate.subject = manifest.subject;
    let preimage = wire::Vote {
        round: certificate.round,
        proposal_round: certificate.proposal_round,
        phase: certificate.phase,
        subject: certificate.subject,
        execution_commitment: certificate.execution_commitment,
        signer: certificate.signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = certificate
        .signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("small signer index")].private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate exact Fetch certificate");
    certificate
        .validate(&context)
        .expect("exact Fetch certificate is authenticated");

    let fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate),
    };
    let bound = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 71)],
    )
    .expect("bind one exact certified Fetch");
    let pending = bound[0]
        .pending_adapter_effect_binding(&fetch)
        .expect("certified Fetch mints one pending binding");
    let store = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let successor = pending
        .project_certified_fetch_store_successor(&fetch, &store)
        .expect("certified Fetch derives one exact Store successor");
    assert!(successor.exactly_binds_adapter_effect(&store));
    assert_eq!(
        successor.causal_lifecycle_key(),
        pending.causal_lifecycle_key(),
        "the coordinator causal owner survives the direct completion handoff",
    );
    assert_eq!(
        successor.candidate_statement(),
        pending.candidate_statement()
    );
    assert_ne!(
        successor.exact_effect_identity(),
        pending.exact_effect_identity()
    );

    let validate = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let validate_successor = successor
        .project_store_validate_successor(&store, &validate)
        .expect("exact Store derives one sealed Validate successor");
    assert!(validate_successor.exactly_binds_adapter_effect(&validate));
    assert_eq!(
        validate_successor.causal_lifecycle_key(),
        successor.causal_lifecycle_key(),
        "the coordinator causal owner survives the durable-body handoff",
    );
    assert_eq!(
        validate_successor.candidate_statement(),
        successor.candidate_statement(),
        "Validate inherits the complete certified body statement",
    );
    assert_ne!(
        validate_successor.exact_effect_identity(),
        successor.exact_effect_identity()
    );
    assert_ne!(
        validate_successor.projection_hash, successor.projection_hash,
        "the concrete successor receives a new integrity projection",
    );
    assert!(!successor.exactly_binds_adapter_effect(&validate));
    assert!(!validate_successor.exactly_binds_adapter_effect(&store));

    let wrong_tag = AdapterEffect::StoreBody {
        tag: EventTag::new(context.height, 0, Generation::new(2)),
        round: manifest.round,
        subject: manifest.subject,
    };
    assert!(
        pending
            .project_certified_fetch_store_successor(&fetch, &wrong_tag)
            .is_none()
    );
    let ordinary_fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    assert!(
        pending
            .project_certified_fetch_store_successor(&ordinary_fetch, &store)
            .is_none()
    );

    let wrong_validate_tag = AdapterEffect::ValidateBody {
        tag: EventTag::new(context.height, 0, Generation::new(2)),
        round: manifest.round,
        subject: manifest.subject,
    };
    assert!(
        successor
            .project_store_validate_successor(&store, &wrong_validate_tag)
            .is_none()
    );
    assert!(
        successor
            .project_store_validate_successor(&validate, &validate)
            .is_none(),
        "Validate cannot stand in for the exact Store predecessor"
    );
    assert!(
        successor
            .project_store_validate_successor(&store, &store)
            .is_none(),
        "Store cannot stand in for the exact Validate successor"
    );
    assert!(
        validate_successor
            .project_store_validate_successor(&store, &validate)
            .is_none(),
        "the projected successor cannot duplicate predecessor authority"
    );
}

#[test]
fn pending_validate_projects_only_the_exact_commit_authorized_apply_successor() {
    let (context, keys) = authenticated_runtime_context();
    let commit = signed_runtime_quorum_certificate(&context, &keys, 0x6A);
    let tag = EventTag::new(context.height, commit.round.view, Generation::new(2));
    let store = AdapterEffect::StoreBody {
        tag,
        round: commit.proposal_round,
        subject: commit.subject,
    };
    let validate = AdapterEffect::ValidateBody {
        tag,
        round: commit.proposal_round,
        subject: commit.subject,
    };
    let apply = AdapterEffect::Apply {
        tag,
        subject: commit.subject,
        certificate: commit.clone(),
    };

    let local_store = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&store),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 72)],
    )
    .expect("bind one ordinary Store")
    .pop()
    .expect("one ordinary Store owner")
    .pending_adapter_effect_binding(&store)
    .expect("ordinary Store mints one pending binding");
    let local_validate = local_store
        .project_store_validate_successor(&store, &validate)
        .expect("ordinary Store derives one Validate successor");
    let local_apply = local_validate
        .project_validate_apply_successor(&validate, &apply)
        .expect("a durable CommitQC refines ordinary validation to Apply");
    assert!(local_apply.exactly_binds_adapter_effect(&apply));
    assert_eq!(
        local_apply.causal_lifecycle_key(),
        local_validate.causal_lifecycle_key()
    );
    assert_eq!(
        local_apply
            .candidate_statement()
            .expect("Apply carries its exact candidate statement")
            .phase(),
        Some(wire::GlobalPhase::Commit)
    );
    assert_ne!(
        local_apply.exact_effect_identity(),
        local_validate.exact_effect_identity()
    );

    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6A,
        wire::GlobalPhase::Prepare,
    );
    let fetch = AdapterEffect::FetchBody {
        tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: None,
        certified_sources: Vec::new(),
        certificate: Some(prepare),
    };
    let prepare_fetch = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 73)],
    )
    .expect("bind one Prepare-certified Fetch")
    .pop()
    .expect("one Prepare-certified Fetch owner")
    .pending_adapter_effect_binding(&fetch)
    .expect("Prepare-certified Fetch mints one pending binding");
    let prepare_store = prepare_fetch
        .project_certified_fetch_store_successor(&fetch, &store)
        .expect("Prepare-certified Fetch derives Store");
    let prepare_validate = prepare_store
        .project_store_validate_successor(&store, &validate)
        .expect("Prepare-certified Store derives Validate");
    let prepare_apply = prepare_validate
        .project_validate_apply_successor(&validate, &apply)
        .expect("matching CommitQC promotes Prepare validation to Apply");
    assert!(prepare_apply.exactly_binds_adapter_effect(&apply));
    assert_eq!(
        prepare_apply.candidate_statement(),
        local_apply.candidate_statement()
    );

    let wrong_tag_apply = AdapterEffect::Apply {
        tag: EventTag::new(context.height, commit.round.view, Generation::new(3)),
        subject: commit.subject,
        certificate: commit.clone(),
    };
    assert!(
        prepare_validate
            .project_validate_apply_successor(&validate, &wrong_tag_apply)
            .is_none()
    );
    let prepare_apply_effect = AdapterEffect::Apply {
        tag,
        subject: commit.subject,
        certificate: signed_runtime_quorum_certificate_for_phase(
            &context,
            &keys,
            0x6A,
            wire::GlobalPhase::Prepare,
        ),
    };
    assert!(
        prepare_validate
            .project_validate_apply_successor(&validate, &prepare_apply_effect)
            .is_none(),
        "Apply must carry Commit rather than Prepare authority"
    );
    let mut changed_commitment = commit;
    changed_commitment.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"foreign Validate-to-Apply parent state"),
            Hash::new(b"foreign Validate-to-Apply post state"),
            Hash::new(b"foreign Validate-to-Apply writes"),
            1,
            Hash::new(b"foreign Validate-to-Apply executed block"),
        );
    let changed_apply = AdapterEffect::Apply {
        tag,
        subject: changed_commitment.subject,
        certificate: changed_commitment,
    };
    assert!(
        prepare_validate
            .project_validate_apply_successor(&validate, &changed_apply)
            .is_none(),
        "Prepare authority cannot change its inherited execution commitment"
    );
    assert!(
        prepare_validate
            .project_validate_apply_successor(&store, &apply)
            .is_none(),
        "Store cannot stand in for the exact Validate predecessor"
    );
    assert!(
        prepare_apply
            .project_validate_apply_successor(&validate, &apply)
            .is_none(),
        "the projected Apply binding cannot duplicate predecessor authority"
    );
}

#[test]
fn adapter_effect_binding_is_exact_route_neutral_and_three_bounded() {
    let (context, keys) = authenticated_runtime_context();
    let manifest = runtime_manifest(&context, 0x6A);
    let tag = EventTag::new(context.height, 0, Generation::new(1));
    let store = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    assert_eq!(
        production_adapter_effect_kind(&store),
        RUNTIME_EFFECT_KIND_STORE_BODY
    );
    assert_eq!(
        production_adapter_effect_candidate_semantic_identity(&store)
            .expect("StoreBody is a causal candidate")
            .0,
        RUNTIME_CANDIDATE_KIND_STORE_BODY
    );

    let owner = RuntimeEffectOwnership::fresh_for_test(tag, 71);
    let bound = bind_adapter_effect_batch_ownership(&[store.clone()], vec![owner])
        .expect("one exact StoreBody candidate is within the bound");
    assert!(bound[0].validate_bound_exact());
    let pending = bound[0]
        .pending_adapter_effect_binding(&store)
        .expect("exact bound effect mints one pending binding");
    assert!(pending.exactly_binds_adapter_effect(&store));
    let different_legacy_ordinal = bind_adapter_effect_batch_ownership(
        &[store.clone()],
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 72)],
    )
    .expect("same effect remains bindable under a different legacy ordinal");
    let mut different_pending = different_legacy_ordinal[0]
        .pending_adapter_effect_binding(&store)
        .expect("different legacy owner mints one pending binding");
    assert_eq!(
        pending, different_pending,
        "pending admission authority deliberately excludes the legacy logical ordinal"
    );
    let exact_effect_kind = different_pending.effect_kind;
    different_pending.effect_kind = 0;
    assert!(!different_pending.exactly_binds_adapter_effect(&store));
    different_pending.effect_kind = exact_effect_kind;
    let exact_candidate_kind = different_pending.candidate_kind;
    different_pending.candidate_kind = RUNTIME_CANDIDATE_KIND_NONE;
    assert!(!different_pending.exactly_binds_adapter_effect(&store));
    different_pending.candidate_kind = exact_candidate_kind;
    let exact_projection_hash = different_pending.projection_hash;
    different_pending.projection_hash = Hash::new(b"mutated pending projection");
    assert!(!different_pending.exactly_binds_adapter_effect(&store));
    different_pending.projection_hash = exact_projection_hash;
    assert!(different_pending.exactly_binds_adapter_effect(&store));
    let first_owner_projection = production_adapter_effect_candidate_trace_projection(
        &store, &bound[0], 1, 1, 1, 1, 0, 1, true,
    )
    .expect("recompute lossless first-owner projection");
    assert!(check_production_effect_to_candidate_transition(first_owner_projection).is_some());
    assert!(first_owner_projection.candidate_owner_admitted);
    assert_eq!(
        production_adapter_effect_candidate_admission_disposition(&store, 0, 1),
        Ok(RuntimeCandidateAdmissionDisposition::FirstAdmission)
    );

    let retry_owner = RuntimeEffectOwnership::fresh_for_test(tag, 71);
    let retry = bind_adapter_effect_batch_ownership(&[store.clone()], vec![retry_owner])
        .expect("same exact producer retry remains bindable");
    assert_eq!(bound[0].candidate_identity(), retry[0].candidate_identity());
    let retry_projection = production_adapter_effect_candidate_trace_projection(
        &store, &retry[0], 1, 1, 1, 1, 1, 1, true,
    )
    .expect("recompute coalesced retry projection");
    assert!(check_production_effect_to_candidate_transition(retry_projection).is_some());
    assert!(!retry_projection.candidate_owner_admitted);
    assert_eq!(
        production_adapter_effect_candidate_admission_disposition(&store, 1, 1),
        Ok(RuntimeCandidateAdmissionDisposition::CoalescedRetry)
    );

    let diagnostic = AdapterEffect::ReportInvalidCertifiedBody {
        subject: manifest.subject,
        certificate: signed_runtime_quorum_certificate(&context, &keys, 0x6B),
    };
    assert_eq!(
        production_adapter_effect_candidate_admission_disposition(&diagnostic, 0, 0),
        Ok(RuntimeCandidateAdmissionDisposition::NonCandidate)
    );
    for invalid in [(0, 0), (1, 0), (0, 2), (2, 1)] {
        assert!(
                production_adapter_effect_candidate_admission_disposition(
                    &store, invalid.0, invalid.1,
                )
                .is_err(),
                "candidate count mutation {invalid:?} must fail closed"
            );
    }
    assert!(
        production_adapter_effect_candidate_admission_disposition(&diagnostic, 0, 1).is_err(),
        "a non-candidate cannot mint an owner"
    );

    let changed_tag = EventTag::new(context.height, 0, Generation::new(2));
    let changed = AdapterEffect::StoreBody {
        tag: changed_tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    assert!(
        !pending.exactly_binds_adapter_effect(&changed),
        "the ordinal-free binding still retains the complete physical effect identity"
    );
    assert_ne!(
        production_adapter_effect_semantic_identity(&store),
        production_adapter_effect_semantic_identity(&changed)
    );
    assert_eq!(
        production_adapter_effect_candidate_semantic_identity(&store),
        production_adapter_effect_candidate_semantic_identity(&changed),
        "process-local generation is absent from abstract candidate identity"
    );

    let changed_subject = AdapterEffect::StoreBody {
        tag: changed_tag,
        round: manifest.round,
        subject: wire::BlockSubject {
            payload_hash: Hash::new(b"changed candidate payload"),
            ..manifest.subject
        },
    };
    assert_ne!(
        production_adapter_effect_candidate_semantic_identity(&store),
        production_adapter_effect_candidate_semantic_identity(&changed_subject),
        "the immutable subject remains part of abstract candidate identity"
    );

    let sources = keys[..2]
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let mut reversed_sources = sources.clone();
    reversed_sources.reverse();
    let fetch = |certified_sources| AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources,
        certificate: None,
    };
    let first_route = fetch(sources);
    let second_route = fetch(reversed_sources);
    assert_ne!(
        production_adapter_effect_semantic_identity(&first_route),
        production_adapter_effect_semantic_identity(&second_route),
        "the exact transport effect includes ordered destinations"
    );
    assert_eq!(
        production_adapter_effect_candidate_semantic_identity(&first_route),
        production_adapter_effect_candidate_semantic_identity(&second_route),
        "transport retries retain one route-neutral abstract candidate"
    );

    let certificate = signed_runtime_quorum_certificate(&context, &keys, 0x73);
    let apply = AdapterEffect::Apply {
        tag,
        subject: certificate.subject,
        certificate: certificate.clone(),
    };
    let mut alternate_carrier = certificate.clone();
    alternate_carrier.signers.reverse();
    alternate_carrier.aggregate_signature = vec![0xA7; 96];
    let alternate_apply = AdapterEffect::Apply {
        tag: changed_tag,
        subject: alternate_carrier.subject,
        certificate: alternate_carrier,
    };
    assert_ne!(
        production_adapter_effect_semantic_identity(&apply),
        production_adapter_effect_semantic_identity(&alternate_apply),
        "concrete effect identity retains signer and aggregate carriers"
    );
    assert_eq!(
        production_adapter_effect_candidate_semantic_identity(&apply),
        production_adapter_effect_candidate_semantic_identity(&alternate_apply),
        "candidate identity excludes aggregate, signer, and local-incarnation carriers"
    );

    let protected_lock = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x74,
        wire::GlobalPhase::Prepare,
    );
    let timeout = signed_runtime_timeout_certificate(&context, &keys);
    let enter_tag = EventTag::new(context.height, 1, Generation::new(2));
    let enter_view = |protected_lock| AdapterEffect::EnterView {
        tag: enter_tag,
        certificate: timeout.clone(),
        protected_lock: Some(protected_lock),
    };
    let exact_enter = enter_view(protected_lock.clone());
    let mut alternate_lock_carrier = protected_lock.clone();
    alternate_lock_carrier.signers.reverse();
    alternate_lock_carrier.aggregate_signature = vec![0xA8; 96];
    let alternate_carrier_enter = enter_view(alternate_lock_carrier);
    assert_ne!(
        production_adapter_effect_semantic_identity(&exact_enter),
        production_adapter_effect_semantic_identity(&alternate_carrier_enter),
        "exact EnterView identity retains the authenticated QC carrier"
    );
    assert_eq!(
        <SumeragiV2Adapter as RuntimeDriver>::fresh_effect_semantic_identity(
            &exact_enter,
            RuntimeFreshRootKind::StartupRecovery,
        ),
        <SumeragiV2Adapter as RuntimeDriver>::fresh_effect_semantic_identity(
            &alternate_carrier_enter,
            RuntimeFreshRootKind::StartupRecovery,
        ),
        "fresh EnterView identity normalizes interchangeable QC carriers"
    );

    let mut conflicting_lock = protected_lock;
    conflicting_lock.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"conflicting EnterView parent state"),
            Hash::new(b"conflicting EnterView post state"),
            Hash::new(b"conflicting EnterView ordinary writes"),
            1,
            Hash::new(b"conflicting EnterView executed block"),
        );
    let conflicting_enter = enter_view(conflicting_lock);
    assert_ne!(
        production_adapter_effect_semantic_identity(&exact_enter),
        production_adapter_effect_semantic_identity(&conflicting_enter),
        "exact EnterView identity retains the protected execution commitment"
    );
    assert_ne!(
        <SumeragiV2Adapter as RuntimeDriver>::fresh_effect_semantic_identity(
            &exact_enter,
            RuntimeFreshRootKind::StartupRecovery,
        ),
        <SumeragiV2Adapter as RuntimeDriver>::fresh_effect_semantic_identity(
            &conflicting_enter,
            RuntimeFreshRootKind::StartupRecovery,
        ),
        "fresh EnterView identity retains the protected lock statement"
    );

    let first_vote = wire::Vote {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment: certificate.execution_commitment,
        signer: 0,
        signature: vec![0xB1; 96],
    };
    let mut second_vote = first_vote.clone();
    second_vote.subject = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"conflicting diagnostic block")),
        payload_hash: Hash::new(b"conflicting diagnostic payload"),
        ..first_vote.subject
    };
    second_vote.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"conflicting diagnostic parent state"),
        Hash::new(b"conflicting diagnostic post state"),
        Hash::new(b"conflicting diagnostic ordinary writes"),
        1,
        Hash::new(b"conflicting diagnostic executed block"),
    );
    second_vote.signature = vec![0xB2; 96];
    let diagnostic = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(
            first_vote.clone(),
            second_vote.clone(),
        ),
    };
    let reversed_diagnostic = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(
            second_vote.clone(),
            first_vote.clone(),
        ),
    };
    assert_ne!(
        production_adapter_effect_semantic_identity(&diagnostic),
        production_adapter_effect_semantic_identity(&reversed_diagnostic),
        "exact diagnostic identity retains signed artifact observation order"
    );
    assert_eq!(
        <SumeragiV2Adapter as RuntimeDriver>::fresh_effect_semantic_identity(
            &diagnostic,
            RuntimeFreshRootKind::StartupRecovery,
        ),
        <SumeragiV2Adapter as RuntimeDriver>::fresh_effect_semantic_identity(
            &reversed_diagnostic,
            RuntimeFreshRootKind::StartupRecovery,
        ),
        "logical diagnostic identity canonicalizes the unsigned statement pair"
    );

    let mut resigned_first = first_vote;
    resigned_first.signature = vec![0xB3; 96];
    let resigned_diagnostic = AdapterEffect::ReportEquivocation {
        evidence: AdapterEquivocationEvidence::vote_for_test(resigned_first, second_vote),
    };
    assert_ne!(
        production_adapter_effect_semantic_identity(&diagnostic),
        production_adapter_effect_semantic_identity(&resigned_diagnostic),
        "exact diagnostic identity retains complete signed artifacts"
    );
    assert_eq!(
        <SumeragiV2Adapter as RuntimeDriver>::fresh_effect_semantic_identity(
            &diagnostic,
            RuntimeFreshRootKind::StartupRecovery,
        ),
        <SumeragiV2Adapter as RuntimeDriver>::fresh_effect_semantic_identity(
            &resigned_diagnostic,
            RuntimeFreshRootKind::StartupRecovery,
        ),
        "logical diagnostic identity excludes signature carrier bytes"
    );

    let mut changed_statement = certificate;
    changed_statement.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"changed candidate parent state"),
            Hash::new(b"changed candidate post state"),
            Hash::new(b"changed candidate ordinary writes"),
            1,
            Hash::new(b"changed candidate block wire"),
        );
    let changed_apply = AdapterEffect::Apply {
        tag,
        subject: changed_statement.subject,
        certificate: changed_statement,
    };
    assert_ne!(
        production_adapter_effect_candidate_semantic_identity(&apply),
        production_adapter_effect_candidate_semantic_identity(&changed_apply),
        "execution commitment remains part of the normalized statement"
    );

    let three_candidates = vec![store.clone(), first_route.clone(), apply.clone()];
    let three_owners = (1_u128..=3)
        .map(|ordinal| RuntimeEffectOwnership::fresh_for_test(tag, 90 + ordinal))
        .collect();
    let three_bound = bind_adapter_effect_batch_ownership(&three_candidates, three_owners)
        .expect("exactly three causal successors remain within the bound");
    assert_eq!(three_bound.len(), 3);
    for (index, (effect, ownership)) in three_candidates.iter().zip(&three_bound).enumerate() {
        let position = u8::try_from(index + 1).expect("three positions fit in u8");
        assert!(ownership.validate_bound_exact());
        let projection = production_adapter_effect_candidate_trace_projection(
            effect, ownership, position, 3, position, 3, 0, 1, true,
        )
        .expect("recompute one of three exact first-admission projections");
        assert!(check_production_effect_to_candidate_transition(projection).is_some());
        assert!(projection.candidate_owner_admitted);
    }

    let four_candidates = vec![store.clone(), store.clone(), store.clone(), store.clone()];
    let four_owners = (1_u128..=4)
        .map(|ordinal| RuntimeEffectOwnership::fresh_for_test(tag, 100 + ordinal))
        .collect();
    assert!(
        bind_adapter_effect_batch_ownership(&four_candidates, four_owners).is_err(),
        "a fourth causal successor must fail before retention"
    );

    let mut forged = bound[0].clone();
    forged
        .binding
        .as_mut()
        .expect("bound ownership has positional evidence")
        .effect_position = 2;
    assert!(!forged.validate_exact());
    assert!(
        production_adapter_effect_candidate_trace_projection(
            &store, &forged, 1, 1, 1, 1, 0, 1, true,
        )
        .is_err(),
        "positional binding mutation must fail before projection"
    );
}

#[test]
fn certified_body_pipeline_retains_statement_and_owner_across_stage_kinds() {
    let (context, keys) = authenticated_runtime_context();
    let certificate = signed_runtime_quorum_certificate(&context, &keys, 0x74);
    let tag = EventTag::new(context.height, certificate.round.view, Generation::new(3));
    let fetch = AdapterEffect::FetchBody {
        tag,
        round: certificate.proposal_round,
        subject: certificate.subject,
        manifest: None,
        certified_sources: Vec::new(),
        certificate: Some(certificate.clone()),
    };
    let fetch_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 4_001)],
    )
    .expect("certified Fetch binds its complete statement")
    .pop()
    .expect("one Fetch owner");
    let statement = fetch_ownership
        .candidate_semantic_statement()
        .expect("production Fetch carries typed statement evidence");
    assert_eq!(statement.phase, Some(wire::GlobalPhase::Commit));
    assert_eq!(
        statement.execution_commitment,
        Some(certificate.execution_commitment)
    );

    let store = AdapterEffect::StoreBody {
        tag,
        round: certificate.proposal_round,
        subject: certificate.subject,
    };
    let store_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&store)
        .expect("Store inherits the certified Fetch statement");
    let validate = AdapterEffect::ValidateBody {
        tag,
        round: certificate.proposal_round,
        subject: certificate.subject,
    };
    let validate_ownership = store_ownership
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("Validate inherits the certified Store statement");
    let apply = AdapterEffect::Apply {
        tag,
        subject: certificate.subject,
        certificate: certificate.clone(),
    };
    let apply_ownership = validate_ownership
        .rebind_as_inherited_adapter_effect(&apply)
        .expect("Apply retains the exact certified body authority");

    for ownership in [&store_ownership, &validate_ownership, &apply_ownership] {
        assert_eq!(ownership.owner(), fetch_ownership.owner());
        assert_eq!(ownership.candidate_semantic_statement(), Some(statement));
    }
    let stage_identities = [
        fetch_ownership.candidate_semantic_identity(),
        store_ownership.candidate_semantic_identity(),
        validate_ownership.candidate_semantic_identity(),
        apply_ownership.candidate_semantic_identity(),
    ];
    assert!(stage_identities.iter().all(Option::is_some));
    assert_eq!(
        stage_identities
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len(),
        stage_identities.len(),
        "the outer work kind distinguishes stage occurrences without replacing the owner"
    );

    let mut lost_phase_and_commitment = store_ownership.clone();
    let fresh_store = production_adapter_effect_candidate_statement(&store)
        .expect("Store is a candidate")
        .1;
    lost_phase_and_commitment
        .binding
        .as_mut()
        .expect("Store has exact binding")
        .candidate_statement = Some(fresh_store);
    assert!(
        !lost_phase_and_commitment.validate_exact(),
        "dropping inherited phase and commitment invalidates the sealed binding"
    );

    let wrong_round = wire::ConsensusRound {
        view: certificate.proposal_round.view + 1,
        ..certificate.proposal_round
    };
    let wrong_store = AdapterEffect::StoreBody {
        tag,
        round: wrong_round,
        subject: certificate.subject,
    };
    assert!(
        fetch_ownership
            .rebind_as_inherited_adapter_effect(&wrong_store)
            .is_err(),
        "a causal Store cannot drop or replace the frozen proposal round"
    );

    let mut wrong_certificate = certificate;
    wrong_certificate.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"foreign pipeline parent state"),
            Hash::new(b"foreign pipeline post state"),
            Hash::new(b"foreign pipeline ordinary writes"),
            1,
            Hash::new(b"foreign pipeline executed block"),
        );
    let wrong_apply = AdapterEffect::Apply {
        tag,
        subject: wrong_certificate.subject,
        certificate: wrong_certificate,
    };
    assert!(
        validate_ownership
            .rebind_as_inherited_adapter_effect(&wrong_apply)
            .is_err(),
        "Apply cannot replace the inherited execution commitment"
    );
}

#[test]
fn body_pipeline_acquires_commit_authority_monotonically_under_one_owner() {
    let (context, keys) = authenticated_runtime_context();
    let commit = signed_runtime_quorum_certificate(&context, &keys, 0x75);
    let tag = EventTag::new(context.height, commit.round.view, Generation::new(4));
    let store = AdapterEffect::StoreBody {
        tag,
        round: commit.proposal_round,
        subject: commit.subject,
    };
    let validate = AdapterEffect::ValidateBody {
        tag,
        round: commit.proposal_round,
        subject: commit.subject,
    };
    let apply = AdapterEffect::Apply {
        tag,
        subject: commit.subject,
        certificate: commit.clone(),
    };

    // A local proposal or ordinary body fetch has no quorum authority at
    // Store/Validate time. A late durable Decision supplies Commit
    // authority without replacing the body's immutable local owner.
    let local_store = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&store),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 4_101)],
    )
    .expect("local Store binds an uncertified body statement")
    .pop()
    .expect("one local Store owner");
    let local_validate = local_store
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("local Validate retains the uncertified statement");
    let local_apply = local_validate
        .rebind_as_inherited_adapter_effect(&apply)
        .expect("late Decision refines local validation to Commit authority");
    assert_eq!(local_store.owner(), local_apply.owner());
    assert_eq!(
        local_validate
            .candidate_semantic_statement()
            .expect("local Validate carries its typed statement")
            .phase,
        None
    );
    assert_eq!(
        local_apply
            .candidate_semantic_statement()
            .expect("local Apply carries its acquired authority")
            .phase,
        Some(wire::GlobalPhase::Commit)
    );

    // A Prepare-certified reconstruction has already frozen the
    // commitment. The matching CommitQC may promote only the phase.
    let mut prepare = commit.clone();
    prepare.phase = wire::GlobalPhase::Prepare;
    let fetch = AdapterEffect::FetchBody {
        tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: None,
        certified_sources: Vec::new(),
        certificate: Some(prepare),
    };
    let prepare_fetch = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 4_102)],
    )
    .expect("Prepare Fetch binds its certified statement")
    .pop()
    .expect("one Prepare Fetch owner");
    let prepare_store = prepare_fetch
        .rebind_as_inherited_adapter_effect(&store)
        .expect("Store retains Prepare authority");
    let prepare_validate = prepare_store
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("Validate retains Prepare authority");
    let prepare_apply = prepare_validate
        .rebind_as_inherited_adapter_effect(&apply)
        .expect("matching Decision promotes Prepare to Commit");
    assert_eq!(prepare_fetch.owner(), prepare_apply.owner());
    assert_eq!(
        prepare_validate
            .candidate_semantic_statement()
            .expect("Prepare Validate carries its statement")
            .phase,
        Some(wire::GlobalPhase::Prepare)
    );
    assert_eq!(
        prepare_apply.candidate_semantic_statement(),
        production_adapter_effect_candidate_statement(&apply).map(|(_, statement)| statement)
    );

    let rejects =
        |certificate: wire::QuorumCertificate, subject: wire::BlockSubject, mutation: &str| {
            let changed = AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            };
            assert!(
                prepare_validate
                    .rebind_as_inherited_adapter_effect(&changed)
                    .is_err(),
                "{mutation} must be rejected before candidate refinement"
            );
        };

    let mut changed_subject = commit.clone();
    changed_subject.subject = wire::BlockSubject {
        payload_hash: Hash::new(b"foreign Apply subject"),
        ..changed_subject.subject
    };
    rejects(
        changed_subject.clone(),
        changed_subject.subject,
        "subject drift",
    );
    rejects(
        changed_subject,
        commit.subject,
        "certificate/effect subject disagreement",
    );

    let mut changed_proposal_round = commit.clone();
    changed_proposal_round.proposal_round.view += 1;
    rejects(
        changed_proposal_round,
        commit.subject,
        "proposal-round drift",
    );

    let mut changed_round = commit.clone();
    changed_round.round.view += 1;
    rejects(changed_round, commit.subject, "round drift");

    let foreign_context = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"foreign Apply height context",
    )));
    let mut changed_context = commit.clone();
    changed_context.round.context_id = foreign_context;
    changed_context.proposal_round.context_id = foreign_context;
    rejects(changed_context, commit.subject, "context drift");

    let mut changed_commitment = commit;
    changed_commitment.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"foreign refinement parent state"),
            Hash::new(b"foreign refinement post state"),
            Hash::new(b"foreign refinement ordinary writes"),
            1,
            Hash::new(b"foreign refinement executed block"),
        );
    let changed_commitment_subject = changed_commitment.subject;
    rejects(
        changed_commitment,
        changed_commitment_subject,
        "commitment drift",
    );
}

#[test]
fn fetch_authority_relation_is_monotonic_and_recognizes_stale_carriers() {
    let (context, keys) = authenticated_runtime_context();
    let commit = signed_runtime_quorum_certificate(&context, &keys, 0x76);
    let ordinary = RuntimeCandidateSemanticStatement::new(
        commit.round,
        commit.proposal_round,
        Some(commit.subject),
        None,
        None,
    );
    let prepare = RuntimeCandidateSemanticStatement::new(
        commit.round,
        commit.proposal_round,
        Some(commit.subject),
        Some(wire::GlobalPhase::Prepare),
        Some(commit.execution_commitment),
    );
    let committed = RuntimeCandidateSemanticStatement::new(
        commit.round,
        commit.proposal_round,
        Some(commit.subject),
        Some(wire::GlobalPhase::Commit),
        Some(commit.execution_commitment),
    );

    for statement in [ordinary, prepare, committed] {
        assert_eq!(
            statement.fetch_authority_relation_to(statement),
            Some(RuntimeFetchAuthorityRelation::Same)
        );
    }
    assert_eq!(
        ordinary.fetch_authority_relation_to(prepare),
        Some(RuntimeFetchAuthorityRelation::Upgrade)
    );
    assert_eq!(
        ordinary.fetch_authority_relation_to(committed),
        Some(RuntimeFetchAuthorityRelation::Upgrade)
    );
    assert_eq!(
        prepare.fetch_authority_relation_to(committed),
        Some(RuntimeFetchAuthorityRelation::Upgrade)
    );
    assert_eq!(
        prepare.fetch_authority_relation_to(ordinary),
        Some(RuntimeFetchAuthorityRelation::Stale)
    );
    assert_eq!(
        committed.fetch_authority_relation_to(prepare),
        Some(RuntimeFetchAuthorityRelation::Stale)
    );
    assert_eq!(
        committed.fetch_authority_relation_to(ordinary),
        Some(RuntimeFetchAuthorityRelation::Stale)
    );

    let mut changed_commitment = prepare;
    changed_commitment.execution_commitment =
        Some(wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"foreign fetch parent state"),
            Hash::new(b"foreign fetch post state"),
            Hash::new(b"foreign fetch writes"),
            1,
            Hash::new(b"foreign fetch block"),
        ));
    assert_eq!(
        prepare.fetch_authority_relation_to(changed_commitment),
        None,
        "same-phase commitment drift must fail closed"
    );
    assert_eq!(
        committed.fetch_authority_relation_to(changed_commitment),
        None,
        "reverse-phase commitment drift is not a stale carrier"
    );

    let mut changed_round = committed;
    changed_round.round.view += 1;
    assert_eq!(
        ordinary.fetch_authority_relation_to(changed_round),
        None,
        "consensus-coordinate drift must fail closed"
    );
    let mut changed_subject = committed;
    changed_subject.subject = Some(wire::BlockSubject {
        payload_hash: Hash::new(b"foreign fetch payload"),
        ..commit.subject
    });
    assert_eq!(
        ordinary.fetch_authority_relation_to(changed_subject),
        None,
        "subject drift must fail closed"
    );
}

#[test]
fn candidate_statement_binds_manifest_by_exact_consensus_coordinates() {
    let (context, _keys) = authenticated_runtime_context();
    let manifest = runtime_manifest(&context, 0x78);
    let statement = RuntimeCandidateSemanticStatement::new(
        manifest.round,
        manifest.round,
        Some(manifest.subject),
        Some(wire::GlobalPhase::Commit),
        Some(wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"manifest parent state"),
            Hash::new(b"manifest post state"),
            Hash::new(b"manifest writes"),
            1,
            Hash::new(b"manifest executed block"),
        )),
    );
    assert!(statement.binds_exact_body_manifest(&manifest));

    let certified_round = wire::ConsensusRound {
        view: manifest.round.view + 3,
        ..manifest.round
    };
    let mut later_statement = statement;
    later_statement.round = certified_round;
    assert!(!later_statement.binds_exact_body_manifest(&manifest));

    let mut changed_round = manifest.clone();
    changed_round.round.view += 1;
    assert!(!statement.binds_exact_body_manifest(&changed_round));

    let mut changed_subject = manifest.clone();
    changed_subject.subject.payload_hash = Hash::new(b"foreign manifest payload");
    assert!(!statement.binds_exact_body_manifest(&changed_subject));

    let mut changed_context = manifest.clone();
    changed_context.round.context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(
        Hash::new(b"foreign manifest context"),
    ));
    assert!(!statement.binds_exact_body_manifest(&changed_context));

    let later_tag = EventTag::new(context.height, certified_round.view, Generation::new(5));
    let later_fetch = AdapterEffect::FetchBody {
        tag: later_tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(wire::QuorumCertificate {
            round: certified_round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: manifest.subject,
            execution_commitment: later_statement
                .execution_commitment
                .expect("certified statement has an execution commitment"),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x78; 96],
        }),
    };
    let later_fetch_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&later_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(later_tag, 7_801)],
    )
    .expect("candidate binding preserves the later certificate round")
    .pop()
    .expect("one later-round FetchBody owner");
    assert!(
        !later_fetch_ownership.binds_exact_fetch_body_manifest(&manifest),
        "a Fetch certificate outside the manifest round is not a valid production bridge"
    );

    let tag = EventTag::new(context.height, manifest.round.view, Generation::new(5));
    let fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(wire::QuorumCertificate {
            round: manifest.round,
            proposal_round: manifest.round,
            phase: wire::GlobalPhase::Commit,
            subject: manifest.subject,
            execution_commitment: statement
                .execution_commitment
                .expect("certified statement has an execution commitment"),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x79; 96],
        }),
    };
    let fetch_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 7_802)],
    )
    .expect("certified FetchBody receives an exact production binding")
    .pop()
    .expect("one certified FetchBody owner");
    assert!(fetch_ownership.binds_exact_fetch_body_manifest(&manifest));
    assert!(!fetch_ownership.binds_exact_fetch_body_manifest(&changed_round));
    assert!(!fetch_ownership.binds_exact_fetch_body_manifest(&changed_subject));

    let store = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&store),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 7_803)],
    )
    .expect("StoreBody receives an exact production binding")
    .pop()
    .expect("one StoreBody owner");
    assert!(store_ownership.candidate_semantic_statement().is_some());
    assert!(
        !store_ownership.binds_exact_fetch_body_manifest(&manifest),
        "matching body coordinates cannot substitute a non-Fetch candidate"
    );
}

#[test]
fn fetch_authority_adoption_retains_owner_and_incoming_positions() {
    let (context, keys) = authenticated_runtime_context();
    let commit = signed_runtime_quorum_certificate(&context, &keys, 0x77);
    let tag = EventTag::new(context.height, commit.round.view, Generation::new(5));
    let bytes = [0x77, 6];
    let manifest = encode_payload(&context, commit.proposal_round, commit.subject, &bytes)
        .expect("ordinary fetch manifest matches its physical lineage")
        .manifest()
        .clone();
    let ordinary_fetch = AdapterEffect::FetchBody {
        tag,
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(manifest),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let ordinary = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&ordinary_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 4_201)],
    )
    .expect("ordinary fetch has one exact owner")
    .pop()
    .expect("one ordinary fetch owner");

    let mut prepare_certificate = commit.clone();
    prepare_certificate.phase = wire::GlobalPhase::Prepare;
    let prepare_fetch = AdapterEffect::FetchBody {
        tag,
        round: prepare_certificate.proposal_round,
        subject: prepare_certificate.subject,
        manifest: None,
        certified_sources: Vec::new(),
        certificate: Some(prepare_certificate),
    };
    let prefix = AdapterEffect::StoreBody {
        tag,
        round: commit.proposal_round,
        subject: commit.subject,
    };
    let incoming = bind_adapter_effect_batch_ownership(
        &[prefix.clone(), prepare_fetch.clone()],
        vec![
            RuntimeEffectOwnership::fresh_for_test(tag, 4_202),
            RuntimeEffectOwnership::fresh_for_test(tag, 4_203),
        ],
    )
    .expect("Prepare carrier retains its two-effect macro-step positions")
    .pop()
    .expect("Prepare fetch is the final effect");

    let (adopted, relation) = ordinary
        .adopt_incumbent_fetch_for_retry_or_authority(&incoming, &prepare_fetch)
        .expect("ordinary fetch adopts authenticated Prepare authority");
    assert_eq!(relation, RuntimeFetchAuthorityRelation::Upgrade);
    assert_eq!(adopted.owner(), ordinary.owner());
    assert_eq!(adopted.causality(), ordinary.causality());
    let adopted_binding = adopted.binding().expect("adopted carrier is bound");
    let incoming_binding = incoming.binding().expect("incoming carrier is bound");
    assert_eq!(
        adopted_binding.effect_position,
        incoming_binding.effect_position
    );
    assert_eq!(adopted_binding.effect_count, incoming_binding.effect_count);
    assert_eq!(
        adopted_binding.candidate_position,
        incoming_binding.candidate_position
    );
    assert_eq!(
        adopted_binding.candidate_count,
        incoming_binding.candidate_count
    );
    assert_eq!(
        adopted.candidate_semantic_statement(),
        incoming.candidate_semantic_statement()
    );

    let (stale, stale_relation) = adopted
        .adopt_incumbent_fetch_for_retry_or_authority(&ordinary, &ordinary_fetch)
        .expect("ordinary retransmission is an exact stale carrier");
    assert_eq!(stale_relation, RuntimeFetchAuthorityRelation::Stale);
    assert_eq!(stale.owner(), adopted.owner());
    assert_eq!(
        stale.candidate_semantic_statement(),
        ordinary.candidate_semantic_statement(),
        "the carrier remains exact while the task reducer retains stronger authority"
    );

    let prefix_owner = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&prefix),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 4_204)],
    )
    .expect("StoreBody has an exact non-fetch candidate owner")
    .pop()
    .expect("one StoreBody owner");
    assert!(
        ordinary
            .adopt_incumbent_fetch_for_retry_or_authority(&prefix_owner, &prefix)
            .is_err(),
        "candidate-kind drift must fail before owner adoption"
    );
}

fn observe_enter_view_for_test(
    runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
    previous: EventTag,
    rebound: EventTag,
    manifest: &wire::PayloadManifest,
) {
    assert_eq!(runtime.round_tag(), previous);
    let protected_lock = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"runtime EnterView parent state"),
            Hash::new(b"runtime EnterView post state"),
            Hash::new(b"runtime EnterView ordinary writes"),
            1,
            Hash::new(b"runtime EnterView executed block"),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xA6; 96],
    };
    runtime
        .observe_effects_with_test_ownership(
            Instant::now(),
            &[AdapterEffect::EnterView {
                tag: rebound,
                certificate: wire::TimeoutCertificate {
                    round: wire::ConsensusRound {
                        view: rebound
                            .view()
                            .checked_sub(1)
                            .expect("test EnterView target has a predecessor"),
                        ..manifest.round
                    },
                    groups: vec![wire::TimeoutVoteGroup {
                        highest_prepare_qc: None,
                        signers: vec![0, 1, 2],
                        aggregate_signature: vec![0xA5; 96],
                    }],
                },
                protected_lock: Some(protected_lock),
            }],
        )
        .expect("test EnterView retains positional producer ownership");
    assert_eq!(runtime.round_tag(), rebound);
}

#[test]
fn body_available_rebind_accepts_same_view_higher_generation() {
    let directory = TempDir::new().expect("temporary same-view rebind directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let initial = runtime.round_tag();
    let view_one = EventTag::new(
        initial.height(),
        1,
        Generation::new(initial.generation().get() + 1),
    );
    let manifest = runtime_manifest(&context, 0x8A);
    observe_enter_view_for_test(&mut runtime, initial, view_one, &manifest);

    stage_completion_for_queue_test(
        &mut runtime,
        view_one,
        AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        },
    );
    let causal_origin = runtime.ingress.commands[0].causal_origin.clone();
    let lifecycle_ordinal = runtime.ingress.commands[0].lifecycle_ordinal;
    let rebound = EventTag::new(
        view_one.height(),
        view_one.view(),
        Generation::new(view_one.generation().get() + 1),
    );
    observe_enter_view_for_test(&mut runtime, view_one, rebound, &manifest);

    assert!(
        runtime
            .rebind_body_available(view_one, rebound, &manifest)
            .expect("same-view generation supersession transfers the exact owner")
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(matches!(
        runtime.ingress.commands.front(),
        Some(TaggedCommand {
            tag,
            command: AdapterCommand::BodyAvailable {
                manifest: queued_manifest,
            },
            ..
        }) if *tag == rebound && queued_manifest == &manifest
    ));
    assert_eq!(runtime.ingress.commands[0].causal_origin, causal_origin);
    assert_eq!(
        runtime.ingress.commands[0].lifecycle_ordinal, lifecycle_ordinal,
        "view/generation rebinding retains the logical lifecycle owner"
    );
    assert!(!runtime.fail_closed);
}

#[test]
fn unpublished_body_token_rebinds_retries_and_retires_as_one_exact_owner() {
    let directory = TempDir::new().expect("temporary reserved-body rebind directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let initial = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x8B);
    let lifecycle_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint the test Fetch lifecycle from the runtime source");
    let fetch_effect = AdapterEffect::FetchBody {
        tag: initial,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let fetch_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(
            initial,
            lifecycle_ordinal,
        )],
    )
    .expect("bind the exact Fetch candidate")
    .pop()
    .expect("one Fetch effect owns one lifecycle");
    assert!(fetch_ownership.binds_exact_fetch_body_manifest(&manifest));
    let reservation = runtime
        .reserve_body_available_with_owner(initial, manifest.clone(), &fetch_ownership)
        .expect("reserve an unpublished body completion under its Fetch owner");
    let rebound = EventTag::new(
        initial.height(),
        initial.view() + 1,
        Generation::new(initial.generation().get() + 1),
    );
    observe_enter_view_for_test(&mut runtime, initial, rebound, &manifest);
    let source_before_rebind = runtime
        .ingress
        .lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect ordinal source before exact token rebind");

    let foreign_subject = runtime_manifest(&context, 0x8C).subject;
    assert!(
        !runtime
            .rebind_unpublished_body_available(initial, rebound, manifest.round, foreign_subject,)
            .expect("foreign coordinates cannot select the reserved token")
    );
    assert_eq!(
        runtime
            .ingress
            .reserved_body_available
            .as_ref()
            .map(BodyAvailableReservation::tag),
        Some(initial),
    );
    assert!(
        runtime
            .rebind_unpublished_body_available(initial, rebound, manifest.round, manifest.subject,)
            .expect("the unpublished token is a serialized body owner")
    );
    let mut rebound_reservation = reservation;
    rebound_reservation.tag = rebound;
    assert_eq!(
        runtime.ingress.reserved_body_available.as_ref(),
        Some(&rebound_reservation),
    );
    assert_eq!(
        runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source after exact token rebind"),
        source_before_rebind,
        "rebind cannot remint the token",
    );
    let retry = runtime
        .reserve_body_available_with_owner(
            rebound,
            manifest.clone(),
            &fetch_ownership
                .rebind_same_adapter_effect(&AdapterEffect::FetchBody {
                    tag: rebound,
                    round: manifest.round,
                    subject: manifest.subject,
                    manifest: Some(manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                })
                .expect("rebind the exact Fetch consumer"),
        )
        .expect("rebound exact owned retry reclaims the immutable root token");
    assert_eq!(retry, rebound_reservation);
    assert_eq!(
        runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source after rebound retry"),
        source_before_rebind,
        "retry cannot remint the rebound token",
    );

    assert!(
        runtime
            .retire_unpublished_body_available(rebound, manifest.round, manifest.subject,)
            .expect("terminal supersession retires the exact unpublished owner")
    );
    assert!(runtime.ingress.reserved_body_available.is_none());
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!runtime.fail_closed);
}

fn authenticated_network_runtime(
    directory: &TempDir,
    queue: RuntimeQueueConfig,
) -> (
    SerializedV2Runtime<SumeragiV2Adapter>,
    wire::HeightContext,
    Vec<KeyPair>,
) {
    authenticated_network_runtime_with_local_validator(directory, queue, None)
}

fn authenticated_network_runtime_with_local_validator(
    directory: &TempDir,
    queue: RuntimeQueueConfig,
    local_validator: Option<wire::ValidatorIndex>,
) -> (
    SerializedV2Runtime<SumeragiV2Adapter>,
    wire::HeightContext,
    Vec<KeyPair>,
) {
    let (context, keys) = authenticated_runtime_context();
    let proofs = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("runtime fixture proof of possession")
        })
        .collect();
    let verified =
        VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified fixture");
    let (adapter, startup) = SumeragiV2Adapter::open(
        directory.path().join("runtime-ingress-safety.wal"),
        verified,
        local_validator,
        Generation::new(1),
        [0x31; 32],
        AdapterFingerprints {
            node: Hash::new(b"runtime ingress node"),
            build: Hash::new(b"runtime ingress build"),
            config: Hash::new(b"runtime ingress config"),
        },
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("open authenticated network runtime adapter");
    assert!(startup.is_empty());
    let runtime = SerializedV2Runtime::new(
        adapter,
        startup,
        Instant::now(),
        Duration::from_secs(10),
        queue,
    )
    .expect("valid authenticated network runtime")
    .0;
    (runtime, context, keys)
}

/// Stage an exact completion directly in the bounded FIFO for tests of
/// queue ownership itself. Production tests use the public enqueue seams,
/// whose reducer preflight correctly rejects callbacks without a live
/// phase or exact terminal lifecycle.
fn stage_completion_for_queue_test(
    runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
    tag: EventTag,
    command: AdapterCommand,
) {
    runtime
        .ingress
        .enqueue(TaggedCommand::new(
            tag,
            CommandClass::Completion,
            command,
            Instant::now(),
        ))
        .expect("queue-ownership fixture stages an exact completion");
}

fn stage_owned_completion_for_queue_test(
    runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
    tag: EventTag,
    command: AdapterCommand,
    ownership: &RuntimeEffectOwnership,
) {
    let mut tagged = TaggedCommand::with_causal_origin(
        tag,
        CommandClass::Completion,
        command,
        Instant::now(),
        ownership.owner().causal_origin().clone(),
        ownership.owner().lifecycle_ordinal(),
    )
    .expect("owned queue fixture retains the exact lifecycle owner");
    tagged.candidate_semantic_statement = ownership.candidate_semantic_statement();
    assert!(tagged.validate_admission_identity());
    runtime
        .ingress
        .enqueue(tagged)
        .expect("owned queue fixture stages an exact completion");
}

/// Attach the same private local/causal runtime wrapper that production
/// dispatch installs around one exact adapter-owned Busy occurrence.
fn mint_local_lifecycle_owner_for_test(
    runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
    semantic_identity: &[u8],
) -> RuntimeLifecycleOwner {
    let lifecycle_ordinal = runtime
        .ingress
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve one exact local lifecycle ordinal");
    let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
        runtime.round_tag(),
        CommandClass::Completion,
        RuntimeFreshRootKind::StartupRecovery,
        semantic_identity,
    );
    RuntimeLifecycleOwner::new(origin, lifecycle_ordinal)
        .expect("bind the local deferred lifecycle ordinal")
}

fn bind_deferred_lifecycle_owner_for_test(
    runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
    deferred_admission_ordinal: u128,
    owner: RuntimeLifecycleOwner,
) -> RuntimeLifecycleOwner {
    let physical_cut = runtime.ingress_physical_cut;
    let runtime_seal = runtime
        .driver
        .bind_deferred_runtime_ownership(
            deferred_admission_ordinal,
            owner.causal_origin().lifecycle_key.clone(),
            owner.lifecycle_ordinal(),
            false,
            None,
            physical_cut,
        )
        .expect("seal the exact local Busy occurrence");
    let deferred = RuntimeDeferredLifecycleOwnership::new(
        owner.clone(),
        deferred_admission_ordinal,
        RuntimeDispatchIngress::LocalOrCausal,
        None,
        physical_cut,
        runtime_seal,
    )
    .expect("freeze the exact local Busy occurrence");
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .insert(deferred_admission_ordinal, deferred)
            .is_none(),
        "the fixture cannot replace an existing runtime wrapper"
    );
    owner
}

fn bind_local_deferred_lifecycle_for_test(
    runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
    deferred_admission_ordinal: u128,
    semantic_identity: &[u8],
) -> RuntimeLifecycleOwner {
    let owner = mint_local_lifecycle_owner_for_test(runtime, semantic_identity);
    bind_deferred_lifecycle_owner_for_test(runtime, deferred_admission_ordinal, owner)
}

/// Inject one real Busy-deferred completion with both its persistent
/// adapter reservation and its matching serialized-runtime wrapper.
fn defer_persistent_body_available_for_test(
    runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
    tag: EventTag,
    manifest: &wire::PayloadManifest,
    semantic_identity: &[u8],
) -> (u128, RuntimeLifecycleOwner) {
    let owner = mint_local_lifecycle_owner_for_test(runtime, semantic_identity);
    defer_persistent_body_available_with_owner_for_test(runtime, tag, manifest, owner)
}

fn defer_persistent_body_available_with_owner_for_test(
    runtime: &mut SerializedV2Runtime<SumeragiV2Adapter>,
    tag: EventTag,
    manifest: &wire::PayloadManifest,
    owner: RuntimeLifecycleOwner,
) -> (u128, RuntimeLifecycleOwner) {
    let before = runtime.driver.all_deferred_admission_ordinals();
    runtime
        .driver
        .bind_selected_producer_lifecycle(
            owner.causal_origin().lifecycle_key.clone(),
            owner.lifecycle_ordinal(),
        )
        .expect("bind persistent body producer lifecycle");
    let outcome = runtime
        .driver
        .body_available(tag, manifest.clone())
        .expect("stage persistent body completion behind the Busy fence");
    runtime.driver.clear_selected_producer_lifecycle();
    assert!(
        outcome.into_effects().is_empty(),
        "the persistent fixture requires a real Busy-deferred occurrence"
    );
    let ordinals = runtime
        .driver
        .all_deferred_admission_ordinals()
        .difference(&before)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(ordinals.len(), 1);
    let deferred_admission_ordinal = ordinals[0];
    let owner = bind_deferred_lifecycle_owner_for_test(runtime, deferred_admission_ordinal, owner);
    assert!(
        runtime
            .driver
            .deferred_body_available_has_persistent_producer(tag, manifest)
            .expect("validate the exact persistent Busy producer")
    );
    (deferred_admission_ordinal, owner)
}

fn fair_network_ownership(
    message: &wire::ConsensusMessageV2,
    sender: PeerId,
) -> FairV2IngressOwnershipEvidence {
    let mut admitted =
        super::super::fair_v2_ingress_admit_for_test(super::super::InboundBlockMessage::new(
            super::super::message::BlockMessage::V2(message.clone()),
            Some(sender),
        ));
    admitted
        .take_ingress_ownership()
        .expect("real test fair ingress produces exact source ownership")
}

fn preowned_leader_wire_ownerships_with_dequeue_mode(
    context: &wire::HeightContext,
    messages: &[(wire::ConsensusMessageV2, PeerId)],
    lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    push_all_before_dequeue: bool,
) -> (
    TempDir,
    Arc<super::super::FairV2Ingress>,
    Vec<FairV2IngressOwnershipEvidence>,
) {
    let directory = TempDir::new().expect("temporary preowned leader-wire directory");
    let ingress = Arc::new(
        super::super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        ),
    );
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    ingress
        .configure_roster_for_context(roster.clone(), &context.network_id, context.da_layout)
        .expect("preowned leader-wire geometry");
    ingress.require_leader_wire_lifecycle_gate();
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            context.da_layout.max_chunk_count,
        )
        .expect("finite preowned leader-wire capacity");
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            context.id(),
            context.height,
            [0xE7; 32],
            0,
            false,
        );
    let (gate, restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &directory.path().join("leader-wire-preowned.wal"),
            context.id(),
            context.height,
            [0xE7; 32],
            roster.iter().cloned().collect(),
            capacity,
            context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open preowned leader-wire gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&gate),
            restore,
            lifecycle_ordinals,
            context.id(),
            context.height,
        )
        .expect("bind preowned leader-wire gate");
    ingress.open().expect("open preowned fair ingress");

    if push_all_before_dequeue {
        for (message, semantic_origin) in messages {
            assert!(matches!(
                ingress.try_push(InboundBlockMessage::new(
                    BlockMessage::V2(message.clone()),
                    Some(semantic_origin.clone()),
                )),
                Ok(super::super::FairV2IngressPushDisposition::Enqueued)
            ));
        }
    }

    let ownerships = messages
        .iter()
        .enumerate()
        .map(|(message_index, (message, semantic_origin))| {
            if !push_all_before_dequeue {
                assert!(matches!(
                    ingress.try_push(InboundBlockMessage::new(
                        BlockMessage::V2(message.clone()),
                        Some(semantic_origin.clone()),
                    )),
                    Ok(super::super::FairV2IngressPushDisposition::Enqueued)
                ));
            }
            let mut admitted = ingress
                .try_recv()
                .expect("drain preowned leader-wire occurrence");
            let mut ownership = admitted
                .take_ingress_ownership()
                .expect("preowned leader wire retains fair ownership");
            assert!(
                ownership.leader_wire_runtime_receipt().is_some(),
                "checked dequeue atomically installs the durable runtime handoff"
            );
            let token = ownership
                .leader_wire_token()
                .expect("productive dequeue retains its immutable leader-wire token");
            let remaining_ingress = gate
                .ingress_scheduler_ordinals()
                .expect("read durable owners after atomic handoff");
            assert!(!remaining_ingress.contains(&token.scheduler_ordinal()));
            assert_eq!(
                remaining_ingress.len(),
                if push_all_before_dequeue {
                    messages.len().saturating_sub(message_index + 1)
                } else {
                    0
                },
                "atomic handoff removes only the dequeued durable Ingress owner"
            );
            {
                let state = ingress.state.lock();
                let record = state
                    .leader_wire_lifecycles
                    .get(&token.slot)
                    .expect("atomic handoff retains the exact lifecycle record");
                assert_eq!(
                    record.status,
                    super::super::FairV2IngressLeaderWireStatus::Runtime,
                    "atomic handoff publishes the in-memory Runtime owner"
                );
            }
            ingress
                .bind_leader_wire_runtime_ownership(&mut ownership)
                .expect("repeated preowned leader-wire bind is idempotent");
            assert!(matches!(
                ingress.try_push(InboundBlockMessage::new(
                    BlockMessage::V2(message.clone()),
                    Some(semantic_origin.clone()),
                )),
                Ok(super::super::FairV2IngressPushDisposition::Coalesced)
            ));
            ownership
        })
        .collect();
    (directory, ingress, ownerships)
}

fn preowned_leader_wire_ownerships(
    context: &wire::HeightContext,
    messages: &[(wire::ConsensusMessageV2, PeerId)],
    lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
) -> (
    TempDir,
    Arc<super::super::FairV2Ingress>,
    Vec<FairV2IngressOwnershipEvidence>,
) {
    preowned_leader_wire_ownerships_with_dequeue_mode(context, messages, lifecycle_ordinals, false)
}

fn preowned_leader_wire_ownerships_at_shared_cut(
    context: &wire::HeightContext,
    messages: &[(wire::ConsensusMessageV2, PeerId)],
    lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
) -> (
    TempDir,
    Arc<super::super::FairV2Ingress>,
    Vec<FairV2IngressOwnershipEvidence>,
) {
    preowned_leader_wire_ownerships_with_dequeue_mode(context, messages, lifecycle_ordinals, true)
}

struct LeaderWireProposalFixture {
    ingress: Arc<super::super::FairV2Ingress>,
    gate: Arc<super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate>,
    message: wire::ConsensusMessageV2,
    ownership: FairV2IngressOwnershipEvidence,
    receipt: LeaderWireLifecycleRuntimeReceipt,
}

fn leader_wire_proposal_fixture(
    directory: &TempDir,
    context: &wire::HeightContext,
    keys: &[KeyPair],
    marker: u8,
    lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
) -> LeaderWireProposalFixture {
    let message = signed_runtime_proposal(context, keys, marker);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
        unreachable!("signed runtime proposal fixture carries Proposal")
    };
    let ingress = Arc::new(
        super::super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        ),
    );
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    ingress
        .configure_roster_for_context(roster.clone(), &context.network_id, context.da_layout)
        .expect("leader-wire runtime fixture geometry");
    ingress.require_leader_wire_lifecycle_gate();
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            context.da_layout.max_chunk_count,
        )
        .expect("finite leader-wire runtime fixture capacity");
    let owner = [marker; 32];
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            context.id(),
            context.height,
            owner,
            proposal.round.view,
            false,
        );
    let (gate, restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &directory
                .path()
                .join(format!("leader-wire-runtime-{marker}.wal")),
            context.id(),
            context.height,
            owner,
            roster.iter().cloned().collect(),
            capacity,
            context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open leader-wire runtime fixture gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&gate),
            restore,
            lifecycle_ordinals,
            context.id(),
            context.height,
        )
        .expect("bind leader-wire runtime fixture gate");
    ingress.open().expect("open leader-wire runtime fixture");
    let semantic_origin = context.roster
        [usize::try_from(proposal.proposer).expect("small fixture proposer")]
    .validator
    .clone();
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message.clone()),
            Some(semantic_origin),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    let mut saw_predequeue_owner = false;
    assert!(
        ingress
            .try_recv_if_checked(|inbound| {
                let ownership = inbound
                    .ingress_ownership()
                    .expect("queued leader-wire message retains fair ownership");
                assert!(ownership.runtime_physical_cut().is_none());
                assert!(ownership.leader_wire_token().is_some());
                assert!(ownership.leader_wire_runtime_receipt().is_none());
                let projected =
                    RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, ownership.clone())
                        .expect("pre-dequeue identity remains valid for the capacity probe");
                assert!(projected.validate_exact());
                assert!(!projected.validate_frozen_physical());
                saw_predequeue_owner = true;
                false
            })
            .expect("rejected pre-dequeue probe preserves the queued owner")
            .is_none()
    );
    assert!(saw_predequeue_owner);
    let mut admitted = ingress
        .try_recv()
        .expect("drain exact leader-wire proposal fixture");
    let mut ownership = admitted
        .take_ingress_ownership()
        .expect("leader-wire proposal retains fair-ingress ownership");
    ingress
        .bind_leader_wire_runtime_ownership(&mut ownership)
        .expect("bind exact leader-wire runtime receipt");
    let receipt = ownership
        .leader_wire_runtime_receipt()
        .expect("productive proposal carries runtime receipt")
        .clone();
    LeaderWireProposalFixture {
        ingress,
        gate,
        message,
        ownership,
        receipt,
    }
}

fn assert_volatile_leader_wire_release(
    fixture: &LeaderWireProposalFixture,
    receipt: &LeaderWireLifecycleRuntimeReceipt,
) {
    assert_eq!(receipt, &fixture.receipt);
    fixture
        .ingress
        .mark_leader_wire_volatile_terminal(receipt)
        .expect("publish process-local leader-wire retirement");
    assert_eq!(
        fixture
            .gate
            .earliest_ingress_scheduler_ordinal()
            .expect("read durable leader-wire minimum"),
        None,
        "a retired runtime owner cannot remain an active scheduler predecessor"
    );
    let semantic_origin = fixture.receipt.token().identity.semantic_origin.clone();
    assert!(matches!(
        fixture.ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(fixture.message.clone()),
            Some(semantic_origin),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Coalesced)
    ));
}

fn bind_authenticated_deferred_proposal_for_test(
    runtime: &mut SerializedV2Runtime,
    fixture: &LeaderWireProposalFixture,
) -> (wire::Proposal, u128) {
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = &fixture.message.payload else {
        unreachable!("leader-wire fixture carries Proposal")
    };
    let proposal = proposal.clone();
    let ingress_ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(
        &fixture.message,
        fixture.ownership.clone(),
    )
    .expect("project exact leader-wire ownership into runtime");
    let tagged = TaggedCommand::with_ingress_ownership(
        runtime.round_tag(),
        CommandClass::Normal,
        AdapterCommand::Authenticated(AuthenticatedConsensusMessage::for_test(
            fixture.message.clone(),
        )),
        Instant::now(),
        ingress_ownership.clone(),
    );
    let lifecycle_ordinal = tagged
        .lifecycle_ordinal
        .expect("leader-wire command carries its scheduler ordinal");
    let lifecycle_owner =
        RuntimeLifecycleOwner::new(tagged.causal_origin.clone(), lifecycle_ordinal)
            .expect("construct exact deferred lifecycle owner");
    let (source_physical_ordinal, physical_cut) = ingress_ownership
        .leader_wire_physical_carrier()
        .expect("leader-wire carrier set is exact")
        .expect("leader-wire carrier exposes its checked physical cut");
    runtime
        .driver
        .defer_authenticated_proposal_for_test(runtime.round_tag(), &proposal)
        .expect("stage Busy-deferred proposal");
    let (_, deferred_ordinal) = runtime
        .driver
        .deferred_authenticated_message_owner(&fixture.message)
        .expect("deferred proposal exposes its adapter ordinal");
    let runtime_seal = runtime
        .driver
        .bind_deferred_runtime_ownership(
            deferred_ordinal,
            lifecycle_owner.causal_origin().lifecycle_key.clone(),
            lifecycle_owner.lifecycle_ordinal(),
            true,
            Some(source_physical_ordinal),
            physical_cut,
        )
        .expect("seal the exact deferred adapter occurrence");
    let lifecycle_owner = RuntimeDeferredLifecycleOwnership::new(
        lifecycle_owner,
        deferred_ordinal,
        RuntimeDispatchIngress::DirectAuthenticated,
        Some(source_physical_ordinal),
        physical_cut,
        runtime_seal,
    )
    .expect("freeze the exact deferred physical cut");
    assert!(
        runtime
            .deferred_ingress_ownership
            .insert(deferred_ordinal, ingress_ownership.clone())
            .is_none()
    );
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .insert(deferred_ordinal, lifecycle_owner)
            .is_none()
    );
    runtime
        .register_leader_wire_runtime_receipt(&ingress_ownership)
        .expect("register deferred leader-wire receipt");
    (proposal, deferred_ordinal)
}

fn fair_network_ownership_with_route(
    message: &wire::ConsensusMessageV2,
    semantic_origin: PeerId,
    authenticated_via: PeerId,
    route: NetworkReplyRoute,
) -> FairV2IngressOwnershipEvidence {
    let inbound = super::super::InboundBlockMessage::try_from_transport_with_reply_route(
        super::super::message::BlockMessage::V2(message.clone()),
        semantic_origin,
        authenticated_via,
        route,
    )
    .expect("test reply route binds the semantic origin and authenticated source");
    let mut admitted = super::super::fair_v2_ingress_admit_for_test(inbound);
    admitted
        .take_ingress_ownership()
        .expect("real test fair ingress produces exact routed ownership")
}
