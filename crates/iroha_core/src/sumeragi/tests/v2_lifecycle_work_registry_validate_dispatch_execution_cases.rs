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
        let pending = ConcreteLifecycleWork::from_inert_fixture_for_test(effect, pending)
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
fn durable_validate_dispatch_rejects_a_substituted_ledger_body_frame() {
    let (mut fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBE);
    let mut coordinator = claimed_durable_validate_coordinator(&fixture);
    let metadata = coordinator
        .durable_records
        .get_mut(&fixture.lease.ordinal())
        .expect("claimed Validate retains durable metadata");
    let DurablePayloadReference::BodyFrame(mut substituted) = metadata.payload else {
        panic!("claimed Validate must retain one durable body frame")
    };
    substituted.frame = LifecycleDigest::new([0xEE; 32]);
    metadata.payload = DurablePayloadReference::BodyFrame(substituted);
    let mut holder = take_dispatch_registry(&mut fixture);
    let coordinator_before = format!("{coordinator:?}");
    let registry_before = format!("{:?}", holder.registry_for_test());
    let lease = fixture.lease.clone();

    let Err((error, returned)) =
        coordinator.begin_durable_validate_dispatch(&mut holder, lease.clone(), &fixture.verified)
    else {
        panic!("a ledger frame foreign to the installed carrier must fail closed")
    };
    assert_eq!(
        error,
        DurableValidateDispatchError::Registry(DurableValidateExecutionError::InvalidValidateShape)
    );
    assert_eq!(returned, lease);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
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
        let primary_key = alias.key;
        alias.key = super::super::LifecycleKey::new(
            primary_key.context(),
            primary_key.round(),
            primary_key.proposal_round(),
            primary_key.subject(),
            super::super::LifecyclePhase::Apply,
            primary_key.execution_commitment(),
        );
        assert_ne!(alias.key, primary_key);
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
fn ready_validate_capacity_classifier_is_exact_and_drop_inert() {
    let (fixture, _directory, _store, _durable) = durable_validate_store_fixture(0xBF);
    let before = format!("{:?}", fixture.registry);
    let digest = fixture.lease.physical_slots()[&fixture.slot];

    let seal = fixture
        .registry
        .classify_ready_validate_carrier(fixture.address, digest)
        .expect("exact durable Validate carrier mints one opaque seal");
    assert!(seal.matches(
        fixture.address.owner,
        fixture.address.ordinal,
        fixture.address.slot,
        digest,
    ));
    assert!(!seal.requires_consensus_capacity());
    assert!(seal.requires_io_dispatch());
    assert_eq!(
        fixture
            .registry
            .classify_ready_validate_carrier(fixture.address, LifecycleDigest::new([0xFF; 32]),),
        Err(ReadyValidateCarrierError::Registry(
            RegistryError::DigestMismatch
        ))
    );
    assert_eq!(format!("{:?}", fixture.registry), before);

    let mut coordinator = claimed_durable_validate_coordinator(&fixture);
    coordinator.active_lease = None;
    coordinator.ready_index.insert(fixture.lease.ordinal());
    coordinator
        .records
        .get_mut(&fixture.lease.ordinal())
        .expect("exact Validate row remains installed")
        .state = LifecycleState::Ready;
    let ordinal = fixture.lease.ordinal();
    let holder = LifecycleWorkRegistryHolder::from_registry_for_test(fixture.registry);
    let coordinator_before = format!("{coordinator:?}");
    assert_eq!(
        coordinator.direct_registry_scheduler_inputs_for_test(&holder),
        Err(ProductionSchedulerInputsError::IoCapacityObservationRequired { ordinal })
    );
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
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
    assert_eq!(
        coordinator
            .attest_ready_validate_demand(&holder, ordinal)
            .expect("validated completion mints one exact scheduler attestation")
            .capacity_class(),
        None
    );
    let inputs = coordinator
        .direct_registry_scheduler_inputs_for_test(&holder)
        .expect("validated completion has no nested service episode");
    let (generations, ready) = inputs.into_parts();
    assert!(generations.is_empty());
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[&ordinal].live_debts(), [0; 6]);
    let plan_inputs = super::super::SchedulerInputs::new(generations, ready)
        .expect("one unique direct validated-completion row");
    let mut planned = coordinator.clone();
    let super::super::TurnPlan::Execute(lease) = planned.plan_turn(plan_inputs) else {
        panic!("direct validated completion must be selectable")
    };
    assert_eq!(lease.ordinal(), ordinal);
    assert!(lease.output_reservation().is_none());

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

    let corrupt_digest = if location.replacement_digest != LifecycleDigest::new([0xE7; 32]) {
        LifecycleDigest::new([0xE7; 32])
    } else {
        LifecycleDigest::new([0xE8; 32])
    };
    holder
        .registry_for_test_mut()
        .entries
        .get_mut(&fixture.address)
        .expect("validated completion remains installed")
        .digest = corrupt_digest;
    let coordinator_before = format!("{coordinator:?}");
    let registry_before = format!("{:?}", holder.registry_for_test());
    assert_eq!(
        coordinator.direct_registry_scheduler_inputs_for_test(&holder),
        Err(ProductionSchedulerInputsError::InvalidValidateCarrier { ordinal })
    );
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
}

#[cfg(feature = "bls")]
#[test]
fn validated_completion_rejects_conflicting_inherited_commitment_intact() {
    let inherited_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"inherited commitment parent"),
        Hash::new(b"inherited commitment post"),
        Hash::new(b"inherited commitment writes"),
        1,
        Hash::new(b"inherited commitment wire"),
    );
    assert!(inherited_commitment.validate().is_ok());
    let (mut fixture, _directory, mut store, durable) =
        durable_validate_store_fixture_at_view_with_commitment(0xCD, 2, Some(inherited_commitment));
    let yielded_commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    assert_ne!(inherited_commitment, yielded_commitment);
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

#[test]
fn ready_validate_publication_and_successor_tokens_remain_move_only() {
    let source = include_str!("../v2_lifecycle_work_registry_validate_recovery.rs");
    for declaration in [
        "struct PublishedValidated",
        "struct PublishedRejected",
        "struct ReadyValidateSuccessorV1",
    ] {
        let declaration_offset = source
            .find(declaration)
            .unwrap_or_else(|| panic!("{declaration} remains declared"));
        let attributes_offset = source[..declaration_offset]
            .rfind("\n///")
            .expect("move-only declaration retains adjacent documentation");
        let attributes = &source[attributes_offset..declaration_offset];
        assert!(!attributes.contains("Clone"), "{declaration} became Clone");
        assert!(!attributes.contains("Copy"), "{declaration} became Copy");
        let name = declaration
            .split_whitespace()
            .last()
            .expect("declaration includes one type name");
        assert!(!source.contains(&format!("impl Clone for {name}")));
        assert!(!source.contains(&format!("impl Copy for {name}")));
    }
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
    let attestation = coordinator
        .attest_ready_validate_demand(&holder, ordinal)
        .expect("rejected completion mints one exact scheduler attestation");
    assert_eq!(attestation.capacity_class(), Some(CapacityClass::Consensus));
    let inputs = coordinator
        .direct_registry_scheduler_inputs_for_test(&holder)
        .expect("rejected completion has no nested service episode");
    let (generations, ready) = inputs.into_parts();
    assert!(generations.is_empty());
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[&ordinal].live_debts(), [0; 6]);

    let mut stale = coordinator.clone();
    stale
        .records
        .get_mut(&ordinal)
        .expect("rejected completion row")
        .physical_slots
        .insert(fixture.slot, LifecycleDigest::new([0xEF; 32]));
    let stale_before = format!("{stale:?}");
    assert_eq!(
        stale.attest_ready_validate_demand(&holder, ordinal),
        Err(ReadyValidateDemandAttestationError::Registry(
            ReadyValidateCarrierError::Registry(RegistryError::DigestMismatch)
        ))
    );
    assert_eq!(format!("{stale:?}"), stale_before);

    let mut substituted = coordinator.clone();
    let metadata = substituted
        .durable_records
        .get_mut(&ordinal)
        .expect("rejected completion retains durable metadata");
    let DurablePayloadReference::BodyFrame(mut foreign_frame) = metadata.payload else {
        panic!("rejected completion must retain one durable body frame")
    };
    foreign_frame.manifest = LifecycleDigest::new([0xED; 32]);
    metadata.payload = DurablePayloadReference::BodyFrame(foreign_frame);
    let substituted_before = format!("{substituted:?}");
    assert_eq!(
        substituted.attest_ready_validate_demand(&holder, ordinal),
        Err(ReadyValidateDemandAttestationError::InvalidCoordinatorIndex)
    );
    assert_eq!(format!("{substituted:?}"), substituted_before);

    let inputs = super::super::SchedulerInputs::new(generations, ready)
        .expect("one unique registry-attested Ready row");
    let super::super::TurnPlan::Execute(lease) = coordinator.plan_turn(inputs) else {
        panic!("registry-attested rejected Validate must claim with its reservation")
    };
    assert_eq!(lease.ordinal(), ordinal);
    assert_eq!(
        lease
            .output_reservation()
            .map(|reservation| reservation.class()),
        Some(CapacityClass::Consensus)
    );

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
            durable,
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
        assert!(prepared.matches_exact_lease(&lease));
        assert!(prepared.matches_exact_durable_receipt(&durable));
        let foreign_receipt = DurableBodyReceipt::for_test(
            durable.context_id(),
            durable.round(),
            durable.subject(),
            HashOf::from_untyped_unchecked(Hash::new(b"foreign Ready Validate manifest")),
        );
        assert!(!prepared.matches_exact_durable_receipt(&foreign_receipt));
        let mut foreign_lease = lease.clone();
        foreign_lease.id = LeaseId(
            foreign_lease
                .id()
                .0
                .checked_add(1)
                .expect("fixture lease id remains bounded"),
        );
        assert!(!prepared.matches_exact_lease(&foreign_lease));
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
            durable,
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
        assert!(prepared.matches_exact_durable_receipt(&durable));
        assert!(prepared.rejected_authority().is_some());
        assert!(prepared.validated_authority().is_none());
        drop(prepared);
        assert_eq!(format!("{:?}", holder.registry_for_test()), before);
    }

    {
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            mut holder,
            mut lease,
            durable: _,
        } = ready_durable_validate_fixture(0xDA, ReadyDurableValidateFixtureOutcome::Rejected);
        lease.output_reservation = None;
        let before = format!("{:?}", holder.registry_for_test());
        assert!(matches!(
            holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified,),
            Err(ReadyDurableValidateExecutionError::InvalidLeaseShape)
        ));
        assert_eq!(format!("{:?}", holder.registry_for_test()), before);
    }

    {
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            mut holder,
            mut lease,
            durable: _,
        } = ready_durable_validate_fixture(0xDB, ReadyDurableValidateFixtureOutcome::Validated);
        lease.output_reservation = Some(super::super::schema::LeaseCapacityReservation::new(
            CapacityClass::Consensus,
            0,
        ));
        let before = format!("{:?}", holder.registry_for_test());
        assert!(matches!(
            holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified,),
            Err(ReadyDurableValidateExecutionError::InvalidLeaseShape)
        ));
        assert_eq!(format!("{:?}", holder.registry_for_test()), before);
    }
}

#[cfg(feature = "bls")]
fn open_empty_ready_validate_adapter(
    fixture: &DurableValidateFixture,
    marker: u8,
) -> (TempDir, std::path::PathBuf, SumeragiV2Adapter) {
    let AdapterEffect::ValidateBody { tag, .. } = &fixture.effect else {
        unreachable!("Ready fixture retains one Validate effect")
    };
    let directory = TempDir::new().expect("temporary empty Ready Validate adapter");
    let wal_path = directory.path().join("safety.wal");
    let (adapter, startup) = SumeragiV2Adapter::open(
        &wal_path,
        fixture.verified.clone(),
        Some(0),
        tag.generation(),
        [marker; 32],
        AdapterFingerprints {
            node: Hash::new([marker, 0xA1]),
            build: Hash::new([marker, 0xA2]),
            config: Hash::new([marker, 0xA3]),
        },
        DeferredAdmissionOrdinalSource::new(1),
    )
    .expect("open empty Ready Validate adapter");
    assert!(startup.is_empty());
    (directory, wal_path, adapter)
}

#[cfg(feature = "bls")]
fn assert_empty_ready_validate_adapter_unchanged(
    adapter: &mut SumeragiV2Adapter,
    wal_path: &std::path::Path,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    status_before: &wire::SumeragiV2Status,
    fence_before: u64,
    wal_before: &[u8],
) {
    assert_eq!(
        &adapter.status().expect("read unchanged adapter status"),
        status_before
    );
    assert_eq!(adapter.reducer_fence_generation(), fence_before);
    assert_eq!(
        adapter.body_state_for_test(round, subject),
        reducer::BodyState::Missing
    );
    assert!(!adapter.has_registered_manifest_for_test(round, subject));
    let wal_after = std::fs::read(wal_path).expect("read unchanged Ready Validate WAL");
    assert_eq!(wal_after.as_slice(), wal_before);
}

#[cfg(feature = "bls")]
#[test]
fn local_ready_validate_success_preflight_stages_manifest_drop_inertly() {
    let ReadyDurableValidateFixture {
        fixture,
        _directory,
        mut holder,
        lease,
        durable,
    } = ready_local_durable_validate_fixture_at_view(
        0xE6,
        0,
        ReadyDurableValidateFixtureOutcome::Validated,
    );
    let AdapterEffect::ValidateBody { round, subject, .. } = &fixture.effect else {
        unreachable!("local Ready fixture retains one Validate effect")
    };
    let (round, subject) = (*round, *subject);
    let (_adapter_directory, wal_path, mut adapter) =
        open_empty_ready_validate_adapter(&fixture, 0xE6);
    let status_before = adapter.status().expect("read empty adapter status");
    let fence_before = adapter.reducer_fence_generation();
    let wal_before = std::fs::read(&wal_path).expect("read empty Ready Validate WAL");
    let registry_before = format!("{:?}", holder.registry_for_test());
    assert!(!adapter.has_registered_manifest_for_test(round, subject));

    let mut prepared = holder
        .registry_for_test_mut()
        .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
        .expect("prepare local validated Ready carrier");
    let catalog_receipt = prepared
        .take_validated_catalog_authority()
        .expect("local validated Ready carrier owns one catalog authority")
        .into_validated_receipt();
    assert_eq!(catalog_receipt.durable(), &durable);
    assert!(prepared.take_validated_catalog_authority().is_none());
    assert!(prepared.project_local_proposal_ready().is_some());
    assert_eq!(
        prepared
            .preflight_adapter_publication_kind(&mut adapter)
            .expect("stage the local manifest for validated preflight"),
        ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
    );
    assert_empty_ready_validate_adapter_unchanged(
        &mut adapter,
        &wal_path,
        round,
        subject,
        &status_before,
        fence_before,
        &wal_before,
    );

    let preview = prepared
        .prepare_adapter_preview(&mut adapter)
        .unwrap_or_else(|_| panic!("prepare local validated adapter preview"));
    drop(preview);
    assert_empty_ready_validate_adapter_unchanged(
        &mut adapter,
        &wal_path,
        round,
        subject,
        &status_before,
        fence_before,
        &wal_before,
    );
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
}

#[cfg(feature = "bls")]
#[test]
fn local_ready_validate_rejection_preflight_stages_manifest_drop_inertly() {
    let ReadyDurableValidateFixture {
        fixture,
        _directory,
        mut holder,
        lease,
        durable: _,
    } = ready_local_durable_validate_fixture_at_view(
        0xE7,
        0,
        ReadyDurableValidateFixtureOutcome::Rejected,
    );
    let AdapterEffect::ValidateBody { round, subject, .. } = &fixture.effect else {
        unreachable!("local Ready fixture retains one Validate effect")
    };
    let (round, subject) = (*round, *subject);
    let (_adapter_directory, wal_path, mut adapter) =
        open_empty_ready_validate_adapter(&fixture, 0xE7);
    let status_before = adapter.status().expect("read empty adapter status");
    let fence_before = adapter.reducer_fence_generation();
    let wal_before = std::fs::read(&wal_path).expect("read empty Ready Validate WAL");
    let registry_before = format!("{:?}", holder.registry_for_test());

    let mut prepared = holder
        .registry_for_test_mut()
        .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
        .expect("prepare local rejected Ready carrier");
    assert!(prepared.take_validated_catalog_authority().is_none());
    assert!(prepared.project_local_proposal_ready().is_none());
    assert_eq!(
        prepared
            .preflight_adapter_publication_kind(&mut adapter)
            .expect("stage the local manifest for rejected preflight"),
        ReadyDurableValidateAdapterPublicationKind::RejectedInactive
    );
    assert_empty_ready_validate_adapter_unchanged(
        &mut adapter,
        &wal_path,
        round,
        subject,
        &status_before,
        fence_before,
        &wal_before,
    );

    let preview = prepared
        .prepare_adapter_preview(&mut adapter)
        .unwrap_or_else(|_| panic!("prepare local rejected adapter preview"));
    drop(preview);
    assert_empty_ready_validate_adapter_unchanged(
        &mut adapter,
        &wal_path,
        round,
        subject,
        &status_before,
        fence_before,
        &wal_before,
    );
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
}

