use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
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
use tempfile::TempDir;

use super::*;
use crate::sumeragi::{
    FairV2Ingress, FairV2IngressBarrierBypass, FairV2IngressClass, FairV2IngressPushDisposition,
    FairV2IngressPushError, FairV2IngressSource, FairV2IngressWireKey, InboundBlockMessage,
    fair_v2_ingress_admit_with_roster_for_test, fair_v2_ingress_is_certified_body_request,
    fair_v2_ingress_required_capacity,
    v2::AdapterEffect,
    v2_block_sync::tests::durable_history_fixture,
    v2_body_store::DurableBodyReceipt,
    v2_chunks::encode_payload,
    v2_core::MAX_EFFECTS_PER_STEP,
    v2_effects::EffectQueueConfig,
    v2_lane_work::tests::durable_lane_history_fixture,
    v2_runtime::{
        BodyAvailableReservation, DecisionProposalRetirement, EnqueueError,
        RetiredBodyPipelineCompletions, RuntimeEffectOwnership, RuntimeLifecycleOwner, RuntimeStep,
        bind_adapter_effect_batch_ownership,
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

/// Build an empty Serve gate together with its exact actor-global ordinal source.
pub(in crate::sumeragi) fn certified_serve_ingress_gate_fixture()
-> (CertifiedServeIngressGate, RuntimeLifecycleOrdinalSource) {
    let (sender, _receiver, _admission) = test_io_command_channel(4);
    let lifecycle_ordinals = sender.queue.lifecycle_ordinals.clone();
    let gate = CertifiedServeIngressGate {
        queue: Arc::clone(&sender.queue),
    };
    (gate, lifecycle_ordinals)
}

fn assert_durable_body_receipt_matches(
    receipt: &DurableBodyReceipt,
    context: &wire::HeightContext,
    manifest: &wire::PayloadManifest,
) {
    assert_eq!(receipt.context_id(), context.id());
    assert_eq!(receipt.round(), manifest.round);
    assert_eq!(receipt.subject(), manifest.subject);
    assert_eq!(receipt.manifest_hash(), HashOf::new(manifest));
}

fn persistent_test_io_command_channel(
    capacity: usize,
    root: &Path,
    context: &wire::HeightContext,
    body_store: &V2BodyStore,
) -> Result<(V2IoCommandSender, V2IoCommandReceiver, Arc<V2IoAdmission>), String> {
    let admission = V2IoAdmission::unbounded_for_tests();
    let (sender, receiver) = persistent_v2_io_command_channel(
        capacity,
        context.roster.len(),
        capacity.max(1),
        capacity.max(1),
        Arc::clone(&admission),
        root,
        context,
        Some(0),
        None,
        body_store,
        RuntimeLifecycleOrdinalSource::after_high_watermark(0),
        CertifiedServeRestartDischarge::PreserveFixtureState,
    )?;
    Ok((sender, receiver, admission))
}

#[allow(clippy::too_many_arguments)]
fn production_persistent_test_io_command_channel(
    capacity: usize,
    root: &Path,
    context: &wire::HeightContext,
    body_store: &V2BodyStore,
    key_pair: &KeyPair,
    validator_set_pops: &[Vec<u8>],
    local_validator: Option<wire::ValidatorIndex>,
    durable_decided_subject: Option<wire::BlockSubject>,
    lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
) -> Result<(V2IoCommandSender, V2IoCommandReceiver, Arc<V2IoAdmission>), String> {
    let admission = V2IoAdmission::unbounded_for_tests();
    let (sender, receiver) = persistent_v2_io_command_channel(
        capacity,
        context.roster.len(),
        capacity.max(1),
        capacity.max(1),
        Arc::clone(&admission),
        root,
        context,
        local_validator,
        durable_decided_subject,
        body_store,
        lifecycle_ordinals,
        CertifiedServeRestartDischarge::Production {
            key_pair,
            validator_set_pops,
        },
    )?;
    Ok((sender, receiver, admission))
}

fn persist_unsealed_serve_fixture(
    root: &Path,
    context: &wire::HeightContext,
    request: &AuthenticatedCertifiedBodyRequest,
    owner: CertifiedServeOwnerKey,
    lifecycle_ordinal: u128,
    scheduler_ordinal: Option<u128>,
) -> CertifiedServeLifecycleId {
    let family_capacity = certified_serve_family_capacity(context.roster.len(), 8, 8)
        .expect("fixture Serve family capacity");
    let (store, mut persisted) = CertifiedServeStateStore::open(root, context, family_capacity)
        .expect("open fixture durable Serve state");
    let lifecycle_id = CertifiedServeLifecycleId {
        admission_ordinal: lifecycle_ordinal,
        request_hash: request.request_hash(),
    };
    persisted.next_lifecycle_admission_ordinal = persisted
        .next_lifecycle_admission_ordinal
        .max(lifecycle_ordinal);
    if let Some(scheduler_ordinal) = scheduler_ordinal {
        persisted.next_ingress_reservation_ordinal = persisted
            .next_ingress_reservation_ordinal
            .max(scheduler_ordinal);
        persisted
            .ingress_waiters
            .push(PersistedCertifiedServeIngressWaiter {
                ingress_ordinal: scheduler_ordinal,
                lifecycle_id,
                owner: owner.clone(),
                request: request.request().clone(),
            });
        persisted
            .ingress_waiters
            .sort_by_key(|waiter| waiter.ingress_ordinal);
    }
    persisted
        .unsealed_lifecycles
        .push(PersistedCertifiedServeLifecycle {
            lifecycle_id,
            owner,
            request: request.request().clone(),
        });
    persisted
        .unsealed_lifecycles
        .sort_by_key(|lifecycle| lifecycle.lifecycle_id);
    store
        .persist(&persisted)
        .expect("persist fixture unsealed Serve lifecycle");
    lifecycle_id
}

fn persist_terminal_serve_fixture(
    root: &Path,
    context: &wire::HeightContext,
    request: &AuthenticatedCertifiedBodyRequest,
    owner: CertifiedServeOwnerKey,
    lifecycle_ordinal: u128,
    response: &wire::CertifiedBodyResponse,
) -> CertifiedServeLifecycleId {
    assert_eq!(response.request_hash, request.request_hash());
    let lifecycle_id = persist_unsealed_serve_fixture(
        root,
        context,
        request,
        owner.clone(),
        lifecycle_ordinal,
        None,
    );
    let family_capacity = certified_serve_family_capacity(context.roster.len(), 8, 8)
        .expect("fixture Serve family capacity");
    let (store, mut persisted) = CertifiedServeStateStore::open(root, context, family_capacity)
        .expect("reopen fixture durable Serve state");
    persisted
        .unsealed_lifecycles
        .retain(|lifecycle| lifecycle.lifecycle_id != lifecycle_id);
    persisted
        .terminal_tombstones
        .push(PersistedCertifiedServeTombstone {
            lifecycle_id,
            owner,
            request: request.request().clone(),
            response_manifest: response.manifest.clone(),
            response_responder: response.responder,
            response_signature: response.signature.clone(),
        });
    persisted
        .terminal_tombstones
        .sort_by_key(|tombstone| tombstone.lifecycle_id);
    store
        .persist(&persisted)
        .expect("persist fixture terminal Serve lifecycle");
    lifecycle_id
}

fn persist_negative_serve_fixture(
    root: &Path,
    context: &wire::HeightContext,
    request: &AuthenticatedCertifiedBodyRequest,
    owner: CertifiedServeOwnerKey,
    lifecycle_ordinal: u128,
    outcome: CertifiedServeNegativeOutcome,
) -> CertifiedServeLifecycleId {
    let lifecycle_id = persist_unsealed_serve_fixture(
        root,
        context,
        request,
        owner.clone(),
        lifecycle_ordinal,
        None,
    );
    let family_capacity = certified_serve_family_capacity(context.roster.len(), 8, 8)
        .expect("fixture Serve family capacity");
    let (store, mut persisted) = CertifiedServeStateStore::open(root, context, family_capacity)
        .expect("reopen fixture durable Serve state");
    persisted
        .unsealed_lifecycles
        .retain(|lifecycle| lifecycle.lifecycle_id != lifecycle_id);
    persisted
        .negative_tombstones
        .push(PersistedCertifiedServeNegativeTombstone {
            lifecycle_id,
            owner,
            request: request.request().clone(),
            outcome,
        });
    persisted
        .negative_tombstones
        .sort_by_key(|tombstone| tombstone.lifecycle_id);
    store
        .persist(&persisted)
        .expect("persist fixture negative Serve lifecycle");
    lifecycle_id
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
            signers: (0..context.roster.len())
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

fn production_authenticated_serve_request(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    requester_key: &KeyPair,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    phase: wire::GlobalPhase,
    signer_indices: &[usize],
) -> (AuthenticatedCertifiedBodyRequest, Vec<Vec<u8>>) {
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

fn commit_and_terminalize_serve(
    command_tx: &V2IoCommandSender,
    command_rx: &V2IoCommandReceiver,
    admission: &CertifiedServeAdmission,
    authenticated_via: PeerId,
    route: NetworkReplyRoute,
    response: wire::CertifiedBodyResponse,
) -> CertifiedServeLifecycleId {
    let requester = admission.request.requester.clone();
    let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(admission.request.clone()),
    ));
    let (routes, ownership) =
        fair_ingress_route_owner(message, requester, authenticated_via, route);
    assert!(matches!(
        command_tx
            .commit_serve(admission, routes, ownership)
            .expect("commit prepared Serve lifecycle"),
        CertifiedServeCommit::Queued
    ));
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Serve { lifecycle_id, .. })
            if lifecycle_id == admission.lifecycle_id
    ));
    command_rx
        .complete_serve_response(admission.lifecycle_id, &response)
        .expect("seal terminal Serve response before exposing completion");
    command_tx
        .acknowledge_serve_completion(
            admission.lifecycle_id,
            V2IoServeTerminal::Response(response),
        )
        .expect("terminal response remains bound to the exact Serve lifecycle");
    admission.lifecycle_id
}

