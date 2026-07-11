use crate::ff::Field;
use crate::gates::circuit::{builder::RangeCircuitBuilder, CircuitBuilderStage};
use crate::{
    gates::{GateChip, GateInstructions},
    halo2_proofs::{
        halo2curves::bn256::Fr,
        plonk::keygen_pk,
        plonk::{keygen_vk, Assigned},
        poly::kzg::commitment::ParamsKZG,
    },
    utils::testing::{check_proof, gen_proof},
    QuantumCell::Witness,
};
use itertools::Itertools;
use rand::{rngs::OsRng, thread_rng, Rng};
use test_log::test;

// soundness checks for `idx_to_indicator` function
fn test_idx_to_indicator_gen(k: u32, len: usize) {
    // first create proving and verifying key
    let mut builder =
        RangeCircuitBuilder::from_stage(CircuitBuilderStage::Keygen).use_k(k as usize);
    let gate = GateChip::default();
    let dummy_idx = Witness(Fr::zero());
    let indicator = gate.idx_to_indicator(builder.main(0), dummy_idx, len);
    // get the offsets of the indicator cells for later 'pranking'
    let ind_offsets = indicator.iter().map(|ind| ind.cell.unwrap().offset).collect::<Vec<_>>();
    let config_params = builder.calculate_params(Some(9));

    let params = ParamsKZG::setup(k, OsRng);
    // generate proving key
    let vk = keygen_vk(&params, &builder).unwrap();
    let pk = keygen_pk(&params, vk, &builder).unwrap();
    let vk = pk.get_vk(); // pk consumed vk
    let break_points = builder.break_points();
    drop(builder);

    // now create different proofs to test the soundness of the circuit

    let gen_pf = |idx: usize, ind_witnesses: &[Fr]| {
        let mut builder = RangeCircuitBuilder::prover(config_params.clone(), break_points.clone());
        let gate = GateChip::default();
        let idx = Witness(Fr::from(idx as u64));
        let ctx = builder.main(0);
        gate.idx_to_indicator(ctx, idx, len);
        // prank the indicator cells
        for (offset, witness) in ind_offsets.iter().zip_eq(ind_witnesses) {
            ctx.advice[*offset] = Assigned::Trivial(*witness);
        }
        gen_proof(&params, &pk, builder)
    };

    // expected answer
    for idx in 0..len {
        let mut ind_witnesses = vec![Fr::zero(); len];
        ind_witnesses[idx] = Fr::one();
        let pf = gen_pf(idx, &ind_witnesses);
        check_proof(&params, vk, &pf, true);
    }

    let mut rng = thread_rng();
    // bad cases
    for idx in 0..len {
        let mut ind_witnesses = vec![Fr::zero(); len];
        // all zeros is bad!
        let pf = gen_pf(idx, &ind_witnesses);
        check_proof(&params, vk, &pf, false);

        // ind[idx] != 1 is bad!
        for _ in 0..100usize {
            ind_witnesses.fill(Fr::zero());
            ind_witnesses[idx] = Fr::random(OsRng);
            if ind_witnesses[idx] == Fr::one() {
                continue;
            }
            let pf = gen_pf(idx, &ind_witnesses);
            check_proof(&params, vk, &pf, false);
        }

        if len < 2 {
            continue;
        }
        // nonzeros where there should be zeros is bad!
        for _ in 0..100usize {
            ind_witnesses.fill(Fr::zero());
            ind_witnesses[idx] = Fr::one();
            let num_nonzeros = rng.gen_range(1..len);
            let mut count = 0usize;
            for _ in 0..num_nonzeros {
                let index = rng.gen_range(0..len);
                if index == idx {
                    continue;
                }
                ind_witnesses[index] = Fr::random(&mut rng);
                count += 1;
            }
            if count == 0usize {
                continue;
            }
            let pf = gen_pf(idx, &ind_witnesses);
            check_proof(&params, vk, &pf, false);
        }
    }
}

#[test]
fn test_idx_to_indicator() {
    test_idx_to_indicator_gen(8, 1);
    test_idx_to_indicator_gen(8, 4);
    test_idx_to_indicator_gen(8, 10);
    test_idx_to_indicator_gen(8, 20);
}

#[test]
#[ignore = "takes too long"]
fn test_idx_to_indicator_large() {
    test_idx_to_indicator_gen(11, 100);
}
