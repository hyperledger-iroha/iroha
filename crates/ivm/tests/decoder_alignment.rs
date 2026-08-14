use ivm::{IVM, ProgramMetadata, VMError, decode, encoding};
#[test]
fn decoder_traps_on_misaligned_fetch() {
    // Minimal program: valid header + a single 32-bit HALT instruction
    let mut program = ProgramMetadata::default_for(1, 0, 1).encode();
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut vm = IVM::new(10_000);
    vm.load_program(&program).expect("load program");
    // A properly aligned fetch at PC=0 should succeed
    let (_op, len) = decode(&vm.memory, 0).expect("aligned decode");
    assert_eq!(len, 4);
    // Misaligned PC=2 must yield a memory access violation because instructions are 4-byte aligned
    let err = decode(&vm.memory, 2).expect_err("misaligned decode must fail");
    assert!(matches!(err, VMError::MemoryAccessViolation { .. }));
}
