//! Exact private paths, construction budgets and full-domain producer regression tests.

use super::super::{
    decode_quantity_units_v1, prepare_public_transfers,
    quantity_tests::{delta, fixture, maximum, tiny},
};
use super::*;
use crate::gadgets::{compact_smt_air::PublicUpdate, transfer};
use iroha_primitives::numeric::Quantity;

fn limits(updates: usize) -> TransferSmtBuildLimits {
    TransferSmtBuildLimits::for_update_limit(updates).unwrap()
}

fn materialize(
    claims: &[PublicTransferTranscript],
    inputs: PublicInputs,
) -> QuantityTransferMaterialization {
    materialize_quantity_public_transfers(
        claims,
        inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
        limits(32),
    )
    .unwrap()
}

fn fold(leaf: [u32; 8], path: u32, siblings: &[[u8; 32]]) -> [u8; 32] {
    assert_eq!(siblings.len(), HEIGHT);
    let mut current = digest(leaf).unwrap();
    for (level, sibling) in siblings.iter().enumerate() {
        let sibling = Hash::prehashed(*sibling);
        let (left, right) = if path >> level & 1 == 0 {
            (current, sibling)
        } else {
            (sibling, current)
        };
        current = Hash::new(
            [
                b"fastpq:v1:smt:node|".as_slice(),
                left.as_ref(),
                right.as_ref(),
            ]
            .concat(),
        );
    }
    current.into()
}

fn verify_paths(
    prepared: &PreparedPublicTransfers<'_, FastpqQuantityUnits>,
    built: &DerivedTransferSmtWitnesses,
) {
    assert_eq!(built.pairs().len(), prepared.pairs().len());
    let mut root = built.roots().0;
    for (pair, paths) in prepared.pairs().iter().zip(built.pairs()) {
        for (update, witness) in pair.updates.iter().zip(paths) {
            assert_eq!(witness.path_bits, update.path.to_le_bytes());
            assert_eq!(witness.root_before, root);
            assert_eq!(fold(update.old_leaf, update.path, &witness.siblings), root);
            assert_eq!(
                fold(update.new_leaf, update.path, &witness.siblings),
                witness.root_after
            );
            root = witness.root_after;
        }
    }
    assert_eq!(root, built.roots().1);
}

#[test]
fn intermediate_roots_preserve_whole_batch_boundaries_and_equal_occurrences() {
    let first = delta(Quantity::from(u128::MAX), Quantity::zero(), Quantity::one());
    let second = delta(
        first.from_balance_after.clone(),
        first.to_balance_after.clone(),
        tiny(),
    );
    let third = delta(
        second.from_balance_after.clone(),
        second.to_balance_after.clone(),
        Quantity::one(),
    );
    let mut unchanged = delta(maximum(), maximum(), Quantity::zero());
    unchanged.to_account = unchanged.from_account.clone();
    for deltas in [
        vec![delta(maximum(), Quantity::zero(), maximum())],
        vec![first.clone()],
        vec![first, second, third],
        vec![unchanged.clone(), unchanged.clone(), unchanged],
    ] {
        let pair_count = deltas.len();
        let (claims, _, inputs) = fixture(deltas);
        let built = materialize(&claims, inputs);
        let prepared = prepare_quantity_public_transfers(
            built.transitions(),
            &claims,
            built.public_inputs(),
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
        )
        .unwrap();
        verify_paths(&prepared, built.witnesses());
        let work = built.witnesses().work();
        let expected: Vec<_> = prepared
            .pairs()
            .iter()
            .zip(built.witnesses().pairs())
            .take(pair_count - 1)
            .map(|(pair, paths)| {
                let credit = &pair.updates[1];
                fold(credit.new_leaf, credit.path, &paths[1].siblings)
            })
            .collect();
        let mut roots = built.witnesses().intermediate_roots();
        assert_eq!(roots.len(), pair_count - 1);
        assert_eq!(roots.next(), expected.first().copied());
        if pair_count > 1 {
            assert_eq!(roots.len(), pair_count - 2);
            assert_eq!(roots.next_back(), expected.last().copied());
        }
        assert_eq!(roots.next(), None);
        assert_eq!(roots.next_back(), None);
        assert_eq!(
            built.witnesses().intermediate_roots().collect::<Vec<_>>(),
            expected
        );
        assert_eq!(built.witnesses().work(), work);
        if built.witnesses().roots().0 == built.witnesses().roots().1 {
            assert_eq!(expected, vec![built.witnesses().roots().0; pair_count - 1]);
        } else if pair_count > 1 {
            assert_ne!(expected[0], expected[1]);
            assert_ne!(expected[1], built.witnesses().roots().1);
        }
    }
    let empty = materialize_quantity_public_transfers(
        &[],
        PublicInputs::default(),
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
        limits(0),
    )
    .unwrap();
    let mut roots = empty.witnesses().intermediate_roots();
    assert_eq!(roots.len(), 0);
    assert_eq!(roots.next(), None);
    assert_eq!(roots.next_back(), None);
    assert_eq!(empty.witnesses().work(), TransferSmtBuildWork::default());
}

