//! Regression coverage for the permanently retired decimal-i64 map-key helper.

use ivm::{CoreHost, IVM, VMError, encoding, syscalls};
mod common;

#[test]
fn decimal_i64_map_key_helper_is_not_part_of_abi_v1() {
    let retired = syscalls::RETIRED_SYSCALL_BUILD_PATH_MAP_KEY;
    assert_eq!(retired, 0x54);
    assert!(!syscalls::is_syscall_allowed(
        ivm::SyscallPolicy::AbiV1,
        retired
    ));
    assert_eq!(syscalls::registered_syscall_access(retired), None);
    assert_eq!(syscalls::syscall_name(retired), None);
    assert!(!syscalls::abi_syscall_list().contains(&retired));
    assert_eq!(
        ivm::host::registered_host_syscall_gas_formula(retired),
        None
    );

    let mut code = Vec::new();
    code.extend_from_slice(
        &encoding::wide::encode_sys(
            ivm::instruction::wide::system::SCALL,
            u8::try_from(retired).expect("retired syscall fits compact encoding"),
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());

    let mut admitted = IVM::new(u64::MAX);
    admitted.set_host(CoreHost::new());
    assert_eq!(
        admitted.load_program(&common::assemble(&code)),
        Err(VMError::UnknownSyscall(retired)),
        "metadata-bearing programs reject the retired number during admission"
    );

    let mut raw = IVM::new(u64::MAX);
    raw.set_host(CoreHost::new());
    raw.load_code(&code)
        .expect("load raw retired-syscall fixture");
    assert_eq!(raw.run(), Err(VMError::UnknownSyscall(retired)));
}
