use ivm::{CoreHost, IVM, PointerType, encoding, syscalls};

mod common;

fn tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
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
fn name_decode_rejects_invalid_utf8() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    // Build NoritoBytes TLV with invalid UTF-8 (0xFF)
    let bad = [0xFFu8, 0xFE, 0xFF];
    let p_nb = vm
        .alloc_input_tlv(&tlv(PointerType::NoritoBytes, &bad))
        .unwrap();
    // Program: publish TLV; NAME_DECODE; HALT
    let prog = common::assemble(
        &[
            encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                syscalls::SYSCALL_NAME_DECODE as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_nb);
    vm.load_program(&prog).unwrap();
    let err = vm.run().expect_err("expected invalid UTF-8 to be rejected");
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn name_decode_rejects_invalid_name_utf8_valid() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    // Valid UTF-8 but invalid Name (contains space)
    let bad = b"bad name";
    let p_nb = vm
        .alloc_input_tlv(&tlv(PointerType::NoritoBytes, bad))
        .unwrap();
    let prog = common::assemble(
        &[
            encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                syscalls::SYSCALL_NAME_DECODE as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_nb);
    vm.load_program(&prog).unwrap();
    let err = vm.run().expect_err("expected invalid Name to be rejected");
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn name_decode_rejects_reserved_chars() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    // Valid UTF-8 but invalid Name (reserved delimiter)
    let bad = b"alice@wonderland";
    let p_nb = vm
        .alloc_input_tlv(&tlv(PointerType::NoritoBytes, bad))
        .unwrap();
    let prog = common::assemble(
        &[
            encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                syscalls::SYSCALL_NAME_DECODE as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_nb);
    vm.load_program(&prog).unwrap();
    let err = vm
        .run()
        .expect_err("expected reserved chars to be rejected");
    assert!(matches!(err, ivm::VMError::NoritoInvalid));
}

#[test]
fn name_decode_accepts_valid_utf8() {
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    let good = b"wonderland";
    let p_nb = vm
        .alloc_input_tlv(&tlv(PointerType::NoritoBytes, good))
        .unwrap();
    let prog = common::assemble(
        &[
            encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                syscalls::SYSCALL_INPUT_PUBLISH_TLV as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_sys(
                ivm::instruction::wide::system::SCALL,
                syscalls::SYSCALL_NAME_DECODE as u8,
            )
            .to_le_bytes(),
            encoding::wide::encode_halt().to_le_bytes(),
        ]
        .concat(),
    );
    vm.set_register(10, p_nb);
    vm.load_program(&prog).unwrap();
    vm.run().unwrap();
    let p = vm.register(10);
    let tlv_name = vm.memory.validate_tlv(p).unwrap();
    assert_eq!(tlv_name.type_id, PointerType::Name);
    assert_eq!(
        core::str::from_utf8(tlv_name.payload).unwrap(),
        "wonderland"
    );
}
