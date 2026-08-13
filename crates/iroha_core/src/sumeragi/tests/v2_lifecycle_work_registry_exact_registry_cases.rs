use crate::sumeragi::v2_lifecycle_coordinator::{
    reviewed_lifecycle_ledger_source_for_test,
    reviewed_lifecycle_work_registry_source_for_test,
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
fn direct_signed_replay_pre_admission_is_closed_exact_and_drop_inert() {
    let tag = EventTag::new(7, 2, Generation::new(1));
    let first_vote = direct_signed_vote(0xD1, 0xD2);
    let broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::Vote(first_vote.clone()),
    ));
    let broadcast_pending = direct_signed_pending(&broadcast, tag, 1);
    let registry = ConcreteLifecycleWorkRegistry::default();
    let before = format!("{registry:?}");
    let Ok(broadcast) =
        PreparedDirectSignedReplayPreAdmission::seal_exact(broadcast, broadcast_pending)
    else {
        panic!("exact signed Broadcast seals its pre-admission evidence")
    };
    assert!(broadcast.validates());
    assert!(matches!(
        &broadcast.replay_evidence,
        DirectSignedReplayEvidenceV1::Broadcast(_)
    ));
    drop(broadcast);
    assert_eq!(format!("{registry:?}"), before);

    let second_vote = direct_signed_vote(0xD1, 0xD3);
    let report = AdapterEffect::ReportEquivocation {
        evidence: crate::sumeragi::v2::AdapterEquivocationEvidence::vote_for_test(
            first_vote,
            second_vote,
        ),
    };
    let report_pending = direct_signed_pending(&report, tag, 2);
    let Ok(report) = PreparedDirectSignedReplayPreAdmission::seal_exact(report, report_pending)
    else {
        panic!("exact authenticated conflict seals its pre-admission evidence")
    };
    assert!(report.validates());
    assert!(matches!(
        &report.replay_evidence,
        DirectSignedReplayEvidenceV1::ReportEquivocation(_)
    ));
    drop(report);
    assert_eq!(format!("{registry:?}"), before);

    let unsupported = effect(0xD4);
    let AdapterEffect::StoreBody {
        tag: unsupported_tag,
        ..
    } = &unsupported
    else {
        unreachable!("unsupported fixture is StoreBody")
    };
    let unsupported_pending = direct_signed_pending(&unsupported, *unsupported_tag, 3);
    assert!(
        PreparedDirectSignedReplayPreAdmission::seal_exact(unsupported, unsupported_pending,)
            .is_err()
    );
    assert_eq!(format!("{registry:?}"), before);
}

#[test]
fn live_wal_pre_admission_surface_is_closed_and_has_one_apply_join() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("work registry has one production prefix");
    let token = production
        .split("pub(super) struct PreparedLiveWalReplayPreAdmission<'a>")
        .nth(1)
        .expect("live WAL pre-admission token has one declaration")
        .split("/// Move-only Validate projection sealed under its closed durable Store parent.")
        .next()
        .expect("Store-to-Validate token follows live WAL token");
    for required in [
        "_persisted: SealedLiveWalPersistedEffectV1",
        "LiveWalReplayPreAdmissionOrigin<'a>",
        "PayloadFree",
        "Apply(PreparedValidatedBodyCompletion<'a>)",
        "seal_payload_free(\n        persisted: SealedLiveWalPersistedEffectV1",
        "persisted.exactly_binds_payload_free_pending()",
    ] {
        assert!(
            token.contains(required),
            "live WAL token omitted {required}"
        );
    }
    for required in [
        "pub(super) fn seal_live_wal_apply(",
        "retained_validated_receipt_is_exact()",
        "project_validate_apply_successor",
        "retained_apply_join_is_exact(&persisted)",
    ] {
        assert!(
            production.contains(required),
            "live WAL Apply join omitted {required}"
        );
    }
    for forbidden in [
        "#[derive(Clone",
        "Option<SealedLiveWalPersistedEffectV1>",
        "pub(super) fn effect(",
        "pub(super) fn pending(",
        "pub(super) fn receipt(",
        "pub(super) fn source(",
        "into_parts",
        "fn install(",
        "fn commit(",
    ] {
        assert!(
            !token.contains(forbidden),
            "live WAL token exposed forbidden surface {forbidden}"
        );
    }
    assert_eq!(
        production.matches(".complete_exact_apply(").count(),
        1,
        "only the fixed retained-Validate join supplies an Apply receipt"
    );
    assert_eq!(
        production.matches(".seal_live_wal_apply(").count(),
        0,
        "the inert prerequisite has no production admission caller"
    );
    for outside in [
        reviewed_lifecycle_ledger_source_for_test(),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        assert!(!outside.contains("PreparedLiveWalReplayPreAdmission"));
    }
}

