#[cfg(feature = "bls")]
#[test]
fn ready_validate_commit_sign_publishes_one_atomic_live_transaction() {
    assert_ready_validate_vote_sign_live_transaction(true, wire::GlobalPhase::Commit, false);
}

#[cfg(feature = "bls")]
#[test]
fn ready_validate_prepare_sign_uses_typed_dispatch_and_exact_predecessor() {
    assert_ready_validate_vote_sign_live_transaction(true, wire::GlobalPhase::Prepare, false);
}

#[cfg(feature = "bls")]
#[test]
fn certified_commit_supersedes_only_an_authenticated_exact_prepare_sign_completion() {
    assert_ready_validate_vote_sign_live_transaction(true, wire::GlobalPhase::Prepare, true);
}

#[cfg(feature = "bls")]
#[test]
fn ready_validate_commit_sign_rejects_missing_ledger_store_and_fails_closed() {
    assert_ready_validate_vote_sign_live_transaction(false, wire::GlobalPhase::Commit, false);
}

#[cfg(feature = "bls")]
#[test]
fn recovered_wal_validate_cut_detaches_only_validated_completion_and_restores_on_drop() {
    {
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            mut holder,
            lease,
            durable: _,
        } = ready_durable_validate_fixture(0xDC, ReadyDurableValidateFixtureOutcome::Validated);
        let before = format!("{:?}", holder.registry_for_test());
        let prepared = holder
            .registry_for_test_mut()
            .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
            .expect("prepare exact validated recovered-WAL parent");
        let cut = match prepared.into_recovered_wal_validate_registry_cut() {
            Ok(cut) => cut,
            Err(_prepared) => panic!("validated completion must detach into WAL parent cut"),
        };
        assert!(cut.detached_work_is_exact_for_test());
        drop(cut);
        assert_eq!(format!("{:?}", holder.registry_for_test()), before);
    }

    {
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            mut holder,
            lease,
            durable: _,
        } = ready_durable_validate_fixture(0xDD, ReadyDurableValidateFixtureOutcome::Rejected);
        let before = format!("{:?}", holder.registry_for_test());
        let prepared = holder
            .registry_for_test_mut()
            .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
            .expect("prepare exact rejected recovered-WAL parent candidate");
        let prepared = match prepared.into_recovered_wal_validate_registry_cut() {
            Ok(_cut) => panic!("rejected completion cannot become a WAL vote parent"),
            Err(prepared) => prepared,
        };
        drop(prepared);
        assert_eq!(format!("{:?}", holder.registry_for_test()), before);
    }
}