#[cfg(feature = "bls")]
#[test]
fn remote_ready_validate_preflight_still_requires_registered_manifest() {
    let ReadyDurableValidateFixture {
        fixture,
        _directory,
        mut holder,
        lease,
        durable,
    } = ready_durable_validate_fixture_at_view(
        0xE8,
        0,
        ReadyDurableValidateFixtureOutcome::Validated,
    );
    let AdapterEffect::ValidateBody { round, subject, .. } = &fixture.effect else {
        unreachable!("remote Ready fixture retains one Validate effect")
    };
    let (round, subject) = (*round, *subject);
    let (_adapter_directory, wal_path, mut adapter) =
        open_empty_ready_validate_adapter(&fixture, 0xE8);
    let status_before = adapter.status().expect("read empty adapter status");
    let fence_before = adapter.reducer_fence_generation();
    let wal_before = std::fs::read(&wal_path).expect("read empty Ready Validate WAL");
    let registry_before = format!("{:?}", holder.registry_for_test());

    let mut prepared = holder
        .registry_for_test_mut()
        .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
        .expect("prepare remote validated Ready carrier");
    let catalog_receipt = prepared
        .take_validated_catalog_authority()
        .expect("remote validated Ready carrier owns one catalog authority")
        .into_validated_receipt();
    assert_eq!(catalog_receipt.durable(), &durable);
    assert!(prepared.take_validated_catalog_authority().is_none());
    assert!(prepared.project_local_proposal_ready().is_none());
    assert!(matches!(
        prepared.preflight_adapter_publication_kind(&mut adapter),
        Err(AdapterError::MissingManifest)
    ));
    drop(prepared);
    assert_empty_ready_validate_adapter_unchanged(
        &mut adapter,
        &wal_path,
        round,
        subject,
        &status_before,
        fence_before,
        &wal_before,
    );
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
}

#[cfg(feature = "bls")]
fn cold_ready_validate_runtime_at_durable(
    fixture: &DurableValidateFixture,
    durable: &DurableBodyReceipt,
    keys: &[KeyPair],
    root: &std::path::Path,
    wal_name: &str,
    now: std::time::Instant,
    lifecycle_ordinals: crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource,
) -> (
    crate::sumeragi::v2_runtime::SerializedV2Runtime,
    std::time::Duration,
    wire::QuorumCertificate,
) {
    let AdapterEffect::ValidateBody {
        tag,
        round,
        subject,
    } = fixture.effect
    else {
        unreachable!("cold Ready Validate fixture retains one Validate effect")
    };
    let wal_path = root.join(wal_name);
    let fingerprints = || AdapterFingerprints {
        node: Hash::new(b"cold Ready Validate node"),
        build: Hash::new(b"cold Ready Validate build"),
        config: Hash::new(b"cold Ready Validate config"),
    };
    let (adapter, startup) = SumeragiV2Adapter::open(
        wal_path.clone(),
        fixture.verified.clone(),
        Some(0),
        tag.generation(),
        [0xC5; 32],
        fingerprints(),
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("open cold Ready Validate runtime adapter");
    assert!(startup.is_empty());
    let (mut runtime, startup) = crate::sumeragi::v2_runtime::SerializedV2Runtime::new(
        adapter,
        startup,
        now,
        std::time::Duration::from_secs(10),
        crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap cold Ready Validate runtime");
    assert!(startup.is_empty());
    runtime
        .arm_live_clocks(now)
        .expect("arm the throwaway pre-crash runtime");

    let execution_commitment =
        ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let prepare_preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let prepare_shares = keys[..3]
        .iter()
        .map(|key| {
            iroha_crypto::Signature::new(key.private_key(), &prepare_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let prepare_refs = prepare_shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&prepare_refs)
            .expect("aggregate the cold Ready Validate PrepareQC"),
    };
    runtime
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(prepare.clone()),
        ))
        .expect("enqueue authenticated cold Ready Validate PrepareQC");
    let crate::sumeragi::v2_runtime::RuntimeStep::Advanced(fetch_effects) = runtime
        .step(now)
        .expect("dispatch cold Ready Validate PrepareQC")
    else {
        panic!("cold Ready Validate PrepareQC unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("PrepareQC dispatch retains its exact scheduler owner");
    let [
        AdapterEffect::FetchBody {
            tag: fetch_tag,
            round: fetch_round,
            subject: fetch_subject,
            manifest: fetch_manifest,
            certified_sources,
            certificate: Some(fetch_certificate),
        },
    ] = fetch_effects.as_slice()
    else {
        panic!("cold Ready Validate PrepareQC emitted foreign effects: {fetch_effects:?}")
    };
    assert_eq!(*fetch_tag, tag);
    assert_eq!(*fetch_round, round);
    assert_eq!(*fetch_subject, subject);
    assert!(fetch_manifest.is_none());
    assert_eq!(
        certified_sources,
        &fixture
            .verified
            .context()
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(fetch_certificate, &prepare);
    let fetch_ownership = runtime
        .take_effect_ownership(fetch_effects.len())
        .expect("take exact cold Ready Validate Fetch owner");
    assert_eq!(fetch_ownership.len(), 1);
    drop(fetch_ownership);
    assert_eq!(runtime.queued_commands(), 0);
    let retransmit_interval = runtime.retransmit_interval();
    drop(runtime);

    let (adapter, startup) = SumeragiV2Adapter::open(
        wal_path,
        fixture.verified.clone(),
        Some(0),
        tag.generation(),
        [0xC5; 32],
        fingerprints(),
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("cold-reopen the persisted Ready Validate runtime adapter");
    assert!(startup.is_empty());
    let cold_steps = [
        CertifiedBodyPipelineColdReplayStepV1::body_available(
            fixture.lease.ordinal(),
            tag,
            fixture.manifest.clone(),
            AdapterEffect::StoreBody {
                tag,
                round,
                subject,
            },
        )
        .expect("seal the recovered BodyAvailable-to-Store predecessor"),
        CertifiedBodyPipelineColdReplayStepV1::body_stored(
            fixture.lease.ordinal(),
            tag,
            durable.clone(),
            fixture.effect.clone(),
        )
        .expect("seal the recovered BodyStored-to-Validate predecessor"),
    ];
    let (adapter, startup) =
        ProductionLifecycleAdapterStartupV1::replay_certified_body_pipeline_for_test(
            adapter,
            startup,
            &cold_steps,
        )
        .expect("replay the exact certified-body prefix before exposing Ready Validate");
    let (runtime, startup) =
        crate::sumeragi::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            startup,
            now,
            std::time::Duration::from_secs(10),
            crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
            lifecycle_ordinals,
        )
        .expect("wrap the cold-reopened Ready Validate runtime");
    assert!(startup.is_empty());
    assert!(!runtime.lifecycle_live_clocks_are_armed());
    (runtime, retransmit_interval, prepare)
}

#[cfg(feature = "bls")]
#[test]
fn cold_ready_validate_open_stutters_real_periodic_retry_while_queued_and_active() {
    let handle = std::thread::Builder::new()
        .name("cold-ready-validate-periodic-retry".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(cold_ready_validate_open_stutters_real_periodic_retry_fixture)
        .expect("spawn cold Ready Validate periodic-retry fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
fn cold_ready_validate_logical_coordinator_snapshot(coordinator: &LifecycleCoordinator) -> String {
    let mut snapshot = coordinator.clone();
    snapshot.lifecycle_ordinal_authority = None;
    format!("{snapshot:?}")
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn cold_ready_validate_open_stutters_real_periodic_retry_fixture() {
    let marker = 0_u8;
    let (mut fixture, body_directory, mut body_store, durable) =
        durable_validate_store_fixture_at_view(marker, 0);
    let key = (durable.round(), durable.subject());
    let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let mut setup_callbacks = 0usize;
    let marker_outcome = body_store
        .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
            setup_callbacks = setup_callbacks.saturating_add(1);
            Ok::<_, String>(commitment)
        })
        .expect("persist the exact pre-crash validated marker");
    assert_eq!(setup_callbacks, 1);
    assert_eq!(
        marker_outcome
            .validated_receipt()
            .map(ValidatedBodyReceipt::execution_commitment),
        Some(commitment)
    );

    let keys = durable_store_keys(marker);
    let now = std::time::Instant::now();
    let runtime_directory = TempDir::new().expect("temporary cold Ready Validate runtime");
    let mut coordinator = ready_durable_validate_coordinator(&[&fixture]);
    let ledger_directory = TempDir::new().expect("temporary cold Ready Validate lifecycle ledger");
    coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .expect("attach the recovered Ready Validate LedgerV1 floor");
    let (runtime_ordinal_authority, coordinator_ordinal_authority) =
        authority::lifecycle_ordinal_authorities_after_high_watermark(coordinator.high_water());
    let lifecycle_ordinals =
        crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::from_authority(
            runtime_ordinal_authority,
        );
    coordinator
        .bind_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)
        .expect("bind the coordinator half of the cold actor-global ordinal authority");
    let (runtime, retransmit_interval, prepare_qc) = cold_ready_validate_runtime_at_durable(
        &fixture,
        &durable,
        &keys,
        runtime_directory.path(),
        "ready-parent.wal",
        now,
        lifecycle_ordinals.clone(),
    );
    let holder = take_dispatch_registry(&mut fixture);
    let payload_directory = TempDir::new().expect("temporary cold Validate payload store");
    let (payload_store, serve_payloads) =
        CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            payload_directory.path(),
            fixture.verified.context(),
        )
        .expect("open empty cold Validate Serve payload owner");
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
        timeout_supersession_successor: None,
    };
    let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    services.set_exact_output_admission_hook(|post, ticket| {
        Err(
            iroha_p2p::network::NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 1,
            },
        )
    });
    let (mut executor, mut planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
        &mut services,
        runtime,
        std::sync::Arc::clone(&output_guard),
        0,
        2,
    );
    crate::sumeragi::v2_worker::tests::install_local_signer_for_test(&mut services, &keys[0]);
    let prepare_qc_envelope = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(prepare_qc.clone()),
    );
    assert_eq!(
        services.consensus_broadcast_count_for_test(&prepare_qc_envelope),
        0
    );
    assert_eq!(
        services
            .pending_exact_prepare_qc_fanouts_for_test(&prepare_qc)
            .expect("inspect the initially empty PrepareQC output corridor"),
        (0, 0)
    );
    executor
        .arm_live_clocks(
            super::super::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            now,
        )
        .expect("arm cold Ready Validate clocks after services open");
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        vec![key],
        "the complete production census installs its sole Ready parent"
    );
    let initial_seal = executor
        .recovered_durable_validate_retry_snapshot_for_test(key)
        .expect("cold open installs one recovered Validate retry seal");
    assert_eq!(initial_seal.phase(), Some(wire::GlobalPhase::Prepare));
    assert_eq!(initial_seal.commitment_ceiling(), Some(commitment));
    assert!(executor.recovered_validate_retry_corridor_is_inert_for_test());

    assert_eq!(
        owner
            .dispatch_completion_for_test(&mut services, &mut executor, 0)
            .expect("queue the exact cold Ready Validate worker"),
        super::super::ProductionCompletionDispatchV1::ValidateQueued {
            ordinal: fixture.lease.ordinal(),
        }
    );
    let queued = planner_io.lifecycle_validate_io_snapshot();
    assert_eq!(queued.command_depth(), 1);
    assert_eq!(queued.physical_admissions(), 1);
    assert_eq!(queued.queued(), 1);
    assert_eq!(queued.active(), 0);
    let queued_registry = format!("{:?}", owner.registry.registry_for_test());
    let queued_coordinator = cold_ready_validate_logical_coordinator_snapshot(&owner.coordinator);
    let validate_ordinal = fixture.lease.ordinal();
    let queued_validate_record = owner.coordinator.records[&validate_ordinal].clone();
    let queued_validate_durable = owner.coordinator.durable_records[&validate_ordinal].clone();
    let queued_debt = {
        let status = executor.status();
        (
            status.pending_signatures,
            status.pending_fetches,
            status.pending_stores,
            status.pending_validations,
            status.pending_applications,
            status.queued_runtime_completions,
            status.effect_dispatch_queue.depth,
            status.pending_outputs,
        )
    };
    let first_due = now + retransmit_interval;
    let queued_step = executor.step(first_due, &mut services);
    let queued_observation = executor.last_runtime_step_observation_for_test();
    let queued_non_validate =
        queued_observation.and_then(|observation| observation.non_validate_class());
    let queued_trace_root = executor.last_recovered_validate_retry_trace_root_for_test();
    let queued_trace_ordinal = executor.last_recovered_validate_retry_trace_ordinal_for_test();
    let queued_observed_exact_retry = matches!(
        &queued_step,
        Ok(crate::sumeragi::v2_effects::EffectExecutorStep::Advanced { effects: 1 })
    ) && queued_observation.is_some_and(|observation| {
        observation.selected()
            == Some(crate::sumeragi::v2_runtime::RuntimeSelectedOwnerKind::PeriodicTimer)
            && observation.effect_count() == 2
            && observation.validate_count() == 1
            && observation.non_validate_class()
                == Some(crate::sumeragi::v2_effects::RuntimeEffectClassV1::Broadcast)
            && observation.sole_broadcast_is_exact_prepare_qc(&prepare_qc)
    }) && queued_trace_root.is_some()
        && queued_trace_ordinal.is_some();
    if !queued_observed_exact_retry {
        let cleanup_output =
            executor.settle_pending_lifecycle_output_admissions(&mut owner, &mut services);
        planner_io.activate_one_lifecycle_validate();
        let cleanup_callbacks = planner_io.execute_held_lifecycle_validate_fixture(
            commitment,
            std::sync::Arc::clone(&output_guard),
        );
        panic!(
            "Queued cold retry did not select one raw periodic Validate plus its exact PrepareQC: step={queued_step:?}, observation={queued_observation:?}, non_validate={queued_non_validate:?}, trace={queued_trace_root:?}, trace_ordinal={queued_trace_ordinal:?}, cleanup_output={cleanup_output:?}, cleanup_callbacks={cleanup_callbacks}"
        );
    }
    assert_eq!(planner_io.lifecycle_validate_io_snapshot(), queued);
    assert_eq!(
        services.consensus_broadcast_count_for_test(&prepare_qc_envelope),
        0,
        "raw stepping parks the remembered PrepareQC before service output"
    );
    assert_eq!(
        services
            .pending_exact_prepare_qc_fanouts_for_test(&prepare_qc)
            .expect("inspect the pre-settlement PrepareQC output corridor"),
        (0, 0)
    );
    assert_eq!(
        format!("{:?}", owner.registry.registry_for_test()),
        queued_registry
    );
    assert_eq!(
        cold_ready_validate_logical_coordinator_snapshot(&owner.coordinator),
        queued_coordinator,
        "raw output reservation may advance only the shared actor-global ordinal frontier"
    );
    let queued_post_reservation_validate_record =
        owner.coordinator.records[&validate_ordinal].clone();
    let queued_post_reservation_validate_durable =
        owner.coordinator.durable_records[&validate_ordinal].clone();
    let queued_post_reservation_high_water = owner.coordinator.high_water();
    let queued_post_reservation_ordinals = owner
        .coordinator
        .records
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let queued_after_debt = {
        let status = executor.status();
        (
            status.pending_signatures,
            status.pending_fetches,
            status.pending_stores,
            status.pending_validations,
            status.pending_applications,
            status.queued_runtime_completions,
            status.effect_dispatch_queue.depth,
            status.pending_outputs,
        )
    };
    assert_eq!(queued_after_debt.0, queued_debt.0);
    assert_eq!(queued_after_debt.1, queued_debt.1);
    assert_eq!(queued_after_debt.2, queued_debt.2);
    assert_eq!(queued_after_debt.3, queued_debt.3);
    assert_eq!(queued_after_debt.4, queued_debt.4);
    assert_eq!(queued_after_debt.5, queued_debt.5);
    assert_eq!(queued_after_debt.6, queued_debt.6);
    assert_eq!(queued_after_debt.7, 1);
    let first_output_settlement =
        executor.settle_pending_lifecycle_output_admissions(&mut owner, &mut services);
    let first_output_count = match first_output_settlement {
        Ok(count) => count,
        Err(error) => {
            planner_io.activate_one_lifecycle_validate();
            let cleanup_callbacks = planner_io.execute_held_lifecycle_validate_fixture(
                commitment,
                std::sync::Arc::clone(&output_guard),
            );
            panic!(
                "first exact PrepareQC output settlement failed: error={error:?}, cleanup_callbacks={cleanup_callbacks}"
            );
        }
    };
    assert_eq!(first_output_count, 1);
    assert_eq!(
        services.consensus_broadcast_count_for_test(&prepare_qc_envelope),
        1,
        "the first settlement services exactly its remembered PrepareQC"
    );
    assert_eq!(
        services
            .pending_exact_prepare_qc_fanouts_for_test(&prepare_qc)
            .expect("inspect the first settled PrepareQC retransmission"),
        (1, 1),
        "the first settlement installs exactly one canonical PrepareQC fanout"
    );
    assert_eq!(planner_io.lifecycle_validate_io_snapshot(), queued);
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        vec![key],
        "first output settlement cannot add or retire a recovered Validate seal"
    );
    assert_eq!(
        format!("{:?}", owner.registry.registry_for_test()),
        queued_registry,
        "the terminal direct-output row retires from the concrete registry"
    );
    assert_eq!(
        owner.coordinator.records[&validate_ordinal],
        queued_post_reservation_validate_record
    );
    assert_eq!(
        owner.coordinator.durable_records[&validate_ordinal],
        queued_post_reservation_validate_durable
    );
    assert_eq!(
        queued_post_reservation_validate_record,
        queued_validate_record
    );
    assert_eq!(
        queued_post_reservation_validate_durable,
        queued_validate_durable
    );
    assert_eq!(queued_post_reservation_high_water, validate_ordinal);
    let transient_runtime_ordinal = validate_ordinal
        .checked_add(1)
        .expect("the transient runtime output ordinal fits");
    let expected_direct_output_ordinal = transient_runtime_ordinal
        .checked_add(1)
        .expect("the direct output ordinal fits after the runtime reservation");
    assert_eq!(expected_direct_output_ordinal, 3);
    assert!(
        !owner
            .coordinator
            .records
            .contains_key(&transient_runtime_ordinal),
        "the runtime output reservation is not itself a coordinator row"
    );
    let new_output_ordinals = owner
        .coordinator
        .records
        .keys()
        .copied()
        .filter(|ordinal| !queued_post_reservation_ordinals.contains(ordinal))
        .collect::<Vec<_>>();
    assert_eq!(new_output_ordinals, vec![expected_direct_output_ordinal]);
    let direct_output_record = &owner.coordinator.records[&expected_direct_output_ordinal];
    assert_eq!(
        direct_output_record.work_class,
        LifecycleWorkClass::Broadcast
    );
    assert!(matches!(
        direct_output_record.state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    ));
    assert!(
        owner
            .coordinator
            .durable_records
            .contains_key(&expected_direct_output_ordinal),
        "the exact terminal direct output is durable before service settlement returns"
    );
    assert_eq!(
        owner.coordinator.high_water(),
        expected_direct_output_ordinal
    );
    let queued_settled_debt = {
        let status = executor.status();
        (
            status.pending_signatures,
            status.pending_fetches,
            status.pending_stores,
            status.pending_validations,
            status.pending_applications,
            status.queued_runtime_completions,
            status.effect_dispatch_queue.depth,
            status.pending_outputs,
        )
    };
    assert_eq!(queued_settled_debt, queued_debt);
    let queued_seal = executor
        .recovered_durable_validate_retry_snapshot_for_test(key)
        .expect("Queued retry retains the recovered seal");
    assert!(queued_seal.same_owner(&initial_seal));
    assert!(queued_seal.effect_tag() >= initial_seal.effect_tag());
    assert_eq!(queued_seal.phase(), initial_seal.phase());
    assert_eq!(queued_seal.commitment_ceiling(), Some(commitment));
    let queued_trace_root = queued_trace_root
        .expect("the exact raw periodic Validate records its authenticated trace root");
    let queued_trace_ordinal = queued_trace_ordinal
        .expect("the exact raw periodic Validate records its actor-global position");
    assert_ne!(queued_trace_root, initial_seal.causal_lifecycle_key());
    assert_eq!(queued_trace_ordinal, transient_runtime_ordinal);
    assert!(executor.recovered_validate_retry_corridor_is_inert_for_test());
    assert!(!output_guard.restart_required());

    planner_io.activate_one_lifecycle_validate();
    let active = planner_io.lifecycle_validate_io_snapshot();
    assert_eq!(active.command_depth(), 0);
    assert_eq!(active.physical_admissions(), 0);
    assert_eq!(active.queued(), 0);
    assert_eq!(active.active(), 1);
    let active_registry = format!("{:?}", owner.registry.registry_for_test());
    let active_coordinator = cold_ready_validate_logical_coordinator_snapshot(&owner.coordinator);
    let active_debt = {
        let status = executor.status();
        (
            status.pending_signatures,
            status.pending_fetches,
            status.pending_stores,
            status.pending_validations,
            status.pending_applications,
            status.queued_runtime_completions,
            status.effect_dispatch_queue.depth,
            status.pending_outputs,
        )
    };
    assert_eq!(active_debt, queued_debt);
    let second_due = first_due + retransmit_interval;
    let active_step = executor.step(second_due, &mut services);
    let active_observation = executor.last_runtime_step_observation_for_test();
    let active_non_validate =
        active_observation.and_then(|observation| observation.non_validate_class());
    let active_trace_root = executor.last_recovered_validate_retry_trace_root_for_test();
    let active_trace_ordinal = executor.last_recovered_validate_retry_trace_ordinal_for_test();
    let active_observed_exact_retry = matches!(
        &active_step,
        Ok(crate::sumeragi::v2_effects::EffectExecutorStep::Advanced { effects: 1 })
    ) && active_observation.is_some_and(|observation| {
        observation.selected()
            == Some(crate::sumeragi::v2_runtime::RuntimeSelectedOwnerKind::PeriodicTimer)
            && observation.effect_count() == 2
            && observation.validate_count() == 1
            && observation.non_validate_class()
                == Some(crate::sumeragi::v2_effects::RuntimeEffectClassV1::Broadcast)
            && observation.sole_broadcast_is_exact_prepare_qc(&prepare_qc)
    }) && active_trace_root.is_some()
        && active_trace_ordinal.is_some_and(|ordinal| ordinal > queued_trace_ordinal);
    if !active_observed_exact_retry {
        let cleanup_output =
            executor.settle_pending_lifecycle_output_admissions(&mut owner, &mut services);
        let cleanup_callbacks = planner_io.execute_held_lifecycle_validate_fixture(
            commitment,
            std::sync::Arc::clone(&output_guard),
        );
        panic!(
            "Active cold retry did not select one fresh raw periodic Validate plus its exact PrepareQC: step={active_step:?}, observation={active_observation:?}, non_validate={active_non_validate:?}, trace={active_trace_root:?}, trace_ordinal={active_trace_ordinal:?}, queued_trace={queued_trace_root:?}, queued_trace_ordinal={queued_trace_ordinal}, cleanup_output={cleanup_output:?}, cleanup_callbacks={cleanup_callbacks}"
        );
    }
    assert_eq!(planner_io.lifecycle_validate_io_snapshot(), active);
    assert_eq!(
        services.consensus_broadcast_count_for_test(&prepare_qc_envelope),
        1,
        "the second raw step parks its PrepareQC before duplicate settlement"
    );
    assert_eq!(
        services
            .pending_exact_prepare_qc_fanouts_for_test(&prepare_qc)
            .expect("inspect output before duplicate PrepareQC settlement"),
        (1, 1),
        "the incumbent PrepareQC fanout remains the sole physical output owner"
    );
    assert_eq!(
        format!("{:?}", owner.registry.registry_for_test()),
        active_registry
    );
    assert_eq!(
        cold_ready_validate_logical_coordinator_snapshot(&owner.coordinator),
        active_coordinator,
        "the second raw reservation may advance only the shared actor-global ordinal frontier"
    );
    let active_post_reservation_coordinator = format!("{:?}", owner.coordinator);
    let active_after_debt = {
        let status = executor.status();
        (
            status.pending_signatures,
            status.pending_fetches,
            status.pending_stores,
            status.pending_validations,
            status.pending_applications,
            status.queued_runtime_completions,
            status.effect_dispatch_queue.depth,
            status.pending_outputs,
        )
    };
    assert_eq!(active_after_debt.0, active_debt.0);
    assert_eq!(active_after_debt.1, active_debt.1);
    assert_eq!(active_after_debt.2, active_debt.2);
    assert_eq!(active_after_debt.3, active_debt.3);
    assert_eq!(active_after_debt.4, active_debt.4);
    assert_eq!(active_after_debt.5, active_debt.5);
    assert_eq!(active_after_debt.6, active_debt.6);
    assert_eq!(active_after_debt.7, 1);
    let duplicate_output_settlement =
        executor.settle_pending_lifecycle_output_admissions(&mut owner, &mut services);
    let duplicate_output_count = match duplicate_output_settlement {
        Ok(count) => count,
        Err(error) => {
            let cleanup_callbacks = planner_io.execute_held_lifecycle_validate_fixture(
                commitment,
                std::sync::Arc::clone(&output_guard),
            );
            panic!(
                "duplicate PrepareQC output settlement failed: error={error:?}, cleanup_callbacks={cleanup_callbacks}"
            );
        }
    };
    assert_eq!(duplicate_output_count, 1);
    assert_eq!(
        services.consensus_broadcast_count_for_test(&prepare_qc_envelope),
        1,
        "terminal direct-output deduplication cannot call the service twice"
    );
    assert_eq!(
        services
            .pending_exact_prepare_qc_fanouts_for_test(&prepare_qc)
            .expect("inspect the deduplicated PrepareQC output owner"),
        (1, 1),
        "terminal deduplication preserves exactly one byte-identical fanout"
    );
    assert_eq!(planner_io.lifecycle_validate_io_snapshot(), active);
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        vec![key],
        "duplicate output settlement cannot add or retire a recovered Validate seal"
    );
    assert_eq!(
        format!("{:?}", owner.registry.registry_for_test()),
        active_registry
    );
    assert_eq!(
        format!("{:?}", owner.coordinator),
        active_post_reservation_coordinator,
        "terminal duplicate settlement cannot mutate the post-reservation coordinator"
    );
    let active_settled_debt = {
        let status = executor.status();
        (
            status.pending_signatures,
            status.pending_fetches,
            status.pending_stores,
            status.pending_validations,
            status.pending_applications,
            status.queued_runtime_completions,
            status.effect_dispatch_queue.depth,
            status.pending_outputs,
        )
    };
    assert_eq!(active_settled_debt, active_debt);
    let active_seal = executor
        .recovered_durable_validate_retry_snapshot_for_test(key)
        .expect("Active retry retains the recovered seal");
    assert!(active_seal.same_owner(&initial_seal));
    assert!(active_seal.effect_tag() >= queued_seal.effect_tag());
    assert_eq!(active_seal.phase(), initial_seal.phase());
    assert_eq!(active_seal.commitment_ceiling(), Some(commitment));
    let active_trace_root = active_trace_root
        .expect("second raw periodic Validate retains its authenticated trace root");
    let active_trace_ordinal = active_trace_ordinal
        .expect("second raw periodic Validate retains its actor-global position");
    assert_ne!(active_trace_root, initial_seal.causal_lifecycle_key());
    assert_eq!(active_trace_root, queued_trace_root);
    assert_eq!(
        active_trace_ordinal,
        expected_direct_output_ordinal
            .checked_add(1)
            .expect("the second periodic owner fits after direct output")
    );
    assert!(executor.recovered_validate_retry_corridor_is_inert_for_test());
    assert!(!output_guard.restart_required());

    assert_eq!(
        planner_io.execute_held_lifecycle_validate_fixture(
            commitment,
            std::sync::Arc::clone(&output_guard),
        ),
        0,
        "the persisted marker bypasses the validator callback"
    );
    let completion_pending = planner_io.lifecycle_validate_io_snapshot();
    assert_eq!(completion_pending.active(), 0);
    assert_eq!(completion_pending.completion_pending(), 1);
    assert_eq!(completion_pending.completion_owners(), 1);
    let completion = match services
        .take_next_lifecycle_completion()
        .expect("take the exact guarded cold Validate completion")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::Validate(completion) => completion,
        _ => panic!("cold Validate worker published a foreign completion class"),
    };
    let (executed, ack) = completion.into_publication_parts();
    let published = owner
        .coordinator
        .complete_durable_validate_dispatch(&mut owner.registry, executed)
        .expect("publish the sole marker-backed lifecycle Validate completion");
    let super::super::DurableValidateCompletionPublication::PublishedValidated(published) =
        published
    else {
        panic!("persisted validated marker must publish one validated replacement")
    };
    assert_eq!(published.lifecycle_ordinal(), fixture.lease.ordinal());
    drop(published);
    ack.acknowledge_after_publication();
    let settled_worker = planner_io.lifecycle_validate_io_snapshot();
    assert_eq!(settled_worker.command_depth(), 0);
    assert_eq!(settled_worker.physical_admissions(), 0);
    assert_eq!(settled_worker.queued(), 0);
    assert_eq!(settled_worker.active(), 0);
    assert_eq!(settled_worker.completion_pending(), 0);
    assert_eq!(settled_worker.completion_owners(), 0);

    let replacement_digest = owner.registry.registry_for_test().entries[&fixture.address].digest;
    let mut terminal_lease = fixture.lease.clone();
    assert_eq!(
        terminal_lease
            .physical_slots
            .insert(fixture.slot, replacement_digest),
        Some(fixture.lease.physical_slots()[&fixture.slot])
    );
    let ordinal = terminal_lease.ordinal();
    owner.coordinator.ready_index.remove(&ordinal);
    owner
        .coordinator
        .records
        .get_mut(&ordinal)
        .expect("published Validate replacement retains its logical row")
        .state = LifecycleState::Claimed(terminal_lease.id());
    owner.coordinator.active_lease = Some(terminal_lease.clone());
    assert!(
        owner
            .registry
            .registry_for_test_mut()
            .entries
            .remove(&fixture.address)
            .is_some()
    );
    owner.coordinator.settle_turn(
        terminal_lease,
        super::super::TurnOutcome::Terminal(TerminalOutcome::Cancelled),
    );
    assert!(owner.coordinator.active_lease.is_none());
    assert!(matches!(
        owner.coordinator.records[&ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Cancelled)
    ));
    let terminal_census = owner
        .registry
        .project_recovered_durable_validate_retry_census(&owner.coordinator, None)
        .expect("terminal Validate parent projects an empty retry census");
    assert_eq!(terminal_census.len_for_test(), 0);
    let body_store = planner_io.detach_with_body_store(&mut services);
    drop(executor);

    let replay_now = second_due + retransmit_interval;
    let (replay_runtime, _replay_interval, _) = cold_ready_validate_runtime_at_durable(
        &fixture,
        &durable,
        &keys,
        runtime_directory.path(),
        "terminal-child.wal",
        replay_now,
        lifecycle_ordinals,
    );
    let replay_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (replay_executor, body_store) =
        crate::sumeragi::v2_effects::V2EffectExecutor::open_with_body_store(
            replay_runtime,
            body_store,
            terminal_census,
            None,
            fixture.verified.context().clone(),
            fixture.verified.context().roster[0].validator.clone(),
            Some(0),
            std::sync::Arc::clone(&replay_guard),
            crate::sumeragi::v2_effects::EffectQueueConfig::default(),
        )
        .expect("reopen terminal Validate marker without deferral");
    assert!(
        replay_executor
            .recovered_durable_validate_retry_keys_for_test()
            .is_empty(),
        "terminal marker replay cannot reinstall the retired parent seal"
    );
    assert!(
        replay_executor.recovered_validated_body_was_bound_for_test(key),
        "the real executor open must route a terminal/no-owner marker into its replacement runtime"
    );
    assert!(replay_executor.lifecycle_live_clocks_are_unarmed());
    assert!(replay_executor.recovered_validate_retry_corridor_is_inert_for_test());
    let replay_status = replay_executor.status();
    assert!(!replay_status.fail_closed);
    assert!(replay_status.fatal_reason.is_none());
    assert_eq!(replay_status.pending_signatures, 0);
    assert_eq!(replay_status.pending_fetches, 0);
    assert_eq!(replay_status.pending_stores, 0);
    assert_eq!(replay_status.pending_validations, 0);
    assert_eq!(replay_status.pending_applications, 0);
    assert_eq!(replay_status.pending_outputs, 0);
    assert_eq!(replay_status.queued_runtime_completions, 0);
    assert_eq!(replay_status.effect_dispatch_queue.depth, 0);
    assert!(!replay_guard.restart_required());
    drop(replay_executor);
    drop(body_store);
    drop(body_directory);
}