#[test]
fn direct_signed_replay_pre_admission_surface_is_move_only_inert_and_unwired() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("registry has one production prefix");
    let token = production
        .split("pub(super) struct PreparedDirectSignedReplayPreAdmission {")
        .nth(1)
        .expect("direct signed pre-admission token has one declaration")
        .split("/// Closed concrete form of one fsynced recovered WAL `Sign` successor.")
        .next()
        .expect("recovered WAL carrier follows direct signed token");
    for required in [
        "effect: AdapterEffect",
        "pending: PendingRuntimeEffectBinding",
        "replay_evidence: DirectSignedReplayEvidenceV1",
        "Broadcast(SignedBroadcastReplayEvidenceV1)",
        "ReportEquivocation(SignedEquivocationReplayEvidenceV1)",
        "pub(super) fn seal_exact(",
        "SignedBroadcastReplayEvidenceV1::from_exact_effect(&effect, &pending)",
        "SignedEquivocationReplayEvidenceV1::from_exact_effect(&effect, &pending)",
        "evidence.exactly_matches_effect(&self.effect, &self.pending)",
        "_effect: AdapterEffect",
        "_pending: PendingRuntimeEffectBinding",
    ] {
        assert!(
            token.contains(required),
            "direct signed pre-admission token omitted {required}"
        );
    }
    let declaration = token
        .split('}')
        .next()
        .expect("direct signed token declaration is bounded");
    assert!(!declaration.contains("Option<"));
    assert!(!declaration.contains("derive(Clone"));
    assert!(!declaration.contains("derive(Debug"));
    for forbidden in [
        "fn new(",
        "fn from_parts(",
        "fn into_parts(",
        "fn effect(",
        "fn pending(",
        "fn replay_evidence(",
        "fn install(",
        "fn commit(",
        "ConcreteLifecycleWorkRegistry",
        ".entries",
        "PendingAdapter {",
    ] {
        assert!(
            !token.contains(forbidden),
            "direct signed pre-admission token acquired forbidden authority {forbidden}"
        );
    }
    assert_eq!(
        production.matches("pub(super) fn seal_exact(").count(),
        1,
        "the token has one exact seal"
    );
    assert_eq!(
        production
            .matches("PreparedDirectSignedReplayPreAdmission::seal_exact(")
            .count(),
        0,
        "the inert token must have no production caller"
    );
    for caller in [
        include_str!("../v2.rs"),
        include_str!("../v2_lifecycle_selector.rs"),
        include_str!("../v2_lifecycle_concrete_admission.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        let caller = caller
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("caller production prefix is bounded");
        assert!(!caller.contains("PreparedDirectSignedReplayPreAdmission"));
    }
}

#[test]
fn remote_proposal_replay_pre_admission_is_closed_exact_and_unwired() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("work registry has one production prefix");
    let token = production
        .split("pub(super) struct PreparedRemoteProposalFetchReplayPreAdmission {")
        .nth(1)
        .expect("remote Proposal replay token has one declaration")
        .split("/// Closed concrete form of one fsynced recovered WAL `Sign` successor.")
        .next()
        .expect("recovered WAL carrier follows remote Proposal replay tokens");
    for required in [
        "PreparedRemoteProposalStoreReplayPreAdmission",
        "PreparedRemoteProposalStoredReplayPreAdmission",
        "PreparedRemoteProposalValidateReplayPreAdmission",
        "replay_evidence: RemoteProposalFetchReplayEvidenceV1",
        "replay_evidence: RemoteProposalStoreReplayEvidenceV1",
        "replay_evidence: RemoteProposalStoredReplayEvidenceV1",
        "replay_evidence: RemoteProposalValidateReplayEvidenceV1",
        "pub(super) fn seal_exact_fetch(",
        "ownership.exact_remote_proposal_fetch_replay(&effect)",
        "pub(super) fn project_store(",
        ".project_exact_store(&effect, &pending)",
        "pub(super) fn bind_durable_body(",
        ".bind_durable_body(&effect, &durable_receipt)",
        "pub(super) fn project_validate(",
        ".project_exact_validate(",
        "fn into_durable_validate_carrier(",
        "replay_evidence: DurableValidateReplayEvidenceV1::remote_proposal(",
        "_fetch: PreparedRemoteProposalFetchReplayPreAdmission",
        "_store: PreparedRemoteProposalStoreReplayPreAdmission",
        "_stored: PreparedRemoteProposalStoredReplayPreAdmission",
        "_ownership: RuntimeEffectOwnership",
    ] {
        assert!(
            token.contains(required),
            "remote Proposal replay token omitted {required}"
        );
    }
    for declaration in [
        "PreparedRemoteProposalFetchReplayPreAdmission {",
        "PreparedRemoteProposalStoreReplayPreAdmission {",
        "PreparedRemoteProposalStoredReplayPreAdmission {",
        "PreparedRemoteProposalValidateReplayPreAdmission {",
    ] {
        let declaration = token
            .split(declaration)
            .nth(1)
            .expect("remote Proposal token declaration is present")
            .split('}')
            .next()
            .expect("remote Proposal token declaration is bounded");
        assert!(!declaration.contains("Option<"));
        assert!(!declaration.contains("derive(Clone"));
    }
    for forbidden in [
        "Decode",
        "fn from_parts(",
        "fn into_parts(",
        "fn effect(",
        "fn pending(",
        "fn receipt(",
        "fn source(",
        "fn ingress(",
        "fn proposal(",
        "fn install(",
        "fn commit(",
        "ConcreteLifecycleWorkRegistry",
        ".entries",
        "!= [0; 32]",
        "== [0; 32]",
        "is_zero()",
    ] {
        assert!(
            !token.contains(forbidden),
            "remote Proposal replay token exposed forbidden surface {forbidden}"
        );
    }
    assert_eq!(
        production
            .matches("PreparedRemoteProposalFetchReplayPreAdmission::seal_exact_fetch(")
            .count(),
        0,
        "the inert remote Proposal token has no production admission caller"
    );
    for caller in [
        include_str!("../v2.rs"),
        include_str!("../v2_lifecycle_selector.rs"),
        include_str!("../v2_lifecycle_concrete_admission.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        let caller = caller
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("caller production prefix is bounded");
        assert!(!caller.contains("PreparedRemoteProposalFetchReplayPreAdmission"));
    }
}

