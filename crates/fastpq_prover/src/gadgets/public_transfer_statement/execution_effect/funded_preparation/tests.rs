//! Native original-credit and independent canonical-preparation parity controls.

use super::super::tests::{balance, public_inputs, supply, tape, transfer};
use super::*;
use iroha_data_model::fastpq::{FastpqExecutionQuantityKeyV1, execution_quantity_key_v1};
use iroha_test_samples::{ALICE_ID, BOB_ID};

mod reference;

fn fixture() -> FastpqExecutionEffectsV1 {
    tape(vec![
        transfer(1, 10, 0),
        supply(true, 5, 9, 10),
        transfer(2, 14, 1),
    ])
}
fn funded(effects: &FastpqExecutionEffectsV1) -> (Transitions, PreparedExecutionEffects) {
    let limits = ExecutionEffectLimits::default();
    let bytes = allocation_bytes(effects, limits).unwrap();
    let budget = AllocationBudget::new(bytes);
    let mut reservation = budget.try_reserve_bytes(bytes).unwrap();
    prepare(effects, public_inputs(), limits, &budget, &mut reservation).unwrap()
}
fn retained_bytes(transitions: &Transitions, prepared: &PreparedExecutionEffects) -> usize {
    let rows = transitions.as_slice().len();
    let keys = prepared.keys.as_slice().len();
    Layout::array::<FastpqStateTransition>(rows).unwrap().size()
        + Layout::array::<AllocationCharge>(rows * 3).unwrap().size()
        + transitions
            .as_slice()
            .iter()
            .map(|t| t.key.capacity() + t.pre_value.capacity() + t.post_value.capacity())
            .sum::<usize>()
        + Layout::array::<PublicKeyAllocation>(keys).unwrap().size()
        + Layout::array::<AllocationCharge>(keys).unwrap().size()
        + prepared
            .keys
            .as_slice()
            .iter()
            .map(|k| k.key.capacity())
            .sum::<usize>()
        + Layout::array::<ExecutionEffectRow>(rows).unwrap().size()
        + Layout::array::<[usize; 2]>(rows / 2).unwrap().size()
}

#[test]
fn borrowed_port_frames_match_all_owned_key_variants_and_root_alignment() {
    let balance = balance(&ALICE_ID);
    let owned = [
        FastpqExecutionQuantityKeyV1::Balance(balance.clone()),
        FastpqExecutionQuantityKeyV1::Supply(balance.asset.clone()),
        FastpqExecutionQuantityKeyV1::Lifecycle(balance.asset.clone()),
    ];
    let borrowed = [
        Key::Balance(Field(&balance)),
        Key::Supply(Field(&balance.asset)),
        Key::Lifecycle(Field(&balance.asset)),
    ];
    assert_eq!(
        std::mem::align_of::<Key<'_>>(),
        std::mem::align_of::<FastpqExecutionQuantityKeyV1>()
    );
    for (key, borrowed) in owned.iter().zip(borrowed) {
        for flags in [0, norito::core::header_flags::COMPACT_LEN] {
            let _flags = norito::core::DecodeFlagsGuard::enter(flags);
            let expected = execution_quantity_key_v1(key).unwrap();
            let budget = AllocationBudget::new(expected.len());
            let mut reservation = budget.try_reserve_bytes(expected.len()).unwrap();
            let actual = frame(&borrowed, KEY_PREFIX, expected.len(), &mut reservation).unwrap();
            assert_eq!(actual.as_slice(), expected);
            assert!(actual.belongs_to(&budget));
            assert_eq!(budget.reserved_bytes(), expected.len());
            assert!(frame(&borrowed, KEY_PREFIX, expected.len() - 1, &mut reservation).is_err());
            drop(actual);
            assert_eq!(budget.reserved_bytes(), 0);
        }
    }
}

