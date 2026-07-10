//! Privacy-tag enforcement tests for ZK execution.

use iroha_crypto::Hash;
use ivm::{
    IVM, Instruction, Memory, ProgramMetadata, VMError, encoding, host::DefaultHost, instruction,
    pedersen_commit_truncated, pointer_abi::PointerType, syscalls,
};

fn meta_with_mode(mode: u8) -> ProgramMetadata {
    ProgramMetadata {
        mode,
        max_cycles: 2,
        ..ProgramMetadata::default()
    }
}

fn raw_zk_program(words: &[u32]) -> Vec<u8> {
    let mut program = ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        max_cycles: 32,
        ..ProgramMetadata::default()
    }
    .encode();
    for word in words {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    program
}

fn scall(number: u32) -> u32 {
    encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        u8::try_from(number).expect("test syscall fits compact SCALL"),
    )
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

#[test]
fn private_input_syscall_marks_result_private() {
    let program = raw_zk_program(&[scall(syscalls::SYSCALL_GET_PRIVATE_INPUT)]);
    let mut vm = IVM::new(10_000);
    vm.set_host(DefaultHost::with_private_inputs(vec![42]));
    vm.load_program(&program).unwrap();
    vm.set_register(10, 0);

    vm.run().expect("private input should load");

    assert_eq!(vm.register(10), 42);
    assert!(vm.registers.tag(10));
}

#[test]
fn private_input_syscall_requires_zk_execution_mode() {
    let mut program = ProgramMetadata {
        max_cycles: 32,
        ..ProgramMetadata::default()
    }
    .encode();
    program.extend_from_slice(&scall(syscalls::SYSCALL_GET_PRIVATE_INPUT).to_le_bytes());
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut vm = IVM::new(10_000);
    vm.set_host(DefaultHost::with_private_inputs(vec![42]));
    vm.load_program(&program).expect("load non-ZK program");
    vm.set_register(10, 0);

    let error = vm
        .run()
        .expect_err("non-ZK execution must not consume private input");

    assert!(matches!(error, VMError::PrivacyViolation));
    assert_eq!(vm.register(10), 0);
    assert!(!vm.registers.tag(10));
}

#[test]
fn private_input_cannot_flow_directly_to_public_syscall_sinks() {
    for sink in [
        syscalls::SYSCALL_USE_NULLIFIER,
        syscalls::SYSCALL_DEBUG_PRINT,
        syscalls::SYSCALL_INPUT_PUBLISH_TLV,
    ] {
        let program = raw_zk_program(&[scall(syscalls::SYSCALL_GET_PRIVATE_INPUT), scall(sink)]);
        let mut vm = IVM::new(10_000);
        vm.set_host(DefaultHost::with_private_inputs(vec![42]));
        vm.load_program(&program).unwrap();
        vm.set_register(10, 0);

        let error = vm
            .run()
            .expect_err("private public-sink argument must trap");
        assert!(matches!(error, VMError::PrivacyViolation), "sink {sink:#x}");
    }
}

#[test]
fn valcom_declassifies_matching_private_operands() {
    let program = raw_zk_program(&[
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 10, 0, 0),
        scall(syscalls::SYSCALL_GET_PRIVATE_INPUT),
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 10, 0),
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 10, 0, 1),
        scall(syscalls::SYSCALL_GET_PRIVATE_INPUT),
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 2, 10, 0),
        encoding::wide::encode_rr(instruction::wide::crypto::VALCOM, 3, 1, 2),
    ]);
    let mut vm = IVM::new(10_000);
    vm.set_host(DefaultHost::with_private_inputs(vec![7, 11]));
    vm.load_program(&program).unwrap();

    vm.run().expect("private commitment should run");

    assert_eq!(vm.register(3), pedersen_commit_truncated(7, 11));
    assert!(!vm.registers.tag(3));
}

