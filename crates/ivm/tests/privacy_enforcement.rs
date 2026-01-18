//! Privacy-tag enforcement tests for ZK execution.

use ivm::{IVM, Instruction, Memory, ProgramMetadata, VMError, encoding, instruction};

fn meta_with_mode(mode: u8) -> ProgramMetadata {
    ProgramMetadata {
        mode,
        max_cycles: 2,
        ..ProgramMetadata::default()
    }
}

#[test]
fn branch_on_private_fails() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, 5);
    vm.registers.set_tag(1, true);
    vm.set_register(2, 5);
    vm.registers.set_tag(2, false);
    let res = vm.execute_instruction(Instruction::Beq {
        rs: 1,
        rt: 2,
        offset: 1,
    });
    assert!(matches!(res, Err(VMError::PrivacyViolation)));
}

#[test]
fn load_private_address_fails() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, Memory::HEAP_START);
    vm.registers.set_tag(1, true);
    let res = vm.execute_instruction(Instruction::Load {
        rd: 2,
        addr_reg: 1,
        offset: 0,
    });
    assert!(matches!(res, Err(VMError::PrivacyViolation)));
}

#[test]
fn add_private_succeeds() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, 3);
    vm.registers.set_tag(1, true);
    vm.set_register(2, 4);
    vm.registers.set_tag(2, true);
    vm.execute_instruction(Instruction::Add {
        rd: 3,
        rs: 1,
        rt: 2,
    })
    .unwrap();
    assert_eq!(vm.register(3), 7);
    assert!(vm.registers.tag(3));
}

#[test]
fn simple_addi_propagates_tag() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, 11);
    vm.registers.set_tag(1, true);
    vm.execute_instruction(Instruction::AddImm {
        rd: 3,
        rs: 1,
        imm: 5,
    })
    .unwrap();
    assert_eq!(vm.register(3), 16);
    assert!(vm.registers.tag(3));
}

#[test]
fn simple_shift_mismatched_tags_fails() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, 1);
    vm.set_register(2, 2);
    vm.registers.set_tag(1, true);
    vm.registers.set_tag(2, false);
    let err = vm.execute_instruction(Instruction::Sll {
        rd: 3,
        rs: 1,
        rt: 2,
    });
    assert!(matches!(err, Err(VMError::PrivacyViolation)));
}

#[test]
fn parallel_addi_propagates_tag() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, 7);
    vm.registers.set_tag(1, true);
    let block = [Instruction::AddImm {
        rd: 3,
        rs: 1,
        imm: 4,
    }];
    vm.execute_block_parallel(&block).unwrap();
    assert_eq!(vm.register(3), 11);
    assert!(vm.registers.tag(3));
}

#[test]
fn parallel_shift_mismatched_tags_fails() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, 1);
    vm.set_register(2, 3);
    vm.registers.set_tag(1, true);
    vm.registers.set_tag(2, false);
    let block = [Instruction::Srl {
        rd: 3,
        rs: 1,
        rt: 2,
    }];
    let err = vm.execute_block_parallel(&block);
    assert!(matches!(err, Err(VMError::PrivacyViolation)));
}

#[test]
fn sha256block_private_address_fails() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK | ivm::ivm_mode::VECTOR).encode();
    let sha = encoding::wide::encode_rr(instruction::wide::crypto::SHA256BLOCK, 0, 1, 0);
    program.extend_from_slice(&sha.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(1, Memory::HEAP_START);
    vm.registers.set_tag(1, true);
    let err = vm.run().unwrap_err();
    assert!(matches!(err, VMError::PrivacyViolation));
}

#[test]
fn sha3block_private_address_fails() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK).encode();
    let sha3 = encoding::wide::encode_rr(instruction::wide::crypto::SHA3BLOCK, 4, 10, 11);
    program.extend_from_slice(&sha3.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(10, Memory::HEAP_START);
    vm.set_register(11, Memory::HEAP_START);
    vm.set_register(4, Memory::HEAP_START);
    vm.registers.set_tag(10, true);
    let err = vm.run().unwrap_err();
    assert!(matches!(err, VMError::PrivacyViolation));
}

