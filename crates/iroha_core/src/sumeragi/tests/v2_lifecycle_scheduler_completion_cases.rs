#[cfg(test)]
#[allow(dead_code)] // Whole-state equality observes these fields in the fail-closed regression.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RecoveredBroadcastSchedulerStateForTest {
    records: BTreeMap<u128, super::LifecycleRecord>,
    key_index: BTreeMap<super::LifecycleKey, u128>,
    owner_index: BTreeMap<super::CausalRoot, super::OwnerId>,
    ready_index: BTreeSet<u128>,
    active_lease: Option<super::TurnLease>,
    high_water: u128,
    next_lease: Option<u128>,
    durable_records: BTreeMap<u128, super::schema::DurableRecordMetadata>,
    capacity_used: BTreeMap<CapacityClass, usize>,
    capacity_generation: BTreeMap<CapacityClass, u64>,
    observed_generation: BTreeMap<super::WaitSource, u64>,
    producer_debts: BTreeMap<u128, u128>,
    fault: Option<super::CoordinatorFault>,
    declares_pair: bool,
    paired_ordinal: Option<u128>,
}

#[cfg(test)]
mod recovered_sign_capacity_tests {
    use super::super::schema::SchedulerEpisode;
    use super::{
        AuthenticatedSchedulerInputsFactory, ProductionCompletionDispatchV1,
        ProductionCompletionReadyWorkV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1,
        ProductionRecoveredLifecycleSignedBroadcastRefanoutV1, ProductionV2Services,
        authenticated_ready_row,
    };
    use crate::sumeragi::v2_effects::{ConsensusBroadcastDisposition, V2EffectServices};
    use crate::sumeragi::v2_lifecycle_coordinator::{
        CapacityClass, CausalRoot, LifecycleDigest, LifecycleKey, LifecyclePhase, LifecycleRecord,
        LifecycleRound, LifecycleStage, LifecycleStageKind, LifecycleState, LifecycleWorkClass,
        OwnerId, PhysicalSlotId, PredecessorScope, ProductionLifecycleOutputAdmissionSettlementV1,
        ProductionLifecycleOwnerV1, SchedulerEpisodeUniverse, TerminalOutcome,
        work_registry::ReadyRecoveredLifecycleSignAttestationV1,
    };
    use iroha_crypto::{Hash, KeyPair};
    use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
        time::{Duration, Instant},
    };

    macro_rules! sumeragi_stack_test {
        ($name:ident, $body:block) => {
            #[test]
            fn $name() {
                let handle = crate::sumeragi::sumeragi_thread_builder(concat!(
                    "sumeragi-v2-scheduler-test-",
                    stringify!($name)
                ))
                .spawn(move || $body)
                .expect("spawn scheduler regression on the production consensus stack");
                if let Err(payload) = handle.join() {
                    std::panic::resume_unwind(payload);
                }
            }
        };
    }

    fn digest(byte: u8) -> LifecycleDigest {
        LifecycleDigest::new([byte; 32])
    }

    fn worker_context(keys: &[KeyPair]) -> wire::HeightContext {
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
            quorum: wire::DualQuorum::from_roster(&roster)
                .expect("scheduler fixture equal-vote quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"v2-worker-test-context"),
            execution_policy_hash: Hash::new(b"test execution policy"),
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
        context.validate().expect("valid scheduler fixture context");
        context
    }

    #[allow(clippy::type_complexity)]
    fn recovered_broadcast_scheduler_fixture() -> (
        ProductionLifecycleOwnerV1,
        ProductionV2Services,
        crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
        tempfile::TempDir,
        u128,
        u128,
        u128,
    ) {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("scheduler fixture validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified scheduler fixture context");
        let directory = tempfile::TempDir::new().expect("temporary scheduler fixture storage");
        let (mut owner, broadcast_ordinal, paired_ordinal, unrelated_ordinal) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let planner_io = owner.bind_body_store_to_planner_io_for_test(
            &mut services,
            Arc::clone(&output_guard),
            8,
        );
        (
            owner,
            services,
            planner_io,
            directory,
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        )
    }
    fn recovered_completion_runtime(
        verified: crate::sumeragi::v2::VerifiedHeightContext,
        root: &std::path::Path,
    ) -> crate::sumeragi::v2_runtime::SerializedV2Runtime {
        let (adapter, startup) = crate::sumeragi::v2::SumeragiV2Adapter::open(
            root.join("completion-runtime.wal"),
            verified,
            Some(0),
            crate::sumeragi::v2_core::Generation::new(1),
            [0xC7; 32],
            crate::sumeragi::v2::AdapterFingerprints {
                node: Hash::new(b"lifecycle completion node"),
                build: Hash::new(b"lifecycle completion build"),
                config: Hash::new(b"lifecycle completion config"),
            },
            crate::sumeragi::v2::DeferredAdmissionOrdinalSource::new(0),
        )
        .expect("open lifecycle Completion runtime");
        assert!(startup.is_empty());
        crate::sumeragi::v2_runtime::SerializedV2Runtime::new(
            adapter,
            startup,
            Instant::now(),
            Duration::from_secs(10),
            crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
        )
        .expect("wrap lifecycle Completion adapter")
        .0
    }
    #[test]
    fn recovered_sign_ready_row_reserves_its_broadcast_capacity_before_claim() {
        let context = digest(0x31);
        let round = LifecycleRound::new(4, 2);
        let key = LifecycleKey::new(
            context,
            round,
            Some(round),
            Some(digest(0x32)),
            LifecyclePhase::Proposal,
            Some(digest(0x33)),
        );
        let ordinal = 9;
        let owner = OwnerId::new(CausalRoot::new(digest(0x34)), ordinal);
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let record = LifecycleRecord {
            key,
            owner,
            ordinal,
            work_class: LifecycleWorkClass::SignProposal,
            stage: LifecycleStage::new(
                LifecycleStageKind::SignProposal,
                PredecessorScope::Independent,
            ),
            state: LifecycleState::Ready,
            physical_slots: BTreeMap::from([(slot, digest(0x35))]),
            episode: SchedulerEpisode {
                universe: SchedulerEpisodeUniverse {
                    target: key.scheduler_target(),
                    context,
                    leader: digest(0x36),
                    view: round.view(),
                    subject: key.subject(),
                    phase: key.phase(),
                    authenticated_roster_slots: BTreeSet::new(),
                    capacity_geometry: BTreeMap::new(),
                },
                slot_universe: BTreeSet::from([slot]),
                consumed_slots: BTreeSet::from([slot]),
                frozen_predecessors: BTreeSet::new(),
            },
        };
        let attestation = ReadyRecoveredLifecycleSignAttestationV1::for_test(&record)
            .expect("exact recovered Sign row mints its closed test attestation");
        let factory = AuthenticatedSchedulerInputsFactory::new();
        let row = authenticated_ready_row(
            &factory,
            &record,
            None,
            None,
            Some(attestation),
            None,
            [0; 6],
        )
        .expect("closed recovered Sign attestation authenticates its Ready row");
        assert_eq!(
            row.output_capacity_class(),
            Some(CapacityClass::Consensus),
            "every recovered signature must reserve its mandatory Broadcast slot before claim"
        );
    }

    #[test]
    fn standalone_recovered_sign_binds_broadcast_after_actor_global_ordinal_skew() {
        let (_services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("ordinal-skew validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified ordinal-skew context");
        let mut coordinator = crate::sumeragi::v2_lifecycle_coordinator::LifecycleCoordinator::new(
            super::super::projection::lifecycle_context(verified.context()),
            10,
            super::super::schema::CapacityGeometry::new(
                CapacityClass::ALL.into_iter().map(|class| (class, 8)),
            ),
        );
        let mut registry =
            crate::sumeragi::v2_lifecycle_coordinator::LifecycleWorkRegistryHolder::empty();
        let parent_ordinal = registry
            .add_recovered_next_vote_scheduler_fixture_for_test(&mut coordinator, &verified, 0x45)
            .expect("install exact standalone recovered Sign");
        assert_eq!(parent_ordinal, 11);
        let (projection, mut vote) =
            super::super::work_registry::recovered_next_vote_projection_for_scheduler_fixture(
                &verified, 0x45,
            );
        vote.signature = iroha_crypto::Signature::new(
            keys[0].private_key(),
            &crate::sumeragi::v2::SignRequest::Vote(vote.clone()).signature_preimage(),
        )
        .payload()
        .to_vec();
        let broadcast = super::super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1::from_next_wal_vote_for_scheduler_fixture(
            &projection,
            &verified,
            crate::sumeragi::v2::AdapterEffect::Broadcast(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
            ),
        )
        .expect("project exact standalone signed Broadcast");
        let record = &coordinator.records[&parent_ordinal];
        let ready = super::super::SchedulerReadyInputs::new(record, None, [0; 6]);
        let inputs = super::super::SchedulerInputs::new([], [(parent_ordinal, ready)])
            .expect("unique standalone Sign scheduler census");
        let super::super::TurnPlan::Execute(mut lease) = coordinator.plan_turn(inputs) else {
            panic!("claim the standalone recovered Sign")
        };
        lease.output_reservation = Some(super::super::schema::LeaseCapacityReservation::new(
            CapacityClass::Consensus,
            coordinator.capacity_generation[&CapacityClass::Consensus],
        ));
        coordinator.active_lease = Some(lease.clone());
        let (&parent_slot, _) = lease
            .physical_slots()
            .first_key_value()
            .expect("standalone Sign has one slot");
        let sign_address = super::super::work_registry::ConcreteWorkAddress::new(
            lease.owner(),
            parent_ordinal,
            parent_slot,
        )
        .expect("exact standalone Sign address");
        let local_prediction = coordinator
            .high_water
            .checked_add(1)
            .expect("local successor ordinal");
        assert_eq!(local_prediction, 12);
        let (runtime_ordinals, coordinator_ordinals) =
            super::super::authority::lifecycle_ordinal_authorities_after_high_watermark(
                coordinator.high_water,
            );
        coordinator.lifecycle_ordinal_authority = Some(coordinator_ordinals);
        let runtime_ordinals =
            crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::from_authority(
                runtime_ordinals,
            );
        runtime_ordinals
            .advance_past(12)
            .expect("advance actor-global ordinals past the local prediction");
        let (staged, ordinal_reservation, child_ordinal, child_slot, child_digest) =
            super::super::body_pipeline_transition::stage_recovered_lifecycle_sign_broadcast_for_test(
                &coordinator,
                &lease,
                broadcast.candidate().clone(),
            )
            .expect("stage Broadcast at the actor-global reserved ordinal");
        assert_eq!(child_ordinal, 13);
        assert_ne!(child_ordinal, local_prediction);
        let bound =
            super::super::work_registry::exact_staged_recovered_lifecycle_broadcast_address(
                registry.registry_for_test(),
                sign_address,
                &broadcast,
                &verified,
                &staged,
                child_ordinal,
                child_slot,
                child_digest,
            )
            .expect("bind registry successor to the exact staged Broadcast row");
        assert_eq!(bound.ordinal, 13);
        assert!(
            super::super::work_registry::exact_staged_recovered_lifecycle_broadcast_address(
                registry.registry_for_test(),
                sign_address,
                &broadcast,
                &verified,
                &staged,
                local_prediction,
                child_slot,
                child_digest,
            )
            .is_err(),
            "the stale coordinator-local prediction must not name a registry child"
        );
        drop(ordinal_reservation);
    }

    #[test]
    fn composite_recovered_completion_dispatches_one_ranked_sign_and_preserves_the_other() {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("composite scheduler validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified composite scheduler context");
        let directory = tempfile::TempDir::new().expect("temporary composite scheduler storage");
        let runtime = recovered_completion_runtime(verified.clone(), directory.path());
        let (mut owner, broadcast, paired, unrelated) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        assert!(owner.retire_ready_work_for_completion_test(broadcast));
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let (mut executor, planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            0,
            2,
        );

        assert_eq!(
            owner
                .dispatch_completion_with_runner_debt(&mut services, &mut executor, 0,)
                .expect("the joint physical census dispatches one exact Sign"),
            ProductionCompletionDispatchV1::SignQueued { ordinal: paired }
        );
        let state = owner.recovered_broadcast_scheduler_state_for_test(broadcast);
        assert!(matches!(
            state.records[&paired].state,
            LifecycleState::Claimed(_)
        ));
        assert_eq!(state.records[&unrelated].state, LifecycleState::Ready);
        assert!(state.active_lease.is_some());
        assert!(state.fault.is_none());
        assert!(!output_guard.restart_required());
        planner_io.detach(&mut services);
    }

    sumeragi_stack_test!(
        direct_broadcast_deferred_behind_completion_is_passive_and_sign_is_selected,
        {
            let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
            let context = worker_context(&keys);
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("direct-output scheduler proof of possession")
                })
                .collect::<Vec<_>>();
            let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
                .expect("verified direct-output scheduler context");
            let directory =
                tempfile::TempDir::new().expect("temporary direct-output scheduler store");
            let runtime = recovered_completion_runtime(verified.clone(), directory.path());
            let (mut owner, broadcast, paired, unrelated) =
                ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                    verified,
                    &keys[0],
                    directory.path(),
                );
            let (direct, _direct_pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x71)
                .expect("defer one exact direct timeout Broadcast behind Ready completion work");
            assert!(owner.retire_ready_work_for_completion_test(broadcast));
            assert_eq!(
                owner.exact_ready_completion_classification_for_test(),
                ProductionCompletionReadyWorkV1::CompletionIo,
                "the later direct output cannot masquerade as recovered refanout"
            );
            let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
            let (mut executor, planner_io) = owner
                .bind_body_store_to_lifecycle_completion_io_for_test(
                    &mut services,
                    runtime,
                    Arc::clone(&output_guard),
                    0,
                    2,
                );

            assert_eq!(
                owner
                    .dispatch_completion_with_runner_debt(&mut services, &mut executor, 0)
                    .expect("the exact completion census retains direct output passively"),
                ProductionCompletionDispatchV1::SignQueued { ordinal: paired }
            );
            let state = owner.recovered_broadcast_scheduler_state_for_test(broadcast);
            assert!(matches!(
                state.records[&paired].state,
                LifecycleState::Claimed(_)
            ));
            assert_eq!(state.records[&unrelated].state, LifecycleState::Ready);
            assert_eq!(state.records[&direct].state, LifecycleState::Ready);
            assert!(state.ready_index.contains(&direct));
            assert!(state.active_lease.is_some());
            assert!(state.fault.is_none());
            assert!(!output_guard.restart_required());
            planner_io.detach(&mut services);
        }
    );

    sumeragi_stack_test!(
        prospectively_woken_direct_broadcast_is_authenticated_and_sign_is_selected,
        {
            let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
            let context = worker_context(&keys);
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("prospective Broadcast proof of possession")
                })
                .collect::<Vec<_>>();
            let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
                .expect("verified prospective Broadcast context");
            let directory =
                tempfile::TempDir::new().expect("temporary prospective Broadcast store");
            let runtime = recovered_completion_runtime(verified.clone(), directory.path());
            let (mut owner, broadcast, paired, unrelated) =
                ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                    verified,
                    &keys[0],
                    directory.path(),
                );
            let (direct, _direct_pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x71)
                .expect("defer one exact direct Broadcast behind completion work");
            assert!(owner.retire_ready_work_for_completion_test(broadcast));
            let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
            let (mut executor, planner_io) = owner
                .bind_body_store_to_lifecycle_completion_io_for_test(
                    &mut services,
                    runtime,
                    Arc::clone(&output_guard),
                    0,
                    2,
                );
            let fence = executor.lifecycle_reducer_fence_observation();
            assert!(owner.park_direct_broadcast_before_fence_for_test(direct, fence));
            assert_eq!(
                owner.classify_completion_ready_work(fence),
                ProductionCompletionReadyWorkV1::CompletionIo,
                "the exact prospective direct output remains passive beside recovered Sign work"
            );

            assert_eq!(
                owner
                    .dispatch_completion_with_runner_debt(&mut services, &mut executor, 0)
                    .expect("the fence-aware census dispatches the oldest recovered Sign"),
                ProductionCompletionDispatchV1::SignQueued { ordinal: paired }
            );
            let state = owner.recovered_broadcast_scheduler_state_for_test(broadcast);
            assert!(matches!(
                state.records[&paired].state,
                LifecycleState::Claimed(_)
            ));
            assert_eq!(state.records[&unrelated].state, LifecycleState::Ready);
            assert_eq!(state.records[&direct].state, LifecycleState::Ready);
            assert!(state.ready_index.contains(&direct));
            assert!(state.fault.is_none());
            assert!(!output_guard.restart_required());
            planner_io.detach(&mut services);
        }
    );

    sumeragi_stack_test!(
        prospectively_woken_direct_broadcast_rejects_a_mismatched_carrier,
        {
            let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
            let context = worker_context(&keys);
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("mismatched prospective Broadcast proof of possession")
                })
                .collect::<Vec<_>>();
            let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
                .expect("verified mismatched prospective Broadcast context");
            let directory = tempfile::TempDir::new().expect("temporary mismatched Broadcast store");
            let runtime = recovered_completion_runtime(verified.clone(), directory.path());
            let (mut owner, _broadcast, _paired, _unrelated) =
                ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                    verified,
                    &keys[0],
                    directory.path(),
                );
            let (direct, _direct_pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x73)
                .expect("defer one exact direct Broadcast before corruption");
            let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
            let (executor, planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
                &mut services,
                runtime,
                Arc::clone(&output_guard),
                0,
                2,
            );
            let fence = executor.lifecycle_reducer_fence_observation();
            assert!(owner.park_direct_broadcast_before_fence_for_test(direct, fence));
            assert!(owner.corrupt_ready_digest_for_test(direct));
            assert_eq!(
                owner.classify_completion_ready_work(fence),
                ProductionCompletionReadyWorkV1::Invalid,
                "a fence wake cannot bypass exact direct-output carrier authentication"
            );
            assert!(!output_guard.restart_required());
            planner_io.detach(&mut services);
        }
    );

    sumeragi_stack_test!(apply_terminal_settles_only_the_attested_direct_broadcast, {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("post-Apply direct-output proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified post-Apply direct-output context");
        let directory = tempfile::TempDir::new().expect("temporary post-Apply direct-output store");
        let runtime = recovered_completion_runtime(verified.clone(), directory.path());
        let (mut owner, recovered, paired, unrelated) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        let (direct, pending) = owner
            .defer_direct_timeout_broadcast_for_test(0x74)
            .expect("defer one exact direct Broadcast behind recovered work");
        for ordinal in [recovered, paired, unrelated] {
            assert!(owner.retire_ready_work_for_completion_test(ordinal));
        }
        assert_eq!(
            owner.exact_ready_completion_classification_for_test(),
            ProductionCompletionReadyWorkV1::RetainedDirectOutput
        );
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let (mut executor, planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            0,
            2,
        );
        services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
        assert!(executor.install_pending_lifecycle_output_for_test(*pending));
        let prepared = owner
            .prepare_apply_terminal_direct_broadcast()
            .expect("bind the exact Ready minimum to its pending output key");
        assert_eq!(prepared.ordinal(), direct);
        assert_eq!(
                executor
                    .settle_apply_terminal_direct_broadcast(
                        &mut owner,
                        &mut services,
                        prepared,
                    )
                    .expect("settle only the sealed post-Apply direct Broadcast"),
                crate::sumeragi::v2_effects::ProductionApplyTerminalDirectBroadcastSettlementV1::Completed
            );
        assert!(!executor.has_pending_lifecycle_output_admissions());
        let state = owner.recovered_broadcast_scheduler_state_for_test(recovered);
        assert!(matches!(
            state.records[&direct].state,
            LifecycleState::Terminal(TerminalOutcome::Advanced)
        ));
        assert!(!output_guard.restart_required());
        planner_io.detach(&mut services);
    });

    sumeragi_stack_test!(
        apply_terminal_direct_broadcast_source_retention_reinstalls_exact_pending_owner,
        {
            let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
            let context = worker_context(&keys);
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("post-Apply retained-output proof of possession")
                })
                .collect::<Vec<_>>();
            let verified =
                crate::sumeragi::v2::VerifiedHeightContext::genesis(context.clone(), proofs)
                    .expect("verified post-Apply retained-output context");
            let directory =
                tempfile::TempDir::new().expect("temporary retained post-Apply output store");
            let runtime = recovered_completion_runtime(verified.clone(), directory.path());
            let (mut owner, recovered, paired, unrelated) =
                ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                    verified,
                    &keys[0],
                    directory.path(),
                );
            let (direct, pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x75)
                .expect("defer one exact direct Broadcast for retained retry");
            for ordinal in [recovered, paired, unrelated] {
                assert!(owner.retire_ready_work_for_completion_test(ordinal));
            }
            let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
            let (mut executor, planner_io) = owner
                .bind_body_store_to_lifecycle_completion_io_for_test(
                    &mut services,
                    runtime,
                    Arc::clone(&output_guard),
                    0,
                    2,
                );
            assert!(executor.install_pending_lifecycle_output_for_test(*pending));
            let fence = executor.lifecycle_reducer_fence_observation();
            assert!(owner.park_direct_broadcast_before_fence_for_test(direct, fence));
            assert_eq!(
                owner.classify_completion_ready_work(fence),
                ProductionCompletionReadyWorkV1::RetainedDirectOutput
            );
            assert!(
                owner.prepare_apply_terminal_direct_broadcast().is_none(),
                "a fenced row cannot combine its wake and service authority"
            );
            assert_eq!(
                owner.wake_apply_terminal_direct_broadcast_if_fenced(fence),
                Ok(true)
            );
            let prepared = owner
                .prepare_apply_terminal_direct_broadcast()
                .expect("wake and seal the stale-fence direct Broadcast");
            assert_eq!(prepared.ordinal(), direct);
            services
                .set_exact_output_shared_unit_capacity_for_test(1)
                .expect("install one shared exact-output unit for source retention");
            services.set_exact_output_admission_hook(|post, ticket| {
                Err(
                    iroha_p2p::network::NetworkActorAdmissionError::Backpressured {
                        message: post,
                        ticket,
                        rank: 1,
                    },
                )
            });
            let blocker = wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                    round: wire::ConsensusRound {
                        context_id: context.id(),
                        height: context.height,
                        view: 0x74,
                    },
                    highest_prepare_qc: None,
                    signer: 0,
                    signature: vec![0x74],
                }),
            );
            assert_eq!(
                services
                    .broadcast_consensus(blocker)
                    .expect("retain one exact pacemaker fanout in the service corridor"),
                ConsensusBroadcastDisposition::ExactServiceAccepted
            );
            assert!(
                services
                    .has_pending_exact_output()
                    .expect("inspect retained exact-output service ownership")
            );
            assert_eq!(
                executor
                    .settle_apply_terminal_direct_broadcast(
                        &mut owner,
                        &mut services,
                        prepared,
                    )
                    .expect("retain the exact post-Apply pending owner under backpressure"),
                crate::sumeragi::v2_effects::ProductionApplyTerminalDirectBroadcastSettlementV1::SourceRetained
            );
            assert!(executor.has_pending_lifecycle_output_admissions());
            let state = owner.recovered_broadcast_scheduler_state_for_test(recovered);
            assert_eq!(state.records[&direct].state, LifecycleState::Ready);
            assert!(!output_guard.restart_required());

            services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
            assert!(
                !services
                    .retry_pending_exact_output()
                    .expect("drain the incumbent exact-output corridor owner")
            );
            let prepared = owner
                .prepare_apply_terminal_direct_broadcast()
                .expect("reseal the unchanged direct Broadcast after source retention");
            assert_eq!(prepared.ordinal(), direct);
            assert_eq!(
                executor
                    .settle_apply_terminal_direct_broadcast(
                        &mut owner,
                        &mut services,
                        prepared,
                    )
                    .expect("complete the retained post-Apply Broadcast on retry"),
                crate::sumeragi::v2_effects::ProductionApplyTerminalDirectBroadcastSettlementV1::Completed
            );
            assert!(!executor.has_pending_lifecycle_output_admissions());
            let state = owner.recovered_broadcast_scheduler_state_for_test(recovered);
            assert!(matches!(
                state.records[&direct].state,
                LifecycleState::Terminal(TerminalOutcome::Advanced)
            ));
            assert!(!output_guard.restart_required());
            planner_io.detach(&mut services);
        }
    );

    sumeragi_stack_test!(apply_terminal_wakes_fenced_direct_before_settlement, {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("post-Apply fence-wake proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified post-Apply fence-wake context");
        let directory = tempfile::TempDir::new().expect("temporary post-Apply fence-wake store");
        let runtime = recovered_completion_runtime(verified.clone(), directory.path());
        let (mut owner, recovered, paired, unrelated) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        let (direct, pending) = owner
            .defer_direct_timeout_broadcast_for_test(0x75)
            .expect("defer one direct Broadcast for a reducer-fence wake");
        for ordinal in [recovered, paired, unrelated] {
            assert!(owner.retire_ready_work_for_completion_test(ordinal));
        }
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let (mut executor, planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            0,
            2,
        );
        services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
        let fence = executor.lifecycle_reducer_fence_observation();
        assert!(owner.park_direct_broadcast_before_fence_for_test(direct, fence));
        assert_eq!(
            owner.classify_completion_ready_work(fence),
            ProductionCompletionReadyWorkV1::RetainedDirectOutput
        );
        assert_eq!(
            owner.wake_apply_terminal_direct_broadcast_if_fenced(fence),
            Ok(true),
            "the terminal barrier must publish the fence wake before cold-output ordering"
        );
        assert_eq!(
            owner.wake_apply_terminal_direct_broadcast_if_fenced(fence),
            Ok(false),
            "the same reducer generation cannot wake the direct row twice"
        );
        let state = owner.recovered_broadcast_scheduler_state_for_test(recovered);
        assert_eq!(state.ready_index.first().copied(), Some(direct));
        assert_eq!(state.records[&direct].state, LifecycleState::Ready);

        assert!(executor.install_pending_lifecycle_output_for_test(*pending));
        let prepared = owner
            .prepare_apply_terminal_direct_broadcast()
            .expect("seal the newly Ready direct Broadcast");
        assert_eq!(prepared.ordinal(), direct);
        assert_eq!(
                executor
                    .settle_apply_terminal_direct_broadcast(
                        &mut owner,
                        &mut services,
                        prepared,
                    )
                    .expect("settle the fence-woken direct Broadcast"),
                crate::sumeragi::v2_effects::ProductionApplyTerminalDirectBroadcastSettlementV1::Completed
            );
        assert!(!output_guard.restart_required());
        planner_io.detach(&mut services);
    });

    sumeragi_stack_test!(
        apply_terminal_fence_wake_rejects_collateral_non_broadcast,
        {
            let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
            let context = worker_context(&keys);
            let proofs = keys
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("post-Apply collateral-wake proof of possession")
                })
                .collect::<Vec<_>>();
            let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
                .expect("verified post-Apply collateral-wake context");
            let directory =
                tempfile::TempDir::new().expect("temporary post-Apply collateral-wake store");
            let runtime = recovered_completion_runtime(verified.clone(), directory.path());
            let (mut owner, recovered, paired, unrelated) =
                ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                    verified,
                    &keys[0],
                    directory.path(),
                );
            let (direct, _pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x79)
                .expect("defer the lower direct Broadcast");
            for ordinal in [recovered, paired, unrelated] {
                assert!(owner.retire_ready_work_for_completion_test(ordinal));
            }
            let collateral = owner.add_recovered_next_vote_completion_for_test(0x7A);
            assert!(direct < collateral);
            let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
            let (executor, planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
                &mut services,
                runtime,
                Arc::clone(&output_guard),
                0,
                2,
            );
            let fence = executor.lifecycle_reducer_fence_observation();
            assert!(owner.park_direct_broadcast_before_fence_for_test(direct, fence));
            assert!(owner.park_ready_work_before_fence_for_test(collateral, fence));
            let before = owner.recovered_broadcast_scheduler_state_for_test(recovered);
            for ordinal in [direct, collateral] {
                assert!(matches!(
                    before.records[&ordinal].state,
                    LifecycleState::Waiting(wait)
                        if wait.source() == fence.source()
                            && wait.observed_generation() < fence.generation()
                ));
            }
            assert!(before.ready_index.is_empty());
            assert_eq!(
                owner.classify_completion_ready_work(fence),
                ProductionCompletionReadyWorkV1::RetainedDirectOutput,
                "the lower direct row demonstrates why minimum-only classification is insufficient"
            );
            assert!(matches!(
                owner.wake_apply_terminal_direct_broadcast_if_fenced(fence),
                Err(super::ProductionSchedulerInputsError::UnsupportedReadyCarrier {
                    ordinal,
                    work_class: LifecycleWorkClass::SignVote,
                }) if ordinal == collateral
            ));
            assert_eq!(
                owner.recovered_broadcast_scheduler_state_for_test(recovered),
                before,
                "the shared generation must not advance when it would wake a non-Broadcast"
            );
            assert!(owner.prepare_apply_terminal_direct_broadcast().is_none());
            assert!(!output_guard.restart_required());
            planner_io.detach(&mut services);
        }
    );

    sumeragi_stack_test!(apply_terminal_reenters_for_each_ordered_direct_broadcast, {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("post-Apply multi-output proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified post-Apply multi-output context");
        let directory = tempfile::TempDir::new().expect("temporary post-Apply multi-output store");
        let runtime = recovered_completion_runtime(verified.clone(), directory.path());
        let (mut owner, recovered, paired, unrelated) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        let (first, first_pending) = owner
            .defer_direct_timeout_broadcast_for_test(0x76)
            .expect("defer the first direct Broadcast");
        let (second, second_pending) = owner
            .defer_direct_timeout_broadcast_for_test(0x77)
            .expect("defer the second direct Broadcast");
        assert!(first < second);
        for ordinal in [recovered, paired, unrelated] {
            assert!(owner.retire_ready_work_for_completion_test(ordinal));
        }
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let (mut executor, planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            0,
            2,
        );
        services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
        assert!(executor.install_pending_lifecycle_output_for_test(*first_pending));
        assert!(executor.install_pending_lifecycle_output_for_test(*second_pending));
        let fence = executor.lifecycle_reducer_fence_observation();

        for (index, expected) in [first, second].into_iter().enumerate() {
            assert_eq!(
                owner.classify_completion_ready_work(fence),
                ProductionCompletionReadyWorkV1::RetainedDirectOutput
            );
            let prepared = owner
                .prepare_apply_terminal_direct_broadcast()
                .expect("seal the next exact direct Broadcast");
            assert_eq!(prepared.ordinal(), expected);
            assert_eq!(
                    executor
                        .settle_apply_terminal_direct_broadcast(
                            &mut owner,
                            &mut services,
                            prepared,
                        )
                        .expect("settle one ordered direct Broadcast"),
                    crate::sumeragi::v2_effects::ProductionApplyTerminalDirectBroadcastSettlementV1::Completed
                );
            assert_eq!(
                executor.has_pending_lifecycle_output_admissions(),
                index == 0,
                "only the later exact output may remain after one settlement"
            );
            let state = owner.recovered_broadcast_scheduler_state_for_test(recovered);
            assert!(matches!(
                state.records[&expected].state,
                LifecycleState::Terminal(TerminalOutcome::Advanced)
            ));
        }
        assert!(!output_guard.restart_required());
        planner_io.detach(&mut services);
    });

    #[test]
    fn composite_recovered_completion_capacity_unavailable_claims_no_ready_sign() {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("capacity scheduler validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified capacity scheduler context");
        let directory = tempfile::TempDir::new().expect("temporary capacity scheduler storage");
        let runtime = recovered_completion_runtime(verified.clone(), directory.path());
        let (mut owner, broadcast, paired, unrelated) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        assert!(owner.retire_ready_work_for_completion_test(broadcast));
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let (mut executor, planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
            &mut services,
            runtime,
            Arc::clone(&output_guard),
            0,
            1,
        );
        planner_io.saturate_consensus_prefix(&services);
        let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast);

        assert_eq!(
            owner
                .dispatch_completion_with_runner_debt(&mut services, &mut executor, 0,)
                .expect("a saturated joint census is a typed unavailable turn"),
            ProductionCompletionDispatchV1::CapacityUnavailable {
                protected_live_apply_ordinal: None,
            }
        );
        assert_eq!(
            owner.recovered_broadcast_scheduler_state_for_test(broadcast),
            before,
            "physical unavailability cannot claim or reorder either Ready Sign"
        );
        assert_eq!(before.records[&paired].state, LifecycleState::Ready);
        assert_eq!(before.records[&unrelated].state, LifecycleState::Ready);
        assert!(!output_guard.restart_required());
        planner_io.release_all_predecessors();
        planner_io.detach(&mut services);
    }

    #[test]
    fn recovered_broadcast_refanout_ranks_exact_pair_before_unrelated_ready_sign() {
        let (
            mut owner,
            services,
            _planner_io,
            _directory,
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        ) = recovered_broadcast_scheduler_fixture();
        let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(before.declares_pair);
        assert_eq!(before.paired_ordinal, Some(paired_ordinal));
        assert_eq!(broadcast_ordinal.checked_add(1), Some(paired_ordinal));
        assert!(before.ready_index.contains(&broadcast_ordinal));
        assert!(before.ready_index.contains(&paired_ordinal));
        assert!(before.ready_index.contains(&unrelated_ordinal));

        assert_eq!(
            owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                .expect("the complete exact pair census refans out"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: broadcast_ordinal,
            }
        );

        let after = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(matches!(
            after.records[&broadcast_ordinal].state,
            LifecycleState::Waiting(_)
        ));
        assert_eq!(after.records[&paired_ordinal].state, LifecycleState::Ready);
        assert_eq!(
            after.records[&unrelated_ordinal].state,
            LifecycleState::Ready
        );
        assert!(after.active_lease.is_none());
        assert!(after.fault.is_none());

        let (
            mut bounded_owner,
            bounded_services,
            _bounded_planner_io,
            _bounded_directory,
            bounded_broadcast,
            bounded_pair,
            bounded_unrelated,
        ) = recovered_broadcast_scheduler_fixture();
        assert!(bounded_owner.retire_unrelated_sign_for_finalization_test(bounded_unrelated));
        assert_eq!(
            bounded_owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(
                    &bounded_services,
                    0,
                )
                .expect("the bounded finalization pair refans out"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: bounded_broadcast,
            }
        );
        assert!(
            !bounded_owner.finalization_registry_census_is_exact_for_test(),
            "finalization cannot consume the still-schedulable paired next Sign"
        );
        assert!(bounded_owner.retire_unrelated_sign_for_finalization_test(bounded_pair));
        assert!(
            bounded_owner.finalization_registry_census_is_exact_for_test(),
            "finalization accepts the exact volatile refanout wait after its next Sign retires"
        );
        assert!(bounded_owner.corrupt_recovered_broadcast_pair_link_for_test(bounded_broadcast));
        assert!(
            !bounded_owner.finalization_registry_census_is_exact_for_test(),
            "finalization rejects a corrupted retained digest after paired Sign retirement"
        );
    }

    #[test]
    fn finalization_waits_for_every_authenticated_recovered_broadcast_refanout() {
        let (mut services, keys) = crate::sumeragi::v2_worker::tests::fixture();
        let context = worker_context(&keys);
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("multi-refanout fixture validator proof of possession")
            })
            .collect::<Vec<_>>();
        let verified = crate::sumeragi::v2::VerifiedHeightContext::genesis(context, proofs)
            .expect("verified multi-refanout scheduler context");
        let directory =
            tempfile::TempDir::new().expect("temporary multi-refanout scheduler storage");
        let (mut owner, first_broadcast, paired, unrelated) =
            ProductionLifecycleOwnerV1::recovered_broadcast_pair_scheduler_fixture_for_test(
                verified,
                &keys[0],
                directory.path(),
            );
        let second_broadcast = owner
            .add_recovered_broadcast_scheduler_fixture_for_test(&keys[0], 0x71)
            .expect("add a second exact recovered signed-Broadcast carrier");
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let planner_io = owner.bind_body_store_to_planner_io_for_test(
            &mut services,
            Arc::clone(&output_guard),
            8,
        );

        assert!(
            !owner.finalization_registry_census_is_exact_for_test(),
            "Ready recovered Broadcasts remain schedulable before rollover"
        );
        assert_eq!(
            owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                .expect("refan the oldest exact recovered Broadcast"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: first_broadcast,
            }
        );
        assert!(owner.retire_unrelated_sign_for_finalization_test(paired));
        assert!(owner.retire_unrelated_sign_for_finalization_test(unrelated));
        assert!(
            !owner.finalization_registry_census_is_exact_for_test(),
            "the second Ready Broadcast must receive its own Completion turn"
        );
        assert_eq!(
            owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                .expect("refan the second exact recovered Broadcast"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: second_broadcast,
            }
        );
        assert!(
            owner.finalization_registry_census_is_exact_for_test(),
            "all exact post-output waits form one bounded finalization census"
        );
        assert!(owner.corrupt_recovered_broadcast_wait_for_finalization_test(first_broadcast));
        assert!(
            !owner.finalization_registry_census_is_exact_for_test(),
            "a foreign Recovery wait cannot borrow another Broadcast's output handoff"
        );
        assert!(!output_guard.restart_required());
        planner_io.detach(&mut services);
    }

    sumeragi_stack_test!(
        recovered_refanout_authenticates_coexisting_direct_broadcast_without_claiming_it,
        {
            let (mut owner, services, _planner_io, _directory, broadcast, _paired, _unrelated) =
                recovered_broadcast_scheduler_fixture();
            let (direct, _direct_pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x72)
                .expect("defer one exact direct timeout Broadcast beside recovered refanout");
            assert_eq!(
                owner.exact_ready_completion_classification_for_test(),
                ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast
            );

            assert_eq!(
                owner
                    .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                    .expect("refanout authenticates the complete mixed Broadcast census"),
                ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                    ordinal: broadcast
                }
            );
            let after = owner.recovered_broadcast_scheduler_state_for_test(broadcast);
            assert!(matches!(
                after.records[&broadcast].state,
                LifecycleState::Waiting(_)
            ));
            assert_eq!(after.records[&direct].state, LifecycleState::Ready);
            assert!(after.ready_index.contains(&direct));
            assert!(after.active_lease.is_none());
            assert!(after.fault.is_none());
        }
    );

    sumeragi_stack_test!(
        broadcast_carrier_classifier_rejects_mismatched_direct_output,
        {
            let (mut owner, services, _planner_io, _directory, broadcast, _paired, _unrelated) =
                recovered_broadcast_scheduler_fixture();
            let (direct, _direct_pending) = owner
                .defer_direct_timeout_broadcast_for_test(0x73)
                .expect("defer one exact direct timeout Broadcast before corruption");
            assert!(owner.corrupt_ready_digest_for_test(direct));
            assert_eq!(
                owner.exact_ready_completion_classification_for_test(),
                ProductionCompletionReadyWorkV1::Invalid,
                "a mismatched direct carrier must fail before route selection"
            );
            let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast);
            assert_eq!(
                owner.refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0),
                Err(ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier)
            );
            assert_eq!(
                owner.recovered_broadcast_scheduler_state_for_test(broadcast),
                before,
                "foreign direct-output coordinates fail before coordinator mutation"
            );
        }
    );

    #[test]
    fn generic_retransmit_of_recovered_broadcast_stutters_without_registry_mutation() {
        let (
            mut owner,
            services,
            _planner_io,
            _directory,
            broadcast_ordinal,
            _paired_ordinal,
            _unrelated_ordinal,
        ) = recovered_broadcast_scheduler_fixture();
        let ready_before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        let ready_retransmit = owner
            .recovered_broadcast_runtime_retransmit_for_test(broadcast_ordinal, 11, 0xA1)
            .expect("seal the byte-identical Ready recovered-Broadcast retransmit");
        assert!(matches!(
            owner.settle_lifecycle_output_admission::<()>(
                ready_retransmit,
                |_effect, _ownership| {
                    panic!("the typed recovered Broadcast remains the sole service-I/O owner")
                },
            ),
            ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted
        ));
        assert_eq!(
            owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal),
            ready_before,
            "generic settlement cannot mutate or terminalize the Ready typed carrier"
        );

        assert_eq!(
            owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                .expect("the typed owner performs its exact refanout"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: broadcast_ordinal,
            }
        );
        let waiting_before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(matches!(
            waiting_before.records[&broadcast_ordinal].state,
            LifecycleState::Waiting(_)
        ));
        let waiting_retransmit = owner
            .recovered_broadcast_runtime_retransmit_for_test(broadcast_ordinal, 12, 0xA2)
            .expect("seal the byte-identical Waiting recovered-Broadcast retransmit");
        assert!(matches!(
            owner.settle_lifecycle_output_admission::<()>(
                waiting_retransmit,
                |_effect, _ownership| {
                    panic!("generic settlement cannot repeat typed recovered refanout")
                },
            ),
            ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted
        ));
        assert_eq!(
            owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal),
            waiting_before,
            "generic settlement cannot mutate the volatile typed refanout wait"
        );
    }

    #[test]
    fn recovered_broadcast_refanout_treats_adjacent_unlinked_sign_independently() {
        let (
            mut owner,
            services,
            _planner_io,
            _directory,
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        ) = recovered_broadcast_scheduler_fixture();
        assert_eq!(broadcast_ordinal.checked_add(1), Some(paired_ordinal));
        assert!(owner.clear_recovered_broadcast_pair_link_for_test(broadcast_ordinal));
        let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(!before.declares_pair);
        assert_eq!(before.paired_ordinal, None);

        assert_eq!(
            owner
                .refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0)
                .expect("an adjacent unlinked Sign uses ordinary Sign attestation"),
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned {
                ordinal: broadcast_ordinal,
            }
        );
        let after = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert_eq!(after.records[&paired_ordinal].state, LifecycleState::Ready);
        assert_eq!(
            after.records[&unrelated_ordinal].state,
            LifecycleState::Ready
        );
        assert!(after.active_lease.is_none());
        assert!(after.fault.is_none());
    }

    #[test]
    fn recovered_broadcast_refanout_rejects_corrupt_retained_link_without_mutation() {
        let (
            mut owner,
            services,
            _planner_io,
            _directory,
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        ) = recovered_broadcast_scheduler_fixture();
        assert!(owner.corrupt_recovered_broadcast_pair_link_for_test(broadcast_ordinal));
        let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal);
        assert!(before.declares_pair);
        assert_eq!(before.paired_ordinal, None);
        assert!(before.ready_index.contains(&broadcast_ordinal));
        assert!(before.ready_index.contains(&paired_ordinal));
        assert!(before.ready_index.contains(&unrelated_ordinal));

        assert_eq!(
            owner.refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(&services, 0),
            Err(ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidCarrier)
        );
        assert_eq!(
            owner.recovered_broadcast_scheduler_state_for_test(broadcast_ordinal),
            before,
            "a declared but invalid retained link must fail before coordinator mutation"
        );
        assert!(owner.retire_unrelated_sign_for_finalization_test(unrelated_ordinal));
        assert!(
            !owner.finalization_registry_census_is_exact_for_test(),
            "finalization must reject the corrupted exact next-Sign link"
        );
    }
}
#[cfg(test)]
impl LifecycleCoordinator {
    /// Exercise the sealed production factory without constructing storage.
    pub(super) fn direct_registry_scheduler_inputs_for_test(
        &self,
        registry: &LifecycleWorkRegistryHolder,
    ) -> Result<SchedulerInputs, ProductionSchedulerInputsError> {
        direct_registry_scheduler_inputs(self, registry)
    }
}
#[cfg(test)]
impl ProductionLifecycleOwnerV1 {
    /// Add one closed WAL-backed Sign beside an existing recovered I/O row.
    pub(in crate::sumeragi) fn add_recovered_next_vote_completion_for_test(
        &mut self,
        marker: u8,
    ) -> u128 {
        self.registry
            .add_recovered_next_vote_scheduler_fixture_for_test(
                &mut self.coordinator,
                &self.verified,
                marker,
            )
            .expect("install one exact recovered next-Vote Sign fixture")
    }