#[test]
fn valcom_rejects_mixed_public_and_private_operands() {
    let program = raw_zk_program(&[encoding::wide::encode_rr(
        instruction::wide::crypto::VALCOM,
        3,
        1,
        2,
    )]);
    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(1, 7);
    vm.set_register(2, 11);
    vm.registers.set_tag(1, true);

    let error = vm.run().expect_err("mixed commitment tags must trap");
    assert!(matches!(error, VMError::PrivacyViolation));
}

#[test]
fn elliptic_curve_operations_propagate_matching_private_tags() {
    for opcode in [
        instruction::wide::crypto::ECADD,
        instruction::wide::crypto::ECMUL_VAR,
        instruction::wide::crypto::PAIRING,
    ] {
        let program = raw_zk_program(&[encoding::wide::encode_rr(opcode, 3, 1, 2)]);
        let mut vm = IVM::new(10_000);
        vm.load_program(&program).unwrap();
        vm.set_register(1, 7);
        vm.set_register(2, 11);
        vm.registers.set_tag(1, true);
        vm.registers.set_tag(2, true);

        vm.run()
            .unwrap_or_else(|error| panic!("private EC opcode {opcode:#x} failed: {error}"));

        assert!(
            vm.registers.tag(3),
            "EC opcode {opcode:#x} laundered a private result"
        );
    }
}

#[test]
fn elliptic_curve_operations_reject_mixed_visibility() {
    for opcode in [
        instruction::wide::crypto::ECADD,
        instruction::wide::crypto::ECMUL_VAR,
        instruction::wide::crypto::PAIRING,
    ] {
        let program = raw_zk_program(&[encoding::wide::encode_rr(opcode, 3, 1, 2)]);
        let mut vm = IVM::new(10_000);
        vm.load_program(&program).unwrap();
        vm.set_register(1, 7);
        vm.set_register(2, 11);
        vm.registers.set_tag(1, true);
        vm.registers.set_tag(2, false);

        let error = vm
            .run()
            .expect_err("mixed public/private EC operands must trap");
        assert!(
            matches!(error, VMError::PrivacyViolation),
            "EC opcode {opcode:#x} returned {error}"
        );
    }
}

#[test]
fn elliptic_curve_results_cannot_reach_public_syscall_sinks() {
    for opcode in [
        instruction::wide::crypto::ECADD,
        instruction::wide::crypto::ECMUL_VAR,
        instruction::wide::crypto::PAIRING,
    ] {
        let program = raw_zk_program(&[
            encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 10, 0, 0),
            scall(syscalls::SYSCALL_GET_PRIVATE_INPUT),
            encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 10, 0),
            encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 2, 10, 0),
            encoding::wide::encode_rr(opcode, 10, 1, 2),
            scall(syscalls::SYSCALL_DEBUG_PRINT),
        ]);
        let mut vm = IVM::new(10_000);
        vm.set_host(DefaultHost::with_private_inputs(vec![7]));
        vm.load_program(&program).unwrap();

        let error = vm
            .run()
            .expect_err("an EC-derived secret must not reach a public syscall");
        assert!(
            matches!(error, VMError::PrivacyViolation),
            "EC opcode {opcode:#x} returned {error}"
        );
    }
}

#[test]
fn zk_field_operations_propagate_private_tags_and_reject_mixed_visibility() {
    for opcode in [
        instruction::wide::zk::FADD,
        instruction::wide::zk::FSUB,
        instruction::wide::zk::FMUL,
    ] {
        let program = raw_zk_program(&[encoding::wide::encode_rr(opcode, 3, 1, 2)]);
        let mut private_vm = IVM::new(10_000);
        private_vm.load_program(&program).unwrap();
        private_vm.set_register(1, 7);
        private_vm.set_register(2, 11);
        private_vm.registers.set_tag(1, true);
        private_vm.registers.set_tag(2, true);
        private_vm
            .run()
            .unwrap_or_else(|error| panic!("private field opcode {opcode:#x} failed: {error}"));
        assert!(
            private_vm.registers.tag(3),
            "field opcode {opcode:#x} laundered a private result"
        );

        let mut mixed_vm = IVM::new(10_000);
        mixed_vm.load_program(&program).unwrap();
        mixed_vm.set_register(1, 7);
        mixed_vm.set_register(2, 11);
        mixed_vm.registers.set_tag(1, true);
        let error = mixed_vm.run().expect_err("mixed field operands must trap");
        assert!(matches!(error, VMError::PrivacyViolation));
    }
}

