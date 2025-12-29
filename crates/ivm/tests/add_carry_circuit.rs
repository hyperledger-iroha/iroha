#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::AddCarryCircuit;

#[test]
fn test_add_carry_circuit() {
    let c = AddCarryCircuit {
        a: 0xffff_ffff,
        b: 1,
        carry_in: 0,
        sum: 0xffff_ffffu32.wrapping_add(1),
        carry_out: 1,
    };
    assert!(c.verify().is_ok());
    let fail = AddCarryCircuit {
        a: 1,
        b: 1,
        carry_in: 1,
        sum: 3,
        carry_out: 1,
    };
    assert!(fail.verify().is_err());
}
