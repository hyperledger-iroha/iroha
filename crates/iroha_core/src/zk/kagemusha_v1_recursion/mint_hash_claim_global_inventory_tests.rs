//! Both-Pasta mutation checks for the proof-visible global source carrier.

use ff::{Field as _, WithSmallOrderMulGroup};
use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
};

use super::*;

const TEST_K: usize = 13;
const TEST_SOURCES: usize = 2;

fn fixture<F: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>>(
    parity: KagemushaPastaParityV1,
) -> (BaseCircuitBuilder<F>, Vec<Vec<F>>) {
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(TEST_K)
        .use_lookup_bits(TEST_K - 1)
        .use_instance_columns(3);
    let semantic = (0..KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1)
        .map(|value| builder.main(0).load_witness(F::from(value as u64)))
        .collect();
    builder.assigned_instances = vec![semantic];
    let common = (0..KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1)
        .map(|index| builder.main(0).load_witness(F::from(index as u64 + 201)))
        .collect::<Vec<_>>();
    let mut own = [11_u128, 12, 13, 14, 15, 16, 17, 18]
        .map(|value| builder.main(0).load_witness(F::from_u128(value)))
        .to_vec();
    own.extend(common.iter().copied());
    let mut opposite = vec![0_u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
    opposite[..8].copy_from_slice(&[21, 22, 23, 24, 25, 26, 27, 28]);
    for (index, value) in opposite[8..8 + common.len()].iter_mut().enumerate() {
        *value = index as u128 + 201;
    }
    // These known integers are the exact canonical u128 values of the common cells.
    attach_global_inventory_instances_v1(
        &mut builder,
        parity,
        own,
        TEST_SOURCES,
        &opposite,
        TEST_SOURCES,
        &common,
    )
    .expect("two-source global inventory carrier");
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let instances = builder
        .assigned_instances
        .iter()
        .map(|column| column.iter().map(|cell| *cell.value()).collect())
        .collect();
    (builder, instances)
}

fn check_parity<F: KagemushaPoseidonFieldV1 + WithSmallOrderMulGroup<3>>(
    parity: KagemushaPastaParityV1,
) {
    let (builder, baseline) = fixture::<F>(parity);
    assert_eq!(
        baseline.iter().map(Vec::len).collect::<Vec<_>>(),
        [113, 4090, 4090]
    );
    assert_eq!(baseline[0][INVENTORY_EQ_SOURCE_COUNT], F::from(2));
    assert_eq!(baseline[0][INVENTORY_EP_SOURCE_COUNT], F::from(2));
    MockProver::run(TEST_K as u32, &builder, baseline.clone())
        .expect("global inventory public binding")
        .assert_satisfied();

    let own_column = match parity {
        KagemushaPastaParityV1::Eq => 1,
        KagemushaPastaParityV1::Ep => 2,
    };
    let opposite_column = 3 - own_column;
    for (column, index) in [
        (own_column, 0), // source point
        (own_column, 2), // aggregate coefficient
        (own_column, 4), // second source, preserving count
        (opposite_column, 0),
        (opposite_column, 8),  // shared 58-cell bound tail
        (opposite_column, 90), // zero padding
        (0, INVENTORY_EQ_SOURCE_COUNT),
        (0, INVENTORY_EP_SOURCE_COUNT),
    ] {
        let mut changed = baseline.clone();
        changed[column][index] += F::ONE;
        assert!(
            MockProver::run(TEST_K as u32, &builder, changed)
                .expect("mutated global inventory public cell")
                .verify()
                .is_err(),
            "parity {parity:?} column {column} cell {index} escaped the proof boundary"
        );
    }
    let mut reordered = baseline;
    reordered[own_column].swap(0, 4);
    reordered[own_column].swap(1, 5);
    reordered[own_column].swap(2, 6);
    reordered[own_column].swap(3, 7);
    assert!(
        MockProver::run(TEST_K as u32, &builder, reordered)
            .expect("reordered global inventory sources")
            .verify()
            .is_err()
    );

    // Both the witness and public count agree on the false value. The source
    // count must still be constrained to the actual graph cardinality.
    let count_circuit = |claimed_sources| {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let count = constrain_inventory_source_count_v1(
            builder.main(0),
            &range,
            TEST_SOURCES,
            claimed_sources,
        )
        .expect("bounded graph count");
        builder.assigned_instances = vec![vec![count]];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        builder
    };
    MockProver::run(
        TEST_K as u32,
        &count_circuit(TEST_SOURCES),
        vec![vec![F::from(TEST_SOURCES as u64)]],
    )
    .expect("honest graph count")
    .assert_satisfied();
    assert!(
        MockProver::run(TEST_K as u32, &count_circuit(3), vec![vec![F::from(3)]])
            .expect("same false count in witness and public instance")
            .verify()
            .is_err()
    );
}

#[test]
fn global_inventory_public_sources_coefficients_counts_and_padding_are_bound_in_both_fields() {
    check_parity::<Fp>(KagemushaPastaParityV1::Eq);
    check_parity::<Fq>(KagemushaPastaParityV1::Ep);
}

#[test]
fn global_inventory_rejects_nonzero_reciprocal_padding_and_invalid_counts() {
    for parity in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K)
            .use_instance_columns(3);
        builder.assigned_instances = vec![
            (0..KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1)
                .map(|_| builder.main(0).load_witness(Fp::ZERO))
                .collect(),
        ];
        let common = (0..KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1)
            .map(|_| builder.main(0).load_witness(Fp::ZERO))
            .collect::<Vec<_>>();
        let mut own = (0..8)
            .map(|_| builder.main(0).load_witness(Fp::ONE))
            .collect::<Vec<_>>();
        own.extend(common.iter().copied());
        let mut opposite = vec![0_u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1];
        opposite[100] = 1;
        assert!(
            attach_global_inventory_instances_v1(
                &mut builder,
                parity,
                own.clone(),
                2,
                &opposite,
                2,
                &common
            )
            .is_err()
        );
        opposite[100] = 0;
        assert!(
            attach_global_inventory_instances_v1(
                &mut builder,
                parity,
                own.clone(),
                0,
                &opposite,
                2,
                &common
            )
            .is_err()
        );
        assert!(
            attach_global_inventory_instances_v1(
                &mut builder,
                parity,
                own,
                2,
                &opposite,
                1009,
                &common
            )
            .is_err()
        );
    }
}
