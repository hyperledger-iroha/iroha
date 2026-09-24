// Actual body-store, worker-queue and lifecycle custody across physical retries.
#[cfg(feature = "bls")]
fn physical_validate_retry_preserves_original_owner(
    release_before_poll: bool,
    fill_capacity: bool,
) {
    // All three cases call the same full lifecycle fixture. Its compiled frame
    // includes the launched-service branch even when fill_capacity is false.
    // Use the surrounding lifecycle tests' explicit stack for every invocation.
    let handle = std::thread::Builder::new()
        .name("physical-validate-retry".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            physical_validate_retry_preserves_original_owner_fixture(
                release_before_poll,
                fill_capacity,
            );
        })
        .expect("spawn physical Validate lifecycle fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
fn physical_validate_retry_preserves_original_owner_fixture(
    release_before_poll: bool,
    fill_capacity: bool,
) {
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_apply::V2ApplyError,
        v2_body_store::{BodyValidationBusy, LocalValidationRefusal},
        v2_runner::LifecycleProducerClaimDispositionV1 as Claim,
        v2_worker::{LifecycleCompletionTakeV1, LocalLifecycleValidateRetryV1},
    };
    use std::sync::Arc;
    let marker = 0_u8;
    let (mut fixture, _body_directory, body_store, durable) =
        durable_validate_store_fixture_at_view(marker, 0);
    let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let keys = durable_store_keys(marker);
    let now = std::time::Instant::now();
    let runtime_directory = TempDir::new().unwrap();
    let mut coordinator = ready_durable_validate_coordinator(&[&fixture]);
    let ledger_directory = TempDir::new().unwrap();
    coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .unwrap();
    let (runtime_authority, coordinator_authority) =
        authority::lifecycle_ordinal_authorities_after_high_watermark(coordinator.high_water());
    let ordinals = crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::from_authority(
        runtime_authority,
    );
    coordinator
        .bind_live_lifecycle_ordinal_authority(coordinator_authority)
        .unwrap();
    let (runtime, _, _) = cold_ready_validate_runtime_at_durable(
        &fixture,
        &durable,
        &keys,
        runtime_directory.path(),
        "physical-validate.wal",
        now,
        ordinals,
    );
    let holder = take_dispatch_registry(&mut fixture);
    let payload_directory = TempDir::new().unwrap();
    let (payload_store, serve_payloads) =
        CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            payload_directory.path(),
            fixture.verified.context(),
        )
        .unwrap();
    let mut owner = super::super::ProductionLifecycleOwnerV1 {
        verified: fixture.verified.clone(),
        coordinator,
        registry: holder,
        recovered_lifecycle_outputs: None,
        payload_store,
        serve_payloads,
        body_store: Some(body_store),
        body_store_identity: None,
        kura_binding: None,
        apply_service: None,
        adapter_startup: None,
        owner_open_successor: None,
    };
    let output_guard = ConsensusOutputGuard::isolated();
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let (mut executor, mut worker) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
        &mut services,
        runtime,
        Arc::clone(&output_guard),
        0,
        2,
    );
    let ordinal = fixture.lease.ordinal();
    assert_eq!(
        owner
            .dispatch_completion_for_test(&mut services, &mut executor, 0)
            .unwrap(),
        super::super::ProductionCompletionDispatchV1::ValidateQueued { ordinal }
    );
    let records = owner.coordinator.records.clone();
    let registry = format!("{:?}", owner.registry.registry_for_test());
    let (events, _) = tokio::sync::broadcast::channel(8);
    let queue = Arc::new(crate::queue::Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events,
    ));
    let (wake_tx, wake_rx) = std::sync::mpsc::sync_channel(1);
    queue.set_sumeragi_wake(wake_tx);
    let held = queue.lock_lane_retirement_observer();
    let wait = match queue.try_lock_lane_retirement_observer() {
        Err(wait) => wait,
        Ok(_) => panic!("actual original Queue transition is held"),
    };
    worker.activate_one_lifecycle_validate();
    assert_eq!(
        worker.execute_held_lifecycle_validate_result_fixture(
            Err::<wire::ExecutionCommitment, _>(V2ApplyError::LocalValidation(
                LocalValidationRefusal::PhysicalBusy(BodyValidationBusy::new(
                    "lane_reservation_transition_lock",
                    wait,
                    queue.sumeragi_waker(),
                )),
            )),
            Arc::clone(&output_guard),
        ),
        1
    );
    let completion = match services.take_next_lifecycle_completion().unwrap() {
        LifecycleCompletionTakeV1::Validate(completion) => completion,
        _ => panic!("the original Validate still owns its guarded completion"),
    };
    assert_eq!(
        worker.lifecycle_validate_io_snapshot().completion_pending(),
        1
    );
    assert_eq!(owner.coordinator.records, records);
    assert_eq!(
        format!("{:?}", owner.registry.registry_for_test()),
        registry
    );
    assert!(!output_guard.restart_required());
    let retained = match completion.into_local_or_publication() {
        Err(retained) => retained,
        Ok(_) => panic!("physical dependency cannot authorize publication"),
    };
    let retained = if release_before_poll {
        drop(held);
        retained
    } else {
        let LocalLifecycleValidateRetryV1::Waiting(retained) = retained.retry() else {
            panic!("no retry before the actual physical owner releases");
        };
        // An unrelated Queue release must not authorize this dispatch or notify its runner.
        let (events, _) = tokio::sync::broadcast::channel(8);
        let foreign = crate::queue::Queue::from_config(
            iroha_config::parameters::actual::Queue::default(),
            events,
        );
        drop(foreign.lock_lane_retirement_observer());
        assert!(wake_rx.try_recv().is_err());
        let LocalLifecycleValidateRetryV1::Waiting(retained) = retained.retry() else {
            panic!("foreign physical release cannot resume the original request");
        };
        drop(held);
        wake_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("actual release wakes the original runner");
        retained
    };
    if fill_capacity {
        let queued = worker.fill_validate_retry_capacity_for_test(durable.subject());
        assert!(queued > 0);
        let binding_directory = TempDir::new().expect("same-owner physical retry ingress");
        let validator = fixture.verified.context().roster[0].validator.clone();
        let ingress = super::super::LaunchedProductionLifecycleV1::prepare_ready_local_proposal_sign_ingress_for_test(
            &executor, &binding_directory, &validator,
        );
        let mut launched =
            super::super::LaunchedProductionLifecycleV1::ready_local_proposal_sign_fixture_for_test(
                owner, executor, services, ingress,
            );
        launched.park_local_validate_completion_for_test(retained);
        let mut launched = ReadyLocalProposalSignLaunchedFixtureGuard::new(launched, worker);
        let assert_completion_claim = |launched: &ReadyLocalProposalSignLaunchedFixtureGuard| {
            use super::super::{LifecycleDigest, WaitSource, wait_token_for_test};

            let claim = launched
                .producer_claim_projection()
                .expect("read original physical owner");
            assert_eq!(claim, Claim::AwaitingCompletion);
            assert!(claim.requires_yield());
            assert!(claim.permits_runtime_for_test());
            assert_eq!(claim.fresh_admission_permissions_for_test(), (false, false));
            // A retry selection cannot mint an unrelated scheduling owner. These
            // are observations of the actual parked/queued request, not a replay
            // of completion labels through a second transition model.
            for foreign in [
                Claim::Eligible,
                Claim::AwaitingValidateSuccessor { ordinal },
                Claim::AwaitingValidateFence {
                    ordinal,
                    wait: wait_token_for_test(
                        WaitSource::External(LifecycleDigest::new([7; 32])),
                        1,
                    ),
                },
                Claim::AwaitingLiveApplyQueue {
                    parent_ordinal: ordinal,
                    child_ordinal: ordinal + 1,
                },
                Claim::AwaitingNativeSource,
                Claim::AwaitingApplyCompletion,
                Claim::ApplyTerminalSettled,
                Claim::AwaitingReplayCompletion,
            ] {
                assert_ne!(
                    claim, foreign,
                    "physical retry retains its exact completion owner"
                );
            }
        };
        assert_completion_claim(&launched);
        let drive =
            |launched: &mut ReadyLocalProposalSignLaunchedFixtureGuard| {
                let (selected, after) =
                    super::super::v2_runner::with_lifecycle_current_runner_turn_for_test(
                        fixture.verified.context(),
                        super::super::v2_runner::LifecycleRunnerRankTarget::Completion,
                        |runner| match launched.drive_completion_pre_gate(runner) {
                            super::super::ProductionLifecycleCompletionPreGateV1::Selected(
                                selected,
                            ) => Ok(selected),
                            super::super::ProductionLifecycleCompletionPreGateV1::Ordinary(
                                runner,
                            ) => {
                                let drained = launched.drain_ordinary_completion_head_for_ready_sign_test()
                            .expect("normal single-head drain while the exact Validate stays parked");
                                drop(runner);
                                Err(drained)
                            }
                            super::super::ProductionLifecycleCompletionPreGateV1::Ready(ready) => {
                                drop(ready);
                                panic!("an original physical wait must not mint fresh Ready work");
                            }
                        },
                    );
                assert_eq!(
                    after,
                    super::super::v2_runner::LifecycleRunnerRankTarget::Runtime
                );
                if matches!(
                    &selected,
                    Ok(super::super::ProductionLifecycleCompletionSelectionV1::LifecycleValidateLocalWaiting
                        | super::super::ProductionLifecycleCompletionSelectionV1::LifecycleValidateLocalRequeued)
                        | Err(_)
                ) {
                    assert_completion_claim(launched);
                }
                selected
            };
        assert!(matches!(drive(&mut launched),
            Ok(super::super::ProductionLifecycleCompletionSelectionV1::LifecycleValidateLocalWaiting)));
        let before = launched
            .planner
            .as_ref()
            .expect("original worker")
            .lifecycle_validate_io_snapshot();
        assert!(matches!(drive(&mut launched),
            Ok(super::super::ProductionLifecycleCompletionSelectionV1::LifecycleValidateLocalWaiting)));
        assert_eq!(
            launched
                .planner
                .as_ref()
                .expect("original worker")
                .lifecycle_validate_io_snapshot(),
            before,
            "unchanged physical capacity cannot churn or replace dispatch custody"
        );
        let ordinary = launched
            .planner
            .as_ref()
            .expect("original worker")
            .fill_auxiliary_completion_capacity_for_test();
        assert!(
            ordinary > 0,
            "exercise actual bounded completion-channel saturation"
        );
        for remaining in (0..ordinary).rev() {
            assert!(
                matches!(drive(&mut launched), Err(1)),
                "a parked capacity wait must permit exactly the normal ordinary-head drain"
            );
            let snapshot = launched
                .planner
                .as_ref()
                .expect("original worker")
                .lifecycle_validate_io_snapshot();
            assert_eq!(snapshot.completion_owners(), remaining);
            assert_eq!(snapshot.completion_pending(), 1);
            assert_eq!(snapshot.queued(), 0);
            launched.with_proposal_restart_fixture_for_test(|owner, _, _| {
                assert_eq!(owner.coordinator.records, records);
                assert_eq!(
                    format!("{:?}", owner.registry.registry_for_test()),
                    registry
                );
            });
        }
        assert!(matches!(drive(&mut launched),
            Ok(super::super::ProductionLifecycleCompletionSelectionV1::LifecycleValidateLocalWaiting)),
            "draining ordinary completions is not release of the full command owner");
        assert!(wake_rx.try_recv().is_err());
        assert_eq!(
            launched
                .planner
                .as_ref()
                .expect("original worker")
                .drain_validate_retry_capacity_for_test(),
            queued,
            "the actual command receiver can advance once completion pressure is relieved"
        );
        wake_rx
            .try_recv()
            .expect("actual admission release wakes the original runner");
        assert!(matches!(drive(&mut launched),
            Ok(super::super::ProductionLifecycleCompletionSelectionV1::LifecycleValidateLocalRequeued)));
        let snapshot = launched
            .planner
            .as_ref()
            .expect("original worker")
            .lifecycle_validate_io_snapshot();
        assert_eq!(snapshot.queued(), 1);
        assert_eq!(snapshot.completion_pending(), 0);
        launched.with_proposal_restart_fixture_for_test(|owner, _, _| {
            assert_eq!(
                owner.coordinator.records, records,
                "retry does not mint a logical wake"
            );
            assert_eq!(
                format!("{:?}", owner.registry.registry_for_test()),
                registry
            );
        });
        let worker = launched.planner.as_mut().expect("original worker");
        worker.activate_one_lifecycle_validate();
        assert_eq!(
            worker.execute_held_lifecycle_validate_fixture(commitment, Arc::clone(&output_guard)),
            1,
            "physical refusal left both semantic marker caches empty"
        );
        assert!(matches!(drive(&mut launched),
            Ok(super::super::ProductionLifecycleCompletionSelectionV1::LifecycleValidatePublished { ordinal: published })
                if published == ordinal));
        assert_eq!(
            launched
                .producer_claim_projection()
                .expect("read published successor owner"),
            Claim::AwaitingValidateSuccessor { ordinal },
        );
        assert_eq!(
            launched
                .planner
                .as_ref()
                .expect("original worker")
                .lifecycle_validate_io_snapshot()
                .completion_pending(),
            0
        );
        assert!(!output_guard.restart_required());
        return;
    }
    assert!(matches!(
        retained.retry(),
        LocalLifecycleValidateRetryV1::Requeued
    ));
    assert_eq!(worker.lifecycle_validate_io_snapshot().queued(), 1);
    assert_eq!(
        worker.lifecycle_validate_io_snapshot().completion_pending(),
        0
    );
    assert_eq!(
        owner.coordinator.records, records,
        "physical retry does not mint a logical wake"
    );
    assert_eq!(
        format!("{:?}", owner.registry.registry_for_test()),
        registry
    );
    worker.activate_one_lifecycle_validate();
    assert_eq!(
        worker.execute_held_lifecycle_validate_fixture(commitment, Arc::clone(&output_guard)),
        1,
        "local contention wrote neither a success nor a rejection cache marker"
    );
    let completion = match services.take_next_lifecycle_completion().unwrap() {
        LifecycleCompletionTakeV1::Validate(completion) => completion,
        _ => panic!("same request returns its exact semantic completion"),
    };
    let Ok(completion) = completion.into_local_or_publication() else {
        panic!("successful validation proceeds to its original publication transaction");
    };
    let (dispatch, ack) = completion.into_publication_parts();
    let publication = owner
        .coordinator
        .complete_durable_validate_dispatch(&mut owner.registry, dispatch)
        .unwrap();
    let super::super::DurableValidateCompletionPublication::PublishedValidated(published) =
        publication
    else {
        panic!("exact body validated after the actual lock release");
    };
    assert_eq!(published.lifecycle_ordinal(), ordinal);
    ack.acknowledge_after_publication();
    assert_eq!(
        worker.lifecycle_validate_io_snapshot().completion_pending(),
        0
    );
    assert!(!output_guard.restart_required());
}

