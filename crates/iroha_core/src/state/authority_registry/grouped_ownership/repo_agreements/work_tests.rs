//! Literal Name/controller geometry, complete original tails and refusal custody.
use super::test_support::*;
use super::*;
use iroha_data_model::account::{MultisigMember, MultisigPolicy};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::Storage;
use std::collections::BTreeMap;
fn other_id() -> RepoAgreementId {
    "untouched".parse().unwrap()
}
fn absent_id() -> RepoAgreementId {
    "absentundo".parse().unwrap()
}
fn absent_account() -> AccountId {
    AccountId::new(
        iroha_crypto::KeyPair::from_seed(
            b"repo absent account".to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        )
        .public_key()
        .clone(),
    )
}
fn multisig() -> AccountId {
    AccountId::new_multisig(
        MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(ALICE_ID.expect_single_signatory().clone(), 1).unwrap(),
                MultisigMember::new(BOB_ID.expect_single_signatory().clone(), 2).unwrap(),
            ],
        )
        .unwrap(),
    )
}
#[test]
fn independent_repo_singletons_variable_names_and_original_fixture_work_are_exact() {
    // Required group: 2*(1+69+31+1+1+1+31+68)=406; Some custodian adds four tags.
    assert_eq!(REPO_AGREEMENT_WORK_PER_ROW, 406 + 406 + 410);
    for (custodian, exact) in [(false, 816), (true, 1222)] {
        let world = fixture(custodian);
        assert_eq!(world_work(&world), exact);
        assert_eq!(
            check(&world, exact - 1),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(check(&world, exact), Ok(()));
    }
    for (label, bytes, exact) in [("é".to_owned(), 2, 910), ("x".repeat(255), 255, 6982)] {
        let mut world = fixture(true);
        let mut record = world.repo_agreements.view().get(&id()).unwrap().clone();
        let name: RepoAgreementId = label.parse().unwrap();
        assert_eq!(name.name().as_ref().len(), bytes);
        record.id = name.clone();
        world.repo_agreements = [(name, record)].into_iter().collect();
        world.rebuild_repo_agreement_indexes();
        assert_eq!(world_work(&world), exact);
        assert_eq!(
            check(&world, exact - 1),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(check(&world, exact), Ok(()));
    }
    for (custodian, exact) in [(false, 1616), (true, 2422)] {
        let mut world = fixture(custodian);
        let mut record = world.repo_agreements.view().get(&id()).unwrap().clone();
        record.initiator = multisig();
        record.counterparty = multisig();
        record.custodian = custodian.then(multisig);
        world.repo_agreements.insert(id(), record);
        world.rebuild_repo_agreement_indexes();
        assert_eq!(world_work(&world), exact);
        assert_eq!(
            check(&world, exact - 1),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(check(&world, exact), Ok(()));
    }
    for (initial_custodian, exact) in [(false, 1824), (true, 1787)] {
        let mut world = fixture(initial_custodian);
        let mut record = world.repo_agreements.view().get(&id()).unwrap().clone();
        record.initiator = BOB_ID.clone();
        record.counterparty = ALICE_ID.clone();
        record.custodian = (!initial_custodian).then(|| ALICE_ID.clone());
        {
            let mut block = world.block();
            let mut tx =
                block.transaction_without_telemetry(crate::state::LaneConfig::default(), 0);
            tx.insert_repo_agreement_entry(record);
            tx.apply();
            block.commit();
        }
        // Mandatory groups 756 each; optional appearing/disappearing group 312/275.
        assert_eq!(world_work(&world), exact);
        assert_eq!(
            check(&world, exact - 1),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(check(&world, exact), Ok(()));
        world.rebuild_repo_agreement_indexes();
        assert_eq!(world_work(&world), exact);
        world.block_and_revert().commit();
        assert!(world_work(&world) < TEST_WORK_ALLOWANCE);
    }
    // Genuine original untouched/insert/remove fixture, with raw geometry asserted first.
    let mut world = fixture(true);
    let original = world.repo_agreements.view().get(&id()).unwrap().clone();
    let mut untouched = original.clone();
    untouched.id = other_id();
    world
        .repo_agreements
        .insert(untouched.id.clone(), untouched);
    world.rebuild_repo_agreement_indexes();
    let mut inserted = original;
    inserted.id = "inserted".parse().unwrap();
    inserted.custodian = None;
    {
        let mut block = world.block();
        let mut tx = block.transaction_without_telemetry(crate::state::LaneConfig::default(), 0);
        tx.remove_repo_agreement_entry(&id());
        tx.insert_repo_agreement_entry(inserted);
        tx.apply();
        block.commit();
    }
    world.rebuild_repo_agreement_indexes();
    let source = world
        .repo_agreements
        .try_committed_view_nonblocking()
        .unwrap();
    let current: Vec<_> = source
        .current_entries()
        .map(|(k, _)| k.name().as_ref().len())
        .collect();
    let undo: Vec<_> = source
        .undo_entries()
        .map(|(k, v)| (k.name().as_ref().len(), v.is_some()))
        .collect();
    assert_eq!(current, vec![8, 9]);
    assert_eq!(undo, vec![(15, true), (8, false)]);
    for rows in [
        &world.repo_agreements_by_initiator,
        &world.repo_agreements_by_counterparty,
        &world.repo_agreements_by_custodian,
    ] {
        let view = rows.try_committed_view_nonblocking().unwrap();
        assert_eq!(
            (view.current_entries().len(), view.undo_entries().len()),
            (1, 1)
        );
        assert!(view.undo_entries().all(|(_, v)| v.is_some()));
    }
    // Required groups 424+957 each; optional group 201+961.
    assert_eq!(world_work(&world), 3924);
    assert_eq!(check(&world, 3923), Err(GroupedOwnershipError::WorkLimit));
    assert_eq!(check(&world, 3924), Ok(()));
}
#[test]
fn repo_physical_mask_lookup_and_member_tails_are_fully_prepaid() {
    let first = id();
    let second = other_id();
    let rows: Storage<RepoAgreementId, u32> = [(first.clone(), 1), (second.clone(), 2)]
        .into_iter()
        .collect();
    {
        let mut block = rows.block();
        block.insert(first.clone(), 1);
        block.remove(absent_id());
        block.commit();
    }
    let view = rows.try_committed_view_nonblocking().unwrap();
    assert_eq!(
        (view.current_entries().len(), view.undo_entries().len()),
        (2, 2)
    );
    // 2 physical current + (31+26+25+20) masks + 4 final undo/tag admissions.
    let mut seen = 0;
    assert_eq!(
        visit_original(
            &view,
            GroupImage::Predecessor,
            &mut RepoWork::bounded(108),
            |_, _, _| {
                seen += 1;
                Ok(())
            }
        ),
        Ok(())
    );
    assert_eq!(seen, 2);
    assert_eq!(
        visit_original(
            &view,
            GroupImage::Predecessor,
            &mut RepoWork::bounded(107),
            |_, _, _| Ok(())
        ),
        Err(GroupedOwnershipError::WorkLimit)
    );
    for (key, exact) in [(&first, 162), (&second, 150)] {
        assert_eq!(
            lookup(
                &view,
                GroupImage::Predecessor,
                key,
                &mut RepoWork::bounded(exact - 1)
            ),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(
            lookup(
                &view,
                GroupImage::Predecessor,
                key,
                &mut RepoWork::bounded(exact)
            ),
            Ok(view.current().get(key))
        );
    }
    let members = BTreeSet::from([first.clone(), second.clone()]);
    for (key, exact) in [(&first, 56), (&second, 44)] {
        assert_eq!(
            contains(&members, key, &mut RepoWork::bounded(exact - 1)),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(
            contains(&members, key, &mut RepoWork::bounded(exact)),
            Ok(true)
        );
    }
    struct Count(usize);
    impl Iterator for Count {
        type Item = ();
        fn next(&mut self) -> Option<()> {
            self.0 += 1;
            Some(())
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (1, Some(1))
        }
    }
    impl ExactSizeIterator for Count {}
    let mut rows = Count(0);
    assert_eq!(
        next_physical(&mut rows, &mut RepoWork::bounded(0)),
        Err(GroupedOwnershipError::WorkLimit)
    );
    assert_eq!(rows.0, 0);
    assert_eq!(
        next_physical(&mut rows, &mut RepoWork::bounded(1)),
        Ok(Some(()))
    );
    assert_eq!(rows.0, 1);
}
#[test]
fn masked_repo_current_rows_and_every_absent_preimage_are_prepaid() {
    let rows = Storage::from_snapshot_parts(
        BTreeMap::from([(id(), 1_u32)]),
        BTreeMap::from([(id(), None), (absent_id(), None)]),
    );
    let view = rows.try_committed_view_nonblocking().unwrap();
    assert_eq!(
        lookup(
            &view,
            GroupImage::Predecessor,
            &id(),
            &mut RepoWork::bounded(61)
        ),
        Err(GroupedOwnershipError::WorkLimit)
    );
    assert_eq!(
        lookup(
            &view,
            GroupImage::Predecessor,
            &id(),
            &mut RepoWork::bounded(62)
        ),
        Ok(None)
    );
}
#[test]
fn full_repo_name_and_controller_geometry_precedes_equal_or_unequal_keys() {
    let wide = multisig();
    let mut key = ALICE_ID.expect_single_signatory().clone();
    key.zeroize_for_confidential_discard();
    let discarded = AccountId::new(key);
    for (left, right, exact, truth) in [
        (&*ALICE_ID, &*ALICE_ID, 68, true),
        (&wide, &wide, 168, true),
        (&*ALICE_ID, &wide, 118, false),
        (&discarded, &discarded, 4, true),
    ] {
        assert_eq!(
            equal(left, right, &mut RepoWork::bounded(exact - 1)),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(equal(left, right, &mut RepoWork::bounded(exact)), Ok(truth));
    }
    let unicode: RepoAgreementId = "é".parse().unwrap();
    let long: RepoAgreementId = "x".repeat(255).parse().unwrap();
    for (left, right, exact, truth) in [
        (&id(), &id(), 30, true),
        (&id(), &absent_id(), 25, false),
        (&unicode, &unicode, 4, true),
        (&long, &long, 510, true),
    ] {
        assert_eq!(
            equal(left, right, &mut RepoWork::bounded(exact - 1)),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(equal(left, right, &mut RepoWork::bounded(exact)), Ok(truth));
    }
    for present in [false, true] {
        let world = fixture(present);
        let view = world.repo_agreements.view();
        let record = view.get(&id()).unwrap();
        assert_eq!(
            initiator(record, &mut RepoWork::bounded(0)),
            Ok(Some(record.initiator()))
        );
        assert_eq!(
            counterparty(record, &mut RepoWork::bounded(0)),
            Ok(Some(record.counterparty()))
        );
        assert_eq!(
            custodian(record, &mut RepoWork::bounded(0)),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(
            custodian(record, &mut RepoWork::bounded(1)),
            Ok(record.custodian().as_ref())
        );
    }
}
#[test]
fn absent_repo_source_and_account_bucket_rows_have_independent_exact_work() {
    for (custodian, base, delta) in [(false, 816, 140), (true, 1222, 168)] {
        let world = fixture(custodian);
        {
            let mut block = world.repo_agreements.block();
            block.remove(absent_id());
            block.commit();
        }
        assert_eq!(world_work(&world), base + delta);
        assert_eq!(
            check(&world, base + delta - 1),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(check(&world, base + delta), Ok(()));
    }
    for (custodian, source, base, delta) in [
        (true, 0, 1222, 142),
        (true, 1, 1222, 142),
        (true, 2, 1222, 142),
        (false, 2, 816, 2),
    ] {
        let world = fixture(custodian);
        match source {
            0 => {
                let mut b = world.repo_agreements_by_initiator.block();
                b.remove(absent_account());
                b.commit();
            }
            1 => {
                let mut b = world.repo_agreements_by_counterparty.block();
                b.remove(absent_account());
                b.commit();
            }
            2 => {
                let mut b = world.repo_agreements_by_custodian.block();
                b.remove(absent_account());
                b.commit();
            }
            _ => unreachable!(),
        }
        assert_eq!(world_work(&world), base + delta);
        assert_eq!(
            check(&world, base + delta - 1),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(check(&world, base + delta), Ok(()));
    }
}
#[test]
fn repo_phase_precedence_preserves_each_original_group_and_image() {
    let mut world = fixture(true);
    let saved = omit_initial(&mut world.repo_agreements_by_initiator, &ALICE_ID);
    {
        let mut b = world.repo_agreements_by_initiator.block();
        b.insert(ALICE_ID.clone(), saved);
        b.commit();
    }
    omit_initial(&mut world.repo_agreements_by_counterparty, &BOB_ID);
    assert_eq!(
        check(&world, TEST_WORK_ALLOWANCE),
        Err(corrupt(
            "world.repo_agreements_by_initiator",
            true,
            GroupMismatch::MissingMember
        ))
    );
    let mut world = fixture(true);
    let saved = omit_initial(&mut world.repo_agreements_by_counterparty, &BOB_ID);
    {
        let mut b = world.repo_agreements_by_counterparty.block();
        b.insert(BOB_ID.clone(), saved);
        b.commit();
    }
    omit_initial(&mut world.repo_agreements_by_custodian, &BOB_ID);
    assert_eq!(
        check(&world, TEST_WORK_ALLOWANCE),
        Err(corrupt(
            "world.repo_agreements_by_counterparty",
            true,
            GroupMismatch::MissingMember
        ))
    );
}
fn publish(world: &World, source: usize) {
    match source {
        0 => world.repo_agreements.block().commit(),
        1 => world.repo_agreements_by_initiator.block().commit(),
        2 => world.repo_agreements_by_counterparty.block().commit(),
        3 => world.repo_agreements_by_custodian.block().commit(),
        _ => unreachable!(),
    }
}
#[test]
fn every_repo_original_publication_precedes_all_validation_outcomes() {
    for source in 0..4 {
        for validation in 0..3 {
            let world = fixture(false);
            let checked = CheckedRepoAgreements::capture(&world, TEST_WORK_ALLOWANCE).unwrap();
            publish(&world, source);
            let result = match validation {
                0 => Ok(()),
                1 => Err(GroupedOwnershipError::WorkLimit),
                _ => Err(corrupt(
                    "world.repo_agreements_by_custodian",
                    false,
                    GroupMismatch::ForeignMember,
                )),
            };
            assert!(matches!(
                checked.finish_validation(result),
                Err(GroupedOwnershipError::Publication(
                    PublicationPreparationError::Changed
                ))
            ));
        }
    }
}
#[test]
fn all_repo_original_results_preserve_first_actual_native_refusal() {
    let world = fixture(true);
    let checked = CheckedRepoAgreements::capture(&world, TEST_WORK_ALLOWANCE).unwrap();
    publish(&world, 1);
    publish(&world, 3);
    let detached = world
        .repo_agreements
        .block()
        .try_detach(|_| Ok::<_, ()>(()))
        .unwrap();
    let prepared = detached
        .try_prepare_publication(&world.repo_agreements, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|(_, e, _)| panic!("original preparation: {e:?}"));
    let original = checked
        .rows
        .try_matches_current(&world.repo_agreements)
        .unwrap_err();
    assert!(matches!(original, PublicationPreparationError::Busy(_)));
    assert_eq!(
        checked
            .finish_validation(Err(GroupedOwnershipError::WorkLimit))
            .err(),
        Some(GroupedOwnershipError::Publication(original))
    );
    drop(prepared);
    assert_eq!(check(&world, TEST_WORK_ALLOWANCE), Ok(()));
}
#[test]
fn repo_grouping_does_not_add_economic_reference_or_record_id_authority() {
    let mut world = fixture(false);
    let mut record = world.repo_agreements.view().get(&id()).unwrap().clone();
    let mut key = ALICE_ID.expect_single_signatory().clone();
    key.zeroize_for_confidential_discard();
    record.initiator = AccountId::new(key);
    record.id = other_id();
    record.cash_leg.quantity = iroha_primitives::numeric::Quantity::zero();
    record.collateral_leg.quantity = iroha_primitives::numeric::Quantity::zero();
    record.maturity_timestamp_ms = 0;
    world.accounts = Storage::default();
    world.repo_agreements.insert(id(), record);
    world.rebuild_repo_agreement_indexes();
    assert_eq!(check(&world, TEST_WORK_ALLOWANCE), Ok(()));
    let source = include_str!("../repo_agreements.rs");
    let source = source.split("#[cfg(test)]").next().unwrap();
    for retired in [
        ".get(",
        ".contains(",
        ".contains_key(",
        ".range(",
        ".clone(",
        ".sort(",
        ".collect(",
        "::parse",
    ] {
        assert!(!source.contains(retired), "unfunded repo access: {retired}");
    }
}