#[test]
fn zk_field_inverse_preserves_private_visibility() {
    let program = raw_zk_program(&[encoding::wide::encode_rr(
        instruction::wide::zk::FINV,
        3,
        1,
        0,
    )]);
    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(1, 7);
    vm.registers.set_tag(1, true);

    vm.run().expect("private field inverse");
    assert!(vm.registers.tag(3));
}

#[test]
fn zk_field_results_cannot_reach_public_syscall_sinks() {
    let program = raw_zk_program(&[
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 10, 0, 0),
        scall(syscalls::SYSCALL_GET_PRIVATE_INPUT),
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 10, 0),
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 2, 10, 0),
        encoding::wide::encode_rr(instruction::wide::zk::FADD, 10, 1, 2),
        scall(syscalls::SYSCALL_DEBUG_PRINT),
    ]);
    let mut vm = IVM::new(10_000);
    vm.set_host(DefaultHost::with_private_inputs(vec![7]));
    vm.load_program(&program).unwrap();

    let error = vm
        .run()
        .expect_err("field-derived secret must not reach a public syscall");
    assert!(matches!(error, VMError::PrivacyViolation));
}

#[test]
fn pubkgen_explicitly_declassifies_its_output() {
    let program = raw_zk_program(&[encoding::wide::encode_rr(
        instruction::wide::crypto::PUBKGEN,
        3,
        1,
        0,
    )]);
    let mut vm = IVM::new(10_000);
    vm.load_program(&program).unwrap();
    vm.set_register(1, 7);
    vm.registers.set_tag(1, true);
    vm.registers.set_tag(3, true);

    vm.run().expect("public-key derivation should run");

    assert!(!vm.registers.tag(3));
}

#[test]
fn private_stack_spill_cannot_launder_a_syscall_argument() {
    let program = raw_zk_program(&[
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 10, 0, 0),
        scall(syscalls::SYSCALL_GET_PRIVATE_INPUT),
        encoding::wide::encode_store(instruction::wide::memory::STORE64, 1, 10, 0),
        encoding::wide::encode_load(instruction::wide::memory::LOAD64, 10, 1, 0),
        scall(syscalls::SYSCALL_USE_NULLIFIER),
    ]);
    let mut vm = IVM::new(10_000);
    vm.set_host(DefaultHost::with_private_inputs(vec![42]));
    vm.load_program(&program).unwrap();
    vm.set_register(1, Memory::STACK_START);

    let error = vm
        .run()
        .expect_err("a stack roundtrip must preserve the private tag");

    assert!(matches!(error, VMError::PrivacyViolation));
    assert!(vm.registers.tag(10));
}

#[test]
fn private_stack_envelope_cannot_be_published_through_a_public_pointer() {
    let program = raw_zk_program(&[
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 10, 0, 0),
        scall(syscalls::SYSCALL_GET_PRIVATE_INPUT),
        encoding::wide::encode_store(instruction::wide::memory::STORE64, 1, 10, 8),
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 10, 1, 0),
        scall(syscalls::SYSCALL_INPUT_PUBLISH_TLV),
    ]);
    let mut vm = IVM::new(10_000);
    vm.set_host(DefaultHost::with_private_inputs(vec![42]));
    vm.load_program(&program).expect("load ZK program");

    let payload = [0_u8; 16];
    let mut envelope = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    envelope.extend_from_slice(&(PointerType::Blob as u16).to_be_bytes());
    envelope.push(1);
    envelope.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    envelope.extend_from_slice(&payload);
    let hash: [u8; Hash::LENGTH] = Hash::new(payload).into();
    envelope.extend_from_slice(&hash);
    vm.store_bytes(Memory::STACK_START, &envelope)
        .expect("seed public stack envelope");
    vm.set_register(1, Memory::STACK_START);

    let error = vm
        .run()
        .expect_err("private envelope bytes must not cross the public host boundary");
    assert!(matches!(error, VMError::PrivacyViolation));
    assert!(
        !vm.registers.tag(10),
        "the pointer register itself is public"
    );
}