// Scheduler-attempt telemetry is intentionally excluded: even a failed
// dequeue records that the fair-ingress queue received a service turn.
#[derive(Debug, PartialEq, Eq)]
struct FairIngressAccountingSnapshot {
    last_admission_ordinal: u64,
    ready: Vec<FairV2IngressSource>,
    pending_wire_owners: Vec<(FairV2IngressWireKey, FairV2IngressSource)>,
    lanes: Vec<FairIngressLaneAccountingSnapshot>,
    len: usize,
    bytes: usize,
    nonempty_since: Option<Instant>,
    open: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct FairIngressLaneAccountingSnapshot {
    source: FairV2IngressSource,
    entries: Vec<FairIngressEntryAccountingSnapshot>,
    progress_len: usize,
    timeout_vote_len: usize,
    transport_completion_len: usize,
    bytes: usize,
    timeout_vote_bytes: usize,
    transport_completion_bytes: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct FairIngressEntryAccountingSnapshot {
    admission_ordinal: u64,
    owns_certified_serve_ticket: bool,
    class: FairV2IngressClass,
    wire_key: Option<FairV2IngressWireKey>,
    encoded_len: usize,
}

fn fair_ingress_accounting_snapshot(ingress: &FairV2Ingress) -> FairIngressAccountingSnapshot {
    let state = ingress.state.lock();
    FairIngressAccountingSnapshot {
        last_admission_ordinal: state.last_admission_ordinal,
        ready: state.ready.iter().cloned().collect(),
        pending_wire_owners: state
            .pending_wire_owners
            .iter()
            .map(|(wire, owner)| (wire.clone(), owner.clone()))
            .collect(),
        lanes: state
            .lanes
            .iter()
            .map(|(source, lane)| FairIngressLaneAccountingSnapshot {
                source: source.clone(),
                entries: lane
                    .entries
                    .iter()
                    .map(|entry| FairIngressEntryAccountingSnapshot {
                        admission_ordinal: entry.admission_ordinal,
                        owns_certified_serve_ticket: entry.certified_serve_reservation.is_some(),
                        class: entry.class,
                        wire_key: entry.wire_key.clone(),
                        encoded_len: entry.encoded_len,
                    })
                    .collect(),
                progress_len: lane.progress_len,
                timeout_vote_len: lane.timeout_vote_len,
                transport_completion_len: lane.transport_completion_len,
                bytes: lane.bytes,
                timeout_vote_bytes: lane.timeout_vote_bytes,
                transport_completion_bytes: lane.transport_completion_bytes,
            })
            .collect(),
        len: state.len,
        bytes: state.bytes,
        nonempty_since: state.nonempty_since,
        open: state.open,
    }
}

fn gated_fair_ingress(
    context: &wire::HeightContext,
    command_tx: &V2IoCommandSender,
) -> (FairV2Ingress, CertifiedServeIngressGate) {
    let ingress = FairV2Ingress::new(
        128,
        5 * 64 * 1024 * 1024,
        64 * 1024 * 1024,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
    );
    ingress
        .configure_roster(context.roster.iter().map(|entry| entry.validator.clone()))
        .expect("test roster fits fair ingress");
    ingress.require_certified_serve_gate();
    let gate = CertifiedServeIngressGate {
        queue: Arc::clone(&command_tx.queue),
    };
    ingress
        .bind_certified_serve_gate(gate.clone())
        .expect("bind exact Serve gate");
    ingress.open().expect("open gated fair ingress");
    (ingress, gate)
}

fn certified_serve_inbound(
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

fn certified_serve_inbound_with_route(
    request: &wire::CertifiedBodyRequest,
    authenticated_via: PeerId,
    route: NetworkReplyRoute,
) -> InboundBlockMessage {
    InboundBlockMessage::try_from_transport_with_reply_route(
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
        )),
        request.requester.clone(),
        authenticated_via,
        route,
    )
    .expect("fixture reply route matches exact Serve ingress")
}

fn drain_and_commit_gated_serve(
    ingress: &FairV2Ingress,
    command_tx: &V2IoCommandSender,
    owner: CertifiedServeOwnerKey,
    request: &AuthenticatedCertifiedBodyRequest,
) -> (CertifiedServeAdmission, CertifiedServeCommit) {
    let mut prepared = None;
    let mut inbound = ingress
        .try_recv_if(|_| {
            prepared = Some(
                command_tx
                    .prepare_reserved_serve(owner.clone(), request.clone())
                    .expect("prepare gated Serve fixture"),
            );
            true
        })
        .expect("drain gated Serve fixture");
    let admission = prepared.expect("predicate retained gated Serve admission");
    let ingress_ownership = inbound
        .take_ingress_ownership()
        .expect("gated Serve fixture retains fair ownership");
    let (_, _, reply_routes) = inbound.into_message_sender_and_reply_routes();
    let committed = command_tx
        .commit_serve(
            &admission,
            reply_routes.expect("gated Serve fixture retains reply routes"),
            ingress_ownership,
        )
        .expect("commit gated Serve fixture");
    (admission, committed)
}

struct SaturatedCompletionRuntime {
    queued: usize,
    capacity: usize,
    next_lifecycle_ordinal: u128,
    effect_owners: BTreeMap<Hash, RuntimeEffectOwnership>,
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
    ) -> Result<RuntimeEffectOwnership, String> {
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
    ) -> Result<RuntimeEffectOwnership, String> {
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
        now: Instant,
    ) -> Result<RuntimeStep<AdapterEffect>, String> {
        self.step_effects(now)
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
    ) -> Result<RuntimeEffectOwnership, String> {
        let mut identity = Vec::from(b"body-pipeline".as_slice());
        identity.extend_from_slice(&manifest.round.encode());
        identity.extend_from_slice(&manifest.subject.encode());
        let ownership = self.ownership_for_identity(tag, Hash::new(identity))?;
        let effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        bind_adapter_effect_batch_ownership(std::slice::from_ref(&effect), vec![ownership])?
            .pop()
            .ok_or_else(|| "saturated local proposal StoreBody binding was empty".to_owned())
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

    fn enqueue_validation_succeeded(
        &mut self,
        _tag: EventTag,
        _round: wire::ConsensusRound,
        _subject: wire::BlockSubject,
        _receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        Self::reject_completion()
    }

    fn enqueue_validation_failed(
        &mut self,
        _tag: EventTag,
        _round: wire::ConsensusRound,
        _subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        Self::reject_completion()
    }

    fn enqueue_validation_failures_atomically(
        &mut self,
        _failures: &[(EventTag, wire::ConsensusRound, wire::BlockSubject)],
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

    fn enqueue_local_proposal(
        &mut self,
        _tag: EventTag,
        _manifest: wire::PayloadManifest,
        _durable_receipt: DurableBodyReceipt,
        _validated_receipt: ValidatedBodyReceipt,
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

    fn has_certified_fence_escape_credit(&self) -> bool {
        false
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
    let proposal_owner = runtime
        .mint_local_proposal_effect_ownership(tag, &manifest)
        .expect("mint local proposal owner");
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
        validation_rejections: VecDeque::new(),
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

#[cfg(feature = "bls")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectedServeTimeoutRecoveryMode {
    TimeoutRecovery,
    LatePassiveFetch,
}

#[cfg(feature = "bls")]
struct SelectedServeLatePassiveFetch {
    body_store: V2BodyStore,
    task: BodyFetchTask,
    manifest: wire::PayloadManifest,
    body: Vec<u8>,
}

/// Build exact signed phase-vote evidence for the production persistence bridge.
fn exact_vote_equivocation(
    service: &ProductionV2Services,
    keys: &[KeyPair],
) -> wire::SumeragiV2Equivocation {
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: 0,
    };
    let signer = 1;
    let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"equivocation parent state"),
        Hash::new(b"equivocation post state"),
        Hash::new(b"equivocation ordinary writes"),
        1,
        Hash::new(b"equivocation executed block"),
    );
    let signed_vote = |seed: u8| {
        let mut vote = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([seed; 32])),
                payload_hash: Hash::prehashed([seed.wrapping_add(1); 32]),
            },
            execution_commitment,
            signer,
            signature: Vec::new(),
        };
        vote.signature = Signature::new(
            keys[usize::try_from(signer).expect("small signer index")].private_key(),
            &vote.signature_preimage(),
        )
        .payload()
        .to_vec();
        vote
    };
    wire::SumeragiV2Equivocation::PhaseVote {
        first: signed_vote(0xA1),
        second: signed_vote(0xA2),
    }
}

