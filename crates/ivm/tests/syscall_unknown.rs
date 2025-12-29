use ivm::{IVM, Memory, VMError, encoding, instruction};

#[test]
fn unknown_syscall_traps_under_default_policy() {
    // Build a small program with an unknown syscall; policy dispatch should reject it.
    let syscall = 0xAB;
    let word = encoding::wide::encode_sys(instruction::wide::system::SCALL, syscall);
    let mut code = Vec::new();
    code.extend_from_slice(&word.to_le_bytes());
    // HALT to terminate if SCALL is skipped (it shouldn't be)
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut vm = IVM::new(u64::MAX);
    vm.load_code(&code).unwrap();
    match vm.run() {
        Err(VMError::UnknownSyscall(n)) => assert_eq!(n, syscall as u32),
        other => panic!("expected UnknownSyscall(0xAB), got {other:?}"),
    }
}

#[test]
fn allowed_syscall_alloc_succeeds() {
    // SCALL 0xF0 (ALLOC) is allowed; x10 holds size, and host writes pointer back to x10
    let word = encoding::wide::encode_sys(instruction::wide::system::SCALL, 0xF0);
    let mut code = Vec::new();
    code.extend_from_slice(&word.to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut vm = IVM::new(u64::MAX);
    vm.set_register(10, 16);
    vm.load_code(&code).unwrap();
    let res = vm.run();
    assert!(res.is_ok(), "unexpected error: {res:?}");
    let ptr = vm.register(10);
    assert!(
        ptr >= Memory::HEAP_START,
        "alloc should return heap pointer, got {ptr:#x}"
    );
}