    /// Recheck one selected and one preserved row without exposing owner parts.
    pub(in crate::sumeragi) fn lifecycle_completion_selection_is_exact_for_test(
        &self,
        selected: u128,
        preserved: u128,
    ) -> bool {
        self.coordinator.fault.is_none()
            && self.coordinator.active_lease.as_ref().is_some_and(|lease| {
                lease.ordinal() == selected
                    && self
                        .coordinator
                        .records
                        .get(&selected)
                        .is_some_and(|record| {
                            matches!(record.state, LifecycleState::Claimed(id) if id == lease.id())
                        })
            })
            && self
                .coordinator
                .records
                .get(&preserved)
                .is_some_and(|record| record.state == LifecycleState::Ready)
            && self.coordinator.ready_index.contains(&preserved)
            && !self.coordinator.ready_index.contains(&selected)
    }

    /// Build the opaque recovered Broadcast-pair census used by scheduler tests.
    ///
    /// The returned scalars are ordinals only; every WAL, body, signature, and
    /// concrete-work authority remains owned by the production-shaped owner.
    fn recovered_broadcast_pair_scheduler_fixture_for_test(
        verified: crate::sumeragi::v2::VerifiedHeightContext,
        local_signer: &iroha_crypto::KeyPair,
        root: &std::path::Path,
    ) -> (Self, u128, u128, u128) {
        use super::{CapacityClass, schema::CapacityGeometry};

        let context = super::projection::lifecycle_context(verified.context());
        let mut coordinator = LifecycleCoordinator::new(
            context,
            0,
            CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 8))),
        );
        let (registry, broadcast_ordinal, paired_ordinal, unrelated_ordinal) =
            LifecycleWorkRegistryHolder::recovered_broadcast_pair_scheduler_fixture_for_test(
                &mut coordinator,
                &verified,
                local_signer,
            );
        coordinator
            .attach_empty_test_ledger(&root.join("ledger"))
            .expect("persist the recovered scheduler fixture ledger");
        let (_runtime_ordinal_authority, coordinator_ordinal_authority) =
            super::authority::lifecycle_ordinal_authorities_after_high_watermark(
                coordinator.high_water(),
            );
        coordinator
            .bind_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)
            .expect("bind the recovered scheduler fixture ordinal authority");
        let body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            root.join("body"),
            verified.context().clone(),
        )
        .expect("open exact scheduler fixture body store");
        let (payload_store, recovery) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &root.join("serve"),
                verified.context(),
            )
            .expect("open exact scheduler fixture Serve payload store");
        let serve_payloads = recovery
            .authenticate(&verified, local_signer, &body_store)
            .expect("authenticate empty scheduler fixture Serve payload census");
        (
            Self {
                verified,
                coordinator,
                registry,
                recovered_lifecycle_outputs: None,
                payload_store,
                serve_payloads,
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: Some(
                    crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
                ),
                timeout_supersession_successor: None,
            },
            broadcast_ordinal,
            paired_ordinal,
            unrelated_ordinal,
        )
    }

    /// Add one exact unpaired recovered signed-Broadcast to this closed owner.
    fn add_recovered_broadcast_scheduler_fixture_for_test(
        &mut self,
        local_signer: &iroha_crypto::KeyPair,
        marker: u8,
    ) -> Option<u128> {
        self.registry
            .add_recovered_broadcast_scheduler_fixture_for_test(
                &mut self.coordinator,
                &self.verified,
                local_signer,
                marker,
            )
    }

    /// Clear the retained link without exposing either closed carrier.
    fn clear_recovered_broadcast_pair_link_for_test(&mut self, broadcast_ordinal: u128) -> bool {
        self.registry
            .clear_recovered_broadcast_pair_link_for_test(&self.coordinator, broadcast_ordinal)
    }

    /// Corrupt the retained link digest without exposing either closed carrier.
    fn corrupt_recovered_broadcast_pair_link_for_test(&mut self, broadcast_ordinal: u128) -> bool {
        self.registry
            .corrupt_recovered_broadcast_pair_link_for_test(&self.coordinator, broadcast_ordinal)
    }

    /// Snapshot only copyable/cloneable scheduler state and pair classification.
    fn recovered_broadcast_scheduler_state_for_test(
        &self,
        broadcast_ordinal: u128,
    ) -> RecoveredBroadcastSchedulerStateForTest {
        RecoveredBroadcastSchedulerStateForTest {
            records: self.coordinator.records.clone(),
            key_index: self.coordinator.key_index.clone(),
            owner_index: self.coordinator.owner_index.clone(),
            ready_index: self.coordinator.ready_index.clone(),
            active_lease: self.coordinator.active_lease.clone(),
            high_water: self.coordinator.high_water,
            next_lease: self.coordinator.next_lease,
            durable_records: self.coordinator.durable_records.clone(),
            capacity_used: self.coordinator.capacity_used.clone(),
            capacity_generation: self.coordinator.capacity_generation.clone(),
            observed_generation: self.coordinator.observed_generation.clone(),
            producer_debts: self.coordinator.producer_debts.clone(),
            fault: self.coordinator.fault,
            declares_pair: self
                .registry
                .recovered_lifecycle_signed_broadcast_declares_next_vote(
                    &self.coordinator,
                    broadcast_ordinal,
                ),
            paired_ordinal: self
                .registry
                .recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(
                    &self.coordinator,
                    broadcast_ordinal,
                ),
        }
    }

    /// Corrupt only one refanned Broadcast's volatile wait source.
    fn corrupt_recovered_broadcast_wait_for_finalization_test(
        &mut self,
        broadcast_ordinal: u128,
    ) -> bool {
        let Some(record) = self.coordinator.records.get_mut(&broadcast_ordinal) else {
            return false;
        };
        let LifecycleState::Waiting(wait) = record.state else {
            return false;
        };
        let mut foreign_digest = super::LifecycleDigest::new([0xE7; 32]);
        if wait.source() == super::WaitSource::Recovery(foreign_digest) {
            foreign_digest = super::LifecycleDigest::new([0xE8; 32]);
        }
        record.state = LifecycleState::Waiting(super::WaitToken::new(
            super::WaitSource::Recovery(foreign_digest),
            wait.observed_generation(),
        ));
        true
    }

    /// Seal a generic runtime retransmit without exposing the typed carrier.
    fn recovered_broadcast_runtime_retransmit_for_test(
        &self,
        broadcast_ordinal: u128,
        generation: u64,
        source_ordinal: u128,
    ) -> Option<super::work_registry::PendingLifecycleOutputAdmissionV1> {
        let record = self.coordinator.records.get(&broadcast_ordinal)?;
        let tag = crate::sumeragi::v2_core::EventTag::new(
            record.key.round().height(),
            record.key.round().view(),
            crate::sumeragi::v2_core::Generation::new(generation),
        );
        self.registry
            .registry()
            .recovered_broadcast_runtime_retransmit_for_test(
                &self.coordinator,
                broadcast_ordinal,
                tag,
                source_ordinal,
            )
    }

    /// Admit one ordinary direct timeout Broadcast behind existing Ready work
    /// and return its unchanged generic-settlement owner.
    fn defer_direct_timeout_broadcast_for_test(
        &mut self,
        marker: u8,
    ) -> Option<(
        u128,
        Box<super::work_registry::PendingLifecycleOutputAdmissionV1>,
    )> {
        use crate::sumeragi::v2_runtime::{
            RuntimeEffectOwnership, bind_adapter_effect_batch_ownership,
        };
        use iroha_data_model::block::consensus_v2 as wire;

        let context = self.verified.context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: u64::from(marker),
        };
        let effect = crate::sumeragi::v2::AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutVote(wire::TimeoutVote {
                round,
                highest_prepare_qc: None,
                signer: 0,
                signature: vec![marker],
            }),
        ));
        let source_ordinal = self.coordinator.high_water.checked_add(0x100)?;
        let tag = crate::sumeragi::v2_core::EventTag::new(
            round.height,
            round.view,
            crate::sumeragi::v2_core::Generation::new(u64::from(marker).saturating_add(1)),
        );
        let ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, source_ordinal)],
        )
        .ok()?
        .pop()?;
        let pending =
            super::work_registry::PendingLifecycleOutputAdmissionV1::seal_exact(effect, ownership)
                .ok()?;
        let expected_ordinal = self.coordinator.high_water.checked_add(1)?;
        let super::ProductionLifecycleOutputAdmissionSettlementV1::Deferred(pending) = self
            .settle_lifecycle_output_admission::<()>(pending, |_effect, _ownership| {
                panic!("a later direct output cannot overtake existing Ready work")
            })
        else {
            return None;
        };
        if self.coordinator.high_water != expected_ordinal {
            return None;
        }
        let ReadyLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation) = self
            .registry
            .registry()
            .attest_ready_lifecycle_broadcast_carrier(&self.coordinator, expected_ordinal)
            .ok()?
        else {
            return None;
        };
        attestation
            .matches_ready_record(self.coordinator.records.get(&expected_ordinal)?)
            .then(|| (expected_ordinal, Box::new(pending)))
    }

    /// Project one exact direct Broadcast into the volatile reducer-fence wait
    /// that production scheduling must authenticate before waking it.
    fn park_direct_broadcast_before_fence_for_test(
        &mut self,
        ordinal: u128,
        fence: crate::sumeragi::v2::LifecycleReducerFenceObservationV1,
    ) -> bool {
        let Some(observed_generation) = fence.generation().checked_sub(1) else {
            return false;
        };
        if !matches!(
            self.registry
                .registry()
                .attest_ready_lifecycle_broadcast_carrier(&self.coordinator, ordinal),
            Ok(ReadyLifecycleBroadcastCarrierV1::RetainedDirectOutput(_))
        ) {
            return false;
        }
        let wait = super::WaitToken::new(fence.source(), observed_generation);
        let mut next = self.coordinator.stage_durable_transaction();
        if next
            .observed_generation
            .get(&fence.source())
            .is_some_and(|known| *known > observed_generation)
            || !next.ready_index.remove(&ordinal)
        {
            return false;
        }
        next.observed_generation
            .insert(fence.source(), observed_generation);
        {
            let Some(record) = next.records.get_mut(&ordinal) else {
                return false;
            };
            record.state = LifecycleState::Waiting(wait);
        }
        let Ok(SchedulableLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation)) = self
            .registry
            .registry()
            .attest_schedulable_lifecycle_broadcast_carrier(&next, ordinal, Some(fence))
        else {
            return false;
        };
        if !next
            .records
            .get(&ordinal)
            .is_some_and(|record| attestation.matches_schedulable_record(record))
        {
            return false;
        }
        self.coordinator = next;
        true
    }

    /// Project one arbitrary Ready fixture row onto the shared reducer fence.
    fn park_ready_work_before_fence_for_test(
        &mut self,
        ordinal: u128,
        fence: crate::sumeragi::v2::LifecycleReducerFenceObservationV1,
    ) -> bool {
        let Some(observed_generation) = fence.generation().checked_sub(1) else {
            return false;
        };
        let mut next = self.coordinator.stage_durable_transaction();
        if next
            .observed_generation
            .get(&fence.source())
            .is_some_and(|known| *known > observed_generation)
            || !next.ready_index.remove(&ordinal)
        {
            return false;
        }
        let Some(record) = next.records.get_mut(&ordinal) else {
            return false;
        };
        if !matches!(record.state, LifecycleState::Ready) {
            return false;
        }
        next.observed_generation
            .insert(fence.source(), observed_generation);
        record.state =
            LifecycleState::Waiting(super::WaitToken::new(fence.source(), observed_generation));
        self.coordinator = next;
        true
    }

    /// Classify the exact current Ready census without constructing a reducer
    /// fence; scheduler fixtures contain no prospectively-woken rows.
    pub(in crate::sumeragi) fn exact_ready_completion_classification_for_test(
        &self,
    ) -> ProductionCompletionReadyWorkV1 {
        let exact_ready = self
            .coordinator
            .records
            .iter()
            .filter_map(|(ordinal, record)| {
                (record.state == LifecycleState::Ready).then_some(*ordinal)
            })
            .collect::<BTreeSet<_>>();
        if exact_ready != self.coordinator.ready_index {
            return ProductionCompletionReadyWorkV1::Invalid;
        }
        self.classify_schedulable_completion_work(&exact_ready, None)
    }

    /// Drift only one Ready row's physical digest away from its concrete
    /// carrier while preserving the rest of the census.
    fn corrupt_ready_digest_for_test(&mut self, ordinal: u128) -> bool {
        let Some(record) = self.coordinator.records.get_mut(&ordinal) else {
            return false;
        };
        let Some((&slot, &digest)) = record.physical_slots.first_key_value() else {
            return false;
        };
        let replacement = if digest == super::LifecycleDigest::new([0xFD; 32]) {
            super::LifecycleDigest::new([0xFE; 32])
        } else {
            super::LifecycleDigest::new([0xFD; 32])
        };
        record.physical_slots.insert(slot, replacement);
        true
    }

    /// Remove the deliberately unrelated fixture Sign from the bounded owner.
    fn retire_unrelated_sign_for_finalization_test(&mut self, ordinal: u128) -> bool {
        self.retire_ready_work_for_completion_test(ordinal)
    }

    /// Remove one exact Ready carrier and terminalize only its logical row.
    fn retire_ready_work_for_completion_test(&mut self, ordinal: u128) -> bool {
        let Some(record) = self.coordinator.records.get(&ordinal) else {
            return false;
        };
        let Some((&slot, _)) = record.physical_slots.first_key_value() else {
            return false;
        };
        let Some(address) =
            super::work_registry::ConcreteWorkAddress::new(record.owner, ordinal, slot)
        else {
            return false;
        };
        let mut staged = self.coordinator.stage_durable_transaction();
        if staged
            .finish_terminal(ordinal, super::TerminalOutcome::Cancelled)
            .is_err()
            || self
                .coordinator
                .persist_exact_staged_successor(&staged)
                .is_err()
        {
            return false;
        }
        assert!(
            self.registry
                .registry_for_test_mut()
                .remove_exact_for_test(address),
            "preflighted Ready fixture carrier must remain installed through LedgerV1 fsync"
        );
        self.coordinator = staged;
        true
    }

    /// Reassemble an already-Ready Validate fixture into its storage-owning
    /// production shape without minting a second coordinator or registry.
    ///
    /// The helper attaches a fresh LedgerV1 and empty structural Serve owner;
    /// the exact body-store instance remains available for the normal launch
    /// transfer into the bounded worker.
    #[cfg(test)]
    pub(in crate::sumeragi) fn ready_validate_completion_owner_for_test(
        verified: crate::sumeragi::v2::VerifiedHeightContext,
        mut coordinator: LifecycleCoordinator,
        registry: LifecycleWorkRegistryHolder,
        body_store: crate::sumeragi::v2_body_store::V2BodyStore,
        root: &std::path::Path,
    ) -> (
        Self,
        crate::sumeragi::v2_lifecycle_coordinator::RuntimeLifecycleOrdinalAuthority,
    ) {
        coordinator
            .attach_empty_test_ledger(&root.join("ledger"))
            .expect("attach exact Ready Validate lifecycle ledger");
        let (runtime_ordinal_authority, coordinator_ordinal_authority) =
            super::authority::lifecycle_ordinal_authorities_after_high_watermark(
                coordinator.high_water(),
            );
        coordinator
            .bind_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)
            .expect("bind paired Ready Validate ordinal authority");
        let (payload_store, serve_payloads) = crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            &root.join("serve"),
            verified.context(),
        )
        .expect("open empty Ready Validate Serve payload owner");
        (
            Self {
                verified,
                coordinator,
                registry,
                recovered_lifecycle_outputs: None,
                payload_store,
                serve_payloads,
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: Some(
                    crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
                ),
                timeout_supersession_successor: None,
            },
            runtime_ordinal_authority,
        )
    }

    /// Open one clean production executor before moving this owner's body
    /// store into the matching bounded service worker.
    pub(in crate::sumeragi) fn bind_body_store_to_lifecycle_completion_io_for_test(
        &mut self,
        services: &mut ProductionV2Services,
        runtime: crate::sumeragi::v2_runtime::SerializedV2Runtime,
        output_guard: std::sync::Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
        local_validator: iroha_data_model::block::consensus_v2::ValidatorIndex,
        class_capacity: usize,
    ) -> (
        crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
    ) {
        let replayed_decision = runtime
            .replayed_decision_key()
            .expect("read the clean recovered runtime Decision");
        let recovered_validate_retry_census = self
            .registry
            .project_recovered_durable_validate_retry_census(&self.coordinator, replayed_decision)
            .expect("project the clean recovered Validate retry census");
        self.bind_body_store_to_lifecycle_completion_io_with_validate_retry_census_for_test(
            services,
            runtime,
            output_guard,
            local_validator,
            class_capacity,
            recovered_validate_retry_census,
        )
    }

    /// Open a focused executor with the exact pre-completion Validate census.
    ///
    /// Production keeps one executor alive across Validate completion. Tests
    /// which manufacture a fresh executor after that volatile replacement
    /// must carry the already-authenticated pre-publication census explicitly.
    pub(in crate::sumeragi) fn bind_body_store_to_lifecycle_completion_io_with_validate_retry_census_for_test(
        &mut self,
        services: &mut ProductionV2Services,
        runtime: crate::sumeragi::v2_runtime::SerializedV2Runtime,
        output_guard: std::sync::Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
        local_validator: iroha_data_model::block::consensus_v2::ValidatorIndex,
        class_capacity: usize,
        recovered_validate_retry_census: crate::sumeragi::v2_lifecycle_coordinator::RecoveredDurableValidateRetryCensusV1,
    ) -> (
        crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
    ) {
        let body_store = self
            .body_store
            .take()
            .expect("the startup owner transfers its body store exactly once");
        let identity = body_store.instance_identity();
        let context = self.verified.context().clone();
        let requester = context.roster[0].validator.clone();
        let (mut executor, body_store) =
            crate::sumeragi::v2_effects::V2EffectExecutor::open_with_body_store(
                runtime,
                body_store,
                recovered_validate_retry_census,
                None,
                context.clone(),
                requester,
                Some(local_validator),
                std::sync::Arc::clone(&output_guard),
                crate::sumeragi::v2_effects::EffectQueueConfig::default(),
            )
            .expect("open the clean lifecycle Completion executor");
        for (effect, pending, durable_receipt) in self
            .registry
            .registry()
            .recovered_published_store_retry_markers()
        {
            executor
                .install_recovered_published_lifecycle_store_retry_marker(
                    effect,
                    pending,
                    durable_receipt,
                )
                .expect("restore the exact cold lifecycle Store retry marker");
        }
        for (effect, pending, durable_receipt, lifecycle_ordinal) in self
            .registry
            .registry()
            .recovered_published_validate_retry_markers()
        {
            executor
                .install_recovered_published_lifecycle_validate_retry_marker(
                    effect,
                    pending,
                    durable_receipt,
                    lifecycle_ordinal,
                )
                .expect("restore the exact cold lifecycle Validate retry marker");
        }
        let fixture = crate::sumeragi::v2_worker::tests::install_lifecycle_planner_io_for_local_validator_for_test(
                services,
                context,
                local_validator,
                output_guard,
                body_store,
                identity.clone(),
                class_capacity,
            );
        self.body_store_identity = Some(identity);
        (executor, fixture)
    }

    /// Consume the genuine recovered Apply adapter startup and bind its body
    /// store to one executor used only for live/recovered authority tests.
    pub(in crate::sumeragi) fn bind_recovered_apply_executor_for_lineage_test(
        &mut self,
        services: &mut ProductionV2Services,
        output_guard: std::sync::Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
        recovered_validate_retry_census: crate::sumeragi::v2_lifecycle_coordinator::RecoveredDurableValidateRetryCensusV1,
        class_capacity: usize,
    ) -> (
        crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
    ) {
        let startup = self
            .adapter_startup
            .take()
            .expect("recovered lineage fixture retains its exact adapter startup");
        let lifecycle_ordinals =
            crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
                self.coordinator.high_water(),
            );
        let runtime = startup.into_lifecycle_apply_runtime_for_lineage_test(lifecycle_ordinals);
        self.bind_body_store_to_lifecycle_completion_io_with_validate_retry_census_for_test(
            services,
            runtime,
            output_guard,
            0,
            class_capacity,
            recovered_validate_retry_census,
        )
    }

    /// Exercise the production all-row Completion transaction without a
    /// forgeable runner snapshot.
    pub(in crate::sumeragi) fn dispatch_completion_for_test(
        &mut self,
        services: &mut ProductionV2Services,
        executor: &mut crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        runner_debt: u64,
    ) -> Result<ProductionCompletionDispatchV1, ProductionCompletionDispatchErrorV1> {
        self.dispatch_completion_with_runner_debt(services, executor, runner_debt)
    }

    /// Exercise the production synchronous Ready-Validate-successor corridor.
    pub(in crate::sumeragi) fn dispatch_ready_validate_successor_for_test(
        &mut self,
        services: &mut ProductionV2Services,
        executor: &mut crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        successor: super::ReadyValidateSuccessorV1,
        runner_debt: u64,
    ) -> Result<super::ReadyValidateSuccessorDispatchV1, ProductionCompletionDispatchErrorV1> {
        self.dispatch_ready_validate_successor(services, executor, successor, runner_debt)
    }

    /// Recheck the exact finalization-only registry census without exposing it.
    fn finalization_registry_census_is_exact_for_test(&self) -> bool {
        self.registry
            .registry_for_test()
            .exactly_covers_finalization_work(&self.coordinator)
    }

    /// Build an empty storage-owning production owner for ingress admission tests.
    pub(in crate::sumeragi) fn empty_owner_for_ingress_test(
        verified: crate::sumeragi::v2::VerifiedHeightContext,
        local_signer: &iroha_crypto::KeyPair,
        root: &std::path::Path,
    ) -> Self {
        use super::{CapacityClass, schema::CapacityGeometry};

        let context = super::projection::lifecycle_context(verified.context());
        let mut coordinator = LifecycleCoordinator::new(
            context,
            0,
            CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 8))),
        );
        coordinator
            .attach_empty_test_ledger(&root.join("ledger"))
            .expect("attach the empty ingress lifecycle ledger");
        let body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            root.join("body"),
            verified.context().clone(),
        )
        .expect("open empty ingress owner body store");
        let (payload_store, recovery) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &root.join("serve"),
                verified.context(),
            )
            .expect("open empty ingress owner Serve payload store");
        let serve_payloads = recovery
            .authenticate(&verified, local_signer, &body_store)
            .expect("authenticate empty ingress owner Serve payload census");
        Self {
            verified,
            coordinator,
            registry: LifecycleWorkRegistryHolder::empty(),
            recovered_lifecycle_outputs: None,
            payload_store,
            serve_payloads,
            body_store: Some(body_store),
            body_store_identity: None,
            kura_binding: None,
            apply_service: None,
            adapter_startup: Some(
                crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
            ),
            timeout_supersession_successor: None,
        }
    }

    /// Build one storage-owning production owner around the exact selected
    /// Fetch carrier used by the cross-module planner transaction regression.
    pub(in crate::sumeragi) fn waiting_fetch_for_ingress_test(
        verified: crate::sumeragi::v2::VerifiedHeightContext,
        prepared: &PreparedLifecycleIngressSelector,
        effect: crate::sumeragi::v2::AdapterEffect,
        pending: crate::sumeragi::v2_runtime::PendingRuntimeEffectBinding,
        local_signer: &iroha_crypto::KeyPair,
        root: &std::path::Path,
    ) -> (Self, u128, super::WaitSource) {
        use super::{
            AdmissionDecision, AdmissionRequest, CapacityClass, WaitToken,
            schema::CapacityGeometry,
            work_registry::{ConcreteLifecycleWork, ConcreteWorkAddress},
        };
        let (context, _, _, _, expected_key, expected_root, source) = prepared
            .certified_fetch_ready_authority_for_test()
            .expect("selected Fetch must derive its exact lifecycle authority");
        assert_eq!(
            context,
            super::projection::lifecycle_context(verified.context()),
            "selected Fetch and verified owner must share one context"
        );
        let mut coordinator = LifecycleCoordinator::new(
            context,
            0,
            CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 8))),
        );
        let mut registry = LifecycleWorkRegistryHolder::empty();
        let candidate = super::replay_authority::exact_pending_certified_fetch_candidate_fixture(
            &verified, &effect, &pending,
        )
        .expect("the verified selected Fetch must derive exact replay authority");
        assert_eq!(candidate.key, expected_key);
        assert_eq!(candidate.causal_root, expected_root);
        assert_eq!(candidate.work_class, LifecycleWorkClass::Fetch);
        let replay_authority = candidate.replay_authority.clone();
        let work =
            ConcreteLifecycleWork::from_candidate_for_test(effect, pending, replay_authority)
                .unwrap_or_else(|(error, _, _)| {
                    panic!("the selected Fetch carrier is invalid: {error:?}")
                });
        let work_digest = work.digest();
        let AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal: None,
        } = coordinator.admit(AdmissionRequest::Candidate(candidate))
        else {
            panic!("the exact selected Fetch candidate must enter the coordinator")
        };
        let slot = super::PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .expect("the admitted Fetch owns one exact concrete address");
        registry
            .registry_mut()
            .install(address, work_digest, work)
            .unwrap_or_else(|(error, _)| {
                panic!("the exact selected Fetch must enter the concrete registry: {error:?}")
            });
        let record = coordinator
            .records
            .get_mut(&ordinal)
            .expect("admitted Fetch owns its logical record");
        assert_eq!(record.key, expected_key);
        assert_eq!(record.owner.causal_root(), expected_root);
        assert_eq!(record.work_class, LifecycleWorkClass::Fetch);
        assert_eq!(record.physical_slots.get(&slot), Some(&work_digest));
        assert!(coordinator.ready_index.remove(&ordinal));
        record.state = LifecycleState::Waiting(WaitToken::new(source, 0));
        assert!(coordinator.observed_generation.insert(source, 0).is_none());
        coordinator
            .attach_empty_test_ledger(&root.join("ledger"))
            .expect("attach the exact waiting-Fetch lifecycle ledger");
        let body_store = crate::sumeragi::v2_body_store::V2BodyStore::open(
            root.join("body"),
            verified.context().clone(),
        )
        .expect("open exact owner body store");
        let (payload_store, recovery) =
            crate::sumeragi::v2_certified_serve_payload_store::CertifiedServePayloadStoreV1::open(
                &root.join("serve"),
                verified.context(),
            )
            .expect("open exact owner Serve payload store");
        let serve_payloads = recovery
            .authenticate(&verified, local_signer, &body_store)
            .expect("authenticate exact owner Serve payload census");
        (
            Self {
                verified,
                coordinator,
                registry,
                recovered_lifecycle_outputs: None,
                payload_store,
                serve_payloads,
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: Some(
                    crate::sumeragi::v2::ProductionLifecycleAdapterStartupV1::fixture_for_test(),
                ),
                timeout_supersession_successor: None,
            },
            ordinal,
            source,
        )
    }
    /// Move the owner's exact startup body store into the bounded test worker
    /// while retaining only its comparison seal in the running owner.
    pub(in crate::sumeragi) fn bind_body_store_to_planner_io_for_test(
        &mut self,
        services: &mut ProductionV2Services,
        output_guard: std::sync::Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
        class_capacity: usize,
    ) -> crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture {
        let body_store = self
            .body_store
            .take()
            .expect("the startup owner transfers its body store exactly once");
        let identity = body_store.instance_identity();
        let fixture = crate::sumeragi::v2_worker::tests::install_lifecycle_planner_io_for_test(
            services,
            self.verified.context().clone(),
            output_guard,
            body_store,
            identity.clone(),
            class_capacity,
        );
        self.body_store_identity = Some(identity);
        fixture
    }
    /// Complete one persisted ordinary certified-Fetch response through the
    /// production Phase-B coordinator transaction.
    pub(in crate::sumeragi) fn complete_certified_fetch_for_test(
        &mut self,
        executor: &mut V2EffectExecutor<SerializedV2Runtime>,
        services: &mut ProductionV2Services,
        ingress: &crate::sumeragi::FairV2Ingress,
        completion: crate::sumeragi::v2_worker::PreparedCertifiedFetchBodyPersistenceCompletion,
    ) -> Result<(), super::selector::CertifiedFetchBodyPersistenceCompletionError> {
        self.coordinator.complete_certified_fetch_body_persistence(
            &mut self.registry,
            executor,
            services,
            ingress,
            completion,
        )
    }
    /// Project the exact Fetch wait state without exposing mutable owner parts.
    pub(in crate::sumeragi) fn fetch_wait_projection_for_test(
        &self,
        ordinal: u128,
        source: super::WaitSource,
    ) -> (
        Option<LifecycleState>,
        Option<u64>,
        Option<super::CoordinatorFault>,
        bool,
    ) {
        (
            self.coordinator
                .records
                .get(&ordinal)
                .map(|record| record.state),
            self.coordinator.observed_generation.get(&source).copied(),
            self.coordinator.fault,
            self.coordinator.active_lease.is_some(),
        )
    }
    /// Opaque byte-stable view of the exact concrete registry for mutation checks.
    pub(in crate::sumeragi) fn fetch_registry_snapshot_for_test(&self) -> String {
        format!("{:?}", self.registry.registry_for_test())
    }
    /// Rejoin the sole executor owner, exact external wait, and recovered WAL
    /// registry carrier after the production request publication cut.
    pub(in crate::sumeragi) fn recovered_fetch_dispatch_projection_for_test(
        &mut self,
        executor: &crate::sumeragi::v2_effects::V2EffectExecutor<
            crate::sumeragi::v2_runtime::SerializedV2Runtime,
        >,
        ordinal: u128,
    ) -> Option<(
        super::work_registry::RecoveredDecisionFetchDispatchKeyV1,
        iroha_crypto::HashOf<iroha_data_model::block::consensus_v2::CertifiedBodyRequest>,
        super::WaitToken,
    )> {
        let (key, request_hash) = executor.recovered_decision_fetch_owner_for_test()?;
        if key.lifecycle_ordinal() != ordinal || self.coordinator.active_lease.is_some() {
            return None;
        }
        let wait_source = super::projection::certified_fetch_wait_source(request_hash);
        let record = self.coordinator.records.get(&ordinal)?;
        let LifecycleState::Waiting(wait) = record.state else {
            return None;
        };
        (wait.source() == wait_source
            && self.coordinator.observed_generation.get(&wait_source)
                == Some(&wait.observed_generation())
            && !self.coordinator.ready_index.contains(&ordinal)
            && self
                .registry
                .registry_mut()
                .matches_waiting_dispatched_recovered_decision_fetch(
                    &self.coordinator,
                    key,
                    wait_source,
                )
            && self
                .registry
                .registry_mut()
                .exactly_covers_all_live_work(&self.verified, &self.coordinator))
        .then_some((key, request_hash, wait))
    }
    /// Corrupt or restore only the volatile recovered-Fetch wait-source join.
    pub(in crate::sumeragi) fn replace_recovered_fetch_wait_source_for_test(
        &mut self,
        ordinal: u128,
        replacement: super::WaitSource,
    ) -> Option<super::WaitSource> {
        self.registry
            .registry_mut()
            .replace_recovered_fetch_wait_source_for_test(ordinal, replacement)
    }
}