#[test]
fn private_store_outside_the_stack_is_rejected() {
    let program = raw_zk_program(&[
        encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 10, 0, 0),
        scall(syscalls::SYSCALL_GET_PRIVATE_INPUT),
        encoding::wide::encode_store(instruction::wide::memory::STORE64, 1, 10, 0),
    ]);
    let mut vm = IVM::new(10_000);
    vm.set_host(DefaultHost::with_private_inputs(vec![42]));
    vm.load_program(&program).unwrap();
    vm.set_register(1, Memory::OUTPUT_START);

    let error = vm
        .run()
        .expect_err("private output and heap stores must trap");

    assert!(matches!(error, VMError::PrivacyViolation));
}

#[test]
fn partial_public_overwrite_does_not_declassify_a_private_stack_word() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, Memory::STACK_START);
    vm.set_register(2, 0xCAFE_BABE_DEAD_BEEF);
    vm.registers.set_tag(2, true);
    vm.execute_instruction(Instruction::Store {
        rs: 2,
        addr_reg: 1,
        offset: 0,
    })
    .unwrap();
    vm.store_u32(Memory::STACK_START, 0).unwrap();

    let error = vm
        .execute_instruction(Instruction::Load {
            rd: 3,
            addr_reg: 1,
            offset: 0,
        })
        .expect_err("mixed-visibility words must trap");

    assert!(matches!(error, VMError::PrivacyViolation));
}

#[test]
fn reset_scrubs_private_stack_spills() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, Memory::STACK_START);
    vm.set_register(2, 0xCAFE_BABE_DEAD_BEEF);
    vm.registers.set_tag(2, true);
    vm.execute_instruction(Instruction::Store {
        rs: 2,
        addr_reg: 1,
        offset: 0,
    })
    .unwrap();

    vm.reset();

    assert_eq!(vm.load_u64(Memory::STACK_START).unwrap(), 0);
}

#[test]
fn disabling_zk_mode_scrubs_private_stack_spills() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, Memory::STACK_START);
    vm.set_register(2, 0xCAFE_BABE_DEAD_BEEF);
    vm.registers.set_tag(2, true);
    vm.execute_instruction(Instruction::Store {
        rs: 2,
        addr_reg: 1,
        offset: 0,
    })
    .unwrap();

    vm.set_zk_mode(false);

    assert_eq!(vm.load_u64(Memory::STACK_START).unwrap(), 0);
}

#[test]
fn runtime_template_restores_private_stack_tags_with_their_bytes() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_zk_mode(true);
    vm.set_register(1, Memory::STACK_START);
    vm.set_register(2, 0xCAFE_BABE_DEAD_BEEF);
    vm.registers.set_tag(2, true);
    vm.execute_instruction(Instruction::Store {
        rs: 2,
        addr_reg: 1,
        offset: 0,
    })
    .unwrap();
    let template = vm.runtime_template();
    vm.store_u64(Memory::STACK_START, 0).unwrap();

    vm.reset_from_runtime_template(&template);
    vm.set_register(1, Memory::STACK_START);
    vm.execute_instruction(Instruction::Load {
        rd: 3,
        addr_reg: 1,
        offset: 0,
    })
    .unwrap();

    assert_eq!(vm.register(3), 0xCAFE_BABE_DEAD_BEEF);
    assert!(vm.registers.tag(3));
}
