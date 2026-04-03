//! WsvHost state overlay staging and commit/rollback behaviour.

use std::{
    collections::HashMap,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use ivm::{
    IVM, Memory, MockWorldStateView, PointerType, WsvHost, encoding, host::IVMHost, instruction,
    syscalls, validate_tlv_bytes,
};

mod common;

fn make_tlv(pty: PointerType, payload: &[u8]) -> Vec<u8> {
    let payload = common::payload_for_type(pty, payload);
    let mut v = Vec::with_capacity(7 + payload.len() + 32);
    v.extend_from_slice(&(pty as u16).to_be_bytes());
    v.push(1);
    v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    v.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    v.extend_from_slice(&h);
    v
}

fn decode_state_payload(ptr: u64, vm: &IVM) -> Vec<u8> {
    assert!(
        (Memory::INPUT_START..Memory::INPUT_START + Memory::INPUT_SIZE).contains(&ptr),
        "state TLV should live in INPUT: 0x{ptr:08x}"
    );
    let tlv = vm.memory.validate_tlv(ptr).expect("valid TLV pointer");
    assert_eq!(tlv.type_id, PointerType::NoritoBytes);
    tlv.payload.to_vec()
}

fn sample_account() -> ivm::mock_wsv::AccountId {
    let _domain: ivm::mock_wsv::DomainId =
        DomainId::try_new("wonderland", "universal").expect("domain id");
    ivm::mock_wsv::AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    )
}

fn set_and_get_program() -> Vec<u8> {
    let mut prog = Vec::new();
    let set_sys = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        syscalls::SYSCALL_STATE_SET as u8,
    );
    let get_sys = encoding::wide::encode_sys(
        instruction::wide::system::SCALL,
        syscalls::SYSCALL_STATE_GET as u8,
    );
    prog.extend_from_slice(&set_sys.to_le_bytes());
    prog.extend_from_slice(&get_sys.to_le_bytes());
    prog.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    common::assemble(&prog)
}

#[test]
fn overlay_stages_and_flushes_on_finish() {
    let p_path = make_tlv(PointerType::Name, b"counter");
    let p_val = make_tlv(PointerType::NoritoBytes, b"5");
    let program = set_and_get_program();

    let mut vm = IVM::new(u64::MAX);
    let host =
        WsvHost::new_with_subject(MockWorldStateView::new(), sample_account(), HashMap::new());
    vm.set_host(host);
    {
        let host = vm
            .host_mut_any()
            .expect("host present")
            .downcast_mut::<WsvHost>()
            .expect("WsvHost");
        IVMHost::begin_tx(host, &Default::default()).expect("begin_tx");
    }
    let p_path_ptr = vm.alloc_input_tlv(&p_path).expect("alloc path");
    let p_val_ptr = vm.alloc_input_tlv(&p_val).expect("alloc val");
    vm.set_register(10, p_path_ptr);
    vm.set_register(11, p_val_ptr);
    vm.load_program(&program).expect("load program");
    let res = vm.run();
    assert!(res.is_ok(), "execute overlay program: {res:?}");
    let value_ptr = vm.register(10);
    assert_eq!(decode_state_payload(value_ptr, &vm), b"5");
    {
        let host = vm
            .host_mut_any()
            .expect("host present")
            .downcast_mut::<WsvHost>()
            .expect("WsvHost");
        assert!(
            host.wsv.sc_get("counter").is_none(),
            "state should not flush before finish_tx"
        );
        IVMHost::finish_tx(host).expect("finish_tx");
        let stored = host
            .wsv
            .sc_get("counter")
            .expect("state flushed after finish_tx");
        let tlv = validate_tlv_bytes(&stored).expect("stored TLV");
        assert_eq!(tlv.payload, b"5");
    }
}

