#[cfg(feature = "bls")]
use super::super::validate_sidecar::{
    LifecycleValidateSidecarRegistrationErrorV1, LifecycleValidateSidecarRegistrationIdentityV1,
    RegisteredLifecycleValidateSidecarWaitV1, cancel_registration_for_test,
    load_registration_for_test, persist_registration_for_test, wake_registration_for_test,
};
#[cfg(feature = "bls")]
use super::super::{CausalRoot, LifecycleValidateDispatchKeyV1};

#[cfg(feature = "bls")]
struct ExactValidateSidecarRegistrationFixture {
    fixture: DurableValidateFixture,
    _body_directory: TempDir,
    coordinator: LifecycleCoordinator,
    holder: LifecycleWorkRegistryHolder,
    _deferred: DeferredDurableValidateDispatch,
    identity: LifecycleValidateSidecarRegistrationIdentityV1,
    _ledger_directory: TempDir,
}

#[cfg(feature = "bls")]
fn exact_validate_sidecar_registration_fixture(
    marker: u8,
) -> ExactValidateSidecarRegistrationFixture {
    let WaitingDurableValidateFixture {
        fixture,
        _directory,
        mut store,
        durable,
        mut coordinator,
        mut holder,
        dispatch,
    } = waiting_durable_validate_sidecar_fixture(marker);
    let reference = exact_detached_validation_merge_reference(&durable);
    let wait = dispatch.wait_token_for_test();
    let AdapterEffect::ValidateBody { round, subject, .. } = fixture.effect.clone() else {
        unreachable!("sidecar fixture retains one Validate effect")
    };
    let mut context = [0_u8; 32];
    context.copy_from_slice(round.context_id.0.as_ref());
    let key = LifecycleValidateDispatchKeyV1::from_recovered_validate_registration(
        LifecycleDigest::new(context),
        round.height,
        fixture.lease.owner(),
        fixture.lease.ordinal(),
        fixture.slot,
        fixture.lease.physical_slots()[&fixture.slot],
    )
    .expect("sidecar fixture reconstructs its sealed Validate key");
    let executed = dispatch
        .execute(&mut store, |_| {
            Err::<wire::ExecutionCommitment, _>(DetachedValidationError::MissingMergeSidecar(
                reference.clone(),
            ))
        })
        .expect("execute exact missing-sidecar Validate");
    let publication = coordinator
        .complete_durable_validate_dispatch(&mut holder, executed)
        .expect("retain exact missing-sidecar Validate outcome");
    let DurableValidateCompletionPublication::DeferredMergeSidecar(deferred) = publication else {
        panic!("missing sidecar must retain one deferred Validate dispatch")
    };
    let identity = deferred
        .sidecar_registration_identity(key)
        .expect("seal exact deferred Validate sidecar identity");
    assert_eq!(identity.round(), round);
    assert_eq!(identity.subject(), subject);
    assert_eq!(identity.wait_token(), wait);
    assert_eq!(identity.reference(), &reference);

    let ledger_directory = TempDir::new().expect("sidecar lifecycle ledger directory");
    coordinator
        .attach_empty_test_ledger(ledger_directory.path())
        .expect("attach exact lifecycle ledger before registration");

    ExactValidateSidecarRegistrationFixture {
        fixture,
        _body_directory: _directory,
        coordinator,
        holder,
        _deferred: deferred,
        identity,
        _ledger_directory: ledger_directory,
    }
}

#[cfg(feature = "bls")]
fn identity_with(
    identity: &LifecycleValidateSidecarRegistrationIdentityV1,
    key: LifecycleValidateDispatchKeyV1,
    wait: WaitToken,
    reference: CertifiedMergeLedgerReference,
) -> LifecycleValidateSidecarRegistrationIdentityV1 {
    LifecycleValidateSidecarRegistrationIdentityV1::from_sealed_dispatch(
        key,
        identity.lifecycle_key(),
        identity.lifecycle_stage(),
        identity.round(),
        identity.subject(),
        wait,
        reference,
    )
    .expect("altered sidecar identity remains structurally well formed")
}