#[cfg(feature = "bls")]
fn plural_recovered_ready_validate_store_fixture(
    marker: u8,
    views: &[wire::View],
) -> (
    Vec<DurableValidateFixture>,
    TempDir,
    V2BodyStore,
    Vec<DurableBodyReceipt>,
    LifecycleCoordinator,
) {
    assert!(views.len() >= 2);
    let mut fixtures = views
        .iter()
        .map(|&view| durable_validate_fixture_at_view(marker, view))
        .collect::<Vec<_>>();
    let first_ordinal = fixtures[0].lease.ordinal();
    for (offset, fixture) in fixtures.iter_mut().enumerate().skip(1) {
        let ordinal = first_ordinal
            .checked_add(u128::try_from(offset).expect("plural Validate offset fits u128"))
            .expect("plural Validate ordinal fits u128");
        readdress_durable_validate_fixture(fixture, ordinal);
    }
    let directory = TempDir::new().expect("temporary plural Ready Validate body store");
    let mut store = V2BodyStore::open(directory.path(), fixtures[0].verified.context().clone())
        .expect("open plural Ready Validate body store");
    let mut persisted = Vec::with_capacity(fixtures.len());
    let mut durables = Vec::with_capacity(fixtures.len());
    for fixture in fixtures {
        let (fixture, durable) =
            persist_durable_validate_fixture_into_store(fixture, &mut store, None);
        persisted.push(fixture);
        durables.push(durable);
    }
    let coordinator = ready_durable_validate_coordinator(&persisted.iter().collect::<Vec<_>>());
    let mut combined_registry = core::mem::take(&mut persisted[0].registry);
    for fixture in persisted.iter_mut().skip(1) {
        combined_registry
            .entries
            .append(&mut fixture.registry.entries);
    }
    persisted[0].registry = combined_registry;
    (persisted, directory, store, durables, coordinator)
}

#[cfg(feature = "bls")]
fn persist_exact_recovered_validate_marker(
    store: &mut V2BodyStore,
    durable: &DurableBodyReceipt,
) -> ValidatedBodyReceipt {
    let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let mut callbacks = 0usize;
    let outcome = store
        .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
            callbacks = callbacks.saturating_add(1);
            Ok::<_, String>(commitment)
        })
        .expect("persist plural Ready Validate marker");
    assert_eq!(callbacks, 1);
    outcome
        .validated_receipt()
        .cloned()
        .expect("plural Ready Validate marker is validated")
}