#[test]
fn invalid_body_replay_pre_admission_is_closed_exact_and_unwired() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("work registry has one production prefix");
    let token = production
        .split("pub(super) struct PreparedInvalidBodyReportReplayPreAdmission")
        .nth(1)
        .expect("invalid-body replay token has one declaration")
        .split("/// Ownership-preserving failure from the fixed Ready Validate adapter join.")
        .next()
        .expect("Ready Validate preview failure follows invalid-body replay");
    for required in [
        "registry: PreparedReadyDurableValidateExecution",
        "adapter: PreparedInvalidBodyReportAdapterReplay",
        "preview: PreparedReadyDurableValidateAdapterPreview",
        "pub(super) fn seal_invalid_body_report_replay(",
        "ReadyDurableValidateOutcomeKind::Rejected",
        "BodyValidationRejectionIdentity::Rejected",
        "let validate_origin = completion.incumbent.replay_evidence.clone()",
        "adapter.seal_invalid_body_report_replay(",
        "&completion.incumbent.effect",
        "&completion.incumbent.pending",
        "&completion.incumbent.durable_receipt",
        "Err(adapter) =>",
        "preview: Self",
        "fn validates(&self)",
        "pub(super) fn project_for_body_transition(",
        "SealedInvalidBodyReportProjectionPermit",
        ".project_invalid_body_report_candidate(",
        "candidate.replay_authority_is_exact(active_context)",
        "SealedInvalidBodyReportProjection::from_registry",
    ] {
        assert!(
            token.contains(required),
            "invalid-body replay token omitted {required}"
        );
    }
    for forbidden in [
        "derive(Clone",
        "Decode",
        "Option<InvalidBodyReport",
        "fn into_parts(",
        "fn effect(",
        "fn pending(",
        "fn receipt(",
        "fn certificate(",
        "fn source(",
        "fn install(",
        "fn commit(",
        "fn candidate(",
        "fn report_effect(",
        "projection::admission_request",
        "!= [0; 32]",
        "== [0; 32]",
        "is_zero()",
    ] {
        assert!(
            !token.contains(forbidden),
            "invalid-body replay token exposed forbidden surface {forbidden}"
        );
    }
    assert_eq!(
        production
            .matches("adapter.seal_invalid_body_report_replay(")
            .count(),
        1,
        "only the fixed Ready registry join may invoke the adapter seal"
    );
    for outside in [
        reviewed_lifecycle_ledger_source_for_test(),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        let outside = outside
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("outside production prefix is bounded");
        assert!(!outside.contains("PreparedInvalidBodyReportReplayPreAdmission"));
        assert!(!outside.contains("InvalidBodyReportReplayEvidenceV1"));
    }
}

