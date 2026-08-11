    #[test]
    fn exact_install_borrow_and_take_are_one_shot() {
        let work = concrete(effect(1), 91);
        let digest = work.digest;
        let owner = admitted_owner(&work, 1);
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
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
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
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
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, 0x21, slot).expect("valid address");
        let mut registry = ConcreteLifecycleWorkRegistry::default();

        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ =
                registry.install_before_publication(address, digest, work, || -> Result<(), ()> {
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
        let owner = admitted_owner(&first, 2);
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let address = ConcreteWorkAddress::new(owner, 2, slot).expect("valid address");
        let exact_lease = lease(owner, 2, slot, digest);
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
        let wrong_ordinal_lease = lease(owner, 3, slot, digest);
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
            registry.take_for_lease(&lease(owner, 2, slot, wrong_digest), slot),
            Err(RegistryError::DigestMismatch)
        ));
        assert_eq!(registry.len(), 1);
        assert!(matches!(
            registry.rollback_exact(address, wrong_digest),
            Err(RegistryError::DigestMismatch)
        ));
        assert_eq!(registry.len(), 1);
        registry
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
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
        let shared_owner = admitted_owner(&first, 4);
        let first_address = ConcreteWorkAddress::new(shared_owner, 4, slot).expect("first address");
        let second_address =
            ConcreteWorkAddress::new(shared_owner, 5, slot).expect("second address");
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
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
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
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
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
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
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
        let slot =
            super::super::PhysicalSlotId::for_capacity(super::super::CapacityClass::Effect, 0);
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
            .pending_adapter_effect_binding(&first)
            .expect("mint first-effect pending binding");
        let (error, returned_effect, returned_pending) =
            ConcreteLifecycleWork::from_exact(second, pending)
                .expect_err("a foreign effect must return the complete move-only pair");
        assert_eq!(error, RegistryError::UnboundEffect);
        assert!(returned_pending.exactly_binds_adapter_effect(&first));
        assert!(!returned_pending.exactly_binds_adapter_effect(&returned_effect));
        assert!(ConcreteLifecycleWorkRegistry::default().is_empty());
    }

    #[test]
    fn registry_remains_inert_and_scheduler_free() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("registry source has one production prefix");
        for forbidden in [
            "SchedulerInputs".to_owned(),
            "TurnPlan".to_owned(),
            "ready_index:".to_owned(),
            "high_water:".to_owned(),
            "active_lease:".to_owned(),
            "next_lease:".to_owned(),
            "capacity_used:".to_owned(),
            "observed_generation:".to_owned(),
            "producer_debts:".to_owned(),
            "fn plan(".to_owned(),
            "fn settle_turn(".to_owned(),
            "reserve_one".to_owned(),
        ] {
            assert!(
                !production.contains(&forbidden),
                "registry acquired forbidden scheduler authority: {forbidden}"
            );
        }
        let coordinator = include_str!("v2_lifecycle_coordinator.rs");
        assert_eq!(
            coordinator
                .matches(&["work_registry", "::"].concat())
                .count(),
            1,
            "only the opaque Ready-validation preview types may cross the module boundary"
        );
        let export = coordinator
            .split("pub(crate) use work_registry::{")
            .nth(1)
            .expect("coordinator has one narrow registry re-export")
            .split("};")
            .next()
            .expect("registry re-export is bounded");
        assert!(export.contains("PreparedReadyDurableValidateExecution"));
        assert!(export.contains("ReadyDurableValidateOutcomeKind"));
        assert!(export.contains("ReadyValidatedAdapterAuthority"));
        assert!(export.contains("ReadyRejectedAdapterAuthority"));
        assert!(!export.contains("ConcreteLifecycleWorkRegistry"));
        assert!(!export.contains("ReadyDurableValidateExecutionError"));
    }

    #[test]
    fn certified_fetch_execution_surface_is_borrow_bound_and_commit_free() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let execution_impl = source
            .split("impl<'a> PreparedCertifiedFetchExecution<'a>")
            .nth(1)
            .expect("execution token has one typed implementation")
            .split("impl<'a> PreparedCertifiedFetchCompletion<'a>")
            .next()
            .expect("completion conversion follows the execution token");
        assert!(execution_impl.contains("pub(super) fn adapter_preview_inputs"));
        assert!(execution_impl.contains("pub(super) fn durable_body_receipt"));
        assert!(execution_impl.contains("pub(super) fn seal_store_successor"));
        assert!(
            !execution_impl.contains("fn commit("),
            "the inert execution tranche must not mutate or publish its parent/child cut"
        );
        assert!(
            !execution_impl.contains("for_test"),
            "the execution token must not acquire a raw test mint"
        );

        let successor_declaration = source
            .split("pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a>")
            .nth(1)
            .expect("Store successor has one private declaration")
            .split("pub(super) struct PreparedCertifiedFetchCompletion<'a>")
            .next()
            .expect("completion token follows the Store successor");
        assert!(successor_declaration.contains("&'a mut ConcreteLifecycleWorkRegistry"));
        assert!(successor_declaration.contains("_store_effect: AdapterEffect"));
        assert!(successor_declaration.contains("PendingRuntimeEffectBinding"));
        assert!(successor_declaration.contains("DurableBodyReceipt"));
        assert!(successor_declaration.contains("_expected_manifest_hash"));
        assert!(!successor_declaration.contains("derive(Clone"));
    }

    #[test]
    fn durable_store_execution_surface_is_closed_borrow_bound_and_inert() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let carrier = production
            .split("struct DurableStoreBody {")
            .nth(1)
            .expect("durable Store carrier has one declaration")
            .split("impl DurableStoreBody")
            .next()
            .expect("durable Store validation follows its declaration");
        for required in [
            "address: ConcreteWorkAddress",
            "effect: AdapterEffect",
            "pending: PendingRuntimeEffectBinding",
            "durable_receipt: DurableBodyReceipt",
            "expected_manifest_hash: HashOf<wire::PayloadManifest>",
        ] {
            assert!(
                carrier.contains(required),
                "Store carrier omitted {required}"
            );
        }
        assert!(!carrier.contains("derive(Clone"));

        let validation = production
            .split("impl DurableStoreBody {")
            .nth(1)
            .expect("durable Store has one validation implementation")
            .split("struct DurableValidateBody")
            .next()
            .expect("Validate carrier follows Store validation");
        for required in [
            "ConcreteWorkAddress::new",
            "causal_lifecycle_key()",
            "exactly_binds_adapter_effect",
            "exact_effect_identity()",
            "durable_receipt.context_id()",
            "durable_receipt.round()",
            "durable_receipt.subject()",
            "durable_receipt.manifest_hash() == self.expected_manifest_hash",
        ] {
            assert!(
                validation.contains(required),
                "durable Store validation omitted {required}"
            );
        }

        let preparation = production
            .split("pub(super) fn prepare_durable_store_execution(")
            .nth(1)
            .expect("durable Store has one preparation method")
            .split("pub(super) fn prepare_durable_validate_execution(")
            .next()
            .expect("Validate preparation follows Store preparation");
        for required in [
            "projection::admission_request",
            "candidate.key != lease.key()",
            "candidate.causal_root != lease.owner().causal_root()",
            ".physical_geometry",
            ".normalized()",
            "projected_slots != *lease.physical_slots()",
            "projected_universe != lease_slots",
            "projected_consumed != lease_slots",
        ] {
            assert!(
                preparation.contains(required),
                "durable Store preparation omitted {required}"
            );
        }
        assert!(!preparation.contains(".insert("));
        assert!(!preparation.contains(".remove("));

        let execution_impl = production
            .split("impl<'a> PreparedDurableStoreExecution<'a>")
            .nth(1)
            .expect("durable Store token has one implementation")
            .split("impl<'a> PreparedDurableValidateExecution<'a>")
            .next()
            .expect("Validate execution follows Store execution token");
        for required in [
            "pub(super) fn adapter_preview_inputs",
            "pub(super) fn durable_body_receipt",
            "pub(super) fn expected_manifest_hash",
            "pub(super) fn seal_validate_successor",
            "project_store_validate_successor",
            "candidate_statement()",
            "exact_effect_identity()",
        ] {
            assert!(
                execution_impl.contains(required),
                "durable Store execution omitted {required}"
            );
        }
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "for_test",
        ] {
            assert!(
                !execution_impl.contains(forbidden),
                "durable Store token acquired forbidden authority: {forbidden}"
            );
        }

        let validate_token = production
            .split("pub(super) struct PreparedDurableStoreValidateSuccessor<'a>")
            .nth(1)
            .expect("Validate successor has one declaration")
            .split("pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a>")
            .next()
            .expect("Fetch successor follows Validate token");
        assert!(validate_token.contains("&'a mut ConcreteLifecycleWorkRegistry"));
        assert!(validate_token.contains("_validate_effect: AdapterEffect"));
        assert!(validate_token.contains("_validate_pending: PendingRuntimeEffectBinding"));
        assert!(validate_token.contains("_durable_body: DurableBodyReceipt"));
        assert!(validate_token.contains("_expected_manifest_hash"));
        assert!(!validate_token.contains("derive(Clone"));

        let fetch_execution = production
            .split("impl<'a> PreparedCertifiedFetchExecution<'a>")
            .nth(1)
            .expect("certified Fetch execution has one implementation")
            .split("impl<'a> PreparedDurableStoreExecution<'a>")
            .next()
            .expect("durable Store execution follows Fetch execution");
        assert!(fetch_execution.contains("HashOf::new(&response.manifest)"));
        assert!(fetch_execution.contains("_expected_manifest_hash: expected_manifest_hash"));
        assert!(
            !fetch_execution.contains("durable_body.manifest_hash()"),
            "parent manifest authority must not be re-read from the body receipt"
        );

        assert_eq!(
            production
                .matches("fn prepare_durable_store_execution(")
                .count(),
            1,
            "the inert Store preflight must have no production caller"
        );
        for caller_source in [
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
        ] {
            assert!(!caller_source.contains("prepare_durable_store_execution"));
        }
    }

    #[test]
    fn durable_validate_execution_surface_is_closed_borrow_bound_and_inert() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let carrier = production
            .split("struct DurableValidateBody {")
            .nth(1)
            .expect("durable Validate carrier has one declaration")
            .split("impl DurableValidateBody")
            .next()
            .expect("durable Validate validation follows its declaration");
        for required in [
            "address: ConcreteWorkAddress",
            "effect: AdapterEffect",
            "pending: PendingRuntimeEffectBinding",
            "durable_receipt: DurableBodyReceipt",
            "expected_manifest_hash: HashOf<wire::PayloadManifest>",
        ] {
            assert!(
                carrier.contains(required),
                "Validate carrier omitted {required}"
            );
        }
        assert!(!carrier.contains("derive(Clone"));

        let validation = production
            .split("impl DurableValidateBody {")
            .nth(1)
            .expect("durable Validate has one validation implementation")
            .split("enum ConcreteLifecycleWorkKind")
            .next()
            .expect("work kind follows Validate validation");
        for required in [
            "AdapterEffect::ValidateBody",
            "ConcreteWorkAddress::new",
            "self.address.owner.causal_root()",
            "causal_lifecycle_key()",
            "exactly_binds_adapter_effect",
            "exact_effect_identity()",
            "durable_receipt.context_id()",
            "durable_receipt.round()",
            "durable_receipt.subject()",
            "durable_receipt.manifest_hash() == self.expected_manifest_hash",
        ] {
            assert!(
                validation.contains(required),
                "durable Validate validation omitted {required}"
            );
        }
        for forbidden in ["fn new(", "for_test", "derive(Clone", "fn commit("] {
            assert!(
                !validation.contains(forbidden),
                "durable Validate carrier acquired a raw authority seam: {forbidden}"
            );
        }

        let common_work = production
            .split("impl ConcreteLifecycleWork {")
            .nth(1)
            .expect("concrete work has one implementation")
            .split("pub(super) enum CertifiedFetchCompletionError")
            .next()
            .expect("completion errors follow common concrete-work paths");
        assert_eq!(
            common_work
                .matches("ConcreteLifecycleWorkKind::DurableValidateBody")
                .count(),
            5,
            "Validate carrier must remain exhaustive in validation, address, effect, pending, and generic-adapter rejection paths"
        );

        let preparation = production
            .split("pub(super) fn prepare_durable_validate_execution(")
            .nth(1)
            .expect("durable Validate has one preparation method")
            .split("pub(super) fn borrow_for_lease(")
            .next()
            .expect("generic lease borrow follows Validate preparation");
        for required in [
            "LifecycleWorkClass::Validate",
            "LifecyclePhase::Validate",
            "LifecycleStageKind::ValidateBody",
            "PredecessorScope::Independent",
            "projection::admission_request",
            "candidate.key != lease.key()",
            "candidate.causal_root != lease.owner().causal_root()",
            "candidate.initial_state != InitialLifecycleState::Ready",
            "candidate.reconstruction_source != lease.owner().causal_root().digest()",
            "candidate.payload != DurablePayloadReference::None",
            "candidate.producer_turn.is_some()",
            ".physical_geometry",
            ".normalized()",
            "projected_slots.len() != 1",
            "projected_slots != *lease.physical_slots()",
            "projected_universe != lease_slots",
            "projected_consumed != lease_slots",
        ] {
            assert!(
                preparation.contains(required),
                "durable Validate preparation omitted {required}"
            );
        }
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "for_test",
        ] {
            assert!(
                !preparation.contains(forbidden),
                "durable Validate preparation acquired forbidden authority: {forbidden}"
            );
        }

        let execution_impl = production
            .split("impl<'a> PreparedDurableValidateExecution<'a>")
            .nth(1)
            .expect("durable Validate token has one implementation")
            .split("impl PreparedValidatedBodyCompletion<'_>")
            .next()
            .expect("validated completion follows Validate execution token");
        for required in [
            "pub(super) fn adapter_preview_inputs",
            "pub(super) fn durable_body_receipt",
            "pub(super) fn expected_manifest_hash",
            "pub(super) fn durable_validation_wait_source",
            "pub(super) fn seal_waiting_dispatch",
            "pub(super) fn detach",
            "pub(super) fn bind_validated_receipt",
            "AdapterEffect::ValidateBody",
            "self.durable_validate().expected_manifest_hash",
            "validate_validated_receipt_authority",
            "validated_body_completion_digest",
        ] {
            assert!(
                execution_impl.contains(required),
                "durable Validate execution omitted {required}"
            );
        }
        assert_eq!(
            execution_impl.matches("pub(super) fn ").count(),
            7,
            "Validate token may expose only preview coordinates, durable authorities, sealed wait dispatch, owned detach, and success binding"
        );
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "for_test",
            "fn new(",
            "durable_body_receipt().manifest_hash()",
        ] {
            assert!(
                !execution_impl.contains(forbidden),
                "durable Validate token acquired forbidden authority: {forbidden}"
            );
        }

        let completion = production
            .split("pub(super) struct PreparedValidatedBodyCompletion<'a>")
            .nth(1)
            .expect("validated completion has one private declaration")
            .split("pub(super) struct PreparedDurableStoreValidateSuccessor<'a>")
            .next()
            .expect("Store successor follows validated completion declaration");
        for required in [
            "&'a mut ConcreteLifecycleWorkRegistry",
            "incumbent_digest: LifecycleDigest",
            "replacement_digest: LifecycleDigest",
            "validated_receipt: ValidatedBodyReceipt",
        ] {
            assert!(completion.contains(required));
        }
        assert!(!completion.contains("derive(Clone"));

        let completion_impl = production
            .split("impl PreparedValidatedBodyCompletion<'_>")
            .nth(1)
            .expect("validated completion has one implementation")
            .split("// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_BEGIN")
            .next()
            .expect("async Validate handoff follows validated completion");
        for required in [
            "pub(super) const fn adapter_preview_inputs",
            "pub(super) const fn validated_receipt",
            "pub(super) const fn incumbent_digest",
            "pub(super) const fn replacement_digest",
        ] {
            assert!(completion_impl.contains(required));
        }
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "for_test",
        ] {
            assert!(!completion_impl.contains(forbidden));
        }

        let validate_successor = production
            .split("pub(super) struct PreparedDurableStoreValidateSuccessor<'a>")
            .nth(1)
            .expect("Store-to-Validate successor has one declaration")
            .split("pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a>")
            .next()
            .expect("Fetch successor follows Validate successor");
        for required in [
            "&'a mut ConcreteLifecycleWorkRegistry",
            "_store_address: ConcreteWorkAddress",
            "_validate_effect: AdapterEffect",
            "_validate_digest: LifecycleDigest",
            "_validate_pending: PendingRuntimeEffectBinding",
            "_durable_body: DurableBodyReceipt",
            "_expected_manifest_hash: HashOf<wire::PayloadManifest>",
        ] {
            assert!(
                validate_successor.contains(required),
                "Store-to-Validate lineage token omitted {required}"
            );
        }
        assert!(!validate_successor.contains("derive(Clone"));

        assert_eq!(
            production
                .matches("prepare_durable_validate_execution(")
                .count(),
            1,
            "the inert Validate preflight must have no production caller"
        );
        for caller_source in [
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("prepare_durable_validate_execution"));
        }
    }

    #[test]
    fn ready_validate_execution_surface_is_closed_borrow_bound_and_unwired() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let declaration = production
            .split("pub(crate) struct PreparedReadyDurableValidateExecution<'a>")
            .nth(1)
            .expect("Ready Validate token has one declaration")
            .split("// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_BEGIN")
            .next()
            .expect("async handoff follows Ready Validate token");
        assert!(declaration.contains("registry: &'a mut ConcreteLifecycleWorkRegistry"));
        assert!(declaration.contains("address: ConcreteWorkAddress"));
        assert!(declaration.contains("outcome_kind: ReadyDurableValidateOutcomeKind"));
        assert!(
            declaration
                .contains("_adapter: PreparedReadyDurableValidateAdapterPublication<'adapter>")
        );
        assert!(!declaration.contains("derive(Clone"));

        assert_eq!(
            production
                .matches("pub(super) fn prepare_ready_durable_validate_execution(")
                .count(),
            1,
            "the exact Ready completion has one registry entrypoint"
        );
        let preparation = production
            .split("pub(super) fn prepare_ready_durable_validate_execution(")
            .nth(1)
            .expect("Ready Validate preflight exists")
            .split("pub(super) fn reattach_durable_validate_execution(")
            .next()
            .expect("async reattachment follows Ready preflight");
        for required in [
            "LifecycleWorkClass::Validate",
            "LifecyclePhase::Validate",
            "LifecycleStageKind::ValidateBody",
            "PredecessorScope::Independent",
            "validated_lease_address(lease, slot)",
            "ConcreteLifecycleWorkKind::DurableValidateCompletion",
            "completion.validates(work.digest)",
            "candidate_statement.context_id()",
            "candidate_statement.proposal_round()",
            "candidate_statement.subject()",
            "completion.incumbent.expected_manifest_hash",
            "BodyValidationRejectionIdentity::Rejected",
            "validate_validated_receipt_authority",
            "projection::admission_request",
            "candidate.key != lease.key()",
            "projected_slots != incumbent_slots",
            "projected_universe != lease_slots",
            "projected_consumed != lease_slots",
        ] {
            assert!(
                preparation.contains(required),
                "Ready Validate preflight omitted {required}"
            );
        }
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "rejection_reason",
            "EffectWorkId",
            "BodyValidationTask",
            "SchedulerRank",
            "TurnPlan",
        ] {
            assert!(
                !preparation.contains(forbidden),
                "Ready Validate preflight acquired forbidden authority {forbidden}"
            );
        }

        let fixed_join = production
            .split("impl<'registry> PreparedReadyDurableValidateExecution<'registry>")
            .nth(1)
            .expect("Ready Validate token has one implementation")
            .split("impl<'a> PreparedDurableValidateExecution<'a>")
            .next()
            .expect("ordinary Validate execution follows the Ready fixed join");
        for required in [
            "pub(crate) const fn outcome_kind",
            "fn validated_authority",
            "fn rejected_authority",
            "pub(super) fn prepare_adapter_preview",
            "adapter.prepare_sealed_ready_durable_validate_succeeded(authority)",
            "adapter.prepare_sealed_ready_durable_validate_failed(authority)",
            "adapter_preview.preflight_publication()",
            "receipt.durable().manifest_hash()",
            "completion.incumbent.expected_manifest_hash",
            "BodyValidationRejectionIdentity::Rejected",
            "validate_validated_receipt_authority",
        ] {
            assert!(
                fixed_join.contains(required),
                "Ready Validate fixed join omitted {required}"
            );
        }
        for forbidden in [
            "with_validated_preview",
            "with_rejected_preview",
            "FnOnce",
            "-> Option<R>",
            "rejection_reason",
            "fn commit(",
            ".insert(",
            ".remove(",
            "into_parts",
            "pub(crate) fn validated_receipt",
            "pub(crate) fn durable_body_receipt",
            "for_test",
        ] {
            assert!(
                !fixed_join.contains(forbidden),
                "Ready Validate fixed join exposed forbidden authority {forbidden}"
            );
        }

        for caller_source in [
            include_str!("v2.rs"),
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("prepare_ready_durable_validate_execution"));
        }
    }

    #[test]
    fn durable_validate_async_handoff_surface_is_move_only_scheduler_free_and_inert() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let declarations = production
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_BEGIN")
            .expect("detached Validate declarations begin")
            .1
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_END")
            .expect("detached Validate declarations end")
            .0;
        for required in [
            "struct DetachedDurableValidateExecution",
            "address: ConcreteWorkAddress",
            "incumbent_digest: LifecycleDigest",
            "tag: EventTag",
            "round: wire::ConsensusRound",
            "subject: wire::BlockSubject",
            "durable_receipt: DurableBodyReceipt",
            "expected_manifest_hash: HashOf<wire::PayloadManifest>",
            "causal_lifecycle_key: Hash",
            "candidate_statement: Option<RuntimeCandidateSemanticStatement>",
            "lifecycle_key: LifecycleKey",
            "lifecycle_stage: LifecycleStage",
            "struct ExecutedDurableValidateExecution",
            "request: DetachedDurableValidateExecution",
            "outcome: DurableBodyValidationOutcome",
            "struct PreparedDurableValidateCompletion<'a>",
            "&'a mut ConcreteLifecycleWorkRegistry",
        ] {
            assert!(
                declarations.contains(required),
                "detached Validate declarations omitted {required}"
            );
        }
        for forbidden in [
            "derive(Clone",
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "lifecycle_ordinal",
            "ordinal:",
            "TurnLease",
            "WaitToken",
            "ReadyEvent",
            "SchedulerInputs",
            "SchedulerRank",
            "TurnPlan",
            "TurnOutcome",
        ] {
            assert!(
                !declarations.contains(forbidden),
                "detached Validate declarations acquired forbidden scheduler surface: {forbidden}"
            );
        }

        let implementation = production
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_BEGIN")
            .expect("detached Validate implementation begins")
            .1
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_END")
            .expect("detached Validate implementation ends")
            .0;
        assert_eq!(implementation.matches("pub(super) fn execute").count(), 0);
        assert_eq!(implementation.matches("fn execute").count(), 1);
        assert_eq!(
            implementation
                .matches("execute_durable_validation(")
                .count(),
            1
        );
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "lifecycle_ordinal",
            "ordinal:",
            "TurnLease",
            "WaitToken",
            "ReadyEvent",
            "SchedulerInputs",
            "SchedulerRank",
            "TurnPlan",
            "TurnOutcome",
            "into_parts",
            "fn commit(",
            ".insert(",
            ".remove(",
            "enqueue_",
            ".publish_ready(",
            ".replace_before_publication(",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "detached Validate implementation acquired forbidden authority: {forbidden}"
            );
        }

        let reattachment = production
            .split("pub(super) fn reattach_durable_validate_execution(")
            .nth(1)
            .expect("detached Validate has one reattachment method")
            .split("pub(super) fn borrow_for_lease(")
            .next()
            .expect("generic borrow follows detached Validate reattachment");
        for required in [
            "ConcreteWorkAddress::new",
            "work.validates_at(request.address)",
            "work.digest != request.incumbent_digest",
            "DurableValidateBody(validate)",
            "exactly_binds_adapter_effect",
            "causal_lifecycle_key() != &request.causal_lifecycle_key",
            "candidate_statement() != request.candidate_statement",
            "executed.outcome.durable_body() != &request.durable_receipt",
            "validate_validated_receipt_authority(validate, receipt)?",
            "return Err((error, executed))",
        ] {
            assert!(
                reattachment.contains(required),
                "detached Validate reattachment omitted {required}"
            );
        }
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "enqueue_",
            ".publish_ready(",
            ".replace_before_publication(",
        ] {
            assert!(
                !reattachment.contains(forbidden),
                "detached Validate reattachment acquired forbidden mutation: {forbidden}"
            );
        }

        assert_eq!(production.matches("pub(super) fn detach(").count(), 1);
        assert_eq!(
            production
                .matches("pub(super) fn reattach_durable_validate_execution(")
                .count(),
            1
        );
        assert_eq!(production.matches(".detach()").count(), 1);
        for caller_source in [
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("DetachedDurableValidateExecution"));
            assert!(!caller_source.contains("reattach_durable_validate_execution"));
        }
    }

    #[test]
    fn durable_validate_wait_dispatch_is_move_only_single_entry_and_unwired() {
        let registry_source = include_str!("v2_lifecycle_work_registry.rs");
        let registry_production = registry_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let declarations = registry_production
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_BEGIN")
            .expect("wait-dispatch declarations begin")
            .1
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_END")
            .expect("wait-dispatch declarations end")
            .0;
        for required in [
            "struct DurableValidateWakeAuthority",
            "wait_token: WaitToken",
            "struct DurableValidateDispatch",
            "request: DetachedDurableValidateExecution",
            "struct ExecutedDurableValidateDispatch",
            "executed: ExecutedDurableValidateExecution",
        ] {
            assert!(
                declarations.contains(required),
                "wait-dispatch declaration omitted {required}"
            );
        }
        for forbidden in [
            "derive(Clone",
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "lifecycle_ordinal",
        ] {
            assert!(
                !declarations.contains(forbidden),
                "wait-dispatch declaration acquired legacy authority: {forbidden}"
            );
        }

        let implementation = registry_production
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_BEGIN")
            .expect("wait-dispatch implementation begins")
            .1
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_END")
            .expect("wait-dispatch implementation ends")
            .0;
        assert_eq!(implementation.matches("pub(super) fn execute").count(), 1);
        assert!(implementation.contains("request.execute(body_store, validator)"));
        assert!(implementation.contains("Err((error, Self { request, wake }))"));
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "enqueue_",
            "publish_ready",
            "ReadyEvent",
            "replace_before_publication",
            "persist_durable_projection",
            "fn commit(",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "wait-dispatch execution acquired forbidden authority: {forbidden}"
            );
        }
        assert_eq!(
            registry_production.matches("pub(super) fn execute").count(),
            1,
            "the outer dispatch must be the sole externally visible validation execution path"
        );
        assert_eq!(
            registry_production
                .matches("projection::durable_validation_wait_source(")
                .count(),
            1,
            "only the sealed registry preflight may call the raw wait projection"
        );

        let concrete_source = include_str!("v2_lifecycle_concrete_admission.rs");
        let concrete_production = concrete_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("concrete admission has one production prefix");
        assert_eq!(
            concrete_production
                .matches("pub(super) fn begin_durable_validate_dispatch(")
                .count(),
            1
        );
        let entrypoint = concrete_production
            .split("pub(super) fn begin_durable_validate_dispatch(")
            .nth(1)
            .expect("concrete admission has one dispatch entrypoint")
            .split("/// Atomically publish one exact executable Validate result across the")
            .next()
            .expect("Validate completion follows dispatch entrypoint");
        for required in [
            "claimed_durable_validate_record_is_exact",
            "prepare_durable_validate_execution",
            "durable_validation_wait_source",
            "observed_generation",
            "observed_generation == u64::MAX",
            "AliasedWaitSource",
            "stage_durable_transaction",
            "TurnOutcome::Blocked(wait_token)",
            "staged_durable_validate_wait_is_exact",
            "seal_waiting_dispatch(wait_token)",
            "DurableValidateDispatchError, TurnLease",
            "*self = next",
        ] {
            assert!(
                entrypoint.contains(required),
                "dispatch entrypoint omitted {required}"
            );
        }
        let staging = entrypoint
            .find("stage_durable_transaction")
            .expect("entrypoint stages coordinator state");
        let sealing = entrypoint
            .find("seal_waiting_dispatch")
            .expect("entrypoint seals its dispatch");
        let publication = entrypoint
            .find("*self = next")
            .expect("entrypoint publishes its staged coordinator");
        assert!(staging < sealing && sealing < publication);
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "enqueue_",
            "publish_ready",
            "ReadyEvent",
            "replace_before_publication",
            "persist_durable_projection",
            "checked_add(",
            "LeaseId(",
            "SchedulerRank::new",
        ] {
            assert!(
                !entrypoint.contains(forbidden),
                "dispatch entrypoint acquired forbidden authority: {forbidden}"
            );
        }

        let claimed_helper = concrete_production
            .split("fn claimed_durable_validate_record_is_exact(")
            .nth(1)
            .expect("claimed Validate exactness helper exists")
            .split("fn staged_durable_validate_wait_is_exact(")
            .next()
            .expect("staged wait helper follows claimed exactness");
        for required in [
            "filter(|candidate| candidate.ordinal == record.ordinal)",
            "filter(|candidate| candidate.key == record.key)",
            "filter(|ordinal| **ordinal == record.ordinal)",
            "filter(|owner| **owner == record.owner)",
            "record.episode.frozen_predecessors.is_empty()",
            "episode_authority.universe_for(record.key)",
            "episode_authority.admits_slots(",
        ] {
            assert!(
                claimed_helper.contains(required),
                "claimed Validate exactness omitted reverse identity check {required}"
            );
        }
        let staged_helper = concrete_production
            .split("fn staged_durable_validate_wait_is_exact(")
            .nth(1)
            .expect("staged Validate wait helper exists")
            .split("fn concrete_work_location(")
            .next()
            .expect("concrete location helper follows staged wait");
        for required in [
            "next.episode_authority == current.episode_authority",
            "next.ledger_store.is_some() == current.ledger_store.is_some()",
            "next.active_lease.is_none()",
            "next.observed_generation == expected_observed",
        ] {
            assert!(
                staged_helper.contains(required),
                "staged Validate wait omitted exact projection check {required}"
            );
        }

        let projection_source = include_str!("v2_lifecycle_projection.rs");
        let projection = projection_source
            .split("pub(super) fn durable_validation_wait_source(")
            .nth(1)
            .expect("durable validation wait projection exists")
            .split("pub(super) fn reducer_fence_wait_source")
            .next()
            .expect("reducer-fence projection follows durable validation");
        for required in [
            "DURABLE_VALIDATION_WAIT_SOURCE_DOMAIN",
            "owner.causal_root().digest()",
            "owner.first_admission_ordinal()",
            "incumbent_digest",
            "causal_lifecycle_key",
            "candidate_statement",
            "durable_frame_hash",
            "expected_manifest_hash",
            "lifecycle_key",
            "lifecycle_stage",
        ] {
            assert!(
                projection.contains(required),
                "durable validation wait projection omitted {required}"
            );
        }

        for caller_source in [
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("begin_durable_validate_dispatch"));
            assert!(!caller_source.contains("DurableValidateDispatch"));
        }
    }

    #[test]
    fn durable_validate_volatile_completion_is_atomic_move_only_and_unwired() {
        let registry_source = include_str!("v2_lifecycle_work_registry.rs");
        let registry_production = registry_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let carrier = registry_production
            .split("struct DurableValidateCompletion {")
            .nth(1)
            .expect("Validate completion carrier has one declaration")
            .split("enum ConcreteLifecycleWorkKind")
            .next()
            .expect("work-kind inventory follows Validate completion carrier");
        for required in [
            "address: ConcreteWorkAddress",
            "incumbent: DurableValidateBody",
            "incumbent_digest: LifecycleDigest",
            "outcome: DurableBodyValidationOutcome",
            "self.incumbent.validates(self.incumbent_digest)",
            "self.address.owner.causal_root()",
            "exactly_binds_adapter_effect",
            "self.outcome.durable_body() == &self.incumbent.durable_receipt",
            "self.incumbent.durable_receipt.manifest_hash()",
            "self.incumbent.expected_manifest_hash",
            "validate_validated_receipt_authority(&self.incumbent, receipt)",
            "durable_validate_completion_digest(",
            "installed_digest != self.incumbent_digest",
        ] {
            assert!(
                carrier.contains(required),
                "Validate completion carrier omitted {required}"
            );
        }
        for forbidden in ["derive(Clone", "fn new(", "into_parts"] {
            assert!(
                !carrier.contains(forbidden),
                "Validate completion carrier acquired raw or remintable authority: {forbidden}"
            );
        }

        let rejected_digest = registry_production
            .split("fn rejected_body_completion_digest(")
            .nth(1)
            .expect("rejected completion has one digest helper")
            .split("fn durable_validate_outcome_kind(")
            .next()
            .expect("outcome classification follows rejected digest");
        assert!(rejected_digest.contains("identity.canonical_code()"));
        assert!(!rejected_digest.contains("reason"));
        let validated_authority = registry_production
            .split("fn validate_validated_receipt_authority(")
            .nth(1)
            .expect("validated receipt has one shared authority helper")
            .split("fn validated_body_completion_digest(")
            .next()
            .expect("validated digest follows shared authority helper");
        for required in [
            "validated_receipt.durable() != &validate.durable_receipt",
            "validated_receipt.execution_commitment().validate().is_err()",
            "validate.pending.candidate_statement()",
            "statement.context_id() != round.context_id",
            "statement.proposal_round() != *round",
            "statement.subject() != Some(*subject)",
            ".execution_commitment()",
            "DurableValidateExecutionError::ConflictingValidationCommitment",
        ] {
            assert!(
                validated_authority.contains(required),
                "shared validated authority helper omitted {required}"
            );
        }
        assert_eq!(
            registry_production
                .matches("validate_validated_receipt_authority(")
                .count(),
            6,
            "carrier validation, binding, reattachment, Ready preflight, and fixed adapter join must share one helper"
        );

        let declarations = registry_production
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_BEGIN")
            .expect("volatile completion declarations begin")
            .1
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_END")
            .expect("volatile completion declarations end")
            .0;
        for required in [
            "struct DurableValidateCompletionAuthority",
            "lifecycle_key: LifecycleKey",
            "lifecycle_stage: LifecycleStage",
            "struct PublishedValidated",
            "struct PublishedRejected",
            "struct DeferredDurableValidateDispatch",
            "dispatch: ExecutedDurableValidateDispatch",
            "enum DurableValidateCompletionPublication",
            "#[allow(variant_size_differences, clippy::large_enum_variant)]",
            "struct PreparedExecutedDurableValidateCompletion<'a>",
            "struct StagedDurableValidateCompletion<'a>",
            "request: Option<DetachedDurableValidateExecution>",
            "wake: Option<DurableValidateWakeAuthority>",
        ] {
            assert!(
                declarations.contains(required),
                "volatile completion declarations omitted {required}"
            );
        }
        for move_only in [
            "pub(super) struct DeferredDurableValidateDispatch",
            "pub(super) struct PreparedExecutedDurableValidateCompletion<'a>",
            "pub(super) struct StagedDurableValidateCompletion<'a>",
        ] {
            let declaration = declarations
                .split(move_only)
                .next()
                .expect("move-only declaration prefix exists")
                .rsplit("#[derive(")
                .next()
                .expect("derive prefix is inspectable");
            assert!(
                !declaration.contains("Clone"),
                "{move_only} must remain move-only"
            );
        }
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "SchedulerRank",
            "TurnPlan",
        ] {
            assert!(
                !declarations.contains(forbidden),
                "volatile completion declarations acquired legacy scheduler authority: {forbidden}"
            );
        }

        let implementation = registry_production
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_BEGIN")
            .expect("volatile completion implementation begins")
            .1
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_END")
            .expect("volatile completion implementation ends")
            .0;
        for required in [
            "pub(super) fn stage_executable_carrier",
            "ConcreteLifecycleWorkKind::DurableValidateBody(incumbent)",
            "ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)",
            "impl Drop for StagedDurableValidateCompletion<'_>",
            "drop(self.restore())",
            "pub(super) fn missing_reference",
        ] {
            assert!(
                implementation.contains(required),
                "volatile completion implementation omitted {required}"
            );
        }
        assert_eq!(implementation.matches("pub(super) fn commit(").count(), 1);
        let commit = implementation
            .split("pub(super) fn commit(mut self)")
            .nth(1)
            .expect("staged completion has one infallible commit")
            .split("impl Drop for StagedDurableValidateCompletion")
            .next()
            .expect("guard Drop follows commit");
        assert!(commit.contains("self.armed = false;"));
        assert!(commit.contains("self.publication"));
        for forbidden in [
            ".get(", ".insert(", ".remove(", "expect(", "assert", "panic!", "?;", "Result<",
        ] {
            assert!(
                !commit.contains(forbidden),
                "post-swap guard commit acquired a fallible operation: {forbidden}"
            );
        }
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeLifecycleOrdinalSource",
            "SchedulerRank",
            "LeaseId(",
            "next_lease",
            "replace_before_publication",
            "enqueue_",
            "persist_durable_projection",
            "into_parts",
            "pub(super) fn new(",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "volatile completion implementation acquired forbidden authority: {forbidden}"
            );
        }

        let concrete_source = include_str!("v2_lifecycle_concrete_admission.rs");
        let concrete_production = concrete_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("concrete admission has one production prefix");
        assert_eq!(
            concrete_production
                .matches("pub(super) fn complete_durable_validate_dispatch(")
                .count(),
            1,
            "there must be one sealed coordinator completion entrypoint"
        );
        assert_eq!(
            concrete_production
                .matches("prepare_executed_durable_validate_completion(dispatch)")
                .count(),
            1,
            "only the coordinator entrypoint may reattach a full dispatch"
        );
        let entrypoint = concrete_production
            .split("pub(super) fn complete_durable_validate_dispatch(")
            .nth(1)
            .expect("concrete admission has one completion entrypoint")
            .split("/// Atomically admit and register one exact adapter effect.")
            .next()
            .expect("generic admission follows completion entrypoint");
        for required in [
            "prepare_executed_durable_validate_completion(dispatch)",
            "waiting_durable_validate_record_is_exact",
            "prepared.defer_merge_sidecar()",
            "authority.ready_event()",
            "stage_durable_transaction()",
            "publish_ready(ready_event)",
            "staged_durable_validate_ready_is_exact",
            "prepared.stage_executable_carrier()?",
            "core::mem::swap(self, &mut next);\n        let published = staged_registry.commit();",
        ] {
            assert!(
                entrypoint.contains(required),
                "completion entrypoint omitted {required}"
            );
        }
        let coordinator_stage = entrypoint
            .find("stage_durable_transaction()")
            .expect("completion stages a coordinator copy");
        let registry_stage = entrypoint
            .find("prepared.stage_executable_carrier()?")
            .expect("completion stages the exact registry carrier");
        let coordinator_swap = entrypoint
            .find("core::mem::swap(self, &mut next)")
            .expect("completion swaps the checked coordinator copy");
        let registry_commit = entrypoint
            .find("staged_registry.commit()")
            .expect("completion infallibly disarms the registry guard");
        assert!(coordinator_stage < registry_stage);
        assert!(registry_stage < coordinator_swap);
        assert!(coordinator_swap < registry_commit);
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeLifecycleOrdinalSource",
            "SchedulerRank",
            "LeaseId(",
            "next_lease",
            "enqueue_",
            "persist_durable_projection",
            "ledger_store.",
            "replace_before_publication",
        ] {
            assert!(
                !entrypoint.contains(forbidden),
                "completion entrypoint acquired forbidden durable or scheduler machinery: {forbidden}"
            );
        }

        let waiting_exact = concrete_production
            .split("fn waiting_durable_validate_record_is_exact(")
            .nth(1)
            .expect("waiting Validate exactness helper exists")
            .split("fn staged_durable_validate_ready_is_exact(")
            .next()
            .expect("staged Ready helper follows waiting exactness");
        for required in [
            "record.key == authority.lifecycle_key()",
            "record.stage == authority.lifecycle_stage()",
            "record.episode.frozen_predecessors.is_empty()",
            "episode_authority.universe_for(record.key)",
            "episode_authority.admits_slots(",
            "filter(|candidate| candidate.ordinal == record.ordinal)",
            "filter(|candidate| candidate.key == record.key)",
            "filter(|ordinal| **ordinal == record.ordinal)",
            "filter(|owner| **owner == record.owner)",
        ] {
            assert!(
                waiting_exact.contains(required),
                "waiting completion exactness omitted {required}"
            );
        }

        for caller_source in [
            include_str!("v2.rs"),
            include_str!("v2_lifecycle_selector.rs"),
            include_str!("v2_lifecycle_coordinator.rs"),
            include_str!("v2_effects.rs"),
            include_str!("v2_worker.rs"),
            include_str!("v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("complete_durable_validate_dispatch"));
            assert!(!caller_source.contains("DurableValidateCompletionPublication"));
        }
    }

    #[test]
    fn certified_fetch_dequeue_commit_requires_the_durable_token() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let preflight_declaration = production
            .split("pub(super) struct PreparedCertifiedFetchCompletion<'a>")
            .nth(1)
            .expect("selector preflight has one declaration")
            .split("pub(super) struct PreparedDurableCertifiedFetchCompletion<'a>")
            .next()
            .expect("durable token follows selector preflight");
        assert!(!preflight_declaration.contains("DurableCertifiedFetchBodyReceipt"));
        assert!(!preflight_declaration.contains("derive(Clone"));

        let durable_declaration = production
            .split("pub(super) struct PreparedDurableCertifiedFetchCompletion<'a>")
            .nth(1)
            .expect("durable completion token has one declaration")
            .split("pub(super) enum RegistryPublicationError")
            .next()
            .expect("registry publication error follows durable token");
        assert!(durable_declaration.contains("DurableCertifiedFetchBodyReceipt"));
        assert!(!durable_declaration.contains("derive(Clone"));

        let preflight_impl = production
            .split("impl<'a> PreparedCertifiedFetchCompletion<'a>")
            .nth(1)
            .expect("selector preflight has one implementation")
            .split("impl PreparedDurableCertifiedFetchCompletion<'_>")
            .next()
            .expect("durable implementation follows selector preflight");
        assert!(preflight_impl.contains("pub(super) fn bind_durable_body_receipt"));
        assert!(!preflight_impl.contains("fn commit_after_exact_dequeue("));
        assert!(!preflight_impl.contains(".remove("));
        assert!(!preflight_impl.contains(".insert("));

        let durable_impl = production
            .split("impl PreparedDurableCertifiedFetchCompletion<'_>")
            .nth(1)
            .expect("durable completion has one implementation")
            .split("fn ingress_identity_matches_round")
            .next()
            .expect("response helpers follow durable completion");
        assert!(durable_impl.contains("fn commit_after_exact_dequeue("));
        assert_eq!(
            production.matches("fn commit_after_exact_dequeue(").count(),
            1,
            "only the receipt-bound token may own the post-CAS commit"
        );

        let installed_completion = production
            .split("struct CertifiedFetchCompletion {")
            .nth(1)
            .expect("installed completion has one declaration")
            .split("impl CertifiedFetchCompletion")
            .next()
            .expect("installed completion validation follows its declaration");
        assert!(installed_completion.contains("DurableCertifiedFetchBodyReceipt"));

        let durable_binding = production
            .split("fn durable_receipt_matches_fetch(")
            .nth(1)
            .expect("durable response binding has one helper")
            .split("fn exact_dequeued_response_matches(")
            .next()
            .expect("exact dequeue validation follows durable binding");
        for required in [
            "receipt.request_hash()",
            "receipt.response_hash()",
            "durable_body.context_id()",
            "durable_body.round()",
            "durable_body.subject()",
            "durable_body.manifest_hash()",
            "fetch_effect_matches_manifest",
        ] {
            assert!(
                durable_binding.contains(required),
                "durable Fetch binding omitted {required}"
            );
        }
    }
