use crate::{
    gates::{range::RangeInstructions, RangeChip},
    halo2_proofs::{arithmetic::Field, halo2curves::bn256::Fr},
    poseidon::hasher::{
        spec::OptimizedPoseidonSpec, PoseidonCompactChunkInput, PoseidonCompactInput,
        PoseidonHasher,
    },
    safe_types::SafeTypeChip,
    utils::{testing::base_test, ScalarField},
    Context,
};
use itertools::Itertools;
use pse_poseidon::Poseidon;
use rand::Rng;

#[derive(Clone)]
struct Payload<F: ScalarField> {
    // Represent value of a right-padded witness array with a variable length
    pub values: Vec<F>,
    // Length of `values`.
    pub len: usize,
}

// check if the results from hasher and native sponge are same for hash_var_len_array.
fn hasher_compatiblity_verification<
    const T: usize,
    const RATE: usize,
    const R_F: usize,
    const R_P: usize,
>(
    payloads: Vec<Payload<Fr>>,
) {
    base_test().k(12).run(|ctx, range| {
        // Construct in-circuit Poseidon hasher. Assuming SECURE_MDS = 0.
        let spec = OptimizedPoseidonSpec::<Fr, T, RATE>::new::<R_F, R_P, 0>();
        let mut hasher = PoseidonHasher::<Fr, T, RATE>::new(spec);
        hasher.initialize_consts(ctx, range.gate());

        for payload in payloads {
            // Construct native Poseidon sponge.
            let mut native_sponge = Poseidon::<Fr, T, RATE>::new(R_F, R_P);
            native_sponge.update(&payload.values[..payload.len]);
            let native_result = native_sponge.squeeze();
            let inputs = ctx.assign_witnesses(payload.values);
            let len = ctx.load_witness(Fr::from(payload.len as u64));
            let hasher_result = hasher.hash_var_len_array(ctx, range, &inputs, len);
            assert_eq!(native_result, *hasher_result.value());
        }
    });
}

// check if the results from hasher and native sponge are same for hash_compact_input.
fn hasher_compact_inputs_compatiblity_verification<
    const T: usize,
    const RATE: usize,
    const R_F: usize,
    const R_P: usize,
>(
    payloads: Vec<Payload<Fr>>,
    ctx: &mut Context<Fr>,
    range: &RangeChip<Fr>,
) {
    // Construct in-circuit Poseidon hasher. Assuming SECURE_MDS = 0.
    let spec = OptimizedPoseidonSpec::<Fr, T, RATE>::new::<R_F, R_P, 0>();
    let mut hasher = PoseidonHasher::<Fr, T, RATE>::new(spec);
    hasher.initialize_consts(ctx, range.gate());

    let mut native_results = Vec::with_capacity(payloads.len());
    let mut compact_inputs = Vec::<PoseidonCompactInput<Fr, RATE>>::new();
    let rate_witness = ctx.load_constant(Fr::from(RATE as u64));
    let true_witness = ctx.load_constant(Fr::ONE);
    let false_witness = ctx.load_zero();
    for payload in payloads {
        assert!(payload.values.len() % RATE == 0);
        assert!(payload.values.len() >= payload.len);
        assert!(payload.values.len() == RATE || payload.values.len() - payload.len < RATE);
        let num_chunk = payload.values.len() / RATE;
        let last_chunk_len = RATE - (payload.values.len() - payload.len);
        let inputs = ctx.assign_witnesses(payload.values.clone());
        for (chunk_idx, input_chunk) in inputs.chunks(RATE).enumerate() {
            let len_witness = if chunk_idx + 1 == num_chunk {
                ctx.load_witness(Fr::from(last_chunk_len as u64))
            } else {
                rate_witness
            };
            let is_final_witness = SafeTypeChip::unsafe_to_bool(if chunk_idx + 1 == num_chunk {
                true_witness
            } else {
                false_witness
            });
            compact_inputs.push(PoseidonCompactInput {
                inputs: input_chunk.try_into().unwrap(),
                len: len_witness,
                is_final: is_final_witness,
            });
        }
        // Construct native Poseidon sponge.
        let mut native_sponge = Poseidon::<Fr, T, RATE>::new(R_F, R_P);
        native_sponge.update(&payload.values[..payload.len]);
        let native_result = native_sponge.squeeze();
        native_results.push(native_result);
    }
    let compact_outputs = hasher.hash_compact_input(ctx, range.gate(), &compact_inputs);
    let mut output_offset = 0;
    for (compact_output, compact_input) in compact_outputs.iter().zip(compact_inputs) {
        // into() doesn't work if ! is in the beginning in the bool expression...
        let is_not_final_input: bool = compact_input.is_final.as_ref().value().is_zero().into();
        let is_not_final_output: bool = compact_output.is_final().as_ref().value().is_zero().into();
        assert_eq!(is_not_final_input, is_not_final_output);
        if !is_not_final_output {
            assert_eq!(native_results[output_offset], *compact_output.hash().value());
            output_offset += 1;
        }
    }
}

