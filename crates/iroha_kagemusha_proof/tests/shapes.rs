//! The shape selector on every relation shape: the smallest `k` that fits,
//! the shape within the 3.5 KB proof budget, exact proof lengths from the
//! descriptor, row inventories, and the descriptor and key bytes a verifier
//! reconstructs from bytes alone.

mod common;

use common::{
    TWO_LEVEL_BUDGET, TWO_LEVEL_SMALLEST, budget_shape, relation_shapes, smallest_shape,
    vesta_prover,
};
use iroha_kagemusha_proof::{
    PROOF_BYTES_GATE, PrefixMode, ProofFormat, RelationShape, SigmaParams, SigmaShape,
    SigmaVerifier, StateLayout, StepRelation, limb_bits_for,
};
use iroha_pasta::{Ep, Eq, Fp};
use iroha_plonk_gadgets::poseidon::ROWS_PER_PERMUTATION;

fn describe(label: &str, shape: &SigmaShape) -> usize {
    let inventory = shape.inventory::<Fp>().expect("inventory");
    let bytes = shape
        .proof_length::<Eq>(ProofFormat::KAGEMUSHA_STEP)
        .expect("proof length");
    let descriptor = shape
        .descriptor::<Eq>(ProofFormat::KAGEMUSHA_STEP)
        .expect("descriptor");
    println!(
        "SHAPE {label} case={} k={} lanes={} limb_bits={} permutations={:?} lane_rows={:?} \
         glue_rows={} range_rows={} cells={} advice_cols={} fixed_cols={} degree={} \
         equality_cols={} proof_bytes={bytes}",
        shape.params.relation().label(),
        shape.k,
        shape.params.lanes(),
        shape.params.limb_bits(),
        inventory.lane_permutations,
        inventory.lane_rows,
        inventory.glue_rows,
        inventory.range_rows,
        inventory.cells,
        descriptor.num_advice_columns,
        descriptor.num_fixed_columns,
        descriptor.degree,
        descriptor.permutation.len(),
    );
    assert_eq!(
        inventory.permutations(),
        shape.params.relation().permutations()
    );
    assert!(inventory.rows() < 1 << shape.k);
    assert_eq!(
        inventory.lane_rows.iter().sum::<usize>(),
        inventory.permutations() * ROWS_PER_PERMUTATION + inventory.glue_rows
    );
    assert_eq!(
        usize::from(descriptor.degree),
        6,
        "Pow5 lanes keep degree 6"
    );
    bytes
}

#[test]
fn selected_shapes_fit_and_meet_the_budget() {
    for relation in relation_shapes() {
        let smallest = smallest_shape(relation);
        let budget = budget_shape(relation);
        describe("smallest_k", &smallest);
        let bytes = describe("budget", &budget);
        assert!(bytes <= PROOF_BYTES_GATE, "{}: {bytes} B", relation.label());
        assert!(smallest.k <= budget.k);
        // Nothing smaller fits.
        if smallest.k > 9 {
            for lanes in 1..=iroha_kagemusha_proof::MAX_LANES {
                let params = SigmaParams::new(
                    relation,
                    lanes,
                    iroha_kagemusha_proof::limb_bits_for(smallest.k - 1),
                )
                .expect("params");
                assert!(
                    SigmaShape::new(params, smallest.k - 1)
                        .inventory::<Fp>()
                        .is_err(),
                    "{} fits k = {} with {lanes} lanes",
                    relation.label(),
                    smallest.k - 1
                );
            }
        }
    }
}