#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn ready_validate_execution_preflight_rejects_foreign_or_malformed_authority() {
    {
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            mut holder,
            mut lease,
            durable: _,
        } = ready_durable_validate_fixture(0xD2, ReadyDurableValidateFixtureOutcome::Validated);
        lease.owner = OwnerId::new(
            super::super::CausalRoot::new(LifecycleDigest::new([0xD2; 32])),
            lease.owner.first_admission_ordinal(),
        );
        assert!(matches!(
            holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified,),
            Err(ReadyDurableValidateExecutionError::Registry(
                RegistryError::Missing
            ))
        ));
    }

    {
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            mut holder,
            mut lease,
            durable: _,
        } = ready_durable_validate_fixture(0xD3, ReadyDurableValidateFixtureOutcome::Validated);
        lease
            .physical_slots
            .insert(fixture.slot, LifecycleDigest::new([0xD3; 32]));
        assert!(matches!(
            holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified,),
            Err(ReadyDurableValidateExecutionError::Registry(
                RegistryError::DigestMismatch
            ))
        ));
    }

    {
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            mut holder,
            mut lease,
            durable: _,
        } = ready_durable_validate_fixture(0xD4, ReadyDurableValidateFixtureOutcome::Rejected);
        lease.stage = super::super::LifecycleStage::new(
            super::super::LifecycleStageKind::StoreBody,
            super::super::PredecessorScope::Independent,
        );
        assert!(matches!(
            holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified,),
            Err(ReadyDurableValidateExecutionError::InvalidLeaseShape)
        ));
    }

    {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xD5);
        assert!(matches!(
            fixture.registry.prepare_ready_durable_validate_execution(
                &fixture.lease,
                fixture.slot,
                &fixture.verified,
            ),
            Err(ReadyDurableValidateExecutionError::WrongWorkKind)
        ));
    }

    {
        let mut exact =
            ready_durable_validate_fixture(0xD6, ReadyDurableValidateFixtureOutcome::Validated);
        let WaitingDurableValidateFixture {
            fixture: deferred_fixture,
            _directory: deferred_directory,
            mut store,
            durable,
            coordinator: _,
            holder: _,
            dispatch,
        } = waiting_durable_validate_fixture(0xD7);
        let reference = detached_validation_merge_reference(&durable);
        let deferred = dispatch
            .execute(&mut store, |_| {
                Err::<wire::ExecutionCommitment, _>(DetachedValidationError::MissingMergeSidecar(
                    reference,
                ))
            })
            .expect("execute foreign deferred outcome");
        let ExecutedDurableValidateDispatch {
            executed: ExecutedDurableValidateExecution { outcome, .. },
            ..
        } = deferred;
        let work = exact
            .holder
            .registry_for_test_mut()
            .entries
            .get_mut(&exact.fixture.address)
            .expect("exact fixture retains Ready carrier");
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &mut work.kind
        else {
            unreachable!("exact fixture retains Ready completion")
        };
        completion.outcome = outcome;
        let _keep_foreign_files = deferred_directory;
        assert_ne!(deferred_fixture.address, exact.fixture.address);
        assert!(matches!(
            exact
                .holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(
                    &exact.lease,
                    exact.fixture.slot,
                    &exact.fixture.verified,
                ),
            Err(ReadyDurableValidateExecutionError::Registry(
                RegistryError::CorruptWork
            ))
        ));
    }

    {
        let mut first =
            ready_durable_validate_fixture(0xD8, ReadyDurableValidateFixtureOutcome::Validated);
        let mut foreign =
            ready_durable_validate_fixture(0xD9, ReadyDurableValidateFixtureOutcome::Rejected);
        let first_work = first
            .holder
            .registry_for_test_mut()
            .entries
            .get_mut(&first.fixture.address)
            .expect("first fixture retains Ready carrier");
        let foreign_work = foreign
            .holder
            .registry_for_test_mut()
            .entries
            .get_mut(&foreign.fixture.address)
            .expect("foreign fixture retains Ready carrier");
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(first_completion) =
            &mut first_work.kind
        else {
            unreachable!("first fixture retains Ready completion")
        };
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(foreign_completion) =
            &mut foreign_work.kind
        else {
            unreachable!("foreign fixture retains Ready completion")
        };
        core::mem::swap(
            &mut first_completion.outcome,
            &mut foreign_completion.outcome,
        );
        assert!(matches!(
            first
                .holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(
                    &first.lease,
                    first.fixture.slot,
                    &first.fixture.verified,
                ),
            Err(ReadyDurableValidateExecutionError::Registry(
                RegistryError::CorruptWork
            ))
        ));
    }

    {
        let mut exact =
            ready_durable_validate_fixture(0xDE, ReadyDurableValidateFixtureOutcome::Rejected);
        let foreign = durable_validate_fixture(0xDF);
        let before = format!("{:?}", exact.holder.registry_for_test());
        assert!(matches!(
            exact
                .holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(
                    &exact.lease,
                    exact.fixture.slot,
                    &foreign.verified,
                ),
            Err(ReadyDurableValidateExecutionError::Projection(_))
        ));
        assert_eq!(format!("{:?}", exact.holder.registry_for_test()), before);
    }
}