#[test]
fn funded_public_preparation_matches_original_complete_rows_paths_and_work() {
    let retire = FastpqExecutionEffectKindV1::Retire(balance(&ALICE_ID).asset);
    let mut self_transfer = transfer(2, 10, 8);
    if let FastpqExecutionEffectKindV1::Transfer(t) = &mut self_transfer {
        t.destination = t.source.clone();
    }
    let mut second_asset = supply(true, 3, 0, 0);
    if let FastpqExecutionEffectKindV1::Mint(t) = &mut second_asset {
        t.balance = balance(&BOB_ID);
        t.balance.asset.incarnation =
            iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
                Hash::new(b"another exact incarnation").into(),
            )
            .unwrap();
    }
    let fixtures = [
        tape(vec![]),
        fixture(),
        tape(vec![self_transfer]),
        tape(vec![supply(false, 10, 10, 10), retire]),
        tape(vec![
            transfer(0, 10, 0),
            second_asset,
            supply(true, 2, 10, 10),
        ]),
    ];
    for effects in fixtures {
        let limits = ExecutionEffectLimits::default();
        let (expected_rows, original) =
            reference::prepare_facts(&effects, public_inputs(), limits).unwrap();
        let (rows, built) = funded(&effects);
        assert_eq!(rows.as_slice(), expected_rows);
        assert_eq!(built.keys(), original.keys);
        assert_eq!(built.rows(), original.rows);
        assert_eq!(built.pairs.as_slice(), original.pairs);
        assert_eq!(built.ordering_hash, original.ordering_hash);
        assert_eq!(built.work(), original.work);
        assert_eq!(
            norito::encode_canonical(&TransitionSequence(rows.as_slice())).unwrap(),
            norito::encode_canonical(&expected_rows).unwrap()
        );
        assert_eq!(
            std::mem::align_of::<TransitionSequence<'_>>(),
            std::mem::align_of::<Vec<FastpqStateTransition>>()
        );
    }
}