#[cfg(feature = "bls")]
#[test]
fn validate_sidecar_registration_roundtrips_and_duplicate_is_idempotent() {
    let exact = exact_validate_sidecar_registration_fixture(0xE2);
    persist_registration_for_test(&exact.coordinator, &exact.identity)
        .expect("persist exact sidecar registration");
    let first = load_registration_for_test(&exact.coordinator)
        .expect("load exact sidecar registration")
        .expect("registration is present");
    assert_eq!(first, exact.identity);

    persist_registration_for_test(&exact.coordinator, &exact.identity)
        .expect("duplicate exact registration is idempotent");
    assert_eq!(
        load_registration_for_test(&exact.coordinator)
            .expect("reload duplicate sidecar registration"),
        Some(exact.identity.clone())
    );

    let mut foreign_reference = exact.identity.reference().clone();
    foreign_reference.entry_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign exact-shape Validate sidecar entry"));
    let foreign = identity_with(
        &exact.identity,
        exact.identity.dispatch_key(),
        exact.identity.wait_token(),
        foreign_reference,
    );
    assert!(matches!(
        persist_registration_for_test(&exact.coordinator, &foreign),
        Err(LifecycleValidateSidecarRegistrationErrorV1::Persistence(_))
    ));
    assert_eq!(
        load_registration_for_test(&exact.coordinator)
            .expect("foreign duplicate preserves exact registration"),
        Some(exact.identity)
    );
}

#[cfg(feature = "bls")]
#[test]
fn validate_sidecar_wake_preserves_ordinal_carrier_and_advances_one_generation() {
    let mut exact = exact_validate_sidecar_registration_fixture(0xE3);
    persist_registration_for_test(&exact.coordinator, &exact.identity)
        .expect("persist exact sidecar registration");
    let ordinal = exact.identity.dispatch_key().lifecycle_ordinal();
    let before_records = exact.coordinator.records.clone();
    let before_high_water = exact.coordinator.high_water;
    let wait = exact.identity.wait_token();
    let registration_path = exact
        .coordinator
        .ledger_store
        .as_ref()
        .expect("sidecar fixture retains its ledger store")
        .validate_sidecar_registration_path()
        .expect("sidecar fixture has a registration path");

    wake_registration_for_test(&mut exact.coordinator, &exact.identity, &exact.holder)
        .expect("wake the exact registered sidecar wait");

    assert_eq!(exact.coordinator.high_water, before_high_water);
    assert_eq!(exact.coordinator.records.len(), before_records.len());
    assert_eq!(
        exact.coordinator.records[&ordinal].physical_slots,
        before_records[&ordinal].physical_slots
    );
    assert_eq!(
        exact.coordinator.records[&ordinal].state,
        LifecycleState::Ready
    );
    assert!(exact.coordinator.ready_index.contains(&ordinal));
    assert_eq!(
        exact.coordinator.observed_generation[&wait.source()],
        wait.observed_generation() + 1
    );
    assert!(!registration_path.exists());

    let after_first_wake = format!("{:?}", exact.coordinator);
    assert!(matches!(
        wake_registration_for_test(&mut exact.coordinator, &exact.identity, &exact.holder),
        Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity)
    ));
    assert_eq!(format!("{:?}", exact.coordinator), after_first_wake);
}