#[test]
fn full_domain_materialization_preserves_claims_and_authenticates_every_private_path() {
    for d in [
        delta(Quantity::from(u128::MAX), Quantity::zero(), Quantity::one()),
        delta(maximum(), Quantity::zero(), maximum()),
        delta(
            maximum().try_sub(&Quantity::one()).unwrap(),
            tiny(),
            Quantity::one(),
        ),
        delta(Quantity::from(2_u32), Quantity::zero(), tiny()),
    ] {
        let (claims, _, mut inputs) = fixture(vec![d]);
        let before = norito::encode_canonical(&claims).unwrap();
        inputs.old_root = [0; 32];
        inputs.new_root = [0; 32];
        let built = materialize(&claims, inputs);
        let actual = built.public_inputs();
        assert_eq!(actual.slot, inputs.slot);
        assert_eq!(actual.dsid, inputs.dsid);
        assert_eq!(actual.perm_root, inputs.perm_root);
        assert_eq!(actual.tx_set_hash, inputs.tx_set_hash);
        assert_ne!(actual.old_root, [0; 32]);
        assert_ne!(actual.old_root, <[u8; 32]>::from(padding(HEIGHT)));
        assert_eq!(
            (actual.old_root, actual.new_root),
            built.witnesses().roots()
        );
        let prepared = prepare_quantity_public_transfers(
            built.transitions(),
            &claims,
            actual,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
        )
        .unwrap();
        assert_eq!(prepared.ordering_hash(), built.ordering_hash());
        verify_paths(&prepared, built.witnesses());
        assert_eq!(
            prepared.build_smt_witnesses(limits(2)).unwrap(),
            *built.witnesses()
        );
        assert_eq!(norito::encode_canonical(&claims).unwrap(), before);
        let expected = built.witnesses().clone();
        let expected_rows = prepared.transitions().to_vec();
        let expected_ordering = prepared.ordering_hash();
        drop(prepared);
        let (rows, returned_inputs, ordering, paths) = built.into_parts();
        assert_eq!(rows, expected_rows);
        assert_eq!(returned_inputs, actual);
        assert_eq!(ordering, expected_ordering);
        assert_eq!(paths, expected);
    }
}

