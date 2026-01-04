use ivm::{IVM, ProgramMetadata, VMError, encoding, instruction};

const CLASSIC_ARITH_IMM: u8 = 0x13;

fn encode_li16(op: u8, rd: u8, imm8: i8) -> u16 {
    ((op & 0xF) as u16) << 12 | ((rd & 0xF) as u16) << 8 | (imm8 as u8 as u16)
}

#[allow(dead_code)]
fn program_with(instrs: &[u32]) -> Vec<u8> {
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let mut bytes = meta.encode();
    for w in instrs {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    bytes
}

#[test]
fn jalr_alignment_evenizes_target() {
    // x1 (RA) <- pc+4; pc <- (x2 + imm) & !1
    // Setup: load x2 with odd address by using ADDI (classic compatibility encoding)
    let mut code: Vec<u8> = Vec::new();
    // addi x2, x0, 3
    let addi = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 2, 0, 3);
    code.extend_from_slice(&addi.to_le_bytes());
    // jalr x3, x2, 5  => target = 3 + 5 = 8 -> evenized to 8
    let jalr = encoding::wide::encode_ri(instruction::wide::control::JALR, 3, 2, 5);
    code.extend_from_slice(&jalr.to_le_bytes());
    // halt
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    let mut vm = IVM::new(100);
    vm.load_program(&prog).unwrap();
    let res = vm.run();
    assert!(res.is_ok());
    // After JALR: target is evenized to 8, then HALT executes and advances PC by 4 → 12
    assert_eq!(vm.pc(), 12);
    // RA saved is implementation: return_addr = old_pc + 4 (here, pc after addi=4, so RA=8)
    assert_eq!(vm.register(3), 8);
}

#[test]
fn compressed_forms_are_rejected() {
    let mut prog = ProgramMetadata::default().encode();
    let li16 = encode_li16(CLASSIC_ARITH_IMM, 1, 7);
    prog.extend_from_slice(&li16.to_le_bytes());
    let mut vm = IVM::new(10);
    let err = vm
        .load_program(&prog)
        .expect_err("compressed forms must be rejected");
    assert!(matches!(err, VMError::MemoryAccessViolation { .. }));
}

#[test]
fn jal_skips_next_instruction() {
    // addi r7, r0, 1; jal x0, +2; addi r7, r0, 2; halt
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&ivm::kotodama::compiler::encode_addi(7, 0, 1).to_le_bytes());
    let jal = encoding::wide::encode_jump(instruction::wide::control::JAL, 0, 2);
    prog.extend_from_slice(&jal.to_le_bytes());
    prog.extend_from_slice(&ivm::kotodama::compiler::encode_addi(7, 0, 2).to_le_bytes());
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut vm = IVM::new(100);
    vm.load_program(&prog).unwrap();
    vm.run().expect("run");
    assert_eq!(vm.register(7), 1);
}

#[test]
fn jal_link_register_saved_correctly() {
    // addi r2,r0,0; jal r5,+2; addi r2,r0,7; halt
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&ivm::kotodama::compiler::encode_addi(2, 0, 0).to_le_bytes());
    let jal = encoding::wide::encode_jump(instruction::wide::control::JAL, 5, 2);
    prog.extend_from_slice(&jal.to_le_bytes());
    prog.extend_from_slice(&ivm::kotodama::compiler::encode_addi(2, 0, 7).to_le_bytes());
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut vm = IVM::new(100);
    vm.load_program(&prog).unwrap();
    vm.run().expect("run");
    // After the first 32-bit instruction, PC = 4. JAL stores PC+4 = 8 in r5.
    assert_eq!(vm.register(5), 8);
    // The addi after JAL is skipped, so r2 stays 0.
    assert_eq!(vm.register(2), 0);
}

#[test]
fn compressed_ebreak_rejected() {
    let mut prog = ProgramMetadata::default().encode();
    prog.extend_from_slice(&[0x02, 0x90]);
    let mut vm = IVM::new(10);
    let err = vm
        .load_program(&prog)
        .expect_err("compressed ebreak must be rejected");
    assert!(matches!(err, VMError::MemoryAccessViolation { .. }));
}
