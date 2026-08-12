#[cfg(feature = "bls")]
#[test]
fn durable_validate_dispatch_moves_claim_to_current_external_wait_and_executes() {
    let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xB0);
    let source = durable_validation_source(&mut fixture);
    let mut coordinator = claimed_durable_validate_coordinator(&fixture);
    coordinator.observed_generation.insert(source, 7);
    let mut holder = take_dispatch_registry(&mut fixture);
    let registry_before = format!("{:?}", holder.registry_for_test());
    let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();

    let dispatch = coordinator
        .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
        .expect("exact claimed Validate becomes one dispatch");
    let wait = dispatch.wait_token_for_test();
    assert_eq!(wait, WaitToken::new(source, 7));
    assert!(coordinator.active_lease.is_none());
    assert_eq!(
        coordinator.records[&fixture.lease.ordinal()].state,
        LifecycleState::Waiting(wait)
    );
    assert_eq!(coordinator.observed_generation.get(&source), Some(&7));
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);

    let executed = dispatch
        .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
        .expect("execute exact waiting Validate request");
    assert_eq!(executed.wait_token_for_test(), wait);
    assert_eq!(executed.outcome().durable_body(), &durable);
    assert_eq!(
        executed
            .outcome()
            .validated_receipt()
            .map(ValidatedBodyReceipt::execution_commitment),
        Some(commitment)
    );
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
}

#[cfg(feature = "bls")]
#[test]
fn dropping_unexecuted_durable_validate_dispatch_preserves_wait_and_registry() {
    let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB1);
    let mut coordinator = claimed_durable_validate_coordinator(&fixture);
    let mut holder = take_dispatch_registry(&mut fixture);
    let registry_before = format!("{:?}", holder.registry_for_test());

    let dispatch = coordinator
        .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
        .expect("exact claimed Validate becomes one dispatch");
    let wait = dispatch.wait_token_for_test();
    drop(dispatch);

    assert!(coordinator.active_lease.is_none());
    assert_eq!(
        coordinator.records[&fixture.lease.ordinal()].state,
        LifecycleState::Waiting(wait)
    );
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
}

#[cfg(feature = "bls")]
#[test]
fn committed_durable_validate_dispatch_cannot_mint_a_second_request() {
    let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB2);
    let mut coordinator = claimed_durable_validate_coordinator(&fixture);
    let mut holder = take_dispatch_registry(&mut fixture);
    let registry_before = format!("{:?}", holder.registry_for_test());
    let lease = fixture.lease.clone();

    let dispatch = coordinator
        .begin_durable_validate_dispatch(&mut holder, lease.clone(), &fixture.verified)
        .expect("first exact claimed Validate mints one dispatch");
    let coordinator_after = format!("{coordinator:?}");
    let Err((error, returned_lease)) =
        coordinator.begin_durable_validate_dispatch(&mut holder, lease.clone(), &fixture.verified)
    else {
        panic!("waiting Validate must not mint a second dispatch")
    };
    assert_eq!(error, DurableValidateDispatchError::StaleLease);
    assert_eq!(returned_lease, lease);
    assert_eq!(format!("{coordinator:?}"), coordinator_after);
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    drop(dispatch);
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_store_error_returns_the_exact_dispatch() {
    let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xB3);
    let mut coordinator = claimed_durable_validate_coordinator(&fixture);
    let mut holder = take_dispatch_registry(&mut fixture);
    let dispatch = coordinator
        .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
        .expect("exact claimed Validate becomes one dispatch");
    let wait = dispatch.wait_token_for_test();
    let empty_directory = TempDir::new().expect("temporary empty Validate body store");
    let mut empty_store =
        V2BodyStore::open(empty_directory.path(), fixture.verified.context().clone())
            .expect("open empty Validate body store");

    let (error, dispatch) = dispatch
        .execute(&mut empty_store, |_| {
            Ok::<_, DetachedValidationError>(
                ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment(),
            )
        })
        .expect_err("missing durable catalog row returns the dispatch");
    assert!(matches!(error, V2BodyStoreError::ReceiptMismatch));
    assert_eq!(dispatch.wait_token_for_test(), wait);

    let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
    let executed = dispatch
        .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
        .expect("returned dispatch remains executable against its exact store");
    assert_eq!(executed.wait_token_for_test(), wait);
    assert_eq!(
        executed
            .outcome()
            .validated_receipt()
            .map(ValidatedBodyReceipt::execution_commitment),
        Some(commitment)
    );
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_dispatch_rejects_stale_foreign_and_wrong_kind_without_mutation() {
    {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB4);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let mut stale = fixture.lease.clone();
        stale.id = super::super::LeaseId(stale.id().0 + 1);

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            stale.clone(),
            &fixture.verified,
        ) else {
            panic!("stale lease must not mint a Validate dispatch")
        };
        assert_eq!(error, DurableValidateDispatchError::StaleLease);
        assert_eq!(returned, stale);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    {
        let (fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB5);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let mut holder = LifecycleWorkRegistryHolder::empty();
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("foreign empty registry must not mint a Validate dispatch")
        };
        assert_eq!(
            error,
            DurableValidateDispatchError::Registry(DurableValidateExecutionError::Registry(
                RegistryError::Missing
            ))
        );
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB6);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let incumbent = fixture
            .registry
            .entries
            .remove(&fixture.address)
            .expect("wrong-kind fixture removes its closed Validate");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = incumbent.kind else {
            unreachable!("wrong-kind fixture starts with one closed Validate")
        };
        let DurableValidateBody {
            effect, pending, ..
        } = validate;
        let pending = ConcreteLifecycleWork::from_exact(effect, pending)
            .expect("rebuild exact pending Validate work");
        assert!(
            fixture
                .registry
                .entries
                .insert(fixture.address, pending)
                .is_none()
        );
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("pending Validate row must not cross the closed-carrier dispatch")
        };
        assert_eq!(
            error,
            DurableValidateDispatchError::Registry(DurableValidateExecutionError::WrongWorkKind)
        );
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_dispatch_rejects_max_generation_and_wait_source_alias() {
    {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB7);
        let source = durable_validation_source(&mut fixture);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        coordinator.observed_generation.insert(source, u64::MAX);
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("maximum wait generation must not mint a Validate dispatch")
        };
        assert_eq!(error, DurableValidateDispatchError::WaitGenerationExhausted);
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB8);
        let source = durable_validation_source(&mut fixture);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let alias_ordinal = fixture.lease.ordinal() + 1000;
        let mut alias = coordinator.records[&fixture.lease.ordinal()].clone();
        alias.ordinal = alias_ordinal;
        alias.state = LifecycleState::Waiting(WaitToken::new(source, 0));
        assert!(coordinator.records.insert(alias_ordinal, alias).is_none());
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("aliased external wait source must not mint a Validate dispatch")
        };
        assert_eq!(error, DurableValidateDispatchError::AliasedWaitSource);
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_dispatch_rejects_reverse_identity_aliases() {
    {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xB9);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let alias_key = fixture.lease.ordinal() + 1000;
        let alias = coordinator.records[&fixture.lease.ordinal()].clone();
        assert!(coordinator.records.insert(alias_key, alias).is_none());
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("reverse internal-ordinal alias must fail before detachment")
        };
        assert_eq!(error, DurableValidateDispatchError::StaleLease);
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBA);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let key = fixture.lease.key();
        let alias_key = super::super::LifecycleKey::new(
            key.context(),
            key.round(),
            key.proposal_round(),
            key.subject(),
            super::super::LifecyclePhase::Apply,
            key.execution_commitment(),
        );
        assert_ne!(alias_key, key);
        assert!(
            coordinator
                .key_index
                .insert(alias_key, fixture.lease.ordinal())
                .is_none()
        );
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("reverse key-index alias must fail before detachment")
        };
        assert_eq!(error, DurableValidateDispatchError::StaleLease);
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBB);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let alias_root = super::super::CausalRoot::new(LifecycleDigest::new([0xBB; 32]));
        assert_ne!(alias_root, fixture.lease.owner().causal_root());
        assert!(
            coordinator
                .owner_index
                .insert(alias_root, fixture.lease.owner())
                .is_none()
        );
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("reverse owner-index alias must fail before detachment")
        };
        assert_eq!(error, DurableValidateDispatchError::StaleLease);
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }

    {
        let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBC);
        let mut coordinator = claimed_durable_validate_coordinator(&fixture);
        let alias_ordinal = fixture.lease.ordinal() + 1000;
        let mut alias = coordinator.records[&fixture.lease.ordinal()].clone();
        alias.ordinal = alias_ordinal;
        alias.state = LifecycleState::Ready;
        assert!(coordinator.records.insert(alias_ordinal, alias).is_none());
        let mut holder = take_dispatch_registry(&mut fixture);
        let coordinator_before = format!("{coordinator:?}");
        let registry_before = format!("{:?}", holder.registry_for_test());
        let lease = fixture.lease.clone();

        let Err((error, returned)) = coordinator.begin_durable_validate_dispatch(
            &mut holder,
            lease.clone(),
            &fixture.verified,
        ) else {
            panic!("duplicate lifecycle record key must fail before detachment")
        };
        assert_eq!(error, DurableValidateDispatchError::StaleLease);
        assert_eq!(returned, lease);
        assert_eq!(format!("{coordinator:?}"), coordinator_before);
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    }
}

