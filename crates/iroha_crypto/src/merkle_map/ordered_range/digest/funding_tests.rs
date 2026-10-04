//! Original-pool custody and unchanged commitment controls for final digest backing.

use super::*;
use iroha_allocation::release::ReleaseRegistration;
use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer};
use std::{
    alloc::Layout,
    panic::{AssertUnwindSafe, catch_unwind},
    task::{Context, Poll, Waker},
};

fn demand(rows: usize, key_bytes: usize) -> usize {
    let (levels, nodes) = if rows == 0 {
        (0, 0)
    } else {
        let leaves = rows.next_power_of_two();
        (leaves.ilog2() as usize + 1, 2 * leaves - 1)
    };
    Layout::array::<ChargedBuffer<Hash>>(levels).unwrap().size()
        + Layout::array::<Hash>(nodes).unwrap().size()
        + owned::owner_bytes()
        + Layout::array::<owned::DigestEntry>(rows).unwrap().size()
        + key_bytes
}

fn build(
    keys: &[&[u8]],
    budget: &AllocationBudget,
) -> Result<NoritoKeyDigestRangeTreeV1, NoritoKeyRangeError> {
    NoritoKeyDigestRangeTreeV1::from_sorted_digests(
        Hash::new(b"funded schema"),
        b"funded table",
        keys.iter().map(|key| (*key, Hash::new(b"value"))),
        MAX_NORITO_TREE_PAYLOAD_BYTES,
        budget,
    )
}

#[test]
fn zero_and_exact_capacity_cover_owner_rows_and_keys() {
    let zero = AllocationBudget::new(0);
    assert!(matches!(
        build(&[], &zero),
        Err(NoritoKeyRangeError::Admission(
            AllocationRefusal::ExceedsLimit { .. }
        ))
    ));
    assert_eq!(zero.reserved_bytes(), 0);
    let empty_budget = AllocationBudget::new(demand(0, 0));
    let empty = build(&[], &empty_budget).unwrap();
    assert_eq!(empty_budget.reserved_bytes(), demand(0, 0));
    assert!(empty.is_empty());
    drop(empty);
    assert_eq!(empty_budget.reserved_bytes(), 0);

    let keys: [&[u8]; 2] = [b"a", b"bb"];
    let budget = AllocationBudget::new(demand(2, 3));
    let tree = build(&keys, &budget).unwrap();
    assert_eq!(budget.reserved_bytes(), demand(2, 3));
    assert_eq!(tree.len(), 2);
    assert_eq!(tree.backing().entries.capacity(), 2);
    for (entry, key) in tree.backing().entries.as_slice().iter().zip(keys) {
        assert_eq!(entry.key.capacity(), key.len());
        assert_eq!(entry.key.as_slice(), key);
    }
    drop(tree);
    assert_eq!(budget.reserved_bytes(), 0);
    let short = AllocationBudget::new(demand(2, 3) - 1);
    assert!(matches!(
        build(&keys, &short),
        Err(NoritoKeyRangeError::Admission(_))
    ));
    assert_eq!(short.reserved_bytes(), 0);
}