#[cfg(feature = "bls")]
#[test]
fn validate_sidecar_supersession_cancels_row_and_recovers_post_ledger_cleanup() {
    let mut exact = exact_validate_sidecar_registration_fixture(0xE6);
    persist_registration_for_test(&exact.coordinator, &exact.identity)
        .expect("persist exact sidecar registration");
    let ordinal = exact.identity.dispatch_key().lifecycle_ordinal();
    let registration_path = exact
        .coordinator
        .ledger_store
        .as_ref()
        .expect("sidecar fixture retains its ledger store")
        .validate_sidecar_registration_path()
        .expect("sidecar fixture has a registration path");

    cancel_registration_for_test(
        &mut exact.coordinator,
        &exact.identity,
        &mut exact.holder,
    )
    .expect("cancel the superseded unprotected sidecar row");

    assert_eq!(
        exact.coordinator.records[&ordinal].state,
        LifecycleState::Terminal(TerminalOutcome::Cancelled)
    );
    assert!(!exact.coordinator.ready_index.contains(&ordinal));
    assert!(
        !exact
            .holder
            .registry_for_test()
            .entries
            .contains_key(&exact.fixture.address)
    );
    assert!(!registration_path.exists());
    assert_eq!(
        load_registration_for_test(&exact.coordinator)
            .expect("cancelled registration remains absent"),
        None
    );

    // Model a crash after LedgerV1 publication but before the auxiliary
    // registration unlink. Cold-open recovery must recognize the exact
    // cancelled row and finish that idempotent cleanup instead of failing.
    persist_registration_for_test(&exact.coordinator, &exact.identity)
        .expect("restore the post-ledger registration residual");
    assert!(registration_path.exists());
    assert!(
        RegisteredLifecycleValidateSidecarWaitV1::recover_at_launch(
            &mut exact.coordinator,
            &mut exact.holder,
        )
        .expect("reconcile cancelled registration residual")
        .is_none()
    );
    assert!(!registration_path.exists());
}

#[cfg(feature = "bls")]
#[test]
fn validate_sidecar_cold_open_restores_exact_waiting_row_and_generation() {
    let mut exact = exact_validate_sidecar_registration_fixture(0xE4);
    persist_registration_for_test(&exact.coordinator, &exact.identity)
        .expect("persist exact sidecar registration before restart");
    let ordinal = exact.identity.dispatch_key().lifecycle_ordinal();
    let store = exact
        .coordinator
        .ledger_store
        .clone()
        .expect("sidecar fixture retains its ledger store");
    let physical_slot_universes = exact
        .coordinator
        .records
        .iter()
        .map(|(ordinal, record)| (*ordinal, record.episode.slot_universe.clone()))
        .collect();
    let snapshot = store
        .load()
        .expect("reload exact lifecycle ledger")
        .recovery_snapshot(physical_slot_universes)
        .expect("project exact lifecycle recovery snapshot");
    let mut reopened = LifecycleCoordinator::new_with_authority(
        exact.coordinator.episode_authority.clone(),
        exact.coordinator.high_water,
    );
    reopened.reconcile_restart(snapshot);
    assert!(reopened.fault.is_none());
    let work = exact
        .holder
        .registry_for_test()
        .entries
        .get(&exact.fixture.address)
        .expect("sidecar registry retains its Validate carrier");
    let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
        unreachable!("sidecar registry retains one durable Validate carrier")
    };
    let candidate = validate
        .project_candidate(&exact.fixture.verified)
        .expect("reproject exact recovered Validate carrier");
    assert!(matches!(
        reopened.reduce_admit(AdmissionRequest::Candidate(candidate)),
        AdmissionDecision::Retry {
            owner,
            ordinal: recovered,
            ..
        } if owner == exact.identity.dispatch_key().owner() && recovered == ordinal
    ));
    assert_eq!(reopened.records[&ordinal].state, LifecycleState::Ready);
    assert!(reopened.ready_index.contains(&ordinal));
    reopened.ledger_store = Some(store);

    let recovered =
        RegisteredLifecycleValidateSidecarWaitV1::recover_at_launch(
            &mut reopened,
            &mut exact.holder,
        )
            .expect("recover exact sidecar registration")
            .expect("durable sidecar registration is present");
    assert_eq!(
        reopened.records[&ordinal].state,
        LifecycleState::Waiting(exact.identity.wait_token())
    );
    assert!(!reopened.ready_index.contains(&ordinal));
    assert_eq!(
        reopened.observed_generation[&exact.identity.wait_token().source()],
        exact.identity.wait_token().observed_generation()
    );
    assert_eq!(
        load_registration_for_test(&reopened).expect("registration survives cold-open restore"),
        Some(exact.identity)
    );
    drop(recovered);
}