#[cfg(feature = "bls")]
fn regular_file_state_below(
    root: &std::path::Path,
) -> BTreeMap<std::path::PathBuf, (Vec<u8>, u32)> {
    fn permission_projection(metadata: &std::fs::Metadata) -> u32 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            metadata.permissions().mode()
        }
        #[cfg(not(unix))]
        {
            u32::from(metadata.permissions().readonly())
        }
    }

    fn visit(
        root: &std::path::Path,
        directory: &std::path::Path,
        files: &mut BTreeMap<std::path::PathBuf, (Vec<u8>, u32)>,
    ) {
        for entry in std::fs::read_dir(directory).expect("read body-store snapshot directory") {
            let entry = entry.expect("read body-store snapshot entry");
            let file_type = entry.file_type().expect("read body-store entry type");
            let path = entry.path();
            if file_type.is_dir() {
                visit(root, &path, files);
            } else if file_type.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .expect("body-store snapshot entry remains below its root")
                    .to_path_buf();
                let metadata = entry.metadata().expect("read body-store snapshot metadata");
                assert!(
                    files
                        .insert(
                            relative,
                            (
                                std::fs::read(&path).expect("read body-store snapshot bytes"),
                                permission_projection(&metadata),
                            ),
                        )
                        .is_none()
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[cfg(feature = "bls")]
fn empty_recovered_validate_runtime_for_test(
    fixture: &DurableValidateFixture,
    root: &std::path::Path,
    wal_name: &str,
) -> crate::sumeragi::v2_runtime::SerializedV2Runtime {
    let AdapterEffect::ValidateBody { tag, .. } = fixture.effect else {
        unreachable!("recovered Validate fixture retains one Validate effect")
    };
    let (adapter, startup) = SumeragiV2Adapter::open(
        root.join(wal_name),
        fixture.verified.clone(),
        Some(0),
        tag.generation(),
        [0xC7; 32],
        AdapterFingerprints {
            node: Hash::new(b"plural recovered Validate node"),
            build: Hash::new(b"plural recovered Validate build"),
            config: Hash::new(b"plural recovered Validate config"),
        },
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("open empty recovered Validate adapter");
    assert!(startup.is_empty());
    let (runtime, startup) = crate::sumeragi::v2_runtime::SerializedV2Runtime::new(
        adapter,
        startup,
        std::time::Instant::now(),
        std::time::Duration::from_secs(10),
        crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap empty recovered Validate runtime");
    assert!(startup.is_empty());
    assert!(!runtime.lifecycle_live_clocks_are_armed());
    runtime
}

#[cfg(feature = "bls")]
#[test]
fn recovered_ready_validate_plural_open_installs_and_reconciles_atomically() {
    let handle = std::thread::Builder::new()
        .name("recovered-ready-validate-plural-open".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(recovered_ready_validate_plural_open_fixture)
        .expect("spawn plural recovered Ready Validate open fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
fn recovered_ready_validate_plural_open_fixture() {
    let (mut fixtures, body_directory, mut body_store, durables, mut coordinator) =
        plural_recovered_ready_validate_store_fixture(0x35, &[2, 3, 4]);
    let markers = durables
        .iter()
        .map(|durable| persist_exact_recovered_validate_marker(&mut body_store, durable))
        .collect::<Vec<_>>();
    assert_eq!(
        body_store
            .recovery_catalog()
            .expect("read the shared plural durable catalog")
            .len(),
        3
    );
    assert_eq!(body_store.validated_recovery_catalog().len(), 3);
    let terminal_lease = fixtures[0].lease.clone();
    let terminal_ordinal = terminal_lease.ordinal();
    coordinator.ready_index.remove(&terminal_ordinal);
    coordinator
        .records
        .get_mut(&terminal_ordinal)
        .expect("plural terminal parent retains its row")
        .state = LifecycleState::Claimed(terminal_lease.id());
    coordinator.active_lease = Some(terminal_lease.clone());
    coordinator.settle_turn(
        terminal_lease,
        super::super::TurnOutcome::Terminal(TerminalOutcome::Cancelled),
    );
    assert!(coordinator.active_lease.is_none());
    assert!(matches!(
        coordinator.records[&terminal_ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Cancelled)
    ));
    let terminal_address = fixtures[0].address;
    assert!(
        fixtures[0]
            .registry
            .entries
            .remove(&terminal_address)
            .is_some()
    );
    let census = fixtures[0]
        .registry
        .project_recovered_durable_validate_retry_census(&coordinator, None)
        .expect("project both surviving Ready Validate owners");
    assert_eq!(census.len_for_test(), 2);

    let runtime_directory = TempDir::new().expect("temporary plural Validate runtime");
    let runtime = empty_recovered_validate_runtime_for_test(
        &fixtures[0],
        runtime_directory.path(),
        "plural-open.wal",
    );
    let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut executor, body_store) =
        crate::sumeragi::v2_effects::V2EffectExecutor::open_with_body_store(
            runtime,
            body_store,
            census,
            None,
            fixtures[0].verified.context().clone(),
            fixtures[0].verified.context().roster[0].validator.clone(),
            Some(0),
            std::sync::Arc::clone(&output_guard),
            crate::sumeragi::v2_effects::EffectQueueConfig::default(),
        )
        .expect("consume the plural Ready Validate census during real executor open");
    let terminal_key = (durables[0].round(), durables[0].subject());
    let mut ready_keys = durables[1..]
        .iter()
        .map(|durable| (durable.round(), durable.subject()))
        .collect::<Vec<_>>();
    ready_keys.sort_unstable();
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        ready_keys
    );
    let first_ready_snapshot = executor
        .recovered_durable_validate_retry_snapshot_for_test(ready_keys[0])
        .expect("first Ready key retains one recovered owner");
    let second_ready_snapshot = executor
        .recovered_durable_validate_retry_snapshot_for_test(ready_keys[1])
        .expect("second Ready key retains one recovered owner");
    assert_ne!(
        first_ready_snapshot.causal_lifecycle_key(),
        second_ready_snapshot.causal_lifecycle_key()
    );
    assert!(!first_ready_snapshot.same_owner(&second_ready_snapshot));
    assert_eq!(
        first_ready_snapshot.commitment_ceiling(),
        Some(markers[1].execution_commitment())
    );
    assert_eq!(
        second_ready_snapshot.commitment_ceiling(),
        Some(markers[2].execution_commitment())
    );
    assert_ne!(
        first_ready_snapshot.commitment_ceiling(),
        second_ready_snapshot.commitment_ceiling()
    );
    assert!(executor.recovered_validated_body_was_bound_for_test(terminal_key));
    for key in &ready_keys {
        assert!(
            !executor.recovered_validated_body_was_bound_for_test(*key),
            "a still-Ready marker must remain deferred to its lifecycle worker"
        );
    }
    assert!(executor.lifecycle_live_clocks_are_unarmed());
    assert!(executor.recovered_validate_retry_corridor_is_inert_for_test());
    assert!(!output_guard.restart_required());

    let selected = (
        durables[1].round(),
        durables[1].round(),
        durables[1].subject(),
        markers[1].execution_commitment(),
    );
    let selected_key = (selected.1, selected.2);
    let selected_before = executor
        .recovered_durable_validate_retry_snapshot_for_test(selected_key)
        .expect("selected Ready key retains its pre-Decision recovered owner");
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    executor
        .reconcile_recovered_validate_retry_decision_for_test(selected, false, &mut services)
        .expect("Decision cleanup retains only the selected recovered retry seal");
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        vec![selected_key]
    );
    let selected_after = executor
        .recovered_durable_validate_retry_snapshot_for_test(selected_key)
        .expect("selected Decision retains its recovered owner");
    assert!(selected_after.same_owner(&selected_before));
    assert_eq!(
        selected_after.commitment_ceiling(),
        selected_before.commitment_ceiling()
    );
    executor
        .reconcile_recovered_validate_retry_decision_for_test(selected, true, &mut services)
        .expect("terminal Decision cleanup drains the selected recovered retry seal");
    assert!(
        executor
            .recovered_durable_validate_retry_keys_for_test()
            .is_empty()
    );
    assert!(executor.recovered_validate_retry_corridor_is_inert_for_test());
    assert!(!output_guard.restart_required());
    drop(executor);
    drop(body_store);
    drop(runtime_directory);
    drop(body_directory);
}

#[cfg(feature = "bls")]
#[test]
fn recovered_ready_validate_plural_late_corruption_installs_nothing() {
    let handle = std::thread::Builder::new()
        .name("recovered-ready-validate-plural-atomic-failure".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(recovered_ready_validate_plural_late_corruption_fixture)
        .expect("spawn plural recovered Ready Validate atomic-failure fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
fn recovered_ready_validate_plural_late_corruption_fixture() {
    let (fixtures, body_directory, mut body_store, durables, coordinator) =
        plural_recovered_ready_validate_store_fixture(0x36, &[2, 3]);
    let first_marker = persist_exact_recovered_validate_marker(&mut body_store, &durables[0]);
    let first_key = (durables[0].round(), durables[0].subject());
    let second_key = (durables[1].round(), durables[1].subject());
    assert!(first_key < second_key);
    let baseline_recovery = body_store
        .recovery_catalog()
        .expect("capture plural durable recovery catalog");
    let baseline_validated = body_store.validated_recovery_catalog();
    let baseline_rejected = body_store.rejected_recovery_catalog();
    let baseline_retired = body_store.retired_rejected_recovery_catalog();
    assert_eq!(baseline_recovery.len(), 2);
    assert_eq!(baseline_validated.len(), 1);
    let baseline_files = regular_file_state_below(body_directory.path());

    let mut direct_census = fixtures[0]
        .registry
        .project_recovered_durable_validate_retry_census(&coordinator, None)
        .expect("project exact plural census for the direct atomic sink");
    assert_eq!(direct_census.len_for_test(), 2);
    assert_eq!(
        direct_census.classify_and_bind_validated_marker(first_key, &first_marker),
        Ok(true)
    );
    assert!(direct_census.corrupt_last_durable_receipt_for_test(durables[0].clone()));
    let mut open_census = fixtures[0]
        .registry
        .project_recovered_durable_validate_retry_census(&coordinator, None)
        .expect("project exact plural census for the by-value open");
    assert!(open_census.corrupt_last_durable_receipt_for_test(durables[0].clone()));

    let runtime_directory = TempDir::new().expect("temporary plural failure runtime");
    let direct_runtime = empty_recovered_validate_runtime_for_test(
        &fixtures[0],
        runtime_directory.path(),
        "plural-direct-install.wal",
    );
    let direct_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut direct_executor, body_store) =
        crate::sumeragi::v2_effects::V2EffectExecutor::open_with_body_store(
            direct_runtime,
            body_store,
            RecoveredDurableValidateRetryCensusV1::empty_for_test(),
            None,
            fixtures[0].verified.context().clone(),
            fixtures[0].verified.context().roster[0].validator.clone(),
            Some(0),
            std::sync::Arc::clone(&direct_guard),
            crate::sumeragi::v2_effects::EffectQueueConfig::default(),
        )
        .expect("open exact catalogs before the direct plural installation test");
    let direct_status_before = direct_executor.status();
    assert!(direct_executor.recovered_validate_retry_corridor_is_inert_for_test());
    assert!(!direct_guard.restart_required());
    assert!(matches!(
        direct_census.install_into_executor(&mut direct_executor),
        Err(crate::sumeragi::v2_effects::EffectExecutorError::Contract(reason))
            if reason.contains("cold Validate retry owner disagreed")
    ));
    assert!(
        direct_executor
            .recovered_durable_validate_retry_keys_for_test()
            .is_empty(),
        "a later invalid owner cannot publish the earlier prepared seal"
    );
    let mut direct_status_after = direct_executor.status();
    direct_status_after.captured_at = direct_status_before.captured_at;
    assert_eq!(direct_status_after, direct_status_before);
    assert!(direct_executor.recovered_validate_retry_corridor_is_inert_for_test());
    assert!(!direct_guard.restart_required());
    assert_eq!(
        body_store
            .recovery_catalog()
            .expect("recheck durable catalog after direct install rejection"),
        baseline_recovery
    );
    assert_eq!(body_store.validated_recovery_catalog(), baseline_validated);
    assert_eq!(body_store.rejected_recovery_catalog(), baseline_rejected);
    assert_eq!(
        body_store.retired_rejected_recovery_catalog(),
        baseline_retired
    );
    assert_eq!(
        regular_file_state_below(body_directory.path()),
        baseline_files,
        "direct atomic-sink rejection cannot mutate persistent body-store bytes or modes"
    );
    drop(direct_executor);

    let open_runtime = empty_recovered_validate_runtime_for_test(
        &fixtures[0],
        runtime_directory.path(),
        "plural-failing-open.wal",
    );
    let open_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let open_error = match crate::sumeragi::v2_effects::V2EffectExecutor::open_with_body_store(
        open_runtime,
        body_store,
        open_census,
        None,
        fixtures[0].verified.context().clone(),
        fixtures[0].verified.context().roster[0].validator.clone(),
        Some(0),
        std::sync::Arc::clone(&open_guard),
        crate::sumeragi::v2_effects::EffectQueueConfig::default(),
    ) {
        Ok(_) => panic!("a late invalid plural owner cannot cross real executor open"),
        Err(error) => error,
    };
    assert!(matches!(
        open_error,
        crate::sumeragi::v2_effects::EffectExecutorError::Contract(reason)
            if reason.contains("cold Validate retry owner disagreed")
    ));
    assert!(open_guard.restart_required());
    assert_eq!(
        regular_file_state_below(body_directory.path()),
        baseline_files,
        "failed by-value open cannot mutate persistent body-store bytes or modes"
    );

    let mut reopened = V2BodyStore::open(
        body_directory.path(),
        fixtures[0].verified.context().clone(),
    )
    .expect("reopen catalogs after the failed by-value executor open");
    reopened
        .revalidate_recovered_markers(|_| Ok::<_, String>(first_marker.execution_commitment()))
        .expect("revalidate the unchanged plural marker catalog");
    assert_eq!(
        reopened
            .recovery_catalog()
            .expect("read durable catalog after failed plural open"),
        baseline_recovery
    );
    assert_eq!(reopened.validated_recovery_catalog(), baseline_validated);
    assert_eq!(reopened.rejected_recovery_catalog(), baseline_rejected);
    assert_eq!(
        reopened.retired_rejected_recovery_catalog(),
        baseline_retired
    );
    drop(reopened);
    drop(runtime_directory);
    drop(body_directory);
}

#[cfg(feature = "bls")]
#[test]
fn local_proposal_intent_live_wal_sign_is_typed_dispatched_once_and_prepares_successors() {
    let handle = std::thread::Builder::new()
        .name("local-proposal-intent-live-wal-sign".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(local_proposal_intent_live_wal_sign_fixture)
        .expect("spawn local ProposalIntent live-WAL Sign fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn local_proposal_intent_live_wal_sign_fixture() {
    let marker = 0xDE;
    let ReadyDurableValidateFixture {
        fixture,
        _directory,
        mut holder,
        lease,
        durable: _,
    } = ready_local_durable_validate_fixture_at_view(
        marker,
        0,
        ReadyDurableValidateFixtureOutcome::Validated,
    );
    let AdapterEffect::ValidateBody { tag, .. } = &fixture.effect else {
        unreachable!("local ProposalIntent fixture retains one Validate effect")
    };
    let tag = *tag;
    let prepared = holder
        .registry_for_test_mut()
        .prepare_ready_durable_validate_execution(&lease, fixture.slot, &fixture.verified)
        .expect("prepare exact local validated carrier");
    let local = prepared
        .project_local_proposal_ready()
        .expect("local validated carrier projects its exact runtime handoff");

    let adapter_directory = TempDir::new().expect("temporary local ProposalIntent WAL");
    let wal_path = adapter_directory.path().join("safety.wal");
    let local_validator = fixture.verified.context().leader(0);
    let (adapter, startup) = SumeragiV2Adapter::open(
        &wal_path,
        fixture.verified.clone(),
        Some(local_validator),
        tag.generation(),
        [marker; 32],
        AdapterFingerprints {
            node: Hash::new(b"live local ProposalIntent node"),
            build: Hash::new(b"live local ProposalIntent build"),
            config: Hash::new(b"live local ProposalIntent config"),
        },
        DeferredAdmissionOrdinalSource::new(
            lease
                .ordinal()
                .checked_add(1)
                .expect("local Proposal lifecycle ordinal remains representable"),
        ),
    )
    .expect("open local ProposalIntent adapter");
    assert!(startup.is_empty());
    let now = std::time::Instant::now();
    let lifecycle_ordinals =
        crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
            lease.ordinal(),
        );
    let lifecycle_ordinal_observer = lifecycle_ordinals.clone();
    let (mut runtime, startup) =
        crate::sumeragi::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            adapter,
            startup,
            now,
            std::time::Duration::from_secs(10),
            crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
            lifecycle_ordinals,
        )
        .expect("wrap local ProposalIntent adapter");
    assert!(startup.is_empty());
    runtime
        .arm_live_clocks(now)
        .expect("arm local ProposalIntent runtime clocks");
    let local_publication = (
        local
            .command_identity()
            .expect("local ProposalReady handoff retains its exact runtime identity"),
        local.lifecycle_ordinal(),
    );
    assert!(matches!(
        runtime
            .preflight_ready_durable_validate_adapter_publication(
                &prepared,
                Some(local_publication),
            )
            .expect("preflight the exact local Ready Validate publication"),
        ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
            | ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect
    ));
    let published = local
        .publish_into_runtime(&mut runtime)
        .unwrap_or_else(|_| panic!("publish exact local-proposal runtime handoff"));
    let physical_admission_ordinal = lease
        .ordinal()
        .checked_add(1)
        .expect("local Proposal lifecycle successor remains representable");
    assert_eq!(
        lifecycle_ordinal_observer
            .next_ordinal_for_test()
            .expect("inspect shared local Proposal lifecycle ordinal"),
        Some(
            physical_admission_ordinal
                .checked_add(1)
                .expect("local Proposal physical successor remains representable")
        )
    );
    let (command_identity, ready_replay) = published.into_entry();
    let crate::sumeragi::v2_runtime::RuntimeStep::Advanced(effects) = runtime
        .step(now)
        .expect("execute the exact local ProposalReady command")
    else {
        panic!("local ProposalReady command unexpectedly idled")
    };
    let scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("local ProposalReady publishes scheduler ownership");
    let crate::sumeragi::v2_runtime::RuntimeSelectedCandidateOwnership::Exact(candidate) =
        scheduler.candidate
    else {
        panic!("local ProposalReady scheduler retains one exact FIFO candidate")
    };
    assert_eq!(
        candidate.kind,
        crate::sumeragi::v2_runtime::RuntimeCommandKind::LocalProposalReady
    );
    assert_eq!(candidate.lifecycle_ordinal, lease.ordinal());
    assert_eq!(candidate.admission_ordinal, physical_admission_ordinal);
    let ownership = runtime
        .take_effect_ownership(effects.len())
        .expect("local ProposalIntent retains positional effect ownership");
    let [proposal_ownership] = ownership.as_slice() else {
        panic!("local ProposalIntent must emit one owned Sign")
    };
    let [proposal_effect] = effects.as_slice() else {
        panic!("local ProposalIntent must emit one Sign")
    };
    assert!(matches!(
        proposal_effect,
        AdapterEffect::Sign {
            request: SignRequest::Proposal(proposal),
            ..
        } if proposal.signature.is_empty()
    ));
    let handoff = runtime
        .take_live_proposal_intent_wal_sign(&effects)
        .expect("consume exact post-fsync ProposalIntent sidecar")
        .expect("local ProposalIntent emits one WAL Sign sidecar");
    let intent = ready_replay
        .bind_proposal_intent(command_identity, proposal_effect, proposal_ownership)
        .expect("local replay lineage binds the exact ProposalIntent");
    let pending = handoff
        .join_local_proposal(intent)
        .unwrap_or_else(|_| panic!("join the local lineage to its live WAL Sign"));
    let mut context_id = [0_u8; 32];
    context_id.copy_from_slice(fixture.verified.context().id().0.as_ref());
    let prepared = pending
        .prepare(
            LifecycleContext::new(
                LifecycleDigest::new(context_id),
                fixture.verified.context().height,
            ),
            &fixture.verified,
        )
        .unwrap_or_else(|_| panic!("prepare exact local live-WAL admission"));
    let active_context = LifecycleContext::new(
        prepared.candidate.key.context(),
        prepared.candidate.key.round().height(),
    );
    let mut coordinator = LifecycleCoordinator::new(
        active_context,
        0,
        CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 64))),
    );
    let ledger_directory = TempDir::new().expect("temporary local Proposal Sign ledger");
    coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .expect("attach local Proposal Sign LedgerV1");
    let mut registry = LifecycleWorkRegistryHolder::empty();
    let super::super::concrete_admission::AdapterEffectAdmissionTransaction::Admitted(
        AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal: None,
        },
    ) = coordinator.admit_prepared_lifecycle(&mut registry, prepared)
    else {
        panic!("local ProposalIntent must install one new Ready SignProposal")
    };
    let record = coordinator.records[&ordinal].clone();
    assert_eq!(record.owner, owner);
    assert_eq!(record.work_class, LifecycleWorkClass::SignProposal);
    assert_eq!(record.stage.kind(), LifecycleStageKind::SignProposal);
    let (&slot, &digest) = record
        .physical_slots
        .first_key_value()
        .expect("local Proposal Sign owns one Effect slot");
    let address = ConcreteWorkAddress::new(owner, ordinal, slot)
        .expect("local Proposal Sign has one concrete address");
    let installed = &registry.registry_for_test().entries[&address];
    assert_eq!(installed.digest, digest);
    assert!(matches!(
        &installed.kind,
        ConcreteLifecycleWorkKind::DurableLiveWalSign(sign)
            if matches!(&sign.origin, DurableLiveWalSignOriginV1::LocalProposal)
                && sign.dispatch_key.is_none()
    ));
    assert!(!matches!(
        &installed.kind,
        ConcreteLifecycleWorkKind::PendingAdapter { .. }
    ));
    let _ = registry
        .registry_for_test()
        .attest_ready_recovered_lifecycle_sign(&coordinator, ordinal)
        .expect("local Proposal Sign has one typed Ready attestation");

    let sign_lease = TurnLease {
        id: LeaseId(1),
        ordinal,
        owner,
        key: record.key,
        work_class: record.work_class,
        stage: record.stage,
        rank: super::super::SchedulerRank::new(0, 0, 0, 0, 0, 0, 0, 0),
        physical_slots: record.physical_slots,
        output_reservation: Some(super::super::schema::LeaseCapacityReservation::new(
            CapacityClass::Consensus,
            0,
        )),
    };
    coordinator.ready_index.remove(&ordinal);
    coordinator
        .records
        .get_mut(&ordinal)
        .expect("local Proposal Sign row remains installed")
        .state = LifecycleState::Claimed(sign_lease.id());
    coordinator.active_lease = Some(sign_lease.clone());
    let prepared_dispatch = registry
        .registry_for_test_mut()
        .prepare_recovered_lifecycle_sign_dispatch(&coordinator, &sign_lease)
        .expect("project local Proposal Sign exactly once");
    let dispatch_key = prepared_dispatch.dispatch_key();
    let task = prepared_dispatch.commit_for_worker();
    assert_eq!(task.dispatch_key(), dispatch_key);
    assert!(matches!(
        registry
            .registry_for_test_mut()
            .prepare_recovered_lifecycle_sign_dispatch(&coordinator, &sign_lease),
        Err(RecoveredLifecycleSignDispatchProjectionErrorV1::AlreadyDispatched)
    ));

    let AdapterEffect::Sign {
        tag: sign_tag,
        request,
    } = proposal_effect.clone()
    else {
        unreachable!("local ProposalIntent retains one Proposal Sign")
    };
    let keys = durable_store_keys(marker);
    let signer = usize::try_from(local_validator).expect("local leader index is representable");
    let signature =
        iroha_crypto::Signature::try_new(keys[signer].private_key(), &request.signature_preimage())
            .expect("sign exact local Proposal task");
    let payload = encode_payload(
        fixture.verified.context(),
        fixture.manifest.round,
        fixture.manifest.subject,
        &fixture.canonical_wire,
    )
    .expect("restore exact local Proposal payload");
    let authority =
        crate::sumeragi::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
            ordinal,
            sign_tag,
            request,
            signature.payload().to_vec(),
            Some(payload),
            RecoveredLifecycleSignClassV1::ControlProposal,
        );
    let unrelated_subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"unrelated queued application completion block",
        )),
        payload_hash: Hash::new(b"unrelated queued application completion payload"),
    };
    runtime
        .enqueue_application_completed(tag, unrelated_subject)
        .expect("queue one unrelated application completion");
    let queue_now = std::time::Instant::now();
    let queue_before = runtime.queue_snapshot(queue_now);
    assert_eq!(runtime.queued_commands(), 1);
    let preview = runtime
        .prepare_recovered_lifecycle_sign_completion(authority)
        .expect("preview exact signed local Proposal ahead of queued ingress");
    assert_eq!(
        preview.shape(),
        crate::sumeragi::v2::RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal
    );
    assert_eq!(
            preview.settlement_family(),
            Some(
                crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal
            )
        );
    drop(preview);
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.queue_snapshot(queue_now), queue_before);
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn assert_ready_validate_vote_sign_live_transaction(
    attach_ledger: bool,
    sign_phase: wire::GlobalPhase,
    supersede_prepare: bool,
) {
    assert!(matches!(
        sign_phase,
        wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit
    ));
    assert!(!supersede_prepare || (attach_ledger && sign_phase == wire::GlobalPhase::Prepare));
    let marker = match sign_phase {
        wire::GlobalPhase::Prepare => 0xDF,
        wire::GlobalPhase::Commit => 0xE0,
    };
    let ReadyDurableValidateFixture {
        fixture,
        _directory,
        holder: _,
        lease: _,
        durable,
    } = ready_durable_validate_fixture_at_view(
        marker,
        0,
        ReadyDurableValidateFixtureOutcome::Validated,
    );
    let (tag, round, subject) = match &fixture.effect {
        AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } => (*tag, *round, *subject),
        _ => unreachable!("Ready fixture retains one Validate effect"),
    };
    let adapter_directory = TempDir::new().expect("temporary Ready Validate adapter");
    let wal_path = adapter_directory.path().join("safety.wal");
    let (mut adapter, startup) = SumeragiV2Adapter::open(
        &wal_path,
        fixture.verified.clone(),
        Some(0),
        tag.generation(),
        [0xE0; 32],
        AdapterFingerprints {
            node: Hash::new(b"Ready Validate registry join node"),
            build: Hash::new(b"Ready Validate registry join build"),
            config: Hash::new(b"Ready Validate registry join config"),
        },
        DeferredAdmissionOrdinalSource::new(1),
    )
    .expect("open exact Ready Validate adapter");
    assert!(startup.is_empty());

    let proposal = wire::Proposal {
        round,
        proposer: fixture.verified.context().leader(round.view),
        subject,
        manifest: fixture.manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![marker],
    };
    let fetch = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            )),
        ))
        .expect("admit exact Ready Validate proposal")
        .into_effects();
    assert!(matches!(
        fetch.as_slice(),
        [AdapterEffect::FetchBody {
            tag: effect_tag,
            manifest: Some(effect_manifest),
            ..
        }] if *effect_tag == tag && effect_manifest == &fixture.manifest
    ));
    let stored = adapter
        .body_available(tag, fixture.manifest.clone())
        .expect("advance exact Ready Validate body to Store")
        .into_effects();
    assert!(matches!(
        stored.as_slice(),
        [AdapterEffect::StoreBody {
            tag: effect_tag,
            round: effect_round,
            subject: effect_subject,
        }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
    ));
    let validate = adapter
        .body_stored(tag, round, subject, &durable)
        .expect("advance exact Ready Validate body to Validate")
        .into_effects();
    assert!(matches!(
        validate.as_slice(),
        [AdapterEffect::ValidateBody {
            tag: effect_tag,
            round: effect_round,
            subject: effect_subject,
        }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
    ));

    let validated_receipt = ValidatedBodyReceipt::for_test(durable.clone());
    if sign_phase == wire::GlobalPhase::Commit {
        let prepare = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: validated_receipt.execution_commitment(),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![marker; 96],
        };
        let observed = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    prepare,
                )),
            ))
            .expect("register exact concurrent PrepareQC");
        assert!(observed.effects().is_empty());
    }
    let mut holder = LifecycleWorkRegistryHolder::empty();
    let (lease, slot, coordinator_candidate) = holder
        .install_remote_proposal_validate_completion_for_test(
            &fixture.verified,
            tag,
            proposal,
            fixture.manifest.clone(),
            validated_receipt,
        );
    let registry_before = format!("{:?}", holder.registry_for_test());
    let prepared = holder
        .registry_for_test_mut()
        .prepare_ready_durable_validate_execution(&lease, slot, &fixture.verified)
        .expect("prepare exact Ready Validate registry carrier");
    let preview = prepared
        .prepare_adapter_preview(&mut adapter)
        .unwrap_or_else(|_| panic!("join exact registry carrier to adapter preview"));
    let wal_before = std::fs::read(&wal_path).expect("read empty Ready Validate WAL");
    let persisted = preview
        .seal_live_wal_validate_sign()
        .unwrap_or_else(|_| panic!("seal exact Ready Validate Sign to real WAL"));
    let wal_after = std::fs::read(&wal_path).expect("read persisted Ready Validate WAL");
    assert!(wal_after.len() > wal_before.len());

    let active_context = LifecycleContext::new(
        coordinator_candidate.key.context(),
        coordinator_candidate.key.round().height(),
    );
    let mut coordinator = LifecycleCoordinator::new(
        active_context,
        0,
        CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 64))),
    );
    assert!(matches!(
        coordinator.reduce_admit(AdmissionRequest::Candidate(coordinator_candidate)),
        AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal: None,
        } if owner == lease.owner() && ordinal == lease.ordinal()
    ));
    coordinator.ready_index.remove(&lease.ordinal());
    let parent = coordinator
        .records
        .get_mut(&lease.ordinal())
        .expect("admitted Validate parent");
    parent.physical_slots = lease.physical_slots().clone();
    parent.state = LifecycleState::Claimed(lease.id());
    coordinator.active_lease = Some(lease.clone());
    if !attach_ledger {
        let result = coordinator.prepare_sealed_validate_sign_transition(
            &lease,
            &fixture.verified,
            persisted,
        );
        assert!(result.is_err());
        drop(result);
        assert!(coordinator.ledger_store.is_none());
        assert_eq!(
            coordinator.records[&lease.ordinal()].state,
            LifecycleState::Claimed(lease.id())
        );
        assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
        assert_eq!(
            std::fs::read(&wal_path).expect("read WAL after missing-store rejection"),
            wal_after
        );
        assert!(matches!(
            adapter.body_available(tag, fixture.manifest.clone()),
            Err(AdapterError::FailClosed)
        ));
        return;
    }
    let ledger_directory = TempDir::new().expect("temporary live publication ledger");
    coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .expect("attach exact current LedgerV1");

    coordinator
        .prepare_sealed_validate_sign_transition(&lease, &fixture.verified, persisted)
        .unwrap_or_else(|_| panic!("stage exact sealed Validate-to-Vote transaction"))
        .persist_and_publish()
        .unwrap_or_else(|_| panic!("fsync and publish exact live Validate-to-Vote cut"));

    let child_ordinal = lease.ordinal() + 1;
    let child_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
    let child_address = ConcreteWorkAddress::new(lease.owner(), child_ordinal, child_slot)
        .expect("exact live Sign child address");
    assert_ne!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(holder.registry_for_test().entries.len(), 1);
    let child_work = holder
        .registry_for_test()
        .entries
        .get(&child_address)
        .expect("reserved Sign child is installed");
    assert!(child_work.validate_exact());
    assert_eq!(child_work.causal_root(), lease.owner().causal_root());
    assert!(matches!(
        &child_work.kind,
        ConcreteLifecycleWorkKind::DurableLiveWalSign(sign)
            if matches!(
                &sign.admission.bound.effect,
                AdapterEffect::Sign {
                    request: SignRequest::Vote(vote),
                    ..
                } if vote.phase == sign_phase
            )
                && sign.dispatch_key.is_none()
    ));
    let child_sign_effect = match &child_work.kind {
        ConcreteLifecycleWorkKind::DurableLiveWalSign(sign) => sign.admission.bound.effect.clone(),
        _ => unreachable!("reserved Validate successor remains one live-WAL Sign"),
    };
    assert_eq!(
        coordinator.records[&lease.ordinal()].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    let expected_edge = match sign_phase {
        wire::GlobalPhase::Prepare => {
            super::super::schema::DurableContinuationEdge::ValidateToSignPrepare
        }
        wire::GlobalPhase::Commit => {
            super::super::schema::DurableContinuationEdge::ValidateToSignCommit
        }
    };
    assert_eq!(
        coordinator.durable_records[&lease.ordinal()].continuation,
        super::super::schema::DurableContinuation::successor(expected_edge, child_ordinal)
    );
    assert_eq!(
        coordinator.records[&child_ordinal].state,
        LifecycleState::Ready
    );
    assert_eq!(
        coordinator.records[&child_ordinal].stage.kind(),
        match sign_phase {
            wire::GlobalPhase::Prepare => LifecycleStageKind::SignPrepareVote,
            wire::GlobalPhase::Commit => LifecycleStageKind::SignCommitVote,
        }
    );
    assert!(
        holder
            .registry_for_test()
            .exactly_covers_all_live_work(&fixture.verified, &coordinator)
    );
    let exact_replay = coordinator.durable_records[&child_ordinal]
        .replay_authority
        .clone();
    let foreign_replay = exact_replay
        .with_foreign_origin_generation_for_test()
        .expect("live WAL Sign replay supports a foreign-generation negative fixture");
    let child = &coordinator.records[&child_ordinal];
    let child_metadata = &coordinator.durable_records[&child_ordinal];
    assert!(foreign_replay.structurally_matches_record(
        coordinator.active_context,
        child.key,
        child.work_class,
        child.stage,
        child_metadata.payload,
    ));
    coordinator
        .durable_records
        .get_mut(&child_ordinal)
        .expect("live Sign metadata")
        .replay_authority = foreign_replay;
    assert!(
        !holder
            .registry_for_test()
            .exactly_covers_all_live_work(&fixture.verified, &coordinator)
    );
    coordinator
        .durable_records
        .get_mut(&child_ordinal)
        .expect("live Sign metadata")
        .replay_authority = exact_replay;
    assert!(coordinator.active_lease.is_none());
    assert!(adapter.signature_fence_is_active());
    assert!(matches!(
        adapter.signature_fence_identity(),
        Some((identity_tag, reducer::SignableMessage::Vote(vote)))
            if identity_tag == tag
                && vote.phase()
                    == match sign_phase {
                        wire::GlobalPhase::Prepare => reducer::Phase::Prepare,
                        wire::GlobalPhase::Commit => reducer::Phase::Commit,
                    }
    ));
    let (_, reopened) =
        super::super::ledger::LifecycleLedgerStoreV1::open(ledger_directory.path(), active_context)
            .expect("reopen exact committed LedgerV1");
    assert_eq!(reopened.high_water(), child_ordinal);
    assert_eq!(reopened.records().len(), 2);
    assert_eq!(
        reopened.records()[0].terminal(),
        Some(Some(TerminalOutcome::Advanced))
    );
    assert_eq!(
        reopened.records()[0].continuation(),
        Some(super::super::schema::DurableContinuation::successor(
            expected_edge,
            child_ordinal,
        ))
    );

    let attestation = holder
        .registry_for_test()
        .attest_ready_recovered_lifecycle_sign(&coordinator, child_ordinal)
        .expect("typed live Commit Sign is the sole Ready bounded-I/O carrier");
    assert_eq!(
        attestation.demand(),
        ReadyRecoveredLifecycleSignDemandV1::BoundedIo
    );
    let child_record = coordinator.records[&child_ordinal].clone();
    let sign_lease = TurnLease {
        id: LeaseId(lease.id().0 + 1),
        ordinal: child_ordinal,
        owner: child_record.owner,
        key: child_record.key,
        work_class: child_record.work_class,
        stage: child_record.stage,
        rank: super::super::SchedulerRank::new(0, 0, 0, 0, 0, 0, 0, 0),
        physical_slots: child_record.physical_slots.clone(),
        output_reservation: Some(super::super::schema::LeaseCapacityReservation::new(
            CapacityClass::Consensus,
            0,
        )),
    };
    coordinator.ready_index.remove(&child_ordinal);
    coordinator
        .records
        .get_mut(&child_ordinal)
        .expect("live Commit Sign row remains installed")
        .state = LifecycleState::Claimed(sign_lease.id());
    coordinator.active_lease = Some(sign_lease.clone());

    let continuation = coordinator.durable_records[&lease.ordinal()].continuation;
    coordinator
        .durable_records
        .get_mut(&lease.ordinal())
        .expect("live Validate predecessor metadata remains installed")
        .continuation = super::super::schema::DurableContinuation::None;
    assert!(matches!(
        holder
            .registry_for_test_mut()
            .prepare_recovered_lifecycle_sign_dispatch(&coordinator, &sign_lease),
        Err(RecoveredLifecycleSignDispatchProjectionErrorV1::InvalidCarrier)
    ));
    coordinator
        .durable_records
        .get_mut(&lease.ordinal())
        .expect("live Validate predecessor metadata remains installed")
        .continuation = continuation;

    let prepared = holder
        .registry_for_test_mut()
        .prepare_recovered_lifecycle_sign_dispatch(&coordinator, &sign_lease)
        .expect("project the exact claimed live Commit Sign once");
    let dispatch_key = prepared.dispatch_key();
    let task = prepared.commit_for_worker();
    assert_eq!(task.dispatch_key(), dispatch_key);
    assert!(matches!(
        holder
            .registry_for_test_mut()
            .prepare_recovered_lifecycle_sign_dispatch(&coordinator, &sign_lease),
        Err(RecoveredLifecycleSignDispatchProjectionErrorV1::AlreadyDispatched)
    ));

    if sign_phase == wire::GlobalPhase::Prepare {
        let AdapterEffect::Sign {
            tag: sign_tag,
            request,
        } = child_sign_effect
        else {
            unreachable!("validated receiver successor remains one Vote Sign")
        };
        let SignRequest::Vote(local_vote) = &request else {
            unreachable!("validated receiver successor remains one Prepare Vote Sign")
        };
        let roster_len = u32::try_from(fixture.verified.context().roster.len())
            .expect("fixture roster length fits a validator index");
        let mut peer_vote = local_vote.clone();
        peer_vote.signer = local_vote
            .signer
            .checked_add(1)
            .expect("fixture signer advances")
            % roster_len;
        peer_vote.signature = vec![marker.wrapping_add(1); 96];
        let peer_message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(peer_vote));
        let deferred = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                peer_message.clone(),
            ))
            .expect("admit one authenticated peer Prepare behind the receiver Sign fence");
        assert_eq!(
            deferred.disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );

        let queue_projection = |adapter: &mut SumeragiV2Adapter| {
            adapter
                .status()
                .expect("snapshot receiver deferred queues")
                .liveness
                .queues
                .into_iter()
                .filter(|status| {
                    matches!(
                        status.queue,
                        wire::SumeragiV2QueueKind::DeferredCompletion
                            | wire::SumeragiV2QueueKind::DeferredProgress
                            | wire::SumeragiV2QueueKind::DeferredNormal
                    )
                })
                .map(|status| {
                    (
                        status.queue,
                        status.depth,
                        status.capacity,
                        status.service_debt,
                    )
                })
                .collect::<Vec<_>>()
        };
        let queue_projection_before = queue_projection(&mut adapter);
        assert_eq!(
            queue_projection_before
                .iter()
                .find(|(queue, ..)| *queue == wire::SumeragiV2QueueKind::DeferredCompletion)
                .map(|(_, depth, ..)| *depth),
            Some(0)
        );
        assert_eq!(
            queue_projection_before
                .iter()
                .find(|(queue, ..)| *queue == wire::SumeragiV2QueueKind::DeferredProgress)
                .map(|(_, depth, ..)| *depth),
            Some(0)
        );
        assert_eq!(
            queue_projection_before
                .iter()
                .find(|(queue, ..)| *queue == wire::SumeragiV2QueueKind::DeferredNormal)
                .map(|(_, depth, ..)| *depth),
            Some(1)
        );
        let ordinals_before = adapter.all_deferred_admission_ordinals();
        let authenticated_before = adapter.authenticated_deferred_admission_ordinals();
        let (owned_tag, deferred_ordinal) = adapter
            .deferred_authenticated_message_owner(&peer_message)
            .expect("the exact authenticated peer Prepare retains one deferred owner");
        assert_eq!(owned_tag, sign_tag);
        let ownership_before = adapter
            .deferred_occurrence_ownership(deferred_ordinal)
            .expect("the deferred peer Prepare retains its opaque occurrence authority");
        assert!(ownership_before.is_authenticated_ingress());
        assert!(ownership_before.still_retained());

        let keys = durable_store_keys(marker);
        let signer =
            usize::try_from(local_vote.signer).expect("fixture local Vote signer is representable");
        let signature = iroha_crypto::Signature::try_new(
            keys[signer].private_key(),
            &request.signature_preimage(),
        )
        .expect("sign exact receiver Prepare Vote task");
        let signature = signature.payload().to_vec();
        let authority = crate::sumeragi::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
            child_ordinal,
            sign_tag,
            request.clone(),
            signature.clone(),
            None,
            RecoveredLifecycleSignClassV1::PhaseVote,
        );
        let preview = adapter
            .prepare_recovered_lifecycle_sign_completion(authority)
            .expect("preview receiver Prepare signature ahead of Busy-deferred peer ingress");
        assert_eq!(
            preview.settlement_family(),
            Some(crate::sumeragi::v2::RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast)
        );
        drop(preview);

        assert_eq!(queue_projection(&mut adapter), queue_projection_before);
        assert_eq!(adapter.all_deferred_admission_ordinals(), ordinals_before);
        assert_eq!(
            adapter.authenticated_deferred_admission_ordinals(),
            authenticated_before
        );
        assert_eq!(
            adapter.deferred_authenticated_message_owner(&peer_message),
            Some((owned_tag, deferred_ordinal))
        );
        let ownership_after = adapter
            .deferred_occurrence_ownership(deferred_ordinal)
            .expect("drop retains the exact authenticated peer Prepare owner");
        assert_eq!(ownership_after, ownership_before);
        assert!(ownership_after.still_retained());
        assert!(adapter.signature_fence_is_active());

        if supersede_prepare {
            let decision = wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: local_vote.execution_commitment.clone(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![marker; 96],
            };
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                    wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::QuorumCertificate(decision),
                    ),
                ))
                .expect("certified CommitQC bypasses the exact Prepare Sign fence");
            let mut forged_signature = signature.clone();
            forged_signature[0] ^= 1;
            let forged = crate::sumeragi::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
                child_ordinal,
                sign_tag,
                request.clone(),
                forged_signature,
                None,
                RecoveredLifecycleSignClassV1::PhaseVote,
            );
            assert!(matches!(
                adapter.prepare_recovered_lifecycle_sign_completion(forged),
                Err(AdapterError::RecoveredLifecycleSignCompletionMismatch)
            ));
            let superseded = crate::sumeragi::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1::for_test(
                child_ordinal,
                sign_tag,
                request,
                signature,
                None,
                RecoveredLifecycleSignClassV1::PhaseVote,
            );
            assert!(matches!(
                adapter.prepare_recovered_lifecycle_sign_completion(superseded),
                Err(AdapterError::RecoveredLifecycleSignCompletionSuperseded)
            ));
            return;
        }

        let signed = adapter
            .signature_completed(sign_tag, signature)
            .expect("apply the same exact receiver Prepare signature after inert preview drop");
        assert!(matches!(
            signed.effects(),
            [AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
                payload: wire::ConsensusMessageV2Payload::Vote(_),
                ..
            })]
        ));
        let (_effects, evidence) = adapter
            .drain_deferred_with_evidence()
            .expect("service receiver peer Prepare after its Sign fence opens")
            .expect("the exact Busy-deferred peer Prepare remains selectable");
        assert!(evidence.validate_exact());
        assert_eq!(evidence.admission_ordinal, deferred_ordinal);
        assert_eq!(
            evidence.priority,
            crate::sumeragi::v2::DeferredPriority::Normal
        );
        assert_eq!(
            evidence.service_cursor_before,
            crate::sumeragi::v2::DeferredPriority::Completion
        );
        assert_eq!(
            evidence.service_cursor_after,
            crate::sumeragi::v2::DeferredPriority::Completion
        );
        assert_eq!(evidence.queue_lengths_before.completion, 0);
        assert_eq!(evidence.queue_lengths_before.progress, 0);
        assert_eq!(evidence.queue_lengths_before.normal, 1);
    } else {
        let cancellation = holder
            .registry_for_test()
            .prepare_recovered_lifecycle_sign_cancellation(&coordinator, &sign_lease, dispatch_key)
            .expect("bind the exact dispatched Commit Sign for cancellation");
        let mut staged = coordinator.stage_durable_transaction();
        staged.reduce_cancel_superseded_sign(sign_lease.clone());
        let mut wrong_staged = staged.clone();
        wrong_staged.high_water = wrong_staged
            .high_water
            .checked_add(1)
            .expect("negative staged high-water mutation fits");
        let cancellation = match holder
            .registry_for_test_mut()
            .publish_recovered_lifecycle_sign_cancellation(
                cancellation,
                &coordinator,
                &wrong_staged,
                &sign_lease,
                || -> Result<(), ()> { panic!("invalid staged cancellation must not publish") },
            ) {
            Err(RecoveredLifecycleSignCancellationPublicationError::Preflight(cancellation)) => {
                cancellation
            }
            Ok(()) => panic!("invalid staged cancellation cannot publish"),
            Err(RecoveredLifecycleSignCancellationPublicationError::Publication(_, _)) => {
                panic!("invalid staged cancellation cannot reach publication")
            }
        };
        assert!(
            holder
                .registry_for_test()
                .entries
                .contains_key(&child_address)
        );
        match holder
            .registry_for_test_mut()
            .publish_recovered_lifecycle_sign_cancellation(
                cancellation,
                &coordinator,
                &staged,
                &sign_lease,
                || coordinator.persist_exact_staged_successor(&staged),
            ) {
            Ok(()) => {}
            Err(RecoveredLifecycleSignCancellationPublicationError::Preflight(_)) => {
                panic!("exact superseded Commit Sign cancellation must pass preflight")
            }
            Err(RecoveredLifecycleSignCancellationPublicationError::Publication(_, _)) => {
                panic!("exact superseded Commit Sign cancellation must publish")
            }
        }
        coordinator = staged;
        assert!(holder.registry_for_test().entries.is_empty());
        assert!(coordinator.active_lease.is_none());
        assert_eq!(
            coordinator.records[&child_ordinal].state,
            LifecycleState::Terminal(TerminalOutcome::Cancelled)
        );
        let (_, reopened_cancelled) = super::super::ledger::LifecycleLedgerStoreV1::open(
            ledger_directory.path(),
            active_context,
        )
        .expect("reopen LedgerV1 after Sign cancellation");
        assert_eq!(
            reopened_cancelled.records()[1].terminal(),
            Some(Some(TerminalOutcome::Cancelled))
        );
    }
}

