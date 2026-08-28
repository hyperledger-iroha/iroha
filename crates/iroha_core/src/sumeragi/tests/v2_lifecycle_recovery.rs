use super::*;
use crate::{
    governance::manifest::{GovernanceRules, LaneManifestRegistry, LaneManifestStatus},
    kura::Kura,
    prelude::World,
    query::store::LiveQueryStore,
    queue::{
        LaneQueueReservationKeyV1, LaneQueueReservationScopeV1, Queue, RoutingDecision,
        RoutingPlan, canonical_lane_queue_reservation_group_identity_projection,
        lane_queue_reservation_group_binding_from_ordered_keys,
    },
    state::State,
    sumeragi::{
        lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal,
        v2_apply::LaneReservationSnapshotPlannerEvidence,
        v2_core::{
            IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED, IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
            ProductionInFlightFirstReleaseCarrierProjection,
            ProductionInFlightFirstReleaseDecisionProjection,
            ProductionInFlightFirstReleaseHistoryProjection,
            ProductionInFlightFirstReleaseQueueProjection,
            ProductionInFlightFirstReleaseReleaseProjection,
            ProductionInFlightFirstReleaseSessionProjection,
        },
    },
    tx::AcceptedTransaction,
};
use iroha_config::{
    base::WithOrigin,
    kura::FsyncMode,
    parameters::{
        actual::{
            Kura as KuraConfig, LaneConfig as RuntimeLaneConfig, Nexus, Queue as QueueConfig,
        },
        defaults::kura::{
            BLOCKS_IN_MEMORY, FSYNC_INTERVAL, LANE_HISTORY_RETENTION, MERGE_LEDGER_CACHE_CAPACITY,
        },
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    ChainId, Level,
    account::{AccountDetails, AccountId, AccountValue},
    block::{
        BlockHeader,
        consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1},
        consensus_v2 as wire,
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    isi::Log,
    nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
    peer::PeerId,
    transaction::{FeePaymentIntent, TransactionBuilder, signed::TransactionEntrypoint},
};
use iroha_primitives::{numeric::Quantity, time::TimeSource};
use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use std::{
    borrow::Cow,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
};
use tempfile::TempDir;
fn lifecycle_key_pair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
        .expect("deterministic BLS lifecycle key")
}
fn lifecycle_lane_catalog() -> LaneCatalog {
    let primary = ModelLaneConfig::default();
    let autonomous = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "lifecycle-recovery".to_owned(),
        ..ModelLaneConfig::default()
    };
    LaneCatalog::new(
        NonZeroU32::new(2).expect("non-zero lane count"),
        vec![primary, autonomous],
    )
    .expect("two-lane recovery catalog")
}
fn lifecycle_runtime_lane_config() -> RuntimeLaneConfig {
    RuntimeLaneConfig::from_catalog(&lifecycle_lane_catalog())
}
fn lifecycle_kura_config(dir: &TempDir) -> KuraConfig {
    KuraConfig {
        init_mode: iroha_config::kura::InitMode::Strict,
        store_dir: WithOrigin::inline(dir.path().to_path_buf()),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: FSYNC_INTERVAL,
        lane_history_retention: LANE_HISTORY_RETENTION,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
    }
}
fn lifecycle_payload(
    signer: &KeyPair,
    lane_incarnation: Hash,
) -> (
    iroha_data_model::NetworkId,
    u64,
    crate::lane_consensus::LaneExecutablePayloadV1,
) {
    let context = lifecycle_context(signer);
    lifecycle_payload_for_validators(
        signer,
        &context,
        vec![PeerId::new(signer.public_key().clone())],
        lane_incarnation,
    )
}
fn lifecycle_payload_for_validators(
    producer_signer: &KeyPair,
    context: &wire::HeightContext,
    validator_set: Vec<PeerId>,
    lane_incarnation: Hash,
) -> (
    iroha_data_model::NetworkId,
    u64,
    crate::lane_consensus::LaneExecutablePayloadV1,
) {
    lifecycle_payload_for_validators_with_count(
        producer_signer,
        context,
        validator_set,
        lane_incarnation,
        1,
    )
}
fn lifecycle_payload_for_validators_with_count(
    producer_signer: &KeyPair,
    context: &wire::HeightContext,
    mut validator_set: Vec<PeerId>,
    lane_incarnation: Hash,
    transaction_count: usize,
) -> (
    iroha_data_model::NetworkId,
    u64,
    crate::lane_consensus::LaneExecutablePayloadV1,
) {
    assert!((1..=4).contains(&transaction_count));
    validator_set.sort();
    validator_set.dedup();
    let producer = PeerId::new(producer_signer.public_key().clone());
    assert!(
        validator_set.contains(&producer),
        "lifecycle payload producer must belong to the frozen validator set",
    );
    let entrypoints = (0..transaction_count)
        .map(|index| {
            let mut builder = TransactionBuilder::new(
                context.network_id,
                (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                Level::INFO,
                format!("lifecycle recovery payload {index}"),
            )])
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            );
            builder.set_nonce(
                NonZeroU32::new(u32::try_from(index + 1).expect("bounded lifecycle nonce"))
                    .expect("lifecycle nonce is non-zero"),
            );
            TransactionEntrypoint::External(
                builder.sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key()),
            )
        })
        .collect::<Vec<_>>();
    let entrypoint_hashes = entrypoints
        .iter()
        .map(|entrypoint| Hash::from(entrypoint.hash()))
        .collect::<Vec<_>>();
    let min_quorum = u32::try_from(
        crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1),
    )
    .expect("lifecycle validator quorum fits u32");
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: LaneId::new(1),
        dataspace_id: DataSpaceId::UNIVERSAL,
        lane_incarnation,
        proposal_height: 1,
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: Hash::new(b"lifecycle-recovery-subject"),
        payload_ownership_hash: Hash::new(b"lifecycle-recovery-ownership"),
        rbc_instance_hash: Hash::new(b"lifecycle-recovery-rbc"),
        accepted_candidate_indices: (0..u64::try_from(transaction_count)
            .expect("bounded lifecycle transaction count"))
            .collect(),
        accepted_transaction_hashes: entrypoint_hashes,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        validator_count: u32::try_from(validator_set.len())
            .expect("lifecycle validator count fits u32"),
        min_quorum,
        qc_mode_tag: "permissioned:lifecycle-recovery".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: Some(
            iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
                proposal_height: 1,
                proposal_view: 0,
                proposal_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"lifecycle-recovery-global-anchor",
                )),
            },
        ),
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let routing_plan = RoutingPlan::single(RoutingDecision::new(
        proposal.descriptor.lane_id,
        proposal.descriptor.dataspace_id,
    ));
    let network_id = context.network_id;
    let epoch = context.epoch;
    let (reservation_owner_hash, proposal_identity_hash) =
        autonomous_lane_reservation_identity_hashes_for_proposal(
            network_id,
            context.id(),
            epoch,
            &proposal,
            &producer,
        )
        .expect("derive height-bound reservation identities");
    let reservations = entrypoints
        .iter()
        .enumerate()
        .map(|(index, entrypoint)| LaneQueueReservationKeyV1 {
            version: LaneQueueReservationKeyV1::VERSION,
            entrypoint_hash: entrypoint.hash(),
            queue_plan_admission_binding_hash: Hash::new_from_chunks(&[
                b"lifecycle-recovery-queue-plan-admission\0",
                &u64::try_from(index)
                    .expect("bounded lifecycle index")
                    .to_be_bytes(),
            ]),
            routing_plan_digest: routing_plan.digest(),
            coordinator_leg: routing_plan.coordinator_leg(),
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_incarnation,
            proposal_height: proposal.descriptor.proposal_height,
            lane_block_height: proposal.descriptor.lane_block_height,
            lane_block_view: proposal.descriptor.lane_block_view,
            reservation_owner_hash,
            proposal_identity_hash,
        })
        .collect::<Vec<_>>();
    let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        proposal,
        entrypoints,
        reservations,
        vec![routing_plan; transaction_count],
        vec![None; transaction_count],
        producer,
        producer_signer.private_key(),
    )
    .expect("signed lifecycle recovery payload");
    (network_id, epoch, payload)
}
fn lifecycle_binding_and_live_state(
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    local_peer: &PeerId,
) -> (
    AutonomousLifecycleAttemptBindingV1,
    ProductionInFlightFirstReleaseStateProjection,
) {
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
            .expect("bind lifecycle reservation group");
    let binding = AutonomousLifecycleAttemptBindingV1::from_payload(
        lifecycle_context_for_peer(local_peer).id(),
        payload.origin_proposal.descriptor.lane_block_height,
        payload,
        reservation_group,
        local_peer,
    )
    .expect("bind lifecycle attempt");
    let validator_count = u8::try_from(binding.validator_set_identity().2)
        .expect("lifecycle validator count fits the refinement width");
    let validator_mask = if validator_count == 128 {
        u128::MAX
    } else {
        (1_u128 << validator_count) - 1
    };
    let (_, local_actor) = binding.local_validator_identity();
    let producer = binding.producer_actor_projection();
    let live_state = ProductionInFlightFirstReleaseStateProjection {
        validator_count,
        producer,
        producer_selected_owner: producer,
        replicated_carrier_owners: validator_mask & !producer,
        payload_binding_a: producer | local_actor,
        binding_a: canonical_lane_queue_reservation_group_identity_projection(reservation_group),
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_SELECTED,
            selected_count: reservation_group.reservation_count,
            reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_LIVE,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection {
            kura_active: local_actor,
            ..ProductionInFlightFirstReleaseCarrierProjection::default()
        },
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: producer | local_actor,
            producer_alive: true,
            ..ProductionInFlightFirstReleaseSessionProjection::default()
        },
        history: ProductionInFlightFirstReleaseHistoryProjection {
            ever_queue_plan_v1: true,
            ever_reservation_v1: true,
            ..ProductionInFlightFirstReleaseHistoryProjection::default()
        },
        decision: ProductionInFlightFirstReleaseDecisionProjection::default(),
        release: ProductionInFlightFirstReleaseReleaseProjection::default(),
    };
    (binding, live_state)
}
fn lifecycle_context_for_peer(local_peer: &PeerId) -> wire::HeightContext {
    let mut validators = vec![local_peer.clone()];
    validators.extend(
        (91_u8..=93).map(|seed| PeerId::new(lifecycle_key_pair(seed).public_key().clone())),
    );
    validators.sort();
    validators.dedup();
    assert_eq!(validators.len(), 4, "lifecycle context validator set");
    let roster = validators
        .into_iter()
        .map(|validator| wire::ValidatorPower {
            validator,
            power: 1,
        })
        .collect::<Vec<_>>();
    wire::HeightContext {
        network_id: crate::sumeragi::synthetic_network_id("lifecycle-recovery-test"),
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        epoch_end_height: u64::MAX,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: wire::DualQuorum::from_roster(&roster).expect("four-validator quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"lifecycle-empty-nexus"),
        execution_policy_hash: Hash::new(b"lifecycle-empty-policy"),
        da_layout: wire::SumeragiV2GenesisContextParameters::recommended().da_layout,
        leader_seed: [0x55; 32],
    }
}
fn lifecycle_context(key_pair: &KeyPair) -> wire::HeightContext {
    lifecycle_context_for_peer(&PeerId::new(key_pair.public_key().clone()))
}
fn open_lifecycle_recovery_state(
    kura_config: &KuraConfig,
    lane_config: &RuntimeLaneConfig,
    context: &wire::HeightContext,
    nexus: &Nexus,
) -> (Arc<Kura>, State) {
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(kura_config, lane_config)
        .expect("open lifecycle Kura");
    let mut state = State::try_new_with_chain_and_network_id_with_default_telemetry(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        ChainId::from("lifecycle-recovery-test"),
        context.network_id,
    )
    .expect("construct lifecycle State");
    let configured_incarnations =
        crate::state::derive_static_lane_incarnations(&nexus.lane_catalog);
    let configured_catalog_hash = kura
        .configured_lane_catalog_baseline()
        .expect("read lifecycle Kura configured-catalog baseline")
        .expect("lifecycle Kura has a configured-catalog baseline");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        configured_incarnations[&LaneId::SINGLE],
        configured_catalog_hash,
    )
    .expect("anchor lifecycle Kura configured-primary geometry");
    state.install_pre_genesis_nexus_for_testing(nexus.clone());
    kura.restore_lane_segments(lane_config)
        .expect("finish lifecycle Kura authenticated lane restore");
    (kura, state)
}
fn install_lifecycle_queue_plan_authority(state: &mut State, validator_keys: &[&KeyPair]) {
    assert_eq!(
        validator_keys.len(),
        4,
        "the lifecycle QueuePlan fixture requires the exact f=1 committee",
    );
    let validator_peers = validator_keys
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    let validator_accounts = validator_keys
        .iter()
        .map(|key| AccountId::new(key.public_key().clone()))
        .collect::<Vec<_>>();
    {
        let mut world_block = state.world.block();
        {
            let mut peers = world_block.peers_mut_for_testing().transaction();
            peers.clear();
            peers.extend(validator_peers.iter().cloned());
            peers.apply();
        }
        world_block.accounts.insert(
            (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world_block.commit();
    }
    for validator_key in validator_keys {
        let validator_pop = iroha_crypto::bls_normal_pop_prove(validator_key.private_key())
            .expect("derive lifecycle QueuePlan validator PoP");
        state
            .world
            .register_validator_pop_for_testing(validator_key.public_key().clone(), validator_pop);
    }
    {
        let mut topology = state.commit_topology.block();
        topology.clear();
        topology.extend(validator_peers);
        topology.commit();
    }
    let statuses = state
        .nexus_snapshot()
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| {
            (
                lane.id,
                LaneManifestStatus {
                    lane: lane.id,
                    alias: lane.alias.clone(),
                    dataspace: lane.dataspace_id,
                    visibility: lane.visibility.clone(),
                    storage: lane.storage.clone(),
                    governance: None,
                    manifest_path: Some(std::path::PathBuf::from(format!(
                        "/test/lifecycle-recovery-lane-{}.json",
                        lane.id.as_u32(),
                    ))),
                    governance_rules: Some(GovernanceRules {
                        validators: validator_accounts.clone(),
                        ..GovernanceRules::default()
                    }),
                    privacy_commitments: Vec::new(),
                },
            )
        })
        .collect();
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));
}
fn open_lifecycle_recovery_queue(
    queue_dir: &TempDir,
    state: &State,
    expect_empty_replay: bool,
) -> Queue {
    let time_source = TimeSource::new_system();
    let queue = Queue::test(QueueConfig::default(), &time_source);
    queue
        .install_plan_journal(
            queue_dir.path().join("queue-plan.norito"),
            1024 * 1024,
            true,
        )
        .expect("install lifecycle QueuePlan journal");
    queue
        .install_lane_reservation_journal(
            queue_dir.path().join("lane-reservation.norito"),
            1024 * 1024,
        )
        .expect("install lifecycle reservation journal");
    let replay = queue
        .replay_plan_journal(state)
        .expect("publish lifecycle QueuePlan replay receipt");
    if expect_empty_replay {
        assert_eq!(replay, Default::default());
    }
    queue
}