#[cfg(feature = "bls")]
#[test]
fn validate_sidecar_mismatches_and_preclear_failure_retain_wait_and_registration() {
    let mut exact = exact_validate_sidecar_registration_fixture(0xE5);
    persist_registration_for_test(&exact.coordinator, &exact.identity)
        .expect("persist exact sidecar registration");
    let ordinal = exact.identity.dispatch_key().lifecycle_ordinal();
    let before = format!("{:?}", exact.coordinator);

    let foreign_owner = OwnerId::new(
        CausalRoot::new(LifecycleDigest::new([0xF1; 32])),
        exact
            .identity
            .dispatch_key()
            .owner()
            .first_admission_ordinal(),
    );
    let foreign_owner_key = LifecycleValidateDispatchKeyV1::from_recovered_validate_registration(
        exact.identity.lifecycle_key().context(),
        exact.identity.round().height,
        foreign_owner,
        ordinal,
        exact.identity.dispatch_key().slot(),
        exact.identity.dispatch_key().digest(),
    )
    .expect("construct exact-shape foreign owner key");
    let foreign_owner_identity = identity_with(
        &exact.identity,
        foreign_owner_key,
        exact.identity.wait_token(),
        exact.identity.reference().clone(),
    );
    assert!(matches!(
        wake_registration_for_test(
            &mut exact.coordinator,
            &foreign_owner_identity,
            &exact.holder,
        ),
        Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity)
    ));
    assert_eq!(format!("{:?}", exact.coordinator), before);

    let foreign_generation_identity = identity_with(
        &exact.identity,
        exact.identity.dispatch_key(),
        WaitToken::new(
            exact.identity.wait_token().source(),
            exact.identity.wait_token().observed_generation() + 1,
        ),
        exact.identity.reference().clone(),
    );
    assert!(matches!(
        wake_registration_for_test(
            &mut exact.coordinator,
            &foreign_generation_identity,
            &exact.holder,
        ),
        Err(LifecycleValidateSidecarRegistrationErrorV1::InvalidIdentity)
    ));
    assert_eq!(format!("{:?}", exact.coordinator), before);

    let mut foreign_reference = exact.identity.reference().clone();
    foreign_reference.entry_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign wake Validate sidecar entry"));
    let foreign_reference_identity = identity_with(
        &exact.identity,
        exact.identity.dispatch_key(),
        exact.identity.wait_token(),
        foreign_reference,
    );
    assert!(matches!(
        wake_registration_for_test(
            &mut exact.coordinator,
            &foreign_reference_identity,
            &exact.holder,
        ),
        Err(LifecycleValidateSidecarRegistrationErrorV1::Persistence(_))
    ));
    assert_eq!(format!("{:?}", exact.coordinator), before);
    assert_eq!(
        load_registration_for_test(&exact.coordinator)
            .expect("mismatches preserve exact registration"),
        Some(exact.identity.clone())
    );

    let registration_path = exact
        .coordinator
        .ledger_store
        .as_ref()
        .expect("sidecar fixture retains its ledger store")
        .validate_sidecar_registration_path()
        .expect("sidecar fixture has a registration path");
    let missing_parent = TempDir::new().expect("missing-parent sidecar test root");
    exact
        .coordinator
        .redirect_test_ledger_to_missing_parent(missing_parent.path());
    let before_preclear_failure = format!("{:?}", exact.coordinator);
    assert!(matches!(
        wake_registration_for_test(&mut exact.coordinator, &exact.identity, &exact.holder),
        Err(LifecycleValidateSidecarRegistrationErrorV1::Persistence(_))
    ));
    assert_eq!(format!("{:?}", exact.coordinator), before_preclear_failure);
    assert!(registration_path.exists());
    assert_eq!(
        exact.coordinator.records[&ordinal].state,
        LifecycleState::Waiting(exact.identity.wait_token())
    );
}