// check if the results from hasher and native sponge are same for hash_compact_input.
fn hasher_compact_chunk_inputs_compatiblity_verification<
    const T: usize,
    const RATE: usize,
    const R_F: usize,
    const R_P: usize,
>(
    payloads: Vec<(Payload<Fr>, bool)>,
    ctx: &mut Context<Fr>,
    range: &RangeChip<Fr>,
) {
    // Construct in-circuit Poseidon hasher. Assuming SECURE_MDS = 0.
    let spec = OptimizedPoseidonSpec::<Fr, T, RATE>::new::<R_F, R_P, 0>();
    let mut hasher = PoseidonHasher::<Fr, T, RATE>::new(spec);
    hasher.initialize_consts(ctx, range.gate());

    let mut native_results = Vec::with_capacity(payloads.len());
    let mut chunk_inputs = Vec::<PoseidonCompactChunkInput<Fr, RATE>>::new();
    let true_witness = SafeTypeChip::unsafe_to_bool(ctx.load_constant(Fr::ONE));
    let false_witness = SafeTypeChip::unsafe_to_bool(ctx.load_zero());

    // Construct native Poseidon sponge.
    let mut native_sponge = Poseidon::<Fr, T, RATE>::new(R_F, R_P);
    for (payload, is_final) in payloads {
        assert!(payload.values.len() == payload.len);
        assert!(payload.values.len() % RATE == 0);
        let inputs = ctx.assign_witnesses(payload.values.clone());

        let is_final_witness = if is_final { true_witness } else { false_witness };
        chunk_inputs.push(PoseidonCompactChunkInput {
            inputs: inputs.chunks(RATE).map(|c| c.try_into().unwrap()).collect_vec(),
            is_final: is_final_witness,
        });
        native_sponge.update(&payload.values);
        if is_final {
            let native_result = native_sponge.squeeze();
            native_results.push(native_result);
            native_sponge = Poseidon::<Fr, T, RATE>::new(R_F, R_P);
        }
    }
    let compact_outputs = hasher.hash_compact_chunk_inputs(ctx, range.gate(), &chunk_inputs);
    assert_eq!(chunk_inputs.len(), compact_outputs.len());
    let mut output_offset = 0;
    for (compact_output, chunk_input) in compact_outputs.iter().zip(chunk_inputs) {
        // into() doesn't work if ! is in the beginning in the bool expression...
        let is_final_input = chunk_input.is_final.as_ref().value();
        let is_final_output = compact_output.is_final.as_ref().value();
        assert_eq!(is_final_input, is_final_output);
        if is_final_output == &Fr::ONE {
            assert_eq!(native_results[output_offset], *compact_output.hash().value());
            output_offset += 1;
        }
    }
}

fn random_payload<F: ScalarField>(max_len: usize, len: usize, max_value: usize) -> Payload<F> {
    assert!(len <= max_len);
    let mut rng = rand::thread_rng();
    let mut values = Vec::new();
    for _ in 0..max_len {
        values.push(F::from(rng.gen_range(0..=max_value) as u64));
    }
    Payload { values, len }
}

