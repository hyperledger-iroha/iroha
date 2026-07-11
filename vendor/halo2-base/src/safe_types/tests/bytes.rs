use crate::{
    gates::{circuit::builder::RangeCircuitBuilder, RangeInstructions},
    halo2_proofs::{
        halo2curves::bn256::{Bn256, Fr},
        plonk::{keygen_pk, keygen_vk},
        poly::kzg::commitment::ParamsKZG,
    },
    safe_types::SafeTypeChip,
    utils::{
        testing::{base_test, check_proof, gen_proof},
        ScalarField,
    },
    Context,
};
use rand::rngs::OsRng;
use std::vec;
use test_case::test_case;

// =========== Utilies ===============
fn mock_circuit_test<FM: FnMut(&mut Context<Fr>, SafeTypeChip<'_, Fr>)>(mut f: FM) {
    base_test().k(10).lookup_bits(8).run(|ctx, range| {
        let safe = SafeTypeChip::new(range);
        f(ctx, safe);
    });
}

// =========== Mock Prover ===========

// Circuit Satisfied for valid inputs
#[test]
fn pos_var_len_bytes() {
    base_test().k(10).lookup_bits(8).run(|ctx, range| {
        let safe = SafeTypeChip::new(range);
        let bytes = ctx.assign_witnesses(
            vec![255u64, 255u64, 255u64, 255u64].into_iter().map(Fr::from).collect::<Vec<_>>(),
        );
        let len = ctx.load_witness(Fr::from(3u64));
        safe.raw_to_var_len_bytes::<4>(ctx, bytes.clone().try_into().unwrap(), len);

        // check edge case len == MAX_LEN
        let len = ctx.load_witness(Fr::from(4u64));
        safe.raw_to_var_len_bytes::<4>(ctx, bytes.try_into().unwrap(), len);
    });
}

#[test_case(vec![1,2,3], 4 => vec![0,1,2,3]; "pos left pad 3 to 4")]
#[test_case(vec![1,2,3], 5 => vec![0,0,1,2,3]; "pos left pad 3 to 5")]
#[test_case(vec![1,2,3], 6 => vec![0,0,0,1,2,3]; "pos left pad 3 to 6")]
fn left_pad_var_len_bytes(mut bytes: Vec<u8>, max_len: usize) -> Vec<u8> {
    base_test().k(10).lookup_bits(8).run(|ctx, range| {
        let safe = SafeTypeChip::new(range);
        let len = bytes.len();
        bytes.resize(max_len, 0);
        let bytes = ctx.assign_witnesses(bytes.into_iter().map(|b| Fr::from(b as u64)));
        let len = ctx.load_witness(Fr::from(len as u64));
        let bytes = safe.raw_to_var_len_bytes_vec(ctx, bytes, len, max_len);
        let padded = bytes.left_pad_to_fixed(ctx, range.gate());
        padded.bytes().iter().map(|b| b.as_ref().value().get_lower_64() as u8).collect()
    })
}

// Checks circuit is unsatisfied for AssignedValue<F>'s are not in range 0..256
#[test]
#[should_panic(expected = "circuit was not satisfied")]
fn neg_var_len_bytes_witness_values_not_bytes() {
    mock_circuit_test(|ctx: &mut Context<Fr>, safe: SafeTypeChip<'_, Fr>| {
        let len = ctx.load_witness(Fr::from(3u64));
        let fake_bytes = ctx.assign_witnesses(
            vec![500u64, 500u64, 500u64, 500u64].into_iter().map(Fr::from).collect::<Vec<_>>(),
        );
        safe.raw_to_var_len_bytes::<4>(ctx, fake_bytes.try_into().unwrap(), len);
    });
}

// Checks assertion len <= max_len
#[test]
#[should_panic]
fn neg_var_len_bytes_len_less_than_max_len() {
    mock_circuit_test(|ctx: &mut Context<Fr>, safe: SafeTypeChip<'_, Fr>| {
        let len = ctx.load_witness(Fr::from(5u64));
        let fake_bytes = ctx.assign_witnesses(
            vec![500u64, 500u64, 500u64, 500u64].into_iter().map(Fr::from).collect::<Vec<_>>(),
        );
        safe.raw_to_var_len_bytes::<4>(ctx, fake_bytes.try_into().unwrap(), len);
    });
}

// Circuit Satisfied for valid inputs
#[test]
fn pos_var_len_bytes_vec() {
    base_test().k(10).lookup_bits(8).run(|ctx, range| {
        let safe = SafeTypeChip::new(range);
        let bytes = ctx.assign_witnesses(
            vec![255u64, 255u64, 255u64, 255u64].into_iter().map(Fr::from).collect::<Vec<_>>(),
        );
        let len = ctx.load_witness(Fr::from(3u64));
        safe.raw_to_var_len_bytes_vec(ctx, bytes.clone(), len, 4);

        // check edge case len == MAX_LEN
        let len = ctx.load_witness(Fr::from(4u64));
        safe.raw_to_var_len_bytes_vec(ctx, bytes, len, 4);
    });
}

// Checks circuit is unsatisfied for AssignedValue<F>'s are not in range 0..256
#[test]
#[should_panic(expected = "circuit was not satisfied")]
fn neg_var_len_bytes_vec_witness_values_not_bytes() {
    mock_circuit_test(|ctx: &mut Context<Fr>, safe: SafeTypeChip<'_, Fr>| {
        let len = ctx.load_witness(Fr::from(3u64));
        let fake_bytes = ctx.assign_witnesses(
            vec![500u64, 500u64, 500u64, 500u64].into_iter().map(Fr::from).collect::<Vec<_>>(),
        );
        let max_len = fake_bytes.len();
        safe.raw_to_var_len_bytes_vec(ctx, fake_bytes, len, max_len);
    });
}

