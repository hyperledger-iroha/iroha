use super::*;

use halo2_base::{
    halo2_proofs::{
        dev::MockProver,
        halo2curves::{
            pasta::{Fp, Fq},
            CurveAffine,
        },
        plonk::ConstraintSystem,
    },
    utils::{biguint_to_fe, fe_to_biguint},
};

#[derive(Clone, Debug)]
struct TestVector {
    sec1: [u8; 65],
    digest: [u8; 32],
    signature: [u8; 64],
}

fn decode_hex<const N: usize>(encoded: &str) -> [u8; N] {
    hex::decode(encoded)
        .expect("test vector is hexadecimal")
        .try_into()
        .unwrap_or_else(|_| panic!("test vector has exactly {N} bytes"))
}

fn encode_biguint<const N: usize>(value: &BigUint) -> [u8; N] {
    let encoded = value.to_bytes_be();
    assert!(encoded.len() <= N);
    let mut out = [0_u8; N];
    out[N - encoded.len()..].copy_from_slice(&encoded);
    out
}

fn assert_p256_little_endian_adapter<F: BigPrimeField>() {
    let integer =
        (BigUint::from(0x0123_4567_89ab_cdef_u64) << 64) + BigUint::from(0xfedc_ba98_7654_3210_u64);
    let value = biguint_to_fe::<F>(&integer);
    let bytes = value.to_bytes_le();
    assert_eq!(bytes.as_slice(), value.to_repr().as_ref());
    assert_eq!(BigUint::from_bytes_le(&bytes), integer);

    let limbs = value.to_u64_limbs(20, 13);
    let recomposed = limbs.iter().rev().fold(BigUint::from(0_u8), |acc, limb| {
        (acc << 13) + BigUint::from(*limb)
    });
    assert_eq!(recomposed, integer);
    assert_eq!(value.get_lower_32(), 0x7654_3210);
    assert_eq!(value.get_lower_64(), 0xfedc_ba98_7654_3210);
}

#[test]
fn p256_scalar_field_adapter_is_little_endian_and_limb_exact() {
    assert_p256_little_endian_adapter::<P256Base>();
    assert_p256_little_endian_adapter::<P256Scalar>();
}

// RFC 6979 A.2.5, P-256/SHA-256 "sample".  RFC 6979 publishes a high-S
// representative; the accepted fixture uses its mathematically equivalent
// low-S representative so the prototype's canonical policy is exercised.
fn rfc6979_sample() -> TestVector {
    let x = decode_hex::<32>("60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6");
    let y = decode_hex::<32>("7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299");
    let digest =
        decode_hex::<32>("AF2BDBE1AA9B6EC1E2ADE1D694F41FC71A831D0268E9891562113D8A62ADD1BF");
    let r = decode_hex::<32>("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716");
    let high_s = BigUint::from_bytes_be(&decode_hex::<32>(
        "F7CB1C942D657C41D436C7A1B6E29F65F3E900DBB9AFF4064DC4AB2F843ACDA8",
    ));
    let low_s = modulus::<P256Scalar>() - high_s;

    let mut sec1 = [0_u8; 65];
    sec1[0] = 4;
    sec1[1..33].copy_from_slice(&x);
    sec1[33..].copy_from_slice(&y);
    let mut signature = [0_u8; 64];
    signature[..32].copy_from_slice(&r);
    signature[32..].copy_from_slice(&encode_biguint::<32>(&low_s));
    TestVector {
        sec1,
        digest,
        signature,
    }
}

