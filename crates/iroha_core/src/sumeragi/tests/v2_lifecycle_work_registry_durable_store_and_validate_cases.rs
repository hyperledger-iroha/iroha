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
fn durable_store_prepare_rejects_foreign_retained_origin_without_mutation() {
    let DurableStoreFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        ..
    } = durable_store_fixture(0x43);
    {
        let work = registry
            .entries
            .get_mut(&address)
            .expect("foreign-origin fixture retains its Store carrier");
        let installed_digest = work.digest;
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &mut work.kind else {
            unreachable!("foreign-origin fixture retains one Store carrier")
        };
        assert!(store.replay_evidence.replace_with_foreign_origin_for_test());
        assert!(!store.validates(installed_digest));
        assert!(matches!(
            store.project_candidate(&verified),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
    }
    let before = format!("{registry:?}");
    assert!(matches!(
        registry.prepare_durable_store_execution(&lease, slot, &verified),
        Err(DurableStoreExecutionError::Registry(
            RegistryError::CorruptWork
        ))
    ));
    assert_eq!(format!("{registry:?}"), before);
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
    let pending_work = ConcreteLifecycleWork::from_inert_fixture_for_test(effect, pending)
        .expect("construct inert pending Store fixture");
    assert_eq!(pending_work.digest, digest);
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
fn durable_validate_prepare_rejects_foreign_retained_origin_without_mutation() {
    let DurableValidateFixture {
        mut registry,
        verified,
        address,
        lease,
        slot,
        ..
    } = durable_validate_fixture(0x86);
    {
        let work = registry
            .entries
            .get_mut(&address)
            .expect("foreign-origin fixture retains its Validate carrier");
        let installed_digest = work.digest;
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &mut work.kind else {
            unreachable!("foreign-origin fixture retains one Validate carrier")
        };
        assert!(
            validate
                .replay_evidence
                .replace_with_foreign_origin_for_test()
        );
        assert!(!validate.validates(installed_digest));
        assert!(matches!(
            validate.project_candidate(&verified),
            Err(AdapterEffectAdmissionError::InvalidCarrier)
        ));
    }
    let before = format!("{registry:?}");
    assert!(matches!(
        registry.prepare_durable_validate_execution(&lease, slot, &verified),
        Err(DurableValidateExecutionError::Registry(
            RegistryError::CorruptWork
        ))
    ));
    assert_eq!(format!("{registry:?}"), before);
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
    let pending_work = ConcreteLifecycleWork::from_inert_fixture_for_test(effect, pending)
        .expect("construct inert pending Validate fixture");
    assert_eq!(pending_work.digest, digest);
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
#[allow(clippy::too_many_lines)]
fn live_wal_apply_join_rejects_foreign_receipt_and_root_before_exact_retry() {
    let DurableValidateFixture {
        mut registry,
        verified,
        lease,
        slot,
        ..
    } = durable_validate_fixture(0x97);
    let before = format!("{registry:?}");
    let prepared = registry
        .prepare_durable_validate_execution(&lease, slot, &verified)
        .expect("prepare exact Validate for live Apply");
    let validated = ValidatedBodyReceipt::for_test(prepared.durable_body_receipt().clone());
    let (apply, exact_child_pending, foreign_child_pending) = {
        let validate = prepared.durable_validate();
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &validate.effect
        else {
            unreachable!("fixture retains Validate")
        };
        let apply = AdapterEffect::Apply {
            tag: *tag,
            subject: *subject,
            certificate: wire::QuorumCertificate {
                round: *round,
                proposal_round: *round,
                phase: wire::GlobalPhase::Commit,
                subject: *subject,
                execution_commitment: validated.execution_commitment(),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x97; 96],
            },
        };
        let exact_child_pending = validate
            .pending
            .project_validate_apply_successor(&validate.effect, &apply)
            .expect("retained Validate projects exact Apply pending");
        let foreign_owner = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&validate.effect),
            vec![RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                *tag,
                9_700,
                b"foreign live-WAL Validate owner",
            )],
        )
        .expect("bind same effect under foreign causal root")
        .pop()
        .expect("one foreign Validate owner");
        let foreign_predecessor = foreign_owner
            .pending_adapter_effect_binding(&validate.effect)
            .expect("mint foreign Validate pending");
        assert_ne!(
            foreign_predecessor.causal_lifecycle_key(),
            validate.pending.causal_lifecycle_key()
        );
        let foreign_child_pending = foreign_predecessor
            .project_validate_apply_successor(&validate.effect, &apply)
            .expect("foreign Validate projects its own same-effect child");
        (apply, exact_child_pending, foreign_child_pending)
    };
    let foreign_manifest =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign same-coordinate Apply manifest"));
    let exact_durable = prepared.durable_body_receipt();
    let foreign_receipt = DurableBodyReceipt::for_test(
        exact_durable.context_id(),
        exact_durable.round(),
        exact_durable.subject(),
        foreign_manifest,
    );
    let foreign_validated = ValidatedBodyReceipt::for_test(foreign_receipt);
    let Err((error, returned)) = prepared.bind_validated_receipt(foreign_validated) else {
        panic!("foreign same-coordinate receipt cannot construct Apply completion")
    };
    assert_eq!(
        error,
        DurableValidateExecutionError::InvalidValidationReceipt
    );
    drop(returned);
    assert_eq!(format!("{registry:?}"), before);

    let prepared = registry
        .prepare_durable_validate_execution(&lease, slot, &verified)
        .expect("repeat exact Validate after foreign receipt rejection");
    let validated = ValidatedBodyReceipt::for_test(prepared.durable_body_receipt().clone());
    let persisted = SealedLiveWalPersistedEffectV1::from_exact_live_append(
        ExactLiveWalPersistedContinuationCause::Apply {
            wal_identity: LiveWalFrameIdentity::for_test(0, 1, [0; 32]),
            effect: apply.clone(),
        },
    )
    .expect("zero-valued exact live WAL hash seals Apply source");
    let completion = prepared
        .bind_validated_receipt(validated)
        .expect("bind exact retained validation receipt");
    let Ok(exact) = completion.seal_live_wal_apply(persisted, exact_child_pending) else {
        panic!("exact retained receipt must complete Apply authority")
    };
    let LiveWalReplayPreAdmissionOrigin::Apply(completion) = &exact._origin;
    assert!(completion.retained_apply_join_is_exact(&exact._persisted));
    drop(exact);
    assert_eq!(format!("{registry:?}"), before);

    let prepared = registry
        .prepare_durable_validate_execution(&lease, slot, &verified)
        .expect("repeat exact Validate after drop");
    let validated = ValidatedBodyReceipt::for_test(prepared.durable_body_receipt().clone());
    let persisted = SealedLiveWalPersistedEffectV1::from_exact_live_append(
        ExactLiveWalPersistedContinuationCause::Apply {
            wal_identity: LiveWalFrameIdentity::for_test(1, 2, [0x97; 32]),
            effect: apply.clone(),
        },
    )
    .expect("seal repeated exact Apply source");
    let completion = prepared
        .bind_validated_receipt(validated)
        .expect("bind repeated exact validation receipt");
    let Err(error) = completion.seal_live_wal_apply(persisted, foreign_child_pending) else {
        panic!("foreign causal root cannot splice into retained Validate")
    };
    let LiveWalReplayPreAdmissionFailure::Apply {
        _completion: completion,
        _persisted: persisted,
        _pending: foreign_pending,
    } = error._failure;
    drop(foreign_pending);
    let exact_child_pending = {
        let validate = completion
            ._registry
            .entries
            .get(&completion.address)
            .expect("completion keeps Validate installed");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &validate.kind else {
            unreachable!("completion retains durable Validate")
        };
        validate
            .pending
            .project_validate_apply_successor(&validate.effect, &apply)
            .expect("retained predecessor still projects exact Apply")
    };
    let Ok(exact) = completion.seal_live_wal_apply(persisted, exact_child_pending) else {
        panic!("foreign-root rejection must leave source-only seal retryable")
    };
    drop(exact);
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
    let pending = ConcreteLifecycleWork::from_inert_fixture_for_test(effect, pending)
        .expect("construct inert pending Validate fixture");
    assert_eq!(pending.digest, digest);
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

    let (mut deferred_fixture, _deferred_directory, mut deferred_store, deferred_durable) =
        durable_validate_store_fixture(0xA6);
    let deferred_before = format!("{:?}", deferred_fixture.registry);
    let reference = detached_validation_merge_reference(&deferred_durable);
    let deferred = deferred_fixture
        .registry
        .prepare_durable_validate_execution(
            &deferred_fixture.lease,
            deferred_fixture.slot,
            &deferred_fixture.verified,
        )
        .expect("prepare deferred detached Validate")
        .detach()
        .execute(&mut deferred_store, |_| {
            Err::<wire::ExecutionCommitment, _>(DetachedValidationError::MissingMergeSidecar(
                reference.clone(),
            ))
        })
        .expect("execute typed sidecar deferral");
    assert_eq!(deferred.outcome().durable_body(), &deferred_durable);
    assert_eq!(deferred.outcome().missing_merge_sidecar(), Some(&reference));
    let deferred = deferred_fixture
        .registry
        .reattach_durable_validate_execution(deferred)
        .expect("reattach exact sidecar deferral");
    assert_eq!(deferred.outcome().missing_merge_sidecar(), Some(&reference));
    drop(deferred);
    assert_eq!(format!("{:?}", deferred_fixture.registry), deferred_before);
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
    let (_store_replay, validate_replay) = certified_pipeline_replay_evidence_for_test(
        tag,
        &fixture.manifest,
        &validate.durable_receipt,
        &upgraded_validate,
    )
    .expect("rebind certified Validate replay to upgraded in-flight authority");
    validate.pending = upgraded_validate;
    validate.replay_evidence = DurableValidateReplayEvidenceV1::certified(validate_replay);
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