fn lifecycle_payload_with_exact_ordinary_fifo(
    producer_signer: &KeyPair,
    context: &wire::HeightContext,
    state: &State,
    queue: &Queue,
) -> crate::lane_consensus::LaneExecutablePayloadV1 {
    let lane_incarnation = state
        .lane_incarnations_snapshot()
        .get(&LaneId::new(1))
        .copied()
        .expect("State lifecycle-lane incarnation");
    let validator_set = context
        .roster
        .iter()
        .map(|validator| validator.validator.clone())
        .collect::<Vec<_>>();
    let (network_id, epoch, template) = lifecycle_payload_for_validators_with_count(
        producer_signer,
        context,
        validator_set,
        lane_incarnation,
        2,
    );
    let entrypoints = template
        .entrypoints
        .iter()
        .map(|entrypoint| {
            let TransactionEntrypoint::External(transaction) = entrypoint else {
                panic!("lifecycle FIFO fixture uses only external transactions");
            };
            let mut payload = transaction.payload().clone();
            payload.admission_intent =
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced;
            TransactionEntrypoint::External(
                TransactionBuilder::from_payload(payload)
                    .expect("rebuild signature-bound lifecycle QueuePlan transaction")
                    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key()),
            )
        })
        .collect::<Vec<_>>();
    let mut proposal = template.origin_proposal.clone();
    proposal.descriptor.lane_id = LaneId::new(1);
    proposal.descriptor.lane_incarnation = lane_incarnation;
    proposal.descriptor.accepted_transaction_hashes = entrypoints
        .iter()
        .map(|entrypoint| Hash::from(entrypoint.hash()))
        .collect();
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    proposal.proposal_hash = proposal.computed_proposal_hash();
    for entrypoint in &entrypoints {
        let TransactionEntrypoint::External(transaction) = entrypoint else {
            panic!("lifecycle FIFO fixture uses only external transactions");
        };
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
        let routing_plan = queue
            .route_plan_with_state(&accepted, state)
            .expect("resolve lifecycle FIFO routing plan");
        assert_eq!(
            routing_plan.coordinator_route().lane_id,
            proposal.descriptor.lane_id,
            "lifecycle FIFO fixture must use the autonomous lifecycle Queue route",
        );
        let admission_context = queue
            .plan_admission_context_with_state(state, &routing_plan)
            .expect("capture lifecycle FIFO admission context");
        let admission = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
            state.network_id_ref(),
            accepted.entrypoint(),
            &routing_plan,
            admission_context,
            queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("bind lifecycle FIFO QueuePlan admission");
        queue
            .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                accepted,
                state,
                routing_plan,
                &admission,
            )
            .expect("durably admit lifecycle FIFO transaction");
        state
            .install_queue_plan_pending_binding_for_test(&admission)
            .expect("install lifecycle FIFO QueuePlan registry value");
    }
    let producer = PeerId::new(producer_signer.public_key().clone());
    let (reservation_owner_hash, proposal_identity_hash) =
        autonomous_lane_reservation_identity_hashes_for_proposal(
            network_id,
            context.id(),
            epoch,
            &proposal,
            &producer,
        )
        .expect("derive lifecycle FIFO reservation identities");
    let scope = LaneQueueReservationScopeV1 {
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash,
        proposal_identity_hash,
    };
    let reserved = queue
        .reserve_transactions_for_lane(
            state,
            scope,
            NonZeroUsize::new(entrypoints.len()).expect("non-empty lifecycle FIFO batch"),
        )
        .expect("reserve lifecycle FIFO batch");
    assert_eq!(reserved.len(), entrypoints.len());
    let reservation_keys = reserved
        .iter()
        .map(|reservation| *reservation.key())
        .collect::<Vec<_>>();
    let routing_plans = reserved
        .iter()
        .map(|reservation| reservation.routing_plan().clone())
        .collect::<Vec<_>>();
    let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        proposal,
        entrypoints,
        reservation_keys,
        routing_plans,
        vec![None; reserved.len()],
        producer,
        producer_signer.private_key(),
    )
    .expect("sign exact lifecycle FIFO payload");
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&payload.reservation_keys)
            .expect("restore exact lifecycle ordinary FIFO"),
        payload.reservation_keys.len(),
    );
    assert_eq!(
        queue.fifo_snapshot_for_test(),
        payload
            .entrypoints
            .iter()
            .map(TransactionEntrypoint::hash)
            .collect::<Vec<_>>()
    );
    assert!(queue.live_lane_reservations().is_empty());
    payload
}

