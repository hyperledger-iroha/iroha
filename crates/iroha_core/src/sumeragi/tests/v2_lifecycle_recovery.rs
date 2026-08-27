use super::*;
use crate::{
    governance::manifest::{
        GovernanceRules, LaneManifestRegistry, LaneManifestStatus, ManifestValidatorBinding,
    },
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
    ChainId, IntoKeyValue, Level, Registrable,
    account::{AccountDetails, AccountId, AccountValue},
    asset::{Asset, AssetBalancePolicy, AssetDefinition, AssetDefinitionId, AssetId},
    block::{
        BlockHeader,
        consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1},
        consensus_v2 as wire,
    },
    consensus::{ConsensusKeyRecord, ConsensusKeyStatus, VALIDATOR_SET_HASH_VERSION_V1},
    isi::Log,
    nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
    peer::PeerId,
    transaction::{
        FeePaymentIntent, TransactionBuilder,
        signed::{FeeChargeKind, FeeChargeLimit, TransactionEntrypoint},
    },
};
use iroha_primitives::{numeric::Quantity, time::TimeSource};
use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
};
use tempfile::TempDir;
fn lifecycle_key_pair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
        .expect("deterministic BLS lifecycle key")
}
fn lifecycle_fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::parse_address_literal(
        &iroha_config::parameters::defaults::nexus::fees::fee_asset_id(),
    )
    .expect("default Nexus fee asset is a canonical asset-definition address")
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
    validator_set: Vec<PeerId>,
    lane_incarnation: Hash,
    transaction_count: usize,
) -> (
    iroha_data_model::NetworkId,
    u64,
    crate::lane_consensus::LaneExecutablePayloadV1,
) {
    lifecycle_payload_for_validators_with_count_and_lane(
        producer_signer,
        context,
        validator_set,
        LaneId::new(1),
        lane_incarnation,
        transaction_count,
        None,
    )
}
fn lifecycle_payload_for_validators_with_count_and_lane(
    producer_signer: &KeyPair,
    context: &wire::HeightContext,
    mut validator_set: Vec<PeerId>,
    lane_id: LaneId,
    lane_incarnation: Hash,
    transaction_count: usize,
    creation_time: Option<core::time::Duration>,
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
                FeePaymentIntent::authority(
                    vec![FeeChargeLimit::new(
                        FeeChargeKind::Nexus,
                        lifecycle_fee_asset_definition_id(),
                        Quantity::from(1_u32),
                    )],
                    None,
                ),
            )
            .with_instructions([Log::new(
                Level::INFO,
                format!("lifecycle recovery payload {index}"),
            )])
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            );
            if let Some(creation_time) = creation_time {
                builder.set_creation_time(creation_time);
            }
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
        lane_id,
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
    state
        .prepare_configured_primary_geometry_anchor(&nexus.lane_catalog)
        .expect("authenticate lifecycle configured-primary geometry");
    state
        .restore_kura_lane_segments_before_startup_replay()
        .expect("restore lifecycle pre-replay geometry cursor");
    let mut startup_nexus = nexus.clone();
    startup_nexus.configured_lane_catalog = startup_nexus.lane_catalog.clone();
    startup_nexus.lane_config = RuntimeLaneConfig::from_catalog(&startup_nexus.lane_catalog);
    state
        .set_nexus_from_config(startup_nexus)
        .expect("install lifecycle Nexus through the authenticated startup boundary");
    (kura, state)
}
fn install_lifecycle_queue_plan_validator_authority(
    state: &State,
    queue: &Queue,
    context: &wire::HeightContext,
    validator_keys: &[KeyPair],
) {
    assert_eq!(
        context.roster.len(),
        validator_keys.len(),
        "lifecycle QueuePlan authority keys must cover the exact roster",
    );
    let mut world_block = state.world.block();
    world_block.accounts.insert(
        (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
        AccountValue::new(AccountDetails::default()),
    );
    let fee_definition_id = lifecycle_fee_asset_definition_id();
    world_block.asset_definitions.insert(
        fee_definition_id.clone(),
        AssetDefinition::numeric(
            fee_definition_id.clone(),
            "lifecycle recovery Nexus fee",
            AssetBalancePolicy::Global,
            None,
        )
        .build(&SAMPLE_GENESIS_ACCOUNT_ID),
    );
    let (fee_asset_id, fee_asset_value) = Asset::new(
        AssetId::new(fee_definition_id, (*SAMPLE_GENESIS_ACCOUNT_ID).clone()),
        Quantity::from(100_u32),
    )
    .into_key_value();
    world_block.assets.insert(fee_asset_id, fee_asset_value);
    {
        let mut peers = world_block.peers_mut_for_testing().transaction();
        for validator in &context.roster {
            if !peers.iter().any(|peer| peer == &validator.validator) {
                peers.push(validator.validator.clone());
            }
        }
        peers.apply();
    }
    for validator in &context.roster {
        let signer = validator_keys
            .iter()
            .find(|signer| signer.public_key() == validator.validator.public_key())
            .expect("lifecycle QueuePlan authority key must match every roster member");
        let public_key = signer.public_key().clone();
        let id = crate::state::derive_validator_key_id(&public_key);
        let record = ConsensusKeyRecord {
            id: id.clone(),
            public_key,
            pop: Some(
                iroha_crypto::bls_normal_pop_prove(signer.private_key())
                    .expect("lifecycle QueuePlan validator PoP"),
            ),
            activation_height: 0,
            expiry_height: None,
            hsm: None,
            replaces: None,
            status: ConsensusKeyStatus::Active,
        };
        world_block
            .consensus_keys
            .insert(id.clone(), record.clone());
        world_block
            .consensus_keys_by_pk
            .insert(record.public_key.to_string(), vec![id]);
    }
    world_block.commit();
    let validators = context
        .roster
        .iter()
        .map(|validator| AccountId::new(validator.validator.public_key().clone()))
        .collect::<Vec<_>>();
    let validator_bindings = validators
        .iter()
        .zip(&context.roster)
        .map(|(validator, power)| ManifestValidatorBinding {
            validator: validator.clone(),
            peer_id: power.validator.clone(),
            torii_url: None,
        })
        .collect();
    let primary_lane = state
        .nexus_snapshot()
        .lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == LaneId::SINGLE)
        .cloned()
        .expect("lifecycle QueuePlan fixture has the primary lane");
    let status = LaneManifestStatus {
        lane: primary_lane.id,
        alias: primary_lane.alias,
        dataspace: primary_lane.dataspace_id,
        visibility: primary_lane.visibility,
        storage: primary_lane.storage,
        governance: primary_lane.governance,
        manifest_path: Some(std::path::PathBuf::from(
            "/tmp/sumeragi-v2-lifecycle-recovery-manifest.json",
        )),
        governance_rules: Some(GovernanceRules {
            validators,
            validator_bindings,
            ..GovernanceRules::default()
        }),
        privacy_commitments: Vec::new(),
    };
    let mut statuses = {
        let manifests = state.lane_manifests.read();
        manifests
            .statuses()
            .into_iter()
            .map(|status| (status.lane, status))
            .collect::<BTreeMap<_, _>>()
    };
    statuses.insert(LaneId::SINGLE, status);
    queue.install_lane_manifests_with_state(
        &Arc::new(LaneManifestRegistry::from_statuses(statuses)),
        state,
    );
}
fn open_empty_lifecycle_recovery_queue(queue_dir: &TempDir, state: &State) -> Queue {
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
            .replay_plan_journal(state)
            .expect("publish lifecycle QueuePlan replay receipt"),
        Default::default(),
    );
    queue
}
fn reserve_lifecycle_replica_retirement_payload(
    queue: &Queue,
    state: &State,
    producer_signer: &KeyPair,
    context: &wire::HeightContext,
    validator_set: Vec<PeerId>,
    lane_incarnation: Hash,
) -> (
    crate::lane_consensus::LaneExecutablePayloadV1,
    Vec<crate::torii_proxy::QueuePlanAdmissionBindingV1>,
) {
    let (_, _, template) = lifecycle_payload_for_validators_with_count_and_lane(
        producer_signer,
        context,
        validator_set,
        LaneId::SINGLE,
        lane_incarnation,
        2,
        Some(core::time::Duration::ZERO),
    );
    let descriptor = &template.origin_proposal.descriptor;
    let expected_route = RoutingDecision::new(descriptor.lane_id, descriptor.dataspace_id);
    let mut admission_bindings = Vec::with_capacity(template.entrypoints.len());
    for entrypoint in &template.entrypoints {
        let accepted =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(entrypoint.clone()));
        let routing_plan = queue
            .route_plan_with_state(&accepted, state)
            .expect("resolve lifecycle replica retirement Queue route");
        assert_eq!(
            routing_plan.coordinator_route(),
            expected_route,
            "replica retirement fixture must reserve the router-selected primary lane",
        );
        let admission_context = queue
            .plan_admission_context_with_state(state, &routing_plan)
            .expect("capture lifecycle replica retirement admission context");
        let binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
            state.network_id_ref(),
            accepted.entrypoint(),
            &routing_plan,
            admission_context,
            queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("build lifecycle replica retirement admission binding");
        queue
            .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                accepted,
                state,
                routing_plan,
                &binding,
            )
            .expect("durably enqueue lifecycle replica retirement transaction");
        state
            .install_queue_plan_pending_binding_for_test(&binding)
            .expect("install lifecycle replica retirement QueuePlan owner");
        admission_bindings.push(binding);
    }
    let template_key = template
        .reservation_keys
        .first()
        .expect("replica retirement template has a reservation key");
    let reserved = queue
        .reserve_transactions_for_lane(
            state,
            LaneQueueReservationScopeV1 {
                lane_id: descriptor.lane_id,
                dataspace_id: descriptor.dataspace_id,
                lane_incarnation: descriptor.lane_incarnation,
                proposal_height: descriptor.proposal_height,
                lane_block_height: descriptor.lane_block_height,
                lane_block_view: descriptor.lane_block_view,
                reservation_owner_hash: template_key.reservation_owner_hash,
                proposal_identity_hash: template_key.proposal_identity_hash,
            },
            NonZeroUsize::new(template.entrypoints.len())
                .expect("replica retirement payload is non-empty"),
        )
        .expect("reserve lifecycle replica retirement transaction");
    assert_eq!(reserved.len(), template.entrypoints.len());
    let reservation_keys = reserved
        .iter()
        .map(|transaction| *transaction.key())
        .collect::<Vec<_>>();
    let routing_plans = reserved
        .iter()
        .map(|transaction| transaction.routing_plan().clone())
        .collect::<Vec<_>>();
    let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
        template.network_id,
        template.epoch,
        template.origin_proposal,
        template.entrypoints,
        reservation_keys,
        routing_plans,
        template.native_amx_receipts,
        template.producer,
        producer_signer.private_key(),
    )
    .expect("sign exact lifecycle replica retirement payload");
    (payload, admission_bindings)
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
fn durable_file_snapshot(
    root: &std::path::Path,
) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn collect(
        root: &std::path::Path,
        directory: &std::path::Path,
        files: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        let mut entries = std::fs::read_dir(directory)
            .expect("read durable fixture directory")
            .map(|entry| entry.expect("read durable fixture entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                collect(root, &path, files);
            } else if path.is_file() {
                files.insert(
                    path.strip_prefix(root)
                        .expect("durable fixture path stays below root")
                        .to_path_buf(),
                    std::fs::read(&path).expect("read durable fixture file"),
                );
            }
        }
    }
    let mut files = std::collections::BTreeMap::new();
    collect(root, root, &mut files);
    files
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RetiredReplicaClaimRestartBoundary {
    AllReleasePending,
    FirstReleased,
}
struct RetiredReplicaStartupFixture {
    _kura_dir: TempDir,
    queue_dir: TempDir,
    signer: KeyPair,
    local_peer: PeerId,
    context: wire::HeightContext,
    payload: crate::lane_consensus::LaneExecutablePayloadV1,
    binding: AutonomousLifecycleAttemptBindingV1,
    initial_live: AutonomousLifecycleCursorV1,
    retirement: crate::kura::AutonomousLaneSlotRetirementV1,
    kura: Arc<Kura>,
    state: State,
    queue: Arc<Queue>,
    generation: AutonomousLifecycleProcessGenerationClaim,
}
fn retired_replica_startup_fixture(
    remove_fifo_owner: bool,
    claim_boundary: RetiredReplicaClaimRestartBoundary,
) -> RetiredReplicaStartupFixture {
    assert!(
        !remove_fifo_owner
            || matches!(
                claim_boundary,
                RetiredReplicaClaimRestartBoundary::AllReleasePending
            ),
        "a missing-FIFO fixture cannot inject an authorized Released claim prefix",
    );
    let kura_dir = TempDir::new().expect("retired replica Kura directory");
    let queue_dir = TempDir::new().expect("retired replica Queue journal directory");
    let kura_config = lifecycle_kura_config(&kura_dir);
    let lane_config = lifecycle_runtime_lane_config();
    let signer = lifecycle_key_pair(31);
    let local_peer = PeerId::new(signer.public_key().clone());
    let producer_signer = lifecycle_key_pair(91);
    let context = lifecycle_context(&signer);
    context
        .validate()
        .expect("retired replica startup context must be structurally valid");
    let nexus = Nexus {
        lane_catalog: lifecycle_lane_catalog(),
        dataspace_catalog: iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata {
                id: DataSpaceId::UNIVERSAL,
                alias: "universal".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("retired replica four-validator dataspace catalog"),
        ..Nexus::default()
    };
    let (kura, state) = open_lifecycle_recovery_state(&kura_config, &lane_config, &context, &nexus);
    let validator_keys = [
        lifecycle_key_pair(31),
        lifecycle_key_pair(91),
        lifecycle_key_pair(92),
        lifecycle_key_pair(93),
    ];
    let lane_incarnation = state
        .lane_incarnations_snapshot()
        .get(&LaneId::SINGLE)
        .copied()
        .expect("State primary-lane incarnation");
    let (_time_handle, time_source) = TimeSource::new_mock(core::time::Duration::ZERO);
    let queue = Arc::new(Queue::test(QueueConfig::default(), &time_source));
    queue.reconfigure_nexus_with_state(&state.nexus_snapshot(), &state, None);
    install_lifecycle_queue_plan_validator_authority(
        &state,
        queue.as_ref(),
        &context,
        &validator_keys,
    );
    queue
        .install_plan_journal(
            queue_dir.path().join("queue-plan.norito"),
            1024 * 1024,
            true,
        )
        .expect("install retired replica QueuePlan journal");
    queue
        .install_lane_reservation_journal(
            queue_dir.path().join("lane-reservation.norito"),
            1024 * 1024,
        )
        .expect("install retired replica reservation journal");
    let validator_set = context
        .roster
        .iter()
        .map(|validator| validator.validator.clone())
        .collect();
    let (payload, admission_bindings) = reserve_lifecycle_replica_retirement_payload(
        &queue,
        &state,
        &producer_signer,
        &context,
        validator_set,
        lane_incarnation,
    );
    let network_id = payload.network_id;
    let epoch = payload.epoch;
    let (binding, live_state) = lifecycle_binding_and_live_state(&payload, &local_peer);
    let local_actor = binding.local_validator_identity().1;
    assert_ne!(
        local_actor,
        binding.producer_actor_projection(),
        "retired replica startup must exercise actor-different custody",
    );
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind initial retired replica peer");
    let generation_one = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim first retired replica process generation");
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist retired replica payload");
    let initial_live = sign_lifecycle_cursor(
        &signer,
        &local_peer,
        &payload.origin_proposal.descriptor.validator_set,
        1,
        None,
        binding.clone(),
        AutonomousLifecycleCursorPhaseV1::live(generation_one.generation(), live_state)
            .expect("construct retired replica Live cursor"),
    )
    .expect("sign retired replica Live cursor");
    let (_, initial_lease) = kura
        .read_autonomous_lifecycle_cursor(&payload, &binding, &generation_one)
        .expect("read absent retired replica cursor")
        .into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(initial_lease, initial_live.clone())
            .expect("publish retired replica Live cursor")
            .cursor(),
        Some(&initial_live),
    );
    let retirement = crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload);
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
        .expect("persist retired replica ReleasePending boundary");
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&payload.reservation_keys)
            .expect("remove retired replica lane reservation owner"),
        payload.reservation_keys.len(),
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(queue.queued_len(), payload.entrypoints.len());
    assert_eq!(
        queue.fifo_snapshot_for_test(),
        payload
            .reservation_keys
            .iter()
            .map(|key| key.entrypoint_hash)
            .collect::<Vec<_>>(),
        "retired replica crash cut requires exact FIFO ownership in barrier order",
    );
    if matches!(
        claim_boundary,
        RetiredReplicaClaimRestartBoundary::FirstReleased
    ) {
        kura.inject_autonomous_lane_first_released_claim_crash_cut_for_test(&payload, &retirement)
            .expect("persist exactly one Released claim before the replica restart");
    }
    if remove_fifo_owner {
        assert_eq!(
            queue.remove_committed_hashes(
                payload
                    .reservation_keys
                    .iter()
                    .map(|key| key.entrypoint_hash),
                None,
            ),
            payload.entrypoints.len(),
            "negative retired replica fixture must durably remove exact ordinary FIFO ownership",
        );
        assert_eq!(queue.queued_len(), 0);
    }
    let descriptor = &payload.origin_proposal.descriptor;
    let retired = kura
        .read_autonomous_lane_retired_attempt(
            descriptor.lane_id,
            descriptor.lane_block_height,
            descriptor.proposal_height,
            network_id,
            epoch,
        )
        .expect("read durable retired replica attempt")
        .expect("retired replica attempt exists");
    assert_eq!(retired.artifact.executable_payload, payload);
    assert_eq!(retired.retirement, retirement);
    drop(generation_one);
    drop(state);
    drop(kura);
    drop(queue);
    let (restarted, restarted_state) =
        open_lifecycle_recovery_state(&kura_config, &lane_config, &context, &nexus);
    restarted
        .bind_local_peer_id(local_peer.clone())
        .expect("bind restarted retired replica peer");
    let generation = restarted
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim restarted retired replica process generation");
    assert_eq!(generation.generation(), 2);
    let (_time_handle, time_source) = TimeSource::new_mock(core::time::Duration::ZERO);
    let restarted_queue = Arc::new(Queue::test(QueueConfig::default(), &time_source));
    restarted_queue.reconfigure_nexus_with_state(
        &restarted_state.nexus_snapshot(),
        &restarted_state,
        None,
    );
    install_lifecycle_queue_plan_validator_authority(
        &restarted_state,
        restarted_queue.as_ref(),
        &context,
        &validator_keys,
    );
    for admission_binding in &admission_bindings {
        restarted_state
            .install_queue_plan_pending_binding_for_test(admission_binding)
            .expect("restore retired replica QueuePlan registry owner");
    }
    restarted_queue
        .install_plan_journal(
            queue_dir.path().join("queue-plan.norito"),
            1024 * 1024,
            true,
        )
        .expect("reopen retired replica QueuePlan journal");
    restarted_queue
        .install_lane_reservation_journal(
            queue_dir.path().join("lane-reservation.norito"),
            1024 * 1024,
        )
        .expect("reopen retired replica reservation journal");
    restarted_queue
        .replay_plan_journal(&restarted_state)
        .expect("replay retired replica QueuePlan journal");
    assert!(restarted_queue.live_lane_reservations().is_empty());
    assert_eq!(
        restarted_queue.queued_len(),
        if remove_fifo_owner {
            0
        } else {
            payload.entrypoints.len()
        },
    );
    RetiredReplicaStartupFixture {
        _kura_dir: kura_dir,
        queue_dir,
        signer,
        local_peer,
        context,
        payload,
        binding,
        initial_live,
        retirement,
        kura: restarted,
        state: restarted_state,
        queue: restarted_queue,
        generation,
    }
}
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
#[test]
#[allow(clippy::too_many_lines)]
fn retired_nonqueue_replica_release_pending_resumes_on_startup_without_queue_owner() {
    let fixture = retired_replica_startup_fixture(
        false,
        RetiredReplicaClaimRestartBoundary::AllReleasePending,
    );
    let reservation_group = lane_queue_reservation_group_binding_from_ordered_keys(
        fixture.payload.reservation_keys.iter(),
    )
    .expect("bind all-ReleasePending replica reservation group");
    let pending = fixture
        .kura
        .authenticate_autonomous_lane_retirement_snapshot_evidence(
            &fixture.payload,
            &fixture.retirement,
            reservation_group,
            crate::kura::AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared,
        )
        .expect("authenticate restarted all-ReleasePending Kura prefix");
    let pending_state = pending.recovered_state();
    assert_eq!(
        (
            pending_state.release.pending_prefix,
            pending_state.release.released_prefix,
        ),
        (2, 0),
    );
    let snapshot = fixture
        .queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture all-ReleasePending replica Queue snapshot");
    assert!(snapshot.is_empty());
    let fifo_before = fixture.queue.fifo_snapshot_for_test();
    let queue_plan_before = std::fs::read(fixture.queue_dir.path().join("queue-plan.norito"))
        .expect("read all-ReleasePending QueuePlan journal before startup");
    let reservation_before =
        std::fs::read(fixture.queue_dir.path().join("lane-reservation.norito"))
            .expect("read all-ReleasePending reservation journal before startup");
    let recovered = reconcile_autonomous_lifecycle_startup(
        &fixture.state,
        &fixture.queue,
        fixture.kura.as_ref(),
        &fixture.context,
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(snapshot.clone(), Vec::new()),
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        Some(&fixture.generation),
        &fixture.local_peer,
        &fixture.signer,
    )
    .expect("resume exact all-ReleasePending retired replica release");
    assert_eq!(recovered.completed_bootstraps(), 0);
    assert_eq!(recovered.recovered_attempts(), 0);
    let (returned_snapshot, receipt, pending_groups) = recovered.into_queue_handoff();
    assert_eq!(returned_snapshot, snapshot);
    assert!(pending_groups.is_empty());
    assert!(
        fixture
            .queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)
            .expect("revalidate all-ReleasePending replica Queue receipt"),
    );
    assert_eq!(
        fixture
            .queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recapture all-ReleasePending replica Queue snapshot"),
        snapshot,
    );
    assert_eq!(fixture.queue.fifo_snapshot_for_test(), fifo_before);
    assert_eq!(
        std::fs::read(fixture.queue_dir.path().join("queue-plan.norito"))
            .expect("read all-ReleasePending QueuePlan journal after startup"),
        queue_plan_before,
    );
    assert_eq!(
        std::fs::read(fixture.queue_dir.path().join("lane-reservation.norito"))
            .expect("read all-ReleasePending reservation journal after startup"),
        reservation_before,
    );
    assert!(fixture.queue.live_lane_reservations().is_empty());
    assert_eq!(fixture.queue.queued_len(), 2);
    let completed = fixture
        .kura
        .authenticate_autonomous_lane_retirement_snapshot_evidence(
            &fixture.payload,
            &fixture.retirement,
            reservation_group,
            crate::kura::AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed,
        )
        .expect("authenticate completed all-Released Kura prefix");
    let completed_state = completed.recovered_state();
    assert_eq!(
        (
            completed_state.release.pending_prefix,
            completed_state.release.released_prefix,
        ),
        (2, 2),
    );
    assert!(
        fixture
            .kura
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("inspect all-ReleasePending replica terminal outcomes")
            .is_empty(),
    );
    let cursor = fixture
        .kura
        .read_autonomous_lifecycle_cursor(&fixture.payload, &fixture.binding, &fixture.generation)
        .expect("read all-ReleasePending replica lifecycle cursor after startup");
    assert_eq!(cursor.cursor(), Some(&fixture.initial_live));
}
#[test]
#[allow(clippy::too_many_lines)]
fn retired_nonqueue_replica_partial_released_prefix_resumes_on_startup_without_queue_owner() {
    let fixture =
        retired_replica_startup_fixture(false, RetiredReplicaClaimRestartBoundary::FirstReleased);
    assert_eq!(
        fixture.payload.entrypoint_hashes.len(),
        2,
        "partial claim-prefix recovery requires exactly two entrypoints",
    );
    let reservation_group = lane_queue_reservation_group_binding_from_ordered_keys(
        fixture.payload.reservation_keys.iter(),
    )
    .expect("bind partial Released-prefix reservation group");
    let partial = fixture
        .kura
        .authenticate_autonomous_lane_retirement_snapshot_evidence(
            &fixture.payload,
            &fixture.retirement,
            reservation_group,
            crate::kura::AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared,
        )
        .expect("authenticate restarted Released/ReleasePending Kura prefix");
    assert_eq!(
        partial.phase(),
        crate::kura::AutonomousLaneRetirementQueueSnapshotPhaseV1::Prepared,
    );
    assert_eq!(partial.reservation_group(), reservation_group);
    assert_eq!(
        partial.retirement_hash(),
        fixture
            .retirement
            .digest()
            .expect("hash partial-prefix retirement"),
    );
    let partial_state = partial.recovered_state();
    assert!(partial_state.release.kura_retired);
    assert_eq!(partial_state.release.pending_prefix, 2);
    assert_eq!(partial_state.release.released_prefix, 1);
    let snapshot = fixture
        .queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture retired replica startup Queue snapshot");
    assert!(snapshot.is_empty());
    let fifo_before = fixture.queue.fifo_snapshot_for_test();
    assert_eq!(
        fifo_before,
        fixture
            .payload
            .reservation_keys
            .iter()
            .map(|key| key.entrypoint_hash)
            .collect::<Vec<_>>(),
        "partial claim-prefix restart must retain exact FIFO barrier order",
    );
    let queue_plan_before = std::fs::read(fixture.queue_dir.path().join("queue-plan.norito"))
        .expect("read retired replica QueuePlan journal before startup");
    let reservation_before =
        std::fs::read(fixture.queue_dir.path().join("lane-reservation.norito"))
            .expect("read retired replica reservation journal before startup");
    let recovered = reconcile_autonomous_lifecycle_startup(
        &fixture.state,
        &fixture.queue,
        fixture.kura.as_ref(),
        &fixture.context,
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(snapshot.clone(), Vec::new()),
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        Some(&fixture.generation),
        &fixture.local_peer,
        &fixture.signer,
    )
    .expect("resume exact retired non-Queue replica release");
    assert_eq!(recovered.completed_bootstraps(), 0);
    assert_eq!(
        recovered.recovered_attempts(),
        0,
        "a retired replica must not fabricate Crash/Recover/rehydration successors",
    );
    let (returned_snapshot, receipt, pending_groups) = recovered.into_queue_handoff();
    assert_eq!(returned_snapshot, snapshot);
    assert!(pending_groups.is_empty());
    assert!(
        fixture
            .queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)
            .expect("revalidate retired replica Queue receipt"),
    );
    assert_eq!(
        fixture
            .queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recapture retired replica Queue snapshot"),
        snapshot,
        "FIFO-only replica release must not create a Queue reservation owner",
    );
    assert_eq!(
        std::fs::read(fixture.queue_dir.path().join("queue-plan.norito"))
            .expect("read retired replica QueuePlan journal after startup"),
        queue_plan_before,
        "FIFO authentication and Kura claim release must not mutate QueuePlan durability",
    );
    assert_eq!(
        std::fs::read(fixture.queue_dir.path().join("lane-reservation.norito"))
            .expect("read retired replica reservation journal after startup"),
        reservation_before,
        "FIFO authentication and Kura claim release must not mutate reservation durability",
    );
    assert!(fixture.queue.live_lane_reservations().is_empty());
    assert_eq!(
        fixture.queue.queued_len(),
        fixture.payload.entrypoints.len()
    );
    assert_eq!(
        fixture.queue.fifo_snapshot_for_test(),
        fifo_before,
        "partial claim-prefix completion must leave exact FIFO order unchanged",
    );
    assert!(
        fixture
            .kura
            .pending_autonomous_lifecycle_terminal_outcome_inventory()
            .expect("inspect completed retired replica terminal outcomes")
            .is_empty(),
        "startup must leave no Pending release outcome",
    );
    let descriptor = &fixture.payload.origin_proposal.descriptor;
    let retired = fixture
        .kura
        .read_autonomous_lane_retired_attempt(
            descriptor.lane_id,
            descriptor.lane_block_height,
            descriptor.proposal_height,
            fixture.payload.network_id,
            fixture.payload.epoch,
        )
        .expect("revalidate completed retired replica attempt")
        .expect("completed retired replica attempt remains durable");
    assert_eq!(retired.artifact.executable_payload, fixture.payload);
    assert_eq!(retired.retirement, fixture.retirement);
    let completed = fixture
        .kura
        .authenticate_autonomous_lane_retirement_snapshot_evidence(
            &fixture.payload,
            &fixture.retirement,
            reservation_group,
            crate::kura::AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed,
        )
        .expect("authenticate completed two-claim Released prefix");
    assert_eq!(
        completed.phase(),
        crate::kura::AutonomousLaneRetirementQueueSnapshotPhaseV1::Completed,
    );
    assert_eq!(completed.reservation_group(), reservation_group);
    assert_eq!(
        completed.retirement_hash(),
        fixture
            .retirement
            .digest()
            .expect("hash completed partial-prefix retirement"),
    );
    let completed_state = completed.recovered_state();
    assert!(completed_state.release.kura_retired);
    assert_eq!(completed_state.release.pending_prefix, 2);
    assert_eq!(completed_state.release.released_prefix, 2);
    let cursor = fixture
        .kura
        .read_autonomous_lifecycle_cursor(&fixture.payload, &fixture.binding, &fixture.generation)
        .expect("read retired replica lifecycle cursor after startup");
    assert_eq!(
        cursor.cursor(),
        Some(&fixture.initial_live),
        "retired replica completion must preserve the old signed terminal-attempt cursor byte-for-byte",
    );
}
#[test]
fn retired_nonqueue_replica_startup_rejects_missing_fifo_before_claim_release() {
    let fixture = retired_replica_startup_fixture(
        true,
        RetiredReplicaClaimRestartBoundary::AllReleasePending,
    );
    let snapshot = fixture
        .queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture missing-FIFO replica Queue snapshot");
    assert!(snapshot.is_empty());
    let queue_plan_before = std::fs::read(fixture.queue_dir.path().join("queue-plan.norito"))
        .expect("read missing-FIFO QueuePlan journal before startup");
    let reservation_before =
        std::fs::read(fixture.queue_dir.path().join("lane-reservation.norito"))
            .expect("read missing-FIFO reservation journal before startup");
    let kura_before = durable_file_snapshot(fixture._kura_dir.path());
    let error = match reconcile_autonomous_lifecycle_startup(
        &fixture.state,
        &fixture.queue,
        fixture.kura.as_ref(),
        &fixture.context,
        LaneReservationSnapshotPlannerEvidence::from_parts_for_test(snapshot.clone(), Vec::new()),
        AutonomousLifecycleDeferredTerminalRecoveryHandoff::empty(),
        Some(&fixture.generation),
        &fixture.local_peer,
        &fixture.signer,
    ) {
        Ok(_) => panic!("missing ordinary FIFO ownership must block retired replica completion"),
        Err(error) => error,
    };
    assert!(error.contains("retired replica release completion failed"));
    assert_eq!(
        fixture
            .queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recapture rejected replica Queue snapshot"),
        snapshot,
    );
    assert_eq!(
        std::fs::read(fixture.queue_dir.path().join("queue-plan.norito"))
            .expect("read rejected replica QueuePlan journal"),
        queue_plan_before,
    );
    assert_eq!(
        std::fs::read(fixture.queue_dir.path().join("lane-reservation.norito"))
            .expect("read rejected replica reservation journal"),
        reservation_before,
    );
    assert_eq!(
        durable_file_snapshot(fixture._kura_dir.path()),
        kura_before,
        "Queue rejection must happen before any ReleasePending-to-Released Kura mutation",
    );
    let cursor = fixture
        .kura
        .read_autonomous_lifecycle_cursor(&fixture.payload, &fixture.binding, &fixture.generation)
        .expect("read rejected replica lifecycle cursor");
    assert_eq!(cursor.cursor(), Some(&fixture.initial_live));
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
    let queue = open_empty_lifecycle_recovery_queue(&queue_dir, &restarted_state);
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
    let reopened_queue = open_empty_lifecycle_recovery_queue(&queue_dir, &reopened_state);
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