#[test]
fn clones_and_borrows_retain_original_credit_after_shrink_and_eviction() {
    let keys: [&[u8]; 2] = [b"a", b"bb"];
    let budget = AllocationBudget::new(demand(2, 3));
    let original = build(&keys, &budget).unwrap();
    let root = original.root();
    let borrower = original.clone();
    assert!(ChargedShared::ptr_eq(&original.inner, &borrower.inner));
    budget.set_limit_bytes(0);
    drop(original);
    assert_eq!(budget.reserved_bytes(), demand(2, 3));
    std::thread::scope(|scope| {
        let concurrent = borrower.clone();
        scope
            .spawn(move || {
                assert_eq!(concurrent.root(), root);
                assert_eq!(concurrent.digest_rows().count(), 2);
            })
            .join()
            .unwrap();
    });
    assert_eq!(borrower.digest_rows().next().unwrap().0, b"a");
    assert_eq!(budget.reserved_bytes(), demand(2, 3));
    drop(borrower);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn failed_admission_keeps_the_original_pool_release_observation() {
    let observer_bytes = ReleaseRegistration::allocation_layout().size();
    let budget = AllocationBudget::new(demand(1, 1) + observer_bytes);
    let mut registration = ReleaseRegistration::from_reservation(
        &mut budget
            .try_reserve(ReleaseRegistration::allocation_layout())
            .unwrap(),
    )
    .unwrap();
    assert!(registration.belongs_to(&budget));
    let occupied = budget.try_reserve_bytes(demand(1, 1)).unwrap();
    let Err(NoritoKeyRangeError::Admission(AllocationRefusal::Capacity { release, .. })) =
        build(&[b"a"], &budget)
    else {
        panic!("expected exact original capacity refusal")
    };
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(
        registration.poll_wait(&release, &mut context),
        Poll::Pending
    );
    let unrelated = AllocationBudget::new(7);
    drop(unrelated.try_reserve_bytes(7).unwrap());
    assert_eq!(
        registration.poll_wait(&release, &mut context),
        Poll::Pending
    );
    drop(occupied);
    assert_eq!(
        registration.poll_wait(&release, &mut context),
        Poll::Ready(())
    );
    drop(build(&[b"a"], &budget).unwrap());
    assert_eq!(budget.reserved_bytes(), observer_bytes);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn final_keys_are_admitted_while_original_staging_still_owns_its_backing() {
    let total = 5 + demand(1, 5);
    let budget = AllocationBudget::new(total);
    let mut staging = ChargedBuffer::<u8>::new(5, &budget).unwrap();
    staging.append(b"abcde").unwrap();
    let tree = build(&[staging.as_slice()], &budget).unwrap();
    assert_eq!(budget.reserved_bytes(), total);
    assert_ne!(
        tree.digest_rows().next().unwrap().0.as_ptr(),
        staging.as_slice().as_ptr()
    );
    drop(staging);
    assert_eq!(budget.reserved_bytes(), demand(1, 5));
    assert_eq!(tree.digest_rows().next().unwrap().0, b"abcde");
    drop(tree);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn failed_order_validation_and_producer_unwind_release_every_funded_backing() {
    let budget = AllocationBudget::new(demand(2, 2));
    for keys in [
        [b"b".as_slice(), b"a".as_slice()],
        [b"a".as_slice(), b"a".as_slice()],
    ] {
        assert!(matches!(
            build(&keys, &budget),
            Err(NoritoKeyRangeError::UnsortedKeys)
        ));
        assert_eq!(budget.reserved_bytes(), 0);
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut count = 0;
        let rows = [b"a".as_slice(), b"b".as_slice()].into_iter().map(|key| {
            count += 1;
            assert_ne!(count, 2, "producer failed after first retained row");
            (key, Hash::new(b"value"))
        });
        let _ = NoritoKeyDigestRangeTreeV1::from_sorted_digests(
            Hash::new(b"schema"),
            b"table",
            rows,
            4096,
            &budget,
        );
    }));
    assert!(result.is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

struct ClaimedRows {
    keys: std::array::IntoIter<&'static [u8], 2>,
    claimed: usize,
}
impl Iterator for ClaimedRows {
    type Item = (&'static [u8], Hash);
    fn next(&mut self) -> Option<Self::Item> {
        self.keys.next().map(|key| (key, Hash::new(b"value")))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.claimed, Some(self.claimed))
    }
}
impl ExactSizeIterator for ClaimedRows {}

#[test]
fn false_exact_size_claims_never_yield_partial_trees() {
    let budget = AllocationBudget::new(4096);
    for claimed in [0, 1, 3] {
        let result = NoritoKeyDigestRangeTreeV1::from_sorted_digests(
            Hash::new(b"schema"),
            b"table",
            ClaimedRows {
                keys: [b"a".as_slice(), b"b".as_slice()].into_iter(),
                claimed,
            },
            4096,
            &budget,
        );
        assert!(matches!(result, Err(NoritoKeyRangeError::Capacity)));
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn funded_root_and_complete_ranges_match_the_existing_full_value_commitment() {
    let schema = Hash::new(b"exact schema");
    let keys: [&[u8]; 3] = [b"a", b"bb", b"ccc"];
    let values: [&[u8]; 3] = [b"first", b"second", b"third"];
    let full = super::super::NoritoKeyRangeTreeV1::from_sorted(
        schema,
        b"exact table",
        keys.into_iter().zip(values),
    )
    .unwrap();
    let budget = AllocationBudget::new(demand(3, 6));
    let funded = NoritoKeyDigestRangeTreeV1::from_sorted_digests(
        schema,
        b"exact table",
        keys.into_iter().zip(values).map(|(key, value)| {
            (
                key,
                super::super::digest_frame(super::super::VALUE_DOMAIN, value),
            )
        }),
        4096,
        &budget,
    )
    .unwrap();
    assert_eq!(full.root(), funded.root());
    for (start, end) in [
        (b"".as_slice(), b"z".as_slice()),
        (b"a".as_slice(), b"ccc".as_slice()),
        (b"b".as_slice(), b"c".as_slice()),
        (b"d".as_slice(), b"z".as_slice()),
    ] {
        let proof = funded.prove_range(start, end, 3, 4096).unwrap();
        let verified = proof
            .verify(NoritoKeyRangeVerifyRequestV1 {
                expected_root: &full.root(),
                schema_hash: &schema,
                domain: b"exact table",
                start,
                end,
                max_rows: 3,
                max_bytes: 4096,
            })
            .unwrap();
        let expected: Vec<_> = keys
            .into_iter()
            .filter(|key| *key >= start && *key < end)
            .collect();
        assert_eq!(
            verified.rows().map(|(key, _)| key).collect::<Vec<_>>(),
            expected
        );
    }
    assert_eq!(budget.reserved_bytes(), demand(3, 6));
    drop(funded);
    assert_eq!(budget.reserved_bytes(), 0);
}
