#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{VectorCircuit, VectorOp};

#[test]
fn test_vadd32_circuit() {
    let c = VectorCircuit {
        op: VectorOp::Vadd32,
        a: [1, 0xffff_ffff, 5, 0],
        b: [1, 2, 0xffff_ffff, 3],
        k: 0,
        result: [2, 1, 4, 3],
    };
    assert!(c.verify().is_ok());
}

#[test]
fn test_vadd64_circuit() {
    let c = VectorCircuit {
        op: VectorOp::Vadd64,
        a: [0xffff_ffff, 0, 0x1234_5678, 0],
        b: [1, 0, 0xffff_ffff, 0],
        k: 0,
        result: [0, 1, 0x1234_5677, 1],
    };
    assert!(c.verify().is_ok());
}

#[test]
fn test_vbit_circuit() {
    let c_and = VectorCircuit {
        op: VectorOp::Vand,
        a: [0xffff_0000, 0x1234_5678, 0xffff_ffff, 0],
        b: [0xff00_ff00, 0xffff_0000, 0, 0xffff_ffff],
        k: 0,
        result: [0xff00_0000, 0x1234_0000, 0, 0],
    };
    assert!(c_and.verify().is_ok());
    let c_xor = VectorCircuit {
        op: VectorOp::Vxor,
        a: [0xAAAA_AAAA, 0x5555_5555, 0, 0xffff_ffff],
        b: [0xffff_ffff, 0, 0xffff_ffff, 0xffff_ffff],
        k: 0,
        result: [0x5555_5555, 0x5555_5555, 0xffff_ffff, 0],
    };
    assert!(c_xor.verify().is_ok());
    let c_or = VectorCircuit {
        op: VectorOp::Vor,
        a: [0, 0xffff_0000, 0x1234_0000, 0],
        b: [0xffff_ffff, 0x0000_ffff, 0x0000_ffff, 0],
        k: 0,
        result: [0xffff_ffff, 0xffff_ffff, 0x1234_ffff, 0],
    };
    assert!(c_or.verify().is_ok());
}

#[test]
fn test_vrot32_circuit() {
    let c = VectorCircuit {
        op: VectorOp::Vrot32,
        a: [0x1111_2222, 0x3333_4444, 0x5555_6666, 0x7777_8888],
        b: [0; 4],
        k: 8,
        result: [0x1122_2211, 0x3344_4433, 0x5566_6655, 0x7788_8877],
    };
    assert!(c.verify().is_ok());
}