#[derive(Clone, Copy, Debug)]
enum NonproducerReplicaQueueCut {
    ExactOrdinaryFifo,
    StrictQueueAbsent,
}

#[derive(Clone, Copy, Debug)]
enum NonproducerRetirementClaimPrefix {
    ReleasePending,
    ReplicaReleased,
}
#[derive(Clone, Copy, Debug)]
enum LifecycleRecoveryPostCasBoundary {
    Crashed,
    PreparedRecover,
    RecoveredLive,
    PreparedRehydration,
    FinalLive,
}
impl LifecycleRecoveryPostCasBoundary {
    const fn cas_ordinal(self) -> u64 {
        match self {
            Self::Crashed => 1,
            Self::PreparedRecover => 2,
            Self::RecoveredLive => 3,
            Self::PreparedRehydration => 4,
            Self::FinalLive => 5,
        }
    }
    fn assert_durable_cursor(self, cursor: &AutonomousLifecycleCursorV1, local_actor: u128) {
        assert_eq!(cursor.sequence(), self.cas_ordinal() + 1);
        assert_eq!(cursor.owner_generation(), 2);
        match self {
            Self::Crashed => {
                assert_eq!(
                    cursor.phase_kind(),
                    AutonomousLifecycleCursorPhaseKindV1::Crashed
                );
                assert_eq!(cursor.source_generation(), Some(1));
                let crashed = cursor
                    .after_projection()
                    .expect("validate durable Crashed projection")
                    .expect("Crashed cursor has an after-state");
                assert_eq!(crashed.session.crashed & local_actor, local_actor);
            }
            Self::PreparedRecover | Self::PreparedRehydration => {
                assert_eq!(
                    cursor.phase_kind(),
                    AutonomousLifecycleCursorPhaseKindV1::Prepared
                );
                assert_eq!(cursor.source_generation(), None);
                let transition = cursor
                    .prepared_transition_projection()
                    .expect("validate durable Prepared transition")
                    .expect("Prepared cursor has a transition");
                let expected_action = match self {
                    Self::PreparedRecover => IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER,
                    Self::PreparedRehydration => {
                        IN_FLIGHT_FIRST_RELEASE_ACTION_REHYDRATE_LOCAL_KURA_CUSTODY
                    }
                    Self::Crashed | Self::RecoveredLive | Self::FinalLive => unreachable!(),
                };
                assert_eq!(transition.action, expected_action);
                if matches!(self, Self::PreparedRehydration) {
                    assert_eq!(transition.before.session.bodies & local_actor, 0);
                    assert_eq!(transition.after.session.bodies & local_actor, local_actor);
                }
            }
            Self::RecoveredLive | Self::FinalLive => {
                assert_eq!(
                    cursor.phase_kind(),
                    AutonomousLifecycleCursorPhaseKindV1::Live
                );
                assert_eq!(cursor.source_generation(), None);
                let live = cursor
                    .before_projection()
                    .expect("validate interrupted Live projection");
                assert_eq!(live.session.crashed & local_actor, 0);
                let expected_bodies = if matches!(self, Self::FinalLive) {
                    local_actor
                } else {
                    0
                };
                assert_eq!(live.session.bodies & local_actor, expected_bodies);
            }
        }
    }
}
#[derive(Debug)]
struct LifecycleCursorCasInterruption;
#[test]
fn generation_takeover_runs_crash_recover_and_rehydrate_then_stutters() {
    let temp_dir = TempDir::new().expect("lifecycle Kura directory");
    let queue_dir = TempDir::new().expect("lifecycle Queue journal directory");
    let kura_config = lifecycle_kura_config(&temp_dir);
    let lane_config = lifecycle_runtime_lane_config();
    let signer = lifecycle_key_pair(31);
    let local_peer = PeerId::new(signer.public_key().clone());
    let context = lifecycle_context(&signer);
    let producer_signer = lifecycle_key_pair(91);
    context
        .validate()
        .expect("lifecycle startup context must be structurally valid");
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&kura_config, &lane_config)
        .expect("initial Kura");
    let mut state = State::try_new_with_chain_and_network_id_with_default_telemetry(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        ChainId::from("lifecycle-recovery-test"),
        context.network_id,
    )
    .expect("construct lifecycle State");
    let nexus = Nexus {
        lane_catalog: lifecycle_lane_catalog(),
        ..Nexus::default()
    };
    state.install_pre_genesis_nexus_for_testing(nexus.clone());
    let lane_incarnation = state
        .lane_incarnations_snapshot()
        .get(&LaneId::new(1))
        .copied()
        .expect("State lifecycle lane incarnation");
    let validator_set = context
        .roster
        .iter()
        .map(|validator| validator.validator.clone())
        .collect();
    let (network_id, epoch, payload) = lifecycle_payload_for_validators(
        &producer_signer,
        &context,
        validator_set,
        lane_incarnation,
    );
    let (binding, live_state) = lifecycle_binding_and_live_state(&payload, &local_peer);
    let local_actor = binding.local_validator_identity().1;
    assert_ne!(
        local_actor,
        binding.producer_actor_projection(),
        "empty local Queue recovery must exercise replicated observer custody",
    );
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind initial local peer");
    let generation_one = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim first process generation");
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist lifecycle payload before its first Live cursor");
    let initial_live = sign_lifecycle_cursor(
        &signer,
        &local_peer,
        &payload.origin_proposal.descriptor.validator_set,
        1,
        None,
        binding.clone(),
        AutonomousLifecycleCursorPhaseV1::live(generation_one.generation(), live_state)
            .expect("construct first-generation Live cursor"),
    )
    .expect("sign first-generation Live cursor");
    let (_, initial_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_one)
        .expect("read absent lifecycle cursor")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(initial_lease, initial_live.clone())
            .expect("publish first-generation Live cursor")
            .cursor(),
        Some(&initial_live),
        "first-generation publication must return its exact durable Live cursor",
    );
    drop(generation_one);
    drop(state);
    drop(kura);
    let (restarted, _) =
        Kura::open_test_kura_with_configured_lane_config(&kura_config, &lane_config)
            .expect("restart Kura");
    let mut restarted_state = State::try_new_with_chain_and_network_id_with_default_telemetry(
        World::default(),
        Arc::clone(&restarted),
        LiveQueryStore::start_test(),
        ChainId::from("lifecycle-recovery-test"),
        context.network_id,
    )
    .expect("reconstruct lifecycle State");
    restarted_state.install_pre_genesis_nexus_for_testing(nexus);
    assert_eq!(
        restarted_state
            .lane_incarnations_snapshot()
            .get(&LaneId::new(1))
            .copied(),
        Some(lane_incarnation),
    );
    restarted
        .bind_local_peer_id(local_peer.clone())
        .expect("rebind restarted local peer");
    let generation_two = restarted
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim second process generation");
    assert_eq!(generation_two.generation(), 2);
    let (_time_handle, time_source) = TimeSource::new_mock(core::time::Duration::ZERO);
    let queue = Queue::test(QueueConfig::default(), &time_source);
    queue
        .install_plan_journal(
            queue_dir.path().join("queue-plan.norito"),
            1024 * 1024,
            true,
        )
        .expect("install lifecycle QueuePlan journal");
    queue
        .install_lane_reservation_journal(
            queue_dir.path().join("lane-reservation.norito"),
            1024 * 1024,
        )
        .expect("install lifecycle reservation journal");
    assert_eq!(
        queue
            .replay_plan_journal(&restarted_state)
            .expect("publish lifecycle QueuePlan replay receipt"),
        Default::default(),
    );
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture lifecycle Queue snapshot");
    assert!(snapshot.is_empty());
    let planner =
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(snapshot.clone(), Vec::new());
    let recovered_startup = reconcile_autonomous_lifecycle_startup(
        &restarted_state,
        &queue,
        restarted.as_ref(),
        &context,
        planner,
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        Some(&generation_two),
        &local_peer,
        &signer,
    )
    .expect("reconcile stale lifecycle generation");
    assert_eq!(recovered_startup.completed_bootstraps(), 0);
    assert_eq!(
        recovered_startup.recovered_attempts(),
        1,
        "the stale generation must publish Crash, Recover, and rehydration successors",
    );
    let (returned_snapshot, receipt, pending_groups) = recovered_startup.into_queue_handoff();
    assert_eq!(returned_snapshot, snapshot);
    assert!(pending_groups.is_empty());
    assert!(
        queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)
            .expect("revalidate generation-recovery Queue receipt"),
        "generation recovery must preserve the exact combined V1 receipt",
    );
    drop(receipt);
    let read = restarted
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_two)
        .expect("read recovered lifecycle cursor");
    let cursor = read.cursor().expect("recovered cursor");
    assert_eq!(
        cursor.phase_kind(),
        AutonomousLifecycleCursorPhaseKindV1::Live
    );
    assert_eq!(cursor.owner_generation(), 2);
    assert_eq!(cursor.sequence(), 6);
    let recovered = cursor
        .before_projection()
        .expect("validate recovered Live projection");
    assert_eq!(recovered.session.crashed & local_actor, 0);
    assert_eq!(recovered.session.bodies & local_actor, local_actor);
    assert!(recovered.session.producer_alive);
    drop(read);
    let repeated_snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("recapture lifecycle Queue snapshot");
    let repeated = reconcile_autonomous_lifecycle_startup(
        &restarted_state,
        &queue,
        restarted.as_ref(),
        &context,
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
            repeated_snapshot.clone(),
            Vec::new(),
        ),
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        Some(&generation_two),
        &local_peer,
        &signer,
    )
    .expect("repeat lifecycle recovery");
    assert_eq!(
        repeated.recovered_attempts(),
        0,
        "an exact current-generation hydrated Live cursor must stutter",
    );
    let (repeated_snapshot_handoff, repeated_receipt, pending_groups) =
        repeated.into_queue_handoff();
    assert_eq!(repeated_snapshot_handoff, repeated_snapshot);
    assert!(pending_groups.is_empty());
    assert!(
        queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(
                &repeated_receipt,
                &repeated_snapshot,
            )
            .expect("revalidate repeated Queue receipt"),
    );
    let repeated = restarted
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_two)
        .expect("read idempotent lifecycle cursor");
    assert_eq!(repeated.cursor().expect("idempotent cursor").sequence(), 6,);
}
fn exercise_lifecycle_recovery_post_cas_interruption(boundary: LifecycleRecoveryPostCasBoundary) {
    let kura_dir = TempDir::new().expect("lifecycle interruption Kura directory");
    let queue_dir = TempDir::new().expect("lifecycle interruption Queue directory");
    let kura_config = lifecycle_kura_config(&kura_dir);
    let lane_config = lifecycle_runtime_lane_config();
    let signer = lifecycle_key_pair(61);
    let local_peer = PeerId::new(signer.public_key().clone());
    let producer_signer = lifecycle_key_pair(91);
    let context = lifecycle_context(&signer);
    context
        .validate()
        .expect("interruption context must be structurally valid");
    let nexus = Nexus {
        lane_catalog: lifecycle_lane_catalog(),
        ..Nexus::default()
    };
    let (kura, state) = open_lifecycle_recovery_state(&kura_config, &lane_config, &context, &nexus);
    let lane_incarnation = state
        .lane_incarnations_snapshot()
        .get(&LaneId::new(1))
        .copied()
        .expect("State lifecycle lane incarnation");
    let validator_set = context
        .roster
        .iter()
        .map(|validator| validator.validator.clone())
        .collect();
    let (network_id, epoch, payload) = lifecycle_payload_for_validators(
        &producer_signer,
        &context,
        validator_set,
        lane_incarnation,
    );
    let (binding, live_state) = lifecycle_binding_and_live_state(&payload, &local_peer);
    let local_actor = binding.local_validator_identity().1;
    assert_ne!(
        local_actor,
        binding.producer_actor_projection(),
        "interruption fixture must exercise replicated observer custody",
    );
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind initial interruption peer");
    let generation_one = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim first interruption generation");
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist interruption payload");
    let initial_live = sign_lifecycle_cursor(
        &signer,
        &local_peer,
        &payload.origin_proposal.descriptor.validator_set,
        1,
        None,
        binding.clone(),
        AutonomousLifecycleCursorPhaseV1::live(generation_one.generation(), live_state)
            .expect("construct initial interruption Live cursor"),
    )
    .expect("sign initial interruption Live cursor");
    let (_, initial_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_one)
        .expect("read absent interruption cursor")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(initial_lease, initial_live.clone())
            .expect("publish initial interruption Live cursor")
            .cursor(),
        Some(&initial_live),
        "interruption setup must return its exact durable Live cursor",
    );
    drop(generation_one);
    drop(state);
    drop(kura);
    let (restarted, restarted_state) =
        open_lifecycle_recovery_state(&kura_config, &lane_config, &context, &nexus);
    restarted
        .bind_local_peer_id(local_peer.clone())
        .expect("bind second-generation interruption peer");
    let generation_two = restarted
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim second interruption generation");
    assert_eq!(generation_two.generation(), 2);
    let queue = open_lifecycle_recovery_queue(&queue_dir, &restarted_state, true);
    let snapshot_before_ownership_plan = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture pre-interruption Queue snapshot");
    assert!(snapshot_before_ownership_plan.is_empty());
    let quarantine_before_ownership_plan = queue.lane_reservation_startup_reconciliation_pending();
    let receipt_before_ownership_plan = queue
        .bind_lane_reservation_startup_reconciliation_receipt(&snapshot_before_ownership_plan)
        .expect("bind pre-interruption Queue receipt")
        .expect("pre-interruption Queue snapshot stays immutable");
    let mut remaining_successful_cas = boundary.cas_ordinal();
    install_post_lifecycle_cursor_cas_hook_for_test(move |cursor| {
        remaining_successful_cas = remaining_successful_cas
            .checked_sub(1)
            .expect("interruption hook cannot outlive its selected CAS");
        if remaining_successful_cas == 0 {
            boundary.assert_durable_cursor(cursor, local_actor);
            std::panic::panic_any(LifecycleCursorCasInterruption);
        }
    });
    let interruption = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let planner = LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
            snapshot_before_ownership_plan.clone(),
            Vec::new(),
        );
        let _unexpected = reconcile_autonomous_lifecycle_startup(
            &restarted_state,
            &queue,
            restarted.as_ref(),
            &context,
            planner,
            AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
            Some(&generation_two),
            &local_peer,
            &signer,
        )
        .expect("selected post-CAS interruption must stop lifecycle reconciliation");
    }));
    clear_post_lifecycle_cursor_cas_hook_for_test();
    let interruption = interruption.expect_err("post-CAS interruption hook must fire");
    assert!(
        interruption.is::<LifecycleCursorCasInterruption>(),
        "{boundary:?} interrupted at an unexpected failure boundary",
    );
    let interrupted_read = restarted
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_two)
        .expect("read the durably interrupted cursor");
    boundary.assert_durable_cursor(
        interrupted_read
            .cursor()
            .expect("post-CAS interruption retains its cursor"),
        local_actor,
    );
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recapture Queue snapshot before ownership-plan application"),
        snapshot_before_ownership_plan,
        "{boundary:?} cursor publication must not apply a Queue ownership plan",
    );
    assert_eq!(
        queue.lane_reservation_startup_reconciliation_pending(),
        quarantine_before_ownership_plan,
        "{boundary:?} cursor publication must not change Queue quarantine",
    );
    assert!(
        queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(
                &receipt_before_ownership_plan,
                &snapshot_before_ownership_plan,
            )
            .expect("revalidate the pre-ownership-plan Queue receipt"),
        "{boundary:?} cursor publication must preserve the exact Queue receipt",
    );
    drop(interrupted_read);
    drop(receipt_before_ownership_plan);
    drop(queue);
    drop(generation_two);
    drop(restarted_state);
    drop(restarted);
    let (reopened, reopened_state) =
        open_lifecycle_recovery_state(&kura_config, &lane_config, &context, &nexus);
    reopened
        .bind_local_peer_id(local_peer.clone())
        .expect("bind post-interruption peer");
    let generation_three = reopened
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim post-interruption generation");
    assert_eq!(generation_three.generation(), 3);
    let reopened_queue = open_lifecycle_recovery_queue(&queue_dir, &reopened_state, true);
    let reopened_snapshot = reopened_queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture reopened Queue snapshot");
    assert_eq!(reopened_snapshot, snapshot_before_ownership_plan);
    let recovered = reconcile_autonomous_lifecycle_startup(
        &reopened_state,
        &reopened_queue,
        reopened.as_ref(),
        &context,
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
            reopened_snapshot.clone(),
            Vec::new(),
        ),
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        Some(&generation_three),
        &local_peer,
        &signer,
    )
    .expect("reconcile the post-interruption generation");
    assert_eq!(recovered.completed_bootstraps(), 0);
    assert_eq!(recovered.recovered_attempts(), 1);
    let (returned_snapshot, recovered_receipt, pending_groups) = recovered.into_queue_handoff();
    assert_eq!(returned_snapshot, reopened_snapshot);
    assert!(pending_groups.is_empty());
    assert!(
        reopened_queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(
                &recovered_receipt,
                &reopened_snapshot,
            )
            .expect("revalidate post-interruption Queue receipt"),
    );
    let final_read = reopened
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_three)
        .expect("read post-interruption Live cursor");
    let final_cursor = final_read
        .cursor()
        .expect("post-interruption recovery has a cursor")
        .clone();
    assert_eq!(
        final_cursor.phase_kind(),
        AutonomousLifecycleCursorPhaseKindV1::Live
    );
    assert_eq!(final_cursor.owner_generation(), 3);
    assert_eq!(final_cursor.source_generation(), None);
    assert_eq!(final_cursor.sequence(), boundary.cas_ordinal() + 6);
    assert_eq!(final_cursor.binding(), &binding);
    let final_live = final_cursor
        .before_projection()
        .expect("validate post-interruption Live projection");
    assert_eq!(final_live.session.crashed & local_actor, 0);
    assert_eq!(final_live.session.bodies & local_actor, local_actor);
    assert!(final_live.session.producer_alive);
    drop(final_read);
    let repeated_snapshot = reopened_queue
        .lane_reservation_reconciliation_snapshot()
        .expect("recapture post-interruption Queue snapshot");
    assert_eq!(repeated_snapshot, reopened_snapshot);
    let repeated = reconcile_autonomous_lifecycle_startup(
        &reopened_state,
        &reopened_queue,
        reopened.as_ref(),
        &context,
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
            repeated_snapshot.clone(),
            Vec::new(),
        ),
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        Some(&generation_three),
        &local_peer,
        &signer,
    )
    .expect("repeat post-interruption lifecycle recovery");
    assert_eq!(
        repeated.recovered_attempts(),
        0,
        "{boundary:?} must stutter after exact Live convergence",
    );
    let (repeated_snapshot_handoff, repeated_receipt, pending_groups) =
        repeated.into_queue_handoff();
    assert_eq!(repeated_snapshot_handoff, repeated_snapshot);
    assert!(pending_groups.is_empty());
    assert!(
        reopened_queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(
                &repeated_receipt,
                &repeated_snapshot,
            )
            .expect("revalidate repeated post-interruption Queue receipt"),
    );
    let repeated_read = reopened
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_three)
        .expect("read stuttered post-interruption cursor");
    assert_eq!(
        repeated_read.cursor().expect("stuttered cursor"),
        &final_cursor,
        "{boundary:?} repeat must preserve the exact current-generation Live cursor",
    );
}
#[test]
fn every_lifecycle_recovery_cursor_cas_boundary_survives_restart() {
    for boundary in [
        LifecycleRecoveryPostCasBoundary::Crashed,
        LifecycleRecoveryPostCasBoundary::PreparedRecover,
        LifecycleRecoveryPostCasBoundary::RecoveredLive,
        LifecycleRecoveryPostCasBoundary::PreparedRehydration,
        LifecycleRecoveryPostCasBoundary::FinalLive,
    ] {
        exercise_lifecycle_recovery_post_cas_interruption(boundary);
    }
}
#[test]
fn local_producer_recovery_requires_the_exact_current_queue_owner() {
    let signer = lifecycle_key_pair(41);
    let local_peer = PeerId::new(signer.public_key().clone());
    let context = lifecycle_context(&signer);
    let (_, _, payload) = lifecycle_payload_for_validators_with_count(
        &signer,
        &context,
        vec![local_peer.clone()],
        Hash::new(b"producer-custody-incarnation"),
        2,
    );
    let (binding, live_state) = lifecycle_binding_and_live_state(&payload, &local_peer);
    assert_eq!(
        binding.local_validator_identity().1,
        binding.producer_actor_projection(),
        "fixture must exercise local producer custody",
    );
    let cursor = sign_lifecycle_cursor(
        &signer,
        &local_peer,
        &payload.origin_proposal.descriptor.validator_set,
        1,
        None,
        binding.clone(),
        AutonomousLifecycleCursorPhaseV1::live(1, live_state)
            .expect("construct local-producer Live cursor"),
    )
    .expect("sign local-producer Live cursor");
    let no_groups = std::collections::BTreeMap::new();
    let missing = require_local_producer_queue_owner(&payload, &cursor, &no_groups)
        .expect_err("payload bytes alone must not replace producer Queue custody");
    assert!(missing.contains("lost its exact current Queue reservation owner"));
    let mut wrong_keys = payload.reservation_keys.clone();
    wrong_keys[0].queue_plan_admission_binding_hash = Hash::new(b"another-admission-binding");
    let mut conflicting_groups = std::collections::BTreeMap::new();
    conflicting_groups.insert(binding.reservation_group_binding().identity, wrong_keys);
    require_local_producer_queue_owner(&payload, &cursor, &conflicting_groups)
        .expect_err("same-slot but byte-different Queue custody must fail closed");
    let mut reversed_keys = payload.reservation_keys.clone();
    reversed_keys.reverse();
    assert_ne!(
        lane_queue_reservation_group_binding_from_ordered_keys(reversed_keys.iter())
            .expect("bind reversed producer Queue group"),
        binding.reservation_group_binding(),
    );
    let mut reordered_groups = std::collections::BTreeMap::new();
    reordered_groups.insert(binding.reservation_group_binding().identity, reversed_keys);
    require_local_producer_queue_owner(&payload, &cursor, &reordered_groups)
        .expect_err("reordered current Queue custody must fail closed");
    let mut exact_groups = std::collections::BTreeMap::new();
    exact_groups.insert(
        binding.reservation_group_binding().identity,
        payload.reservation_keys.clone(),
    );
    require_local_producer_queue_owner(&payload, &cursor, &exact_groups)
        .expect("the byte-exact current Queue group authenticates producer recovery custody");
}