#[cfg(feature = "bls")]
#[test]
fn validated_completion_atomically_publishes_exact_ready_carrier() {
    let WaitingDurableValidateFixture {
        fixture,
        _directory,
        mut store,
        durable,
        mut coordinator,
        mut holder,
        dispatch,
    } = waiting_durable_validate_fixture(0xC0);
    let ordinal = fixture.lease.ordinal();
    let old_digest = fixture.lease.physical_slots()[&fixture.slot];
    let wait = dispatch.wait_token_for_test();
    let before_record = coordinator.records[&ordinal].clone();
    let before_records = coordinator.records.len();
    let before_high_water = coordinator.high_water;
    let before_capacity = coordinator.capacity_used.clone();
    let before_capacity_generation = coordinator.capacity_generation.clone();
    let before_durable = coordinator.durable_records.clone();
    let before_debts = coordinator.producer_debts.clone();
    let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let executed = dispatch
        .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
        .expect("execute exact successful Validate dispatch");

    let publication = coordinator
        .complete_durable_validate_dispatch(&mut holder, executed)
        .expect("publish exact successful Validate completion");
    let DurableValidateCompletionPublication::PublishedValidated(published) = publication else {
        panic!("successful body validation publishes the validated carrier")
    };
    let location = published.location_for_test();
    assert_eq!(location.address, fixture.address);
    assert_eq!(location.incumbent_digest, old_digest);
    assert_ne!(location.replacement_digest, old_digest);

    let record = &coordinator.records[&ordinal];
    assert_eq!(record.owner, fixture.lease.owner());
    assert_eq!(record.ordinal, ordinal);
    assert_eq!(record.state, LifecycleState::Ready);
    assert_eq!(record.physical_slots.len(), 1);
    assert_eq!(
        record.physical_slots.get(&fixture.slot),
        Some(&location.replacement_digest)
    );
    assert_eq!(record.episode, before_record.episode);
    assert_eq!(coordinator.records.len(), before_records);
    assert_eq!(coordinator.high_water, before_high_water);
    assert_eq!(coordinator.capacity_used, before_capacity);
    assert_eq!(coordinator.capacity_generation, before_capacity_generation);
    assert_eq!(coordinator.durable_records, before_durable);
    assert_eq!(coordinator.producer_debts, before_debts);
    assert_eq!(coordinator.observed_generation[&wait.source()], 1);
    assert!(coordinator.ready_index.contains(&ordinal));
    assert!(coordinator.active_lease.is_none());
    assert!(coordinator.ledger_store.is_none());

    assert_eq!(holder.registry_for_test().entries.len(), 1);
    let installed = &holder.registry_for_test().entries[&fixture.address];
    assert_eq!(installed.digest, location.replacement_digest);
    assert!(installed.validates_at(fixture.address));
    let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &installed.kind else {
        panic!("successful validation installs one closed completion carrier")
    };
    assert_eq!(completion.address, fixture.address);
    assert_eq!(completion.incumbent_digest, old_digest);
    assert!(completion.incumbent.validates(old_digest));
    assert_eq!(completion.outcome.durable_body(), &durable);
    assert_eq!(
        completion
            .outcome
            .validated_receipt()
            .map(ValidatedBodyReceipt::execution_commitment),
        Some(commitment)
    );
}

#[cfg(feature = "bls")]
#[test]
fn validated_completion_rejects_conflicting_inherited_commitment_intact() {
    let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xCD);
    let yielded_commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let inherited_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"inherited commitment parent"),
        Hash::new(b"inherited commitment post"),
        Hash::new(b"inherited commitment writes"),
        1,
        Hash::new(b"inherited commitment wire"),
    );
    assert!(inherited_commitment.validate().is_ok());
    assert_ne!(inherited_commitment, yielded_commitment);
    seal_validate_fixture_commitment(&mut fixture, inherited_commitment);
    let mut coordinator = claimed_durable_validate_coordinator(&fixture);
    let mut holder = take_dispatch_registry(&mut fixture);
    let dispatch = coordinator
        .begin_durable_validate_dispatch(&mut holder, fixture.lease.clone(), &fixture.verified)
        .expect("commitment-authorized Validate becomes one waiting dispatch");
    let executed = dispatch
        .execute(&mut store, |_| {
            Ok::<_, DetachedValidationError>(yielded_commitment)
        })
        .expect("body store retains the conflicting deterministic success");
    let coordinator_before = format!("{coordinator:?}");
    let registry_before = format!("{:?}", holder.registry_for_test());
    let dispatch_before = format!("{executed:?}");

    let Err((error, returned)) =
        coordinator.complete_durable_validate_dispatch(&mut holder, executed)
    else {
        panic!("inherited commitment must constrain asynchronous validation success")
    };
    assert_eq!(
        error,
        DurableValidateCompletionPublicationError::Registry(
            DurableValidateCompletionConversionError::Execution(
                DurableValidateExecutionError::ConflictingValidationCommitment
            )
        )
    );
    assert_eq!(format!("{returned:?}"), dispatch_before);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(
        returned
            .outcome()
            .validated_receipt()
            .map(ValidatedBodyReceipt::execution_commitment),
        Some(yielded_commitment)
    );
    assert_eq!(
        returned
            .executed
            .request
            .candidate_statement
            .and_then(RuntimeCandidateSemanticStatement::execution_commitment),
        Some(inherited_commitment)
    );
}