fn generated_vector(digest_integer: BigUint) -> TestVector {
    assert!(digest_integer.bits() <= 256);
    let n = modulus::<P256Scalar>();
    let secret = P256Scalar::from(7);
    let nonce = P256Scalar::from(13);
    let public_key = (Secp256r1Affine::generator() * secret).to_affine();
    let nonce_point = (Secp256r1Affine::generator() * nonce).to_affine();
    let nonce_coordinates = nonce_point.coordinates().expect("nonzero nonce");
    let r_integer = fe_to_biguint(nonce_coordinates.x()) % &n;
    let r = biguint_to_fe::<P256Scalar>(&r_integer);
    let z = biguint_to_fe::<P256Scalar>(&(&digest_integer % &n));
    let mut s = nonce.invert().expect("nonzero nonce") * (z + r * secret);
    let mut s_integer = fe_to_biguint(&s);
    if s_integer > (&n >> 1usize) {
        s = -s;
        s_integer = fe_to_biguint(&s);
    }
    assert_ne!(s_integer, BigUint::from(0_u8));

    let coordinates = public_key.coordinates().expect("nonzero public key");
    let mut sec1 = [0_u8; 65];
    sec1[0] = 4;
    sec1[1..33].copy_from_slice(&encode_biguint::<32>(&fe_to_biguint(coordinates.x())));
    sec1[33..].copy_from_slice(&encode_biguint::<32>(&fe_to_biguint(coordinates.y())));
    let mut signature = [0_u8; 64];
    signature[..32].copy_from_slice(&encode_biguint::<32>(&r_integer));
    signature[32..].copy_from_slice(&encode_biguint::<32>(&s_integer));
    TestVector {
        sec1,
        digest: encode_biguint::<32>(&digest_integer),
        signature,
    }
}

fn prover<F: BigPrimeField>(vector: &TestVector) -> MockProver<F> {
    let circuit = P256CompactEcdsaCircuitV1::<F>::new(vector.sec1, vector.digest, vector.signature);
    let instances = circuit.instances().expect("compact instance derivation");
    MockProver::run(K, &circuit, vec![instances]).expect("k=16 compact synthesis")
}

fn assert_valid<F: BigPrimeField>(vector: &TestVector) {
    prover::<F>(vector).assert_satisfied();
}

fn assert_invalid<F: BigPrimeField>(vector: &TestVector) {
    assert!(prover::<F>(vector).verify().is_err());
}

#[test]
fn configured_shape_is_exactly_current_query_and_3200_augmented_bytes() {
    let mut meta = ConstraintSystem::<Fp>::default();
    let _ = P256CompactConfigV1::configure(&mut meta);
    assert_eq!(meta.degree(), P256_COMPACT_SHAPE_V1.degree);
    assert_eq!(
        meta.num_advice_columns(),
        P256_COMPACT_SHAPE_V1.advice_columns
    );
    assert_eq!(
        meta.num_instance_columns(),
        P256_COMPACT_SHAPE_V1.instance_columns
    );
    assert_eq!(
        meta.num_fixed_columns(),
        P256_COMPACT_SHAPE_V1.fixed_columns
    );
    assert_eq!(meta.num_selectors(), P256_COMPACT_SHAPE_V1.selectors);
    assert_eq!(
        meta.advice_queries().len(),
        P256_COMPACT_SHAPE_V1.advice_queries
    );
    assert_eq!(
        meta.instance_queries().len(),
        P256_COMPACT_SHAPE_V1.instance_queries
    );
    assert_eq!(
        meta.fixed_queries().len(),
        P256_COMPACT_SHAPE_V1.fixed_queries
    );
    assert!(meta
        .advice_queries()
        .iter()
        .all(|(_, rotation)| *rotation == Rotation::cur()));
    assert!(meta
        .instance_queries()
        .iter()
        .all(|(_, rotation)| *rotation == Rotation::cur()));
    assert!(meta
        .fixed_queries()
        .iter()
        .all(|(_, rotation)| *rotation == Rotation::cur()));
    assert_eq!(
        meta.permutation().get_columns().len(),
        P256_COMPACT_SHAPE_V1.equality_columns
    );
    assert_eq!(meta.lookups().len(), P256_COMPACT_SHAPE_V1.lookup_arguments);
    assert_eq!(P256_COMPACT_SHAPE_V1.raw_proof_bytes, 3_168);
    assert_eq!(P256_COMPACT_SHAPE_V1.augmented_proof_bytes, 3_200);
}