fn random_payload_without_len<F: ScalarField>(max_len: usize, max_value: usize) -> Payload<F> {
    let mut rng = rand::thread_rng();
    let mut values = Vec::new();
    for _ in 0..max_len {
        values.push(F::from(rng.gen_range(0..=max_value) as u64));
    }
    Payload { values, len: rng.gen_range(0..=max_len) }
}

fn validate_compact_input_row(len: usize, is_final: bool, expect_satisfied: bool) {
    const RATE: usize = 2;

    base_test().k(10).lookup_bits(8).expect_satisfied(expect_satisfied).run(|ctx, range| {
        let safe = SafeTypeChip::new(range);
        let is_final = safe.load_bool(ctx, is_final);
        let len = ctx.load_witness(Fr::from(len as u64));
        let inputs = [ctx.load_witness(Fr::from(111u64)), ctx.load_witness(Fr::from(222u64))];
        let row = PoseidonCompactInput::<Fr, RATE>::new(inputs, is_final, len);

        row.add_validation_constraints(ctx, range);
    });
}

#[test]
fn test_poseidon_compact_input_validation_accepts_valid_rows() {
    validate_compact_input_row(2, false, true);
    validate_compact_input_row(1, true, true);
    validate_compact_input_row(0, true, true);
}

#[test]
fn test_poseidon_compact_input_validation_rejects_invalid_rows() {
    validate_compact_input_row(1, false, false);
    validate_compact_input_row(3, false, false);
}

#[test]
fn test_poseidon_hasher_compatiblity() {
    {
        const T: usize = 3;
        const RATE: usize = 2;
        let payloads = vec![
            // max_len = 0
            random_payload(0, 0, usize::MAX),
            // max_len % RATE == 0 && len = 0
            random_payload(RATE * 2, 0, usize::MAX),
            // max_len % RATE == 0 && 0 < len < max_len && len % RATE == 0
            random_payload(RATE * 2, RATE, usize::MAX),
            // max_len % RATE == 0 && 0 < len < max_len && len % RATE != 0
            random_payload(RATE * 5, RATE * 2 + 1, usize::MAX),
            // max_len % RATE == 0 && len == max_len
            random_payload(RATE * 2, RATE * 2, usize::MAX),
            random_payload(RATE * 5, RATE * 5, usize::MAX),
            // len % RATE != 0 && len = 0
            random_payload(RATE * 2 + 1, 0, usize::MAX),
            random_payload(RATE * 5 + 1, 0, usize::MAX),
            // len % RATE != 0 && 0 < len < max_len && len % RATE == 0
            random_payload(RATE * 2 + 1, RATE, usize::MAX),
            // len % RATE != 0 && 0 < len < max_len && len % RATE != 0
            random_payload(RATE * 5 + 1, RATE * 2 + 1, usize::MAX),
            // len % RATE != 0 && len = max_len
            random_payload(RATE * 2 + 1, RATE * 2 + 1, usize::MAX),
            random_payload(RATE * 5 + 1, RATE * 5 + 1, usize::MAX),
        ];
        hasher_compatiblity_verification::<T, RATE, 8, 57>(payloads);
    }
}

#[test]
fn test_poseidon_hasher_with_prover() {
    {
        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;

        let max_lens = vec![0, RATE * 2, RATE * 5, RATE * 2 + 1, RATE * 5 + 1];
        for max_len in max_lens {
            let init_input = random_payload_without_len(max_len, usize::MAX);
            let logic_input = random_payload_without_len(max_len, usize::MAX);
            base_test().k(12).bench_builder(init_input, logic_input, |pool, range, payload| {
                let ctx = pool.main();
                // Construct in-circuit Poseidon hasher. Assuming SECURE_MDS = 0.
                let spec = OptimizedPoseidonSpec::<Fr, T, RATE>::new::<R_F, R_P, 0>();
                let mut hasher = PoseidonHasher::<Fr, T, RATE>::new(spec);
                hasher.initialize_consts(ctx, range.gate());
                let inputs = ctx.assign_witnesses(payload.values);
                let len = ctx.load_witness(Fr::from(payload.len as u64));
                hasher.hash_var_len_array(ctx, range, &inputs, len);
            });
        }
    }
}

