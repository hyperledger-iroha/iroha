use ivm::{IVM, ProgramMetadata, cost_of as cost_of_opt, encoding, instruction};

const HALT_WORD: u32 = encoding::wide::encode_halt();

fn cost_of(word: u32) -> u64 {
    cost_of_opt(word).expect("valid opcode must have gas cost")
}

fn assemble_words(words: &[u32]) -> Vec<u8> {
    let mut bytes = header();
    for &word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn header() -> Vec<u8> {
    ProgramMetadata::default().encode()
}

#[test]
fn nested_branches_executed_set_gas() {
    // Program:
    // addi x1, x0, 1
    // addi x2, x0, 1
    // beq x1,x2, +8   ; skip over next jmp if equal
    // jmp +4          ; else path
    // addi x3, x0, 7  ; then-path single op
    // halt
    let a1 = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 1, 0, 1);
    let a2 = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 2, 0, 1);
    let beq = encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 2, 2);
    let jmp = encoding::wide::encode_jump(instruction::wide::control::JMP, 0, 2);
    let then = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 3, 0, 7);
    let halt = HALT_WORD;
    let bytes = assemble_words(&[a1, a2, beq, jmp, then, halt]);
    let mut vm = IVM::new(10_000);
    vm.load_program(&bytes).unwrap();
    let _ = vm.run();
    // Expected executed-set when equal: addi, addi, beq, then, halt
    let executed = [a1, a2, beq, then, halt];
    let expected: u64 = executed.iter().map(|w| cost_of(*w)).sum();
    assert_eq!(10_000 - vm.remaining_gas(), expected);
}

#[test]
fn sha3_error_path_consumes_base_gas() {
    // Craft a SHA3BLOCK with out-of-bounds pointers to trigger error after gas deduction.
    // rd=x4(rslt ptr), rs1=&state, rs2=&block
    let sha3 = encoding::wide::encode_rr(instruction::wide::crypto::SHA3BLOCK, 4, 10, 11);
    let bytes = assemble_words(&[sha3, HALT_WORD]);
    let mut vm = IVM::new(1000);
    vm.load_program(&bytes).unwrap();
    // Set rs1/rs2 to near the end of INPUT so 136/200 byte reads go OOB
    let st_ptr = ivm::Memory::INPUT_START + ivm::Memory::INPUT_SIZE - 8;
    let blk_ptr = ivm::Memory::INPUT_START + ivm::Memory::INPUT_SIZE - 4;
    vm.set_register(10, st_ptr);
    vm.set_register(11, blk_ptr);
    let res = vm.run();
    assert!(res.is_err());
    // Gas consumed should be at least the base cost of SHA3BLOCK
    let used = 1000 - vm.remaining_gas();
    assert!(used >= cost_of(sha3));
}

#[test]
fn aesenc_mode_disabled_consumes_base_gas() {
    // AESENC with VECTOR bit disabled should trap with VectorExtensionDisabled after base gas is consumed.
    let aes = encoding::wide::encode_rr(instruction::wide::crypto::AESENC, 1, 2, 3);
    let bytes = assemble_words(&[aes, HALT_WORD]);
    let mut vm = IVM::new(1000);
    vm.load_program(&bytes).unwrap();
    let res = vm.run();
    assert!(matches!(res, Err(ivm::VMError::VectorExtensionDisabled)));
    let used = 1000 - vm.remaining_gas();
    assert!(used >= cost_of(aes));
}
