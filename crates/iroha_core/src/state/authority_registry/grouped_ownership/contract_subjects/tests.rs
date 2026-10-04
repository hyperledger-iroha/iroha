//! Both-image source/inverse rules and original four-reader custody.
use super::*;
use crate::{
    state::contract_subject_validation::test_support::*, test_allocations::allocations_during,
};
use iroha_data_model::smart_contract::{
    ContractDeploymentOriginV1, ContractEmergencyHoldV1, ContractLifecycleOwnerV1,
    ContractParliamentDelegationV1, ParliamentContractDeploymentOriginV1,
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::Storage;

fn check(world: &World, work: u64) -> Result<(), GroupedOwnershipError> {
    let mut result = None;
    assert_eq!(
        allocations_during(|| {
            result = Some(CheckedContractSubjects::capture(world, work).map(|_| ()));
        }),
        0
    );
    result.unwrap()
}
fn malformed(world: &mut World, bad: ContractSubjectBinding, prior: bool) {
    world.contract_subject_bindings.insert(address(), bad);
    if prior {
        let mut block = world.contract_subject_bindings.block();
        block.insert(address(), binding());
        block.commit();
    }
}
fn source_image(error: GroupedOwnershipError, prior: bool) {
    assert!(
        matches!(error, GroupedOwnershipError::Source { image, .. } if image == if prior { GroupImage::Predecessor } else { GroupImage::Current })
    );
}

#[test]
fn source_derivation_and_every_existing_lifecycle_rule_reject_in_both_images_without_allocations() {
    for prior in [false, true] {
        for defect in 0..7 {
            let mut world = world();
            let mut bad = binding();
            match defect {
                0 => bad.subject = ALICE_ID.clone(),
                1 => bad.lifecycle.version = 2,
                2 => bad.lifecycle.revision = 0,
                3 => bad.lifecycle.pending_owner = Some(bad.lifecycle.owner.clone()),
                4 => {
                    bad.lifecycle.owner = ContractLifecycleOwnerV1::Parliament;
                    bad.lifecycle.parliament_delegation = ContractParliamentDelegationV1::Lifecycle;
                }
                5 => {
                    bad.lifecycle.origin = ContractDeploymentOriginV1::Parliament(
                        ParliamentContractDeploymentOriginV1 {
                            proposer: ALICE_ID.clone(),
                            proposal_content_id: [0; 32],
                            governance_attempt_id: [1; 32],
                        },
                    )
                }
                6 => {
                    bad.lifecycle.emergency_hold = Some(ContractEmergencyHoldV1 {
                        incident_digest: [1; 32],
                        proposal_content_id: [2; 32],
                        governance_attempt_id: [3; 32],
                        reason: "\u{2003}".into(),
                        imposed_at_height: 1,
                        expires_at_height: 2,
                    })
                }
                _ => unreachable!(),
            }
            malformed(&mut world, bad, prior);
            source_image(check(&world, 100_000).unwrap_err(), prior);
        }
    }
}

#[test]
fn subject_current_and_pending_accounts_and_active_hash_use_the_matching_image() {
    for prior in [false, true] {
        for defect in 0..5 {
            let mut world = world();
            if defect < 3 {
                let mut pending = binding();
                pending.lifecycle.pending_owner =
                    Some(ContractLifecycleOwnerV1::Account(BOB_ID.clone()));
                world.contract_subject_bindings.insert(address(), pending);
                let missing = [binding().subject, ALICE_ID.clone(), BOB_ID.clone()][defect].clone();
                let value = world.accounts.view().get(&missing).unwrap().clone();
                let accounts = world
                    .accounts
                    .view()
                    .iter()
                    .filter(|(key, _)| *key != &missing)
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();
                world.accounts = accounts;
                if prior {
                    let mut block = world.accounts.block();
                    block.insert(missing, value);
                    block.commit();
                }
            } else {
                let key = if defect == 3 {
                    address()
                } else {
                    other_address()
                };
                world
                    .contract_instances
                    .insert(key.clone(), Hash::new(b"unbound active code"));
                if prior {
                    let mut block = world.contract_instances.block();
                    block.remove(key);
                    block.commit();
                }
            }
            source_image(check(&world, 100_000).unwrap_err(), prior);
        }
    }
}

#[test]
fn inactive_and_parliament_bindings_do_not_require_historical_origin_accounts() {
    let mut world = world();
    let mut retained = ContractSubjectBinding::new_parliament(
        &address(),
        other_address().subject_id(),
        [1; 32],
        [2; 32],
    );
    retained.lifecycle.pending_owner = Some(ContractLifecycleOwnerV1::Account(BOB_ID.clone()));
    world.contract_subject_bindings.insert(address(), retained);
    assert_eq!(check(&world, 100_000), Ok(()));
    let mut direct = binding();
    if let ContractDeploymentOriginV1::Direct(origin) = &mut direct.lifecycle.origin {
        origin.deployer = other_address().subject_id();
    }
    world.contract_subject_bindings.insert(address(), direct);
    assert_eq!(check(&world, 100_000), Ok(()));
}

#[test]
fn exact_reverse_rejects_missing_foreign_and_wrong_addresses_in_both_images() {
    for prior in [false, true] {
        for defect in 0..3 {
            let mut world = world();
            world.contract_subject_addresses = Storage::new();
            match defect {
                0 => (),
                1 => {
                    world
                        .contract_subject_addresses
                        .insert(binding().subject, other_address());
                }
                2 => {
                    world
                        .contract_subject_addresses
                        .insert(binding().subject, address());
                    world
                        .contract_subject_addresses
                        .insert(BOB_ID.clone(), address());
                }
                _ => unreachable!(),
            }
            if prior {
                let mut block = world.contract_subject_addresses.block();
                block.insert(binding().subject, address());
                if defect == 2 {
                    block.remove(BOB_ID.clone());
                }
                block.commit();
            }
            assert!(
                matches!(check(&world, 100_000), Err(GroupedOwnershipError::Corrupt { image, .. }) if image == if prior {GroupImage::Predecessor} else {GroupImage::Current})
            );
        }
    }
}

#[test]
fn every_original_reader_identity_is_retained_even_for_equal_value_publications() {
    for field in 0..4 {
        let world = world();
        let checked = CheckedContractSubjects::capture(&world, 100_000).unwrap();
        match field {
            0 => {
                let mut block = world.contract_subject_bindings.block();
                block.insert(address(), binding());
                block.commit();
            }
            1 => {
                let mut block = world.contract_subject_addresses.block();
                block.insert(binding().subject, address());
                block.commit();
            }
            2 => {
                let value = world.accounts.view().get(&ALICE_ID).unwrap().clone();
                let mut block = world.accounts.block();
                block.insert(ALICE_ID.clone(), value);
                block.commit();
            }
            3 => {
                let mut block = world.contract_instances.block();
                block.remove(other_address());
                block.commit();
            }
            _ => unreachable!(),
        }
        assert!(!checked.matches_current().unwrap());
        assert_eq!(checked.rows().get(&address()), Some(&binding()));
    }
}

#[test]
fn derivation_reason_geometry_and_physical_tombstones_only_exhaust_local_work() {
    let mut world = world();
    assert_eq!(check(&world, 0), Err(GroupedOwnershipError::WorkLimit));
    assert_eq!(check(&world, 400), Err(GroupedOwnershipError::WorkLimit));
    assert_eq!(check(&world, 100_000), Ok(()));
    let mut held = binding();
    held.lifecycle.emergency_hold = Some(ContractEmergencyHoldV1 {
        incident_digest: [1; 32],
        proposal_content_id: [2; 32],
        governance_attempt_id: [3; 32],
        reason: " ".repeat(100_000) + "incident",
        imposed_at_height: 1,
        expires_at_height: 2,
    });
    world.contract_subject_bindings.insert(address(), held);
    assert_eq!(check(&world, 50_000), Err(GroupedOwnershipError::WorkLimit));
    assert_eq!(check(&world, 1_000_000), Ok(()));
    world.contract_subject_bindings.insert(address(), binding());
    {
        let mut block = world.accounts.block();
        block.remove(other_address().subject_id());
        block.commit();
    }
    assert!(
        world
            .accounts
            .snapshot()
            .revert_map()
            .iter()
            .any(|(_, value)| value.is_none())
    );
    assert_eq!(check(&world, 100_000), Ok(()));
}

#[test]
fn malformed_subject_envelopes_and_nested_owner_comparisons_remain_allocation_free() {
    use iroha_data_model::account::controller::{MultisigMember, MultisigPolicy};
    let members = vec![
        MultisigMember::new(ALICE_ID.expect_single_signatory().clone(), 1).unwrap(),
        MultisigMember::new(BOB_ID.expect_single_signatory().clone(), 1).unwrap(),
    ];
    let multi = AccountId::new_multisig(MultisigPolicy::new(1, members).unwrap());
    let mut discarded = ALICE_ID.expect_single_signatory().clone();
    discarded.zeroize_for_confidential_discard();
    for subject in [AccountId::new(discarded), multi.clone()] {
        let mut world = world();
        let mut bad = binding();
        bad.subject = subject;
        malformed(&mut world, bad, false);
        source_image(check(&world, 100_000).unwrap_err(), false);
    }
    let mut world = world();
    let mut bad = binding();
    bad.lifecycle.owner = ContractLifecycleOwnerV1::Account(multi.clone());
    bad.lifecycle.pending_owner = Some(ContractLifecycleOwnerV1::Account(multi));
    malformed(&mut world, bad, false);
    assert_eq!(check(&world, 1_000), Err(GroupedOwnershipError::WorkLimit));
    assert!(matches!(
        check(&world, 100_000),
        Err(GroupedOwnershipError::Source {
            reason: "pending contract owner must differ from current owner",
            ..
        })
    ));
}

#[test]
fn masked_rows_and_absent_undo_entries_consume_work_before_filtering() {
    fn exact_work(world: &World) -> u64 {
        let (mut refused, mut admitted) = (0, 100_000);
        assert_eq!(check(world, refused), Err(GroupedOwnershipError::WorkLimit));
        assert_eq!(check(world, admitted), Ok(()));
        while admitted - refused > 1 {
            let middle = refused + (admitted - refused) / 2;
            match check(world, middle) {
                Ok(()) => admitted = middle,
                Err(GroupedOwnershipError::WorkLimit) => refused = middle,
                other => panic!("valid unchanged logical images: {other:?}"),
            }
        }
        assert_eq!(
            check(world, admitted - 1),
            Err(GroupedOwnershipError::WorkLimit)
        );
        assert_eq!(check(world, admitted), Ok(()));
        admitted
    }
    let world = world();
    let original = exact_work(&world);
    {
        let mut block = world.contract_subject_bindings.block();
        block.insert(address(), binding());
        block.commit();
    }
    let masked = exact_work(&world);
    assert!(
        masked > original,
        "masked current row and retained prior row are funded"
    );
    {
        let mut block = world.accounts.block();
        block.remove(other_address().subject_id());
        block.commit();
    }
    let tombstone = exact_work(&world);
    assert!(
        tombstone > masked,
        "an absent undo entry is funded before it is skipped"
    );
}