fn exercise_nonproducer_retired_attempt_startup(
    queue_cut: NonproducerReplicaQueueCut,
    claim_prefix: NonproducerRetirementClaimPrefix,
    exercise_complete_claim_presweep: bool,
) {
    let kura_dir = TempDir::new().expect("nonproducer retirement Kura directory");
    let queue_dir = TempDir::new().expect("nonproducer retirement Queue directory");
    let kura_config = lifecycle_kura_config(&kura_dir);
    let lane_config = lifecycle_runtime_lane_config();
    let local_signer = lifecycle_key_pair(71);
    let local_peer = PeerId::new(local_signer.public_key().clone());
    let producer_signer = lifecycle_key_pair(91);
    let context = lifecycle_context(&local_signer);
    context
        .validate()
        .expect("nonproducer retirement context must be structurally valid");
    let mut nexus = Nexus {
        lane_catalog: lifecycle_lane_catalog(),
        ..Nexus::default()
    };
    nexus.routing_policy.default_lane = LaneId::new(1);
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    let (kura, mut state) =
        open_lifecycle_recovery_state(&kura_config, &lane_config, &context, &nexus);
    let auxiliary_signer_92 = lifecycle_key_pair(92);
    let auxiliary_signer_93 = lifecycle_key_pair(93);
    install_lifecycle_queue_plan_authority(
        &mut state,
        &[
            &local_signer,
            &producer_signer,
            &auxiliary_signer_92,
            &auxiliary_signer_93,
        ],
    );
    let queue = open_lifecycle_recovery_queue(&queue_dir, &state, true);
    let payload = match queue_cut {
        NonproducerReplicaQueueCut::ExactOrdinaryFifo => {
            lifecycle_payload_with_exact_ordinary_fifo(&producer_signer, &context, &state, &queue)
        }
        NonproducerReplicaQueueCut::StrictQueueAbsent => {
            let lane_incarnation = state
                .lane_incarnations_snapshot()
                .get(&LaneId::new(1))
                .copied()
                .expect("State lifecycle lane incarnation");
            let validator_set = context
                .roster
                .iter()
                .map(|validator| validator.validator.clone())
                .collect();
            lifecycle_payload_for_validators_with_count(
                &producer_signer,
                &context,
                validator_set,
                lane_incarnation,
                2,
            )
            .2
        }
    };
    let network_id = payload.network_id;
    let epoch = payload.epoch;
    let (binding, live_state) = lifecycle_binding_and_live_state(&payload, &local_peer);
    let (_, local_actor) = binding.local_validator_identity();
    assert_ne!(
        local_actor,
        binding.producer_actor_projection(),
        "retirement fixture must use a signed nonproducer cursor",
    );
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind nonproducer retirement peer");
    let generation_one = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim first nonproducer retirement generation");
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist nonproducer retirement payload");
    let initial_live = sign_lifecycle_cursor(
        &local_signer,
        &local_peer,
        &payload.origin_proposal.descriptor.validator_set,
        1,
        None,
        binding.clone(),
        AutonomousLifecycleCursorPhaseV1::live(generation_one.generation(), live_state)
            .expect("construct nonproducer retirement Live cursor"),
    )
    .expect("sign nonproducer retirement Live cursor");
    let (_, initial_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_one)
        .expect("read absent nonproducer retirement cursor")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(initial_lease, initial_live.clone())
            .expect("publish nonproducer retirement Live cursor")
            .cursor(),
        Some(&initial_live),
    );
    let retirement = crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload);
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
        .expect("persist full ReleasePending retirement prefix");
    if matches!(
        claim_prefix,
        NonproducerRetirementClaimPrefix::ReplicaReleased
    ) {
        let queue_disposition = match queue_cut {
            NonproducerReplicaQueueCut::ExactOrdinaryFifo => {
                crate::kura::AutonomousLifecycleReplicaQueueDispositionV1::ExactOrdinaryFifo
            }
            NonproducerReplicaQueueCut::StrictQueueAbsent => {
                crate::kura::AutonomousLifecycleReplicaQueueDispositionV1::StrictQueueAbsent
            }
        };
        kura.finalize_autonomous_lane_slot_replica_release_for_test(
            &retirement,
            network_id,
            epoch,
            queue_disposition,
        )
        .expect("persist full disposition-bound ReplicaReleased retirement prefix");
    }
    drop(queue);
    let queue = open_lifecycle_recovery_queue(
        &queue_dir,
        &state,
        matches!(queue_cut, NonproducerReplicaQueueCut::StrictQueueAbsent),
    );
    let descriptor = &payload.origin_proposal.descriptor;
    let terminal_path = kura
        .autonomous_lifecycle_terminal_outcome_path_for_test(
            descriptor.lane_id,
            descriptor.lane_block_height,
            descriptor.proposal_height,
        )
        .expect("resolve nonproducer replica terminal path");
    assert!(
        !terminal_path.exists(),
        "{queue_cut:?}/{claim_prefix:?}: crash cut must precede every terminal outcome",
    );
    assert!(
        kura.pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("inventory pre-startup nonproducer outcomes")
            .is_empty(),
    );
    let queue_snapshot_before = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture nonproducer Queue cut");
    assert!(queue_snapshot_before.is_empty());
    let fifo_before = queue.fifo_snapshot_for_test();
    match queue_cut {
        NonproducerReplicaQueueCut::ExactOrdinaryFifo => {
            assert_eq!(
                fifo_before,
                payload
                    .entrypoints
                    .iter()
                    .map(TransactionEntrypoint::hash)
                    .collect::<Vec<_>>()
            )
        }
        NonproducerReplicaQueueCut::StrictQueueAbsent => assert!(fifo_before.is_empty()),
    }
    let live_before = queue.live_lane_reservations();
    let commit_barriers_before = queue.lane_reservation_commit_barriers();
    let release_barriers_before = queue.lane_reservation_release_barriers();
    let quarantine_before = queue.lane_reservation_startup_reconciliation_pending();
    assert!(live_before.is_empty());
    assert!(commit_barriers_before.is_empty());
    assert!(release_barriers_before.is_empty());
    assert!(!quarantine_before);
    drop(generation_one);
    drop(state);
    drop(kura);

    let (restarted, restarted_state) =
        open_lifecycle_recovery_state(&kura_config, &lane_config, &context, &nexus);
    restarted
        .bind_local_peer_id(local_peer.clone())
        .expect("rebind nonproducer retirement peer");
    let generation_two = restarted
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim restarted nonproducer retirement generation");
    assert_eq!(generation_two.generation(), 2);
    let recovered = reconcile_autonomous_lifecycle_startup(
        &restarted_state,
        &queue,
        restarted.as_ref(),
        &context,
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
            queue_snapshot_before.clone(),
            Vec::new(),
        ),
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        Some(&generation_two),
        &local_peer,
        &local_signer,
    )
    .unwrap_or_else(|error| {
        panic!("{queue_cut:?}/{claim_prefix:?}: reconcile nonproducer retirement: {error}")
    });
    assert_eq!(recovered.completed_bootstraps(), 0);
    assert_eq!(recovered.recovered_attempts(), 1);
    let (returned_snapshot, receipt, pending_groups) = recovered.into_queue_handoff();
    assert_eq!(returned_snapshot, queue_snapshot_before);
    assert!(pending_groups.is_empty());
    assert!(
        queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(
                &receipt,
                &queue_snapshot_before,
            )
            .expect("revalidate nonproducer retirement Queue receipt"),
    );
    assert_eq!(queue.fifo_snapshot_for_test(), fifo_before);
    assert_eq!(queue.live_lane_reservations(), live_before);
    assert_eq!(
        queue.lane_reservation_commit_barriers(),
        commit_barriers_before,
    );
    assert_eq!(
        queue.lane_reservation_release_barriers(),
        release_barriers_before,
    );
    assert_eq!(
        queue.lane_reservation_startup_reconciliation_pending(),
        quarantine_before,
    );
    assert!(terminal_path.is_file());
    let terminal_bytes = std::fs::read(&terminal_path).expect("read Complete replica outcome");
    let terminal: crate::kura::AutonomousLifecycleTerminalOutcomeV1 =
        norito::decode_canonical(&terminal_bytes).expect("decode Complete replica outcome");
    let terminal_debug = format!("{terminal:?}");
    assert!(terminal_debug.contains("RetiredReplicaQueueDisposition"));
    assert!(terminal_debug.contains("Complete"));
    let expected_disposition = match queue_cut {
        NonproducerReplicaQueueCut::ExactOrdinaryFifo => "ExactOrdinaryFifo",
        NonproducerReplicaQueueCut::StrictQueueAbsent => "StrictQueueAbsent",
    };
    assert!(terminal_debug.contains(expected_disposition));
    assert!(
        restarted
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("inventory completed nonproducer outcome")
            .is_empty(),
        "{queue_cut:?}/{claim_prefix:?}: Complete replica outcome must leave no Pending inventory",
    );
    let recovered_cursor = restarted
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_two)
        .expect("read recovered nonproducer cursor")
        .cursor()
        .expect("recovered nonproducer cursor remains signed")
        .clone();
    assert_eq!(recovered_cursor.owner_generation(), 2);
    drop(receipt);

    let repeated_snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("recapture nonproducer Queue cut");
    let repeated = reconcile_autonomous_lifecycle_startup(
        &restarted_state,
        &queue,
        restarted.as_ref(),
        &context,
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(
            repeated_snapshot.clone(),
            Vec::new(),
        ),
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        Some(&generation_two),
        &local_peer,
        &local_signer,
    )
    .unwrap_or_else(|error| {
        panic!("{queue_cut:?}/{claim_prefix:?}: repeat nonproducer retirement: {error}")
    });
    assert_eq!(repeated.completed_bootstraps(), 0);
    assert_eq!(repeated.recovered_attempts(), 0);
    let (repeated_handoff, repeated_receipt, pending_groups) = repeated.into_queue_handoff();
    assert_eq!(repeated_handoff, repeated_snapshot);
    assert!(pending_groups.is_empty());
    assert!(
        queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(
                &repeated_receipt,
                &repeated_snapshot,
            )
            .expect("revalidate repeated nonproducer Queue receipt"),
    );
    assert_eq!(queue.fifo_snapshot_for_test(), fifo_before);
    assert_eq!(queue.live_lane_reservations(), live_before);
    assert_eq!(
        queue.lane_reservation_commit_barriers(),
        commit_barriers_before,
    );
    assert_eq!(
        queue.lane_reservation_release_barriers(),
        release_barriers_before,
    );
    assert_eq!(
        queue.lane_reservation_startup_reconciliation_pending(),
        quarantine_before,
    );
    assert_eq!(
        std::fs::read(&terminal_path).expect("reread Complete replica outcome"),
        terminal_bytes,
        "retry must preserve the byte-exact Complete replica outcome",
    );
    let repeated_cursor = restarted
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_two)
        .expect("read repeated nonproducer cursor");
    assert_eq!(
        repeated_cursor
            .cursor()
            .expect("repeated nonproducer cursor remains signed"),
        &recovered_cursor,
    );
    if exercise_complete_claim_presweep {
        assert!(
            payload.entrypoint_hashes.len() > 1,
            "replica Complete crash cut requires a partial claim suffix",
        );
        restarted
            .downgrade_autonomous_lane_replica_complete_claim_suffix_for_test(&payload, 1)
            .expect("recreate Complete-outcome/partial-raw-claim crash cut");
        assert_eq!(
            restarted
                .autonomous_lane_replica_claim_seal_counts_for_test(&payload)
                .expect("count partial replica claim seal"),
            (1, payload.entrypoint_hashes.len() - 1),
        );
        assert_eq!(
            std::fs::read(&terminal_path).expect("preserve Complete replica outcome at crash cut"),
            terminal_bytes,
        );
        drop(repeated_receipt);
        drop(generation_two);
        drop(restarted_state);
        drop(restarted);

        let (sealed_restart, sealed_state) =
            open_lifecycle_recovery_state(&kura_config, &lane_config, &context, &nexus);
        assert_eq!(
            sealed_restart
                .autonomous_lane_replica_claim_seal_counts_for_test(&payload)
                .expect("strict startup seals partial replica claim suffix"),
            (0, payload.entrypoint_hashes.len()),
        );
        assert_eq!(
            std::fs::read(&terminal_path).expect("read presweep Complete replica outcome"),
            terminal_bytes,
            "claim pre-sweep must preserve the byte-exact Complete outcome",
        );
        drop(sealed_state);
        drop(sealed_restart);

        let (idempotent_restart, idempotent_state) =
            open_lifecycle_recovery_state(&kura_config, &lane_config, &context, &nexus);
        assert_eq!(
            idempotent_restart
                .autonomous_lane_replica_claim_seal_counts_for_test(&payload)
                .expect("repeat strict startup stutters on sealed replica claims"),
            (0, payload.entrypoint_hashes.len()),
        );
        assert_eq!(
            std::fs::read(&terminal_path).expect("reread idempotent Complete replica outcome"),
            terminal_bytes,
        );
        drop(idempotent_state);
        drop(idempotent_restart);
    }
}

