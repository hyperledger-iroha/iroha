use super::*;

use halo2_base::halo2_proofs::{
    circuit::SimpleFloorPlanner,
    dev::MockProver,
    halo2curves::{
        ff::Field as _,
        pasta::{Fp, Fq},
    },
    plonk::{Circuit, ConstraintSystem, Error},
};
use halo2_base::utils::modulus;
use sha2::{Digest, Sha256};

use crate::zk::pasta_ipa_recursion::{
    PastaIpaInstanceQueryV1, pasta_ipa_augmented_proof_shape_v1,
    pasta_ipa_direct_instance_compile_config_v1,
};

#[derive(Clone, Debug)]
struct TestVector {
    sec1: [u8; 65],
    digest: [u8; 32],
    signature: [u8; 64],
}

fn decode_hex<const N: usize>(encoded: &str) -> [u8; N] {
    hex::decode(encoded)
        .expect("fixture is hexadecimal")
        .try_into()
        .unwrap_or_else(|_| panic!("fixture has exactly {N} bytes"))
}

fn rfc6979_sample() -> TestVector {
    let x = decode_hex::<32>("60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6");
    let y = decode_hex::<32>("7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299");
    let digest =
        decode_hex::<32>("AF2BDBE1AA9B6EC1E2ADE1D694F41FC71A831D0268E9891562113D8A62ADD1BF");
    let r = decode_hex::<32>("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716");
    let low_s =
        decode_hex::<32>("0834E36AD29A83BF2BC9385E491D6099C8FDF9D1ED67AA7EA5F51F93782857A9");
    let mut sec1 = [0_u8; 65];
    sec1[0] = 4;
    sec1[1..33].copy_from_slice(&x);
    sec1[33..].copy_from_slice(&y);
    let mut signature = [0_u8; 64];
    signature[..32].copy_from_slice(&r);
    signature[32..].copy_from_slice(&low_s);
    TestVector {
        sec1,
        digest,
        signature,
    }
}

#[test]
fn configured_shape_is_exactly_current_query_and_3200_augmented_bytes() {
    let mut meta = ConstraintSystem::<Fp>::default();
    let _ = P256AffineCompactConfigV1::configure(&mut meta);
    assert_eq!(meta.degree(), P256_AFFINE_COMPACT_SHAPE_V1.degree);
    assert_eq!(meta.num_advice_columns(), 8);
    assert_eq!(meta.num_instance_columns(), 1);
    assert_eq!(meta.num_fixed_columns(), 4);
    assert_eq!(meta.num_selectors(), 0);
    assert_eq!(meta.advice_queries().len(), 8);
    assert_eq!(meta.instance_queries().len(), 1);
    assert_eq!(meta.fixed_queries().len(), 4);
    assert!(
        meta.advice_queries()
            .iter()
            .all(|(_, rotation)| *rotation == Rotation::cur())
    );
    assert!(
        meta.instance_queries()
            .iter()
            .all(|(_, rotation)| *rotation == Rotation::cur())
    );
    assert!(
        meta.fixed_queries()
            .iter()
            .all(|(_, rotation)| *rotation == Rotation::cur())
    );
    assert_eq!(meta.permutation().get_columns().len(), 8);
    assert_eq!(meta.lookups().len(), 2);
    assert_eq!(P256_AFFINE_COMPACT_SHAPE_V1.permutation_chunks, 2);
    assert_eq!(P256_AFFINE_COMPACT_SHAPE_V1.proof_points, 57);
    assert_eq!(P256_AFFINE_COMPACT_SHAPE_V1.proof_scalars, 42);
    assert_eq!(P256_AFFINE_COMPACT_SHAPE_V1.raw_proof_bytes, 3_168);
    assert_eq!(P256_AFFINE_COMPACT_SHAPE_V1.augmented_proof_bytes, 3_200);

    let shared = pasta_ipa_augmented_proof_shape_v1(&meta, K, PastaIpaInstanceQueryV1::Direct)
        .expect("shared direct-instance shape accounting");
    assert_eq!(shared.commitments(), 57);
    assert_eq!(shared.evaluations(), 42);
    assert_eq!(shared.point_sets(), 4);
    assert_eq!(shared.transcript_elements(), 100);
    assert_eq!(shared.augmented_proof_bytes(), 3_200);
    assert_eq!(
        usize::try_from(shared.augmented_proof_bytes()).expect("shape fits usize"),
        P256_AFFINE_COMPACT_SHAPE_V1.augmented_proof_bytes
    );
}