#[cfg(feature = "bls")]
#[test]
fn rejected_completion_atomically_publishes_exact_ready_carrier() {
    let WaitingDurableValidateFixture {
        fixture,
        _directory,
        mut store,
        durable,
        mut coordinator,
        mut holder,
        dispatch,
    } = waiting_durable_validate_fixture(0xC1);
    let ordinal = fixture.lease.ordinal();
    let old_digest = fixture.lease.physical_slots()[&fixture.slot];
    let wait = dispatch.wait_token_for_test();
    let before_record = coordinator.records[&ordinal].clone();
    let before_records = coordinator.records.len();
    let before_high_water = coordinator.high_water;
    let before_capacity = coordinator.capacity_used.clone();
    let before_capacity_generation = coordinator.capacity_generation.clone();
    let before_durable = coordinator.durable_records.clone();
    let before_debts = coordinator.producer_debts.clone();
    let executed = dispatch
        .execute(&mut store, |_| {
            Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                "deterministic rejected completion",
            ))
        })
        .expect("execute exact rejected Validate dispatch");

    let publication = coordinator
        .complete_durable_validate_dispatch(&mut holder, executed)
        .expect("publish exact rejected Validate completion");
    let DurableValidateCompletionPublication::PublishedRejected(published) = publication else {
        panic!("deterministic rejection publishes the rejected carrier")
    };
    let location = published.location_for_test();
    assert_eq!(location.address, fixture.address);
    assert_eq!(location.incumbent_digest, old_digest);
    assert_ne!(location.replacement_digest, old_digest);

    let record = &coordinator.records[&ordinal];
    assert_eq!(record.owner, fixture.lease.owner());
    assert_eq!(record.ordinal, ordinal);
    assert_eq!(record.state, LifecycleState::Ready);
    assert_eq!(record.physical_slots.len(), 1);
    assert_eq!(
        record.physical_slots.get(&fixture.slot),
        Some(&location.replacement_digest)
    );
    assert_eq!(record.episode, before_record.episode);
    assert_eq!(coordinator.records.len(), before_records);
    assert_eq!(coordinator.high_water, before_high_water);
    assert_eq!(coordinator.capacity_used, before_capacity);
    assert_eq!(coordinator.capacity_generation, before_capacity_generation);
    assert_eq!(coordinator.durable_records, before_durable);
    assert_eq!(coordinator.producer_debts, before_debts);
    assert_eq!(coordinator.observed_generation[&wait.source()], 1);
    assert!(coordinator.ready_index.contains(&ordinal));
    assert!(coordinator.ledger_store.is_none());

    assert_eq!(holder.registry_for_test().entries.len(), 1);
    let installed = &holder.registry_for_test().entries[&fixture.address];
    assert_eq!(installed.digest, location.replacement_digest);
    assert!(installed.validates_at(fixture.address));
    let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &installed.kind else {
        panic!("rejection installs one closed completion carrier")
    };
    assert_eq!(completion.incumbent_digest, old_digest);
    assert!(completion.incumbent.validates(old_digest));
    assert_eq!(completion.outcome.durable_body(), &durable);
    assert_eq!(
        completion.outcome.rejection_reason(),
        Some("deterministic rejected completion")
    );
    assert!(completion.outcome.validated_receipt().is_none());
}

#[cfg(feature = "bls")]
#[test]
fn ready_validate_execution_preflight_binds_closed_outcomes_and_is_drop_inert() {
    {
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            mut holder,
            lease,
            durable: _,
        } = ready_durable_validate_fixture(0xD0, ReadyDurableValidateFixtureOutcome::Validated);
        let before = format!("{:?}", holder.registry_for_test());
        let prepared = holder
            .registry_for_test_mut()
            .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
            .expect("prepare exact validated Ready carrier");
        assert_eq!(
            prepared.outcome_kind(),
            ReadyDurableValidateOutcomeKind::Validated
        );
        assert!(prepared.validated_authority().is_some());
        assert!(prepared.rejected_authority().is_none());
        drop(prepared);
        assert_eq!(format!("{:?}", holder.registry_for_test()), before);
    }

    {
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            mut holder,
            lease,
            durable: _,
        } = ready_durable_validate_fixture(0xD1, ReadyDurableValidateFixtureOutcome::Rejected);
        let before = format!("{:?}", holder.registry_for_test());
        let prepared = holder
            .registry_for_test_mut()
            .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
            .expect("prepare exact rejected Ready carrier");
        assert_eq!(
            prepared.outcome_kind(),
            ReadyDurableValidateOutcomeKind::Rejected
        );
        assert!(prepared.rejected_authority().is_some());
        assert!(prepared.validated_authority().is_none());
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
        let pending = ConcreteLifecycleWork::from_exact(validate.effect, validate.pending)
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

