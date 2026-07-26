#![cfg(feature = "ivm_zk_tests")]
use blstrs::{G1Projective, Scalar};
use group::{Curve, Group};
use ivm::halo2::{ECAddCircuit, ECMulVarCircuit, PubKeyGenCircuit};

#[test]
fn test_ecadd_circuit() {
    let p = G1Projective::generator() * Scalar::from(3u64);
    let q = G1Projective::generator() * Scalar::from(5u64);
    let r = p + q;
    let circuit = ECAddCircuit {
        p: p.to_affine().to_compressed(),
        q: q.to_affine().to_compressed(),
        result: r.to_affine().to_compressed(),
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_ecmul_var_circuit() {
    let p = G1Projective::generator();
    let scalar = Scalar::from(7u64);
    let r = p * scalar;
    let circuit = ECMulVarCircuit {
        scalar: scalar.to_bytes_be(),
        point: p.to_affine().to_compressed(),
        result: r.to_affine().to_compressed(),
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_pubkgen_circuit() {
    let secret = Scalar::from(9u64);
    let pk = G1Projective::generator() * secret;
    let circuit = PubKeyGenCircuit {
        secret: secret.to_bytes_be(),
        result: pk.to_affine().to_compressed(),
    };
    assert!(circuit.verify().is_ok());
}
