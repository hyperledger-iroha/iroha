//! ABI v1 host syscall coverage checks.

use std::{collections::HashMap, str::FromStr};

use iroha_crypto::PublicKey;
use ivm::{
    CoreHost, IVM, IVMHost, VMError, gas,
    host::DefaultHost,
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
    syscalls,
};

fn sample_account() -> AccountId {
    let public_key = PublicKey::from_str(
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    )
    .expect("sample public key parses");
    AccountId::new(public_key)
}

fn wsv_host() -> WsvHost {
    WsvHost::new_with_subject(MockWorldStateView::new(), sample_account(), HashMap::new())
}

fn assert_not_unknown_syscall(result: Result<u64, VMError>, host_name: &str, number: u32) {
    if let Err(error) = result {
        if let VMError::UnknownSyscall(actual) = error.as_unmetered() {
            panic!(
                "{host_name} returned UnknownSyscall({actual:#x}) for ABI v1 syscall {number:#x}"
            );
        }
    }
}

fn assert_host_covers_abi<H>(host_name: &str, make_host: impl Fn() -> H)
where
    H: IVMHost,
{
    let mut vm = IVM::new(u64::MAX);
    for &number in syscalls::abi_syscall_list() {
        let mut host = make_host();
        for reg in 10..=15 {
            vm.set_register(reg, 0);
        }
        let result = host.syscall(number, &mut vm);
        assert_not_unknown_syscall(result, host_name, number);
    }
}

fn assert_fastpq_scope_gas<H>(mut host: H)
where
    H: IVMHost,
{
    let mut vm = IVM::new(u64::MAX);
    assert_eq!(
        host.syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_BEGIN, &mut vm),
        Ok(gas::G_FASTPQ_BATCH)
    );

    let error = host
        .syscall(syscalls::SYSCALL_TRANSFER_V1_BATCH_END, &mut vm)
        .expect_err("empty FastPQ batch is rejected after fixed scope gas");
    assert_eq!(error.metered_gas(), Some(gas::G_FASTPQ_BATCH));
    assert!(matches!(error.as_unmetered(), VMError::DecodeError));
}

#[test]
fn abi_v1_allowed_syscalls_are_covered_by_lightweight_hosts() {
    assert_host_covers_abi("DefaultHost", DefaultHost::new);
    assert_host_covers_abi("CoreHost", CoreHost::new);
    assert_host_covers_abi("WsvHost", wsv_host);
}

#[test]
fn fastpq_batch_scope_syscalls_charge_fixed_gas_in_all_hosts() {
    assert_fastpq_scope_gas(DefaultHost::new());
    assert_fastpq_scope_gas(CoreHost::new());
    assert_fastpq_scope_gas(wsv_host());
}
