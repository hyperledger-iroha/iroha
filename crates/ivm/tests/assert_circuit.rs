#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{AssertEqCircuit, AssertRangeCircuit, AssertZeroCircuit};

#[test]
fn test_assert_zero_circuit() {
    let ok = AssertZeroCircuit { value: 0 };
    assert!(ok.verify().is_ok());
    let fail = AssertZeroCircuit { value: 5 };
    assert!(fail.verify().is_err());
}

#[test]
fn test_assert_eq_circuit() {
    let ok = AssertEqCircuit { a: 42, b: 42 };
    assert!(ok.verify().is_ok());
    let fail = AssertEqCircuit { a: 5, b: 7 };
    assert!(fail.verify().is_err());
}

#[test]
fn test_assert_range_circuit() {
    let pass_low = AssertRangeCircuit { value: 0, bits: 8 };
    assert!(pass_low.verify().is_ok());
    let pass_high = AssertRangeCircuit {
        value: 255,
        bits: 8,
    };
    assert!(pass_high.verify().is_ok());
    let mid = AssertRangeCircuit { value: 42, bits: 8 };
    assert!(mid.verify().is_ok());
    let fail = AssertRangeCircuit {
        value: 300,
        bits: 8,
    };
    assert!(fail.verify().is_err());
}