#[test]
fn test_poseidon_hasher_compact_inputs() {
    {
        const T: usize = 3;
        const RATE: usize = 2;
        let payloads = vec![
            // len == 0
            random_payload(RATE, 0, usize::MAX),
            // 0 < len < max_len
            random_payload(RATE * 2, RATE + 1, usize::MAX),
            random_payload(RATE * 5, RATE * 4 + 1, usize::MAX),
            // len == max_len
            random_payload(RATE * 2, RATE * 2, usize::MAX),
            random_payload(RATE * 5, RATE * 5, usize::MAX),
        ];
        base_test().k(12).run(|ctx, range| {
            hasher_compact_inputs_compatiblity_verification::<T, RATE, 8, 57>(payloads, ctx, range);
        });
    }
}

#[test]
fn test_poseidon_hasher_compact_inputs_with_prover() {
    {
        const T: usize = 3;
        const RATE: usize = 2;
        let params = [
            (RATE, 0),
            (RATE * 2, RATE + 1),
            (RATE * 5, RATE * 4 + 1),
            (RATE * 2, RATE * 2),
            (RATE * 5, RATE * 5),
        ];
        let init_payloads = params
            .iter()
            .map(|(max_len, len)| random_payload(*max_len, *len, usize::MAX))
            .collect::<Vec<_>>();
        let logic_payloads = params
            .iter()
            .map(|(max_len, len)| random_payload(*max_len, *len, usize::MAX))
            .collect::<Vec<_>>();
        base_test().k(12).bench_builder(init_payloads, logic_payloads, |pool, range, input| {
            let ctx = pool.main();
            hasher_compact_inputs_compatiblity_verification::<T, RATE, 8, 57>(input, ctx, range);
        });
    }
}

#[test]
fn test_poseidon_hasher_compact_chunk_inputs() {
    {
        const T: usize = 3;
        const RATE: usize = 2;
        let payloads = vec![
            (random_payload(RATE * 5, RATE * 5, usize::MAX), true),
            (random_payload(RATE, RATE, usize::MAX), false),
            (random_payload(RATE * 2, RATE * 2, usize::MAX), true),
            (random_payload(RATE * 3, RATE * 3, usize::MAX), true),
        ];
        base_test().k(12).run(|ctx, range| {
            hasher_compact_chunk_inputs_compatiblity_verification::<T, RATE, 8, 57>(
                payloads, ctx, range,
            );
        });
    }
    {
        const T: usize = 3;
        const RATE: usize = 2;
        let payloads = vec![
            (random_payload(0, 0, usize::MAX), true),
            (random_payload(0, 0, usize::MAX), false),
            (random_payload(0, 0, usize::MAX), false),
        ];
        base_test().k(12).run(|ctx, range| {
            hasher_compact_chunk_inputs_compatiblity_verification::<T, RATE, 8, 57>(
                payloads, ctx, range,
            );
        });
    }
}

#[test]
fn test_poseidon_hasher_compact_chunk_inputs_with_prover() {
    {
        const T: usize = 3;
        const RATE: usize = 2;
        let params = [
            (RATE, false),
            (RATE * 2, false),
            (RATE * 5, false),
            (RATE * 2, true),
            (RATE * 5, true),
        ];
        let init_payloads = params
            .iter()
            .map(|(len, is_final)| (random_payload(*len, *len, usize::MAX), *is_final))
            .collect::<Vec<_>>();
        let logic_payloads = params
            .iter()
            .map(|(len, is_final)| (random_payload(*len, *len, usize::MAX), *is_final))
            .collect::<Vec<_>>();
        base_test().k(12).bench_builder(init_payloads, logic_payloads, |pool, range, input| {
            let ctx = pool.main();
            hasher_compact_chunk_inputs_compatiblity_verification::<T, RATE, 8, 57>(
                input, ctx, range,
            );
        });
    }
}
