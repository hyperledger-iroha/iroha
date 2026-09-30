//! Original-pool NFC lease, semantic order and concurrent accumulator controls.

use super::super::decode_tests::seeded_musubi_publication_snapshot;
use super::*;
use iroha_allocation::AllocationRefusal;
use iroha_data_model::musubi::MusubiNamespaceV1;
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

fn limits() -> SourceWorkLimits {
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

fn unicode_world(raw: &str) -> World {
    let (mut world, release, _, selector) = seeded_musubi_publication_snapshot();
    let namespace: MusubiNamespaceV1 =
        json::from_value(json::Value::Array(vec![json::Value::String(raw.into())])).unwrap();
    let mut package = world
        .musubi_packages
        .view()
        .get(&release.package)
        .unwrap()
        .clone();
    let mut binding = world
        .musubi_namespace_bindings
        .view()
        .get(&package.claimed_namespace)
        .unwrap()
        .clone();
    let old_namespace = binding.namespace.clone();
    binding.namespace = namespace.clone();
    package.claimed_namespace_binding = binding.digest();
    let mut bindings = world.musubi_namespace_bindings.block();
    bindings.remove(old_namespace);
    bindings.insert(namespace.clone(), binding);
    bindings.commit();
    package.claimed_namespace = namespace.clone();
    world.musubi_packages.insert(release.package, package);
    let mut entry = world
        .musubi_public_directory
        .view()
        .get(&selector)
        .unwrap()
        .clone();
    entry.selector.namespace = namespace;
    let mut change = world.musubi_public_directory.block();
    change.remove(selector);
    change.insert(entry.selector.clone(), entry);
    change.commit();
    world
}

fn complete_bytes(world: &World, plan: &Plan) -> usize {
    // Measure the existing original-pool scratch owners independently; no
    // duplicate PackageRevision layout or invented fixed budget is used.
    let live = AllocationBudget::new(1_000_000);
    validate_musubi_live_projection_cut(&world.view(), &live).unwrap();
    assert_eq!(live.reserved_bytes(), 0);
    let universal = AllocationBudget::new(1_000_000);
    musubi_universal::validate_musubi_universal_projection_cut(
        &world.view(),
        ProjectionCut::Capture,
        &universal,
    )
    .unwrap();
    assert_eq!(universal.reserved_bytes(), 0);
    assert!(universal.peak_reserved_bytes() > live.peak_reserved_bytes());
    plan.nfc_scratch_bytes + universal.peak_reserved_bytes()
}

#[test]
fn one_maximum_lease_overlaps_real_universal_scratch_and_is_not_retained_by_token() {
    let raw = format!("q{}", "\u{301}".repeat(100));
    let world = unicode_world(&raw);
    let before = json::to_json(&world).unwrap();
    let plan = admit(&world.view(), limits()).unwrap();
    assert_eq!(
        plan.nfc_scratch_bytes,
        iroha_model_base::name::Name::canonical_validation_scratch_bytes(&raw)
    );
    assert!(plan.nfc_scratch_bytes > 0);
    assert_eq!(
        plan.occurrences[Operation::NamespaceScratchPlan as usize],
        2
    );
    let complete = complete_bytes(&world, &plan);
    let short = AllocationBudget::new(complete - 1);
    let Err(SourceValidationError::Attempt(ExecutionAttemptError::Deferred(reason))) =
        validate(&world.view(), &short, limits())
    else {
        panic!("one byte short must refuse actual concurrent accumulator funding")
    };
    assert!(
        matches!(reason.allocation_refusal(), Some(AllocationRefusal::Capacity { reserved_bytes, requested_bytes, .. }) if *reserved_bytes == plan.nfc_scratch_bytes && *requested_bytes == complete - plan.nfc_scratch_bytes)
    );
    assert_eq!(short.reserved_bytes(), 0);
    let budget = AllocationBudget::new(complete);
    let view = world.view();
    let token = validate(&view, &budget, limits()).unwrap();
    assert_eq!(budget.peak_reserved_bytes(), complete);
    assert_eq!(
        budget.reserved_bytes(),
        0,
        "the borrowed result retains no ICU allocation"
    );
    assert_eq!(token.execution_budget().reserved_bytes(), 0);
    assert_eq!(token.directory_rows().count(), 1);
    assert_eq!(json::to_json(&world).unwrap(), before);
}

#[test]
fn nfc_admission_keeps_the_original_capacity_release_and_retries_after_shrink() {
    let world = unicode_world(&format!("q{}", "\u{301}".repeat(100)));
    let plan = admit(&world.view(), limits()).unwrap();
    let complete = complete_bytes(&world, &plan);
    let budget = AllocationBudget::new(complete);
    let occupied = budget.try_reserve_bytes(complete).unwrap();
    let Err(SourceValidationError::Attempt(ExecutionAttemptError::Deferred(reason))) =
        validate(&world.view(), &budget, limits())
    else {
        panic!("NFC preflight must retain the actual pool refusal")
    };
    let Some(AllocationRefusal::Capacity {
        requested_bytes,
        release,
        ..
    }) = reason.allocation_refusal()
    else {
        panic!("original capacity owner required")
    };
    assert_eq!(*requested_bytes, plan.nfc_scratch_bytes);
    let mut wait = pin!(release.clone().wait_for_release());
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
    let unrelated = AllocationBudget::new(complete);
    drop(unrelated.try_reserve_bytes(complete).unwrap());
    assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
    budget.set_limit_bytes(plan.nfc_scratch_bytes - 1);
    drop(occupied);
    assert_eq!(wait.as_mut().poll(&mut context), Poll::Ready(()));
    assert!(matches!(
        validate(&world.view(), &budget, limits()),
        Err(SourceValidationError::Attempt(
            ExecutionAttemptError::Deferred(_)
        ))
    ));
    budget.set_limit_bytes(complete);
    validate(&world.view(), &budget, limits()).unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn scratch_planning_does_not_report_a_later_semantic_failure_before_live_evidence() {
    let raw = format!("q{}\u{300}", "\u{315}".repeat(100));
    let mut world = unicode_world(&raw);
    let plan = admit(&world.view(), limits()).unwrap();
    assert!(plan.nfc_scratch_bytes > 0);
    let (archive_id, mut archive) = world
        .musubi_archives
        .view()
        .iter()
        .next()
        .map(|(key, row)| (*key, row.clone()))
        .unwrap();
    archive.location_revision = 0;
    let expected = archive.validate().unwrap_err().reason();
    world.musubi_archives.insert(archive_id, archive);
    let budget = AllocationBudget::new(1_000_000);
    let Err(SourceValidationError::Attempt(ExecutionAttemptError::Rejected(rejection))) =
        validate(&world.view(), &budget, limits())
    else {
        panic!("later namespace failure must not hide the earlier archive failure")
    };
    assert_eq!(
        rejection,
        ProjectionRejection::new(ProjectionTable::Archives, expected)
    );
    assert_eq!(budget.peak_reserved_bytes(), plan.nfc_scratch_bytes);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn semantic_failure_and_unwind_reclaim_original_nfc_custody_after_scratch_is_gone() {
    let invalid = format!("q{}\u{300}", "\u{315}".repeat(100));
    let world = unicode_world(&invalid);
    let plan = admit(&world.view(), limits()).unwrap();
    let budget = AllocationBudget::new(1_000_000);
    let Err(SourceValidationError::Attempt(ExecutionAttemptError::Rejected(rejection))) =
        validate(&world.view(), &budget, limits())
    else {
        panic!("actual namespace semantic failure required")
    };
    assert_eq!(
        rejection,
        ProjectionRejection::new(
            ProjectionTable::PublicDirectory,
            "Musubi namespace segment is invalid"
        )
        .with_cut(ProjectionCut::Capture)
    );
    assert_eq!(budget.reserved_bytes(), 0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        plan.with_nfc_scratch(&budget, || -> Result<(), SourceValidationError> {
            assert_eq!(budget.reserved_bytes(), plan.nfc_scratch_bytes);
            assert!(iroha_model_base::name::Name::validate_canonical(&invalid).is_err());
            assert_eq!(budget.reserved_bytes(), plan.nfc_scratch_bytes);
            panic!("unwind after actual ICU has released its private scratch")
        })
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn nfc_plan_covers_package_and_directory_payloads_independently_even_when_rows_disagree() {
    let raw = format!("q{}", "\u{301}".repeat(100));
    let namespace: MusubiNamespaceV1 = raw.parse().unwrap();
    let expected = namespace.validation_scratch_bytes();
    assert!(expected > 0);
    for package_only in [true, false] {
        let (mut world, release, _, selector) = seeded_musubi_publication_snapshot();
        if package_only {
            let mut package = world
                .musubi_packages
                .view()
                .get(&release.package)
                .unwrap()
                .clone();
            package.claimed_namespace = namespace.clone();
            world.musubi_packages.insert(release.package, package);
        } else {
            let mut entry = world
                .musubi_public_directory
                .view()
                .get(&selector)
                .unwrap()
                .clone();
            entry.selector.namespace = namespace.clone();
            world.musubi_public_directory.insert(selector, entry);
        }
        let plan = admit(&world.view(), limits()).unwrap();
        assert_eq!(
            plan.nfc_scratch_bytes, expected,
            "both independent payload sources must be included"
        );
        let budget = AllocationBudget::new(1_000_000);
        assert!(matches!(
            validate(&world.view(), &budget, limits()),
            Err(SourceValidationError::Attempt(
                ExecutionAttemptError::Rejected(_)
            ))
        ));
        assert_eq!(budget.reserved_bytes(), 0);
        assert!(budget.peak_reserved_bytes() >= expected);
    }
}