#[cfg(feature = "bls")]
#[test]
fn durable_store_prepare_seal_and_drop_preserve_the_closed_row() {
    let DurableStoreFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        effect,
        expected_manifest_hash,
    } = durable_store_fixture(0x41);
    let AdapterEffect::StoreBody {
        tag,
        round,
        subject,
    } = effect.clone()
    else {
        unreachable!("durable Store fixture retains its Store effect")
    };
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    };
    let before = format!("{registry:?}");

    let prepared = registry
        .prepare_durable_store_execution(&lease, slot, &verified)
        .expect("prepare exact durable Store execution");
    assert_eq!(prepared.adapter_preview_inputs(), (tag, round, subject));
    assert_eq!(prepared.durable_body_receipt().round(), round);
    assert_eq!(prepared.durable_body_receipt().subject(), subject);
    assert_eq!(
        prepared.durable_body_receipt().manifest_hash(),
        expected_manifest_hash
    );
    assert_eq!(prepared.expected_manifest_hash(), expected_manifest_hash);
    let sealed = prepared
        .seal_validate_successor(&validate_effect)
        .expect("seal exact ordinal-free Validate successor");
    assert_eq!(sealed._store_address, address);
    assert_eq!(sealed._validate_effect, validate_effect);
    assert!(
        sealed
            ._validate_pending
            .exactly_binds_adapter_effect(&sealed._validate_effect)
    );
    assert_eq!(
        sealed._validate_digest,
        digest_from_hash(sealed._validate_pending.exact_effect_identity())
    );
    assert_eq!(
        super::super::CausalRoot::new(digest_from_hash(
            sealed._validate_pending.causal_lifecycle_key()
        )),
        lease.owner().causal_root()
    );
    assert_eq!(
        sealed._durable_body.manifest_hash(),
        sealed._expected_manifest_hash
    );
    drop(sealed);

    assert_eq!(format!("{registry:?}"), before);
    assert!(registry.exactly_contains(address, &effect));
    assert_eq!(
        registry.borrow_for_lease(&lease, slot),
        Err(RegistryError::WrongWorkKind)
    );
    assert!(matches!(
        registry.take_for_lease(&lease, slot),
        Err(RegistryError::WrongWorkKind)
    ));
    assert_eq!(format!("{registry:?}"), before);

    let mut disposable = durable_store_fixture(0x42);
    let closed = disposable
        .registry
        .entries
        .remove(&disposable.address)
        .expect("remove disposable closed Store only for into-pair rejection test");
    let unwind = catch_unwind(AssertUnwindSafe(|| closed.into_pair()));
    assert!(unwind.is_err(), "closed Store must not expose a raw pair");
}

#[cfg(feature = "bls")]
#[test]
fn durable_store_prepare_rejects_wrong_lease_projection_and_context_without_mutation() {
    let DurableStoreFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        effect,
        ..
    } = durable_store_fixture(0x51);
    let before = format!("{registry:?}");

    let mut wrong_class = lease.clone();
    wrong_class.work_class = LifecycleWorkClass::Fetch;
    assert!(matches!(
        registry.prepare_durable_store_execution(&wrong_class, slot, &verified),
        Err(DurableStoreExecutionError::InvalidLeaseShape)
    ));

    let other_slot = PhysicalSlotId::for_capacity(lease.work_class().capacity_class(), 1);
    assert!(matches!(
        registry.prepare_durable_store_execution(&lease, other_slot, &verified),
        Err(DurableStoreExecutionError::InvalidLeaseShape)
    ));

    let mut wrong_digest = lease.clone();
    wrong_digest
        .physical_slots
        .insert(slot, LifecycleDigest::new([0xD1; 32]));
    assert!(matches!(
        registry.prepare_durable_store_execution(&wrong_digest, slot, &verified),
        Err(DurableStoreExecutionError::Registry(
            RegistryError::DigestMismatch
        ))
    ));

    let mut stale = lease.clone();
    stale.ordinal = stale.ordinal.saturating_add(1);
    assert!(matches!(
        registry.prepare_durable_store_execution(&stale, slot, &verified),
        Err(DurableStoreExecutionError::Registry(RegistryError::Missing))
    ));

    let exact_key = lease.key();
    let mut wrong_key = lease.clone();
    wrong_key.key = super::super::LifecycleKey::new(
        exact_key.context(),
        exact_key.round(),
        exact_key.proposal_round(),
        Some(LifecycleDigest::new([0xE1; 32])),
        exact_key.phase(),
        exact_key.execution_commitment(),
    );
    assert!(matches!(
        registry.prepare_durable_store_execution(&wrong_key, slot, &verified),
        Err(DurableStoreExecutionError::InvalidProjection)
    ));

    let (foreign_verified, _) = verified_store_context(0x52);
    assert!(matches!(
        registry.prepare_durable_store_execution(&lease, slot, &foreign_verified),
        Err(DurableStoreExecutionError::Projection(
            AdapterEffectAdmissionError::ForeignContext
        ))
    ));

    assert_eq!(format!("{registry:?}"), before);
    assert!(registry.exactly_contains(address, &effect));
}

#[cfg(feature = "bls")]
#[test]
fn durable_store_seal_rejects_wrong_kind_or_tag_and_wrong_row_kind() {
    let DurableStoreFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        effect,
        ..
    } = durable_store_fixture(0x61);
    let before = format!("{registry:?}");

    let prepared = registry
        .prepare_durable_store_execution(&lease, slot, &verified)
        .expect("prepare Store before wrong-kind successor");
    assert!(matches!(
        prepared.seal_validate_successor(&effect),
        Err(DurableStoreExecutionError::InvalidValidateSuccessor)
    ));
    assert_eq!(format!("{registry:?}"), before);

    let AdapterEffect::StoreBody { round, subject, .. } = effect.clone() else {
        unreachable!("durable Store fixture retains its Store effect")
    };
    let wrong_tag_validate = AdapterEffect::ValidateBody {
        tag: EventTag::new(round.height, round.view, Generation::new(999)),
        round,
        subject,
    };
    let prepared = registry
        .prepare_durable_store_execution(&lease, slot, &verified)
        .expect("prepare Store before wrong-tag successor");
    assert!(matches!(
        prepared.seal_validate_successor(&wrong_tag_validate),
        Err(DurableStoreExecutionError::InvalidValidateSuccessor)
    ));
    assert_eq!(format!("{registry:?}"), before);

    let closed = registry
        .entries
        .remove(&address)
        .expect("test-only conversion of closed row to pending kind");
    let ConcreteLifecycleWork {
        digest,
        kind: ConcreteLifecycleWorkKind::DurableStoreBody(store),
    } = closed
    else {
        unreachable!("fixture retains one closed Store row")
    };
    let DurableStoreBody {
        effect, pending, ..
    } = store;
    let pending_work = ConcreteLifecycleWork {
        digest,
        kind: ConcreteLifecycleWorkKind::PendingAdapter { effect, pending },
    };
    assert!(pending_work.validate_exact());
    assert!(registry.entries.insert(address, pending_work).is_none());
    assert!(matches!(
        registry.prepare_durable_store_execution(&lease, slot, &verified),
        Err(DurableStoreExecutionError::WrongWorkKind)
    ));
}

#[cfg(feature = "bls")]
fn assert_corrupt_durable_store_rejected(
    marker: u8,
    corrupt: impl FnOnce(&mut ConcreteLifecycleWork),
) {
    let DurableStoreFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        ..
    } = durable_store_fixture(marker);
    let work = registry
        .entries
        .get_mut(&address)
        .expect("corruption fixture retains its closed Store row");
    corrupt(work);
    assert!(!work.validate_exact());
    let before = format!("{registry:?}");
    assert!(matches!(
        registry.prepare_durable_store_execution(&lease, slot, &verified),
        Err(DurableStoreExecutionError::Registry(
            RegistryError::CorruptWork
        ))
    ));
    assert_eq!(format!("{registry:?}"), before);
    assert_eq!(registry.len(), 1);
    assert!(registry.entries.contains_key(&address));
}