#[test]
fn wide_add_mismatched_tags_fails() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK).encode();
    let add = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 3, 1, 2);
    program.extend_from_slice(&add.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(1, 10);
    vm.set_register(2, 20);
    vm.registers.set_tag(1, true);
    vm.registers.set_tag(2, false);
    let err = vm.run().unwrap_err();
    assert!(matches!(err, VMError::PrivacyViolation));
}

#[test]
fn wide_add_propagates_secret_tag() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK).encode();
    let add = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 3, 1, 2);
    program.extend_from_slice(&add.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(1, 10);
    vm.set_register(2, 20);
    vm.registers.set_tag(1, true);
    vm.registers.set_tag(2, true);
    vm.run().unwrap();
    assert!(vm.registers.tag(3));
}

#[test]
fn wide_addi_propagates_tag() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK).encode();
    let addi = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 3, 1, 7);
    program.extend_from_slice(&addi.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(1, 10);
    vm.registers.set_tag(1, true);
    vm.run().unwrap();
    assert!(vm.registers.tag(3));
}

#[test]
fn wide_cmov_secret_condition_fails() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK).encode();
    let cmov = encoding::wide::encode_rr(instruction::wide::arithmetic::CMOV, 3, 1, 2);
    program.extend_from_slice(&cmov.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(1, 42);
    vm.set_register(2, 1);
    vm.registers.set_tag(2, true);
    let err = vm.run().unwrap_err();
    assert!(matches!(err, VMError::PrivacyViolation));
}

#[test]
fn wide_jal_clears_tag() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK).encode();
    let jal = encoding::wide::encode_jump(instruction::wide::control::JAL, 5, 1);
    program.extend_from_slice(&jal.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.registers.set_tag(5, true);
    vm.run().unwrap();
    assert!(!vm.registers.tag(5));
}

#[test]
fn wide_jalr_secret_target_fails() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK).encode();
    let jalr = encoding::wide::encode_ri(instruction::wide::control::JALR, 5, 1, 0);
    program.extend_from_slice(&jalr.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(1, 0);
    vm.registers.set_tag(1, true);
    let err = vm.run().unwrap_err();
    assert!(matches!(err, VMError::PrivacyViolation));
}

#[test]
fn wide_aesenc_mismatched_tags_fails() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK | ivm::ivm_mode::VECTOR).encode();
    let aes = encoding::wide::encode_rr(instruction::wide::crypto::AESENC, 20, 10, 12);
    program.extend_from_slice(&aes.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(10, 0);
    vm.set_register(11, 0);
    vm.set_register(12, 0);
    vm.set_register(13, 0);
    vm.registers.set_tag(10, true);
    vm.registers.set_tag(11, true);
    vm.registers.set_tag(12, false);
    vm.registers.set_tag(13, false);
    let err = vm.run().unwrap_err();
    assert!(matches!(err, VMError::PrivacyViolation));
}

#[test]
fn wide_aesdec_propagates_tag() {
    let mut program = meta_with_mode(ivm::ivm_mode::ZK | ivm::ivm_mode::VECTOR).encode();
    let aes = encoding::wide::encode_rr(instruction::wide::crypto::AESDEC, 20, 10, 12);
    program.extend_from_slice(&aes.to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(10, 0);
    vm.set_register(11, 0);
    vm.set_register(12, 0);
    vm.set_register(13, 0);
    vm.registers.set_tag(10, true);
    vm.registers.set_tag(11, true);
    vm.registers.set_tag(12, true);
    vm.registers.set_tag(13, true);
    vm.registers.set_tag(20, false);
    vm.registers.set_tag(21, false);
    vm.run().unwrap();
    assert!(vm.registers.tag(20));
    assert!(vm.registers.tag(21));
}