#[test]
fn production_equivocation_bridge_validates_persists_and_deduplicates_restart_replay() {
    let (mut service, keys) = fixture();
    let evidence = exact_vote_equivocation(&service, &keys);
    service
        .report_equivocation(evidence.clone())
        .expect("persist valid exact equivocation evidence");
    let shared_state = Arc::clone(&service.state);
    assert_eq!(
        shared_state.world.consensus_evidence.view().iter().count(),
        1
    );

    let wire::SumeragiV2Equivocation::PhaseVote { first, second } = evidence.clone() else {
        unreachable!("phase-vote fixture")
    };
    service
        .report_equivocation(wire::SumeragiV2Equivocation::PhaseVote {
            first: second,
            second: first,
        })
        .expect("swapped replay is an idempotent duplicate");

    let (mut restarted_service, _) = fixture();
    restarted_service.context = service.context.clone();
    restarted_service.validator_set_pops = service.validator_set_pops.clone();
    restarted_service.state = Arc::clone(&shared_state);
    restarted_service
        .report_equivocation(evidence)
        .expect("restart replay observes the canonical persisted key");
    assert_eq!(
        shared_state.world.consensus_evidence.view().iter().count(),
        1
    );
}

#[test]
fn production_equivocation_bridge_rejects_invalid_or_unanchored_evidence() {
    let (mut invalid_service, invalid_keys) = fixture();
    let mut forged = exact_vote_equivocation(&invalid_service, &invalid_keys);
    let wire::SumeragiV2Equivocation::PhaseVote { second, .. } = &mut forged else {
        unreachable!("phase-vote fixture")
    };
    second.signature[0] ^= 0x80;
    assert!(
        invalid_service.report_equivocation(forged).is_err(),
        "invalid evidence must fail before persistence or reporting"
    );
    assert_eq!(
        invalid_service
            .state
            .world
            .consensus_evidence
            .view()
            .iter()
            .count(),
        0
    );

    let (mut foreign_context_service, foreign_keys) = fixture();
    foreign_context_service.context.network_id =
        crate::sumeragi::synthetic_network_id("foreign-evidence-chain");
    let foreign_evidence = exact_vote_equivocation(&foreign_context_service, &foreign_keys);
    assert!(
        foreign_context_service
            .report_equivocation(foreign_evidence)
            .is_err(),
        "a valid pair from an unanchored context must fail closed"
    );
    assert_eq!(
        foreign_context_service
            .state
            .world
            .consensus_evidence
            .view()
            .iter()
            .count(),
        0
    );
}