#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn durable_store_validation_rejects_every_corrupt_closed_coordinate() {
    assert_corrupt_durable_store_rejected(0x71, |work| {
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Store")
        };
        store.address.ordinal = 0;
    });
    assert_corrupt_durable_store_rejected(0x72, |work| {
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Store")
        };
        let foreign_owner = owner(0xF2, store.address.ordinal);
        assert_ne!(
            foreign_owner.causal_root(),
            super::super::CausalRoot::new(digest_from_hash(store.pending.causal_lifecycle_key()))
        );
        store.address.owner = foreign_owner;
    });

    let mut foreign = durable_store_fixture(0x73);
    let foreign_work = foreign
        .registry
        .entries
        .remove(&foreign.address)
        .expect("take foreign pending only inside private fixture");
    let ConcreteLifecycleWorkKind::DurableStoreBody(foreign_store) = foreign_work.kind else {
        unreachable!("foreign fixture retains one closed Store")
    };
    let foreign_pending = foreign_store.pending;
    assert_corrupt_durable_store_rejected(0x74, move |work| {
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Store")
        };
        store.pending = foreign_pending;
    });

    assert_corrupt_durable_store_rejected(0x75, |work| {
        work.digest = LifecycleDigest::new([0xD5; 32]);
    });
    assert_corrupt_durable_store_rejected(0x76, |work| {
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Store")
        };
        let AdapterEffect::StoreBody { round, subject, .. } = &store.effect else {
            unreachable!("corruption fixture retains one Store effect")
        };
        store.durable_receipt = DurableBodyReceipt::for_test(
            wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"foreign durable Store context",
            ))),
            *round,
            *subject,
            store.expected_manifest_hash,
        );
    });
    assert_corrupt_durable_store_rejected(0x77, |work| {
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Store")
        };
        let AdapterEffect::StoreBody { round, subject, .. } = &store.effect else {
            unreachable!("corruption fixture retains one Store effect")
        };
        let wrong_round = wire::ConsensusRound {
            view: round.view.saturating_add(1),
            ..*round
        };
        store.durable_receipt = DurableBodyReceipt::for_test(
            round.context_id,
            wrong_round,
            *subject,
            store.expected_manifest_hash,
        );
    });
    assert_corrupt_durable_store_rejected(0x78, |work| {
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Store")
        };
        let AdapterEffect::StoreBody { round, subject, .. } = &store.effect else {
            unreachable!("corruption fixture retains one Store effect")
        };
        let wrong_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"foreign durable Store subject")),
            ..*subject
        };
        store.durable_receipt = DurableBodyReceipt::for_test(
            round.context_id,
            *round,
            wrong_subject,
            store.expected_manifest_hash,
        );
    });
    assert_corrupt_durable_store_rejected(0x79, |work| {
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Store")
        };
        let AdapterEffect::StoreBody { round, subject, .. } = &store.effect else {
            unreachable!("corruption fixture retains one Store effect")
        };
        store.durable_receipt = DurableBodyReceipt::for_test(
            round.context_id,
            *round,
            *subject,
            HashOf::from_untyped_unchecked(Hash::new(b"foreign manifest hash")),
        );
    });
    assert_corrupt_durable_store_rejected(0x7A, |work| {
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Store")
        };
        store.expected_manifest_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"altered parent manifest hash"));
    });
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_prepare_and_drop_preserve_the_closed_row() {
    let DurableValidateFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        effect,
        expected_manifest_hash,
        ..
    } = durable_validate_fixture(0x81);
    let AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    } = effect.clone()
    else {
        unreachable!("durable Validate fixture retains its Validate effect")
    };
    let before = format!("{registry:?}");

    let prepared = registry
        .prepare_durable_validate_execution(&lease, slot, &verified)
        .expect("prepare exact durable Validate execution");
    assert_eq!(prepared.adapter_preview_inputs(), (tag, round, subject));
    assert_eq!(
        prepared.durable_body_receipt().context_id(),
        round.context_id
    );
    assert_eq!(prepared.durable_body_receipt().round(), round);
    assert_eq!(prepared.durable_body_receipt().subject(), subject);
    assert_eq!(
        prepared.durable_body_receipt().manifest_hash(),
        expected_manifest_hash
    );
    assert_eq!(prepared.expected_manifest_hash(), expected_manifest_hash);
    drop(prepared);

    assert_eq!(format!("{registry:?}"), before);
    assert!(registry.exactly_contains(address, &effect));
    assert_eq!(
        registry.borrow_for_lease(&lease, slot),
        Err(RegistryError::WrongWorkKind)
    );
    assert!(matches!(
        registry.take_for_lease(&lease, slot),
        Err(RegistryError::WrongWorkKind)
    ));
    assert_eq!(format!("{registry:?}"), before);

    let mut disposable = durable_validate_fixture(0x82);
    let closed = disposable
        .registry
        .entries
        .remove(&disposable.address)
        .expect("remove disposable closed Validate only for into-pair rejection test");
    let unwind = catch_unwind(AssertUnwindSafe(|| closed.into_pair()));
    assert!(
        unwind.is_err(),
        "closed Validate must not expose a raw pair"
    );
}

