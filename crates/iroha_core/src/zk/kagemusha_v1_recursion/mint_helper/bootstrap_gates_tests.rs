//! Constraint-only regressions for unsigned bootstrap messages in both Pasta fields.

use halo2_base::gates::{GateChip, GateInstructions as _, circuit::builder::BaseCircuitBuilder};
use halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
};

use super::{KagemushaPoseidonFieldV1, constrain_bootstrap_message_gates};

const TEST_K: u32 = 6;

// Deliberately assign raw witnesses and invoke the same helper as production. No model
// validator, certificate preflight, or valid-message constructor can reject a negative case
// before the circuit is synthesized. Public cells bind the exact witnesses under test.
fn gate_case<F: KagemushaPoseidonFieldV1>(values: [u64; 4]) -> bool {
    let mut builder = BaseCircuitBuilder::<F>::default()
        .use_k(TEST_K as usize)
        .use_instance_columns(1);
    let gate = GateChip::<F>::default();
    let ctx = builder.main(0);
    let cells = values.map(|value| ctx.load_witness(F::from(value)));
    let [bootstrap, top_up_count, block_height, next_epoch_present] = cells;
    gate.assert_bit(ctx, bootstrap);
    gate.assert_bit(ctx, next_epoch_present);
    constrain_bootstrap_message_gates(
        ctx,
        &gate,
        bootstrap,
        top_up_count,
        block_height,
        next_epoch_present,
    );
    builder.assigned_instances = vec![cells.to_vec()];
    builder.calculate_params(Some(9));
    MockProver::run(TEST_K, &builder, vec![values.map(F::from).to_vec()])
        .expect("tiny bootstrap gate circuit synthesizes")
        .verify()
        .is_ok()
}

#[test]
fn bootstrap_gates_accept_zero_count_initial_height_and_no_successor_in_both_fields() {
    assert!(gate_case::<Fp>([1, 0, 1, 0]));
    assert!(gate_case::<Fq>([1, 0, 1, 0]));
}

#[test]
fn bootstrap_gates_reject_each_forbidden_witness_in_both_fields() {
    for values in [
        [1, 1, 1, 0],
        [1, u64::from(u32::MAX), 1, 0],
        [1, 0, 0, 0],
        [1, 0, 2, 0],
        [1, 0, u64::MAX, 0],
        [1, 0, 1, 1],
        [1, 1, 2, 1],
    ] {
        assert!(!gate_case::<Fp>(values), "Fp accepted {values:?}");
        assert!(!gate_case::<Fq>(values), "Fq accepted {values:?}");
    }
}

#[test]
fn bootstrap_gates_are_conditional_in_both_fields() {
    // The complete Rotate/FinalizedMint relations own their additional rules. This fixture
    // proves that only Bootstrap imposes zero count, height one and an absent successor.
    for values in [
        [0, 0, 1, 0],
        [0, 7, 2, 1],
        [0, u64::from(u32::MAX), u64::MAX, 1],
    ] {
        assert!(gate_case::<Fp>(values), "Fp rejected {values:?}");
        assert!(gate_case::<Fq>(values), "Fq rejected {values:?}");
    }
}