fn constant_tail_digest<F: BigPrimeField>(tail: &[F]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"iroha.offline_cash.p256_affine.constant_tail.v1");
    hasher.update(
        u64::try_from(tail.len())
            .expect("tail length fits u64")
            .to_le_bytes(),
    );
    for value in tail {
        hasher.update(value.to_repr().as_ref());
    }
    hasher.finalize().into()
}

#[test]
fn full_instance_contract_derives_caller_bytes_and_constant_tail() {
    let vector = rfc6979_sample();
    fn assert_contract<F: BigPrimeField>(vector: &TestVector) {
        let circuit =
            P256AffineCompactEcdsaCircuitV1::<F>::new(vector.sec1, vector.digest, vector.signature);
        let (caller, constant_tail) = circuit
            .instance_contract_for_test()
            .expect("pre-cap instance contract");
        assert_eq!(caller.len(), P256_AFFINE_COMPACT_CALLER_INSTANCES_V1);
        assert_eq!(
            caller,
            circuit
                .input_bytes()
                .into_iter()
                .map(|byte| F::from(u64::from(byte)))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            constant_tail.len(),
            P256_AFFINE_COMPACT_CONSTANT_TAIL_INSTANCES_V1
        );
        assert_eq!(
            caller.len() + constant_tail.len(),
            P256_AFFINE_COMPACT_TOTAL_INSTANCES_V1
        );
        assert_eq!(
            constant_tail_digest(&constant_tail),
            P256_AFFINE_COMPACT_CONSTANT_TAIL_DIGEST_V1
        );
        let direct_config =
            pasta_ipa_direct_instance_compile_config_v1(caller.len() + constant_tail.len());
        assert!(format!("{direct_config:?}").contains("query_instance: false"));
    }

    assert_contract::<Fp>(&vector);
    assert_contract::<Fq>(&vector);
}

#[test]
fn typed_range_table_pairs_are_exact_and_complete() {
    assert_eq!(RANGE_CHUNK_BITS, [2, 4, 6, 8, 11, 15]);
    assert_eq!(
        RANGE_CHUNK_BITS
            .into_iter()
            .map(|bits| 1_usize << bits)
            .sum::<usize>(),
        TABLE_ROWS
    );
    let pairs = RANGE_CHUNK_BITS
        .into_iter()
        .flat_map(|bits| (0_u64..(1_u64 << bits)).map(move |value| (bits as u64, value)))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(pairs.len(), TABLE_ROWS);
    for bits in RANGE_CHUNK_BITS {
        assert!(pairs.contains(&(bits as u64, 0)));
        assert!(pairs.contains(&(bits as u64, (1_u64 << bits) - 1)));
        assert!(!pairs.contains(&(bits as u64, 1_u64 << bits)));
    }
    assert!(pairs.contains(&(15, 1_u64 << 11)));
    assert!(!pairs.contains(&(11, 1_u64 << 11)));
}

#[derive(Clone, Debug)]
struct TraceProbeCircuit<F> {
    trace: AffineTrace<F>,
}

impl<F: BigPrimeField> Circuit<F> for TraceProbeCircuit<F> {
    type Config = P256AffineCompactConfigV1;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        P256AffineCompactConfigV1::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.trace.assign(&config, &mut layouter)
    }
}

fn assert_trace_satisfied(trace: AffineTrace<Fp>) {
    let instances = trace.instances.clone();
    MockProver::run(K, &TraceProbeCircuit { trace }, vec![instances])
        .expect("probe synthesis")
        .assert_satisfied();
}

fn assert_trace_rejected(trace: AffineTrace<Fp>) {
    let instances = trace.instances.clone();
    assert!(
        MockProver::run(K, &TraceProbeCircuit { trace }, vec![instances])
            .expect("probe synthesis")
            .verify()
            .is_err()
    );
}

fn range_probe(value: Fp, bits: usize) -> AffineTrace<Fp> {
    let mut builder = TraceBuilder::<Fp>::new();
    let zero = builder.zero();
    let one = builder.one();
    let chunk = builder.witness_fe(value);
    builder.ranges.push(RangeRelation {
        gate: FmaRelation {
            cells: [zero, chunk, one, chunk],
        },
        bits,
    });
    builder.finish().expect("one-row range probe")
}