#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn durable_validate_prepare_rejects_wrong_lease_projection_and_context_without_mutation() {
    let DurableValidateFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        effect,
        ..
    } = durable_validate_fixture(0x83);
    let before = format!("{registry:?}");

    let mut wrong_class = lease.clone();
    wrong_class.work_class = LifecycleWorkClass::Store;
    assert!(matches!(
        registry.prepare_durable_validate_execution(&wrong_class, slot, &verified),
        Err(DurableValidateExecutionError::InvalidLeaseShape)
    ));

    let exact_key = lease.key();
    let mut wrong_phase = lease.clone();
    wrong_phase.key = super::super::LifecycleKey::new(
        exact_key.context(),
        exact_key.round(),
        exact_key.proposal_round(),
        exact_key.subject(),
        LifecyclePhase::Store,
        exact_key.execution_commitment(),
    );
    assert!(matches!(
        registry.prepare_durable_validate_execution(&wrong_phase, slot, &verified),
        Err(DurableValidateExecutionError::InvalidLeaseShape)
    ));

    let mut wrong_stage = lease.clone();
    wrong_stage.stage = super::super::LifecycleStage::new(
        LifecycleStageKind::StoreBody,
        PredecessorScope::Independent,
    );
    assert!(matches!(
        registry.prepare_durable_validate_execution(&wrong_stage, slot, &verified),
        Err(DurableValidateExecutionError::InvalidLeaseShape)
    ));

    let mut wrong_scope = lease.clone();
    wrong_scope.stage = super::super::LifecycleStage::new(
        LifecycleStageKind::ValidateBody,
        PredecessorScope::ReadyOrdinalPrefix,
    );
    assert!(matches!(
        registry.prepare_durable_validate_execution(&wrong_scope, slot, &verified),
        Err(DurableValidateExecutionError::InvalidLeaseShape)
    ));

    let other_slot = PhysicalSlotId::for_capacity(lease.work_class().capacity_class(), 1);
    assert!(matches!(
        registry.prepare_durable_validate_execution(&lease, other_slot, &verified),
        Err(DurableValidateExecutionError::InvalidLeaseShape)
    ));

    let mut wrong_digest = lease.clone();
    wrong_digest
        .physical_slots
        .insert(slot, LifecycleDigest::new([0xD4; 32]));
    assert!(matches!(
        registry.prepare_durable_validate_execution(&wrong_digest, slot, &verified),
        Err(DurableValidateExecutionError::Registry(
            RegistryError::DigestMismatch
        ))
    ));

    let mut stale_address = lease.clone();
    stale_address.ordinal = stale_address.ordinal.saturating_add(1);
    assert!(matches!(
        registry.prepare_durable_validate_execution(&stale_address, slot, &verified),
        Err(DurableValidateExecutionError::Registry(
            RegistryError::Missing
        ))
    ));

    let mut wrong_key = lease.clone();
    wrong_key.key = super::super::LifecycleKey::new(
        exact_key.context(),
        exact_key.round(),
        exact_key.proposal_round(),
        Some(LifecycleDigest::new([0xE4; 32])),
        exact_key.phase(),
        exact_key.execution_commitment(),
    );
    assert!(matches!(
        registry.prepare_durable_validate_execution(&wrong_key, slot, &verified),
        Err(DurableValidateExecutionError::InvalidProjection)
    ));

    let (foreign_verified, _) = verified_store_context(0x84);
    assert!(matches!(
        registry.prepare_durable_validate_execution(&lease, slot, &foreign_verified),
        Err(DurableValidateExecutionError::Projection(
            AdapterEffectAdmissionError::ForeignContext
        ))
    ));

    assert_eq!(format!("{registry:?}"), before);
    assert!(registry.exactly_contains(address, &effect));
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_prepare_rejects_an_executable_adapter_at_the_exact_address() {
    let DurableValidateFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        ..
    } = durable_validate_fixture(0x85);
    let closed = registry
        .entries
        .remove(&address)
        .expect("test-only conversion of closed Validate row to pending kind");
    let ConcreteLifecycleWork {
        digest,
        kind: ConcreteLifecycleWorkKind::DurableValidateBody(validate),
    } = closed
    else {
        unreachable!("fixture retains one closed Validate row")
    };
    let DurableValidateBody {
        effect, pending, ..
    } = validate;
    let pending_work = ConcreteLifecycleWork {
        digest,
        kind: ConcreteLifecycleWorkKind::PendingAdapter { effect, pending },
    };
    assert!(pending_work.validate_exact());
    assert!(registry.entries.insert(address, pending_work).is_none());
    assert!(matches!(
        registry.prepare_durable_validate_execution(&lease, slot, &verified),
        Err(DurableValidateExecutionError::WrongWorkKind)
    ));
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_binds_exact_success_receipt_without_registry_mutation() {
    let DurableValidateFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        effect,
        ..
    } = durable_validate_fixture(0x95);
    let before = format!("{registry:?}");
    let prepared = registry
        .prepare_durable_validate_execution(&lease, slot, &verified)
        .expect("prepare exact closed Validate carrier");
    let preview_inputs = prepared.adapter_preview_inputs();
    let validated = ValidatedBodyReceipt::for_test(prepared.durable_body_receipt().clone());
    let expected_commitment = validated.execution_commitment();
    let completion = prepared
        .bind_validated_receipt(validated)
        .expect("bind exact store-minted validation receipt");
    assert_eq!(completion.address, address);
    assert_eq!(completion.adapter_preview_inputs(), preview_inputs);
    assert_eq!(
        completion.validated_receipt().execution_commitment(),
        expected_commitment
    );
    assert_eq!(completion.incumbent_digest(), lease.physical_slots()[&slot]);
    assert_ne!(
        completion.replacement_digest(),
        completion.incumbent_digest()
    );
    let first_replacement = completion.replacement_digest();
    drop(completion);
    assert_eq!(format!("{registry:?}"), before);
    assert!(registry.exactly_contains(address, &effect));

    let repeated = registry
        .prepare_durable_validate_execution(&lease, slot, &verified)
        .expect("repeat exact closed Validate preflight");
    let repeated_receipt = ValidatedBodyReceipt::for_test(repeated.durable_body_receipt().clone());
    let repeated = repeated
        .bind_validated_receipt(repeated_receipt)
        .expect("repeat deterministic validation binding");
    assert_eq!(repeated.replacement_digest(), first_replacement);
    drop(repeated);
    assert_eq!(format!("{registry:?}"), before);
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_rejects_foreign_success_receipt_without_registry_mutation() {
    let DurableValidateFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        effect,
        expected_manifest_hash,
        ..
    } = durable_validate_fixture(0x96);
    let before = format!("{registry:?}");
    let prepared = registry
        .prepare_durable_validate_execution(&lease, slot, &verified)
        .expect("prepare exact closed Validate carrier");
    let (_, round, subject) = prepared.adapter_preview_inputs();
    let foreign_durable = DurableBodyReceipt::for_test(
        round.context_id,
        wire::ConsensusRound {
            view: round.view.saturating_add(1),
            ..round
        },
        subject,
        expected_manifest_hash,
    );
    let foreign = ValidatedBodyReceipt::for_test(foreign_durable);
    let Err((error, returned)) = prepared.bind_validated_receipt(foreign) else {
        panic!("foreign durable receipt must not bind Validate completion")
    };
    assert_eq!(
        error,
        DurableValidateExecutionError::InvalidValidationReceipt
    );
    assert_ne!(returned.durable().round(), round);
    assert_eq!(format!("{registry:?}"), before);
    assert!(registry.exactly_contains(address, &effect));
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_detach_and_drop_release_the_registry_without_mutation() {
    let DurableValidateFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        effect,
        ..
    } = durable_validate_fixture(0xA0);
    let before = format!("{registry:?}");
    let detached = registry
        .prepare_durable_validate_execution(&lease, slot, &verified)
        .expect("prepare detached durable Validate")
        .detach();

    assert_eq!(format!("{registry:?}"), before);
    assert!(registry.exactly_contains(address, &effect));
    drop(detached);
    assert_eq!(format!("{registry:?}"), before);
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_detached_success_reattaches_and_repeats_idempotently() {
    let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA1);
    let before = format!("{:?}", fixture.registry);
    let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let detached = fixture
        .registry
        .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
        .expect("prepare exact durable Validate")
        .detach();
    assert_eq!(format!("{:?}", fixture.registry), before);
    let executed = detached
        .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
        .expect("execute detached durable validation");
    assert_eq!(executed.outcome().durable_body(), &durable);
    assert_eq!(
        executed
            .outcome()
            .validated_receipt()
            .map(ValidatedBodyReceipt::execution_commitment),
        Some(commitment)
    );
    let completed = fixture
        .registry
        .reattach_durable_validate_execution(executed)
        .expect("reattach exact durable Validate success");
    assert_eq!(
        completed.adapter_preview_inputs(),
        match fixture.effect {
            AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } => (tag, round, subject),
            _ => unreachable!("fixture retains one Validate effect"),
        }
    );
    assert!(completed.outcome().validated_receipt().is_some());
    drop(completed);
    assert_eq!(format!("{:?}", fixture.registry), before);

    let repeated = fixture
        .registry
        .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
        .expect("repeat exact durable Validate preflight")
        .detach()
        .execute(
            &mut store,
            |_| -> Result<wire::ExecutionCommitment, DetachedValidationError> {
                panic!("durable validation marker must bypass the callback")
            },
        )
        .expect("repeat reuses durable validation marker");
    assert_eq!(
        repeated
            .outcome()
            .validated_receipt()
            .map(ValidatedBodyReceipt::execution_commitment),
        Some(commitment)
    );
    let repeated = fixture
        .registry
        .reattach_durable_validate_execution(repeated)
        .expect("reattach repeated deterministic success");
    drop(repeated);
    assert_eq!(format!("{:?}", fixture.registry), before);
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_reattach_rejects_row_and_digest_changes_with_outcome_intact() {
    let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA2);
    let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let executed = fixture
        .registry
        .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
        .expect("prepare exact durable Validate")
        .detach()
        .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
        .expect("execute exact detached validation");

    fixture
        .registry
        .entries
        .get_mut(&fixture.address)
        .expect("fixture retains exact Validate row")
        .digest = LifecycleDigest::new([0xEF; 32]);
    let mutated = format!("{:?}", fixture.registry);
    let Err((error, executed)) = fixture
        .registry
        .reattach_durable_validate_execution(executed)
    else {
        panic!("mutated incumbent digest must reject reattachment")
    };
    assert_eq!(
        error,
        DurableValidateExecutionError::Registry(RegistryError::CorruptWork)
    );
    assert_eq!(format!("{:?}", fixture.registry), mutated);
    assert_eq!(executed.outcome().durable_body(), &durable);
    assert_eq!(
        executed
            .outcome()
            .validated_receipt()
            .map(ValidatedBodyReceipt::execution_commitment),
        Some(commitment)
    );
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_reattach_rejects_foreign_registry_address_and_carrier() {
    let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA3);
    let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
    let executed = fixture
        .registry
        .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
        .expect("prepare exact durable Validate")
        .detach()
        .execute(&mut store, |_| Ok::<_, DetachedValidationError>(commitment))
        .expect("execute exact detached validation");

    let mut foreign_registry = ConcreteLifecycleWorkRegistry::default();
    let Err((error, mut executed)) = foreign_registry.reattach_durable_validate_execution(executed)
    else {
        panic!("foreign empty registry must reject reattachment")
    };
    assert_eq!(
        error,
        DurableValidateExecutionError::Registry(RegistryError::Missing)
    );

    let exact_address = executed.request.address;
    executed.request.address = ConcreteWorkAddress::new(
        exact_address.owner,
        exact_address.ordinal.saturating_add(1),
        exact_address.slot,
    )
    .expect("construct foreign detached address");
    let Err((error, mut executed)) = fixture
        .registry
        .reattach_durable_validate_execution(executed)
    else {
        panic!("foreign detached address must reject reattachment")
    };
    assert_eq!(
        error,
        DurableValidateExecutionError::Registry(RegistryError::Missing)
    );
    executed.request.address = exact_address;

    let closed = fixture
        .registry
        .entries
        .remove(&fixture.address)
        .expect("replace exact carrier only in this rejection fixture");
    let ConcreteLifecycleWork {
        digest,
        kind: ConcreteLifecycleWorkKind::DurableValidateBody(validate),
    } = closed
    else {
        unreachable!("fixture retains one closed Validate carrier")
    };
    let DurableValidateBody {
        effect, pending, ..
    } = validate;
    let pending = ConcreteLifecycleWork {
        digest,
        kind: ConcreteLifecycleWorkKind::PendingAdapter { effect, pending },
    };
    assert!(pending.validates_at(fixture.address));
    assert!(
        fixture
            .registry
            .entries
            .insert(fixture.address, pending)
            .is_none()
    );
    let foreign_carrier = format!("{:?}", fixture.registry);
    let Err((error, returned)) = fixture
        .registry
        .reattach_durable_validate_execution(executed)
    else {
        panic!("foreign carrier kind must reject reattachment")
    };
    assert_eq!(error, DurableValidateExecutionError::WrongWorkKind);
    assert_eq!(format!("{:?}", fixture.registry), foreign_carrier);
    assert!(returned.outcome().validated_receipt().is_some());
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_detached_rejection_and_sidecar_deferral_remain_bound() {
    let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA4);
    let before = format!("{:?}", fixture.registry);
    let rejected = fixture
        .registry
        .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
        .expect("prepare rejected detached Validate")
        .detach()
        .execute(&mut store, |_| {
            Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                "detached candidate rejected",
            ))
        })
        .expect("execute deterministic rejection");
    assert_eq!(rejected.outcome().durable_body(), &durable);
    assert_eq!(
        rejected.outcome().rejection_reason(),
        Some("detached candidate rejected")
    );
    let rejected = fixture
        .registry
        .reattach_durable_validate_execution(rejected)
        .expect("reattach exact deterministic rejection");
    assert_eq!(
        rejected.outcome().rejection_reason(),
        Some("detached candidate rejected")
    );
    drop(rejected);
    assert_eq!(format!("{:?}", fixture.registry), before);

    let reference = detached_validation_merge_reference(&durable);
    let deferred = fixture
        .registry
        .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
        .expect("prepare deferred detached Validate")
        .detach()
        .execute(&mut store, |_| {
            Err::<wire::ExecutionCommitment, _>(DetachedValidationError::MissingMergeSidecar(
                reference.clone(),
            ))
        })
        .expect("execute typed sidecar deferral");
    assert_eq!(deferred.outcome().durable_body(), &durable);
    assert_eq!(deferred.outcome().missing_merge_sidecar(), Some(&reference));
    let deferred = fixture
        .registry
        .reattach_durable_validate_execution(deferred)
        .expect("reattach exact sidecar deferral");
    assert_eq!(deferred.outcome().missing_merge_sidecar(), Some(&reference));
    drop(deferred);
    assert_eq!(format!("{:?}", fixture.registry), before);
}