#[cfg(feature = "bls")]
#[test]
fn ready_validate_apply_publishes_at_actor_global_child_coordinates() {
    let handle = std::thread::Builder::new()
        .name("ready-validate-apply-actor-global-child".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| ready_validate_apply_actor_global_child_fixture(false, false))
        .expect("spawn Ready Validate Apply actor-global child fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
#[test]
fn ready_validate_apply_rejects_a_tampered_body_frame_before_publication() {
    let handle = std::thread::Builder::new()
        .name("ready-validate-apply-tampered-body-frame".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| ready_validate_apply_actor_global_child_fixture(true, false))
        .expect("spawn tampered Ready Validate Apply body-frame fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
#[test]
fn lifecycle_decision_apply_live_recovered_substitution_matrix_is_inert() {
    let handle = std::thread::Builder::new()
        .name("lifecycle-apply-live-recovered-substitution".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(|| ready_validate_apply_actor_global_child_fixture(false, true))
        .expect("spawn lifecycle Decision Apply lineage substitution fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn ready_validate_apply_actor_global_child_fixture(
    tamper_apply_frame: bool,
    exercise_lineage_matrix: bool,
) {
    let marker = 0xE0;
    let ReadyDurableValidateFixture {
        fixture,
        _directory,
        holder: _,
        lease: _,
        durable,
    } = ready_durable_validate_fixture_at_view(
        marker,
        0,
        ReadyDurableValidateFixtureOutcome::Validated,
    );
    let (tag, round, subject) = match &fixture.effect {
        AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } => (*tag, *round, *subject),
        _ => unreachable!("Ready fixture retains one Validate effect"),
    };
    let adapter_directory = TempDir::new().expect("temporary Ready Validate Apply adapter");
    let wal_path = adapter_directory.path().join("safety.wal");
    let (mut adapter, startup) = SumeragiV2Adapter::open(
        &wal_path,
        fixture.verified.clone(),
        Some(0),
        tag.generation(),
        [marker; 32],
        AdapterFingerprints {
            node: Hash::new(b"Ready Validate Apply registry join node"),
            build: Hash::new(b"Ready Validate Apply registry join build"),
            config: Hash::new(b"Ready Validate Apply registry join config"),
        },
        DeferredAdmissionOrdinalSource::new(1),
    )
    .expect("open exact Ready Validate Apply adapter");
    assert!(startup.is_empty());

    let proposal = wire::Proposal {
        round,
        proposer: fixture.verified.context().leader(round.view),
        subject,
        manifest: fixture.manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![marker],
    };
    let fetch = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            )),
        ))
        .expect("admit exact Ready Validate Apply proposal")
        .into_effects();
    assert!(
        matches!(
            fetch.as_slice(),
            [AdapterEffect::FetchBody {
                tag: effect_tag,
                manifest: Some(effect_manifest),
                ..
            }] if *effect_tag == tag && effect_manifest == &fixture.manifest
        ),
        "unexpected proposal ingress effects: {fetch:?}"
    );
    let stored = adapter
        .body_available(tag, fixture.manifest.clone())
        .expect("advance exact decided body to Store")
        .into_effects();
    assert!(matches!(
        stored.as_slice(),
        [AdapterEffect::StoreBody {
            tag: effect_tag,
            round: effect_round,
            subject: effect_subject,
        }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
    ));
    let validate = adapter
        .body_stored(tag, round, subject, &durable)
        .expect("advance exact decided body to Validate")
        .into_effects();
    assert!(matches!(
        validate.as_slice(),
        [AdapterEffect::ValidateBody {
            tag: effect_tag,
            round: effect_round,
            subject: effect_subject,
        }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
    ));
    let validated_receipt = ValidatedBodyReceipt::for_test(durable.clone());
    let prepare = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: validated_receipt.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![marker; 96],
    };
    let observed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                prepare,
            )),
        ))
        .expect("register exact concurrent PrepareQC");
    assert!(observed.effects().is_empty());
    let decision = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: validated_receipt.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![marker; 96],
    };
    let observed = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                decision.clone(),
            )),
        ))
        .expect("register exact concurrent CommitQC");
    assert!(observed.effects().is_empty());

    let mut holder = LifecycleWorkRegistryHolder::empty();
    let (lease, slot, coordinator_candidate) = holder
        .install_remote_proposal_validate_completion_for_test(
            &fixture.verified,
            tag,
            proposal,
            fixture.manifest.clone(),
            validated_receipt,
        );
    let active_context = LifecycleContext::new(
        coordinator_candidate.key.context(),
        coordinator_candidate.key.round().height(),
    );
    let mut coordinator = LifecycleCoordinator::new(
        active_context,
        0,
        CapacityGeometry::new(CapacityClass::ALL.into_iter().map(|class| (class, 64))),
    );
    assert!(matches!(
        coordinator.reduce_admit(AdmissionRequest::Candidate(coordinator_candidate)),
        AdmissionDecision::Admitted {
            owner,
            ordinal,
            producer_turn_ordinal: None,
        } if owner == lease.owner() && ordinal == lease.ordinal()
    ));
    coordinator
        .records
        .get_mut(&lease.ordinal())
        .expect("admitted Validate parent")
        .physical_slots = lease.physical_slots().clone();
    let live_validate_attestation = coordinator
        .attest_ready_validate_demand(&holder, lease.ordinal())
        .expect("attest exact Ready Validate predecessor before publication");
    assert!(!live_validate_attestation.requires_io_dispatch());
    let live_validate_dispatch_key = live_validate_attestation.dispatch_key();
    assert!(live_validate_dispatch_key.matches_consensus_round(&round));
    coordinator.ready_index.remove(&lease.ordinal());
    coordinator
        .records
        .get_mut(&lease.ordinal())
        .expect("claim admitted Validate parent")
        .state = LifecycleState::Claimed(lease.id());
    coordinator.active_lease = Some(lease.clone());
    let prepared = holder
        .registry_for_test_mut()
        .prepare_ready_durable_validate_execution(&lease, slot, &fixture.verified)
        .expect("prepare exact Ready Validate Apply registry carrier");
    let preview = prepared
        .prepare_adapter_preview(&mut adapter)
        .unwrap_or_else(|_| panic!("join exact Validate carrier to decided adapter preview"));
    let publication = preview
        .seal_live_wal_validate_apply()
        .unwrap_or_else(|_| panic!("seal exact Ready Validate Apply publication"));

    let local_prediction = coordinator
        .high_water
        .checked_add(1)
        .expect("local child prediction remains bounded");
    assert_eq!(local_prediction, 2);
    let (runtime_ordinals, coordinator_ordinals) =
        authority::lifecycle_ordinal_authorities_after_high_watermark(coordinator.high_water);
    coordinator.lifecycle_ordinal_authority = Some(coordinator_ordinals);
    let runtime_ordinals =
        crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::from_authority(
            runtime_ordinals,
        );
    runtime_ordinals
        .advance_past(7)
        .expect("advance actor-global ordinals past the local prediction");
    let ledger_directory = TempDir::new().expect("temporary Validate Apply lifecycle ledger");
    coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .expect("attach exact current LedgerV1");

    let mut transition = coordinator
        .prepare_sealed_validate_apply_transition(&lease, &fixture.verified, publication)
        .unwrap_or_else(|_| panic!("stage exact sealed Validate-to-Apply transaction"));
    if tamper_apply_frame {
        assert!(transition.tamper_apply_body_frame_for_test());
        let publication_error = transition
            .persist_and_publish()
            .expect_err("reject a candidate body frame foreign to the retained receipt");
        assert_eq!(
            publication_error.registry_failure_reason(),
            Some(LiveValidateApplyRegistryPublicationFailureReason::AdapterWork)
        );
        drop(publication_error);
        assert_eq!(
            coordinator.fault,
            Some(crate::sumeragi::v2_lifecycle_coordinator::CoordinatorFault::DurabilityFailure)
        );
        assert_eq!(holder.registry_for_test().entries.len(), 1);
        let parent_address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .expect("retained Validate parent address");
        assert!(
            holder
                .registry_for_test()
                .entries
                .contains_key(&parent_address)
        );
        return;
    }
    let publication_result = transition.persist_and_publish();
    if let Err(error) = publication_result {
        panic!(
            "fsync and publish exact live Validate-to-Apply cut: {:?}",
            error.registry_failure_reason()
        );
    }

    let child_ordinal = 8;
    assert_ne!(child_ordinal, local_prediction);
    let child_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
    let child_address = ConcreteWorkAddress::new(lease.owner(), child_ordinal, child_slot)
        .expect("actor-global Apply child address");
    let predicted_address = ConcreteWorkAddress::new(lease.owner(), local_prediction, child_slot)
        .expect("obsolete local Apply prediction");
    assert!(
        !holder
            .registry_for_test()
            .entries
            .contains_key(&predicted_address)
    );
    let child_work = holder
        .registry_for_test()
        .entries
        .get(&child_address)
        .expect("exact actor-global Apply child is installed");
    assert!(child_work.validate_exact());
    assert!(matches!(
        &child_work.kind,
        ConcreteLifecycleWorkKind::DurableLiveWalApply(_)
    ));
    let cleanup = holder
        .prepare_ready_live_decision_apply_reconciliation(&coordinator, child_ordinal)
        .expect("attest the exact dedicated live Apply carrier")
        .expect("live Apply projects queue-inert cleanup authority");
    assert_eq!(
        cleanup.dispatch_key().lineage(),
        LifecycleDecisionApplyLineageV1::Live
    );
    assert_eq!(cleanup.subject(), subject);
    assert_eq!(cleanup.certificate(), &decision);
    assert_eq!(holder.registry_for_test().entries.len(), 1);
    assert_eq!(coordinator.high_water, child_ordinal);
    assert_eq!(
        coordinator.records[&lease.ordinal()].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    assert_eq!(
        coordinator.durable_records[&lease.ordinal()].continuation,
        super::super::schema::DurableContinuation::successor(
            super::super::schema::DurableContinuationEdge::ValidateToApply,
            child_ordinal,
        )
    );
    assert_eq!(
        coordinator.records[&child_ordinal].state,
        LifecycleState::Ready
    );
    assert_eq!(
        coordinator.records[&child_ordinal].stage.kind(),
        LifecycleStageKind::ApplyDecision
    );
    if exercise_lineage_matrix {
        assert_lifecycle_decision_apply_live_recovered_substitution_matrix(
            &fixture.verified,
            &mut holder,
            &mut coordinator,
            child_ordinal,
            child_address,
            tag,
            ledger_directory.path(),
            adapter,
            startup,
            cleanup,
            live_validate_dispatch_key,
            _directory.path(),
        );
        return;
    }
    assert!(
        holder
            .registry_for_test()
            .exactly_covers_all_live_work(&fixture.verified, &coordinator)
    );
    let mut tampered_coordinator = coordinator.clone();
    let DurablePayloadReference::BodyFrame(mut tampered_frame) = tampered_coordinator
        .durable_records
        .get(&child_ordinal)
        .expect("actor-global Apply child retains durable metadata")
        .payload
    else {
        panic!("actor-global Apply child retains one body-frame payload")
    };
    let first_tamper = LifecycleDigest::new([0xA6; 32]);
    tampered_frame.frame = if tampered_frame.frame == first_tamper {
        LifecycleDigest::new([0x6A; 32])
    } else {
        first_tamper
    };
    tampered_coordinator
        .durable_records
        .get_mut(&child_ordinal)
        .expect("tamper only the copied actor-global Apply metadata")
        .payload = DurablePayloadReference::BodyFrame(tampered_frame);
    assert!(
        !holder
            .registry_for_test()
            .exactly_covers_all_live_work(&fixture.verified, &tampered_coordinator)
    );
    assert_eq!(
        runtime_ordinals
            .next_ordinal_for_test()
            .expect("inspect committed actor-global cursor"),
        Some(9)
    );
    let (_, reopened) =
        super::super::ledger::LifecycleLedgerStoreV1::open(ledger_directory.path(), active_context)
            .expect("reopen exact committed Validate-to-Apply LedgerV1");
    assert_eq!(reopened.high_water(), child_ordinal);
    assert_eq!(reopened.records().len(), 2);

    // Continue the same actor-global successor through the production
    // cleanup, capacity, queue, guarded completion, LedgerV1, and registry
    // terminal seams. The worker fixture preserves the real bounded queue and
    // tracker transitions while supplying only the structurally authenticated
    // Kura terminal; State/Kura execution is covered by the apply-service and
    // four-peer acceptance lanes.
    let payload_directory = TempDir::new().expect("temporary live Apply payload store");
    let (payload_store, serve_payloads) =
        CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            payload_directory.path(),
            fixture.verified.context(),
        )
        .expect("open empty live Apply Serve payload owner");
    let mut body_store = V2BodyStore::open(_directory.path(), fixture.verified.context().clone())
        .expect("reopen exact live Apply body store");
    body_store
        .revalidate_recovered_markers(|_| {
            Ok::<_, String>(cleanup.validated_receipt().execution_commitment())
        })
        .expect("semantically revalidate exact live Apply body marker");
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
        timeout_supersession_successor: None,
    };
    let runtime = crate::sumeragi::v2_runtime::SerializedV2Runtime::new(
        adapter,
        startup,
        std::time::Instant::now(),
        std::time::Duration::from_secs(10),
        crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap exact live Apply adapter")
    .0;
    let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let (mut executor, mut planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
        &mut services,
        runtime,
        std::sync::Arc::clone(&output_guard),
        0,
        2,
    );
    let live_started_at = std::time::Instant::now();
    executor
        .arm_live_clocks(
            super::super::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            live_started_at,
        )
        .expect("arm exact live Apply clocks after service construction");
    executor
        .arm_live_lifecycle_validate_successor(live_validate_dispatch_key, round, subject, true)
        .expect("restore exact published Validate predecessor owner before runtime import");
    assert_eq!(
        executor
            .reconcile_reopened_decision_for_lifecycle_apply_lineage_test(&mut services)
            .expect("reconcile exact live Apply Decision into the executor"),
        (
            decision.round,
            decision.proposal_round,
            subject,
            decision.execution_commitment,
        )
    );
    executor
        .reconcile_live_lifecycle_decision_apply(cleanup, &mut services)
        .expect("install exact live Apply owner before the normal Ready scheduler gate");

    planner_io.saturate_consensus_prefix(&services);
    assert_eq!(
        owner
            .dispatch_completion_for_test(&mut services, &mut executor, 0)
            .expect("clean live Apply before the frozen queue census"),
        super::super::ProductionCompletionDispatchV1::CapacityUnavailable {
            protected_live_apply_ordinal: Some(child_ordinal),
        }
    );
    let barrier_key = executor
        .live_lifecycle_decision_apply_key_for_test()
        .expect("capacity retry retains the exact live Apply retransmit owner");
    assert_eq!(barrier_key.lineage(), LifecycleDecisionApplyLineageV1::Live);
    assert_eq!(barrier_key.lifecycle_ordinal(), child_ordinal);
    let live_work = owner
        .registry
        .registry_for_test()
        .entries
        .get(&child_address)
        .expect("capacity retry retains the exact live Apply carrier");
    let ConcreteLifecycleWorkKind::DurableLiveWalApply(live_apply) = &live_work.kind else {
        panic!("capacity retry changed the dedicated live Apply carrier")
    };
    assert!(live_apply.dispatch_key.is_none());
    assert_eq!(planner_io.queued_lifecycle_decision_apply_count(), 0);

    planner_io.release_all_predecessors();
    assert_eq!(
        owner
            .dispatch_completion_for_test(&mut services, &mut executor, 0)
            .expect("claim and queue the exact live Apply carrier"),
        super::super::ProductionCompletionDispatchV1::ApplyQueued {
            ordinal: child_ordinal,
        }
    );
    assert_eq!(planner_io.queued_lifecycle_decision_apply_count(), 1);
    assert_eq!(
        executor.live_lifecycle_decision_apply_key_for_test(),
        Some(barrier_key)
    );
    executor
        .coalesce_live_lifecycle_apply_retransmit_for_test(
            tag,
            subject,
            decision.clone(),
            &mut services,
        )
        .expect("exact due Apply retransmit coalesces on the live lifecycle owner");
    assert_eq!(planner_io.queued_lifecycle_decision_apply_count(), 1);
    assert_eq!(executor.status().pending_applications, 0);

    planner_io.execute_one_lifecycle_decision_apply_fixture(std::sync::Arc::clone(&output_guard));
    let completion = match services
        .take_next_lifecycle_completion()
        .expect("take exact guarded live Apply completion")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::Apply(completion) => completion,
        _ => panic!("live Apply queue produced a foreign completion class"),
    };
    let crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(applied) =
        completion.result()
    else {
        panic!("live Apply fixture must produce an applied terminal")
    };
    let lease = owner
        .coordinator
        .active_lease
        .clone()
        .expect("queued live Apply retains its exact active lease");
    let (transition, authority) = owner
        .registry
        .prepare_lifecycle_decision_apply_terminal_transition(&owner.coordinator, &lease, applied)
        .expect("join exact live worker result to its installed carrier");
    let adapter = executor
        .prepare_lifecycle_decision_apply_completion(authority)
        .expect("preview exact live Apply completion on the serialized adapter");
    let mut staged = owner.coordinator.stage_durable_transaction();
    staged.reduce_settle_turn(lease.clone(), super::super::TurnOutcome::Advanced, None);
    assert!(staged.fault.is_none());
    owner
        .registry
        .publish_lifecycle_decision_apply_terminal_transition(
            transition,
            &owner.coordinator,
            &staged,
            &lease,
            || owner.coordinator.persist_exact_staged_successor(&staged),
        )
        .unwrap_or_else(|_| panic!("publish exact live Apply terminal through LedgerV1"));
    owner.coordinator = staged;
    let finality = adapter.commit_after_durable_settlement();
    let status = executor.commit_lifecycle_decision_apply_finality(finality);
    let settled = completion.acknowledge_after_owner_settlement();
    assert!(matches!(
        settled,
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(_)
    ));
    assert_eq!(status.height, fixture.verified.context().height);
    assert!(owner.registry.registry_for_test().entries.is_empty());
    assert_eq!(
        owner.coordinator.records[&child_ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    assert!(owner.coordinator.active_lease.is_none());
    assert!(
        executor
            .live_lifecycle_decision_apply_key_for_test()
            .is_none()
    );
    assert!(executor.durable_finality().is_some());
    let (_, terminal_ledger) =
        super::super::ledger::LifecycleLedgerStoreV1::open(ledger_directory.path(), active_context)
            .expect("reopen exact terminal live Apply LedgerV1");
    assert_eq!(terminal_ledger.high_water(), child_ordinal);
    assert_eq!(
        terminal_ledger
            .records()
            .iter()
            .find(|record| record.ordinal() == child_ordinal)
            .and_then(super::super::ledger::LifecycleLedgerRecordV1::terminal),
        Some(Some(TerminalOutcome::Advanced))
    );
    assert!(!output_guard.restart_required());
    planner_io.detach(&mut services);
}

#[cfg(feature = "bls")]
fn claim_ready_apply_for_lineage_test(
    coordinator: &mut LifecycleCoordinator,
    ordinal: u128,
) -> TurnLease {
    assert!(coordinator.fault.is_none());
    assert!(coordinator.active_lease.is_none());
    assert!(coordinator.ready_index.remove(&ordinal));
    let id_value = coordinator
        .next_lease
        .expect("lineage fixture retains one next lease id");
    coordinator.next_lease = Some(
        id_value
            .checked_add(1)
            .expect("lineage fixture lease id remains bounded"),
    );
    let id = LeaseId(id_value);
    let record = coordinator
        .records
        .get_mut(&ordinal)
        .expect("lineage fixture retains its exact Ready Apply row");
    assert_eq!(record.work_class, LifecycleWorkClass::Apply);
    assert_eq!(record.state, LifecycleState::Ready);
    record.state = LifecycleState::Claimed(id);
    let lease = TurnLease {
        id,
        ordinal: record.ordinal,
        owner: record.owner,
        key: record.key,
        work_class: record.work_class,
        stage: record.stage,
        rank: super::super::schema::SchedulerRank::new(0, 0, 0, 0, 0, 0, 0, 0),
        physical_slots: record.physical_slots.clone(),
        output_reservation: None,
    };
    coordinator.active_lease = Some(lease.clone());
    lease
}

#[cfg(feature = "bls")]
fn assert_lifecycle_decision_apply_key_coordinates_are_closed(
    key: LifecycleDecisionApplyDispatchKeyV1,
    context: LifecycleContext,
    address: ConcreteWorkAddress,
    digest: LifecycleDigest,
    lineage: LifecycleDecisionApplyLineageV1,
) {
    assert!(key.matches_carrier(context, address, digest, lineage));

    let mut foreign_context = key;
    let first_context = LifecycleDigest::new([0x8F; 32]);
    foreign_context.context = if context.id() == first_context {
        LifecycleDigest::new([0x90; 32])
    } else {
        first_context
    };
    assert!(!foreign_context.matches_carrier(context, address, digest, lineage));
    foreign_context.context = context.id();
    assert_eq!(foreign_context, key);

    let mut foreign_height = key;
    foreign_height.height = context
        .height()
        .checked_add(1)
        .expect("lineage fixture height remains bounded");
    assert!(!foreign_height.matches_carrier(context, address, digest, lineage));
    foreign_height.height = context.height();
    assert_eq!(foreign_height, key);

    let mut foreign_owner = key;
    let mut root = CausalRoot::new(LifecycleDigest::new([0x91; 32]));
    if root == address.owner.causal_root() {
        root = CausalRoot::new(LifecycleDigest::new([0x92; 32]));
    }
    foreign_owner.owner = OwnerId::new(root, address.owner.first_admission_ordinal());
    assert!(!foreign_owner.matches_carrier(context, address, digest, lineage));

    let mut foreign_ordinal = key;
    foreign_ordinal.ordinal = address
        .ordinal
        .checked_add(1)
        .expect("lineage fixture ordinal remains bounded");
    assert!(!foreign_ordinal.matches_carrier(context, address, digest, lineage));

    let mut foreign_slot = key;
    foreign_slot.slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 1);
    assert!(!foreign_slot.matches_carrier(context, address, digest, lineage));

    let mut foreign_digest = key;
    let first_digest = LifecycleDigest::new([0x93; 32]);
    foreign_digest.digest = if digest == first_digest {
        LifecycleDigest::new([0x94; 32])
    } else {
        first_digest
    };
    assert!(!foreign_digest.matches_carrier(context, address, digest, lineage));

    let opposite = match lineage {
        LifecycleDecisionApplyLineageV1::Live => LifecycleDecisionApplyLineageV1::Recovered,
        LifecycleDecisionApplyLineageV1::Recovered => LifecycleDecisionApplyLineageV1::Live,
    };
    let foreign_lineage = key.with_lineage_for_test(opposite);
    assert_eq!(foreign_lineage.with_lineage_for_test(lineage), key);
    assert!(!foreign_lineage.matches_carrier(context, address, digest, lineage));
}