// Checks assertion len <= max_len
#[test]
#[should_panic]
fn neg_var_len_bytes_vec_len_less_than_max_len() {
    mock_circuit_test(|ctx: &mut Context<Fr>, safe: SafeTypeChip<'_, Fr>| {
        let len = ctx.load_witness(Fr::from(5u64));
        let fake_bytes = ctx.assign_witnesses(
            vec![500u64, 500u64, 500u64, 500u64].into_iter().map(Fr::from).collect::<Vec<_>>(),
        );
        let max_len = 4;
        safe.raw_to_var_len_bytes_vec(ctx, fake_bytes, len, max_len);
    });
}

// Circuit Satisfied for valid inputs
#[test]
fn pos_fix_len_bytes() {
    base_test().k(10).lookup_bits(8).run(|ctx, range| {
        let safe = SafeTypeChip::new(range);
        let fake_bytes = ctx.assign_witnesses(
            vec![255u64, 255u64, 255u64, 255u64].into_iter().map(Fr::from).collect::<Vec<_>>(),
        );
        safe.raw_to_fix_len_bytes::<4>(ctx, fake_bytes.try_into().unwrap());
    });
}

// Assert inputs.len() == len
#[test]
#[should_panic]
fn neg_fix_len_bytes_vec() {
    base_test().k(10).lookup_bits(8).run(|ctx, range| {
        let safe = SafeTypeChip::new(range);
        let fake_bytes = ctx.assign_witnesses(
            vec![255u64, 255u64, 255u64, 255u64].into_iter().map(Fr::from).collect::<Vec<_>>(),
        );
        safe.raw_to_fix_len_bytes_vec(ctx, fake_bytes, 5);
    });
}

// Circuit Satisfied for valid inputs
#[test]
fn pos_fix_len_bytes_vec() {
    base_test().k(10).lookup_bits(8).run(|ctx, range| {
        let safe = SafeTypeChip::new(range);
        let fake_bytes = ctx.assign_witnesses(
            vec![255u64, 255u64, 255u64, 255u64].into_iter().map(Fr::from).collect::<Vec<_>>(),
        );
        safe.raw_to_fix_len_bytes_vec(ctx, fake_bytes, 4);
    });
}

// =========== Prover ===========
#[test]
fn pos_prover_satisfied() {
    const KEYGEN_MAX_LEN: usize = 4;
    const PROVER_MAX_LEN: usize = 4;
    let keygen_inputs = (vec![1u64, 2u64, 3u64, 4u64], 3);
    let proof_inputs = (vec![1u64, 2u64, 3u64, 4u64], 3);
    prover_satisfied::<KEYGEN_MAX_LEN, PROVER_MAX_LEN>(keygen_inputs, proof_inputs);
}

#[test]
fn pos_diff_len_same_max_len() {
    const KEYGEN_MAX_LEN: usize = 4;
    const PROVER_MAX_LEN: usize = 4;
    let keygen_inputs = (vec![1u64, 2u64, 3u64, 4u64], 3);
    let proof_inputs = (vec![1u64, 2u64, 3u64, 4u64], 2);
    prover_satisfied::<KEYGEN_MAX_LEN, PROVER_MAX_LEN>(keygen_inputs, proof_inputs);
}

#[test]
#[should_panic]
fn neg_different_proof_max_len() {
    const KEYGEN_MAX_LEN: usize = 4;
    const PROVER_MAX_LEN: usize = 3;
    let keygen_inputs = (vec![1u64, 2u64, 3u64, 4u64], 4);
    let proof_inputs = (vec![1u64, 2u64, 3u64], 3);
    prover_satisfied::<KEYGEN_MAX_LEN, PROVER_MAX_LEN>(keygen_inputs, proof_inputs);
}

// test circuit
fn var_byte_array_circuit<const MAX_LEN: usize>(
    k: usize,
    witness_gen_only: bool,
    (bytes, len): (Vec<u64>, usize),
) -> RangeCircuitBuilder<Fr> {
    let lookup_bits = 3;
    let mut builder =
        RangeCircuitBuilder::new(witness_gen_only).use_k(k).use_lookup_bits(lookup_bits);
    let range = builder.range_chip();
    let safe = SafeTypeChip::new(&range);
    let ctx = builder.main(0);
    let len = ctx.load_witness(Fr::from(len as u64));
    let fake_bytes = ctx.assign_witnesses(bytes.into_iter().map(Fr::from).collect::<Vec<_>>());
    safe.raw_to_var_len_bytes::<MAX_LEN>(ctx, fake_bytes.try_into().unwrap(), len);
    builder.calculate_params(Some(9));
    builder
}

// Prover test
fn prover_satisfied<const KEYGEN_MAX_LEN: usize, const PROVER_MAX_LEN: usize>(
    keygen_inputs: (Vec<u64>, usize),
    proof_inputs: (Vec<u64>, usize),
) {
    let k = 11;
    let rng = OsRng;
    let params = ParamsKZG::<Bn256>::setup(k as u32, rng);
    let keygen_circuit = var_byte_array_circuit::<KEYGEN_MAX_LEN>(k, false, keygen_inputs);
    let vk = keygen_vk(&params, &keygen_circuit).unwrap();
    let pk = keygen_pk(&params, vk.clone(), &keygen_circuit).unwrap();
    let break_points = keygen_circuit.break_points();

    let mut proof_circuit = var_byte_array_circuit::<PROVER_MAX_LEN>(k, true, proof_inputs);
    proof_circuit.set_break_points(break_points);
    let proof = gen_proof(&params, &pk, proof_circuit);
    check_proof(&params, &vk, &proof[..], true);
}