#[test]
fn overlay_restores_snapshot_on_rollback() {
    let p_path = make_tlv(PointerType::Name, b"counter");
    let initial = make_tlv(PointerType::NoritoBytes, b"1");
    let updated = make_tlv(PointerType::NoritoBytes, b"9");
    let program = set_and_get_program();

    let mut wsv = MockWorldStateView::new();
    wsv.sc_set("counter", initial).expect("seed durable state");

    let mut vm = IVM::new(u64::MAX);
    let host = WsvHost::new_with_subject(wsv, sample_account(), HashMap::new());
    vm.set_host(host);
    {
        let host = vm
            .host_mut_any()
            .expect("host present")
            .downcast_mut::<WsvHost>()
            .expect("WsvHost");
        IVMHost::begin_tx(host, &Default::default()).expect("begin_tx");
    }
    let snapshot = {
        let host = vm
            .host_mut_any()
            .expect("host present")
            .downcast_mut::<WsvHost>()
            .expect("WsvHost");
        host.checkpoint().expect("checkpoint captured")
    };

    let p_path_ptr = vm.alloc_input_tlv(&p_path).expect("alloc path");
    let p_val_ptr = vm.alloc_input_tlv(&updated).expect("alloc val");
    vm.set_register(10, p_path_ptr);
    vm.set_register(11, p_val_ptr);
    vm.load_program(&program).expect("load program");
    let res = vm.run();
    assert!(res.is_ok(), "execute overlay program: {res:?}");
    assert_eq!(decode_state_payload(vm.register(10), &vm), b"9");
    {
        let host = vm
            .host_mut_any()
            .expect("host present")
            .downcast_mut::<WsvHost>()
            .expect("WsvHost");
        assert!(host.restore(snapshot.as_ref()));
        IVMHost::finish_tx(host).expect("finish_tx after restore");
        let stored = host
            .wsv
            .sc_get("counter")
            .expect("state after rollback should exist");
        let tlv = validate_tlv_bytes(&stored).expect("stored TLV");
        assert_eq!(tlv.payload, b"1");
    }
}

#[test]
fn overlay_flush_errors_surface_and_reset_overlay() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "ivm_overlay_flush_err_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    fs::create_dir_all(&tmp_dir).expect("tmp dir");
    let blocker = tmp_dir.join("blocker");
    fs::write(&blocker, b"block").expect("blocker file");
    let persist_path = blocker.join("state.json");

    let wsv =
        MockWorldStateView::with_state_store(persist_path).expect("persisted mock WSV available");
    let mut vm = IVM::new(u64::MAX);
    let host = WsvHost::new_with_subject(wsv, sample_account(), HashMap::new());
    vm.set_host(host);

    let p_path = make_tlv(PointerType::Name, b"counter");
    let p_val = make_tlv(PointerType::NoritoBytes, b"5");
    let program = set_and_get_program();

    {
        let host = vm
            .host_mut_any()
            .expect("host present")
            .downcast_mut::<WsvHost>()
            .expect("WsvHost");
        IVMHost::begin_tx(host, &Default::default()).expect("begin_tx");
    }
    let p_path_ptr = vm.alloc_input_tlv(&p_path).expect("alloc path");
    let p_val_ptr = vm.alloc_input_tlv(&p_val).expect("alloc val");
    vm.set_register(10, p_path_ptr);
    vm.set_register(11, p_val_ptr);
    vm.load_program(&program).expect("load program");
    vm.run().expect("execute overlay program");

    {
        let host = vm
            .host_mut_any()
            .expect("host present")
            .downcast_mut::<WsvHost>()
            .expect("WsvHost");
        assert!(
            host.wsv.sc_get("counter").is_none(),
            "flush should not occur before finish_tx"
        );
        let finish_err = IVMHost::finish_tx(host);
        assert!(
            finish_err.is_err(),
            "finish_tx should return the flush error"
        );
        assert!(
            host.wsv.sc_get("counter").is_none(),
            "state should stay unflushed after error"
        );
        let retry = IVMHost::finish_tx(host);
        assert!(
            retry.is_ok(),
            "finish_tx should clear overlay and become idempotent after errors"
        );
    }

    let _ = fs::remove_dir_all(&tmp_dir);
}