#[test]
fn generated_rows_keys_and_nested_bytes_keep_exact_original_credit_until_drop() {
    let effects = fixture();
    let limits = ExecutionEffectLimits::default();
    let demand = allocation_bytes(&effects, limits).unwrap();
    let sentinel = 37;
    let budget = AllocationBudget::new(demand + sentinel);
    let mut reservation = budget.try_reserve_bytes(demand + sentinel).unwrap();
    let (transitions, prepared) =
        prepare(&effects, public_inputs(), limits, &budget, &mut reservation).unwrap();
    assert_eq!(reservation.remaining_bytes(), sentinel);
    let exact = retained_bytes(&transitions, &prepared);
    assert_eq!(budget.reserved_bytes(), exact + sentinel);
    assert!(transitions.values.belongs_to(&budget));
    assert!(transitions.charges.belongs_to(&budget));
    assert!(
        transitions
            .charges
            .as_slice()
            .iter()
            .all(|charge| charge.belongs_to(&budget))
    );
    assert!(prepared.keys.values.belongs_to(&budget));
    assert!(prepared.keys.charges.belongs_to(&budget));
    assert!(
        prepared
            .keys
            .charges
            .as_slice()
            .iter()
            .all(|charge| charge.belongs_to(&budget))
    );
    assert!(prepared.rows.belongs_to(&budget));
    assert!(prepared.pairs.belongs_to(&budget));
    for row in transitions.as_slice() {
        assert_eq!(
            (row.key.len(), row.pre_value.len(), row.post_value.len()),
            (
                row.key.capacity(),
                row.pre_value.capacity(),
                row.post_value.capacity()
            )
        );
    }
    let pointer = transitions.as_slice()[0].key.as_ptr();
    let key_pointer = prepared.keys()[0].key.as_ptr();
    let moved = (transitions, prepared);
    assert_eq!(moved.0.as_slice()[0].key.as_ptr(), pointer);
    assert_eq!(moved.1.keys()[0].key.as_ptr(), key_pointer);
    drop(reservation);
    assert_eq!(budget.reserved_bytes(), exact);
    drop(moved);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn short_and_foreign_reservations_refuse_before_public_backing_allocates() {
    let effects = fixture();
    let limits = ExecutionEffectLimits::default();
    let demand = allocation_bytes(&effects, limits).unwrap();
    let budget = AllocationBudget::new(demand);
    let foreign = AllocationBudget::new(demand);
    let mut reservation = budget.try_reserve_bytes(demand - 1).unwrap();
    assert!(
        matches!(prepare(&effects, public_inputs(), limits, &budget, &mut reservation), Err(crate::Error::AllocationReservation(refusal)) if refusal.requested_bytes == demand && refusal.remaining_bytes == demand - 1)
    );
    assert_eq!(reservation.remaining_bytes(), demand - 1);
    assert_eq!(budget.reserved_bytes(), demand - 1);
    assert!(matches!(
        prepare(
            &effects,
            public_inputs(),
            limits,
            &foreign,
            &mut reservation
        ),
        Err(crate::Error::AllocationForeignPool)
    ));
    assert_eq!(reservation.remaining_bytes(), demand - 1);
    assert_eq!(foreign.reserved_bytes(), 0);
    drop(reservation);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn late_chronology_error_refunds_every_generated_backing_and_no_prefix_escapes() {
    let mut effects = fixture();
    if let FastpqExecutionEffectKindV1::Transfer(t) = &mut effects.effects[2].kind {
        t.source_before = 15_u32.into();
        t.source_after = 13_u32.into();
    }
    let limits = ExecutionEffectLimits::default();
    let bytes = allocation_bytes(&effects, limits).unwrap();
    let budget = AllocationBudget::new(bytes);
    let mut reservation = budget.try_reserve_bytes(bytes).unwrap();
    assert!(
        matches!(prepare(&effects, public_inputs(), limits, &budget, &mut reservation), Err(crate::Error::TransferInvariant { details }) if details == "execution effect repeated-key quantities do not chain")
    );
    assert_eq!(reservation.remaining_bytes(), 0);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn empty_funded_table_needs_no_backing_and_retains_root_equality_check() {
    let effects = tape(vec![]);
    let limits = ExecutionEffectLimits::default();
    assert_eq!(allocation_bytes(&effects, limits).unwrap(), 0);
    let budget = AllocationBudget::new(0);
    let mut reservation = budget.try_reserve_bytes(0).unwrap();
    let (rows, prepared) =
        prepare(&effects, public_inputs(), limits, &budget, &mut reservation).unwrap();
    assert!(rows.as_slice().is_empty());
    assert!(prepared.keys().is_empty());
    assert_eq!(budget.reserved_bytes(), 0);
    let mut changed = public_inputs();
    changed.new_root = Hash::new(b"wrong empty root").into();
    assert!(prepare(&effects, changed, limits, &budget, &mut reservation).is_err());
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn funded_quantity_frames_and_streamed_leaf_hash_match_the_full_limb_oracle() {
    use super::super::super::{TransferValue, encode_quantity_units_v1};
    use iroha_primitives::{bigint::BigInt, numeric::Numeric};
    let mut bytes = [0xff; 64];
    bytes[63] = 0x7f;
    let maximum = Quantity::from_canonical_numeric(
        Numeric::try_new(BigInt::from_twos_bytes(&bytes).unwrap(), 0).unwrap(),
    )
    .unwrap();
    for quantity in [
        Quantity::zero(),
        Quantity::one(),
        Quantity::from(u128::MAX),
        maximum,
    ] {
        for scale in 0..=28 {
            let units = FastpqQuantityUnits::from_quantity(&quantity, scale).unwrap();
            let expected = encode_quantity_units_v1(&units).unwrap();
            let budget = AllocationBudget::new(expected.len());
            let mut reservation = budget.try_reserve_bytes(expected.len()).unwrap();
            let actual = frame(
                &super::super::super::quantity::quantity_value_frame(&units),
                &[],
                super::super::super::QUANTITY_VALUE_MAX_BYTES_V1,
                &mut reservation,
            )
            .unwrap();
            assert_eq!(actual.as_slice(), expected);
            let key: [u8; 32] = Hash::new(b"exact full limb leaf key").into();
            assert_eq!(
                leaf(&key, actual.as_slice()),
                FastpqQuantityUnits::leaf(&key, units).unwrap()
            );
            drop(actual);
            assert_eq!(budget.reserved_bytes(), 0);
        }
    }
}

#[test]
fn compact_statement_backing_preserves_original_credit_and_root_rejections() {
    let effects = fixture();
    let (_, prepared) = funded(&effects);
    let roots = [
        Hash::new(b"intermediate one").into(),
        Hash::new(b"intermediate two").into(),
    ];
    let bytes = Layout::array::<PublicStatement>(prepared.pair_count())
        .unwrap()
        .size();
    let budget = AllocationBudget::new(bytes);
    let mut short = budget.try_reserve_bytes(bytes - 1).unwrap();
    assert!(
        prepared
            .compact_statements(&roots, &budget, &mut short)
            .is_err()
    );
    assert_eq!(short.remaining_bytes(), bytes - 1);
    drop(short);
    let mut reservation = budget.try_reserve_bytes(bytes).unwrap();
    assert!(
        prepared
            .compact_statements(&roots[..1], &budget, &mut reservation)
            .is_err()
    );
    assert_eq!(reservation.remaining_bytes(), bytes);
    let mut bad_roots = roots;
    bad_roots[0][31] &= !1;
    assert!(
        prepared
            .compact_statements(&bad_roots, &budget, &mut reservation)
            .is_err()
    );
    assert_eq!(reservation.remaining_bytes(), bytes);
    let output = prepared
        .compact_statements(&roots, &budget, &mut reservation)
        .unwrap();
    assert!(output.belongs_to(&budget));
    assert_eq!(output.as_slice().len(), 3);
    assert_eq!(output.as_slice()[0].new_root, output.as_slice()[1].old_root);
    assert_eq!(output.as_slice()[1].new_root, output.as_slice()[2].old_root);
    drop(reservation);
    assert_eq!(budget.reserved_bytes(), bytes);
    drop(output);
    assert_eq!(budget.reserved_bytes(), 0);
}

mod allocations;