#[cfg(feature = "bls")]
fn project_live_apply_task_for_lineage_test(
    holder: &LifecycleWorkRegistryHolder,
    address: ConcreteWorkAddress,
    key: LifecycleDecisionApplyDispatchKeyV1,
) -> crate::sumeragi::v2_apply::LifecycleDecisionApplyTaskV1 {
    let work = &holder.registry_for_test().entries[&address];
    let ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) = &work.kind else {
        panic!("lineage fixture expected one genuine live Apply carrier")
    };
    apply
        .project_task(LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(
            key,
        ))
        .expect("project exact live Apply task for lineage test")
}

#[cfg(feature = "bls")]
fn project_recovered_apply_task_for_lineage_test(
    holder: &LifecycleWorkRegistryHolder,
    address: ConcreteWorkAddress,
    key: LifecycleDecisionApplyDispatchKeyV1,
) -> crate::sumeragi::v2_apply::LifecycleDecisionApplyTaskV1 {
    let work = &holder.registry_for_test().entries[&address];
    let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &work.kind else {
        panic!("lineage fixture expected one genuine recovered Apply carrier")
    };
    apply
        .carrier
        .project_recovered_apply_task(
            LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(key),
            address,
        )
        .expect("project exact recovered Apply task for lineage test")
}

#[cfg(feature = "bls")]
fn lifecycle_ledger_frame_for_lineage_test(root: &std::path::Path) -> Vec<u8> {
    std::fs::read(root.join("lifecycle-ledger-v1.norito"))
        .expect("read lifecycle Decision Apply lineage fixture ledger")
}

