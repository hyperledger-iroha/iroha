use ivm::{IVM, IVMHost, Memory, ProgramMetadata, VMError, encoding, instruction};
use std::any::Any;
#[test]
fn unknown_syscall_traps_under_default_policy() {
    // Build a small program with an unknown syscall; policy dispatch should reject it.
    let syscall = 0xDF;
    let word = encoding::wide::encode_scall(u32::from(syscall));
    let mut code = Vec::new();
    code.extend_from_slice(&word.to_le_bytes());
    // HALT to terminate if SCALL is skipped (it shouldn't be)
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut vm = IVM::new(u64::MAX);
    vm.load_code(&code).unwrap();
    match vm.run() {
        Err(VMError::UnknownSyscall(n)) => assert_eq!(n, syscall as u32),
        other => panic!("expected UnknownSyscall(0xDF), got {other:?}"),
    }
}
#[test]
fn allowed_syscall_alloc_succeeds() {
    // SCALL 0xF0 (ALLOC) is allowed; x10 holds size, and host writes pointer back to x10
    let word = encoding::wide::encode_scall(0xF0);
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
struct PermissiveHost {
    called: bool,
}
impl IVMHost for PermissiveHost {
    fn prepare_syscall(&self, _number: u32, _vm: &IVM) -> Result<u64, VMError> {
        Ok(0)
    }
    fn syscall(&mut self, _number: u32, _vm: &mut IVM) -> Result<u64, VMError> {
        self.called = true;
        Ok(0)
    }
    fn as_any(&mut self) -> &mut dyn Any
    where
        Self: 'static,
    {
        self
    }
}
#[test]
fn program_admission_enforces_syscall_policy_before_host_dispatch() {
    let scall = encoding::wide::encode_scall(0xDF);
    let mut program = ProgramMetadata::default().encode();
    for word in [scall, encoding::wide::encode_halt()] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    let mut vm = IVM::new(10_000);
    vm.set_host(PermissiveHost { called: false });
    assert!(
        matches!(vm.load_program(&program), Err(VMError::UnknownSyscall(__actual_error_field)) if __actual_error_field == (0xDF))
    );
    let host = vm
        .host_mut_any()
        .expect("permissive host remains attached")
        .downcast_mut::<PermissiveHost>()
        .expect("permissive host type");
    assert!(!host.called);
}