#[cfg(feature = "bls")]
#[test]
fn rejected_completion_digest_ignores_diagnostic_display_text() {
    let first = waiting_durable_validate_fixture(0xCE);
    let second = waiting_durable_validate_fixture(0xCE);
    let WaitingDurableValidateFixture {
        fixture: first_fixture,
        _directory: first_directory,
        store: mut first_store,
        durable: first_durable,
        coordinator: _first_coordinator,
        holder: _first_holder,
        dispatch: first_dispatch,
    } = first;
    let WaitingDurableValidateFixture {
        fixture: second_fixture,
        _directory: second_directory,
        store: mut second_store,
        durable: second_durable,
        coordinator: _second_coordinator,
        holder: _second_holder,
        dispatch: second_dispatch,
    } = second;
    assert_eq!(first_fixture.address, second_fixture.address);
    assert_eq!(first_durable, second_durable);
    let first_executed = first_dispatch
        .execute(&mut first_store, |_| {
            Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                "diagnostic wording alpha",
            ))
        })
        .expect("execute first deterministic rejection");
    let second_executed = second_dispatch
        .execute(&mut second_store, |_| {
            Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                "diagnostic wording beta",
            ))
        })
        .expect("execute second deterministic rejection");
    assert_ne!(
        first_executed.outcome().rejection_reason(),
        second_executed.outcome().rejection_reason()
    );
    assert_eq!(
        first_executed.outcome().rejection_identity(),
        Some(&BodyValidationRejectionIdentity::Rejected)
    );
    assert_eq!(
        first_executed.outcome().rejection_identity(),
        second_executed.outcome().rejection_identity()
    );
    let incumbent_digest = first_fixture.lease.physical_slots()[&first_fixture.slot];
    let first_digest = durable_validate_completion_digest(
        incumbent_digest,
        first_fixture.expected_manifest_hash,
        first_executed.outcome(),
    )
    .expect("first rejection derives one replacement digest");
    let second_digest = durable_validate_completion_digest(
        incumbent_digest,
        second_fixture.expected_manifest_hash,
        second_executed.outcome(),
    )
    .expect("second rejection derives one replacement digest");
    assert_ne!(first_digest, incumbent_digest);
    assert_eq!(first_digest, second_digest);
    drop(first_directory);
    drop(second_directory);
}

#[cfg(feature = "bls")]
#[test]
fn merge_sidecar_deferral_retains_dispatch_and_leaves_waiting_row_original() {
    let WaitingDurableValidateFixture {
        fixture,
        _directory,
        mut store,
        durable,
        mut coordinator,
        mut holder,
        dispatch,
    } = waiting_durable_validate_fixture(0xC2);
    let reference = detached_validation_merge_reference(&durable);
    let wait = dispatch.wait_token_for_test();
    let coordinator_before = format!("{coordinator:?}");
    let registry_before = format!("{:?}", holder.registry_for_test());
    let old_digest = fixture.lease.physical_slots()[&fixture.slot];
    let executed = dispatch
        .execute(&mut store, |_| {
            Err::<wire::ExecutionCommitment, _>(DetachedValidationError::MissingMergeSidecar(
                reference.clone(),
            ))
        })
        .expect("execute exact deferred Validate dispatch");

    let publication = coordinator
        .complete_durable_validate_dispatch(&mut holder, executed)
        .expect("retain exact merge-sidecar deferral");
    let DurableValidateCompletionPublication::DeferredMergeSidecar(deferred) = publication else {
        panic!("missing merge sidecar must not publish an executable carrier")
    };
    assert_eq!(deferred.missing_reference(), &reference);
    assert_eq!(deferred.dispatch_for_test().wait_token_for_test(), wait);
    assert_eq!(
        deferred.dispatch_for_test().outcome().durable_body(),
        &durable
    );
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(
        coordinator.records[&fixture.lease.ordinal()].state,
        LifecycleState::Waiting(wait)
    );
    assert_eq!(
        coordinator.records[&fixture.lease.ordinal()].physical_slots[&fixture.slot],
        old_digest
    );
    assert!(!coordinator.ready_index.contains(&fixture.lease.ordinal()));
    assert!(matches!(
        holder.registry_for_test().entries[&fixture.address].kind,
        ConcreteLifecycleWorkKind::DurableValidateBody(_)
    ));
}

