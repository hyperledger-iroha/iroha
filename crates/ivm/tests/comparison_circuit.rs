#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{CmpCircuit, CmpOp};

#[test]
fn test_slt_sltu_circuit() {
    let slt = CmpCircuit {
        op: CmpOp::Slt,
        a: (-1i64) as u64,
        b: 0,
        prev: 0,
        result: 1,
    };
    assert!(slt.verify().is_ok());

    let sltu = CmpCircuit {
        op: CmpOp::Sltu,
        a: (-1i64) as u64,
        b: 0,
        prev: 0,
        result: 0,
    };
    assert!(sltu.verify().is_ok());
}

#[test]
fn test_seq_sne_circuit() {
    let seq = CmpCircuit {
        op: CmpOp::Seq,
        a: 5,
        b: 5,
        prev: 0,
        result: 1,
    };
    assert!(seq.verify().is_ok());
    let sne = CmpCircuit {
        op: CmpOp::Sne,
        a: 5,
        b: 6,
        prev: 0,
        result: 1,
    };
    assert!(sne.verify().is_ok());
    let seq_bits = CmpCircuit {
        op: CmpOp::Seq,
        a: (-1i64) as u64,
        b: 0xFFFF_FFFF_FFFF_FFFF,
        prev: 0,
        result: 1,
    };
    assert!(seq_bits.verify().is_ok());
}

#[test]
fn test_cmov_circuit() {
    let cmov_true = CmpCircuit {
        op: CmpOp::Cmov,
        a: 7,
        b: 1,
        prev: 42,
        result: 7,
    };
    assert!(cmov_true.verify().is_ok());
    let cmov_false = CmpCircuit {
        op: CmpOp::Cmov,
        a: 7,
        b: 0,
        prev: 42,
        result: 42,
    };
    assert!(cmov_false.verify().is_ok());
    let cmov_bad = CmpCircuit {
        op: CmpOp::Cmov,
        a: 7,
        b: 3,
        prev: 42,
        result: 43,
    };
    assert!(cmov_bad.verify().is_err());
}
