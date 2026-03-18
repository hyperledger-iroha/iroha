#![cfg(feature = "ivm_zk_tests")]
use ivm::{halo2::PairingCircuit, pairing_check_truncated};

#[test]
fn test_pairing_circuit() {
    let a = 2u64;
    let b = 3u64;
    let expected = pairing_check_truncated(a, b);
    let circuit = PairingCircuit {
        a,
        b,
        result: expected,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_pairing_circuit_bad_witness() {
    let a = 2u64;
    let b = 3u64;
    let mut expected = pairing_check_truncated(a, b);
    expected ^= 1;
    let circuit = PairingCircuit {
        a,
        b,
        result: expected,
    };
    assert!(circuit.verify().is_err());
}
