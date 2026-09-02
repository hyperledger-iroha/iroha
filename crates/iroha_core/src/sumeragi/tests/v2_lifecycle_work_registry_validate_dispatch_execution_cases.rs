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
    let inherited_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
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
#[test]
fn direct_cached_validate_successor_releases_retry_ordinal_before_parent_retirement() {
    let handle = std::thread::Builder::new()
        .name("direct-cached-validate-retry-release".to_owned())
        .stack_size(32 * 1024 * 1024)
        .spawn(direct_cached_validate_successor_releases_retry_ordinal_fixture)
        .expect("spawn direct cached Validate retry-release fixture");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[cfg(feature = "bls")]
fn direct_cached_validate_successor_releases_retry_ordinal_fixture() {
    // Reuse the view-zero committee fixture whose PrepareQC deterministically
    // selects the remote certified-body Fetch path.
    let marker = 0_u8;
    let (mut fixture, _body_directory, mut body_store, durable) =
        durable_validate_store_fixture_at_view(marker, 0);
    let key = (durable.round(), durable.subject());
    let commitment = ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    let marker_outcome = body_store
        .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
            Ok::<_, String>(commitment)
        })
        .expect("persist the direct cached Validate marker");
    assert_eq!(
        marker_outcome
            .validated_receipt()
            .map(ValidatedBodyReceipt::execution_commitment),
        Some(commitment)
    );

    let keys = durable_store_keys(marker);
    let now = std::time::Instant::now();
    let runtime_directory = TempDir::new().expect("temporary direct cached Validate runtime");
    let mut coordinator = ready_durable_validate_coordinator(&[&fixture]);
    let ledger_directory =
        TempDir::new().expect("temporary direct cached Validate lifecycle ledger");
    coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .expect("attach direct cached Validate LedgerV1");
    let (runtime_ordinal_authority, coordinator_ordinal_authority) =
        authority::lifecycle_ordinal_authorities_after_high_watermark(coordinator.high_water());
    let lifecycle_ordinals =
        crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::from_authority(
            runtime_ordinal_authority,
        );
    coordinator
        .bind_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)
        .expect("bind direct cached Validate actor-global ordinal authority");
    let (runtime, retransmit_interval, _) = cold_ready_validate_runtime_at_durable(
        &fixture,
        &durable,
        &keys,
        runtime_directory.path(),
        "direct-cached-parent.wal",
        now,
        lifecycle_ordinals,
    );
    let holder = take_dispatch_registry(&mut fixture);
    let payload_directory = TempDir::new().expect("temporary direct cached payload store");
    let (payload_store, serve_payloads) =
        CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            payload_directory.path(),
            fixture.verified.context(),
        )
        .expect("open empty direct cached Serve payload owner");
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
    executor
        .arm_live_clocks(
            super::super::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            now,
        )
        .expect("arm direct cached Validate clocks after service construction");

    let validate_ordinal = fixture.lease.ordinal();
    assert_eq!(
        executor.validate_retry_lifecycle_ordinal_for_test(key),
        Some(Some(validate_ordinal)),
        "direct cached Validate open binds its exact retry authority"
    );
    assert_eq!(
        owner
            .dispatch_completion_for_test(&mut services, &mut executor, 0)
            .expect("queue the direct cached Validate worker"),
        super::super::ProductionCompletionDispatchV1::ValidateQueued {
            ordinal: validate_ordinal,
        }
    );
    assert!(matches!(
        executor
            .step(now + retransmit_interval, &mut services)
            .expect("select the exact PrepareQC periodic refinement"),
        crate::sumeragi::v2_effects::EffectExecutorStep::Advanced { effects: 1 }
    ));
    let output_summary = executor
        .settle_pending_lifecycle_output_admissions(&mut owner, &mut services)
        .expect("settle the PrepareQC paired with the cached Validate refinement");
    assert_eq!(output_summary.newly_completed(), 1);
    planner_io.activate_one_lifecycle_validate();
    assert_eq!(
        planner_io.execute_held_lifecycle_validate_fixture(
            commitment,
            std::sync::Arc::clone(&output_guard),
        ),
        0,
        "the cached marker must bypass a second validation callback"
    );
    let completion = match services
        .take_next_lifecycle_completion()
        .expect("take the direct cached Validate completion")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::Validate(completion) => completion,
        _ => panic!("direct cached Validate worker produced a foreign completion class"),
    };
    let (executed, ack) = completion.into_publication_parts();
    let publication = owner
        .coordinator
        .complete_durable_validate_dispatch(&mut owner.registry, executed)
        .expect("publish the direct cached validated replacement");
    let super::super::DurableValidateCompletionPublication::PublishedValidated(published) =
        publication
    else {
        panic!("direct cached Validate must publish one validated replacement")
    };
    assert_eq!(published.lifecycle_ordinal(), validate_ordinal);
    ack.acknowledge_after_publication();

    let resolved = owner
        .dispatch_ready_validate_successor_for_test(
            &mut services,
            &mut executor,
            super::ReadyValidateSuccessorV1::from_validated(published),
            0,
        )
        .expect("resolve the direct cached Validate successor");
    let super::super::ReadyValidateSuccessorDispatchV1::Resolved(
        super::super::ProductionCompletionDispatchV1::BodyStageAdvanced {
            parent_ordinal,
            child: LifecycleWorkClass::SignVote,
            ..
        },
    ) = resolved
    else {
        panic!("Prepare-refined cached Validate must advance to its Sign child")
    };
    assert_eq!(parent_ordinal, validate_ordinal);
    assert!(matches!(
        owner.coordinator.records[&validate_ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    ));
    assert!(
        owner
            .registry
            .registry_for_test()
            .entries
            .keys()
            .all(|address| address.ordinal != validate_ordinal),
        "durable parent retirement must remove the exact Validate registry carrier"
    );
    assert_eq!(
        executor.validate_retry_lifecycle_ordinal_for_test(key),
        None,
        "synchronous cached-successor dispatch must release the retired Validate row"
    );
    assert!(!output_guard.restart_required());
    planner_io.detach(&mut services);
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
        durable_local_validate_store_fixture_at_view(marker, 0);
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
    assert_eq!(initial_seal.phase(), None);
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
    let first_output_summary = match first_output_settlement {
        Ok(summary) => summary,
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
    assert_eq!(first_output_summary.newly_completed(), 1);
    assert_eq!(first_output_summary.already_completed(), 0);
    assert!(
        first_output_summary.requires_outer_executor_yield(),
        "fresh service I/O plus terminal publication must yield the outer executor slice"
    );
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
    let duplicate_output_summary = match duplicate_output_settlement {
        Ok(summary) => summary,
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
    assert_eq!(duplicate_output_summary.newly_completed(), 0);
    assert_eq!(duplicate_output_summary.already_completed(), 1);
    assert!(
        !duplicate_output_summary.requires_outer_executor_yield(),
        "an exact terminal duplicate stutters before service I/O and cannot starve ingress"
    );
    assert_eq!(
        services.consensus_broadcast_count_for_test(&prepare_qc_envelope),
        2,
        "a fresh periodic episode must reservice the terminal PrepareQC"
    );
    assert_eq!(
        services
            .pending_exact_prepare_qc_fanouts_for_test(&prepare_qc)
            .expect("inspect the deduplicated PrepareQC output owner"),
        (1, 1),
        "periodic reservice coalesces behind the one byte-identical fanout"
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
    assert_eq!(active_seal.phase(), queued_seal.phase());
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
        .map(|&view| {
            let (fixture, _directory, _store, durable) =
                durable_local_validate_store_fixture_at_view(marker, view);
            (fixture, durable)
        })
        .collect::<Vec<_>>();
    let first_ordinal = fixtures[0].0.lease.ordinal();
    for (offset, (fixture, _)) in fixtures.iter_mut().enumerate().skip(1) {
        let ordinal = first_ordinal
            .checked_add(u128::try_from(offset).expect("plural Validate offset fits u128"))
            .expect("plural Validate ordinal fits u128");
        readdress_durable_validate_fixture(fixture, ordinal);
    }
    let directory = TempDir::new().expect("temporary plural Ready Validate body store");
    let mut store = V2BodyStore::open(directory.path(), fixtures[0].0.verified.context().clone())
        .expect("open plural Ready Validate body store");
    let mut persisted = Vec::with_capacity(fixtures.len());
    let mut durables = Vec::with_capacity(fixtures.len());
    for (fixture, expected_durable) in fixtures {
        let durable = store
            .store(fixture.manifest.clone(), fixture.canonical_wire.clone())
            .expect("persist admission-owned plural Validate body");
        assert_eq!(durable, expected_durable);
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
    let ordinal_by_key = BTreeMap::from([
        (
            (durables[1].round(), durables[1].subject()),
            fixtures[1].lease.ordinal(),
        ),
        (
            (durables[2].round(), durables[2].subject()),
            fixtures[2].lease.ordinal(),
        ),
    ]);
    let selected_before = executor
        .recovered_durable_validate_retry_snapshot_for_test(selected_key)
        .expect("selected Ready key retains its pre-Decision recovered owner");
    let (mut services, _) = crate::sumeragi::v2_worker::tests::fixture();
    executor
        .reconcile_recovered_validate_retry_decision_for_test(selected, false, &mut services)
        .expect("Decision cleanup preserves every still-live recovered retry seal");
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        ready_keys
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
        .expect("terminal Decision cleanup defers every live retry seal to exact resolution");
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        ready_keys
    );

    let selected_ordinal = ordinal_by_key[&selected_key];
    let selected_before_wrong_release = executor
        .recovered_durable_validate_retry_snapshot_for_test(selected_key)
        .expect("selected terminal cleanup retains its live recovered owner");
    let wrong_ordinal = selected_ordinal
        .checked_add(10_000)
        .expect("wrong recovered Validate ordinal");
    assert!(matches!(
        executor.release_validate_retry_lifecycle_ordinal(selected_key, wrong_ordinal),
        Err(crate::sumeragi::v2_effects::EffectExecutorError::Contract(reason))
            if reason.contains("ordinal")
    ));
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        ready_keys
    );
    assert_eq!(
        executor.recovered_durable_validate_retry_snapshot_for_test(selected_key),
        Some(selected_before_wrong_release),
        "wrong-ordinal release must leave the selected owner byte-for-byte inert",
    );

    let other_key = ready_keys
        .iter()
        .copied()
        .find(|key| *key != selected_key)
        .expect("plural Ready census retains one non-selected key");
    assert_eq!(
        executor
            .release_validate_retry_lifecycle_ordinal(other_key, ordinal_by_key[&other_key])
            .expect("release exact non-selected Ready Validate ordinal"),
        true,
    );
    assert_eq!(
        executor.recovered_durable_validate_retry_keys_for_test(),
        vec![selected_key]
    );
    assert_eq!(
        executor
            .release_validate_retry_lifecycle_ordinal(selected_key, selected_ordinal)
            .expect("release exact selected Ready Validate ordinal"),
        true,
    );
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
struct ReadyLocalProposalSignLaunchedFixtureGuard {
    launched: Option<super::super::LaunchedProductionLifecycleV1>,
    planner: Option<crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture>,
}

#[cfg(feature = "bls")]
impl ReadyLocalProposalSignLaunchedFixtureGuard {
    fn new(
        launched: super::super::LaunchedProductionLifecycleV1,
        planner: crate::sumeragi::v2_worker::tests::LifecyclePlannerIoFixture,
    ) -> Self {
        Self {
            launched: Some(launched),
            planner: Some(planner),
        }
    }
}

#[cfg(feature = "bls")]
impl core::ops::Deref for ReadyLocalProposalSignLaunchedFixtureGuard {
    type Target = super::super::LaunchedProductionLifecycleV1;

    fn deref(&self) -> &Self::Target {
        self.launched
            .as_ref()
            .expect("Ready Sign fixture guard retains its launched owner")
    }
}

#[cfg(feature = "bls")]
impl core::ops::DerefMut for ReadyLocalProposalSignLaunchedFixtureGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.launched
            .as_mut()
            .expect("Ready Sign fixture guard retains its launched owner")
    }
}

#[cfg(feature = "bls")]
impl Drop for ReadyLocalProposalSignLaunchedFixtureGuard {
    fn drop(&mut self) {
        match (self.launched.take(), self.planner.take()) {
            (Some(mut launched), Some(planner)) => {
                launched.detach_ready_sign_planner_for_test(planner);
                drop(launched);
            }
            (Some(launched), None) => drop(launched),
            (None, Some(planner)) => drop(planner),
            (None, None) => {}
        }
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
        durable,
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
    coordinator
        .bind_test_lifecycle_ordinal_authority()
        .expect("bind the live Proposal successor ordinal authority");

    let AdapterEffect::Sign { request, .. } = proposal_effect.clone() else {
        unreachable!("local ProposalIntent retains one Proposal Sign")
    };
    let keys = durable_store_keys(marker);
    let signer = usize::try_from(local_validator).expect("local leader index is representable");
    let signature =
        iroha_crypto::Signature::try_new(keys[signer].private_key(), &request.signature_preimage())
            .expect("sign exact local Proposal task");
    let SignRequest::Proposal(mut expected_proposal) = request else {
        unreachable!("local ProposalIntent retains one unsigned Proposal")
    };
    expected_proposal.signature = signature.payload().to_vec();
    let expected_proposal =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(expected_proposal));

    let timeout_round = fixture.manifest.round;
    let timeout_signers = vec![0, 1, 2];
    let timeout_preimage = wire::TimeoutVote {
        round: timeout_round,
        highest_prepare_qc: None,
        signer: timeout_signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let timeout_shares = timeout_signers
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).expect("small timeout signer index");
            iroha_crypto::Signature::new(keys[index].private_key(), &timeout_preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let timeout_aggregate = iroha_crypto::bls_normal_aggregate_signatures(
        &timeout_shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate the pending timeout certificate");
    runtime
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(wire::TimeoutCertificate {
                round: timeout_round,
                groups: vec![wire::TimeoutVoteGroup {
                    highest_prepare_qc: None,
                    signers: timeout_signers,
                    aggregate_signature: timeout_aggregate,
                }],
            }),
        ))
        .expect("queue one authenticated timeout certificate behind the Ready Proposal Sign");
    let queue_now = std::time::Instant::now();
    let queue_before = runtime.queue_snapshot(queue_now);
    assert_eq!(queue_before.progress.depth, 1);

    let mut body_store = V2BodyStore::open(_directory.path(), fixture.verified.context().clone())
        .expect("reopen the local Proposal body store");
    let execution_commitment =
        ValidatedBodyReceipt::for_test(durable.clone()).execution_commitment();
    body_store
        .revalidate_recovered_markers(|_| Ok::<_, String>(execution_commitment))
        .expect("revalidate the local Proposal body marker");
    let payload_directory = TempDir::new().expect("temporary local Proposal payload store");
    let (payload_store, serve_payloads) =
        CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            payload_directory.path(),
            fixture.verified.context(),
        )
        .expect("open the local Proposal payload owner");
    let mut owner = super::super::ProductionLifecycleOwnerV1 {
        verified: fixture.verified.clone(),
        coordinator,
        registry,
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
    crate::sumeragi::v2_worker::tests::install_active_tag_for_test(&mut services, tag);
    let admitted = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let admitted_for_hook = std::sync::Arc::clone(&admitted);
    services.set_exact_output_admission_hook(move |post, _ticket| {
        if let crate::NetworkMessage::SumeragiBlock(message) = post.data {
            admitted_for_hook
                .lock()
                .expect("record admitted Proposal output")
                .push(message.as_message().clone());
        }
        Ok(())
    });
    let (executor, planner_io) = owner.bind_body_store_to_lifecycle_completion_io_for_test(
        &mut services,
        runtime,
        std::sync::Arc::clone(&output_guard),
        local_validator,
        4,
    );
    services
        .set_exact_output_shared_unit_capacity_for_test(64)
        .expect("bind exact output to the local Proposal roster");
    crate::sumeragi::v2_worker::tests::install_local_signer_for_test(&mut services, &keys[signer]);
    let fence = executor.lifecycle_reducer_fence_observation();
    assert_eq!(
        owner.ready_proposal_sign_preempts_bounded_producer_point(fence),
        Ok(true),
        "the genuine Ready local Proposal Sign owns Completion ahead of timeout work"
    );
    let binding_directory = TempDir::new().expect("temporary Ready Sign ingress binding");
    let validator = fixture.verified.context().roster[signer].validator.clone();
    let ingress = super::super::LaunchedProductionLifecycleV1::prepare_ready_local_proposal_sign_ingress_for_test(
        &executor,
        &binding_directory,
        &validator,
    );
    let mut launched =
        super::super::LaunchedProductionLifecycleV1::ready_local_proposal_sign_fixture_for_test(
            owner, executor, services, ingress,
        );
    launched.install_ordinary_completion_head_for_ready_sign_test(&planner_io);
    let (mut lane_work, _) =
        crate::sumeragi::v2_lane_work::tests::fixture(wire::ConsensusMode::Permissioned);
    let (dispatched, after_completion) =
        super::super::v2_runner::with_lifecycle_current_runner_turn_for_test(
            fixture.verified.context(),
            super::super::v2_runner::LifecycleRunnerRankTarget::Completion,
            |runner| {
                let permit =
                    super::super::v2_runner::LifecycleProducerClaimDispositionV1::initial()
                        .ready_proposal_sign_preemption_permit()
                        .expect("an eligible height mints the exact Proposal Sign exception");
                let ready = match launched
                    .drive_completion_pre_gate_with_ready_proposal_sign_preemption(
                        runner,
                        &mut lane_work,
                        &permit,
                    ) {
                    super::super::ProductionLifecycleCompletionPreGateV1::Ready(ready) => ready,
                    super::super::ProductionLifecycleCompletionPreGateV1::Ordinary(runner) => {
                        drop(runner);
                        panic!("the durable Proposal Sign must outrank the retained ordinary head")
                    }
                    super::super::ProductionLifecycleCompletionPreGateV1::Selected(_) => {
                        panic!("the ordinary-head pre-gate cannot settle unrelated lifecycle work")
                    }
                };
                match launched.drive_ready_completion_turn(ready) {
                    super::super::ProductionLifecycleCompletionTurnV1::Selected(
                        super::super::ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(
                            result,
                        ),
                    ) => result.expect("dispatch the genuine Ready local Proposal Sign"),
                    super::super::ProductionLifecycleCompletionTurnV1::PassThrough(runner) => {
                        drop(runner);
                        panic!("the authenticated Proposal Sign cannot pass through Completion")
                    }
                    super::super::ProductionLifecycleCompletionTurnV1::Selected(_) => {
                        panic!("the Proposal Sign selected the wrong Completion class")
                    }
                }
            },
        );
    assert_eq!(
        after_completion,
        super::super::v2_runner::LifecycleRunnerRankTarget::Runtime
    );
    assert_eq!(
        dispatched,
        super::super::ProductionCompletionDispatchV1::SignQueued { ordinal }
    );
    assert!(
        launched.ordinary_completion_head_retained_for_ready_sign_test(),
        "Proposal Sign preemption must not acknowledge or remove the ordinary physical head"
    );
    launched.execute_ready_local_proposal_sign_for_test(
        &planner_io,
        std::sync::Arc::clone(&output_guard),
    );
    let mut launched = ReadyLocalProposalSignLaunchedFixtureGuard::new(launched, planner_io);
    assert_eq!(
        launched.runtime_queue_snapshot_for_ready_sign_test(queue_now),
        queue_before,
        "dispatch and worker completion cannot consume the pending TC"
    );
    assert_eq!(
        launched
            .drain_ordinary_completion_head_for_ready_sign_test()
            .expect("drain only the retained ordinary Completion head"),
        1,
        "the next Completion turn must retire exactly the preempted ordinary owner"
    );
    assert!(
        !launched.ordinary_completion_head_retained_for_ready_sign_test(),
        "ordinary Completion must drain before the recovered Sign result is retained"
    );
    assert!(
        launched
            .retain_recovered_lifecycle_sign_completion()
            .expect("retain the exact recovered Sign completion")
    );
    assert_eq!(
        launched.settle_recovered_lifecycle_proposal_prepare_wal(),
        super::super::ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied
    );
    assert!(
        launched
            .has_pending_exact_output_for_ready_sign_test()
            .expect("inspect the published Proposal output")
    );
    assert_eq!(
        launched.runtime_queue_snapshot_for_ready_sign_test(queue_now),
        queue_before,
        "Proposal settlement must still precede the pending TC"
    );
    let mut output_pending = true;
    for _ in 0..256 {
        output_pending = launched
            .retry_exact_output_for_ready_sign_test()
            .expect("drain the exact Proposal control and chunk fanouts");
        if !output_pending {
            break;
        }
    }
    assert!(
        !output_pending,
        "exact Proposal output must drain boundedly"
    );
    assert!(
        !launched
            .has_pending_exact_output_for_ready_sign_test()
            .expect("inspect the drained Proposal output")
    );
    assert_eq!(
        launched.runtime_queue_snapshot_for_ready_sign_test(queue_now),
        queue_before,
        "exact Proposal output drains before any TimeoutCertificate runtime work"
    );
    assert_eq!(
        admitted
            .lock()
            .expect("inspect admitted Proposal output")
            .iter()
            .filter(|message| match *message {
                crate::BlockMessage::V2(actual) => actual == &expected_proposal,
                _ => false,
            })
            .count(),
        fixture.verified.context().roster.len() - 1,
        "the exact signed Proposal reaches every remote validator once"
    );
    drop(launched);
    assert!(!output_guard.restart_required());
}

#[cfg(feature = "bls")]
impl super::super::ProductionLifecycleOwnerV1 {
    /// Run the genuine Ready local-Proposal Sign driver fixture from v2 tests.
    pub(in crate::sumeragi) fn run_ready_local_proposal_sign_boundary_fixture_for_test() {
        local_proposal_intent_live_wal_sign_fixture();
    }
}

include!("v2_lifecycle_work_registry_validate_apply_cases.rs");
include!("v2_lifecycle_work_registry_validate_completion_cases.rs");
