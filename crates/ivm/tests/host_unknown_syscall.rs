use iroha_data_model::nexus::{DataSpaceId, LaneId};
use ivm::{
    IVM, IVMHost, PointerType, VMError,
    axt::{
        self, AssetHandle, GroupBinding, HandleBudget, HandleSubject, ProofBlob, RemoteSpendIntent,
        SpendOp, TouchManifest,
    },
    host::DefaultHost,
};

#[test]
fn default_host_unknown_syscall_returns_unknown() {
    let mut vm = IVM::new(1000);
    let mut host = DefaultHost::new();
    match host.syscall(0xAB, &mut vm) {
        Err(VMError::UnknownSyscall(n)) => assert_eq!(n, 0xAB),
        other => panic!("expected UnknownSyscall, got {other:?}"),
    }
}

fn make_tlv(ty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(ty as u16).to_be_bytes());
    tlv.push(1);
    tlv.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    tlv.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&h);
    tlv
}

fn store_tlv(vm: &mut IVM, ty: PointerType, value: &[u8]) -> u64 {
    let tlv = make_tlv(ty, value);
    vm.alloc_input_tlv(&tlv).expect("alloc input")
}

#[test]
fn default_host_axt_syscalls_handle_valid_sequence() {
    let mut vm = IVM::new(1_000_000);
    let mut host = DefaultHost::new();

    let dsid = DataSpaceId::new(21);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv(
        &mut vm,
        PointerType::AxtDescriptor,
        &norito::to_bytes(&descriptor).expect("encode descriptor"),
    );
    vm.set_register(10, desc_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Ok(0)
    );

    let ds_ptr = store_tlv(
        &mut vm,
        PointerType::DataSpaceId,
        &norito::to_bytes(&dsid).expect("encode dsid"),
    );
    let manifest = TouchManifest {
        read: vec!["orders/123".into()],
        write: vec!["ledger/123".into()],
    };
    let manifest_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&manifest).expect("encode manifest"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Ok(0)
    );

    let proof = ProofBlob {
        payload: vec![9, 9, 9],
        expiry_slot: None,
    };
    let proof_ptr = store_tlv(
        &mut vm,
        PointerType::ProofBlob,
        &norito::to_bytes(&proof).expect("encode proof"),
    );
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Ok(0)
    );

    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: "alice@wonderland".into(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: 200,
            per_use: Some(150),
        },
        handle_era: 1,
        sub_nonce: 5,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: vec![0; 32],
        expiry_slot: 10,
        max_clock_skew_ms: Some(0),
    };
    let handle_ptr = store_tlv(
        &mut vm,
        PointerType::AssetHandle,
        &norito::to_bytes(&handle).expect("encode handle"),
    );
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            kind: "transfer".into(),
            from: "alice@wonderland".into(),
            to: "merchant@wonderland".into(),
            amount: "100".into(),
        },
    };
    let intent_ptr = store_tlv(
        &mut vm,
        PointerType::NoritoBytes,
        &norito::to_bytes(&intent).expect("encode intent"),
    );
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Ok(0)
    );

    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Ok(0)
    );

    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}