#[cfg(test)]
mod unified_completion_classifier_tests {
    use super::*;

    #[test]
    fn supported_ready_coexistence_selects_only_a_full_census_transaction() {
        assert_eq!(
            classify_completion_ready_classes(
                &[
                    LifecycleWorkClass::Validate,
                    LifecycleWorkClass::Broadcast,
                    LifecycleWorkClass::SignVote,
                    LifecycleWorkClass::Apply,
                    LifecycleWorkClass::Fetch,
                ],
                false,
                false,
            ),
            ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast
        );
        assert_eq!(
            classify_completion_ready_classes(
                &[
                    LifecycleWorkClass::Broadcast,
                    LifecycleWorkClass::ProducerTurn,
                ],
                false,
                false,
            ),
            ProductionCompletionReadyWorkV1::PassThrough
        );
        assert_eq!(
            classify_completion_ready_classes(
                &[
                    LifecycleWorkClass::Broadcast,
                    LifecycleWorkClass::CertifiedServe,
                ],
                false,
                false,
            ),
            ProductionCompletionReadyWorkV1::PassThrough
        );
        assert_eq!(
            classify_completion_ready_classes(
                &[
                    LifecycleWorkClass::Apply,
                    LifecycleWorkClass::SignProposal,
                    LifecycleWorkClass::Fetch,
                ],
                false,
                false,
            ),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
    }

    #[test]
    fn exact_single_ready_io_classes_use_the_same_composite_dispatcher() {
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::Apply], false, false),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::SignTimeout], false, false),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::Fetch], false, false),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
        assert_eq!(
            classify_completion_ready_classes(&[], false, false),
            ProductionCompletionReadyWorkV1::None
        );
    }

    #[test]
    fn later_retained_direct_broadcast_routes_validate_to_completion_io() {
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::Validate], true, false),
            ProductionCompletionReadyWorkV1::CompletionIo
        );
        assert_eq!(
            classify_completion_ready_classes(&[LifecycleWorkClass::Validate], true, true),
            ProductionCompletionReadyWorkV1::RetainedDirectOutput,
            "an oldest direct output retains its distinct settlement owner"
        );
        assert_eq!(
            classify_completion_ready_classes(&[], true, true),
            ProductionCompletionReadyWorkV1::RetainedDirectOutput
        );
    }
}