#[cfg(feature = "bls")]
fn assert_executor_completion_lineage_substitution_is_inert(
    executor: &mut crate::sumeragi::v2_effects::V2EffectExecutor<
        crate::sumeragi::v2_runtime::SerializedV2Runtime,
    >,
    holder: &LifecycleWorkRegistryHolder,
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    exact: &crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1,
    opposite_lineage: LifecycleDecisionApplyLineageV1,
) {
    let crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(completion) =
        exact
    else {
        panic!("executor lineage substitution requires one exact Applied result")
    };
    let registry_before = format!("{:?}", holder.registry_for_test());
    let coordinator_before = format!("{coordinator:?}");
    let (transition, authority) = holder
        .prepare_lifecycle_decision_apply_terminal_transition(coordinator, lease, completion)
        .expect("project exact completion authority before lineage substitution");
    drop(transition);
    executor.assert_lifecycle_apply_completion_lineage_substitution_is_inert_for_test(
        authority,
        opposite_lineage,
    );
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);

    let (transition, exact_authority) = holder
        .prepare_lifecycle_decision_apply_terminal_transition(coordinator, lease, completion)
        .expect("reproject fresh exact completion authority after inert substitution");
    drop(transition);
    let exact_preview = executor
        .prepare_lifecycle_decision_apply_completion(exact_authority)
        .unwrap_or_else(|error| {
            panic!("fresh exact completion authority must still preview: {error}")
        });
    drop(exact_preview);
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
}

