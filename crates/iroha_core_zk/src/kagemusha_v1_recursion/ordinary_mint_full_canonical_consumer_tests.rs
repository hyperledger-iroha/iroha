//! Strict original transcript/history topology regressions, with no recursive proof grant.
use super::*;
use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
};

fn original_widths<F: KagemushaPoseidonFieldV1>() {
    for parity in [KagemushaPastaParityV1::Eq, KagemushaPastaParityV1::Ep] {
        let mut b = BaseCircuitBuilder::<F>::new(false)
            .use_k(10)
            .use_instance_columns(1);
        let range = b.range_chip();
        let ctx = b.main(0);
        let current_bytes = if parity == KagemushaPastaParityV1::Eq {
            vec![2; 32]
        } else {
            vec![3; 64]
        };
        let assigned = assign_bytes(ctx, &range, &current_bytes);
        let [eq, ep] =
            proof_streams(ctx, &range, parity, &assigned, &[2; 32], &[3; 64], [32, 64]).unwrap();
        assert_eq!(eq.bytes().len(), 32);
        assert_eq!(ep.bytes().len(), 64);
        assert!(
            proof_streams(ctx, &range, parity, &assigned, &[2; 31], &[3; 64], [32, 64]).is_err()
        );
        assert!(
            proof_streams(ctx, &range, parity, &assigned, &[2; 32], &[3; 65], [32, 64]).is_err()
        );
        assert!(
            proof_streams(
                ctx,
                &range,
                parity,
                &assigned[..assigned.len() - 1],
                &[2; 32],
                &[3; 64],
                [32, 64]
            )
            .is_err()
        );
        b.assigned_instances = vec![Vec::new()];
        b.calculate_params(Some(9));
        MockProver::run(10, &b, vec![Vec::new()])
            .unwrap()
            .assert_satisfied();
    }
}
fn full_history<F: KagemushaPoseidonFieldV1>() {
    let mut b = BaseCircuitBuilder::<F>::new(false)
        .use_k(11)
        .use_instance_columns(1);
    let range = b.range_chip();
    let ctx = b.main(0);
    let original = [0x71; 544];
    let fields = original
        .chunks_exact(16)
        .map(|chunk| {
            let limb = u128::from_le_bytes(chunk.try_into().unwrap());
            ctx.load_witness(crate::kagemusha_v1_poseidon::from_u128::<F>(limb))
        })
        .collect::<Vec<_>>();
    let stream = history(ctx, &range, &fields, 0).unwrap();
    let expected = assign_bytes(ctx, &range, &original);
    for (actual, expected) in stream.bytes().iter().zip(expected) {
        let diff = range
            .gate()
            .sub(ctx, actual.quantum_cell(), expected.quantum_cell());
        range.gate().assert_is_const(ctx, &diff, &F::ZERO);
    }
    assert!(history_original(ctx, &range, &original[..543]).is_err());
    assert!(history_original(ctx, &range, &[0; 545]).is_err());
    assert!(history(ctx, &range, &fields[..33], 0).is_err());
    b.assigned_instances = vec![Vec::new()];
    b.calculate_params(Some(10));
    MockProver::run(11, &b, vec![Vec::new()])
        .unwrap()
        .assert_satisfied();
}
#[test]
fn original_proof_transcripts_preserve_both_installed_parity_widths() {
    original_widths::<Fp>();
    original_widths::<Fq>();
}
#[test]
fn canonical_histories_use_all_34_full_u128_limbs_and_refuse_truncation() {
    full_history::<Fp>();
    full_history::<Fq>();
}
