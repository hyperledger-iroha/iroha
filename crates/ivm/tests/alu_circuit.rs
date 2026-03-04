#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{ALUCircuit, ALUOp};

#[test]
fn test_add_sub_circuit() {
    let add = ALUCircuit {
        op: ALUOp::Add,
        a: 5,
        b: 7,
        result: 12,
    };
    assert!(add.verify().is_ok());
    let add_fail = ALUCircuit {
        op: ALUOp::Add,
        a: 5,
        b: 7,
        result: 13,
    };
    assert!(add_fail.verify().is_err());

    let sub = ALUCircuit {
        op: ALUOp::Sub,
        a: 5,
        b: 10,
        result: 5u64.wrapping_sub(10),
    };
    assert!(sub.verify().is_ok());
}

#[test]
fn test_bitwise_ops_circuit() {
    let and_c = ALUCircuit {
        op: ALUOp::And,
        a: 0xF0F0u64,
        b: 0xAAAAu64,
        result: 0xF0F0 & 0xAAAA,
    };
    assert!(and_c.verify().is_ok());
    let or_c = ALUCircuit {
        op: ALUOp::Or,
        a: 0xF0F0u64,
        b: 0xAAAAu64,
        result: 0xF0F0 | 0xAAAA,
    };
    assert!(or_c.verify().is_ok());
    let xor_c = ALUCircuit {
        op: ALUOp::Xor,
        a: 0xF0F0u64,
        b: 0xAAAAu64,
        result: 0xF0F0 ^ 0xAAAA,
    };
    assert!(xor_c.verify().is_ok());
    let not_c = ALUCircuit {
        op: ALUOp::Not,
        a: 0xFFFF_0000_0000_0000,
        b: 0,
        result: !0xFFFF_0000_0000_0000u64,
    };
    assert!(not_c.verify().is_ok());
}

#[test]
fn test_shift_rotate_circuit() {
    let sll = ALUCircuit {
        op: ALUOp::Sll,
        a: 1,
        b: 4,
        result: 16,
    };
    assert!(sll.verify().is_ok());
    let srl = ALUCircuit {
        op: ALUOp::Srl,
        a: 0x80,
        b: 4,
        result: 0x8,
    };
    assert!(srl.verify().is_ok());
    let sra = ALUCircuit {
        op: ALUOp::Sra,
        a: (-8i64 as u64),
        b: 1,
        result: (-4i64) as u64,
    };
    assert!(sra.verify().is_ok());
    let rol = ALUCircuit {
        op: ALUOp::Rol,
        a: 0x01,
        b: 4,
        result: 0x10,
    };
    assert!(rol.verify().is_ok());
    let ror = ALUCircuit {
        op: ALUOp::Ror,
        a: 0x10,
        b: 4,
        result: 0x01,
    };
    assert!(ror.verify().is_ok());
}