#[test]
fn live_validate_sign_join_is_linear_opaque_and_unwired_from_runner() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("work registry has one production prefix");
    let authority = production
        .split("pub(in crate::sumeragi) struct ReadyValidateSignPredecessorAuthority")
        .nth(1)
        .expect("Validate Sign predecessor authority is declared")
        .split("impl<'a> ReadyValidatedAdapterAuthority<'a>")
        .next()
        .expect("validated preview authority follows the Sign authority");
    for required in [
        "_linearity: ReadyValidateSignPredecessorLinearity",
        "impl Drop for ReadyValidateSignPredecessorLinearity",
        "pub(in crate::sumeragi) fn project_successor(\n        self,",
        "project_validate_sign_prepare_successor",
        "project_validate_sign_commit_successor_with_registered_prepare",
    ] {
        assert!(
            authority.contains(required),
            "Validate Sign predecessor authority omitted {required}"
        );
    }
    for forbidden in [
        "derive(Clone",
        "derive(Copy",
        "fn into_parts(",
        "fn effect(",
        "fn pending(",
        "fn certificate(",
    ] {
        assert!(
            !authority.contains(forbidden),
            "Validate Sign predecessor authority exposed {forbidden}"
        );
    }

    let join = production
        .split("pub(super) fn seal_live_wal_validate_sign(")
        .nth(1)
        .expect("fixed Validate Sign join is declared")
        .split("/// Consume only the exact Ready/rejected report preview")
        .next()
        .expect("invalid-body join follows the live Sign join");
    for required in [
        "registry.validate_sign_predecessor_authority()",
        "adapter.bind_validate_sign_predecessor(predecessor)",
        "adapter.append_live_wal()",
        "ReadyDurableValidateSignPreAdmissionFailure::PreWal",
        "ReadyDurableValidateSignPreAdmissionFailure::Wal",
        "PreparedReadyDurableValidatePersistedSignPreAdmission",
    ] {
        assert!(
            join.contains(required),
            "fixed live Sign join omitted {required}"
        );
    }
    for forbidden in [
        "&AdapterEffect",
        "&PendingRuntimeEffectBinding",
        "&DurableBodyReceipt",
        "QuorumCertificate",
        "LiveWalFrameIdentity",
        "fn into_parts(",
        "fn commit(",
        "fn install(",
    ] {
        assert!(
            !join.contains(forbidden),
            "fixed live Sign join exposed {forbidden}"
        );
    }
    assert_eq!(
        production
            .matches("adapter.bind_validate_sign_predecessor(predecessor)")
            .count(),
        1,
        "only the fixed Ready registry join binds the adapter Sign predecessor"
    );
    for caller in [
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        let caller = caller
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("caller production prefix is bounded");
        assert!(!caller.contains("seal_live_wal_validate_sign"));
        assert!(!caller.contains("PreparedReadyDurableValidatePersistedSignPreAdmission"));
    }
}

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

#[test]
fn registry_remains_inert_and_scheduler_free() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
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
    let coordinator = include_str!("../v2_lifecycle_coordinator.rs");
    assert_eq!(
        coordinator
            .matches(&["work_registry", "::"].concat())
            .count(),
        1,
        "only the narrow opaque registry authority types may cross the module boundary"
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
    assert!(export.contains("RecoveredWalValidateRegistryCut"));
    assert!(export.contains("RecoveredWalValidateRegistryJoinError"));
    assert!(export.contains("AuthenticatedRecoveredWalValidateLifecycleRepair"));
    assert!(export.contains("DurableAuthenticatedRecoveredWalValidateLifecycleRepair"));
    assert!(export.contains("RecoveredWalValidateLedgerPersistError"));
    assert!(export.contains("InstalledRecoveredWalSignRegistryCut"));
    assert!(export.contains("RecoveredWalSignInstallError"));
    assert!(!export.contains("ConcreteLifecycleWorkRegistry"));
    assert!(!export.contains("ReadyDurableValidateExecutionError"));
    assert!(!coordinator.contains("pub(crate) use wal_recovery"));
}