#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn validate_completion_precommit_failures_preserve_both_sides_and_dispatch() {
    {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC3);
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let mut executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute stale-digest completion fixture");
        executed.executed.request.incumbent_digest = LifecycleDigest::new([0xC3; 32]);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let dispatch_before = format!("{executed:?}");

        let Err((error, returned)) =
            coordinator.complete_durable_validate_dispatch(&mut holder, executed)
        else {
            panic!("stale incumbent digest must fail before publication")
        };
        assert_eq!(
            error,
            DurableValidateCompletionPublicationError::Registry(
                DurableValidateCompletionConversionError::Execution(
                    DurableValidateExecutionError::Registry(RegistryError::DigestMismatch)
                )
            )
        );
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        assert_eq!(returned.outcome().durable_body(), &durable);
        assert_eq!(returned.executed.request.address, fixture.address);
    }

    {
        let WaitingDurableValidateFixture {
            fixture: _,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC4);
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let mut executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute stale-address completion fixture");
        executed.executed.request.address.slot = PhysicalSlotId::for_capacity(
            CapacityClass::Effect,
            executed.executed.request.address.slot.1.saturating_add(1),
        );
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let dispatch_before = format!("{executed:?}");

        let Err((_, returned)) =
            coordinator.complete_durable_validate_dispatch(&mut holder, executed)
        else {
            panic!("foreign Validate address must fail before publication")
        };
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        assert_eq!(returned.outcome().durable_body(), &durable);
    }

    {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC5);
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute wrong-carrier completion fixture");
        let incumbent = holder
            .registry_for_test_mut()
            .entries
            .remove(&fixture.address)
            .expect("wrong-carrier fixture removes exact Validate incumbent");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = incumbent.kind else {
            unreachable!("wrong-carrier fixture starts with durable Validate")
        };
        let pending =
            ConcreteLifecycleWork::from_inert_fixture_for_test(validate.effect, validate.pending)
                .expect("rebuild pending Validate wrong carrier");
        assert!(
            holder
                .registry_for_test_mut()
                .entries
                .insert(fixture.address, pending)
                .is_none()
        );
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let dispatch_before = format!("{executed:?}");

        let Err((error, returned)) =
            coordinator.complete_durable_validate_dispatch(&mut holder, executed)
        else {
            panic!("wrong concrete carrier must fail before publication")
        };
        assert_eq!(
            error,
            DurableValidateCompletionPublicationError::Registry(
                DurableValidateCompletionConversionError::Execution(
                    DurableValidateExecutionError::WrongWorkKind
                )
            )
        );
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC6);
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute key-mutation completion fixture");
        let old_key = fixture.lease.key();
        let foreign_subject = LifecycleDigest::new([0xC6; 32]);
        let foreign_key = super::super::LifecycleKey::new(
            old_key.context(),
            old_key.round(),
            old_key.proposal_round(),
            Some(foreign_subject),
            LifecyclePhase::Validate,
            old_key.execution_commitment(),
        );
        assert_ne!(foreign_key, old_key);
        assert_eq!(
            coordinator.key_index.remove(&old_key),
            Some(fixture.lease.ordinal())
        );
        coordinator
            .records
            .get_mut(&fixture.lease.ordinal())
            .expect("key-mutation fixture retains target record")
            .key = foreign_key;
        assert!(
            coordinator
                .key_index
                .insert(foreign_key, fixture.lease.ordinal())
                .is_none()
        );
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let dispatch_before = format!("{executed:?}");

        let Err((error, returned)) =
            coordinator.complete_durable_validate_dispatch(&mut holder, executed)
        else {
            panic!("consistent key/index mutation must fail exact async authority")
        };
        assert_eq!(
            error,
            DurableValidateCompletionPublicationError::InvalidWaitingState
        );
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xC7);
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute corrupt-episode completion fixture");
        coordinator
            .records
            .get_mut(&fixture.lease.ordinal())
            .expect("episode corruption fixture retains target record")
            .episode
            .frozen_predecessors
            .insert(fixture.lease.ordinal() + 1000);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let dispatch_before = format!("{executed:?}");

        let Err((error, returned)) =
            coordinator.complete_durable_validate_dispatch(&mut holder, executed)
        else {
            panic!("corrupt independent episode must fail before publication")
        };
        assert_eq!(
            error,
            DurableValidateCompletionPublicationError::InvalidWaitingState
        );
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }
}

#[cfg(feature = "bls")]
#[test]
fn validate_completion_rejects_reverse_index_and_duplicate_record_key_intact() {
    {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xCA);
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute reverse-index completion fixture");
        let key = fixture.lease.key();
        let alias_key = super::super::LifecycleKey::new(
            key.context(),
            key.round(),
            key.proposal_round(),
            key.subject(),
            LifecyclePhase::Apply,
            key.execution_commitment(),
        );
        assert_ne!(alias_key, key);
        assert!(
            coordinator
                .key_index
                .insert(alias_key, fixture.lease.ordinal())
                .is_none()
        );
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let dispatch_before = format!("{executed:?}");

        let Err((error, returned)) =
            coordinator.complete_durable_validate_dispatch(&mut holder, executed)
        else {
            panic!("reverse key-index alias must fail completion preflight")
        };
        assert_eq!(
            error,
            DurableValidateCompletionPublicationError::InvalidWaitingState
        );
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    {
        let WaitingDurableValidateFixture {
            fixture,
            _directory,
            mut store,
            durable,
            mut coordinator,
            mut holder,
            dispatch,
        } = waiting_durable_validate_fixture(0xCB);
        let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
        let executed = dispatch
            .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
            .expect("execute duplicate-key completion fixture");
        let alias_ordinal = fixture.lease.ordinal() + 1000;
        let mut alias = coordinator.records[&fixture.lease.ordinal()].clone();
        alias.ordinal = alias_ordinal;
        alias.state = LifecycleState::Ready;
        assert!(coordinator.records.insert(alias_ordinal, alias).is_none());
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let dispatch_before = format!("{executed:?}");

        let Err((error, returned)) =
            coordinator.complete_durable_validate_dispatch(&mut holder, executed)
        else {
            panic!("duplicate lifecycle record key must fail completion preflight")
        };
        assert_eq!(
            error,
            DurableValidateCompletionPublicationError::InvalidWaitingState
        );
        assert_eq!(format!("{returned:?}"), dispatch_before);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }
}