#[cfg(feature = "bls")]
#[test]
fn durable_validate_reattach_rejects_an_inflight_authority_upgrade() {
    let (mut fixture, _directory, mut store, durable) = durable_validate_store_fixture(0xA5);
    let executed = fixture
        .registry
        .prepare_durable_validate_execution(&fixture.lease, fixture.slot, &fixture.verified)
        .expect("prepare exact durable Validate")
        .detach()
        .execute(&mut store, |_| {
            Err::<wire::ExecutionCommitment, _>(DetachedValidationError::Invalid(
                "authority-upgrade fixture rejection",
            ))
        })
        .expect("execute detached validation before authority upgrade");
    let original_statement = executed.request.candidate_statement;
    let AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    } = fixture.effect.clone()
    else {
        unreachable!("fixture retains one Validate effect")
    };
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round,
        subject,
    };
    let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round,
        subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: commitment,
            signers: Vec::new(),
            aggregate_signature: Vec::new(),
        }),
    };
    let certified_fetch_owner = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 50_001)],
    )
    .expect("bind one Commit-authorized Fetch")
    .pop()
    .expect("one Commit Fetch owner");
    let incoming_store_owner = certified_fetch_owner
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("carry Commit authority into Store");
    let adopted_store_owner = fixture
        .store_ownership
        .adopt_incumbent_body_stage_for_retry_or_authority(&incoming_store_owner, &store_effect)
        .expect("retain physical Store owner while upgrading authority");
    let upgraded_store = adopted_store_owner
        .pending_adapter_effect_binding(&store_effect)
        .expect("mint upgraded Store binding");
    let upgraded_validate = upgraded_store
        .project_store_validate_successor(&store_effect, &fixture.effect)
        .expect("carry upgraded authority into Validate");
    assert_eq!(
        upgraded_validate.causal_lifecycle_key(),
        &executed.request.causal_lifecycle_key
    );
    assert_ne!(upgraded_validate.candidate_statement(), original_statement);

    let work = fixture
        .registry
        .entries
        .get_mut(&fixture.address)
        .expect("authority fixture retains exact Validate row");
    let digest = work.digest;
    let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
        unreachable!("authority fixture retains one closed Validate")
    };
    validate.pending = upgraded_validate;
    assert!(validate.validates(digest));
    assert!(work.validates_at(fixture.address));
    let upgraded = format!("{:?}", fixture.registry);
    let Err((error, returned)) = fixture
        .registry
        .reattach_durable_validate_execution(executed)
    else {
        panic!("in-flight authority upgrade must reject unchanged-row CAS")
    };
    assert_eq!(error, DurableValidateExecutionError::InvalidValidateShape);
    assert_eq!(format!("{:?}", fixture.registry), upgraded);
    assert_eq!(
        returned.outcome().rejection_reason(),
        Some("authority-upgrade fixture rejection")
    );
}