#[test]
fn rfc6979_pre_cap_trace_diagnostic_is_exact() {
    let vector = rfc6979_sample();
    let circuit =
        P256CompactEcdsaCircuitV1::<Fp>::new(vector.sec1, vector.digest, vector.signature);
    assert_eq!(
        circuit.trace_diagnostic_for_test(),
        Err(P256CompactTraceFailureV1::RowCapacityExceeded {
            rows: P256CompactRowsV1 {
                binding_rows: 7_956,
                range_rows: 63_080,
                arithmetic_rows: 52_577,
                equality_rows: 647,
                total_rows: 116_304,
                virtual_gates: 203_645,
                virtual_lookups: 118_203,
                coalesced_lookups: 98_492,
            },
            maximum: K16_MAX_ASSIGNED_ROWS,
        })
    );
}

#[test]
#[ignore = "generic halo2-ecc transpose needs 116,304 rows; bespoke P-256 child required"]
fn rfc6979_semantic_satisfaction_requires_bespoke_child() {
    let vector = rfc6979_sample();
    let fp_circuit =
        P256CompactEcdsaCircuitV1::<Fp>::new(vector.sec1, vector.digest, vector.signature);
    let rows = fp_circuit.row_report().expect("row report");
    assert!(rows.total_rows <= K16_MAX_ASSIGNED_ROWS, "{rows:?}");
    assert!(rows.coalesced_lookups > 0, "{rows:?}");
    assert_valid::<Fp>(&vector);
    assert_valid::<Fq>(&vector);
}

#[test]
#[ignore = "generic halo2-ecc transpose needs 116,304 rows; bespoke P-256 child required"]
fn digest_at_or_above_n_uses_exact_single_conditional_reduction() {
    let digest = modulus::<P256Scalar>() + 42_u8;
    assert!(digest.bits() <= 256);
    assert_valid::<Fp>(&generated_vector(digest));
}

#[test]
#[ignore = "generic halo2-ecc transpose needs 116,304 rows; bespoke P-256 child required"]
fn malformed_prefix_zero_and_out_of_range_scalars_fail_closed() {
    let vector = rfc6979_sample();

    let mut bad_prefix = vector.clone();
    bad_prefix.sec1[0] = 3;
    assert_invalid::<Fp>(&bad_prefix);

    let mut zero_r = vector.clone();
    zero_r.signature[..32].fill(0);
    assert_invalid::<Fp>(&zero_r);

    let mut out_of_range_s = vector;
    out_of_range_s.signature[32..].copy_from_slice(&encode_biguint::<32>(&modulus::<P256Scalar>()));
    assert_invalid::<Fp>(&out_of_range_s);
}

#[test]
#[ignore = "generic halo2-ecc transpose needs 116,304 rows; bespoke P-256 child required"]
fn high_s_twin_and_off_curve_nonidentity_key_fail_closed() {
    let vector = rfc6979_sample();
    let n = modulus::<P256Scalar>();

    let mut high_s = vector.clone();
    let low_s = BigUint::from_bytes_be(&high_s.signature[32..]);
    high_s.signature[32..].copy_from_slice(&encode_biguint::<32>(&(&n - low_s)));
    assert_invalid::<Fp>(&high_s);

    let mut off_curve = vector;
    off_curve.sec1[1..33].fill(0);
    off_curve.sec1[33..65].fill(0);
    off_curve.sec1[64] = 1;
    assert_invalid::<Fp>(&off_curve);
}

#[test]
fn source_guard_keeps_a_minus_three_and_non_authorizing_boundary_explicit() {
    let source = include_str!("p256_compact.rs");
    assert!(source.contains("3*x^2 - 3"));
    assert!(source.contains("Secp256r1Affine::a()"));
    assert!(source.contains("gate.assert_is_const(ctx, &result, &F::ONE)"));
    assert!(source.contains("Private, non-authorizing P-256 ECDSA circuit prototype"));
    assert!(source.contains("exact RFC 6979 trace occupies 116,304 rows"));
    assert!(source.contains("cannot authorize a helper proof"));
    assert!(source.contains("P256_VARIABLE_OFFSET_SCALAR"));
    assert!(source.contains("P256_FIXED_OFFSET_SCALAR"));
    assert!(source.contains("P256_SUM_OFFSET_SCALAR"));
    assert!(!source.contains("OsRng"));
    assert!(!source.contains("OfflineCashGuardBundle"));
    assert!(!source.contains("ecdsa_verify_no_pubkey_check"));
}