#[test]
fn nonproducer_release_pending_attempt_startup_completes_replica_for_fifo_and_absent_queue() {
    for queue_cut in [
        NonproducerReplicaQueueCut::ExactOrdinaryFifo,
        NonproducerReplicaQueueCut::StrictQueueAbsent,
    ] {
        exercise_nonproducer_retired_attempt_startup(
            queue_cut,
            NonproducerRetirementClaimPrefix::ReleasePending,
            false,
        );
    }
}

#[test]
fn nonproducer_released_attempt_startup_completes_replica_for_fifo_and_absent_queue() {
    for queue_cut in [
        NonproducerReplicaQueueCut::ExactOrdinaryFifo,
        NonproducerReplicaQueueCut::StrictQueueAbsent,
    ] {
        exercise_nonproducer_retired_attempt_startup(
            queue_cut,
            NonproducerRetirementClaimPrefix::ReplicaReleased,
            false,
        );
    }
}

#[test]
fn strict_kura_startup_seals_complete_replica_outcome_claim_suffix_idempotently() {
    for queue_cut in [
        NonproducerReplicaQueueCut::ExactOrdinaryFifo,
        NonproducerReplicaQueueCut::StrictQueueAbsent,
    ] {
        exercise_nonproducer_retired_attempt_startup(
            queue_cut,
            NonproducerRetirementClaimPrefix::ReplicaReleased,
            true,
        );
    }
}

