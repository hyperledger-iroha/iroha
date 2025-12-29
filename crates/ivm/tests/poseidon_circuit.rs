#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{Poseidon2Circuit, Poseidon6Circuit};

#[test]
fn test_poseidon2_circuit() {
    let a = 1u64;
    let b = 2u64;
    let expected = ivm::poseidon2(a, b);
    let circuit = Poseidon2Circuit {
        a,
        b,
        result: expected,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_poseidon2_bad_witness() {
    let a = 1u64;
    let b = 2u64;
    let mut expected = ivm::poseidon2(a, b);
    expected ^= 1;
    let circuit = Poseidon2Circuit {
        a,
        b,
        result: expected,
    };
    assert!(circuit.verify().is_err());
}

#[test]
fn test_poseidon6_circuit() {
    let inputs = [1u64, 2, 3, 4, 5, 6];
    let expected = ivm::poseidon6(inputs);
    let circuit = Poseidon6Circuit {
        inputs,
        result: expected,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_poseidon6_bad_witness() {
    let inputs = [1u64, 2, 3, 4, 5, 6];
    let mut expected = ivm::poseidon6(inputs);
    expected ^= 1;
    let circuit = Poseidon6Circuit {
        inputs,
        result: expected,
    };
    assert!(circuit.verify().is_err());
}
