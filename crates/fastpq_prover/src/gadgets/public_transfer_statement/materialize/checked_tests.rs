//! Shared private two-update tree parity, port integrity and bounded-work checks.

use super::super::{
    decode_quantity_units_v1, prepare_public_transfers,
    quantity_tests::{delta, fixture},
};
use super::*;
use crate::gadgets::transfer;
use iroha_primitives::numeric::Quantity;

#[derive(Clone)]
struct Table {
    inputs: PublicInputs,
    keys: Vec<PublicKeyAllocation>,
    rows: Vec<CheckedUpdateRow>,
    pairs: Vec<CheckedUpdatePair>,
}

impl Table {
    fn copy_from(table: &impl CheckedUpdateTable) -> Self {
        Self {
            inputs: table.public_inputs(),
            keys: table.keys().to_vec(),
            rows: (0..table.row_count())
                .map(|index| table.row(index).unwrap())
                .collect(),
            pairs: (0..table.pair_count())
                .map(|index| table.pair(index).unwrap())
                .collect(),
        }
    }
}

impl CheckedUpdateTable for Table {
    fn public_inputs(&self) -> PublicInputs {
        self.inputs
    }
    fn keys(&self) -> &[PublicKeyAllocation] {
        &self.keys
    }
    fn row_count(&self) -> usize {
        self.rows.len()
    }
    fn pair_count(&self) -> usize {
        self.pairs.len()
    }
    fn row(&self, index: usize) -> Option<CheckedUpdateRow> {
        self.rows.get(index).copied()
    }
    fn pair(&self, index: usize) -> Option<CheckedUpdatePair> {
        self.pairs.get(index).copied()
    }
}

fn limits(updates: usize) -> TransferSmtBuildLimits {
    TransferSmtBuildLimits::for_update_limit(updates).unwrap()
}

fn two_pairs() -> Table {
    let first = delta(
        Quantity::from(20_u32),
        Quantity::from(5_u32),
        Quantity::from(3_u32),
    );
    let second = delta(
        first.from_balance_after.clone(),
        first.to_balance_after.clone(),
        Quantity::one(),
    );
    let (claims, _, inputs) = fixture(vec![first, second]);
    let materialized = materialize_quantity_public_transfers(
        &claims,
        inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
        limits(4),
    )
    .unwrap();
    let prepared = prepare_quantity_public_transfers(
        materialized.transitions(),
        &claims,
        materialized.public_inputs(),
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let copied = Table::copy_from(&prepared);
    assert_eq!(
        derive_two_update_smt(&copied, limits(4)).unwrap(),
        *materialized.witnesses()
    );
    assert_eq!(
        prepared.build_smt_witnesses(limits(4)).unwrap(),
        *materialized.witnesses()
    );
    copied
}

#[test]
fn transfer_adapter_preserves_full_occurrences_and_native_tree_bytes() {
    let d = delta(
        Quantity::from(20_u32),
        Quantity::from(5_u32),
        Quantity::from(3_u32),
    );
    let from = balance_key(&d.asset_definition, &d.from_account).unwrap();
    let to = balance_key(&d.asset_definition, &d.to_account).unwrap();
    let (debit, credit) =
        transfer::build_transfer_smt_witness_pair(&from, 20, 17, &to, 5, 8).unwrap();
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
    inputs.old_root = debit.root_before;
    inputs.new_root = credit.root_after;
    let prepared = prepare_public_transfers(
        &rows,
        &claims,
        inputs,
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
    )
    .unwrap();
    let table = Table::copy_from(&prepared);
    for leg in 0..2 {
        let row = table.rows[table.pairs[0].row_indices[leg]];
        assert_eq!(row.occurrence, [0, 0, 0]);
        assert_eq!(row.leg, leg);
    }
    let generic = derive_two_update_smt(&table, limits(2)).unwrap();
    assert_eq!(generic.pairs(), &[[debit, credit]]);
    assert_eq!(generic, prepared.build_smt_witnesses(limits(2)).unwrap());
    assert_eq!(generic.roots(), (inputs.old_root, inputs.new_root));
    // Quantity-valued preparation uses exactly the same checked tree engine.
    let quantity = two_pairs();
    assert_eq!(quantity.pairs.len(), 2);
    assert_eq!(quantity.pairs[1].occurrence, [0, 1, 1]);
}

#[test]
fn generic_table_rejects_wrong_ports_roles_occurrences_and_reused_rows() {
    let original = two_pairs();
    for mutation in 0..11 {
        let mut table = original.clone();
        let first = table.pairs[0].row_indices[0];
        match mutation {
            0 => table.rows[first].leg = 1,
            1 => table.rows[first].occurrence[0] += 1,
            2 => table.rows[first].occurrence[1] += 1,
            3 => table.pairs[0].occurrence[2] = 1,
            4 => table.pairs[0].row_indices[1] = first,
            5 => table.pairs[0].row_indices[0] = usize::MAX,
            6 => table.rows[first].key_index = usize::MAX,
            7 => table.keys[1].path = table.keys[0].path,
            8 => table.pairs[0].updates[0].path ^= 1,
            9 => {
                table.rows[first].update.path ^= 1;
                table.pairs[0].updates[0] = table.rows[first].update;
            }
            10 => {
                table.rows.pop();
            }
            _ => unreachable!(),
        }
        assert!(
            derive_two_update_smt(&table, limits(4)).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn generic_table_rejects_stale_later_leaf_even_with_matching_public_ports() {
    let mut table = two_pairs();
    let row_index = table.pairs[1].row_indices[0];
    let old_leaf = super::super::digest_limbs(Hash::new(b"stale later state").into());
    table.rows[row_index].update.old_leaf = old_leaf;
    table.pairs[1].updates[0].old_leaf = old_leaf;
    assert!(matches!(derive_two_update_smt(&table, limits(4)),
        Err(crate::Error::TransferInvariant { details }) if details == "SMT chronological pre-leaf does not match current state"));
}

#[test]
fn generic_table_preserves_exact_work_limits_and_requires_empty_root_equality() {
    let table = two_pairs();
    let work = derive_two_update_smt(&table, limits(4)).unwrap().work();
    let exact = TransferSmtBuildLimits {
        max_updates: work.updates,
        max_unique_keys: work.unique_keys,
        max_retained_nodes: work.retained_nodes,
        max_sibling_hashes: work.sibling_hashes,
        max_node_hashes: work.node_hashes,
    };
    assert_eq!(derive_two_update_smt(&table, exact).unwrap().work(), work);
    for dimension in 0..5 {
        let mut short = exact;
        match dimension {
            0 => short.max_updates -= 1,
            1 => short.max_unique_keys -= 1,
            2 => short.max_retained_nodes -= 1,
            3 => short.max_sibling_hashes -= 1,
            4 => short.max_node_hashes -= 1,
            _ => unreachable!(),
        }
        assert!(
            derive_two_update_smt(&table, short).is_err(),
            "dimension {dimension}"
        );
    }
    let mut empty = Table {
        inputs: PublicInputs::default(),
        keys: Vec::new(),
        rows: Vec::new(),
        pairs: Vec::new(),
    };
    let derived = derive_two_update_smt(&empty, limits(0)).unwrap();
    assert_eq!(
        derived.roots(),
        (empty.inputs.old_root, empty.inputs.new_root)
    );
    assert_eq!(derived.work(), TransferSmtBuildWork::default());
    assert!(derived.pairs().is_empty());
    empty.inputs.new_root = Hash::new(b"changed empty root").into();
    assert!(derive_two_update_smt(&empty, limits(0)).is_err());
}