/// Production-shaped selected-Serve recovery shared with the runner regression.
#[cfg(feature = "bls")]
pub(in crate::sumeragi) struct SelectedServeTimeoutRecoveryFixture {
    _runtime_directory: TempDir,
    _leader_wire_directory: TempDir,
    ingress: Arc<FairV2Ingress>,
    serve_gate: CertifiedServeIngressGate,
    missing_proposal_request: AuthenticatedCertifiedBodyRequest,
    missing_proposal_request_hash: HashOf<wire::CertifiedBodyRequest>,
    late_passive_fetch: Option<SelectedServeLatePassiveFetch>,
    executor: V2EffectExecutor<SerializedV2Runtime>,
    services: ProductionV2Services,
    command_rx: V2IoCommandReceiver,
    completion_tx: mpsc::SyncSender<V2IoCompletion>,
    completion_admission: Arc<V2IoAdmission>,
    local_key: KeyPair,
    consensus_observations: Arc<Mutex<Vec<ConsensusRouteObservation>>>,
    remote_timeout_votes_admitted: usize,
    timeout_prefix_completions: usize,
    local_timeout_signature_completed: bool,
}

#[cfg(feature = "bls")]
impl SelectedServeTimeoutRecoveryFixture {
    /// Build one missing-body Serve barrier followed by two authenticated timeout votes.
    pub(in crate::sumeragi) fn new() -> Self {
        Self::new_for_mode(SelectedServeTimeoutRecoveryMode::TimeoutRecovery)
    }

    /// Build one passive Fetch before the selected missing-body Serve barrier.
    pub(in crate::sumeragi) fn new_late_passive_fetch() -> Self {
        Self::new_for_mode(SelectedServeTimeoutRecoveryMode::LatePassiveFetch)
    }

    #[allow(clippy::too_many_lines)]
    fn new_for_mode(mode: SelectedServeTimeoutRecoveryMode) -> Self {
        let (mut services, keys) = fixture();
        if mode == SelectedServeTimeoutRecoveryMode::LatePassiveFetch {
            allow_fixture_block_payload(&mut services.context);
            services.leader_wire_recovery_authority = super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
                    services.context.id(),
                    services.context.height,
                    [0xF4; 32],
                    services.active_tag.view(),
                    false,
                );
        }
        let context = services.context.clone();
        assert_eq!(
            context.roster.len(),
            4,
            "selected-Serve timeout recovery requires four representative validators"
        );
        let view_zero_leader = context.leader(0);
        let local_validator = (0..context.roster.len())
            .map(|index| u32::try_from(index).expect("fixture roster index fits u32"))
            .find(|index| *index != view_zero_leader)
            .expect("four-validator fixture has a non-leader timeout signer");
        let local_index =
            usize::try_from(local_validator).expect("fixture local validator fits usize");
        let local_key = keys[local_index].clone();
        services.local_validator = Some(local_validator);
        services.local_peer = context.roster[local_index].validator.clone();
        services.key_pair = local_key.clone();

