use crate::sumeragi::v2_lifecycle_coordinator::{
    reviewed_lifecycle_ledger_source_for_test, reviewed_lifecycle_work_registry_source_for_test,
};
#[test]
fn exact_install_borrow_and_take_are_one_shot() {
    let work = concrete(effect(1), 91);
    let digest = work.digest;
    let owner = admitted_owner(&work, 1);
    let slot = super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
    let address = ConcreteWorkAddress::new(owner, 1, slot).expect("valid address");
    let lease = lease(owner, 1, slot, digest);
    let expected = work.effect().clone();
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    registry
        .install(address, digest, work)
        .expect("install exact work");
    assert_eq!(registry.borrow_for_lease(&lease, slot), Ok(&expected));
    let taken = registry
        .take_for_lease(&lease, slot)
        .expect("take complete exact work");
    assert_eq!(taken.effect(), &expected);
    assert!(taken.validate_exact());
    registry
        .install(address, digest, taken)
        .expect("restore the complete token after a deferred outcome");
    assert_eq!(registry.borrow_for_lease(&lease, slot), Ok(&expected));
    let retired = registry
        .take_for_lease(&lease, slot)
        .expect("terminal execution takes the restored token once");
    assert_eq!(retired.effect(), &expected);
    assert!(matches!(
        registry.take_for_lease(&lease, slot),
        Err(RegistryError::Missing)
    ));
    assert!(registry.is_empty());
}
#[test]
fn certified_fetch_execution_rejects_unclosed_or_inexact_leases_without_mutation() {
    let work = concrete(effect(0x31), 0x31);
    let digest = work.digest();
    let expected = work.effect().clone();
    let owner = admitted_owner(&work, 0x31);
    let slot = super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
    let address = ConcreteWorkAddress::new(owner, 0x31, slot).expect("valid exact address");
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    registry
        .install(address, digest, work)
        .expect("install still-pending work");
    let store_lease = lease(owner, 0x31, slot, digest);
    assert!(matches!(
        registry.prepare_certified_fetch_execution(&store_lease, slot),
        Err(CertifiedFetchExecutionError::InvalidLeaseShape)
    ));
    assert!(registry.exactly_contains(address, &expected));
    let exact_fetch_lease = fetch_lease(owner, 0x31, slot, digest);
    assert!(matches!(
        registry.prepare_certified_fetch_execution(&exact_fetch_lease, slot),
        Err(CertifiedFetchExecutionError::WrongWorkKind)
    ));
    assert!(registry.exactly_contains(address, &expected));
    let wrong_digest_lease = fetch_lease(owner, 0x31, slot, LifecycleDigest::new([0xFF; 32]));
    assert!(matches!(
        registry.prepare_certified_fetch_execution(&wrong_digest_lease, slot),
        Err(CertifiedFetchExecutionError::Registry(
            RegistryError::DigestMismatch
        ))
    ));
    assert!(registry.exactly_contains(address, &expected));
    let other_slot =
        super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 1);
    let mut multi_slot_lease = exact_fetch_lease.clone();
    multi_slot_lease
        .physical_slots
        .insert(other_slot, LifecycleDigest::new([0xEE; 32]));
    assert!(matches!(
        registry.prepare_certified_fetch_execution(&multi_slot_lease, slot),
        Err(CertifiedFetchExecutionError::InvalidLeaseShape)
    ));
    assert!(matches!(
        registry.prepare_certified_fetch_execution(&exact_fetch_lease, other_slot),
        Err(CertifiedFetchExecutionError::InvalidLeaseShape)
    ));
    assert!(registry.exactly_contains(address, &expected));
    assert_eq!(registry.len(), 1);
}
#[test]
fn installation_unwind_removes_unpublished_work() {
    let work = concrete(effect(0x21), 0x21);
    let digest = work.digest();
    let owner = admitted_owner(&work, 0x21);
    let slot = super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
    let address = ConcreteWorkAddress::new(owner, 0x21, slot).expect("valid address");
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let _ = registry.install_before_publication(address, digest, work, || -> Result<(), ()> {
            panic!("injected admission publication unwind")
        });
    }));
    assert!(unwind.is_err());
    assert!(registry.is_empty());
}
#[test]
fn mismatches_and_duplicates_never_remove_or_overwrite() {
    let first = concrete(effect(2), 92);
    let digest = first.digest;
    let admitted = admitted_owner(&first, 2);
    let slot = super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
    let address = ConcreteWorkAddress::new(admitted, 2, slot).expect("valid address");
    let exact_lease = lease(admitted, 2, slot, digest);
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    registry
        .install(address, digest, first)
        .expect("install first work");
    let duplicate = concrete(effect(3), 93);
    assert!(matches!(
        registry.install(address, duplicate.digest, duplicate),
        Err((RegistryError::Occupied, _))
    ));
    assert_eq!(registry.len(), 1);
    let wrong_owner = owner(9, 2);
    let wrong_owner_lease = lease(wrong_owner, 2, slot, digest);
    assert!(matches!(
        registry.take_for_lease(&wrong_owner_lease, slot),
        Err(RegistryError::Missing)
    ));
    let wrong_ordinal_lease = lease(admitted, 3, slot, digest);
    assert!(matches!(
        registry.take_for_lease(&wrong_ordinal_lease, slot),
        Err(RegistryError::Missing)
    ));
    let wrong_slot =
        super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 1);
    assert!(matches!(
        registry.take_for_lease(&exact_lease, wrong_slot),
        Err(RegistryError::DigestMismatch)
    ));
    let wrong_digest = LifecycleDigest::new([0xFF; 32]);
    assert!(matches!(
        registry.take_for_lease(&lease(admitted, 2, slot, wrong_digest), slot),
        Err(RegistryError::DigestMismatch)
    ));
    assert_eq!(registry.len(), 1);
    assert!(matches!(
        registry.rollback_exact(address, wrong_digest),
        Err(RegistryError::DigestMismatch)
    ));
    assert_eq!(registry.len(), 1);
    let _rolled_back_work = registry
        .rollback_exact(address, digest)
        .expect("exact rollback returns work");
    assert!(registry.is_empty());
}
#[test]
fn physical_digest_does_not_alias_distinct_logical_addresses() {
    let first = concrete(effect(4), 94);
    let second = concrete(effect(4), 95);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.causal_root(), second.causal_root());
    let digest = first.digest;
    let slot = super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
    let shared_owner = admitted_owner(&first, 4);
    let first_address = ConcreteWorkAddress::new(shared_owner, 4, slot).expect("first address");
    let second_address = ConcreteWorkAddress::new(shared_owner, 5, slot).expect("second address");
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    registry
        .install(first_address, digest, first)
        .expect("install first logical address");
    registry
        .install(second_address, digest, second)
        .expect("install second logical address");
    assert_eq!(registry.len(), 2);
}
#[test]
fn install_rejects_a_foreign_causal_owner_without_consuming_work() {
    let work = concrete(effect(7), 97);
    let digest = work.digest;
    let slot = super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
    let address = ConcreteWorkAddress::new(owner(0xA7, 7), 7, slot)
        .expect("syntactically valid foreign address");
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    let returned = registry
        .install(address, digest, work)
        .expect_err("causal owner mismatch must fail closed");
    assert_eq!(returned.0, RegistryError::CausalOwnerMismatch);
    assert!(returned.1.validate_exact());
    assert!(registry.is_empty());
}
#[test]
fn exact_replacement_commits_or_restores_the_incumbent_atomically() {
    let incumbent = concrete(effect_at_generation(0xB1, 7), 0xB7);
    let replacement = concrete(effect_at_generation(0xB2, 7), 0xB7);
    assert_eq!(incumbent.causal_root(), replacement.causal_root());
    assert_ne!(incumbent.digest(), replacement.digest());
    let incumbent_digest = incumbent.digest();
    let replacement_digest = replacement.digest();
    let incumbent_effect = incumbent.effect().clone();
    let replacement_effect = replacement.effect().clone();
    let owner = admitted_owner(&incumbent, 11);
    let slot = super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
    let address = ConcreteWorkAddress::new(owner, 11, slot).expect("valid address");
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    registry
        .install(address, incumbent_digest, incumbent)
        .expect("install replacement incumbent");
    let error = registry
        .replace_before_publication(
            address,
            incumbent_digest,
            replacement_digest,
            replacement,
            || Err::<(), _>("queue CAS changed"),
        )
        .expect_err("failed publication must restore the incumbent");
    let RegistryReplacementError::Publication(reason, returned) = error else {
        panic!("exact replacement returned an unexpected error variant")
    };
    assert_eq!(reason, "queue CAS changed");
    assert_eq!(returned.effect(), &replacement_effect);
    assert!(returned.validate_exact());
    assert!(registry.exactly_contains(address, &incumbent_effect));
    let (published, retired) = registry
        .replace_before_publication(
            address,
            incumbent_digest,
            replacement_digest,
            returned,
            || Ok::<_, ()>(0xC0DE_u16),
        )
        .expect("exact publication commits the replacement");
    assert_eq!(published, 0xC0DE);
    assert_eq!(retired.effect(), &incumbent_effect);
    assert!(retired.validate_exact());
    assert!(registry.exactly_contains(address, &replacement_effect));
    assert_eq!(registry.len(), 1);
}
#[test]
fn replacement_unwind_restores_the_incumbent() {
    let incumbent = concrete(effect_at_generation(0xD1, 9), 0xD9);
    let replacement = concrete(effect_at_generation(0xD2, 9), 0xD9);
    assert_eq!(incumbent.causal_root(), replacement.causal_root());
    let incumbent_digest = incumbent.digest();
    let replacement_digest = replacement.digest();
    let incumbent_effect = incumbent.effect().clone();
    let owner = admitted_owner(&incumbent, 13);
    let slot = super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
    let address = ConcreteWorkAddress::new(owner, 13, slot).expect("valid address");
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    registry
        .install(address, incumbent_digest, incumbent)
        .expect("install unwind incumbent");
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let _ = registry.replace_before_publication(
            address,
            incumbent_digest,
            replacement_digest,
            replacement,
            || -> Result<(), ()> { panic!("injected publication unwind") },
        );
    }));
    assert!(unwind.is_err());
    assert!(registry.exactly_contains(address, &incumbent_effect));
    assert_eq!(registry.len(), 1);
}
#[test]
fn replacement_validation_never_changes_the_incumbent() {
    let incumbent = concrete(effect_at_generation(0xC1, 8), 0xC8);
    let replacement = concrete(effect_at_generation(0xC2, 8), 0xC8);
    let incumbent_digest = incumbent.digest();
    let replacement_digest = replacement.digest();
    let incumbent_effect = incumbent.effect().clone();
    let incumbent_owner = admitted_owner(&incumbent, 12);
    let slot = super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
    let address = ConcreteWorkAddress::new(incumbent_owner, 12, slot).expect("valid address");
    let mut registry = ConcreteLifecycleWorkRegistry::default();
    registry
        .install(address, incumbent_digest, incumbent)
        .expect("install validation incumbent");
    let wrong_digest = LifecycleDigest::new([0xFF; 32]);
    let error = registry
        .replace_before_publication(
            address,
            wrong_digest,
            replacement_digest,
            replacement,
            || -> Result<(), ()> { unreachable!("validation precedes publication") },
        )
        .expect_err("wrong incumbent digest must reject before mutation");
    let RegistryReplacementError::Validation(RegistryError::DigestMismatch, returned) = error
    else {
        panic!("wrong incumbent digest has one typed failure")
    };
    assert_eq!(returned.digest(), replacement_digest);
    assert!(registry.exactly_contains(address, &incumbent_effect));
    assert_eq!(registry.len(), 1);
    let foreign_owner = owner(0xEE, 12);
    let foreign_address =
        ConcreteWorkAddress::new(foreign_owner, 12, slot).expect("syntactic foreign address");
    let error = registry
        .replace_before_publication(
            foreign_address,
            incumbent_digest,
            replacement_digest,
            returned,
            || -> Result<(), ()> { unreachable!("validation precedes publication") },
        )
        .expect_err("foreign address must reject before mutation");
    assert!(matches!(
        error,
        RegistryReplacementError::Validation(RegistryError::CausalOwnerMismatch, _)
    ));
    assert!(registry.exactly_contains(address, &incumbent_effect));
    assert_eq!(registry.len(), 1);
}
#[test]
fn mismatched_pending_binding_never_becomes_registry_work() {
    let first = effect(5);
    let second = effect(6);
    let tag = match &first {
        AdapterEffect::StoreBody { tag, .. } => *tag,
        _ => unreachable!("registry fixture uses one StoreBody effect"),
    };
    let ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&first),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 96)],
    )
    .expect("bind first effect")
    .pop()
    .expect("one first-effect owner");
    let pending = ownership
        .exact_pending_adapter_effect_binding(&first)
        .expect("mint first-effect pending binding");
    let (error, returned_effect, returned_pending) =
        ConcreteLifecycleWork::from_inert_fixture_for_test(second, pending)
            .expect_err("a foreign effect must return the complete move-only pair");
    assert_eq!(error, RegistryError::UnboundEffect);
    assert!(returned_pending.exactly_binds_adapter_effect(&first));
    assert!(!returned_pending.exactly_binds_adapter_effect(&returned_effect));
    assert!(ConcreteLifecycleWorkRegistry::default().is_empty());
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    remote_proposal_replay_pre_admission_is_closed_exact_and_live
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    invalid_body_replay_pre_admission_is_closed_exact_and_lifecycle_owned
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    live_validate_sign_join_is_linear_opaque_and_scheduler_owned
);
#[test]
fn sealed_validate_no_successor_branch_inventory_is_exact() {
    for publication in [
        ReadyDurableValidateAdapterPublicationKind::ValidatedInactive,
        ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect,
    ] {
        assert_eq!(
            sealed_validate_no_successor_reservation(
                publication,
                ReadyDurableValidateOutcomeKind::Validated,
            ),
            Ok(false)
        );
        assert_eq!(
            sealed_validate_no_successor_reservation(
                publication,
                ReadyDurableValidateOutcomeKind::Rejected,
            ),
            Err(SealedValidateTerminalProjectionError::InvalidCarrier)
        );
    }
    for publication in [
        ReadyDurableValidateAdapterPublicationKind::RejectedInactive,
        ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect,
    ] {
        assert_eq!(
            sealed_validate_no_successor_reservation(
                publication,
                ReadyDurableValidateOutcomeKind::Rejected,
            ),
            Ok(true)
        );
        assert_eq!(
            sealed_validate_no_successor_reservation(
                publication,
                ReadyDurableValidateOutcomeKind::Validated,
            ),
            Err(SealedValidateTerminalProjectionError::InvalidCarrier)
        );
    }
    for publication in [
        ReadyDurableValidateAdapterPublicationKind::ValidatedBusy,
        ReadyDurableValidateAdapterPublicationKind::ValidatedApply,
        ReadyDurableValidateAdapterPublicationKind::ValidatedPersist,
        ReadyDurableValidateAdapterPublicationKind::RejectedBusy,
        ReadyDurableValidateAdapterPublicationKind::RejectedReport,
    ] {
        for outcome in [
            ReadyDurableValidateOutcomeKind::Validated,
            ReadyDurableValidateOutcomeKind::Rejected,
        ] {
            assert_eq!(
                sealed_validate_no_successor_reservation(publication, outcome),
                Err(SealedValidateTerminalProjectionError::InvalidBranch)
            );
        }
    }
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    registry_remains_inert_and_scheduler_free
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    installed_body_projection_and_recovered_prepare_fixture_keep_authority_closed
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    certified_fetch_execution_surface_is_borrow_bound_and_commit_free
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    durable_store_execution_surface_is_closed_borrow_bound_and_inert
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    durable_validate_execution_surface_is_closed_borrow_bound_and_scheduler_owned
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    ready_validate_execution_surface_is_closed_borrow_bound_and_scheduler_owned
);

