use super::*;
use crate::sumeragi::{
    FairV2Ingress, FairV2IngressPushDisposition, InboundBlockMessage,
    fair_v2_ingress_admit_with_roster_for_test,
    v2::AdapterEffect,
    v2_block_sync::tests::durable_history_fixture,
    v2_body_store::DurableBodyReceipt,
    v2_chunks::encode_payload,
    v2_core::MAX_EFFECTS_PER_STEP,
    v2_effects::EffectQueueConfig,
    v2_lane_work::tests::{
        durable_lane_history_fixture, historical_autonomous_lane_certificate_fixture,
    },
    v2_runtime::{
        BodyAvailableReservation, DecisionProposalRetirement, EnqueueError,
        LocalProposalEffectOwnership, RetiredBodyPipelineCompletions, RuntimeEffectOwnership,
        RuntimeLifecycleOwner, RuntimeStep, bind_adapter_effect_batch_ownership,
    },
    v2_transport::{authenticate_certified_body_request, authenticate_payload_chunk},
};
#[cfg(feature = "bls")]
use crate::sumeragi::{
    v2::{
        AdapterFingerprints, DeferredAdmissionOrdinalSource, SignRequest, SumeragiV2Adapter,
        VerifiedHeightContext,
    },
    v2_body_store::BlockSignaturePolicy,
    v2_effects::EffectExecutorStep,
    v2_runtime::{RuntimeQueueConfig, SerializedV2Runtime},
};
use crate::{
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
use iroha_data_model::{
    DataSpaceId, LaneId,
    block::{
        BlockHeader, BlockSignature, CertifiedMergeLedgerReference, SignedBlock,
        consensus::{
            CertPhase, LaneBlockDescriptorV1, LaneBlockProposalV1, LaneBlockQcV1,
            LaneBlockVoteBodyV1,
        },
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    merge::{
        LaneDrainCertificateBodyV1, LaneDrainIntentV1, MergeLedgerEntry, MergeQuorumCertificate,
    },
};
use mv::storage::StorageReadOnly;
use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tempfile::TempDir;
#[test]
fn orphan_chunk_budget_obeys_encoded_payload_ceiling() {
    let maximum = wire::DataAvailabilityLayout {
        encoding: wire::PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: wire::MAX_DA_CHUNK_SIZE_BYTES,
        data_shards: 4,
        parity_shards: 2,
        max_payload_size_bytes: wire::MAX_DA_PAYLOAD_SIZE_BYTES,
        max_chunk_count: wire::MAX_DA_CHUNK_COUNT,
    };
    assert_eq!(
        maximum_orphan_chunk_bytes(maximum),
        wire::MAX_DA_ENCODED_PAYLOAD_BYTES
    );
    let small = wire::DataAvailabilityLayout {
        chunk_size_bytes: 8,
        max_chunk_count: 4,
        ..maximum
    };
    assert_eq!(maximum_orphan_chunk_bytes(small), 32);
}
fn test_io_command_channel(
    capacity: usize,
) -> (V2IoCommandSender, V2IoCommandReceiver, Arc<V2IoAdmission>) {
    let admission = V2IoAdmission::unbounded_for_tests();
    let (sender, receiver) = v2_io_command_channel(
        capacity,
        capacity.max(1),
        capacity.max(1),
        capacity.max(1),
        Arc::clone(&admission),
    );
    (sender, receiver, admission)
}
fn authenticated_serve_request(
    context: &wire::HeightContext,
    requester_key: &KeyPair,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    phase: wire::GlobalPhase,
) -> AuthenticatedCertifiedBodyRequest {
    let mut request = wire::CertifiedBodyRequest {
        round,
        subject,
        certificate: wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"Serve fixture parent state"),
                Hash::new(b"Serve fixture post state"),
                Hash::new(b"Serve fixture ordinary writes"),
                1,
                Hash::new(b"Serve fixture executed block"),
            ),
            signers: (0..super::super::network_topology::commit_quorum_from_len(
                context.roster.len(),
            ))
                .map(|index| u32::try_from(index).expect("fixture roster index fits u32"))
                .collect(),
            aggregate_signature: vec![0xA5; 48],
        },
        requester: PeerId::new(requester_key.public_key().clone()),
        signature: Vec::new(),
    };
    request.signature = Signature::new(requester_key.private_key(), &request.signature_preimage())
        .payload()
        .to_vec();
    let requester = request.requester.clone();
    authenticate_certified_body_request(context, request, &requester, |_, _| {
        Ok::<(), &'static str>(())
    })
    .expect("authenticate certified Serve fixture")
}
pub(in crate::sumeragi) fn production_authenticated_serve_request(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    requester_key: &KeyPair,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    phase: wire::GlobalPhase,
    signer_indices: &[usize],
) -> (AuthenticatedCertifiedBodyRequest, Vec<Vec<u8>>) {
    let exact_quorum = super::super::network_topology::commit_quorum_from_len(context.roster.len());
    let signer_indices = signer_indices
        .get(..exact_quorum)
        .expect("production Serve fixture supplies an exact quorum");
    let mut request = authenticated_serve_request(context, requester_key, round, subject, phase)
        .request()
        .clone();
    let signers = signer_indices
        .iter()
        .map(|index| u32::try_from(*index).expect("fixture signer index fits u32"))
        .collect::<Vec<_>>();
    let vote_preimage = wire::Vote {
        round: request.certificate.round,
        proposal_round: request.certificate.proposal_round,
        phase: request.certificate.phase,
        subject: request.certificate.subject,
        execution_commitment: request.certificate.execution_commitment,
        signer: *signers
            .first()
            .expect("production Serve fixture has a signer"),
        signature: Vec::new(),
    }
    .signature_preimage();
    let signature_shares = signer_indices
        .iter()
        .map(|index| {
            Signature::new(keys[*index].private_key(), &vote_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_share_refs = signature_shares
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    request.certificate.signers = signers;
    request.certificate.aggregate_signature =
        iroha_crypto::bls_normal_aggregate_signatures(&signature_share_refs)
            .expect("aggregate production Serve fixture certificate");
    request.signature = Signature::new(requester_key.private_key(), &request.signature_preimage())
        .payload()
        .to_vec();
    let requester = request.requester.clone();
    let authenticated =
        authenticate_certified_body_request(context, request, &requester, |_, _| {
            Ok::<(), &'static str>(())
        })
        .expect("authenticate production Serve fixture after certificate replacement");
    let validator_pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("fixture validator proof of possession")
        })
        .collect();
    (authenticated, validator_pops)
}
fn certified_serve_response(
    request: &AuthenticatedCertifiedBodyRequest,
    manifest: wire::PayloadManifest,
    body: Vec<u8>,
    responder_key: &KeyPair,
) -> wire::CertifiedBodyResponse {
    let mut response = wire::CertifiedBodyResponse {
        request_hash: request.request_hash(),
        manifest,
        body,
        responder: 0,
        signature: Vec::new(),
    };
    response.signature =
        Signature::new(responder_key.private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
    response
}
pub(in crate::sumeragi) fn certified_serve_inbound(
    request: &wire::CertifiedBodyRequest,
    authenticated_via: PeerId,
) -> InboundBlockMessage {
    let mut routes = NetworkReplyRouteTestFixture::new(authenticated_via.clone());
    let route = routes.mint_via(request.requester.clone(), authenticated_via.clone());
    InboundBlockMessage::try_from_transport_with_reply_route(
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
        )),
        request.requester.clone(),
        authenticated_via,
        route,
    )
    .expect("fixture exact Serve route matches requester and authenticated hop")
}
struct SaturatedCompletionRuntime {
    queued: usize,
    capacity: usize,
    next_lifecycle_ordinal: u128,
    effect_owners: BTreeMap<Hash, crate::sumeragi::v2_runtime::RuntimeEffectOwnerAssignment>,
    external_lifecycle_owners: Vec<RuntimeLifecycleOwner>,
    external_lifecycle_owner_capacity: Option<usize>,
}
impl SaturatedCompletionRuntime {
    fn new(queued: usize, capacity: usize) -> Self {
        Self {
            queued,
            capacity,
            next_lifecycle_ordinal: 1,
            effect_owners: BTreeMap::new(),
            external_lifecycle_owners: Vec::new(),
            external_lifecycle_owner_capacity: None,
        }
    }
    fn reject_completion() -> Result<(), EnqueueError> {
        Err(EnqueueError::Full)
    }
    fn effect_ownership(
        &mut self,
        effect: &AdapterEffect,
    ) -> Result<crate::sumeragi::v2_runtime::RuntimeEffectOwnerAssignment, String> {
        let mut identity = Vec::new();
        let tag = match effect {
            AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                ..
            }
            | AdapterEffect::StoreBody {
                tag,
                round,
                subject,
            }
            | AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } => {
                identity.extend_from_slice(b"body-pipeline");
                identity.extend_from_slice(&round.encode());
                identity.extend_from_slice(&subject.encode());
                *tag
            }
            AdapterEffect::Sign { tag, request } => {
                identity.extend_from_slice(b"sign");
                identity.extend_from_slice(&request.signature_preimage());
                *tag
            }
            AdapterEffect::Apply {
                tag,
                subject,
                certificate,
            } => {
                identity.extend_from_slice(b"apply");
                identity.extend_from_slice(&subject.encode());
                identity.extend_from_slice(&certificate.as_ref().encode());
                *tag
            }
            AdapterEffect::Broadcast(message) => {
                identity.extend_from_slice(b"broadcast");
                identity.extend_from_slice(&message.encode());
                EventTag::new(1, 0, Generation::new(0))
            }
            AdapterEffect::EnterView { tag, .. } => {
                identity.extend_from_slice(b"enter-view");
                identity.extend_from_slice(&tag.height().to_le_bytes());
                identity.extend_from_slice(&tag.view().to_le_bytes());
                *tag
            }
            AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => {
                identity.extend_from_slice(format!("{effect:?}").as_bytes());
                EventTag::new(1, 0, Generation::new(0))
            }
        };
        self.ownership_for_identity(tag, Hash::new(identity))
    }
    fn ownership_for_identity(
        &mut self,
        tag: EventTag,
        identity: Hash,
    ) -> Result<crate::sumeragi::v2_runtime::RuntimeEffectOwnerAssignment, String> {
        if let Some(existing) = self.effect_owners.get(&identity) {
            return Ok(existing.clone());
        }
        let next_lifecycle_ordinal = self
            .next_lifecycle_ordinal
            .checked_add(1)
            .ok_or_else(|| "saturated runtime lifecycle-owner ordinal overflowed".to_owned())?;
        let ownership = RuntimeEffectOwnership::fresh_for_test(tag, self.next_lifecycle_ordinal);
        self.next_lifecycle_ordinal = next_lifecycle_ordinal;
        self.effect_owners.insert(identity, ownership.clone());
        Ok(ownership)
    }
}
impl EffectRuntime for SaturatedCompletionRuntime {
    fn step_effects(&mut self, _now: Instant) -> Result<RuntimeStep<AdapterEffect>, String> {
        Ok(RuntimeStep::Idle)
    }
    fn step_recovery_effects(
        &mut self,
        _now: Instant,
    ) -> Result<RuntimeStep<AdapterEffect>, String> {
        Err("synthetic runtime cannot drive pending-tip recovery".to_owned())
    }
    fn take_effect_ownership(
        &mut self,
        effects: &[AdapterEffect],
    ) -> Result<Vec<RuntimeEffectOwnership>, String> {
        let ownership = effects
            .iter()
            .map(|effect| self.effect_ownership(effect))
            .collect::<Result<Vec<_>, _>>()?;
        bind_adapter_effect_batch_ownership(effects, ownership)
    }
    fn take_leader_wire_runtime_terminals(
        &mut self,
    ) -> Result<Vec<LeaderWireRuntimeTerminal>, String> {
        Ok(Vec::new())
    }
    fn set_external_lifecycle_owners(
        &mut self,
        owners: Vec<RuntimeLifecycleOwner>,
    ) -> Result<(), String> {
        let capacity = self.external_lifecycle_owner_capacity.ok_or_else(|| {
            "saturated test runtime external-owner capacity is not configured".to_owned()
        })?;
        if owners.len() > capacity || owners.iter().any(|owner| owner.lifecycle_ordinal() == 0) {
            return Err(
                "saturated test runtime external lifecycle ownership is invalid".to_owned(),
            );
        }
        let mut exact_by_ordinal = BTreeMap::new();
        for owner in &owners {
            if exact_by_ordinal
                .insert(owner.lifecycle_ordinal(), owner)
                .is_some()
            {
                return Err(
                    "saturated test runtime external lifecycle ownership is not unique".to_owned(),
                );
            }
        }
        self.external_lifecycle_owners = owners;
        Ok(())
    }
    fn configure_external_lifecycle_owner_capacity(
        &mut self,
        max_pending_work: usize,
    ) -> Result<(), String> {
        let retained_capacity = MAX_EFFECTS_PER_STEP.checked_mul(2).ok_or_else(|| {
            "saturated test runtime external-owner capacity overflowed".to_owned()
        })?;
        let capacity = max_pending_work
            .checked_add(retained_capacity)
            .ok_or_else(|| {
                "saturated test runtime external-owner capacity overflowed".to_owned()
            })?;
        if max_pending_work == 0 || self.external_lifecycle_owners.len() > capacity {
            return Err("saturated test runtime external-owner capacity is invalid".to_owned());
        }
        self.external_lifecycle_owner_capacity = Some(capacity);
        Ok(())
    }
    fn mint_local_proposal_effect_ownership(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<LocalProposalEffectOwnership, String> {
        let mut identity = Vec::from(b"body-pipeline".as_slice());
        identity.extend_from_slice(&manifest.round.encode());
        identity.extend_from_slice(&manifest.subject.encode());
        let ownership = self.ownership_for_identity(tag, Hash::new(identity))?;
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let raw = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&store_effect),
            vec![ownership],
        )?
        .pop()
        .ok_or_else(|| "saturated local proposal StoreBody binding was empty".to_owned())?;
        LocalProposalEffectOwnership::for_test(raw, &store_effect, manifest).ok_or_else(|| {
            "saturated local proposal StoreBody replay seal did not match its owner".to_owned()
        })
    }
    fn reconcile_active_view_producer(
        &mut self,
        _tag: EventTag,
        _retain: bool,
    ) -> Result<(), String> {
        Ok(())
    }
    fn complete_active_view_producer_after_proposal_fanout(
        &mut self,
        _proposal_round: wire::ConsensusRound,
        _ownership: &RuntimeEffectOwnership,
    ) -> Result<(), String> {
        Ok(())
    }
    fn take_scheduler_ownership(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn authoritative_tag(&self) -> Option<EventTag> {
        None
    }
    fn decided_body(
        &self,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        String,
    > {
        Ok(None)
    }
    fn reserve_body_available(
        &mut self,
        _tag: EventTag,
        _manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        Err(EnqueueError::Full)
    }
    fn commit_body_available(
        &mut self,
        _reservation: BodyAvailableReservation,
    ) -> Result<(), EnqueueError> {
        Ok(())
    }
    fn abort_body_available(&mut self, _reservation: BodyAvailableReservation) {}
    fn rebind_body_available(
        &mut self,
        _previous: EventTag,
        _rebound: EventTag,
        _manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        Ok(false)
    }
    fn rebind_unpublished_body_available(
        &mut self,
        _previous: EventTag,
        _rebound: EventTag,
        _round: wire::ConsensusRound,
        _subject: wire::BlockSubject,
    ) -> Result<bool, String> {
        Ok(false)
    }
    fn retire_unpublished_body_available(
        &mut self,
        _tag: EventTag,
        _round: wire::ConsensusRound,
        _subject: wire::BlockSubject,
    ) -> Result<bool, String> {
        Ok(false)
    }
    fn retire_body_available(
        &mut self,
        _tag: EventTag,
        _manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        Ok(false)
    }
    fn retire_body_pipeline_completions(
        &mut self,
        _tag: EventTag,
        _round: wire::ConsensusRound,
        _subject: wire::BlockSubject,
    ) -> Result<RetiredBodyPipelineCompletions, String> {
        Ok(RetiredBodyPipelineCompletions::default())
    }
    fn retire_unsafe_proposals_for_lock(
        &mut self,
        _locked_round: wire::ConsensusRound,
        _locked_subject: wire::BlockSubject,
    ) -> Result<usize, String> {
        Ok(0)
    }
    fn retire_proposal_work_after_decision(
        &mut self,
        _decision_round: wire::ConsensusRound,
        _decision_subject: wire::BlockSubject,
        _decision_commitment: wire::ExecutionCommitment,
    ) -> Result<DecisionProposalRetirement, String> {
        Ok(DecisionProposalRetirement::default())
    }
    fn enqueue_body_stored(
        &mut self,
        _tag: EventTag,
        _round: wire::ConsensusRound,
        _subject: wire::BlockSubject,
        _receipt: DurableBodyReceipt,
    ) -> Result<(), EnqueueError> {
        Self::reject_completion()
    }
    fn enqueue_signature(
        &mut self,
        _tag: EventTag,
        _signature: Vec<u8>,
    ) -> Result<(), EnqueueError> {
        Self::reject_completion()
    }
    fn enqueue_application_completed(
        &mut self,
        _tag: EventTag,
        _subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        Self::reject_completion()
    }
    fn verify_certificate(
        &self,
        _context: &wire::HeightContext,
        _certificate: &wire::QuorumCertificate,
    ) -> Result<(), String> {
        Ok(())
    }
    fn authenticate_certified_body_request(
        &self,
        context: &wire::HeightContext,
        request: wire::CertifiedBodyRequest,
        authenticated_requester: &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyRequest, crate::sumeragi::v2_transport::V2TransportError>
    {
        authenticate_certified_body_request(
            context,
            request,
            authenticated_requester,
            |context, certificate| self.verify_certificate(context, certificate),
        )
    }
    fn queued_commands(&self) -> usize {
        self.queued
    }
    fn remaining_completion_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.queued)
    }
    fn queue_snapshot(&self, _now: Instant) -> RuntimeQueueSnapshot {
        let empty = RuntimeQueueLaneSnapshot {
            depth: 0,
            capacity: self.capacity,
            oldest_age: None,
            max_service_debt: 0,
        };
        RuntimeQueueSnapshot {
            normal: empty,
            progress: empty,
            completion: RuntimeQueueLaneSnapshot {
                depth: self.queued,
                oldest_age: (self.queued != 0).then_some(Duration::ZERO),
                ..empty
            },
        }
    }
    fn watchdog_threshold(&self) -> Duration {
        Duration::from_secs(1)
    }
}
#[test]
fn saturated_completion_runtime_preserves_bounded_body_pipeline_ownership() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let manifest = proposal.manifest;
    let tag = EventTag::new(
        service.context.height,
        manifest.round.view,
        Generation::new(service.context.height),
    );
    let mut runtime = SaturatedCompletionRuntime::new(0, 1);
    runtime
        .configure_external_lifecycle_owner_capacity(1)
        .expect("configure bounded external owners");
    let proposal_ownership = runtime
        .mint_local_proposal_effect_ownership(tag, &manifest)
        .expect("mint local proposal owner");
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let proposal_owner = proposal_ownership
        .exact_store_task_ownership(&store_effect, &manifest)
        .expect("project the exact local Store task owner");
    let fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let fetch_owner = runtime
        .take_effect_ownership(&[fetch])
        .expect("derive positional fetch owner")
        .pop()
        .expect("one effect has one owner");
    assert_eq!(fetch_owner, proposal_owner);
    runtime
        .set_external_lifecycle_owners(vec![proposal_owner.owner().clone()])
        .expect("one external owner fits");
    assert_eq!(runtime.external_lifecycle_owners.len(), 1);
    assert!(
        runtime
            .set_external_lifecycle_owners(vec![
                proposal_owner.owner().clone();
                MAX_EFFECTS_PER_STEP + 2
            ])
            .is_err()
    );
    assert_eq!(
        runtime.external_lifecycle_owners.len(),
        1,
        "rejected publication must preserve the prior bounded owner set"
    );
}
/// Build closed-network production services for sibling runner tests.
pub(in crate::sumeragi) fn fixture() -> (ProductionV2Services, Vec<KeyPair>) {
    let (exact_output_handoff_owner, _) = durable_exact_output_handoff_owner_pair();
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic validator key")
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
        network_id: crate::sumeragi::synthetic_network_id("v2-worker-test"),
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        epoch_end_height: u64::MAX,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: wire::DualQuorum::from_roster(&roster).expect("equal-vote quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"v2-worker-test-context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 32,
            max_chunk_count: 8,
        },
        leader_seed: [0x33; 32],
    };
    context.validate().expect("valid context");
    let active_tag = EventTag::new(context.height, 0, Generation::new(context.height));
    let leader_wire_recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            context.id(),
            context.height,
            [0xF4; 32],
            active_tag.view(),
            false,
        );
    let local_peer = context.roster[0].validator.clone();
    let frozen_semantic_targets = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let validator_set_pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("worker fixture validator PoP")
        })
        .collect::<Vec<_>>();
    let kura = Kura::blank_kura_for_testing();
    let state = Arc::new(State::new_with_chain_and_network_id_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        iroha_data_model::ChainId::from("sumeragi-v2-worker-display-name"),
        context.network_id,
    ));
    let kura_replica_advert_refresh = Arc::new(
        KuraReplicaAdvertRefreshOwner::from_kura(kura.as_ref(), Instant::now())
            .expect("valid test Kura replica advert refresh owner"),
    );
    let service = ProductionV2Services {
        context,
        validator_set_pops,
        state,
        local_peer,
        local_validator: Some(0),
        key_pair: keys[0].clone(),
        network: crate::IrohaNetwork::closed_for_tests(),
        kura,
        chunk_root: PathBuf::new(),
        io: None,
        lifecycle_body_store_identity: None,
        lifecycle_payload_store_identity: None,
        fetches: BTreeMap::new(),
        fetch_by_manifest: BTreeMap::new(),
        orphan_chunks: BTreeMap::new(),
        orphan_chunk_count: 0,
        orphan_chunk_bytes: 0,
        orphan_lifecycle_sweep_cursor: None,
        max_orphan_chunks: 1,
        max_orphan_chunk_bytes: 32,
        max_merge_sidecar_deferrals: 1,
        local_completions: VecDeque::new(),
        held_io_completion: None,
        next_completion_source: CompletionSource::Io,
        locked_candidate_acquisition: None,
        next_locked_candidate_acquisition_id: 0,
        proposal_work_retired: false,
        prepared_candidates: VecDeque::new(),
        merge_sidecar_deferrals: VecDeque::new(),
        outbound_chunks: BTreeMap::new(),
        fast_path_proposals: BTreeSet::new(),
        pending_exact_output: Mutex::new(
            PendingExactOutput::new(16, 5, 4, &frozen_semantic_targets)
                .expect("bounded test output corridor"),
        ),
        kura_replica_advert_refresh,
        exact_output_handoff_owner,
        exact_output_admission_hook: None,
        consensus_broadcasts: Vec::new(),
        active_tag,
        last_status: None,
        fatal_reason: None,
        output_guard: ConsensusOutputGuard::isolated(),
        leader_wire_ingress: Arc::new(FairV2Ingress::new(1, 1024 * 1024, 1024 * 1024, 0, 0)),
        leader_wire_recovery_authority,
        clean_teardown: true,
    };
    (service, keys)
}
fn lane_commit_qc(validator: PeerId) -> LaneBlockQcV1 {
    let validator_set = vec![validator];
    let validator_set_hash = HashOf::new(&validator_set);
    let body = LaneBlockVoteBodyV1 {
        phase: CertPhase::Commit,
        lane_id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(1),
        lane_incarnation: Hash::new(b"outbound corridor lane incarnation"),
        proposal_height: 1,
        lane_block_height: 1,
        lane_block_view: 0,
        proposal_hash: Hash::new(b"outbound corridor proposal"),
        descriptor_hash: Hash::new(b"outbound corridor descriptor"),
        subject_hash: Hash::new(b"outbound corridor subject"),
        payload_ownership_hash: Hash::new(b"outbound corridor ownership"),
        rbc_instance_hash: Hash::new(b"outbound corridor RBC"),
        accepted_candidate_indices: Vec::new(),
        accepted_transaction_hashes: Vec::new(),
        validator_set_hash_version: 1,
        validator_set_hash,
        validator_count: 1,
        min_quorum: 1,
        qc_mode_tag: "outbound-corridor-test".to_owned(),
    };
    LaneBlockQcV1 {
        body,
        validator_set_hash_version: 1,
        validator_set_hash,
        validator_set,
        signers_bitmap: vec![1],
        bls_aggregate_signature: vec![1],
        payload_availability_qc: None,
    }
}
fn non_retireable_lane_transport_messages(validator: PeerId) -> Vec<BlockMessage> {
    let qc = lane_commit_qc(validator.clone());
    let body = &qc.body;
    let descriptor = LaneBlockDescriptorV1 {
        lane_id: body.lane_id,
        dataspace_id: body.dataspace_id,
        lane_incarnation: body.lane_incarnation,
        proposal_height: body.proposal_height,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height: body.lane_block_height,
        lane_block_view: body.lane_block_view,
        subject_hash: body.subject_hash,
        payload_ownership_hash: body.payload_ownership_hash,
        rbc_instance_hash: body.rbc_instance_hash,
        accepted_candidate_indices: body.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: body.accepted_transaction_hashes.clone(),
        validator_set_hash_version: body.validator_set_hash_version,
        validator_set_hash: body.validator_set_hash,
        validator_set: qc.validator_set.clone(),
        validator_count: body.validator_count,
        min_quorum: body.min_quorum,
        qc_mode_tag: body.qc_mode_tag.clone(),
        descriptor_hash: body.descriptor_hash,
    };
    let proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: body.proposal_hash,
        payload_block_hint: None,
    };
    let payload_hash = Hash::new(b"non-retireable lane transport payload");
    let payload = crate::lane_consensus::LaneExecutablePayloadV1 {
        version: crate::lane_consensus::LANE_EXECUTABLE_PAYLOAD_VERSION_V1,
        network_id: crate::sumeragi::synthetic_network_id("non-retireable-lane-transport"),
        epoch: 0,
        origin_proposal: proposal,
        entrypoint_hashes: Vec::new(),
        entrypoints: Vec::new(),
        reservation_keys: Vec::new(),
        routing_plans: Vec::new(),
        native_amx_receipts: Vec::new(),
        payload_hash,
        producer: validator.clone(),
        producer_signature: Vec::new(),
    };
    let new_view_body = crate::lane_consensus::LaneBlockNewViewBodyV1 {
        version: 1,
        network_id: payload.network_id,
        epoch: payload.epoch,
        lane_id: body.lane_id,
        dataspace_id: body.dataspace_id,
        lane_incarnation: body.lane_incarnation,
        proposal_height: body.proposal_height,
        lane_block_height: body.lane_block_height,
        from_view: body.lane_block_view,
        target_view: body.lane_block_view.saturating_add(1),
        locked_proposal_hash: body.proposal_hash,
        locked_descriptor_hash: body.descriptor_hash,
        executable_payload_hash: payload_hash,
        validator_set_hash_version: body.validator_set_hash_version,
        validator_set_hash: body.validator_set_hash,
        validator_count: body.validator_count,
        min_quorum: body.min_quorum,
        qc_mode_tag: body.qc_mode_tag.clone(),
    };
    vec![
        BlockMessage::LaneExecutablePayload(payload),
        BlockMessage::LaneBlockNewViewVote(crate::lane_consensus::LaneBlockNewViewVoteV1 {
            body: new_view_body.clone(),
            signer: validator.clone(),
            bls_signature: Vec::new(),
        }),
        BlockMessage::LaneBlockNewViewCertificate(
            crate::lane_consensus::LaneBlockNewViewCertificateV1 {
                body: new_view_body,
                validator_set: qc.validator_set,
                signers_bitmap: vec![1],
                bls_aggregate_signature: Vec::new(),
            },
        ),
    ]
}
/// Build a deterministic lane CommitQC block for sibling Sumeragi tests.
pub(in crate::sumeragi) fn lane_commit_qc_block_message(validator: PeerId) -> BlockMessage {
    BlockMessage::LaneBlockQc(lane_commit_qc(validator))
}
fn lane_commit_qc_message(validator: PeerId) -> NetworkMessage {
    let wire = BlockMessageWire::try_preencoded(Arc::new(lane_commit_qc_block_message(validator)))
        .expect("encode final lane CommitQC");
    NetworkMessage::SumeragiBlock(Arc::new(wire))
}
fn global_commit_qc_message(
    artifact: &wire::finality::V2FinalityArtifact,
) -> wire::ConsensusMessageV2 {
    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
        artifact.commit_qc.clone(),
    ))
}
fn merge_share(label: &[u8]) -> MergeCommitteeSignature {
    MergeCommitteeSignature {
        version: iroha_data_model::merge::MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
        epoch_id: 7,
        view: 11,
        signer: 0,
        message_digest: Hash::new(label),
        bls_sig: vec![9; 48],
        leader_candidate_body: None,
    }
}
fn merge_share_message(label: &[u8]) -> NetworkMessage {
    NetworkMessage::MergeCommitteeSignature(Arc::new(merge_share(label)))
}
fn lane_drain_vote(keypair: &KeyPair) -> LaneDrainVoteV1 {
    let signer = PeerId::new(keypair.public_key().clone());
    let validator_set = vec![signer.clone()];
    LaneDrainVoteV1::new_signed(
        LaneDrainCertificateBodyV1 {
            version: 1,
            intent: LaneDrainIntentV1 {
                version: 1,
                network_id: crate::sumeragi::synthetic_network_id("v2-worker-drain"),
                lane_id: LaneId::new(3),
                dataspace_id: DataSpaceId::new(5),
                lane_incarnation: Hash::new(b"v2-worker-drain-incarnation"),
                close_global_height: 1,
                initial_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                    LaneId::new(3),
                    DataSpaceId::new(5),
                    Hash::new(b"v2-worker-drain-incarnation"),
                    0,
                    None,
                ),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count: 1,
                min_quorum: 1,
            },
            final_frontier: iroha_data_model::merge::LaneDrainFrontierV1::ordinary(
                LaneId::new(3),
                DataSpaceId::new(5),
                Hash::new(b"v2-worker-drain-incarnation"),
                0,
                None,
            ),
        },
        signer,
        keypair.private_key(),
    )
    .expect("valid worker lane-drain vote")
}
fn native_amx_output(context: &wire::HeightContext, signer: PeerId) -> NativeAmxMessage {
    let validator_set = vec![signer.clone()];
    NativeAmxMessage::PrepareVote(crate::native_amx::NativeAmxVoteV2 {
        body: NativeAmxAttestationBodyV2 {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 0,
            },
            epoch: context.epoch,
            network_id: context.network_id,
            source_id: [0x31; 32],
            tx_entrypoint_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"worker Native AMX entrypoint",
            )),
            plan_digest: Hash::new(b"worker Native AMX plan"),
            phase: NativeAmxPhase::Prepare,
            coordinator_lane_id: LaneId::new(1),
            coordinator_dataspace_id: DataSpaceId::new(1),
            coordinator_lane_incarnation: Hash::new(b"worker coordinator incarnation"),
            participant_lane_id: LaneId::new(2),
            participant_dataspace_id: DataSpaceId::new(2),
            participant_lane_incarnation: Hash::new(b"worker participant incarnation"),
            participant_previous_block_height: 0,
            participant_previous_block_descriptor_hash: None,
            participant_lane_block_height: 1,
            participant_lane_block_view: 0,
            participant_proposal_hash: Hash::new(b"worker participant proposal"),
            participant_settlement_commitment: Hash::new(b"worker participant settlement"),
            participant_validator_set_hash: HashOf::new(&validator_set),
            participant_validator_count: 1,
            participant_min_quorum: 1,
            authority_context_height: context.height,
            planned_coordinator_block_height: 1,
            coordinator_lane_block_view: 0,
            coordinator_proposal_hash: Hash::new(b"worker coordinator proposal"),
        },
        signer,
        bls_signature: vec![0x41; 48],
    })
}
fn certified_sidecar_outputs(
    local: &PeerId,
    peer: &PeerId,
) -> (CertifiedMergeSidecarMessage, CertifiedMergeSidecarMessage) {
    let semantic_sequence = CertifiedMergeSidecarSemanticSequenceV1(
        NonZeroU64::new(1).expect("worker semantic sequence is non-zero"),
    );
    let entry_hash = HashOf::from_untyped_unchecked(Hash::new(b"worker sidecar entry"));
    let reference_digest = Hash::new(b"worker sidecar reference");
    let mut request = CertifiedMergeSidecarRequestV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(1).expect("non-zero worker sidecar request stream epoch"),
        ),
        semantic_sequence,
        closed_through: 0,
        request_id: Hash::prehashed([0; Hash::LENGTH]),
        entry_hash,
        encoded_len: 4,
        epoch_id: 7,
        reference_digest,
        requester: local.clone(),
        responder: peer.clone(),
    };
    request.request_id = request.canonical_request_id();
    let chunk = CertifiedMergeSidecarChunkV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(1).expect("non-zero worker sidecar response stream epoch"),
        ),
        semantic_sequence,
        request_id: Hash::new(b"worker sidecar response request"),
        entry_hash,
        encoded_len: 4,
        epoch_id: 7,
        reference_digest,
        requester: peer.clone(),
        responder: local.clone(),
        chunk_index: 0,
        chunk_count: 1,
        bytes: vec![1, 2, 3, 4],
    };
    (
        CertifiedMergeSidecarMessage::Request(request),
        CertifiedMergeSidecarMessage::Chunk(chunk),
    )
}
fn certified_sidecar_generation_hint(
    local: &PeerId,
    peer: &PeerId,
    ordinal: u64,
) -> CertifiedMergeSidecarMessage {
    let current_generation = crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1(
        NonZeroU64::new(
            ordinal
                .checked_add(2)
                .expect("worker hint generation does not overflow"),
        )
        .expect("worker hint generation is non-zero"),
    );
    let mut hint = crate::merge_sidecar::CertifiedMergeSidecarGenerationHintV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        observed_generation:
            crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        current_generation,
        observed_message_hash: Hash::new_from_chunks(&[
            b"worker retryable generation hint",
            &ordinal.to_le_bytes(),
        ]),
        hint_id: Hash::prehashed([0; Hash::LENGTH]),
        requester: peer.clone(),
        responder: local.clone(),
    };
    hint.hint_id = hint.canonical_hint_id();
    CertifiedMergeSidecarMessage::GenerationHint(hint)
}
fn certified_sidecar_close(
    local: &PeerId,
    peer: &PeerId,
    ordinal: u64,
) -> CertifiedMergeSidecarMessage {
    let stream_epoch = ordinal
        .checked_add(1)
        .expect("worker close epoch does not overflow");
    let mut close = crate::merge_sidecar::CertifiedMergeSidecarCloseV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(stream_epoch).expect("worker close epoch is non-zero"),
        ),
        closed_through: stream_epoch,
        close_id: Hash::prehashed([0; Hash::LENGTH]),
        requester: local.clone(),
        responder: peer.clone(),
    };
    close.close_id = close.canonical_close_id();
    CertifiedMergeSidecarMessage::Close(close)
}