        let (command_tx, command_rx, admission) = test_io_command_channel(8);
        let lifecycle_ordinals = command_tx.queue.lifecycle_ordinals.clone();
        let completion_admission = Arc::clone(&admission);
        let (completion_tx, completion_rx) = mpsc::sync_channel(8);
        services.io = Some(V2IoHandle {
            command_tx,
            completion_rx,
            join: None,
            allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
            admission,
        });
        let serve_gate = services
            .io
            .as_ref()
            .expect("install the manual production I/O boundary")
            .certified_serve_ingress_gate();

        let ingress = Arc::new(
            FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
                128,
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
            .collect::<BTreeSet<_>>();
        ingress
            .configure_roster_for_context(
                roster.iter().cloned(),
                &context.network_id,
                context.da_layout,
            )
            .expect("configure selected-Serve timeout ingress");
        ingress.require_certified_serve_gate();
        ingress.require_leader_wire_lifecycle_gate();
        ingress
            .bind_certified_serve_gate(serve_gate.clone())
            .expect("bind the production Serve gate");

        let leader_wire_directory =
            TempDir::new().expect("temporary selected-Serve leader-wire directory");
        let capacity =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
                roster.len(),
                context.da_layout.max_chunk_count,
            )
            .expect("derive selected-Serve leader-wire capacity");
        let recovery_authority = services.leader_wire_recovery_authority;
        let (leader_wire_gate, restore) =
            super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
                &leader_wire_directory
                    .path()
                    .join("selected-serve-timeout-recovery.wal"),
                context.id(),
                context.height,
                [0xF4; 32],
                roster,
                capacity,
                context.da_layout.max_chunk_count,
                recovery_authority,
                &[],
                &[],
            )
            .expect("open selected-Serve leader-wire gate");
        ingress
            .bind_leader_wire_lifecycle_gate(
                leader_wire_gate,
                restore,
                lifecycle_ordinals.clone(),
                context.id(),
                context.height,
            )
            .expect("bind the shared leader-wire lifecycle source");
        ingress.open().expect("open selected-Serve timeout ingress");
        services.leader_wire_ingress = Arc::clone(&ingress);

        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator proof of possession")
            })
            .collect();
        let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
            .expect("verify selected-Serve runtime context");
        let runtime_directory = TempDir::new().expect("temporary selected-Serve runtime directory");
        if mode == SelectedServeTimeoutRecoveryMode::LatePassiveFetch {
            services.chunk_root = runtime_directory.path().join("chunks");
        }
        let (adapter, startup_effects) = SumeragiV2Adapter::open(
            runtime_directory.path().join("selected-serve-runtime.wal"),
            verified,
            Some(local_validator),
            Generation::new(context.height),
            [0xF4; 32],
            AdapterFingerprints {
                node: Hash::new(b"selected Serve timeout node"),
                build: Hash::new(b"selected Serve timeout build"),
                config: Hash::new(b"selected Serve timeout config"),
            },
            DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("open selected-Serve runtime adapter");
        assert!(startup_effects.is_empty());
        let round_timeout = match mode {
            SelectedServeTimeoutRecoveryMode::TimeoutRecovery => Duration::from_millis(1),
            SelectedServeTimeoutRecoveryMode::LatePassiveFetch => Duration::from_secs(24 * 60 * 60),
        };
        let started_at = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .expect("fixture clock has a one-second predecessor");
        let (runtime, startup_effects) = SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            startup_effects,
            started_at,
            round_timeout,
            RuntimeQueueConfig::new(8, 2, 2),
            lifecycle_ordinals,
        )
        .expect("construct selected-Serve serialized runtime");
        assert!(startup_effects.is_empty());
        let mut executor = V2EffectExecutor::with_runtime(
            runtime,
            BTreeMap::new(),
            context.clone(),
            services.local_peer.clone(),
            Some(local_validator),
            EffectQueueConfig::default(),
        )
        .expect("construct selected-Serve effect executor");
        let late_passive_fetch = match mode {
            SelectedServeTimeoutRecoveryMode::TimeoutRecovery => {
                executor
                    .arm_live_clocks(started_at)
                    .expect("arm selected-Serve timeout clocks");
                let timeout_owner = executor
                    .freeze_due_timeout_owner_for_test(Instant::now())
                    .expect("freeze the height-start timeout before later Serve ingress");
                assert_eq!(
                    timeout_owner.lifecycle_ordinal(),
                    1,
                    "the height-start timeout owns the first actor-global scheduler position"
                );
                None
            }
            SelectedServeTimeoutRecoveryMode::LatePassiveFetch => {
                let late_dispatch_at = Instant::now();
                executor
                    .arm_live_clocks(late_dispatch_at)
                    .expect("arm non-due late-passive-Fetch clocks");
                let (body, payload, mut proposal) = proposal_body_and_payload(&context, &keys);
                let proposer_index =
                    usize::try_from(proposal.proposer).expect("fixture proposal index fits usize");
                proposal.signature = Signature::new(
                    keys[proposer_index].private_key(),
                    &proposal.signature_preimage(),
                )
                .payload()
                .to_vec();
                executor
                    .enqueue_network(wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::Proposal(proposal),
                    ))
                    .expect("enqueue the signed late-passive-Fetch proposal");
                assert!(matches!(
                    executor
                        .step(late_dispatch_at, &mut services)
                        .expect("dispatch the signed proposal into passive Fetch work"),
                    EffectExecutorStep::Advanced { .. }
                ));
                assert_eq!(
                    executor.status().pending_fetches,
                    1,
                    "the signed Proposal must establish reducer body-work ownership"
                );
                assert_eq!(
                    services.fetches.len(),
                    1,
                    "the passive Fetch must cross the production service boundary"
                );
                let task = services
                    .fetches
                    .values()
                    .next()
                    .expect("one production passive Fetch remains live")
                    .task
                    .clone();
                assert_eq!(task.manifest(), Some(payload.manifest()));
                let body_store =
                    V2BodyStore::open(runtime_directory.path().join("bodies"), context.clone())
                        .expect("open the retained late-passive-Fetch body store");
                Some(SelectedServeLatePassiveFetch {
                    body_store,
                    task,
                    manifest: payload.manifest().clone(),
                    body,
                })
            }
        };
        let consensus_observations = install_consensus_route_observer(&mut services);

        // Timeout mode freezes the height-start owner before later ingress.
        // Late-Fetch mode instead established its passive reducer owner
        // above. In both cases the selected Serve must take the next shared
        // actor-global position without jumping its predecessor.
        let missing_proposal_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let missing_proposal_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"selected Serve missing proposal",
            )),
            payload_hash: Hash::new(b"selected Serve missing proposal payload"),
        };
        let requester_index = (0..keys.len())
            .find(|index| *index != local_index)
            .expect("four-validator fixture has a remote Serve requester");
        let missing_request = authenticated_serve_request(
            &context,
            &keys[requester_index],
            missing_proposal_round,
            missing_proposal_subject,
            wire::GlobalPhase::Prepare,
        );
        let missing_proposal_request_hash = missing_request.request_hash();
        let authenticated_via = missing_request.request().requester.clone();
        assert!(matches!(
            ingress.try_push(certified_serve_inbound(
                missing_request.request(),
                authenticated_via,
            )),
            Ok(FairV2IngressPushDisposition::Enqueued)
        ));

        if let Some(late_passive_fetch) = &late_passive_fetch {
            let barrier = serve_gate
                .selected_barrier()
                .expect("inspect late-passive-Fetch Serve barrier")
                .expect("late-passive-Fetch Serve remains selected");
            assert_eq!(
                barrier.scheduler_ordinal(),
                late_passive_fetch
                    .task
                    .lifecycle_ordinal()
                    .checked_add(1)
                    .expect("late passive Fetch ordinal has a successor"),
                "Serve admission must take the next shared actor-global ordinal"
            );
        }

        if mode == SelectedServeTimeoutRecoveryMode::TimeoutRecovery {
            let remote_signers = (0..keys.len())
                .filter(|index| *index != local_index)
                .take(2)
                .collect::<Vec<_>>();
            assert_eq!(remote_signers.len(), 2);
            for signer_index in remote_signers {
                let signer = u32::try_from(signer_index).expect("timeout signer fits u32");
                let mut timeout_vote = wire::TimeoutVote {
                    round: missing_proposal_round,
                    highest_prepare_qc: None,
                    signer,
                    signature: Vec::new(),
                };
                timeout_vote.signature = Signature::new(
                    keys[signer_index].private_key(),
                    &timeout_vote.signature_preimage(),
                )
                .payload()
                .to_vec();
                let source = context.roster[signer_index].validator.clone();
                assert!(matches!(
                    ingress.try_push(InboundBlockMessage::new(
                        BlockMessage::V2(wire::ConsensusMessageV2::new(
                            wire::ConsensusMessageV2Payload::TimeoutVote(timeout_vote),
                        )),
                        Some(source),
                    )),
                    Ok(FairV2IngressPushDisposition::Enqueued)
                ));
            }
        }

        let fixture = Self {
            _runtime_directory: runtime_directory,
            _leader_wire_directory: leader_wire_directory,
            ingress,
            serve_gate,
            missing_proposal_request: missing_request,
            missing_proposal_request_hash,
            late_passive_fetch,
            executor,
            services,
            command_rx,
            completion_tx,
            completion_admission,
            local_key,
            consensus_observations,
            remote_timeout_votes_admitted: 0,
            timeout_prefix_completions: 0,
            local_timeout_signature_completed: false,
        };
        fixture.assert_missing_proposal_serve_selected();
        fixture
    }

    /// Service the production exact-Serve prefix before its liveness suffix.
    pub(in crate::sumeragi) fn service_exact_serve_runtime_prefix(
        &mut self,
    ) -> Result<bool, String> {
        let barrier = self
            .services
            .certified_serve_barrier()?
            .ok_or_else(|| "selected-Serve fixture lost its exact barrier".to_owned())?;
        let completion_evidence = self
            .services
            .certified_serve_predecessor_completion_evidence(
                self.executor.remaining_completion_capacity() != 0,
                barrier.scheduler_ordinal(),
            )?;
        if let Some(witness) = self
            .executor
            .exact_serve_predecessor_episode_witness(
                Instant::now(),
                barrier.scheduler_ordinal(),
                completion_evidence,
            )
            .map_err(|error| error.to_string())?
        {
            let _ = self
                .services
                .observe_certified_serve_predecessor_episode_witness(barrier, witness)?;
        }
        let claimed = self
            .services
            .claim_certified_serve_runtime_episode(barrier)?;
        if !claimed {
            self.assert_missing_proposal_serve_selected();
            return Ok(false);
        }
        let _ = self
            .services
            .drain_exact_serve_runtime_predecessor(&mut self.executor, barrier.scheduler_ordinal())
            .map_err(|error| error.to_string())?;
        let completion_evidence = self
            .services
            .certified_serve_predecessor_completion_evidence(
                self.executor.remaining_completion_capacity() != 0,
                barrier.scheduler_ordinal(),
            )?;
        let predecessor_witness = self
            .executor
            .exact_serve_predecessor_episode_witness(
                Instant::now(),
                barrier.scheduler_ordinal(),
                completion_evidence,
            )
            .map_err(|error| error.to_string())?;
        if let Some(witness) = predecessor_witness {
            let _ = self
                .services
                .observe_certified_serve_predecessor_episode_witness(barrier, witness)?;
        }
        if predecessor_witness.is_some()
            && self
                .services
                .certified_serve_runtime_predecessor_capacity_available(barrier)?
        {
            self.executor
                .set_ingress_physical_cut(self.ingress.next_physical_admission_ordinal())
                .map_err(|error| error.to_string())?;
            let _ = self
                .executor
                .step(Instant::now(), &mut self.services)
                .map_err(|error| error.to_string())?;
        }
        let completion_evidence = self
            .services
            .certified_serve_predecessor_completion_evidence(
                self.executor.remaining_completion_capacity() != 0,
                barrier.scheduler_ordinal(),
            )?;
        let predecessor_witness = self
            .executor
            .exact_serve_predecessor_episode_witness(
                Instant::now(),
                barrier.scheduler_ordinal(),
                completion_evidence,
            )
            .map_err(|error| error.to_string())?;
        if let Some(witness) = predecessor_witness {
            let _ = self
                .services
                .observe_certified_serve_predecessor_episode_witness(barrier, witness)?;
        }
        let older_predecessor_remains = predecessor_witness.is_some();
        self.services
            .finish_certified_serve_runtime_episode_turn(barrier, older_predecessor_remains)?;
        self.assert_missing_proposal_serve_selected();
        Ok(true)
    }

    /// Drive a late passive Fetch through Store and rejected validation, then release Serve.
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn assert_late_passive_fetch_completion_reopens_selected_serve(
        &mut self,
    ) {
        let mut late = self
            .late_passive_fetch
            .take()
            .expect("fixture owns one late passive Fetch");
        let fetch_ordinal = late.task.lifecycle_ordinal();

        assert!(
            self.service_exact_serve_runtime_prefix()
                .expect("complete the initially selected Serve predecessor episode")
        );
        assert!(
            !self
                .service_exact_serve_runtime_prefix()
                .expect("the passive Fetch alone cannot reopen the completed episode"),
            "transport-passive Fetch work is not runnable reducer progress"
        );

        assert_eq!(
            self.executor
                .complete_body_reconstruction(
                    &late.task,
                    late.manifest.clone(),
                    late.body.clone(),
                    &mut self.services,
                )
                .expect("complete the exact passive body reconstruction"),
            CompletionDisposition::Accepted
        );
        assert!(
            self.service_exact_serve_runtime_prefix()
                .expect("the late BodyAvailable successor reopens the Serve episode")
        );

        let store_task = match self.command_rx.try_recv() {
            Ok(V2IoCommand::Store(task)) => task,
            Ok(_) => panic!("late passive Fetch queued a non-Store command"),
            Err(error) => panic!("late passive Fetch omitted its Store command: {error}"),
        };
        assert_eq!(
            store_task.lifecycle_ordinal(),
            fetch_ordinal,
            "Store must retain the original passive Fetch owner"
        );
        assert!(
            !self
                .service_exact_serve_runtime_prefix()
                .expect("an incomplete Store cannot reopen the completed episode"),
            "active Store work remains passive until its tracked completion exists"
        );
        let stored = late
            .body_store
            .execute_store_task(&store_task)
            .expect("durably store the late reconstructed body");
        self.command_rx.complete_work(store_task.id());
        try_send_tracked_completion_with_lifecycle_ordinal(
            &self.completion_tx,
            &self.completion_admission,
            V2IoCompletion::Stored(stored),
            Some(fetch_ordinal),
        )
        .expect("deliver the exact tracked Store completion");

        assert!(
            self.service_exact_serve_runtime_prefix()
                .expect("the stored-body completion reopens and queues validation")
        );
        let validation_task = match self.command_rx.try_recv() {
            Ok(V2IoCommand::Validate(task)) => task,
            Ok(_) => panic!("late passive Fetch queued a non-Validate command"),
            Err(error) => {
                panic!("late passive Fetch omitted its Validate command: {error}")
            }
        };
        assert_eq!(
            validation_task.lifecycle_ordinal(),
            fetch_ordinal,
            "Validate must retain the original passive Fetch owner"
        );
        assert!(
            !self
                .service_exact_serve_runtime_prefix()
                .expect("an incomplete Validate cannot reopen the completed episode"),
            "active Validate work remains passive until its tracked completion exists"
        );
        let validated = late
            .body_store
            .execute_validation_task(&validation_task, |_| {
                Err::<wire::ExecutionCommitment, String>(
                    "deterministic late-passive-Fetch rejection".to_owned(),
                )
            })
            .expect("execute deterministic late-body validation");
        assert!(matches!(
            &validated,
            BodyValidationCompletion::Rejected { work_id, reason }
                if *work_id == validation_task.id()
                    && reason == "deterministic late-passive-Fetch rejection"
        ));
        self.command_rx.complete_work(validation_task.id());
        try_send_tracked_completion_with_lifecycle_ordinal(
            &self.completion_tx,
            &self.completion_admission,
            V2IoCompletion::Validated(validated),
            Some(fetch_ordinal),
        )
        .expect("deliver the exact tracked validation completion");

        assert!(
            self.service_exact_serve_runtime_prefix()
                .expect("the rejected validation retires its ValidationFailed successor")
        );
        assert!(
            !self
                .service_exact_serve_runtime_prefix()
                .expect("the retired body pipeline leaves no older predecessor"),
            "the rejected late body pipeline must terminate before Serve"
        );

        let requester = self.missing_proposal_request.request().requester.clone();
        let (admission, committed) = drain_and_commit_gated_serve(
            &self.ingress,
            &self
                .services
                .io
                .as_ref()
                .expect("late-passive-Fetch fixture retains its I/O service")
                .command_tx,
            CertifiedServeOwnerKey::Roster(requester),
            &self.missing_proposal_request,
        );
        assert!(matches!(committed, CertifiedServeCommit::Queued));
        assert!(matches!(
            self.command_rx.try_recv(),
            Ok(V2IoCommand::Serve {
                lifecycle_id,
                request,
            }) if lifecycle_id == admission.lifecycle_id
                && request.request_hash() == self.missing_proposal_request_hash
        ));

        let producer_episode = self
            .services
            .try_begin_certified_serve_producer_episode()
            .expect("inspect producer ownership after exact Serve drain")
            .expect("the exact Serve completion must reopen one producer episode");
        assert!(
            self.services
                .try_begin_certified_serve_producer_episode()
                .is_err(),
            "one live producer lease must reject a nested ownership claim"
        );
        drop(producer_episode);
    }

    /// Admit at most one exact timeout-vote owner through the Serve-only bypass.
    pub(in crate::sumeragi) fn service_timeout_vote_episode(&mut self) -> Result<(), String> {
        let executor = &self.executor;
        let Some((mut inbound, disposition)) = self
            .ingress
            .try_recv_if_checked_retiring_obsolete_with_barrier_bypass(
                FairV2IngressBarrierBypass::TimeoutVoteEpisode,
                |inbound| {
                    let BlockMessage::V2(message) = inbound.message() else {
                        return false;
                    };
                    inbound.ingress_ownership().is_some_and(|ownership| {
                        executor.can_admit_timeout_vote_recovery_episode(message, ownership)
                    })
                },
            )?
        else {
            self.assert_missing_proposal_serve_selected();
            return Ok(());
        };
        if disposition != super::super::FairV2IngressDequeueDisposition::Admit {
            return Err("timeout episode selected an obsolete leader-wire owner".to_owned());
        }
        let mut ownership = inbound
            .take_ingress_ownership()
            .ok_or_else(|| "selected TimeoutVote lost fair-ingress ownership".to_owned())?;
        self.ingress
            .bind_leader_wire_runtime_ownership(&mut ownership)?;
        let (message, _, _) = inbound.into_message_sender_and_reply_routes();
        let BlockMessage::V2(message) = message else {
            return Err("timeout episode selected a non-v2 message".to_owned());
        };
        self.executor
            .enqueue_network_with_ingress_ownership(message, ownership)
            .map_err(|error| error.to_string())?;
        self.remote_timeout_votes_admitted = self.remote_timeout_votes_admitted.saturating_add(1);
        self.assert_missing_proposal_serve_selected();
        Ok(())
    }

    /// Execute and deliver the local timeout signature through the worker completion lane.
    pub(in crate::sumeragi) fn service_timeout_recovery_prefix(&mut self) -> Result<(), String> {
        match self.command_rx.try_recv() {
            Ok(V2IoCommand::Sign {
                task,
                restore_outbound_payload: false,
            }) if matches!(task.request(), SignRequest::TimeoutVote(_)) => {
                let work_id = task.id();
                let lifecycle_ordinal = task.lifecycle_ordinal();
                let signature = Signature::new(
                    self.local_key.private_key(),
                    &task.request().signature_preimage(),
                )
                .payload()
                .to_vec();
                self.command_rx.complete_work(work_id);
                try_send_tracked_completion_with_lifecycle_ordinal(
                    &self.completion_tx,
                    &self.completion_admission,
                    V2IoCompletion::Signature {
                        work_id,
                        signature,
                        outbound_payload: None,
                    },
                    Some(lifecycle_ordinal),
                )
                .map_err(|_| {
                    "selected-Serve timeout completion channel is unavailable".to_owned()
                })?;
                self.local_timeout_signature_completed = true;
            }
            Ok(_) => {
                return Err(
                    "selected-Serve timeout fixture received an unexpected I/O command".to_owned(),
                );
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("selected-Serve timeout worker disconnected".to_owned());
            }
        }
        if let Some(cut) = self
            .executor
            .timeout_recovery_lifecycle_cut()
            .map_err(|error| error.to_string())?
        {
            self.timeout_prefix_completions = self.timeout_prefix_completions.saturating_add(
                self.services
                    .drain_timeout_recovery_prefix_completion(&mut self.executor, cut)
                    .map_err(|error| error.to_string())?,
            );
        }
        self.assert_missing_proposal_serve_selected();
        Ok(())
    }

    /// Run one typed pacemaker transition while the exact Serve carrier remains selected.
    pub(in crate::sumeragi) fn service_pacemaker(&mut self) -> Result<(), String> {
        self.executor
            .set_ingress_physical_cut(self.ingress.next_physical_admission_ordinal())
            .map_err(|error| error.to_string())?;
        let _ = self
            .executor
            .step_pacemaker_once(Instant::now(), &mut self.services)
            .map_err(|error| error.to_string())?;
        self.assert_missing_proposal_serve_selected();
        Ok(())
    }

    /// Return whether the real reducer and production service both installed view one.
    pub(in crate::sumeragi) fn entered_view_one(&self) -> bool {
        self.executor.current_tag().view() == 1 && self.services.active_tag.view() == 1
    }

    /// Check the complete local + dual-remote timeout recovery result.
    pub(in crate::sumeragi) fn assert_complete(&self) {
        self.assert_missing_proposal_serve_selected();
        assert!(self.local_timeout_signature_completed);
        assert_eq!(self.remote_timeout_votes_admitted, 2);
        assert_eq!(self.timeout_prefix_completions, 1);
        assert_eq!(self.ingress.len(), 1, "only the missing-body Serve remains");
        assert!(self.entered_view_one());
        let observations = self
            .consensus_observations
            .lock()
            .expect("inspect selected-Serve consensus broadcasts");
        assert!(observations.iter().any(|(_, message)| matches!(
            &message.payload,
            wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                if vote.signer
                    == self.services.local_validator.expect("fixture is a validator")
        )));
        assert!(observations.iter().any(|(_, message)| matches!(
            &message.payload,
            wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate)
                if certificate.round.view == 0
                    && certificate
                        .groups
                        .iter()
                        .map(|group| group.signers.len())
                        .sum::<usize>()
                        == 3
        )));
    }

    fn assert_missing_proposal_serve_selected(&self) {
        let barrier = self
            .serve_gate
            .selected_barrier()
            .expect("inspect missing-proposal Serve barrier")
            .expect("missing-proposal Serve remains selected");
        assert_eq!(barrier.request_hash(), self.missing_proposal_request_hash);
    }
}

#[cfg(feature = "bls")]
impl Drop for SelectedServeTimeoutRecoveryFixture {
    fn drop(&mut self) {
        // This fixture drives the worker endpoints synchronously and has
        // no background thread to acknowledge a queued Shutdown command.
        drop(self.services.io.take());
    }
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
        version: crate::lane_consensus::LANE_EXECUTABLE_PAYLOAD_VERSION_V2,
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