#[cfg(feature = "bls")]
fn assert_corrupt_durable_validate_rejected(
    marker: u8,
    corrupt: impl FnOnce(&mut ConcreteLifecycleWork),
) {
    let DurableValidateFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        ..
    } = durable_validate_fixture(marker);
    let work = registry
        .entries
        .get_mut(&address)
        .expect("corruption fixture retains its closed Validate row");
    corrupt(work);
    assert!(!work.validate_exact());
    let before = format!("{registry:?}");
    assert!(matches!(
        registry.prepare_durable_validate_execution(&lease, slot, &verified),
        Err(DurableValidateExecutionError::Registry(
            RegistryError::CorruptWork
        ))
    ));
    assert_eq!(format!("{registry:?}"), before);
    assert_eq!(registry.len(), 1);
    assert!(registry.entries.contains_key(&address));
}

#[cfg(feature = "bls")]
#[test]
#[allow(clippy::too_many_lines)]
fn durable_validate_validation_rejects_every_corrupt_closed_coordinate() {
    assert_corrupt_durable_validate_rejected(0x86, |work| {
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Validate")
        };
        validate.address.ordinal = 0;
    });
    assert_corrupt_durable_validate_rejected(0x87, |work| {
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Validate")
        };
        let foreign_owner = owner(0xF7, validate.address.ordinal);
        assert_ne!(
            foreign_owner.causal_root(),
            super::super::CausalRoot::new(digest_from_hash(
                validate.pending.causal_lifecycle_key()
            ))
        );
        validate.address.owner = foreign_owner;
    });

    let mut foreign = durable_validate_fixture(0x88);
    let foreign_work = foreign
        .registry
        .entries
        .remove(&foreign.address)
        .expect("take foreign pending only inside private fixture");
    let ConcreteLifecycleWorkKind::DurableValidateBody(foreign_validate) = foreign_work.kind else {
        unreachable!("foreign fixture retains one closed Validate")
    };
    let foreign_pending = foreign_validate.pending;
    assert_corrupt_durable_validate_rejected(0x89, move |work| {
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Validate")
        };
        validate.pending = foreign_pending;
    });

    assert_corrupt_durable_validate_rejected(0x8A, |work| {
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Validate")
        };
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &validate.effect
        else {
            unreachable!("corruption fixture retains one Validate effect")
        };
        validate.effect = AdapterEffect::StoreBody {
            tag: *tag,
            round: *round,
            subject: *subject,
        };
    });
    assert_corrupt_durable_validate_rejected(0x8B, |work| {
        work.digest = LifecycleDigest::new([0xDB; 32]);
    });
    assert_corrupt_durable_validate_rejected(0x8C, |work| {
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Validate")
        };
        let AdapterEffect::ValidateBody { round, subject, .. } = &validate.effect else {
            unreachable!("corruption fixture retains one Validate effect")
        };
        validate.durable_receipt = DurableBodyReceipt::for_test(
            wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"foreign durable Validate context",
            ))),
            *round,
            *subject,
            validate.expected_manifest_hash,
        );
    });
    assert_corrupt_durable_validate_rejected(0x8D, |work| {
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Validate")
        };
        let AdapterEffect::ValidateBody { round, subject, .. } = &validate.effect else {
            unreachable!("corruption fixture retains one Validate effect")
        };
        let wrong_round = wire::ConsensusRound {
            view: round.view.saturating_add(1),
            ..*round
        };
        validate.durable_receipt = DurableBodyReceipt::for_test(
            round.context_id,
            wrong_round,
            *subject,
            validate.expected_manifest_hash,
        );
    });
    assert_corrupt_durable_validate_rejected(0x8E, |work| {
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Validate")
        };
        let AdapterEffect::ValidateBody { round, subject, .. } = &validate.effect else {
            unreachable!("corruption fixture retains one Validate effect")
        };
        let wrong_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"foreign durable Validate subject",
            )),
            ..*subject
        };
        validate.durable_receipt = DurableBodyReceipt::for_test(
            round.context_id,
            *round,
            wrong_subject,
            validate.expected_manifest_hash,
        );
    });
    assert_corrupt_durable_validate_rejected(0x8F, |work| {
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("corruption fixture retains one closed Validate")
        };
        validate.expected_manifest_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"altered Validate manifest hash"));
    });
}
