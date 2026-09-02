use super::*;
use crate::sumeragi::v2_core::Generation;
use crate::sumeragi::{
    InboundBlockMessage,
    message::BlockMessage,
    v2::{
        AdapterFingerprints, DeferredBodyPipelineStageForTest, SignRequest, VerifiedHeightContext,
    },
    v2_chunks::encode_payload,
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::peer::PeerId;
use iroha_p2p::network::{NetworkReplyRoute, NetworkReplyRouteError, NetworkReplyRouteTestFixture};
use std::collections::VecDeque;
use tempfile::TempDir;
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
    protected_prepare: Option<(
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
            protected_prepare: None,
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
        ) || matches!(
            (payload, self.protected_prepare),
            (
                wire::ConsensusMessageV2Payload::Vote(vote),
                Some((round, subject, execution_commitment))
            ) if vote.phase == wire::GlobalPhase::Prepare
                && vote.round == round
                && vote.proposal_round == round
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
    let network_id = crate::sumeragi::synthetic_network_id("sumeragi-v2-runtime-ingress-test");
    let (offline_cash_mint_finality_epoch_id, offline_cash_mint_finality_epoch_roster) =
        crate::offline_cash_v1_test_fixtures::mint_finality_roster_and_id(network_id, 1, &roster);
    let context = wire::HeightContext {
        network_id,
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
        offline_cash_mint_finality_epoch_id,
        offline_cash_mint_finality_epoch_roster,
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
    let execution_commitment =
        wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
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
fn pending_validate_binding_for_test(
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    certificate: Option<wire::QuorumCertificate>,
    owner_marker: u128,
) -> (AdapterEffect, PendingRuntimeEffectBinding) {
    let store = AdapterEffect::StoreBody {
        tag,
        round,
        subject,
    };
    let validate = AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    };
    let store_binding = if let Some(certificate) = certificate {
        let fetch = AdapterEffect::FetchBody {
            tag,
            round,
            subject,
            manifest: None,
            certified_sources: Vec::new(),
            certificate: Some(certificate),
        };
        let fetch_binding = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&fetch),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, owner_marker)],
        )
        .expect("bind one certified Fetch fixture")
        .pop()
        .expect("one certified Fetch fixture owner")
        .exact_pending_adapter_effect_binding(&fetch)
        .expect("certified Fetch fixture mints one pending binding");
        fetch_binding
            .project_certified_fetch_store_successor(&fetch, &store)
            .expect("certified Fetch fixture derives Store")
    } else {
        bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&store),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, owner_marker)],
        )
        .expect("bind one ordinary Store fixture")
        .pop()
        .expect("one ordinary Store fixture owner")
        .exact_pending_adapter_effect_binding(&store)
        .expect("ordinary Store fixture mints one pending binding")
    };
    let validate_binding = store_binding
        .project_store_validate_successor(&store, &validate)
        .expect("Store fixture derives Validate");
    (validate, validate_binding)
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
fn recovered_next_wal_vote_seal_fixture(
    marker: u8,
) -> (
    VerifiedHeightContext,
    RecoveredLifecycleNextWalVoteSealV1,
    ValidatedBodyReceipt,
    AdapterEffect,
) {
    let (context, keys) = authenticated_runtime_context();
    let proofs = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("next-WAL Vote fixture proof of possession")
        })
        .collect();
    let verified =
        VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified fixture");
    let manifest = runtime_manifest(&context, marker);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable);
    let vote = wire::Vote {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment: validated.execution_commitment(),
        signer: 0,
        signature: Vec::new(),
    };
    let tag = EventTag::new(
        context.height,
        manifest.round.view,
        Generation::new(u64::from(marker).saturating_add(1)),
    );
    let wal_identity = RecoveredWalFrameIdentity::for_test(
        u64::from(marker).saturating_add(8),
        u64::from(marker).saturating_add(9),
        [marker; 32],
    );
    let effect = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(vote.clone()),
    };
    let seal =
        RecoveredLifecycleNextWalVoteSealV1::for_test(wal_identity, tag, vote, validated.clone())
            .expect("exact next-WAL Vote seal fixture");
    (verified, seal, validated, effect)
}
#[test]
fn recovered_next_wal_vote_projection_is_exact_and_fail_closed() {
    let (verified, seal, _, _) = recovered_next_wal_vote_seal_fixture(0x31);
    let projection = project_recovered_lifecycle_next_wal_vote_candidate(&verified, seal)
        .expect("exact seal projects one canonical standalone Sign");
    assert!(projection.is_exact(&verified));
    let (verified, foreign_context_seal, _, _) = recovered_next_wal_vote_seal_fixture(0x32);
    let (mut foreign_context, foreign_keys) = authenticated_runtime_context();
    foreign_context.nexus_amx_context_hash = Hash::new(b"foreign next-WAL context");
    let foreign_proofs = foreign_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("foreign context proof of possession")
        })
        .collect();
    let foreign_verified = VerifiedHeightContext::genesis(foreign_context, foreign_proofs)
        .expect("verified foreign context");
    assert!(
        project_recovered_lifecycle_next_wal_vote_candidate(
            &foreign_verified,
            foreign_context_seal,
        )
        .is_err(),
        "a foreign verified height cannot authorize the retained Vote"
    );
    let (_, mut foreign_body_seal, validated, _) = recovered_next_wal_vote_seal_fixture(0x33);
    let manifest = runtime_manifest(verified.context(), 0x34);
    let foreign_durable = DurableBodyReceipt::for_test(
        verified.context().id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    foreign_body_seal
        .substitute_validated_for_test(ValidatedBodyReceipt::for_test(foreign_durable));
    assert!(
        project_recovered_lifecycle_next_wal_vote_candidate(&verified, foreign_body_seal).is_err(),
        "a substituted body receipt cannot authorize the retained Vote"
    );
    drop(validated);
    let (verified, mut foreign_wal_seal, _, _) = recovered_next_wal_vote_seal_fixture(0x35);
    foreign_wal_seal
        .substitute_wal_identity_for_test(RecoveredWalFrameIdentity::for_test(91, 92, [0xE1; 32]));
    assert!(
        project_recovered_lifecycle_next_wal_vote_candidate(&verified, foreign_wal_seal).is_err(),
        "a substituted WAL identity cannot authorize canonical replay evidence"
    );
    let (verified, mut foreign_effect_seal, _, mut foreign_effect) =
        recovered_next_wal_vote_seal_fixture(0x36);
    let AdapterEffect::Sign {
        request: SignRequest::Vote(vote),
        ..
    } = &mut foreign_effect
    else {
        unreachable!("fixture effect is a Vote Sign")
    };
    vote.subject = runtime_manifest(verified.context(), 0x37).subject;
    foreign_effect_seal.substitute_effect_for_test(foreign_effect);
    assert!(
        project_recovered_lifecycle_next_wal_vote_candidate(&verified, foreign_effect_seal)
            .is_err(),
        "a substituted Sign effect cannot reuse the retained replay evidence"
    );
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    recovered_next_wal_vote_projection_surface_is_affine_and_closed
);
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
        .exact_pending_adapter_effect_binding(&fetch)
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
    let recovered_store = validate_successor
        .project_validate_store_predecessor(&validate, &store)
        .expect("the exact Validate round-trips to its ordinal-free Store predecessor");
    assert_eq!(
        recovered_store, successor,
        "the inverse must recover the byte-identical causal binding"
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
    assert!(
        validate_successor
            .project_validate_store_predecessor(&validate, &wrong_tag)
            .is_none(),
        "the inverse cannot change the exact Store tag"
    );
    let wrong_round_store = AdapterEffect::StoreBody {
        tag,
        round: wire::ConsensusRound {
            view: manifest.round.view.saturating_add(1),
            ..manifest.round
        },
        subject: manifest.subject,
    };
    assert!(
        validate_successor
            .project_validate_store_predecessor(&validate, &wrong_round_store)
            .is_none(),
        "the inverse cannot change the exact Store round"
    );
    let wrong_subject_store = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: runtime_manifest(&context, 0x6B).subject,
    };
    assert!(
        validate_successor
            .project_validate_store_predecessor(&validate, &wrong_subject_store)
            .is_none(),
        "the inverse cannot change the exact Store subject"
    );
    let mut corrupted_statement_validate = successor
        .project_store_validate_successor(&store, &validate)
        .expect("derive a second exact Validate binding for corruption testing");
    corrupted_statement_validate.candidate_statement = None;
    assert!(
        corrupted_statement_validate
            .project_validate_store_predecessor(&validate, &store)
            .is_none(),
        "the inverse cannot accept a binding whose authority statement lost integrity"
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
        validate_successor
            .project_validate_store_predecessor(&wrong_validate_tag, &store)
            .is_none(),
        "the inverse cannot substitute another Validate tag"
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
    .exact_pending_adapter_effect_binding(&store)
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
    .exact_pending_adapter_effect_binding(&fetch)
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
    let rebound_tag = EventTag::new(
        context.height,
        tag.view()
            .checked_add(1)
            .expect("fixture view remains bounded"),
        tag.generation(),
    );
    let rebound_apply = AdapterEffect::Apply {
        tag: rebound_tag,
        subject: commit.subject,
        certificate: commit.clone(),
    };
    let rebound = prepare_validate
        .project_validate_apply_successor(&validate, &rebound_apply)
        .expect("a later-view Apply may consume the exact stale Validate completion");
    assert!(rebound.exactly_binds_adapter_effect(&rebound_apply));
    assert_eq!(
        rebound.causal_lifecycle_key(),
        prepare_validate.causal_lifecycle_key(),
        "view rebinding cannot change the physical lifecycle root"
    );
    let wrong_rebound_generation = AdapterEffect::Apply {
        tag: EventTag::new(
            context.height,
            rebound_tag.view(),
            Generation::new(
                rebound_tag
                    .generation()
                    .get()
                    .checked_add(1)
                    .expect("fixture generation remains bounded"),
            ),
        ),
        subject: commit.subject,
        certificate: commit.clone(),
    };
    assert!(
        prepare_validate
            .project_validate_apply_successor(&validate, &wrong_rebound_generation)
            .is_none(),
        "a later view cannot launder another local generation"
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
        wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
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
fn live_wal_payload_free_pending_roots_bind_all_five_stages_and_exact_frames() {
    let (context, keys) = authenticated_runtime_context();
    let wire::ConsensusMessageV2Payload::Proposal(mut proposal) =
        signed_runtime_proposal(&context, &keys, 0x68).payload
    else {
        unreachable!("runtime proposal fixture")
    };
    proposal.signature.clear();
    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x69,
        wire::GlobalPhase::Prepare,
    );
    let tag = EventTag::new(context.height, 0, Generation::new(9));
    let prepare_vote = wire::Vote {
        round: prepare.round,
        proposal_round: prepare.proposal_round,
        phase: wire::GlobalPhase::Prepare,
        subject: prepare.subject,
        execution_commitment: prepare.execution_commitment,
        signer: prepare.signers[0],
        signature: Vec::new(),
    };
    let commit_vote = wire::Vote {
        phase: wire::GlobalPhase::Commit,
        ..prepare_vote.clone()
    };
    let timeout_vote = wire::TimeoutVote {
        round: prepare.round,
        highest_prepare_qc: Some(prepare),
        signer: 0,
        signature: Vec::new(),
    };
    let enter = wire::TimeoutCertificate {
        round: timeout_vote.round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: timeout_vote.highest_prepare_qc.clone(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x69; 96],
        }],
    };
    let effects = [
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Proposal(proposal),
        },
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(prepare_vote),
        },
        AdapterEffect::Sign {
            tag,
            request: SignRequest::Vote(commit_vote),
        },
        AdapterEffect::Sign {
            tag,
            request: SignRequest::TimeoutVote(timeout_vote),
        },
        AdapterEffect::EnterView {
            tag,
            certificate: enter,
            protected_lock: None,
        },
    ];
    let mut roots = Vec::new();
    for (index, effect) in effects.iter().enumerate() {
        let sequence = u64::try_from(index).expect("small stage index");
        let frame_hash = if index == 0 {
            [0; 32]
        } else {
            [u8::try_from(index).expect("small stage marker"); 32]
        };
        let identity = LiveWalFrameIdentity::for_test(
            sequence,
            sequence.checked_add(1).expect("bounded persistence id"),
            frame_hash,
        );
        let pending = PendingRuntimeEffectBinding::from_exact_live_wal_append(&identity, effect)
            .expect("exact payload-free live WAL effect derives one pending owner");
        assert!(pending.exactly_binds_adapter_effect(effect));
        roots.push(*pending.causal_lifecycle_key());
    }
    assert_eq!(roots.iter().collect::<BTreeSet<_>>().len(), effects.len());
    let first = LiveWalFrameIdentity::for_test(9, 10, [0; 32]);
    let second = LiveWalFrameIdentity::for_test(10, 11, [0; 32]);
    let first_pending =
        PendingRuntimeEffectBinding::from_exact_live_wal_append(&first, &effects[0])
            .expect("zero-valued digest remains structurally valid");
    let second_pending =
        PendingRuntimeEffectBinding::from_exact_live_wal_append(&second, &effects[0])
            .expect("second exact locator derives a pending owner");
    assert_ne!(
        first_pending.causal_lifecycle_key(),
        second_pending.causal_lifecycle_key(),
        "identical effects from different exact WAL frames cannot share causal authority"
    );
}
#[test]
fn pending_validate_projects_exact_prepare_commit_and_report_successors() {
    let (context, keys) = authenticated_runtime_context();
    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6B,
        wire::GlobalPhase::Prepare,
    );
    let tag = EventTag::new(context.height, prepare.round.view, Generation::new(4));
    let (validate, ordinary_validate) =
        pending_validate_binding_for_test(tag, prepare.proposal_round, prepare.subject, None, 74);
    let (_, prepare_validate) = pending_validate_binding_for_test(
        tag,
        prepare.proposal_round,
        prepare.subject,
        Some(prepare.clone()),
        75,
    );
    let prepare_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Prepare,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    let prepare_sign_binding = ordinary_validate
        .project_validate_sign_prepare_successor(&validate, &prepare_sign)
        .expect("ordinary Validate acquires exact Prepare-vote authority");
    assert!(prepare_sign_binding.exactly_binds_adapter_effect(&prepare_sign));
    assert_eq!(
        prepare_sign_binding.causal_lifecycle_key(),
        ordinary_validate.causal_lifecycle_key()
    );
    let prepare_statement = prepare_sign_binding
        .candidate_statement()
        .expect("Prepare Sign carries one candidate statement");
    assert_eq!(prepare_statement.phase(), Some(wire::GlobalPhase::Prepare));
    assert_eq!(
        prepare_statement.execution_commitment(),
        Some(prepare.execution_commitment)
    );
    let commit_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    let commit_sign_binding = prepare_validate
        .project_validate_sign_commit_successor(&validate, &commit_sign)
        .expect("Prepare-authorized Validate promotes to exact Commit vote");
    assert!(commit_sign_binding.exactly_binds_adapter_effect(&commit_sign));
    assert_eq!(
        commit_sign_binding.causal_lifecycle_key(),
        prepare_validate.causal_lifecycle_key()
    );
    let commit_statement = commit_sign_binding
        .candidate_statement()
        .expect("Commit Sign carries one candidate statement");
    assert_eq!(commit_statement.phase(), Some(wire::GlobalPhase::Commit));
    assert_eq!(
        commit_statement.execution_commitment(),
        Some(prepare.execution_commitment)
    );
    let report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: prepare.subject,
        certificate: prepare,
    };
    let report_binding = prepare_validate
        .project_validate_report_invalid_certified_body_successor(&validate, &report)
        .expect("Prepare-authorized Validate derives its exact invalid-body report");
    assert!(report_binding.exactly_binds_adapter_effect(&report));
    assert_eq!(
        report_binding.causal_lifecycle_key(),
        prepare_validate.causal_lifecycle_key()
    );
    assert_eq!(report_binding.candidate_statement(), None);
    assert_ne!(
        report_binding.exact_effect_identity(),
        prepare_validate.exact_effect_identity()
    );
}
#[test]
fn pending_sign_projects_only_its_exact_signed_broadcast_successor() {
    let (context, keys) = authenticated_runtime_context();
    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6D,
        wire::GlobalPhase::Prepare,
    );
    let tag = EventTag::new(context.height, prepare.round.view, Generation::new(4));
    let (validate, validate_pending) =
        pending_validate_binding_for_test(tag, prepare.proposal_round, prepare.subject, None, 76);
    let unsigned_vote = wire::Vote {
        round: prepare.round,
        proposal_round: prepare.proposal_round,
        phase: wire::GlobalPhase::Prepare,
        subject: prepare.subject,
        execution_commitment: prepare.execution_commitment,
        signer: prepare.signers[0],
        signature: Vec::new(),
    };
    let sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(unsigned_vote.clone()),
    };
    let sign_pending = validate_pending
        .project_validate_sign_prepare_successor(&validate, &sign)
        .expect("Validate projects its exact unsigned Prepare vote");
    let mut signed_vote = unsigned_vote.clone();
    signed_vote.signature = vec![0xD6; 96];
    let broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(signed_vote.clone()),
    ));
    let broadcast_pending = sign_pending
        .project_signed_broadcast_successor(&sign, &broadcast)
        .expect("the signed copy of the exact request projects one Broadcast owner");
    assert!(broadcast_pending.exactly_binds_adapter_effect(&broadcast));
    assert_eq!(
        broadcast_pending.causal_lifecycle_key(),
        sign_pending.causal_lifecycle_key()
    );
    assert_eq!(broadcast_pending.candidate_statement(), None);
    assert_ne!(
        broadcast_pending.exact_effect_identity(),
        sign_pending.exact_effect_identity()
    );
    signed_vote.signature.clear();
    let unsigned_broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(signed_vote.clone()),
    ));
    assert!(
        sign_pending
            .project_signed_broadcast_successor(&sign, &unsigned_broadcast)
            .is_none(),
        "an unsigned envelope is not a completed Sign successor"
    );
    signed_vote.signature = vec![0xD6; 96];
    signed_vote.subject.payload_hash = Hash::new(b"foreign signed subject");
    let foreign_broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(signed_vote),
    ));
    assert!(
        sign_pending
            .project_signed_broadcast_successor(&sign, &foreign_broadcast)
            .is_none(),
        "a signature cannot authorize changed consensus coordinates"
    );
}
#[test]
fn recovered_commit_retags_monotonically_without_widening_live_projection() {
    let (context, keys) = authenticated_runtime_context();
    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6C,
        wire::GlobalPhase::Prepare,
    );
    let original_tag = EventTag::new(context.height, prepare.round.view, Generation::new(12));
    let current_tag = EventTag::new(
        context.height,
        prepare.round.view + 1,
        original_tag.generation(),
    );
    let (validate, ordinary_validate) = pending_validate_binding_for_test(
        original_tag,
        prepare.proposal_round,
        prepare.subject,
        None,
        76,
    );
    let (_, prepare_validate) = pending_validate_binding_for_test(
        original_tag,
        prepare.proposal_round,
        prepare.subject,
        Some(prepare.clone()),
        77,
    );
    let historical_commit = AdapterEffect::Sign {
        tag: current_tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&validate, &historical_commit)
            .is_none(),
        "the live inherited-Prepare projection retains exact tag equality"
    );
    assert!(
        prepare_validate
            .project_recovered_inherited_validate_commit_successor(
                &validate,
                &historical_commit,
                &prepare,
            )
            .is_some(),
        "sealed recovery may emit the exact old Commit under a later current tag"
    );
    assert!(
        ordinary_validate
            .project_recovered_ordinary_validate_commit_successor(
                &validate,
                &historical_commit,
                &prepare,
            )
            .is_some(),
        "the recovered ordinary-Validate refinement uses the same bounded relation"
    );
    let AdapterEffect::Sign { request, .. } = &historical_commit else {
        unreachable!("historical Commit fixture is a Sign effect")
    };
    let foreign_generation_commit = AdapterEffect::Sign {
        tag: EventTag::new(
            current_tag.height(),
            current_tag.view(),
            Generation::new(current_tag.generation().get() + 1),
        ),
        request: request.clone(),
    };
    assert!(
        prepare_validate
            .project_recovered_inherited_validate_commit_successor(
                &validate,
                &foreign_generation_commit,
                &prepare,
            )
            .is_none(),
        "recovery cannot cross a reducer generation"
    );
}
#[test]
fn pending_validate_successor_projection_rejects_forged_coordinates_and_authority() {
    let (context, keys) = authenticated_runtime_context();
    let prepare = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6C,
        wire::GlobalPhase::Prepare,
    );
    let commit = signed_runtime_quorum_certificate(&context, &keys, 0x6C);
    let tag = EventTag::new(context.height, prepare.round.view, Generation::new(5));
    let (validate, ordinary_validate) =
        pending_validate_binding_for_test(tag, prepare.proposal_round, prepare.subject, None, 76);
    let (_, prepare_validate) = pending_validate_binding_for_test(
        tag,
        prepare.proposal_round,
        prepare.subject,
        Some(prepare.clone()),
        77,
    );
    let store = AdapterEffect::StoreBody {
        tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
    };
    let prepare_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Prepare,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    let commit_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    let report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: prepare.subject,
        certificate: prepare.clone(),
    };
    assert!(
        ordinary_validate
            .project_validate_sign_prepare_successor(&validate, &commit_sign)
            .is_none(),
        "Prepare projection rejects a Commit vote"
    );
    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&validate, &prepare_sign)
            .is_none(),
        "Commit projection rejects a Prepare vote"
    );
    let commit_report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: commit.subject,
        certificate: commit,
    };
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(&validate, &commit_report,)
            .is_none(),
        "invalid-body reporting requires Prepare rather than Commit authority"
    );
    let foreign = signed_runtime_quorum_certificate_for_phase(
        &context,
        &keys,
        0x6D,
        wire::GlobalPhase::Prepare,
    );
    let changed_commitment_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: prepare.subject,
            execution_commitment: foreign.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&validate, &changed_commitment_sign)
            .is_none(),
        "Commit vote cannot change the registered Prepare commitment"
    );
    let mut changed_commitment_certificate = prepare.clone();
    changed_commitment_certificate.execution_commitment = foreign.execution_commitment;
    let changed_commitment_report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: prepare.subject,
        certificate: changed_commitment_certificate,
    };
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(
                &validate,
                &changed_commitment_report,
            )
            .is_none(),
        "report cannot change the registered Prepare commitment"
    );
    let wrong_tag_prepare = AdapterEffect::Sign {
        tag: EventTag::new(context.height, prepare.round.view, Generation::new(6)),
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Prepare,
            subject: prepare.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    assert!(
        ordinary_validate
            .project_validate_sign_prepare_successor(&validate, &wrong_tag_prepare)
            .is_none(),
        "Sign successor must retain the complete predecessor tag"
    );
    let wrong_tag_validate = AdapterEffect::ValidateBody {
        tag: EventTag::new(context.height, prepare.round.view, Generation::new(6)),
        round: prepare.proposal_round,
        subject: prepare.subject,
    };
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(&wrong_tag_validate, &report,)
            .is_none(),
        "report projection requires the exactly bound Validate tag"
    );
    let wrong_subject_sign = AdapterEffect::Sign {
        tag,
        request: SignRequest::Vote(wire::Vote {
            round: prepare.round,
            proposal_round: prepare.proposal_round,
            phase: wire::GlobalPhase::Commit,
            subject: foreign.subject,
            execution_commitment: prepare.execution_commitment,
            signer: prepare.signers[0],
            signature: Vec::new(),
        }),
    };
    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&validate, &wrong_subject_sign)
            .is_none(),
        "Sign successor cannot change the validated subject"
    );
    let mut wrong_subject_certificate = prepare.clone();
    wrong_subject_certificate.subject = foreign.subject;
    let wrong_subject_report = AdapterEffect::ReportInvalidCertifiedBody {
        subject: foreign.subject,
        certificate: wrong_subject_certificate,
    };
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(
                &validate,
                &wrong_subject_report,
            )
            .is_none(),
        "report cannot change the validated subject"
    );
    assert!(
        ordinary_validate
            .project_validate_sign_prepare_successor(&store, &prepare_sign)
            .is_none(),
        "Store cannot stand in for the exact Validate predecessor"
    );
    assert!(
        prepare_validate
            .project_validate_sign_commit_successor(&store, &commit_sign)
            .is_none(),
        "Store cannot stand in for the exact Validate predecessor"
    );
    assert!(
        prepare_validate
            .project_validate_report_invalid_certified_body_successor(&store, &report)
            .is_none(),
        "Store cannot stand in for the exact Validate predecessor"
    );
    assert!(
        ordinary_validate
            .project_validate_sign_commit_successor(&validate, &commit_sign)
            .is_none(),
        "ordinary Validate needs an opaque concurrent-Prepare refinement capability"
    );
    assert!(
        ordinary_validate
            .project_validate_report_invalid_certified_body_successor(&validate, &report)
            .is_none(),
        "ordinary Validate needs an opaque registered-report carrier capability"
    );
    assert!(
        prepare_validate
            .project_validate_sign_prepare_successor(&validate, &prepare_sign)
            .is_none(),
        "Prepare-authorized Validate cannot regress to the ordinary Prepare-sign branch"
    );
}