#[test]
fn shared_private_tree_matches_existing_narrow_native_paths_exactly() {
    let d = delta(
        Quantity::from(20_u32),
        Quantity::from(5_u32),
        Quantity::from(3_u32),
    );
    let from_key = balance_key(&d.asset_definition, &d.from_account).unwrap();
    let to_key = balance_key(&d.asset_definition, &d.to_account).unwrap();
    let (native_from, native_to) =
        transfer::build_transfer_smt_witness_pair(&from_key, 20, 17, &to_key, 5, 8).unwrap();
    let (claims, mut rows, mut inputs) = fixture(vec![d]);
    for row in &mut rows {
        row.pre_value = decode_quantity_units_v1(&row.pre_value)
            .unwrap()
            .try_to_u64()
            .unwrap()
            .to_le_bytes()
            .to_vec();
        row.post_value = decode_quantity_units_v1(&row.post_value)
            .unwrap()
            .try_to_u64()
            .unwrap()
            .to_le_bytes()
            .to_vec();
    }
    inputs.old_root = native_from.root_before;
    inputs.new_root = native_to.root_after;
    let prepared = prepare_public_transfers(
        &rows,
        &claims,
        inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let generated = prepared.build_smt_witnesses(limits(2)).unwrap();
    assert_eq!(generated.pairs(), &[[native_from, native_to]]);
}

#[test]
fn exact_private_work_bounds_pass_and_one_less_rejects() {
    let (claims, _, inputs) = fixture(vec![delta(maximum(), Quantity::zero(), Quantity::one())]);
    let built = materialize(&claims, inputs);
    let prepared = prepare_quantity_public_transfers(
        built.transitions(),
        &claims,
        built.public_inputs(),
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let work = built.witnesses().work();
    assert_eq!(work.updates, 2);
    assert_eq!(work.unique_keys, 2);
    assert_eq!(work.sibling_hashes, 64);
    assert_eq!(
        work.node_hashes,
        work.retained_nodes - work.unique_keys + 64
    );
    let exact = TransferSmtBuildLimits {
        max_updates: work.updates,
        max_unique_keys: work.unique_keys,
        max_retained_nodes: work.retained_nodes,
        max_sibling_hashes: work.sibling_hashes,
        max_node_hashes: work.node_hashes,
    };
    assert!(prepared.build_smt_witnesses(exact).is_ok());
    for limited in [
        TransferSmtBuildLimits {
            max_updates: exact.max_updates - 1,
            ..exact
        },
        TransferSmtBuildLimits {
            max_unique_keys: exact.max_unique_keys - 1,
            ..exact
        },
        TransferSmtBuildLimits {
            max_retained_nodes: exact.max_retained_nodes - 1,
            ..exact
        },
        TransferSmtBuildLimits {
            max_sibling_hashes: exact.max_sibling_hashes - 1,
            ..exact
        },
        TransferSmtBuildLimits {
            max_node_hashes: exact.max_node_hashes - 1,
            ..exact
        },
    ] {
        assert!(prepared.build_smt_witnesses(limited).is_err());
    }
    assert!(TransferSmtBuildLimits::for_update_limit(usize::MAX).is_none());
}

#[test]
fn rooted_builder_rejects_wrong_expected_roots() {
    let (claims, rows, inputs) = fixture(vec![delta(maximum(), Quantity::zero(), Quantity::one())]);
    let prepared = prepare_quantity_public_transfers(
        &rows,
        &claims,
        inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    assert!(
        matches!(prepared.build_smt_witnesses(limits(2)), Err(crate::Error::TransferInvariant { details }) if details.contains("roots differ"))
    );
}

#[test]
fn invalid_internal_ports_and_chronological_leaves_never_produce_paths() {
    let first = delta(Quantity::from(u128::MAX), Quantity::zero(), Quantity::one());
    let second = delta(
        first.from_balance_after.clone(),
        first.to_balance_after.clone(),
        Quantity::one(),
    );
    let (claims, rows, inputs) = fixture(vec![first, second]);
    for mutation in 0..7 {
        let mut prepared = prepare_quantity_public_transfers(
            &rows,
            &claims,
            inputs,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
        )
        .unwrap();
        match mutation {
            0 => prepared.keys[1].path = prepared.keys[0].path,
            1 => prepared.pairs[0].row_indices[1] = prepared.pairs[0].row_indices[0],
            2 => prepared.pairs[0].row_indices[0] = usize::MAX,
            3 => prepared.pairs[0].occurrence.pair_ordinal = 1,
            4 => prepared.rows[0].key_index = usize::MAX,
            5 => prepared.pairs[0].updates[0].path ^= 1,
            _ => {
                let index = prepared.pairs[1].row_indices[0];
                let update = PublicUpdate {
                    old_leaf: super::super::digest_limbs(Hash::new(b"wrong later leaf").into()),
                    ..prepared.rows[index].update
                };
                prepared.rows[index].update = update;
                prepared.pairs[1].updates[0] = update;
            }
        }
        assert!(derive(&prepared, limits(4)).is_err(), "mutation {mutation}");
    }
}

#[test]
fn zero_self_updates_retain_all_occurrences_and_empty_roots_stay_unchanged() {
    let mut d = delta(maximum(), maximum(), Quantity::zero());
    d.to_account = d.from_account.clone();
    let (claims, _, inputs) = fixture(vec![d.clone(), d]);
    let built = materialize(&claims, inputs);
    assert_eq!(built.witnesses().pairs().len(), 2);
    assert_eq!(built.witnesses().work().updates, 4);
    assert_eq!(built.witnesses().work().unique_keys, 1);
    assert_eq!(built.witnesses().roots().0, built.witnesses().roots().1);
    let prepared = prepare_quantity_public_transfers(
        built.transitions(),
        &claims,
        built.public_inputs(),
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    verify_paths(&prepared, built.witnesses());
    let empty = materialize_quantity_public_transfers(
        &[],
        PublicInputs::default(),
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
        limits(0),
    )
    .unwrap();
    assert!(empty.transitions().is_empty());
    assert!(empty.witnesses().pairs().is_empty());
    assert_eq!(empty.public_inputs(), PublicInputs::default());
    assert_eq!(empty.witnesses().work(), TransferSmtBuildWork::default());
    assert!(
        materialize_quantity_public_transfers(
            &[],
            inputs,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
            limits(0)
        )
        .is_err()
    );
    assert!(
        materialize_quantity_public_transfers(
            &[],
            PublicInputs::default(),
            ProofSemantics::AxtTransferClaim,
            PublicTransferLimits::default(),
            limits(0)
        )
        .is_err()
    );
}

#[test]
fn materialization_honors_exact_public_bytes_and_never_repairs_bad_claims() {
    let (claims, _, inputs) = fixture(vec![delta(maximum(), Quantity::zero(), Quantity::one())]);
    let built = materialize(&claims, inputs);
    let prepared = prepare_quantity_public_transfers(
        built.transitions(),
        &claims,
        built.public_inputs(),
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let exact = PublicTransferLimits {
        max_public_bytes: prepared.work().public_bytes,
        ..PublicTransferLimits::default()
    };
    assert!(
        materialize_quantity_public_transfers(
            &claims,
            inputs,
            ProofSemantics::StateTransition,
            exact,
            limits(2)
        )
        .is_ok()
    );
    assert!(
        materialize_quantity_public_transfers(
            &claims,
            inputs,
            ProofSemantics::StateTransition,
            PublicTransferLimits {
                max_public_bytes: exact.max_public_bytes - 1,
                ..exact
            },
            limits(2)
        )
        .is_err()
    );
    let mut bad = claims.clone();
    bad[0].deltas[0].from_balance_after = Quantity::zero();
    let before = norito::encode_canonical(&bad).unwrap();
    assert!(
        materialize_quantity_public_transfers(
            &bad,
            inputs,
            ProofSemantics::StateTransition,
            exact,
            limits(2)
        )
        .is_err()
    );
    assert_eq!(norito::encode_canonical(&bad).unwrap(), before);
}

#[test]
fn materialization_is_deterministic_across_ambient_codec_layouts() {
    let (claims, _, inputs) = fixture(vec![delta(maximum(), Quantity::zero(), Quantity::one())]);
    let expected = materialize(&claims, inputs);
    let _flags = norito::core::DecodeFlagsGuard::enter(0);
    let actual = materialize(&claims, inputs);
    assert_eq!(actual.transitions(), expected.transitions());
    assert_eq!(actual.public_inputs(), expected.public_inputs());
    assert_eq!(actual.ordering_hash(), expected.ordering_hash());
    assert_eq!(actual.witnesses(), expected.witnesses());
}