#[test]
fn installed_body_projection_and_recovered_prepare_fixture_keep_authority_closed() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
    let permit = source
        .split("pub(in crate::sumeragi) struct InstalledBodyCandidateProjectionPermit")
        .nth(1)
        .expect("installed-body projection permit has one declaration")
        .split("impl ConcreteWorkAddress")
        .next()
        .expect("concrete address follows the projection permit");
    for required in [
        "_linearity: InstalledBodyCandidateProjectionLinearity",
        "impl Drop for InstalledBodyCandidateProjectionLinearity",
        "struct SealedBodySuccessorProjectionPermit",
        "_linearity: SealedBodySuccessorProjectionLinearity",
        "impl Drop for SealedBodySuccessorProjectionLinearity",
        "fn new() -> Self",
    ] {
        assert!(
            permit.contains(required),
            "projection permit omitted {required}"
        );
    }
    assert!(!permit.contains("derive(Clone"));
    assert!(!permit.contains("derive(Copy"));

    let fixture = source
        .split("fn install_remote_proposal_validate_completion_for_test")
        .nth(1)
        .expect("remote-Proposal fixture has one registry entrypoint")
        .split("pub(crate) fn recovered_wal_validate_registry_cut_for_test")
        .next()
        .expect("recovered WAL wrapper follows its shared fixture");
    for required in [
        "bind_authenticated_remote_proposal_replay_for_test",
        "exact_remote_proposal_fetch_replay",
        "project_proposal_fetch_store_successor",
        "project_exact_store",
        "bind_durable_body",
        "project_store_validate_successor",
        "project_exact_validate",
        "DurableValidateReplayEvidenceV1::remote_proposal",
        "project_installed_validate_candidate",
    ] {
        assert!(
            fixture.contains(required),
            "recovered Prepare fixture omitted exact origin step {required}"
        );
    }
    for forbidden in [
        "certified_pipeline_replay_evidence_for_test",
        "projection::admission_request(",
        "DurableValidateReplayEvidenceV1::certified",
    ] {
        assert!(
            !fixture.contains(forbidden),
            "recovered Prepare fixture fabricated authority through {forbidden}"
        );
    }

    let replay = include_str!("../v2_lifecycle_replay_authority.rs").replacen(
        "include!(\"v2_lifecycle_replay_authority_certified_body.rs\");\n",
        include_str!("../v2_lifecycle_replay_authority_certified_body.rs"),
        1,
    );
    for required in [
        "fn project_installed_store_candidate(",
        "fn project_installed_validate_candidate(",
        "fn project_sealed_store_successor_candidate(",
        "fn project_sealed_validate_successor_candidate(",
        "_permit: InstalledBodyCandidateProjectionPermit",
        "_permit: SealedBodySuccessorProjectionPermit",
        "authority_free_admission_projection(",
    ] {
        assert!(
            replay.contains(required),
            "installed-body replay projection omitted {required}"
        );
    }

    let production = source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("registry has one production prefix");
    let sealed_projection = production
        .split("fn sealed_successor_parent")
        .nth(1)
        .expect("sealed successor parent join has one implementation")
        .split("// READY_DURABLE_VALIDATE_ADAPTER_JOIN_BEGIN")
        .next()
        .expect("Ready Validate join follows sealed body projection");
    for required in [
        "ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)",
        "work.validates_at(address)",
        "work.digest != digest",
        "completion.address != self._completion_address",
        "store.address != self._store_address",
        "self._durable_body.manifest_hash() != self._expected_manifest_hash",
        "self._store_digest",
        "self._validate_digest",
        "project_sealed_store_successor_candidate",
        "project_sealed_validate_successor_candidate",
        "SealedBodySuccessorProjectionPermit::new()",
        "sealed_successor_candidate_has_exact_geometry",
    ] {
        assert!(
            sealed_projection.contains(required),
            "sealed body successor projection omitted {required}"
        );
    }
    assert_eq!(
        sealed_projection
            .matches("pub(super) fn project_for_body_transition(")
            .count(),
        2
    );
    for forbidden in [
        "projection::admission_request(",
        "candidate.payload =",
        "fn commit(",
        ".insert(",
        ".remove(",
        "for_test",
    ] {
        assert!(
            !sealed_projection.contains(forbidden),
            "sealed body successor projection acquired {forbidden}"
        );
    }
}

#[test]
fn certified_fetch_execution_surface_is_borrow_bound_and_commit_free() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
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
    let source = reviewed_lifecycle_work_registry_source_for_test();
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
        "store\n            .project_candidate(verified)",
        "durable_validate_body_payload(&store.durable_receipt)",
        "candidate.key != lease.key()",
        "candidate.causal_root != lease.owner().causal_root()",
        "candidate.payload != expected_payload",
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
    assert!(!preparation.contains("projection::admission_request("));
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
        "pub(super) fn matches_durable_payload",
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
        include_str!("../v2_lifecycle_selector.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
    ] {
        assert!(!caller_source.contains("prepare_durable_store_execution"));
    }
}

#[test]
fn durable_validate_execution_surface_is_closed_borrow_bound_and_inert() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
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
    assert_eq!(
        common_work
            .matches("ConcreteLifecycleWorkKind::DurableRecoveredWalSign")
            .count(),
        5,
        "recovered Sign must remain exhaustive in validation, address, causal-root, effect-borrow, and generic-adapter rejection paths"
    );
    assert_eq!(
        common_work
            .matches("ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch")
            .count(),
        5,
        "recovered Decision Fetch must remain exhaustive in validation, address, causal-root, effect-borrow, and generic-adapter rejection paths"
    );

    assert!(production.contains("pub(super) struct ConcreteLifecycleWork {"));
    assert!(!production.contains("pub(in crate::sumeragi) struct ConcreteLifecycleWork {"));
    let live_sign_work = production
        .split("pub(in crate::sumeragi) struct PreparedLiveValidateSignRegistryWork {")
        .nth(1)
        .and_then(|suffix| {
            suffix
                .split("pub(super) enum CertifiedFetchCompletionError")
                .next()
        })
        .expect("opaque live Sign work has one bounded implementation");
    for required in [
        "work: ConcreteLifecycleWork",
        "_permit: LiveValidateSignWorkProjectionPermit",
        "ConcreteLifecycleWork::from_exact(effect, pending)",
        "self.work.validate_exact()",
        "self.work.digest() == digest",
        "self.work.causal_root() == owner.causal_root()",
        "ConcreteLifecycleWorkKind::PendingAdapter",
        "SignRequest::Vote(vote)",
        "wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit",
        "ConcreteWorkAddress::new(owner, ordinal, slot)",
        "PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)",
        "reservation.install_live_sign(self.work)",
    ] {
        assert!(
            live_sign_work.contains(required),
            "opaque live Sign work omitted {required}"
        );
    }
    for forbidden in [
        "pub work:",
        "derive(Clone",
        "into_parts",
        "fn effect(",
        "fn pending(",
        "fn receipt(",
        "fn candidate(",
    ] {
        assert!(
            !live_sign_work.contains(forbidden),
            "opaque live Sign work exposes forbidden surface {forbidden}"
        );
    }

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
        "validate\n            .project_candidate(verified)",
        "durable_validate_body_payload(&validate.durable_receipt)",
        "candidate.key != lease.key()",
        "candidate.causal_root != lease.owner().causal_root()",
        "candidate.initial_state != InitialLifecycleState::Ready",
        "candidate.reconstruction_source != lease.owner().causal_root().digest()",
        "candidate.payload != expected_payload",
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
    assert!(!preparation.contains("projection::admission_request("));
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
        8,
        "Validate token may expose only preview coordinates, the fixed durable-payload equality oracle, durable authorities, sealed wait dispatch, owned detach, and success binding"
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
        include_str!("../v2_lifecycle_selector.rs"),
        include_str!("../v2_lifecycle_coordinator.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        assert!(!caller_source.contains("prepare_durable_validate_execution"));
    }
}

