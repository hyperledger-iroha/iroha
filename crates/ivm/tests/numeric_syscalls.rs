//! Tests for Numeric syscall behavior.

use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;
use ivm::host::IVMHost;
use ivm::{CoreHost, IVM, PointerType, VMError, host::DefaultHost, syscalls};

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
    let h: [u8; 32] = Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn make_numeric_tlv(value: Numeric) -> Vec<u8> {
    let payload = norito::to_bytes(&value).expect("encode numeric");
    make_tlv(PointerType::NoritoBytes as u16, &payload)
}

fn decode_numeric(vm: &IVM, ptr: u64) -> Numeric {
    let tlv = vm.memory.validate_tlv(ptr).expect("validate tlv");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    norito::decode_from_bytes(tlv.payload).expect("decode numeric")
}

#[test]
fn numeric_from_int_rejects_negative() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    vm.set_register(10, (-12_i64) as u64);
    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_FROM_INT, &mut vm)
        .expect_err("negative numeric from int should fail");
    assert!(matches!(err, VMError::AssertionFailed));
}

#[test]
fn numeric_from_int_rejects_negative_core_host() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = CoreHost::new();

    vm.set_register(10, (-12_i64) as u64);
    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_FROM_INT, &mut vm)
        .expect_err("negative numeric from int should fail (core host)");
    assert!(matches!(err, VMError::AssertionFailed));
}

#[test]
fn numeric_from_int_encodes_i64() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    vm.set_register(10, 42_u64);
    host.syscall(syscalls::SYSCALL_NUMERIC_FROM_INT, &mut vm)
        .expect("numeric from int");

    let out = decode_numeric(&vm, vm.register(10));
    assert_eq!(out, Numeric::new(42_i64, 0));
}

#[test]
fn numeric_to_int_rejects_negative() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    let value = Numeric::new(-1_i32, 0);
    let ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(value))
        .expect("alloc numeric");
    vm.set_register(10, ptr);

    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_TO_INT, &mut vm)
        .expect_err("negative numeric to int should fail");
    assert!(matches!(err, VMError::AssertionFailed));
}

#[test]
fn numeric_to_int_rejects_fractional() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    let value = Numeric::new(15_u32, 1); // 1.5
    let ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(value))
        .expect("alloc numeric");
    vm.set_register(10, ptr);

    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_TO_INT, &mut vm)
        .expect_err("fractional to int should fail");
    assert!(matches!(err, VMError::AssertionFailed));
}

#[test]
fn numeric_to_int_rejects_overflow() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    let value = Numeric::new(i128::from(i64::MAX) + 1, 0);
    let ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(value))
        .expect("alloc numeric");
    vm.set_register(10, ptr);

    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_TO_INT, &mut vm)
        .expect_err("overflow to int should fail");
    assert!(matches!(err, VMError::AssertionFailed));

    let value = Numeric::new(i128::from(i64::MIN) - 1, 0);
    let ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(value))
        .expect("alloc numeric");
    vm.set_register(10, ptr);

    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_TO_INT, &mut vm)
        .expect_err("negative overflow to int should fail");
    assert!(matches!(err, VMError::AssertionFailed));
}

#[test]
fn numeric_add_sub_mul_reject_fractional() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    let lhs = Numeric::new(15_i32, 1); // 1.5
    let rhs = Numeric::new(25_i32, 2); // 0.25
    let lhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(lhs))
        .expect("alloc lhs");
    let rhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(rhs))
        .expect("alloc rhs");

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_ADD, &mut vm)
        .expect_err("numeric add should reject fractional");
    assert!(matches!(err, VMError::AssertionFailed));

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_SUB, &mut vm)
        .expect_err("numeric sub should reject fractional");
    assert!(matches!(err, VMError::AssertionFailed));

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_MUL, &mut vm)
        .expect_err("numeric mul should reject fractional");
    assert!(matches!(err, VMError::AssertionFailed));
}

#[test]
fn numeric_sub_rejects_underflow() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    let lhs = Numeric::new(1_u32, 0);
    let rhs = Numeric::new(2_u32, 0);
    let lhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(lhs))
        .expect("alloc lhs");
    let rhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(rhs))
        .expect("alloc rhs");

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_SUB, &mut vm)
        .expect_err("numeric sub underflow should fail");
    assert!(matches!(err, VMError::AssertionFailed));
}

#[test]
fn numeric_div_rem_zero_rejected() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    let lhs = Numeric::new(10_u32, 0);
    let rhs = Numeric::new(0_u32, 0);
    let lhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(lhs))
        .expect("alloc lhs");
    let rhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(rhs))
        .expect("alloc rhs");

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_DIV, &mut vm)
        .expect_err("div by zero should fail");
    assert!(matches!(err, VMError::AssertionFailed));

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_REM, &mut vm)
        .expect_err("rem by zero should fail");
    assert!(matches!(err, VMError::AssertionFailed));
}

#[test]
fn numeric_div_rem_outputs_expected() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    let lhs = Numeric::new(10_u32, 0);
    let rhs = Numeric::new(3_u32, 0);
    let lhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(lhs))
        .expect("alloc lhs");
    let rhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(rhs))
        .expect("alloc rhs");

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    host.syscall(syscalls::SYSCALL_NUMERIC_DIV, &mut vm)
        .expect("numeric div");
    let out = decode_numeric(&vm, vm.register(10));
    assert_eq!(out, Numeric::new(3_u32, 0));

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    host.syscall(syscalls::SYSCALL_NUMERIC_REM, &mut vm)
        .expect("numeric rem");
    let out = decode_numeric(&vm, vm.register(10));
    assert_eq!(out, Numeric::new(1_u32, 0));
}

#[test]
fn numeric_neg_rejects_nonzero() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    let value = Numeric::new(1_u32, 0);
    let value_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(value))
        .expect("alloc value");
    vm.set_register(10, value_ptr);
    let err = host
        .syscall(syscalls::SYSCALL_NUMERIC_NEG, &mut vm)
        .expect_err("numeric neg should reject non-zero");
    assert!(matches!(err, VMError::AssertionFailed));

    let zero = Numeric::new(0_u32, 0);
    let zero_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(zero))
        .expect("alloc zero");
    vm.set_register(10, zero_ptr);
    host.syscall(syscalls::SYSCALL_NUMERIC_NEG, &mut vm)
        .expect("numeric neg zero");
    let out = decode_numeric(&vm, vm.register(10));
    assert_eq!(out, Numeric::new(0_u32, 0));
}

#[test]
fn numeric_cmp_unsigned() {
    let mut vm = IVM::new(u64::MAX);
    let mut host = DefaultHost::new();

    let lhs = Numeric::new(5_i32, 0);
    let rhs = Numeric::new(7_i32, 0);
    let lhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(lhs))
        .expect("alloc lhs");
    let rhs_ptr = vm
        .alloc_input_tlv(&make_numeric_tlv(rhs))
        .expect("alloc rhs");

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    host.syscall(syscalls::SYSCALL_NUMERIC_LT, &mut vm)
        .expect("numeric lt");
    assert_eq!(vm.register(10), 1);

    vm.set_register(10, lhs_ptr);
    vm.set_register(11, rhs_ptr);
    host.syscall(syscalls::SYSCALL_NUMERIC_GT, &mut vm)
        .expect("numeric gt");
    assert_eq!(vm.register(10), 0);
}