#[cfg(feature = "bls")]
#[test]
fn superseded_local_validate_retires_guarded_completion_without_rejection() {
    std::thread::Builder::new()
        .name("superseded-validate-retirement".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use crate::sumeragi::{
                output_guard::ConsensusOutputGuard,
                v2_apply::V2ApplyError,
                v2_body_store::LocalValidationRefusal,
                v2_worker::LifecycleCompletionTakeV1,
            };
            use std::sync::Arc;

            let (mut fixture, _body_directory, body_store, _durable) =
                durable_validate_store_fixture_at_view(0xDA, 0);
            let runtime_directory = TempDir::new().unwrap();
            let ledger_directory = TempDir::new().unwrap();
            let mut coordinator = ready_durable_validate_coordinator(&[&fixture]);
            coordinator
                .attach_empty_test_ledger(ledger_directory.path())
                .unwrap();
            let (runtime_authority, coordinator_authority) =
                authority::lifecycle_ordinal_authorities_after_high_watermark(
                    coordinator.high_water(),
                );
            coordinator
                .bind_live_lifecycle_ordinal_authority(coordinator_authority)
                .unwrap();
            let keys = durable_store_keys(0xDA);
            let (runtime, _, _) = cold_ready_validate_runtime_at_durable(
                &fixture,
                &_durable,
                &keys,
                runtime_directory.path(),
                "superseded-validate.wal",
                std::time::Instant::now(),
                crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::from_authority(
                    runtime_authority,
                ),
            );
            let holder = take_dispatch_registry(&mut fixture);
            let payload_directory = TempDir::new().unwrap();
            let (payload_store, serve_payloads) =
                CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
                    payload_directory.path(),
                    fixture.verified.context(),
                )
                .unwrap();
            let mut owner = super::super::ProductionLifecycleOwnerV1 {
                verified: fixture.verified.clone(),
                coordinator,
                registry: holder,
                recovered_lifecycle_outputs: None,
                payload_store,
                serve_payloads,
                body_store: Some(body_store),
                body_store_identity: None,
                kura_binding: None,
                apply_service: None,
                adapter_startup: None,
                owner_open_successor: None,
            };
            let output_guard = ConsensusOutputGuard::isolated();
            let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
            let (mut executor, mut worker) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
                &mut services,
                runtime,
                Arc::clone(&output_guard),
                0,
                2,
            );
            let ordinal = fixture.lease.ordinal();
            assert_eq!(
                owner
                    .dispatch_completion_for_test(&mut services, &mut executor, 0)
                    .unwrap(),
                super::super::ProductionCompletionDispatchV1::ValidateQueued { ordinal }
            );
            worker.activate_one_lifecycle_validate();
            assert_eq!(
                worker.execute_held_lifecycle_validate_result_fixture(
                    Err::<wire::ExecutionCommitment, _>(V2ApplyError::LocalValidation(
                        LocalValidationRefusal::Superseded,
                    )),
                    Arc::clone(&output_guard),
                ),
                1
            );
            let LifecycleCompletionTakeV1::Validate(completion) =
                services.take_next_lifecycle_completion().unwrap()
            else {
                panic!("original superseded Validate retained its guarded completion");
            };
            let retained = match completion.into_local_or_publication() {
                Err(retained) => retained,
                Ok(_) => panic!("supersession is no body-validation verdict"),
            };
            let binding_directory = TempDir::new().unwrap();
            let validator = fixture.verified.context().roster[0].validator.clone();
            let ingress = super::super::LaunchedProductionLifecycleV1::prepare_ready_local_proposal_sign_ingress_for_test(
                &executor,
                &binding_directory,
                &validator,
            );
            let mut launched = super::super::LaunchedProductionLifecycleV1::ready_local_proposal_sign_fixture_for_test(
                owner, executor, services, ingress,
            );
            launched.park_local_validate_completion_for_test(retained);
            let mut launched = ReadyLocalProposalSignLaunchedFixtureGuard::new(launched, worker);
            let (selected, _) = super::super::v2_runner::with_lifecycle_current_runner_turn_for_test(
                fixture.verified.context(),
                super::super::v2_runner::LifecycleRunnerRankTarget::Completion,
                |runner| match launched.drive_completion_pre_gate(runner) {
                    super::super::ProductionLifecycleCompletionPreGateV1::Selected(selected) => {
                        selected
                    }
                    _ => panic!("superseded Validate must settle before other work"),
                },
            );
            assert!(matches!(
                selected,
                super::super::ProductionLifecycleCompletionSelectionV1::LifecycleValidateLocalSuperseded
            ));
            launched.with_proposal_restart_fixture_for_test(|owner, _, _| {
                assert_eq!(
                    owner.coordinator.records[&ordinal].state,
                    LifecycleState::Terminal(TerminalOutcome::Cancelled)
                );
                assert!(!owner
                    .registry
                    .registry_for_test()
                    .entries
                    .contains_key(&fixture.address));
                assert!(owner
                    .registry
                    .registry_for_test()
                    .exactly_covers_finalization_work(&owner.coordinator));
            });
            assert!(!launched.has_pending_lifecycle_completion_for_test());
            assert_eq!(
                launched
                    .planner
                    .as_ref()
                    .unwrap()
                    .lifecycle_validate_io_snapshot()
                    .completion_pending(),
                0
            );
            assert!(!output_guard.restart_required());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[cfg(feature = "bls")]
#[test]
fn physical_validate_retry_waits_for_original_queue_release() {
    physical_validate_retry_preserves_original_owner(false, false);
}
#[cfg(feature = "bls")]
#[test]
fn physical_validate_retry_observes_release_before_registration() {
    physical_validate_retry_preserves_original_owner(true, false);
}
#[cfg(feature = "bls")]
#[test]
fn physical_validate_retry_retains_dispatch_through_worker_backpressure() {
    physical_validate_retry_preserves_original_owner(false, true);
}

#[cfg(feature = "bls")]
#[test]
fn local_validate_retry_keeps_only_the_original_completion_claim() {
    physical_validate_retry_preserves_original_owner(false, true);
}