#[test]
fn two_level_steps_select_their_pinned_shapes() {
    for step in [StepRelation::Send, StepRelation::Receive] {
        let relation = RelationShape::new(step, StateLayout::TwoLevel, PrefixMode::Folded);
        let smallest = smallest_shape(relation);
        assert_eq!(
            (smallest.k, smallest.params.lanes()),
            TWO_LEVEL_SMALLEST,
            "{step:?}"
        );
        let budget = budget_shape(relation);
        assert_eq!(
            (budget.k, budget.params.lanes()),
            TWO_LEVEL_BUDGET,
            "{step:?}"
        );
        // The proof length does not depend on the curve.
        assert_eq!(
            budget.proof_length::<Eq>(ProofFormat::KAGEMUSHA_STEP),
            budget.proof_length::<Ep>(ProofFormat::KAGEMUSHA_STEP)
        );
        // At k = 11 the spec core needs two lanes (65 or 64 permutations do
        // not fit 2,048 rows), whose proof exceeds the budget.
        let one_lane = SigmaParams::new(relation, 1, limb_bits_for(11)).expect("params");
        assert!(SigmaShape::new(one_lane, 11).inventory::<Fp>().is_err());
        let two_lanes = SigmaParams::new(relation, 2, limb_bits_for(11)).expect("params");
        let bytes = SigmaShape::new(two_lanes, 11)
            .proof_length::<Eq>(ProofFormat::KAGEMUSHA_STEP)
            .expect("k = 11 with two lanes fits");
        println!(
            "SHAPE k11_two_lanes case={} proof_bytes={bytes}",
            relation.label()
        );
        assert!(bytes > PROOF_BYTES_GATE, "{bytes}");
    }
}

#[test]
fn verifiers_rebuild_from_descriptor_and_key_bytes() {
    let relation = RelationShape::new(
        StepRelation::Receive,
        StateLayout::TwoLevel,
        PrefixMode::Folded,
    );
    let shape = smallest_shape(relation);
    let prover = vesta_prover(shape);
    let descriptor = shape
        .descriptor::<Eq>(ProofFormat::KAGEMUSHA_STEP)
        .expect("descriptor");
    assert_eq!(prover.proving_key().binding().descriptor(), &descriptor);
    let verifier = prover.verifier();
    let rebuilt = SigmaVerifier::<Eq>::from_bytes(
        StepRelation::Receive,
        prover.params().clone(),
        verifier.descriptor_bytes(),
        verifier.vk_bytes(),
    )
    .expect("verifier from bytes");
    assert_eq!(rebuilt.vk_bytes(), verifier.vk_bytes());
    assert_eq!(rebuilt.binding(), verifier.binding());
    // Keys are deterministic.
    let again = vesta_prover(shape);
    assert_eq!(again.verifier().vk_bytes(), verifier.vk_bytes());
    // Parameters of another k are refused.
    assert!(
        SigmaVerifier::<Eq>::from_bytes(
            StepRelation::Receive,
            common::vesta_params(shape.k + 1),
            verifier.descriptor_bytes(),
            verifier.vk_bytes(),
        )
        .is_err()
    );
    let mut truncated = verifier.vk_bytes().to_vec();
    truncated.pop();
    assert!(
        SigmaVerifier::<Eq>::from_bytes(
            StepRelation::Receive,
            prover.params().clone(),
            verifier.descriptor_bytes(),
            &truncated,
        )
        .is_err()
    );
}

#[test]
fn commitment_tables_change_no_key_or_proof_byte() {
    use iroha_kagemusha_proof::{KeyOptions, Mutation, SigmaProver, sample_witness};

    let relation = RelationShape::new(
        StepRelation::Receive,
        StateLayout::TwoLevel,
        PrefixMode::Folded,
    );
    let shape = smallest_shape(relation);
    let plain = vesta_prover(shape);
    let tabled = SigmaProver::<Eq>::keygen_with_options(
        shape,
        ProofFormat::KAGEMUSHA_STEP,
        common::vesta_params(shape.k),
        KeyOptions::WITH_TABLES,
    )
    .expect("keys with tables");
    assert_eq!(KeyOptions::default().commitment_tables, None);
    assert_eq!(
        tabled.proving_key().commitment_tables().present(),
        (true, true)
    );
    assert_eq!(
        plain.proving_key().commitment_tables().present(),
        (false, false)
    );
    assert_eq!(tabled.verifier().vk_bytes(), plain.verifier().vk_bytes());
    let witness = sample_witness::<Fp>(3, StepRelation::Receive, Mutation::None);
    let with_tables = tabled
        .prove(&witness, common::recovery(4))
        .expect("proof with tables");
    let without = plain.prove(&witness, common::recovery(4)).expect("proof");
    assert_eq!(with_tables, without);
    assert_eq!(
        plain
            .verifier()
            .verify(&with_tables.public, &with_tables.bytes),
        Ok(())
    );
}
