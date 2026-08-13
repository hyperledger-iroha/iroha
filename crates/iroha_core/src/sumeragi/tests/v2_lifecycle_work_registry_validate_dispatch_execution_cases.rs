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
#[allow(clippy::too_many_lines)]
fn assert_ready_validate_commit_sign_live_transaction(attach_ledger: bool) {
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
        .unwrap_or_else(|_| panic!("stage exact sealed Validate-to-Commit transaction"))
        .persist_and_publish()
        .unwrap_or_else(|_| panic!("fsync and publish exact live Validate-to-Commit cut"));
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
        ConcreteLifecycleWorkKind::PendingAdapter {
            effect:
                AdapterEffect::Sign {
                    request: SignRequest::Vote(vote),
                    ..
                },
            ..
        } if vote.phase == wire::GlobalPhase::Commit
    ));
    assert_eq!(
        coordinator.records[&lease.ordinal()].state,
        LifecycleState::Terminal(TerminalOutcome::Advanced)
    );
    assert_eq!(
        coordinator.durable_records[&lease.ordinal()].continuation,
        super::super::schema::DurableContinuation::successor(
            super::super::schema::DurableContinuationEdge::ValidateToSignCommit,
            child_ordinal,
        )
    );
    assert_eq!(
        coordinator.records[&child_ordinal].state,
        LifecycleState::Ready
    );
    assert_eq!(
        coordinator.records[&child_ordinal].stage.kind(),
        LifecycleStageKind::SignCommitVote
    );
    assert!(coordinator.active_lease.is_none());
    assert!(adapter.signature_fence_is_active());
    assert!(matches!(
        adapter.signature_fence_identity(),
        Some((identity_tag, reducer::SignableMessage::Vote(vote)))
            if identity_tag == tag && vote.phase() == reducer::Phase::Commit
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
            super::super::schema::DurableContinuationEdge::ValidateToSignCommit,
            child_ordinal,
        ))
    );
}
#[cfg(feature = "bls")]
#[test]
fn ready_validate_commit_sign_publishes_one_atomic_live_transaction() {
    assert_ready_validate_commit_sign_live_transaction(true);
}
#[cfg(feature = "bls")]
#[test]
fn ready_validate_commit_sign_rejects_missing_ledger_store_and_fails_closed() {
    assert_ready_validate_commit_sign_live_transaction(false);
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
