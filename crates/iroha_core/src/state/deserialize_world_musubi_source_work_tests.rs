//! Whole-cut ceilings, repeated evidence and original-pool refusal controls.

use super::super::decode_tests::{
    seed_provider_attested_location, seeded_musubi_publication_snapshot,
};
use super::*;
use iroha_data_model::musubi::{MusubiArchiveLocationIdV1, MusubiContentDigestV1};
use mv::allocation::AllocationRefusal;
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

fn allowance() -> SourceWorkLimits {
    SourceWorkLimits {
        geometry: SourceGeometryLimits {
            elements: 1_000_000,
            variable_bytes: 1_000_000,
        },
        table_pass_rows: 1_000_000,
        lookup_index_entries: 1_000_000,
        model_operations: 1_000_000,
        signature_checks: 1_000_000,
    }
}

fn measured(plan: &Plan) -> SourceWorkLimits {
    SourceWorkLimits {
        geometry: plan.geometry.used(),
        table_pass_rows: plan.table_rows,
        lookup_index_entries: plan.index_entries,
        model_operations: plan.operations,
        signature_checks: plan.signatures,
    }
}

#[test]
fn empty_cut_needs_no_work_or_original_pool_capacity() {
    let world = World::default();
    let budget = AllocationBudget::new(0);
    let zero = SourceWorkLimits {
        geometry: SourceGeometryLimits {
            elements: 0,
            variable_bytes: 0,
        },
        table_pass_rows: 0,
        lookup_index_entries: 0,
        model_operations: 0,
        signature_checks: 0,
    };
    validate(&world.view(), &budget, zero).unwrap();
    assert_eq!(budget.peak_reserved_bytes(), 0);
}

#[test]
fn complete_cut_accepts_exact_bounds_and_refuses_each_short_dimension() {
    let (world, _, _, _) = seeded_musubi_publication_snapshot();
    let before = json::to_json(&world).unwrap();
    let plan = admit(&world.view(), allowance()).unwrap();
    assert_eq!(plan.table_rows, 18);
    assert_eq!(plan.operations, 6);
    assert_eq!(plan.signatures, 0);
    let exact = measured(&plan);
    let budget = AllocationBudget::new(1_000_000);
    validate(&world.view(), &budget, exact).unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
    for dimension in 0..4 {
        let mut short = exact;
        match dimension {
            0 => short.geometry.elements -= 1,
            1 => short.geometry.variable_bytes -= 1,
            2 => short.table_pass_rows -= 1,
            3 => short.model_operations -= 1,
            _ => unreachable!(),
        }
        let empty = AllocationBudget::new(0);
        let error = validate(&world.view(), &empty, short).unwrap_err();
        assert!(matches!(error, SourceValidationError::Work(_)));
        assert_eq!(empty.peak_reserved_bytes(), 0);
    }
    assert_eq!(json::to_json(&world).unwrap(), before);
}