#[test]
fn prepared_bootstrap_and_crash_boundaries_resolve_only_their_durable_side() {
    let signer = lifecycle_key_pair(41);
    let local_peer = PeerId::new(signer.public_key().clone());
    let (_, _, payload) = lifecycle_payload(
        &signer,
        Hash::new(b"lifecycle-prepared-boundary-incarnation"),
    );
    let (binding, live_state) = lifecycle_binding_and_live_state(&payload, &local_peer);
    let validator_set = &payload.origin_proposal.descriptor.validator_set;
    let mut before_activate = live_state;
    before_activate.carrier.kura_active = 0;
    let activate = crate::sumeragi::v2_core::ProductionInFlightFirstReleaseTransitionProjection {
        action: IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA,
        actor: 1,
        target: 0,
        before: before_activate,
        after: live_state,
    };
    let activate_cursor = sign_lifecycle_cursor(
        &signer,
        &local_peer,
        validator_set,
        1,
        None,
        binding.clone(),
        AutonomousLifecycleCursorPhaseV1::prepared(1, activate)
            .expect("construct Prepared ActivateKura"),
    )
    .expect("sign Prepared ActivateKura");
    assert_eq!(
        prepared_recovery_state(&activate_cursor).expect("resolve Prepared ActivateKura"),
        live_state,
        "payload inventory proves ActivateKura reached its durable after-state",
    );
    let crash = check_production_in_flight_first_release_crash_transition(live_state, 1)
        .expect("derive Crash")
        .into_projection();
    let prepared_crash = sign_lifecycle_cursor(
        &signer,
        &local_peer,
        validator_set,
        2,
        Some(activate_cursor.cursor_hash()),
        binding.clone(),
        AutonomousLifecycleCursorPhaseV1::prepared(1, crash).expect("construct Prepared Crash"),
    )
    .expect("sign Prepared Crash");
    assert_eq!(
        prepared_recovery_state(&prepared_crash).expect("resolve Prepared Crash"),
        crash.before,
    );
    let crashed = sign_lifecycle_cursor(
        &signer,
        &local_peer,
        validator_set,
        3,
        Some(prepared_crash.cursor_hash()),
        binding.clone(),
        AutonomousLifecycleCursorPhaseV1::crashed(1, 2, crash.before, crash.after)
            .expect("construct Crashed cursor"),
    )
    .expect("sign Crashed cursor");
    assert_eq!(
        cursor_recovery_state(&crashed).expect("resolve durable Crashed cursor"),
        crash.after,
    );
    let recover = check_production_in_flight_first_release_recover_transition(crash.after, 1)
        .expect("derive Recover")
        .into_projection();
    let prepared_recover = sign_lifecycle_cursor(
        &signer,
        &local_peer,
        validator_set,
        4,
        Some(crashed.cursor_hash()),
        binding.clone(),
        AutonomousLifecycleCursorPhaseV1::prepared(2, recover).expect("construct Prepared Recover"),
    )
    .expect("sign Prepared Recover");
    assert_eq!(
        prepared_recovery_state(&prepared_recover).expect("resolve Prepared Recover"),
        recover.before,
    );
    let rehydrate =
        check_production_in_flight_first_release_rehydrate_local_kura_custody_transition(
            recover.after,
            1,
        )
        .expect("derive rehydration")
        .into_projection();
    let prepared_rehydrate = sign_lifecycle_cursor(
        &signer,
        &local_peer,
        validator_set,
        5,
        Some(prepared_recover.cursor_hash()),
        binding,
        AutonomousLifecycleCursorPhaseV1::prepared(2, rehydrate)
            .expect("construct Prepared rehydration"),
    )
    .expect("sign Prepared rehydration");
    assert_eq!(
        prepared_recovery_state(&prepared_rehydrate).expect("resolve Prepared rehydration"),
        rehydrate.before,
    );
}
#[test]
fn empty_queue_reconciliation_returns_the_same_checked_receipt() {
    let temp_dir = TempDir::new().expect("Queue journal directory");
    let (_time_handle, time_source) = TimeSource::new_mock(core::time::Duration::ZERO);
    let queue = Queue::test(QueueConfig::default(), &time_source);
    queue
        .install_plan_journal(temp_dir.path().join("queue-plan.norito"), 1024 * 1024, true)
        .expect("install empty QueuePlan journal");
    queue
        .install_lane_reservation_journal(
            temp_dir.path().join("lane-reservation.norito"),
            1024 * 1024,
        )
        .expect("install empty reservation journal");
    let kura = Kura::blank_kura_for_testing();
    let state = State::new_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
    );
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("publish empty QueuePlan replay receipt"),
        Default::default(),
    );
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture empty Queue snapshot");
    assert!(snapshot.is_empty());
    let planner =
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(snapshot.clone(), Vec::new());
    let signer = lifecycle_key_pair(51);
    let local_peer = PeerId::new(signer.public_key().clone());
    let recovered = reconcile_autonomous_lifecycle_startup(
        &state,
        &queue,
        kura.as_ref(),
        &lifecycle_context(&signer),
        planner,
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        None,
        &local_peer,
        &signer,
    )
    .expect("reconcile an empty Queue snapshot");
    assert_eq!(recovered.completed_bootstraps(), 0);
    assert_eq!(recovered.recovered_attempts(), 0);
    let (returned_snapshot, receipt, pending_groups) = recovered.into_queue_handoff();
    assert_eq!(returned_snapshot, snapshot);
    assert!(pending_groups.is_empty());
    assert!(
        queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot,)
            .expect("revalidate returned Queue receipt"),
        "reconciliation must return the exact combined V1 receipt it authenticated",
    );
}
