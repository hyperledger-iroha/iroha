//! Control-flow verifier circuit regressions.

#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{BranchCircuit, HaltCircuit, JalCircuit, JumpCircuit, JumpRegCircuit};
#[test]
fn test_jump_circuit() {
    let c = JumpCircuit {
        pc: 0,
        offset: 8,
        next_pc: 8,
        code_len: 16,
    };
    assert!(c.verify().is_ok());
}
#[test]
fn jump_circuit_rejects_half_word_target() {
    let c = JumpCircuit {
        pc: 0,
        offset: 2,
        next_pc: 2,
        code_len: 16,
    };
    assert!(c.verify().is_err());
}
#[test]
fn jump_circuit_rejects_unbounded_pc_without_panicking() {
    let c = JumpCircuit {
        pc: i64::MAX as u64,
        offset: 4,
        next_pc: (i64::MAX as u64).wrapping_add(4),
        code_len: u64::MAX,
    };
    let result = std::panic::catch_unwind(|| c.verify());
    assert!(result.expect("verification must not panic").is_err());
}
#[test]
fn test_jal_circuit() {
    let c = JalCircuit {
        pc: 0,
        offset: 8,
        next_pc: 8,
        link: 4,
        code_len: 16,
    };
    assert!(c.verify().is_ok());
}
#[test]
fn test_jump_reg_circuit() {
    let c = JumpRegCircuit {
        pc: 4,
        base: 100,
        imm: 4,
        next_pc: 104,
        link: Some(8),
        code_len: 200,
    };
    assert!(c.verify().is_ok());
}
#[test]
fn jump_reg_circuit_uses_runtime_four_byte_alignment() {
    let c = JumpRegCircuit {
        pc: 0,
        base: 2,
        imm: 0,
        next_pc: 2,
        link: None,
        code_len: 16,
    };
    assert!(c.verify().is_err());
}
#[test]
fn test_branch_circuit_taken() {
    let c = BranchCircuit {
        pc: 0,
        offset: 8,
        take_branch: true,
        next_pc: 8,
        code_len: 32,
    };
    assert!(c.verify().is_ok());
}
#[test]
fn branch_circuit_rejects_half_word_target() {
    let c = BranchCircuit {
        pc: 0,
        offset: 2,
        take_branch: true,
        next_pc: 2,
        code_len: 16,
    };
    assert!(c.verify().is_err());
}
#[test]
fn test_branch_circuit_not_taken() {
    let c = BranchCircuit {
        pc: 0,
        offset: 8,
        take_branch: false,
        next_pc: 4,
        code_len: 32,
    };
    assert!(c.verify().is_ok());
}
#[test]
fn test_halt_circuit() {
    let c = HaltCircuit { pc: 8, next_pc: 12 };
    assert!(c.verify().is_ok());
}