#[test]
fn repeated_location_edges_are_not_deduplicated_from_crypto_allowances() {
    let (mut world, release, archive, _) = seeded_musubi_publication_snapshot();
    let attestation_key = seed_provider_attested_location(&mut world, &release, archive);
    let mut location = world
        .musubi_archive_locations
        .view()
        .first_key_value()
        .unwrap()
        .1
        .clone();
    location.location_id = MusubiArchiveLocationIdV1::new([0x91; 32]);
    world
        .musubi_archive_locations
        .insert(location.key(), location);
    let plan = admit(&world.view(), allowance()).unwrap();
    assert_eq!(plan.occurrences[Operation::ArchiveValidation as usize], 7);
    assert_eq!(plan.occurrences[Operation::LocationValidation as usize], 6);
    assert_eq!(
        plan.occurrences[Operation::AttestationRecordValidation as usize],
        3
    );
    assert_eq!(
        plan.occurrences[Operation::AttestationValidation as usize],
        5
    );
    assert_eq!(plan.occurrences[Operation::AttestationDigest as usize], 5);
    assert_eq!(plan.occurrences[Operation::SigningHash as usize], 2);
    assert_eq!(plan.occurrences[Operation::ProviderSetDigest as usize], 4);
    assert_eq!(plan.signatures, 2);
    let mut exact = measured(&plan);
    exact.signature_checks -= 1;
    let budget = AllocationBudget::new(0);
    assert!(matches!(
        validate(&world.view(), &budget, exact),
        Err(SourceValidationError::Work(WorkRefusal::Limit {
            dimension: WorkDimension::SignatureChecks,
            requested: 2,
            limit: 1,
        }))
    ));
    assert_eq!(budget.peak_reserved_bytes(), 0);
    let mut record = world
        .musubi_provider_bundle_attestations
        .view()
        .get(&attestation_key)
        .unwrap()
        .clone();
    record
        .attestation
        .approvals
        .push(record.attestation.approvals[0].clone());
    world
        .musubi_provider_bundle_attestations
        .insert(attestation_key, record);
    // Shape admission never decides duplicate-approval validity; even invalid
    // complete input charges every possible repeated signature operation first.
    assert_eq!(admit(&world.view(), allowance()).unwrap().signatures, 4);
}