#[test]
fn verifier_bound_boolean_constant_rejects_mutated_instance_tail() {
    let mut builder = TraceBuilder::<Fp>::new();
    let truth = builder.constant_bool(true);
    let one = builder.one();
    builder.assert_equal(truth.cell(), one);
    let trace = builder.finish().expect("constant Boolean probe");
    assert_trace_satisfied(trace.clone());

    let mut mutated_instances = trace.instances.clone();
    let truth_index = mutated_instances
        .iter()
        .position(|value| *value == Fp::ONE)
        .expect("true constant is in the verifier-derived tail");
    mutated_instances[truth_index] = Fp::ZERO;
    assert!(
        MockProver::run(K, &TraceProbeCircuit { trace }, vec![mutated_instances])
            .expect("mutated constant-tail probe synthesis")
            .verify()
            .is_err()
    );
}

#[test]
fn zero_kind_neutralizes_both_lookups_on_non_range_rows() {
    let mut builder = TraceBuilder::<Fp>::new();
    for values in [[1_u8, 2, 3], [4, 5, 6]] {
        let add = builder.constant(values[0]);
        let left = builder.constant(values[1]);
        let right = builder.constant(values[2]);
        let _output = builder.fma(add, left, right);
    }
    assert_trace_satisfied(builder.finish().expect("non-range FMA probe"));
}

#[test]
fn typed_partial_range_rejects_cap_plus_one_and_fractional_forgery_witnesses() {
    assert_trace_satisfied(range_probe(Fp::from((1_u64 << 11) - 1), 11));
    assert_trace_rejected(range_probe(Fp::from(1_u64 << 11), 11));
    let old_scale_inverse = Fp::from(1_u64 << (LOOKUP_BITS - 11)).invert().unwrap();
    assert_trace_rejected(range_probe(old_scale_inverse, 11));

    // This forged value matched another kind in the retracted one-column
    // encoding. The tuple lookup fixes the kind before checking the raw chunk.
    let offset = Fp::from(1_u64 << LOOKUP_BITS);
    let cross_kind_entry = Fp::from(15_u64) * (offset + Fp::from(123_u64));
    let cross_kind_fraction = cross_kind_entry * Fp::from(11_u64).invert().unwrap() - offset;
    assert_trace_rejected(range_probe(cross_kind_fraction, 11));
}

#[test]
fn exact_signed_quotient_carry_and_lift_bounds_fit() {
    let p = modulus_base();
    assert!((3_u8 * &p).bits() <= QUOTIENT_BITS as u64);
    let radix = radix();
    assert!((13_u8 * &radix).bits() <= CARRY_BITS as u64);
    assert!((BigUint::from(1_u8) << 177) < modulus::<Fp>());
    assert!((BigUint::from(1_u8) << 177) < modulus::<Fq>());
}

#[test]
fn host_complete_affine_cases_cover_identity_double_inverse_and_unequal() {
    let identity = AffineValue::identity();
    let generator = AffineValue::generator();
    let double = affine_double_value(&generator).0;
    assert_eq!(affine_add_value(&identity, &identity).0, identity);
    assert_eq!(affine_add_value(&identity, &generator).0, generator);
    assert_eq!(affine_add_value(&generator, &identity).0, generator);
    assert_eq!(affine_add_value(&generator, &generator).0, double);
    let inverse = AffineValue {
        x: generator.x.clone(),
        y: modular_sub(&BigUint::from(0_u8), &generator.y, &modulus_base()),
        infinity: false,
    };
    assert_eq!(affine_add_value(&generator, &inverse).0, identity);
    assert_eq!(
        affine_add_value(&generator, &double).0,
        fixed_generator_values()[3]
    );
}

#[test]
fn fixed_generator_staged_selection_covers_all_six_limbs_and_sixteen_digits() {
    let values = fixed_generator_values();
    let mut builder = TraceBuilder::<Fp>::new();
    for digit in 0_usize..16 {
        let bits: [BoolVar<Fp>; 4] =
            std::array::from_fn(|bit| builder.boolean(((digit >> bit) & 1) == 1));
        let point = select_fixed_window(&mut builder, &bits).expect("fixed staged selection");
        assert_eq!(point.value, values[digit]);
        for (actual, expected) in point
            .x
            .limbs
            .iter()
            .zip(decompose_limbs(&values[digit].x))
            .chain(point.y.limbs.iter().zip(decompose_limbs(&values[digit].y)))
        {
            let expected = builder.constant(expected);
            builder.assert_equal(actual.cell, expected);
        }
        let expected_identity = builder.constant_bool(digit == 0);
        builder.assert_equal(point.infinity.cell(), expected_identity.cell());
    }
    let trace = builder.finish().expect("all fixed digits fit k=16");
    assert_eq!(trace.rows.select_relations, 16 * 6 * 15);
    assert!(trace.rows.total_rows <= K16_MAX_ASSIGNED_ROWS);
    assert_trace_satisfied(trace);
}