#[cfg(feature = "bls")]
fn assert_terminal_lineage_substitution_is_inert(
    holder: &mut LifecycleWorkRegistryHolder,
    coordinator: &LifecycleCoordinator,
    lease: &TurnLease,
    exact: &crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1,
    opposite: &crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1,
    opposite_lineage: LifecycleDecisionApplyLineageV1,
    ledger_root: &std::path::Path,
) {
    let crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(
        opposite_completion,
    ) = opposite
    else {
        panic!("terminal lineage substitution requires one applied result")
    };
    let registry_before = format!("{:?}", holder.registry_for_test());
    let coordinator_before = format!("{coordinator:?}");
    let ledger_before = lifecycle_ledger_frame_for_lineage_test(ledger_root);
    assert!(
        holder
            .prepare_lifecycle_decision_apply_terminal_transition(
                coordinator,
                lease,
                opposite_completion,
            )
            .is_none(),
        "an opposite-lineage completion cannot rejoin the claimed carrier"
    );
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
    assert_eq!(
        lifecycle_ledger_frame_for_lineage_test(ledger_root),
        ledger_before
    );

    let crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Applied(exact_completion) =
        exact
    else {
        panic!("terminal lineage fixture requires one exact applied result")
    };
    let (mut prepared, authority) = holder
        .prepare_lifecycle_decision_apply_terminal_transition(coordinator, lease, exact_completion)
        .expect("exact completion prepares one terminal transition");
    assert_eq!(
        authority.lineage(),
        exact_completion.dispatch_key().lineage()
    );
    drop(authority);
    prepared.substitute_lineage_for_test(opposite_lineage);
    let mut staged = coordinator.stage_durable_transaction();
    staged.reduce_settle_turn(lease.clone(), super::super::TurnOutcome::Advanced, None);
    assert!(staged.fault.is_none());
    let staged_before = format!("{staged:?}");
    let callback_called = Cell::new(false);
    let result = holder.publish_lifecycle_decision_apply_terminal_transition(
        prepared,
        coordinator,
        &staged,
        lease,
        || {
            callback_called.set(true);
            Ok::<(), &'static str>(())
        },
    );
    assert!(matches!(
        result,
        Err(LifecycleDecisionApplyTerminalPublicationErrorV1::Preflight(
            _
        ))
    ));
    assert!(!callback_called.get());
    assert_eq!(format!("{:?}", holder.registry_for_test()), registry_before);
    assert_eq!(format!("{coordinator:?}"), coordinator_before);
    assert_eq!(format!("{staged:?}"), staged_before);
    assert_eq!(
        lifecycle_ledger_frame_for_lineage_test(ledger_root),
        ledger_before
    );
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn assert_lifecycle_decision_apply_live_recovered_substitution_matrix(
    verified: &crate::sumeragi::v2::VerifiedHeightContext,
    live_holder: &mut LifecycleWorkRegistryHolder,
    live_coordinator: &mut LifecycleCoordinator,
    live_ordinal: u128,
    live_address: ConcreteWorkAddress,
    live_tag: crate::sumeragi::v2_core::EventTag,
    live_ledger_root: &std::path::Path,
    live_adapter: crate::sumeragi::v2::SumeragiV2Adapter,
    live_startup: Vec<AdapterEffect>,
    live_cleanup: LiveLifecycleDecisionApplyReconciliationAuthorityV1,
    live_validate_dispatch_key: LifecycleValidateDispatchKeyV1,
    live_body_root: &std::path::Path,
) {
    let live_runtime = crate::sumeragi::v2_runtime::SerializedV2Runtime::new(
        live_adapter,
        live_startup,
        std::time::Instant::now(),
        std::time::Duration::from_secs(10),
        crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("wrap exact live Apply lineage adapter")
    .0;
    let mut live_body_store = V2BodyStore::open(live_body_root, verified.context().clone())
        .expect("reopen exact live Apply lineage body store");
    live_body_store
        .revalidate_recovered_markers(|_| {
            Ok::<_, String>(live_cleanup.validated_receipt().execution_commitment())
        })
        .expect("semantically revalidate exact live Apply lineage body marker");
    let live_body_store_identity = live_body_store.instance_identity();
    let replayed_decision = live_runtime
        .replayed_decision_key()
        .expect("read exact live Apply lineage Decision");
    let recovered_validate_retry_census = live_holder
        .project_recovered_durable_validate_retry_census(live_coordinator, replayed_decision)
        .expect("project empty recovered Validate census beside live Apply");
    let live_output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut live_executor, live_body_store) =
        crate::sumeragi::v2_effects::V2EffectExecutor::open_with_body_store(
            live_runtime,
            live_body_store,
            recovered_validate_retry_census,
            None,
            verified.context().clone(),
            verified.context().roster[0].validator.clone(),
            Some(0),
            std::sync::Arc::clone(&live_output_guard),
            crate::sumeragi::v2_effects::EffectQueueConfig::default(),
        )
        .expect("open exact live Apply lineage executor");
    let (mut live_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let live_planner_io = crate::sumeragi::v2_worker::tests::install_lifecycle_planner_io_for_test(
        &mut live_services,
        verified.context().clone(),
        std::sync::Arc::clone(&live_output_guard),
        live_body_store,
        live_body_store_identity,
        2,
    );
    let live_started_at = std::time::Instant::now();
    live_executor
        .arm_live_clocks(
            super::super::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            live_started_at,
        )
        .expect("arm exact live Apply lineage clocks after service construction");
    live_executor
        .arm_live_lifecycle_validate_successor(
            live_validate_dispatch_key,
            live_cleanup.certificate().proposal_round,
            live_cleanup.subject(),
            true,
        )
        .expect("restore exact published Validate predecessor owner before lineage import");
    let live_certificate = live_cleanup.certificate();
    assert_eq!(
        live_executor
            .reconcile_reopened_decision_for_lifecycle_apply_lineage_test(&mut live_services)
            .expect("reconcile exact live Apply Decision into the lineage executor"),
        (
            live_certificate.round,
            live_certificate.proposal_round,
            live_cleanup.subject(),
            live_certificate.execution_commitment,
        )
    );
    live_executor
        .reconcile_live_lifecycle_decision_apply(live_cleanup, &mut live_services)
        .expect("install exact live Apply executor owner before lineage substitution");

    let (mut recovered, _recovered_safety, recovered_storage) =
        crate::sumeragi::v2::recovered_decision_apply_owner_for_lineage_test(0xE8);
    let (_, recovered_ordinal) = recovered
        .recovered_decision_apply_summary_for_test()
        .expect("genuine recovered owner retains one Ready Decision Apply");
    let recovered_record = &recovered.coordinator.records[&recovered_ordinal];
    let recovered_view = recovered_record.key.round().view();
    let (&recovered_slot, &recovered_digest) = recovered_record
        .physical_slots
        .first_key_value()
        .expect("recovered Apply row retains one physical slot");
    let recovered_address =
        ConcreteWorkAddress::new(recovered_record.owner, recovered_ordinal, recovered_slot)
            .expect("recovered Apply address is exact");
    let recovered_output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
    let (mut recovered_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let (mut recovered_executor, recovered_planner_io) = recovered
        .bind_recovered_apply_executor_for_lineage_test(
            &mut recovered_services,
            std::sync::Arc::clone(&recovered_output_guard),
            2,
        );
    assert!(
        recovered_executor
            .live_lifecycle_decision_apply_key_for_test()
            .is_none(),
        "genuine recovered executor must not inherit the live Apply owner"
    );

    let live_ready_before = (
        format!("{live_coordinator:?}"),
        format!("{:?}", live_holder.registry_for_test()),
    );
    let live_record = &live_coordinator.records[&live_ordinal];
    let mut live_attestation = live_holder
        .attest_ready_lifecycle_decision_apply(live_coordinator, live_ordinal)
        .expect("attest genuine Ready live Apply carrier");
    let live_key = live_attestation.dispatch_key();
    assert_eq!(live_key.lineage(), LifecycleDecisionApplyLineageV1::Live);
    assert_eq!(
        live_executor.live_lifecycle_decision_apply_key_for_test(),
        Some(live_key),
        "live reconciliation and Ready attestation must retain the same complete key"
    );
    assert!(live_attestation.matches_ready_record(live_record));
    live_attestation
        .substitute_dispatch_lineage_for_test(LifecycleDecisionApplyLineageV1::Recovered);
    assert!(!live_attestation.matches_ready_record(live_record));
    assert_eq!(
        (
            format!("{live_coordinator:?}"),
            format!("{:?}", live_holder.registry_for_test()),
        ),
        live_ready_before,
        "Ready live carrier lineage substitution must be read-only"
    );

    let recovered_ready_before = (
        format!("{:?}", recovered.coordinator),
        format!("{:?}", recovered.registry.registry_for_test()),
    );
    let recovered_record = &recovered.coordinator.records[&recovered_ordinal];
    let mut recovered_attestation = recovered
        .registry
        .attest_ready_lifecycle_decision_apply(&recovered.coordinator, recovered_ordinal)
        .expect("attest genuine Ready recovered Apply carrier");
    let recovered_key = recovered_attestation.dispatch_key();
    assert_eq!(
        recovered_key.lineage(),
        LifecycleDecisionApplyLineageV1::Recovered
    );
    assert!(recovered_attestation.matches_ready_record(recovered_record));
    recovered_attestation
        .substitute_dispatch_lineage_for_test(LifecycleDecisionApplyLineageV1::Live);
    assert!(!recovered_attestation.matches_ready_record(recovered_record));
    assert_eq!(
        (
            format!("{:?}", recovered.coordinator),
            format!("{:?}", recovered.registry.registry_for_test()),
        ),
        recovered_ready_before,
        "Ready recovered carrier lineage substitution must be read-only"
    );

    let live_digest = live_holder.registry_for_test().entries[&live_address].digest;
    assert_lifecycle_decision_apply_key_coordinates_are_closed(
        live_key,
        live_coordinator.active_context,
        live_address,
        live_digest,
        LifecycleDecisionApplyLineageV1::Live,
    );
    assert_lifecycle_decision_apply_key_coordinates_are_closed(
        recovered_key,
        recovered.coordinator.active_context,
        recovered_address,
        recovered_digest,
        LifecycleDecisionApplyLineageV1::Recovered,
    );

    {
        let work = &live_holder.registry_for_test().entries[&live_address];
        let ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) = &work.kind else {
            panic!("live lineage fixture changed carrier kind")
        };
        assert!(
            apply
                .project_task(LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(
                    live_key.with_lineage_for_test(LifecycleDecisionApplyLineageV1::Recovered),
                ))
                .is_none(),
            "live carrier must reject the opposite task constructor identity"
        );
    }
    {
        let work = &recovered.registry.registry_for_test().entries[&recovered_address];
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &work.kind else {
            panic!("recovered lineage fixture changed carrier kind")
        };
        assert!(
            apply
                .carrier
                .project_recovered_apply_task(
                    LifecycleDecisionApplyDispatchIdentityV1::from_key_for_test(
                        recovered_key.with_lineage_for_test(LifecycleDecisionApplyLineageV1::Live,),
                    ),
                    recovered_address,
                )
                .is_none(),
            "recovered carrier must reject the opposite task constructor identity"
        );
    }

    let live_lease = claim_ready_apply_for_lineage_test(live_coordinator, live_ordinal);
    let recovered_lease =
        claim_ready_apply_for_lineage_test(&mut recovered.coordinator, recovered_ordinal);
    let live_task = live_holder
        .prepare_lifecycle_decision_apply_dispatch(live_coordinator, &live_lease)
        .expect("prepare exact claimed live Apply")
        .commit_for_worker();
    let recovered_task = recovered
        .registry
        .prepare_lifecycle_decision_apply_dispatch(&recovered.coordinator, &recovered_lease)
        .expect("prepare exact claimed recovered Apply")
        .commit_for_worker();
    assert_eq!(live_task.dispatch_key(), live_key);
    assert_eq!(recovered_task.dispatch_key(), recovered_key);
    drop(live_task);
    drop(recovered_task);

    let live_exact =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::applied_fixture(
            verified.context(),
            project_live_apply_task_for_lineage_test(live_holder, live_address, live_key),
        )
        .expect("build exact live Applied result");
    let live_opposite =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::applied_fixture(
            verified.context(),
            project_live_apply_task_for_lineage_test(live_holder, live_address, live_key)
                .into_lineage_for_test(LifecycleDecisionApplyLineageV1::Recovered, live_tag),
        )
        .expect("build same-coordinate recovered-lineage result from live material");
    let live_deferred_task =
        project_live_apply_task_for_lineage_test(live_holder, live_address, live_key)
            .into_lineage_for_test(LifecycleDecisionApplyLineageV1::Recovered, live_tag);
    let live_deferred = crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Deferred {
        reference: detached_validation_merge_reference(
            live_deferred_task.validated_receipt().durable(),
        ),
        task: live_deferred_task,
    };
    crate::sumeragi::v2_worker::tests::lifecycle_decision_apply_result_substitution_is_inert_for_test(
        live_key,
        &live_opposite,
    );
    crate::sumeragi::v2_worker::tests::lifecycle_decision_apply_result_substitution_is_inert_for_test(
        live_key,
        &live_deferred,
    );

    let recovered_context = recovered.verified.context();
    let recovered_live_tag = crate::sumeragi::v2_core::EventTag::new(
        recovered_context.height,
        recovered_view,
        live_tag.generation(),
    );
    let recovered_exact =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::applied_fixture(
            recovered_context,
            project_recovered_apply_task_for_lineage_test(
                &recovered.registry,
                recovered_address,
                recovered_key,
            ),
        )
        .expect("build exact recovered Applied result");
    let recovered_opposite =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::applied_fixture(
            recovered_context,
            project_recovered_apply_task_for_lineage_test(
                &recovered.registry,
                recovered_address,
                recovered_key,
            )
            .into_lineage_for_test(LifecycleDecisionApplyLineageV1::Live, recovered_live_tag),
        )
        .expect("build same-coordinate live-lineage result from recovered material");
    let recovered_deferred_task = project_recovered_apply_task_for_lineage_test(
        &recovered.registry,
        recovered_address,
        recovered_key,
    )
    .into_lineage_for_test(LifecycleDecisionApplyLineageV1::Live, recovered_live_tag);
    let recovered_deferred =
        crate::sumeragi::v2_apply::LifecycleDecisionApplyWorkerResultV1::Deferred {
            reference: detached_validation_merge_reference(
                recovered_deferred_task.validated_receipt().durable(),
            ),
            task: recovered_deferred_task,
        };
    crate::sumeragi::v2_worker::tests::lifecycle_decision_apply_result_substitution_is_inert_for_test(
        recovered_key,
        &recovered_opposite,
    );
    crate::sumeragi::v2_worker::tests::lifecycle_decision_apply_result_substitution_is_inert_for_test(
        recovered_key,
        &recovered_deferred,
    );

    assert_executor_completion_lineage_substitution_is_inert(
        &mut live_executor,
        live_holder,
        live_coordinator,
        &live_lease,
        &live_exact,
        LifecycleDecisionApplyLineageV1::Recovered,
    );
    assert_eq!(
        live_executor.live_lifecycle_decision_apply_key_for_test(),
        Some(live_key),
        "live executor keeps its exact owner after lineage rejection and exact reprojection"
    );
    assert_executor_completion_lineage_substitution_is_inert(
        &mut recovered_executor,
        &recovered.registry,
        &recovered.coordinator,
        &recovered_lease,
        &recovered_exact,
        LifecycleDecisionApplyLineageV1::Live,
    );
    assert!(
        recovered_executor
            .live_lifecycle_decision_apply_key_for_test()
            .is_none(),
        "recovered executor cannot acquire a live owner through authority substitution"
    );

    assert_terminal_lineage_substitution_is_inert(
        live_holder,
        live_coordinator,
        &live_lease,
        &live_exact,
        &live_opposite,
        LifecycleDecisionApplyLineageV1::Recovered,
        live_ledger_root,
    );
    assert_terminal_lineage_substitution_is_inert(
        &mut recovered.registry,
        &recovered.coordinator,
        &recovered_lease,
        &recovered_exact,
        &recovered_opposite,
        LifecycleDecisionApplyLineageV1::Live,
        &recovered_storage.path().join("ledger"),
    );
    live_planner_io.detach(&mut live_services);
    recovered_planner_io.detach(&mut recovered_services);
}

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
