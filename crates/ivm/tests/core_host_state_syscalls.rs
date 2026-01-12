//! CoreHost durable state syscalls: STATE_GET/SET/DEL with pointer-ABI.

use ivm::{CoreHost, IVM, Memory, PointerType, VMError, encoding, instruction, syscalls};
mod common;

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload);
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    v.extend_from_slice(&h);
    v
}

#[test]
fn core_host_state_set_get_del_roundtrip() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());

    // Allocate TLVs for path and value in INPUT
    let path_tlv = make_tlv(PointerType::Name, b"foo");
    let val1 = vec![1u8, 2, 3, 4];
    let val1_tlv = make_tlv(PointerType::NoritoBytes, &val1);
    let p_path = vm.alloc_input_tlv(&path_tlv).expect("alloc path");
    let p_val1 = vm.alloc_input_tlv(&val1_tlv).expect("alloc val1");

    // Build program: SCALL STATE_SET; HALT
    let mut set_prog = Vec::new();
    let set_sys = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        syscalls::SYSCALL_STATE_SET as u8,
    );
    set_prog.extend_from_slice(&set_sys.to_le_bytes());
    set_prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let set_prog = common::assemble(&set_prog);
    vm.set_register(10, p_path);
    vm.set_register(11, p_val1);
    vm.load_program(&set_prog).expect("load set");
    vm.run().expect("state set");

    // GET program: r10 = path; SCALL GET; value returned in r10 (pointer or 0)
    let mut get_prog = Vec::new();
    let get_sys = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        syscalls::SYSCALL_STATE_GET as u8,
    );
    get_prog.extend_from_slice(&get_sys.to_le_bytes());
    get_prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let get_prog = common::assemble(&get_prog);
    vm.set_register(10, p_path);
    vm.load_program(&get_prog).expect("load get");
    vm.run().expect("state get");
    let p_out = vm.register(10);
    assert!((Memory::INPUT_START..Memory::INPUT_START + Memory::INPUT_SIZE).contains(&p_out));
    let tlv = vm.memory.validate_tlv(p_out).expect("validate out");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    assert_eq!(tlv.payload, &val1[..]);

    // DEL program: r10=path; SCALL DEL; HALT
    let mut del_prog = Vec::new();
    let del_sys = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        syscalls::SYSCALL_STATE_DEL as u8,
    );
    del_prog.extend_from_slice(&del_sys.to_le_bytes());
    del_prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let del_prog = common::assemble(&del_prog);
    vm.set_register(10, p_path);
    vm.load_program(&del_prog).expect("load del");
    vm.run().expect("state del");

    // GET again -> expect r10 = 0
    vm.set_register(10, p_path);
    vm.load_program(&get_prog).expect("load get again");
    vm.run().expect("state get after del");
    assert_eq!(vm.register(10), 0);
}

#[test]
fn core_host_state_syscalls_require_pointers() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());

    let get_prog = common::assemble_syscalls(&[syscalls::SYSCALL_STATE_GET as u8]);
    vm.set_register(10, 0);
    vm.load_program(&get_prog).expect("load get");
    let err = vm.run().expect_err("state get without path should fail");
    assert!(matches!(err, VMError::NoritoInvalid));

    let path_tlv = make_tlv(PointerType::Name, b"foo");
    let p_path = vm.alloc_input_tlv(&path_tlv).expect("alloc path");
    let set_prog = common::assemble_syscalls(&[syscalls::SYSCALL_STATE_SET as u8]);
    vm.set_register(10, p_path);
    vm.set_register(11, 0);
    vm.load_program(&set_prog).expect("load set");
    let err = vm.run().expect_err("state set without value should fail");
    assert!(matches!(err, VMError::NoritoInvalid));

    let del_prog = common::assemble_syscalls(&[syscalls::SYSCALL_STATE_DEL as u8]);
    vm.set_register(10, 0);
    vm.load_program(&del_prog).expect("load del");
    let err = vm.run().expect_err("state del without path should fail");
    assert!(matches!(err, VMError::NoritoInvalid));
}

#[test]
fn core_host_debug_log_accepts_json() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());

    let tlv = make_tlv(PointerType::Json, br#"{"msg":"hello"}"#);
    let ptr = vm.alloc_input_tlv(&tlv).expect("alloc tlv");
    vm.set_register(10, ptr);

    let prog = common::assemble_syscalls(&[syscalls::SYSCALL_DEBUG_LOG as u8]);
    vm.load_program(&prog).expect("load program");
    vm.run().expect("debug log should succeed");
}