#[test]
fn scalar_bits_are_exactly_86_86_84_and_boundary_stable() {
    let value = [85_usize, 86, 87, 171, 172, 173]
        .into_iter()
        .fold(BigUint::from(0_u8), |value, bit| {
            value | (BigUint::from(1_u8) << bit)
        });
    let mut builder = TraceBuilder::<Fp>::new();
    let scalar = builder
        .load_uint(value, "boundary scalar")
        .expect("bounded scalar");
    let bits = scalar_bits(&mut builder, &scalar);
    for bit in 0..256 {
        assert_eq!(
            bits[bit].value(),
            [85_usize, 86, 87, 171, 172, 173].contains(&bit),
            "bit {bit}"
        );
    }
}

#[test]
fn rfc_trace_is_fixed_schedule_and_fails_closed_at_the_row_cap() {
    let vector = rfc6979_sample();
    let circuit =
        P256AffineCompactEcdsaCircuitV1::<Fp>::new(vector.sec1, vector.digest, vector.signature);
    let Err(P256AffineCompactFailureV1::RowCapacityExceeded { rows, maximum }) =
        circuit.trace_diagnostic_for_test()
    else {
        panic!("RFC trace must remain ineligible until its exact rows fit k=16");
    };
    assert_eq!(maximum, K16_MAX_ASSIGNED_ROWS);
    assert_eq!(*rows, P256_AFFINE_COMPACT_RFC6979_ROWS_V1);
    assert_eq!(rows.total_rows - maximum, 172_712);
    assert_eq!(
        rows.binding_rows
            + rows.range_rows
            + rows.arithmetic_rows
            + rows.selection_rows
            + rows.fixed_selection_rows,
        rows.total_rows
    );
    // Range relations are paired only within an exact bit-width batch. Two
    // odd batches therefore cost one more row than a global ceil(count / 2).
    assert_eq!(rows.range_lookups.div_ceil(2) + 1, rows.range_rows);
    assert_eq!(rows.fma_relations.div_ceil(2), rows.arithmetic_rows);
    assert_eq!(rows.select_relations.div_ceil(2), rows.selection_rows);

    // With every other current family frozen, only 5,936 rows remain for all
    // arithmetic. The present 178,648-row family is the exact dominant blocker,
    // more than thirty times that residual budget; incremental packing cannot
    // turn this realization into a k=16 child.
    let non_arithmetic =
        rows.binding_rows + rows.range_rows + rows.selection_rows + rows.fixed_selection_rows;
    assert_eq!(non_arithmetic, 59_591);
    assert_eq!(maximum - non_arithmetic, 5_936);
    assert!(rows.arithmetic_rows > 30 * (maximum - non_arithmetic));
}

fn assert_valid<F: BigPrimeField>(vector: &TestVector) {
    let circuit =
        P256AffineCompactEcdsaCircuitV1::<F>::new(vector.sec1, vector.digest, vector.signature);
    let instances = circuit.instances().expect("eligible compact affine trace");
    MockProver::run(K, &circuit, vec![instances])
        .expect("k=16 synthesis")
        .assert_satisfied();
}

#[test]
#[ignore = "run only after the exact row reporter establishes k=16 eligibility"]
fn rfc6979_semantics_hold_over_both_pasta_fields() {
    let vector = rfc6979_sample();
    assert_valid::<Fp>(&vector);
    assert_valid::<Fq>(&vector);
}

#[test]
fn source_guard_keeps_the_affine_child_private_and_non_authorizing() {
    let source = include_str!("p256_affine_compact.rs");
    assert!(source.contains("declared only for source-settled prototype tests"));
    assert!(source.contains("not a production verifier"));
    assert!(source.contains("Every constant instance tail"));
    assert!(source.contains("cannot authorize a helper proof"));
    assert!(source.contains("instance_contract_for_test"));
    assert!(source.contains("P256_AFFINE_COMPACT_CONSTANT_TAIL_DIGEST_V1"));
    assert!(source.contains("every exact verifier-derived tail value"));
    assert!(source.contains("Disabled = 0"));
    assert!(source.contains("row.mode = RowMode::Fma"));
    assert!(source.contains("K16_MAX_ASSIGNED_ROWS"));
    assert!(source.contains("P256_AFFINE_COMPACT_SHAPE_V1"));
    assert!(!source.contains("GuardBundleVerifier"));
    assert!(!source.contains("register_backend"));
}