#[test]
fn live_wal_sign_carrier_uses_typed_dispatch_and_both_signed_successor_families() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("registry has one production prefix");
    let live_install = production
        .split("pub(super) fn install_live_wal_before_publication")
        .nth(1)
        .expect("live WAL admission has one registry transaction")
        .split("/// Install one origin-specific durable Validate carrier")
        .next()
        .expect("live WAL transaction ends before durable Validate admission");
    for required in [
        "PreparedLiveWalCompanionV1::LocalProposal(_)",
        "DurableLiveWalSignOriginV1::LocalProposal",
        "ConcreteLifecycleWorkKind::DurableLiveWalSign(work)",
    ] {
        assert!(
            live_install.contains(required),
            "live ProposalIntent admission omitted typed carrier step {required}"
        );
    }
    let live_validate_install = production
        .split("fn install_live_sign(self, prepared: PreparedLiveValidateSignRegistryWork)")
        .nth(1)
        .expect("live Validate Sign has one registry installation")
        .split("impl LiveValidateApplyRegistryReservation")
        .next()
        .expect("live Validate Sign installation stays bounded");
    assert!(live_validate_install.contains("DurableLiveWalSignOriginV1::Validate"));
    assert!(live_validate_install.contains("into_live_sign_work"));
    assert!(
        !live_validate_install.contains("PendingAdapter"),
        "fsynced live Validate Sign must never re-enter generic effect dispatch"
    );

    let dispatch = production
        .split("pub(super) fn prepare_recovered_lifecycle_sign_dispatch(")
        .nth(1)
        .expect("typed Sign dispatch has one implementation")
        .split("/// Attest one exact Ready recovered Decision Fetch")
        .next()
        .expect("typed Sign dispatch stays bounded");
    for required in [
        "ConcreteLifecycleWorkKind::DurableLiveWalSign(sign)",
        "sign.matches_claimed_record(address, digest, coordinator, lease)",
        "PreparedRecoveredLifecycleSignCarrier::Live(sign)",
        "RecoveredLifecycleSignDispatchProjectionErrorV1::AlreadyDispatched",
    ] {
        assert!(
            dispatch.contains(required),
            "typed live Sign dispatch omitted {required}"
        );
    }

    let broadcast_only = production
        .split("pub(super) fn prepare_recovered_lifecycle_sign_broadcast_successor")
        .nth(1)
        .expect("Broadcast-only Sign successor has one preparation")
        .split("/// Seal the exact Broadcast-and-next-WAL-Sign pair")
        .next()
        .expect("Broadcast-only preparation stays bounded");
    for required in [
        "ConcreteLifecycleWorkKind::DurableLiveWalSign(sign)",
        "sign.project_authenticated_signed_broadcast(verified, projection_authority)",
    ] {
        assert!(
            broadcast_only.contains(required),
            "live Commit Vote completion omitted {required}"
        );
    }

    let broadcast_and_sign = production
        .split("pub(super) fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor")
        .nth(1)
        .expect("Broadcast-and-Sign successor has one preparation")
        .split("impl<'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor")
        .next()
        .expect("combined successor preparation stays bounded");
    for required in [
        "ConcreteLifecycleWorkKind::DurableLiveWalSign(sign)",
        ".project_authenticated_signed_broadcast_and_sign(verified, projection_authority)",
    ] {
        assert!(
            broadcast_and_sign.contains(required),
            "live Prepare Vote or WAL-ahead Proposal completion omitted {required}"
        );
    }
}