#[cfg(feature = "bls")]
#[test]
fn validate_completion_guard_restores_incumbent_on_unwind_before_swap() {
    let WaitingDurableValidateFixture {
        fixture: _,
        _directory,
        mut store,
        durable,
        coordinator,
        mut holder,
        dispatch,
    } = waiting_durable_validate_fixture(0xC8);
    let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
    let executed = dispatch
        .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
        .expect("execute unwind completion fixture");
    let coordinator_before = format!("{coordinator:?}");
    let registry_before = format!("{:?}", holder.registry_for_test());
    let prepared = holder
        .registry_for_test_mut()
        .prepare_executed_durable_validate_completion(executed)
        .expect("reattach unwind completion fixture");

    let unwind = catch_unwind(AssertUnwindSafe(move || {
        let _staged = prepared
            .stage_executable_carrier()
            .expect("stage unwind-safe Validate carrier");
        panic!("test-only panic before coordinator swap");
    }));
    assert!(unwind.is_err());
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
}

#[cfg(feature = "bls")]
#[test]
fn duplicate_old_digest_completion_cas_returns_exact_dispatch_intact() {
    let first = waiting_durable_validate_fixture(0xC9);
    let second = waiting_durable_validate_fixture(0xC9);
    let WaitingDurableValidateFixture {
        fixture: first_fixture,
        _directory: first_directory,
        store: mut first_store,
        durable: first_durable,
        coordinator: mut first_coordinator,
        holder: mut first_holder,
        dispatch: first_dispatch,
    } = first;
    let WaitingDurableValidateFixture {
        fixture: second_fixture,
        _directory: second_directory,
        store: mut second_store,
        durable: second_durable,
        coordinator: _second_coordinator,
        holder: _second_holder,
        dispatch: second_dispatch,
    } = second;
    assert_eq!(first_fixture.address, second_fixture.address);
    assert_eq!(first_durable, second_durable);
    let first_commitment =
        ValidatedBodyReceipt::for_test(first_durable.clone()).execution_commitment();
    let second_commitment = ValidatedBodyReceipt::for_test(second_durable).execution_commitment();
    let first_executed = first_dispatch
        .execute(&mut first_store, |_| {
            Ok::<_, DetachedValidationError>(first_commitment)
        })
        .expect("execute first duplicate-CAS fixture");
    let second_executed = second_dispatch
        .execute(&mut second_store, |_| {
            Ok::<_, DetachedValidationError>(second_commitment)
        })
        .expect("execute second duplicate-CAS fixture");
    let mut waiting_again = first_coordinator.clone();
    let _publication = first_coordinator
        .complete_durable_validate_dispatch(&mut first_holder, first_executed)
        .expect("publish first exact completion carrier");
    let coordinator_before = format!("{waiting_again:?}");
    let registry_before = format!("{:?}", first_holder.registry_for_test());
    let dispatch_before = format!("{second_executed:?}");

    let Err((error, returned)) =
        waiting_again.complete_durable_validate_dispatch(&mut first_holder, second_executed)
    else {
        panic!("old-digest completion must not replace an installed completion")
    };
    assert!(matches!(
        error,
        DurableValidateCompletionPublicationError::Registry(
            DurableValidateCompletionConversionError::Execution(
                DurableValidateExecutionError::Registry(RegistryError::DigestMismatch)
                    | DurableValidateExecutionError::WrongWorkKind
            )
        )
    ));
    assert_eq!(format!("{returned:?}"), dispatch_before);
    assert_eq!(format!("{waiting_again:?}"), coordinator_before);
    assert_eq!(
        format!("{:?}", first_holder.registry_for_test()),
        registry_before
    );
    drop(first_directory);
    drop(second_directory);
}