#[test]
fn ready_validate_execution_surface_is_closed_borrow_bound_and_unwired() {
    let source = reviewed_lifecycle_work_registry_source_for_test();
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
    assert!(declaration.contains("lease: TurnLease"));
    assert!(
        declaration.contains("_adapter: PreparedReadyDurableValidateAdapterPublication<'adapter>")
    );
    assert!(!declaration.contains("derive(Clone"));

    let preview_oracles = production
        .split("impl PreparedReadyDurableValidateAdapterPreview<'_, '_>")
        .nth(1)
        .expect("Ready Validate preview has one sealed oracle surface")
        .split("/// Ownership-preserving failure")
        .next()
        .expect("preview failure follows its oracle surface");
    for required in [
        "project_no_successor_for_body_transition",
        "self._registry.matches_exact_lease(lease)",
        "self._adapter.kind()",
        "SealedValidateNoSuccessorProjectionPermit",
        "sealed_validate_no_successor_reservation(",
        "durable_validate_body_payload(&completion.incumbent.durable_receipt)",
        "SealedValidateNoSuccessorProjection::from_registry",
    ] {
        assert!(
            preview_oracles.contains(required),
            "Ready Validate preview omitted sealed oracle {required}"
        );
    }
    for forbidden in [
        "into_parts",
        "-> &DurableBodyReceipt",
        "-> Option<&DurableBodyReceipt>",
        "fn durable_receipt(",
        "fn receipt(",
        "matches_exact_successor_effect",
        "publication_kind",
        "projection::admission_request",
        "CandidateAdmission",
    ] {
        assert!(
            !preview_oracles.contains(forbidden),
            "Ready Validate preview exposed body authority {forbidden}"
        );
    }

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
        "output_reservation()",
        "CapacityClass::Consensus",
        ".incumbent\n            .project_candidate(verified)",
        "durable_validate_body_payload(&completion.incumbent.durable_receipt)",
        "candidate.key != lease.key()",
        "candidate.payload != expected_payload",
        "projected_slots != incumbent_slots",
        "projected_universe != lease_slots",
        "projected_consumed != lease_slots",
    ] {
        assert!(
            preparation.contains(required),
            "Ready Validate preflight omitted {required}"
        );
    }
    assert!(!preparation.contains("projection::admission_request("));
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
        .split_once("// READY_DURABLE_VALIDATE_ADAPTER_JOIN_BEGIN")
        .expect("Ready Validate fixed join begins")
        .1
        .split_once("// READY_DURABLE_VALIDATE_ADAPTER_JOIN_END")
        .expect("Ready Validate fixed join ends")
        .0;
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

    let recovered_detach = production
        .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_DETACH_BEGIN")
        .expect("recovered WAL Validate detach begins")
        .1
        .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_DETACH_END")
        .expect("recovered WAL Validate detach ends")
        .0;
    for required in [
        "into_recovered_wal_validate_registry_cut",
        "ReadyDurableValidateOutcomeKind::Validated",
        "self.completion().is_none()",
        "self.registry.entries.remove(&address)",
        "work: Some(work)",
    ] {
        assert!(
            recovered_detach.contains(required),
            "recovered WAL Validate detach omitted {required}"
        );
    }
    for forbidden in ["into_parts", "Clone", "pub(super) fn new(", "for_test"] {
        assert!(
            !recovered_detach.contains(forbidden),
            "recovered WAL Validate detach exposed forbidden authority {forbidden}"
        );
    }

    let live_publication = production
        .split("pub(super) fn prepare_registry_publication(")
        .nth(1)
        .and_then(|suffix| suffix.split("/// Ownership-preserving failure").next())
        .expect("live Validate-to-Sign registry publication has one bounded surface");
    for required in [
        "prepare_registry_work(LiveValidateSignWorkProjectionPermit::new())",
        "ConcreteWorkAddress::new(lease.owner(), child_ordinal, child_slot)",
        "adapter.registry_work_matches(",
        "registry.into_recovered_wal_validate_registry_cut()",
        ".into_live_validate_sign_reservation()",
        "reservation.bind_exact_child(child_address, child_digest)",
        "PreparedLiveValidateSignRegistryPublication",
        "publish_after_ledger_fsync",
        ".install_registry_and_commit_adapter(self.reservation)",
    ] {
        assert!(
            live_publication.contains(required),
            "live Validate-to-Sign registry publication omitted {required}"
        );
    }
    for forbidden in [
        "into_parts",
        "fn effect(",
        "fn pending(",
        "fn receipt(",
        "fn candidate(",
        "persist_durable_projection",
        "publish_status(",
    ] {
        assert!(
            !live_publication.contains(forbidden),
            "live Validate-to-Sign registry publication exposes {forbidden}"
        );
    }

    let recovered_join = production
        .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_BEGIN")
        .expect("recovered WAL Validate join begins")
        .1
        .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_END")
        .expect("recovered WAL Validate join ends")
        .0;
    for required in [
        "pub(crate) fn join_recovered_vote",
        "completion.outcome.validated_receipt()",
        "receipt.execution_commitment() == recovered_commitment",
        "pending.project_recovered_wal_vote_successor(&effect, recovered)",
        "DetachedValidateReplayEvidenceV1::Retained(replay_evidence)",
        "authenticate_recovered_wal_vote_lifecycle_from_durable_body(",
        "completion.restore(effect, pending)",
        "self.registry.take()",
        "RecoveredWalValidateRegistryReservation",
    ] {
        assert!(
            recovered_join.contains(required),
            "recovered WAL Validate join omitted {required}"
        );
    }
    for forbidden in [
        "into_parts",
        "pub(crate) fn effect(",
        "pub(crate) fn pending(",
        "fresh_for_test",
        "RuntimeEffectOwnership",
    ] {
        assert!(
            !recovered_join.contains(forbidden),
            "recovered WAL Validate join exposed forbidden authority {forbidden}"
        );
    }

    let recovered_fsync = production
        .split_once("// RECOVERED_WAL_VALIDATE_LEDGER_FSYNC_BEGIN")
        .expect("recovered WAL Validate ledger fsync begins")
        .1
        .split_once("// RECOVERED_WAL_VALIDATE_LEDGER_FSYNC_END")
        .expect("recovered WAL Validate ledger fsync ends")
        .0;
    for required in [
        "pub(crate) struct DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>",
        "pub(crate) struct RecoveredWalValidateLedgerPersistError<'registry>",
        "AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>",
        "RecoveredWalValidateRegistryReservation<'registry>",
        "fn ledger_parent_core_identity_is_exact(",
        "parent.owner() == self.validation.address.owner",
        "parent.ordinal() == self.validation.address.ordinal",
        "fn projected_child_address(",
        "bind_child_if_vacant(child_address, child_digest)",
        "pub(super) fn persist_in_opened_ledger(",
        "opened.stage_authenticated_wal_vote_repair(&self.repair)",
        "store.persist_authenticated_wal_vote_repair(opened, repair)",
        "DurableAuthenticatedWalVoteLifecycleRepair",
        "PostFsync",
    ] {
        assert!(
            recovered_fsync.contains(required),
            "recovered WAL Validate fsync splice omitted {required}"
        );
    }
    for forbidden in [
        "into_parts",
        "pub(crate) fn effect(",
        "pub(crate) fn pending(",
        "pub(crate) fn receipt(",
        "FnOnce",
        "RuntimeEffectOwnership",
        "PendingRuntimeEffectBinding",
    ] {
        assert!(
            !recovered_fsync.contains(forbidden),
            "recovered WAL Validate fsync splice exposed forbidden authority {forbidden}"
        );
    }

    let recovered_install = production
        .split_once("// RECOVERED_WAL_SIGN_REGISTRY_INSTALL_BEGIN")
        .expect("recovered WAL Sign registry install begins")
        .1
        .split_once("// RECOVERED_WAL_SIGN_REGISTRY_INSTALL_END")
        .expect("recovered WAL Sign registry install ends")
        .0;
    for required in [
        "pub(super) fn install_recovered_sign(",
        "self.post_fsync_authority_is_exact(store)",
        "PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)",
        ".all(|address| address.owner != child.owner)",
        "store.revalidates_durable_authenticated_wal_vote_repair(",
        "ConcreteLifecycleWorkKind::DurableRecoveredWalSign(",
        "std::collections::btree_map::Entry::Vacant(entry)",
        "entry.insert(work);",
        "pub(crate) struct InstalledRecoveredWalSignRegistryCut<'registry>",
        "pub(crate) struct RecoveredWalSignInstallError<'registry>",
        "fn installed_entry_is_exact(",
        "self.registry.entries.contains_key(&self.parent_address)",
        ".filter(|address| address.owner == self.child_address.owner)",
        "sign.validates_in_store(",
    ] {
        assert!(
            recovered_install.contains(required),
            "recovered WAL Sign install omitted {required}"
        );
    }
    for forbidden in [
        "into_parts",
        "into_pair",
        "pub(crate) fn effect(",
        "pub(crate) fn pending(",
        "pub(crate) fn receipt(",
        "PendingRuntimeEffectBinding",
        "RuntimeEffectOwnership",
        "DurableWalVoteLedgerRepairReceipt {",
        "DetachedRecoveredValidateCompletion {",
        "FnOnce",
        "LifecycleCoordinator",
        "publish_status(",
        ".remove(",
    ] {
        assert!(
            !recovered_install.contains(forbidden),
            "recovered WAL Sign install exposed forbidden authority {forbidden}"
        );
    }
    let after_insert = recovered_install
        .split_once("entry.insert(work);")
        .expect("recovered Sign has one insertion")
        .1
        .split_once("    }")
        .expect("install method ends after insertion")
        .0;
    for forbidden in ["return Err", "?", "if ", "match ", "debug_assert"] {
        assert!(
            !after_insert.contains(forbidden),
            "post-insert recovered Sign path acquired fallible check {forbidden}"
        );
    }

    let carrier_inventory = production
        .split("struct DurableRecoveredWalSignWork")
        .nth(1)
        .expect("closed recovered Sign carrier exists")
        .split("enum ConcreteLifecycleWorkKind")
        .next()
        .expect("work-kind inventory follows recovered Sign carrier");
    for required in [
        "repair: DurableAuthenticatedWalVoteLifecycleRepair",
        "validation: DetachedRecoveredValidateCompletion",
        "fn validates_digest(",
        "fn validates_in_store(",
    ] {
        assert!(
            carrier_inventory.contains(required),
            "closed recovered Sign carrier omitted {required}"
        );
    }
    for forbidden in [
        "derive(Clone",
        "into_parts",
        "into_pair",
        "PendingRuntimeEffectBinding",
    ] {
        assert!(
            !carrier_inventory.contains(forbidden),
            "closed recovered Sign carrier exposes {forbidden}"
        );
    }
    let work_kind_inventory = production
        .split("enum ConcreteLifecycleWorkKind")
        .nth(1)
        .expect("concrete work kind has one inventory")
        .split("/// One move-only concrete effect")
        .next()
        .expect("concrete work follows its kind inventory");
    assert_eq!(
        work_kind_inventory
            .matches("DurableRecoveredWalSign(DurableRecoveredWalSignWork)")
            .count(),
        1,
        "the durable recovered phase-vote handoff owns exactly one closed work variant"
    );

    let wal_recovery = include_str!("../v2_lifecycle_wal_recovery.rs");
    let child_effect_borrow = wal_recovery
        .split("pub(super) const fn installed_child_effect(")
        .nth(1)
        .expect("durable WAL repair exposes one narrow child-effect borrow")
        .split("    }")
        .next()
        .expect("child-effect borrow is bounded");
    assert!(child_effect_borrow.contains("self.repair.projection.installed_child_effect()"));
    for forbidden in ["pending", "into_", "clone", "receipt"] {
        assert!(
            !child_effect_borrow.contains(forbidden),
            "child-effect borrow exposed forbidden {forbidden}"
        );
    }
    assert_eq!(
        production.matches(".installed_child_effect()").count(),
        1,
        "only the closed concrete carrier may borrow the durable child effect"
    );

    let ledger_source = reviewed_lifecycle_ledger_source_for_test();
    let frame_revalidation = ledger_source
        .split("pub(super) fn revalidates_durable_authenticated_wal_vote_repair(")
        .nth(1)
        .expect("ledger exposes one narrow durable repair revalidation")
        .split("    /// Atomically replace the ledger")
        .next()
        .expect("ledger revalidation ends before persistence");
    for required in [
        "let Ok(loaded) = self.load()",
        "durable.belongs_to_loaded(self, &loaded)",
        "loaded.stage_authenticated_wal_vote_repair(durable.repair())",
        "!changed",
        "observed_child_ordinal == durable.child_ordinal()",
        "staged == loaded",
    ] {
        assert!(
            frame_revalidation.contains(required),
            "same-frame recovered Sign preflight omitted {required}"
        );
    }
    assert_eq!(
        frame_revalidation.matches("self.load()").count(),
        1,
        "receipt hash and repaired-pair shape must share one loaded frame"
    );

    for caller_source in [
        include_str!("../v2.rs"),
        include_str!("../v2_lifecycle_selector.rs"),
        include_str!("../v2_lifecycle_coordinator.rs"),
        include_str!("../v2_effects.rs"),
        include_str!("../v2_worker.rs"),
        include_str!("../v2_runner.rs"),
    ] {
        assert!(!caller_source.contains("prepare_ready_durable_validate_execution"));
        assert!(!caller_source.contains("installed_child_effect"));
    }
}