#[test]
fn source_work_refusal_precedes_validation_but_sufficient_work_preserves_rejection() {
    let (mut world, release, _, selector) = seeded_musubi_publication_snapshot();
    let mut row = world
        .musubi_resolver_index
        .view()
        .get(&release)
        .unwrap()
        .clone();
    row.source_digest = MusubiContentDigestV1::new([0x94; 32]);
    world.musubi_resolver_index.insert(release, row);
    let mut directory = world
        .musubi_public_directory
        .view()
        .get(&selector)
        .unwrap()
        .clone();
    directory.metadata_revision += 1;
    world.musubi_public_directory.insert(selector, directory);
    let mut short = allowance();
    short.table_pass_rows = 0;
    let budget = AllocationBudget::new(1_000_000);
    assert!(matches!(
        validate(&world.view(), &budget, short),
        Err(SourceValidationError::Work(WorkRefusal::Limit {
            dimension: WorkDimension::TablePassRows,
            ..
        }))
    ));
    assert_eq!(budget.peak_reserved_bytes(), 0);
    let direct = musubi_universal::validate_musubi_universal_projection_cut(
        &world.view(),
        "capture",
        &budget,
    )
    .unwrap_err();
    let SourceValidationError::Attempt(actual) =
        validate(&world.view(), &budget, allowance()).unwrap_err()
    else {
        panic!("complete admission preserves original semantic error")
    };
    assert_eq!(actual.to_string(), direct.to_string());
    assert!(
        matches!(&actual, ExecutionAttemptError::Rejected(json::Error::InvalidField { field, .. })
        if field == "world.musubi_resolver_index"),
        "{actual:?}"
    );
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn memory_deferral_retains_the_supplied_pool_release_not_an_unrelated_one() {
    let (world, _, _, _) = seeded_musubi_publication_snapshot();
    let before = json::to_json(&world).unwrap();
    let budget = AllocationBudget::new(1_000_000);
    let held = budget.try_reserve_bytes(1_000_000).unwrap();
    let Err(SourceValidationError::Attempt(ExecutionAttemptError::Deferred(reason))) =
        validate(&world.view(), &budget, allowance())
    else {
        panic!("actual live scratch must retain the original pool refusal")
    };
    let Some(AllocationRefusal::Capacity { release, .. }) = reason.allocation_refusal() else {
        panic!("exact original capacity error")
    };
    let mut waiter = pin!(release.clone().wait_for_release());
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(waiter.as_mut().poll(&mut context), Poll::Pending);
    let unrelated = AllocationBudget::new(1_000_000);
    drop(unrelated.try_reserve_bytes(1_000_000).unwrap());
    assert_eq!(waiter.as_mut().poll(&mut context), Poll::Pending);
    budget.set_limit_bytes(0);
    drop(held);
    assert_eq!(waiter.as_mut().poll(&mut context), Poll::Ready(()));
    assert!(matches!(
        validate(&world.view(), &budget, allowance()),
        Err(SourceValidationError::Attempt(
            ExecutionAttemptError::Deferred(_)
        ))
    ));
    budget.set_limit_bytes(1_000_000);
    validate(&world.view(), &budget, allowance()).unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(json::to_json(&world).unwrap(), before);
}

#[test]
fn every_work_counter_refuses_overflow_without_wrapping() {
    let mut limits = allowance();
    limits.table_pass_rows = u64::MAX;
    limits.lookup_index_entries = u64::MAX;
    limits.model_operations = u64::MAX;
    limits.signature_checks = u64::MAX;
    let mut plan = Plan::new(limits);
    plan.table_rows = u64::MAX;
    plan.index_entries = u64::MAX;
    plan.operations = u64::MAX;
    plan.signatures = u64::MAX;
    for dimension in [
        WorkDimension::TablePassRows,
        WorkDimension::LookupIndexEntries,
        WorkDimension::ModelOperations,
        WorkDimension::SignatureChecks,
    ] {
        assert_eq!(
            plan.add(dimension, 1),
            Err(WorkRefusal::Overflow(dimension))
        );
    }
    assert_eq!(plan.table_rows, u64::MAX);
    assert_eq!(plan.index_entries, u64::MAX);
    assert_eq!(plan.operations, u64::MAX);
    assert_eq!(plan.signatures, u64::MAX);
}

#[test]
fn variable_storage_keys_are_measured_independently_of_unchanged_payloads() {
    let (world, release, _, _) = seeded_musubi_publication_snapshot();
    let before = admit(&world.view(), allowance()).unwrap().geometry.used();
    let row = world
        .musubi_resolver_index
        .view()
        .get(&release)
        .unwrap()
        .clone();
    let mut key = release.clone();
    key.version.prerelease = vec![
        iroha_data_model::musubi::MusubiPrereleaseIdentifierV1::AlphaNumeric("x".repeat(1024)),
    ];
    let mut block = world.musubi_resolver_index.block();
    block.remove(release);
    block.insert(key, row);
    block.commit();
    let after = admit(&world.view(), allowance()).unwrap().geometry.used();
    assert_eq!(after.variable_bytes, before.variable_bytes + 1024);
    let mut limits = allowance();
    limits.geometry.variable_bytes = before.variable_bytes;
    let budget = AllocationBudget::new(0);
    assert!(matches!(
        validate(&world.view(), &budget, limits),
        Err(SourceValidationError::Work(WorkRefusal::Geometry(
            SourceGeometryError::Limit { .. }
        )))
    ));
    assert_eq!(budget.peak_reserved_bytes(), 0);
}

#[test]
fn fixed_key_evidence_index_population_is_admitted_before_point_lookups() {
    let (mut world, release, archive, _) = seeded_musubi_publication_snapshot();
    let attestation = seed_provider_attested_location(&mut world, &release, archive);
    let owner = world
        .musubi_provider_bundle_attestations
        .view()
        .get(&attestation)
        .unwrap()
        .attestation
        .payload
        .binding
        .completed_by
        .clone();
    world.provider_owners.insert(attestation.provider_id, owner);
    let plan = admit(&world.view(), allowance()).unwrap();
    assert_eq!(plan.index_entries, 1);
    let mut limits = measured(&plan);
    limits.lookup_index_entries = 0;
    let budget = AllocationBudget::new(0);
    assert!(matches!(
        validate(&world.view(), &budget, limits),
        Err(SourceValidationError::Work(WorkRefusal::Limit {
            dimension: WorkDimension::LookupIndexEntries,
            requested: 1,
            limit: 0,
        }))
    ));
    assert_eq!(budget.peak_reserved_bytes(), 0);
}
